//! `cargo xtask bench`: measure xdevcmd the same way every time, and keep the numbers
//! (Round 032, the owner's request: each new feature compared with a baseline, not a
//! guess).
//!
//! ```text
//! cargo xtask bench [--quick] [--runs N] [--host NAME] [--save-baseline]
//! ```
//!
//! - Builds the release binary, then runs each case `--runs` times (5 by default) after
//!   one warm-up run, and prints the median, p95 and best, with the change from this
//!   host's baseline (marked past ±15 %).
//! - The cases that call xdevcmd-core (listing, the planner, copies, undo) each run in a
//!   child process of this one, so the peak memory printed is that case's own.
//! - Start-up runs the real program in a pseudo-terminal, twice: one that answers the
//!   terminal's questions at once (a local terminal) and one that answers nothing (a
//!   silent terminal, a slow SSH link). The numbers are xdevcmd's own `--timings`.
//! - Results go to `bench/results/<version>-<host>-<date>[-quick].json`; with
//!   `--save-baseline` also to `bench/baseline-<host>[-quick].json`. `--host` is a name
//!   you choose for the machine: nothing is read from the system to name it.
//! - Every tree is made from a fixed recipe in a scratch directory, never your files.
//!   Numbers from two machines are not comparable; nor are runs on a busy machine (the
//!   load is printed first).
//!
//! Not a CI gate: shared runners vary by tens of per cent. CI keeps hard budgets only
//! (`crates/xdevcmd/tests/budgets.rs`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

const USAGE: &str = "usage: cargo xtask bench [--quick] [--runs N] [--host NAME] [--save-baseline]";
/// Past this change from the baseline, a case is marked.
const MARK: f64 = 15.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("bench") => bench(&args[1..]),
        // A case run in a child: `bench-case NAME SIZE DIR`, its result as JSON.
        Some("bench-case") if args.len() == 4 => {
            let size = args[2].parse().unwrap_or(0);
            match run_case(&args[1], size, Path::new(&args[3])) {
                Ok(ms) => {
                    println!("{}", json!({ "ms": ms, "rss_kib": peak_rss_kib() }));
                    0
                }
                Err(e) => {
                    eprintln!("{}: {e}", args[1]);
                    1
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

struct Opts {
    quick: bool,
    runs: usize,
    host: String,
    save_baseline: bool,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        quick: false,
        runs: 5,
        host: "unnamed".into(),
        save_baseline: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--quick" => o.quick = true,
            "--save-baseline" => o.save_baseline = true,
            "--runs" => {
                o.runs = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|n| *n > 0)
                    .ok_or("--runs needs a number above 0")?
            }
            "--host" => {
                let h = it.next().ok_or("--host needs a name")?;
                if h.is_empty()
                    || !h
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
                {
                    return Err("--host: letters, digits, '.', '_' and '-' only".into());
                }
                o.host = h.clone();
            }
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    if o.save_baseline && o.host == "unnamed" {
        return Err("--save-baseline needs --host NAME: a baseline is per machine".into());
    }
    Ok(o)
}

/// One case's numbers over its runs.
struct Result_ {
    unit: &'static str,
    values: Vec<f64>,
    rss_kib: Option<u64>,
}

impl Result_ {
    fn sorted(&self) -> Vec<f64> {
        let mut v = self.values.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        v
    }
    fn median(&self) -> f64 {
        let v = self.sorted();
        match v.len() {
            0 => f64::NAN,
            n if n % 2 == 1 => v[n / 2],
            n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
        }
    }
    /// Nearest rank.
    fn p95(&self) -> f64 {
        let v = self.sorted();
        if v.is_empty() {
            return f64::NAN;
        }
        let rank = ((0.95 * v.len() as f64).ceil() as usize).clamp(1, v.len());
        v[rank - 1]
    }
    fn best(&self) -> f64 {
        self.sorted().first().copied().unwrap_or(f64::NAN)
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace")
        .to_path_buf()
}

fn bench(args: &[String]) -> i32 {
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let root = root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    eprintln!("building the release binary…");
    let built = Command::new(&cargo)
        .args(["build", "--release", "--locked", "-p", "xdevcmd"])
        .current_dir(&root)
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("cargo build --release failed");
        return 1;
    }
    let bin = root.join("target/release").join(if cfg!(windows) {
        "xdevcmd.exe"
    } else {
        "xdevcmd"
    });
    let version = workspace_version(&root).unwrap_or_else(|| "unknown".into());
    let date = today();
    println!(
        "xdevcmd {version} · host {} · {date} · {} · {} runs{}",
        o.host,
        std::env::consts::OS,
        o.runs,
        if o.quick { " · quick" } else { "" }
    );
    if let Some(load) = load() {
        println!("load average {load} (numbers from a busy machine are not comparable)");
    }

    let scratch = std::env::temp_dir().join(format!("xdevcmd-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("a scratch directory");

    let mut results: BTreeMap<String, Result_> = BTreeMap::new();
    // Package: measured once.
    if let Ok(m) = std::fs::metadata(&bin) {
        results.insert(
            "package: binary".into(),
            Result_ {
                unit: "MiB",
                values: vec![m.len() as f64 / 1_048_576.0],
                rss_kib: None,
            },
        );
    }
    if let Ok(lock) = std::fs::read_to_string(root.join("Cargo.lock")) {
        results.insert(
            "package: crates in Cargo.lock".into(),
            Result_ {
                unit: "",
                values: vec![lock.matches("[[package]]").count() as f64],
                rss_kib: None,
            },
        );
    }

    // In-process cases: (name shown, case, size quick, size full).
    let cases: &[(&str, &str, u64, u64)] = &[
        ("list 10,000", "list", 10_000, 10_000),
        ("list 100,000", "list", 0, 100_000),
        ("plan 10,000", "plan", 10_000, 10_000),
        ("plan 100,000", "plan", 0, 100_000),
        ("plan 200,000 (the cap)", "plan", 0, 200_000),
        ("copy small files", "copy-small", 2_000, 10_000),
        ("copy one large file", "copy-big", 32, 256),
        // Each replaced file is synced before its rename (Round 029), then its old
        // version goes to the trash (Round 031): mostly the sync's cost.
        (
            "replace small files (synced, old to the trash)",
            "replace-trash",
            1_000,
            5_000,
        ),
        ("undo a copy", "undo", 2_000, 10_000),
    ];
    let me = std::env::current_exe().expect("this program's path");
    for (shown, case, quick, full) in cases {
        let size = if o.quick { *quick } else { *full };
        if size == 0 {
            continue;
        }
        let name = shown.to_string();
        let mut r = Result_ {
            unit: "ms",
            values: Vec::new(),
            rss_kib: None,
        };
        eprint!("{name}: ");
        for run in 0..=o.runs {
            let dir = scratch.join(format!("{case}-{run}"));
            let out = Command::new(&me)
                .args(["bench-case", case, &size.to_string()])
                .arg(&dir)
                .output();
            let _ = std::fs::remove_dir_all(&dir);
            let Some(v) = out
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
            else {
                eprint!("✗ ");
                continue;
            };
            eprint!(".");
            // The first run warms the caches and is not counted.
            if run == 0 {
                continue;
            }
            if let Some(ms) = v["ms"].as_f64() {
                r.values.push(ms);
            }
            if let Some(kib) = v["rss_kib"].as_u64() {
                r.rss_kib = Some(r.rss_kib.unwrap_or(0).max(kib));
            }
        }
        eprintln!();
        results.insert(name, r);
    }

    // Start-up and idle CPU: the real program in a pseudo-terminal.
    #[cfg(unix)]
    for (answering, label) in [(true, "answering"), (false, "silent")] {
        let mut total = Result_ {
            unit: "ms",
            values: Vec::new(),
            rss_kib: None,
        };
        let mut check = Result_ {
            unit: "ms",
            values: Vec::new(),
            rss_kib: None,
        };
        let mut idle = Result_ {
            unit: "% cpu",
            values: Vec::new(),
            rss_kib: None,
        };
        eprint!("start-up ({label} terminal): ");
        for run in 0..=o.runs {
            let dir = scratch.join(format!("start-{label}-{run}"));
            let idle_for = (answering && run == 1 && cfg!(target_os = "linux"))
                .then_some(Duration::from_secs(if o.quick { 3 } else { 10 }));
            match pty::start(&bin, &dir, answering, idle_for) {
                Some(s) => {
                    eprint!(".");
                    if run > 0 {
                        total.values.push(s.total_ms);
                        check.values.push(s.check_ms);
                        if let Some(c) = s.idle_cpu {
                            idle.values.push(c);
                        }
                        if let Some(k) = s.rss_kib {
                            total.rss_kib = Some(total.rss_kib.unwrap_or(0).max(k));
                        }
                    }
                }
                None => eprint!("✗ "),
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        eprintln!();
        results.insert(
            format!("start-up, {label} terminal: to the first frame"),
            total,
        );
        results.insert(
            format!("start-up, {label} terminal: the terminal check"),
            check,
        );
        if !idle.values.is_empty() {
            results.insert("idle cpu, after start-up".into(), idle);
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);

    // Compare, print, keep.
    let suffix = if o.quick { "-quick" } else { "" };
    let base_path = root
        .join("bench")
        .join(format!("baseline-{}{suffix}.json", o.host));
    let base: Option<Value> = std::fs::read(&base_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    println!();
    println!(
        "{:<52} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "case", "median", "p95", "best", "peak mem", "vs base"
    );
    let mut cases_json = serde_json::Map::new();
    let mut marked = 0;
    for (name, r) in &results {
        if r.values.is_empty() {
            println!("{name:<52} {:>10}", "failed");
            continue;
        }
        let (med, p95, best) = (r.median(), r.p95(), r.best());
        let vs = base
            .as_ref()
            .and_then(|b| b["cases"][name]["median"].as_f64())
            .filter(|b| *b > 0.0)
            .map(|b| (med - b) / b * 100.0);
        let vs_text = match vs {
            Some(p) if p.abs() > MARK => {
                marked += 1;
                format!("{p:+.0}% ◀")
            }
            Some(p) => format!("{p:+.0}%"),
            None => "—".into(),
        };
        let mem = r
            .rss_kib
            .map(|k| format!("{:.1} MiB", k as f64 / 1024.0))
            .unwrap_or_else(|| "—".into());
        let f = |x: f64| {
            if x >= 100.0 {
                format!("{x:.0} {}", r.unit)
            } else {
                format!("{x:.2} {}", r.unit)
            }
        };
        println!(
            "{name:<52} {:>10} {:>10} {:>10} {mem:>9} {vs_text:>9}",
            f(med),
            f(p95),
            f(best)
        );
        cases_json.insert(
            name.clone(),
            json!({ "unit": r.unit, "median": med, "p95": p95, "best": best,
                    "runs": r.values.len(), "rss_kib": r.rss_kib }),
        );
    }
    if base.is_none() {
        println!(
            "\nno baseline for host {} yet: --save-baseline keeps this run as one",
            o.host
        );
    } else if marked > 0 {
        println!("\n{marked} case(s) moved more than {MARK}% from the baseline (◀)");
    }
    let doc = json!({
        "version": version, "host": o.host, "date": date, "os": std::env::consts::OS,
        "quick": o.quick, "runs": o.runs, "load": load(), "cases": cases_json,
    });
    let text = serde_json::to_string_pretty(&doc).expect("JSON") + "\n";
    let out_dir = root.join("bench/results");
    let _ = std::fs::create_dir_all(&out_dir);
    let out = out_dir.join(format!("{version}-{}-{date}{suffix}.json", o.host));
    match std::fs::write(&out, &text) {
        Ok(()) => println!(
            "results: {}",
            out.strip_prefix(&root).unwrap_or(&out).display()
        ),
        Err(e) => eprintln!("could not write {}: {e}", out.display()),
    }
    if o.save_baseline {
        match std::fs::write(&base_path, &text) {
            Ok(()) => println!(
                "baseline: {}",
                base_path
                    .strip_prefix(&root)
                    .unwrap_or(&base_path)
                    .display()
            ),
            Err(e) => eprintln!("could not write {}: {e}", base_path.display()),
        }
    }
    0
}

// ── the cases run in a child ──────────────────────────────────────────────────────────

/// `n` files named as a person would, in folders of 1,000, `bytes` each.
fn make_tree(dir: &Path, n: u64, bytes: usize) -> std::io::Result<()> {
    let body = vec![b'x'; bytes];
    for i in 0..n {
        let d = dir.join(format!("folder-{:03}", i / 1000));
        if i % 1000 == 0 {
            std::fs::create_dir_all(&d)?;
        }
        std::fs::write(d.join(format!("file-{i:06}.txt")), &body)?;
    }
    Ok(())
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Run a copy job to its end, answering every question "replace".
fn run_job(h: xdevcmd_core::ops::Handle) -> Result<xdevcmd_core::Summary, String> {
    use xdevcmd_core::ops::{Answer, Event};
    loop {
        match h.events.recv_timeout(Duration::from_secs(600)) {
            Ok(Event::Ask(_)) => h.answer(Answer::YesToAll),
            Ok(Event::Done(s)) if s.failed == 0 => return Ok(s),
            Ok(Event::Done(s)) => {
                return Err(format!("{} failed: {:?}", s.failed, s.errors.first()))
            }
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn run_case(case: &str, size: u64, dir: &Path) -> Result<f64, String> {
    use xdevcmd_core::ops::{self, Existing, Request};
    let e = |e: std::io::Error| e.to_string();
    let _ = std::fs::remove_dir_all(dir);
    let (src, to) = (dir.join("src"), dir.join("to"));
    std::fs::create_dir_all(&src).map_err(e)?;
    std::fs::create_dir_all(&to).map_err(e)?;
    // Safe operations' journal, as the app keeps it (synced before each replace).
    xdevcmd_core::journal::init(dir.join("journal"));
    match case {
        "list" => {
            // One directory of `size` entries: the 100,000-entry budget's shape.
            for i in 0..size {
                std::fs::write(src.join(format!("file-{i:06}.txt")), b"").map_err(e)?;
            }
            let t = Instant::now();
            let mut v = xdevcmd_core::read_dir(&src, false).map_err(e)?;
            xdevcmd_core::sort_entries(&mut v, xdevcmd_core::Sort::default());
            let took = ms(t);
            if v.len() as u64 != size + 1 {
                return Err(format!("listed {}", v.len()));
            }
            Ok(took)
        }
        "plan" => {
            make_tree(&src, size, 0).map_err(e)?;
            let t = Instant::now();
            let p = xdevcmd_core::plan::start(ops::Kind::Copy, vec![src.clone()], to.clone(), None);
            loop {
                match p.rx.recv_timeout(Duration::from_secs(600)) {
                    Ok(xdevcmd_core::plan::Msg::Done(_)) => return Ok(ms(t)),
                    Ok(_) => {}
                    Err(err) => return Err(err.to_string()),
                }
            }
        }
        "copy-small" | "copy-big" | "undo" => {
            if case == "copy-big" {
                let mb = vec![b'x'; 1 << 20];
                let mut f = std::fs::File::create(src.join("large.bin")).map_err(e)?;
                for _ in 0..size {
                    std::io::Write::write_all(&mut f, &mb).map_err(e)?;
                }
            } else {
                make_tree(&src, size, 1024).map_err(e)?;
            }
            let mut req = Request::copy(vec![src.clone()], to.clone());
            // Whatever the user's setting, the default's way of writing.
            req.safe = true;
            let t = Instant::now();
            let s = run_job(ops::start(req))?;
            if case != "undo" {
                return Ok(ms(t));
            }
            let t = Instant::now();
            let u = run_job(xdevcmd_core::undo::unmake(s.made))?;
            if u.done < size {
                return Err(format!("undo removed {}", u.done));
            }
            Ok(ms(t))
        }
        "replace-trash" => {
            make_tree(&src, size, 1024).map_err(e)?;
            make_tree(&to.join("src"), size, 512).map_err(e)?;
            let mut req = Request::copy(vec![src.clone()], to.clone());
            req.safe = true;
            req.existing = Existing::Replace;
            // A trash of its own in the scratch directory: never the real one.
            req.trash_root = Some(dir.join("trash"));
            let t = Instant::now();
            let s = run_job(ops::start(req))?;
            if s.replaced_to_trash < size {
                return Err(format!("{} went to the trash", s.replaced_to_trash));
            }
            Ok(ms(t))
        }
        other => Err(format!("no case {other}")),
    }
}

/// This process's peak resident memory, where it can be read without unsafe code.
fn peak_rss_kib() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        status_kib("/proc/self/status", "VmHWM:")
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn status_kib(path: &str, key: &str) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .find(|l| l.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

// ── start-up in a pseudo-terminal ────────────────────────────────────────────────────

#[cfg(unix)]
mod pty {
    use super::*;
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    pub struct Start {
        pub total_ms: f64,
        pub check_ms: f64,
        pub rss_kib: Option<u64>,
        pub idle_cpu: Option<f64>,
    }

    fn has(buf: &Mutex<Vec<u8>>, needle: &[u8]) -> bool {
        buf.lock()
            .unwrap()
            .windows(needle.len())
            .any(|w| w == needle)
    }

    /// Start xdevcmd with `--timings` in a fresh pseudo-terminal, `answering` (or not)
    /// its question about pictures; once the screen is up, optionally measure its CPU
    /// while idle for `idle_for`; end it with SIGTERM and read its timings.
    pub fn start(
        bin: &Path,
        dir: &Path,
        answering: bool,
        idle_for: Option<Duration>,
    ) -> Option<Start> {
        std::fs::create_dir_all(dir.join("a")).ok()?;
        std::fs::create_dir_all(dir.join("b")).ok()?;
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 30,
                cols: 110,
                pixel_width: 0,
                pixel_height: 0,
            })
            .ok()?;
        let mut cmd = CommandBuilder::new(bin);
        cmd.args(["--timings", "--no-restore"]);
        cmd.arg(dir.join("a"));
        cmd.arg(dir.join("b"));
        cmd.env("XDEVCMD_CONFIG", dir.join("none.toml"));
        for (k, sub) in [
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CONFIG_HOME", "config"),
        ] {
            cmd.env(k, dir.join(sub));
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env_remove("TMUX");
        cmd.env_remove("STY");
        let mut child = pair.slave.spawn_command(cmd).ok()?;
        drop(pair.slave);
        let pid = child.process_id()?;
        let mut reader = pair.master.try_clone_reader().ok()?;
        let mut writer = pair.master.take_writer().ok()?;
        let buf = Arc::new(Mutex::new(Vec::new()));
        let sink = buf.clone();
        std::thread::spawn(move || {
            let mut b = [0u8; 8192];
            while let Ok(n) = reader.read(&mut b) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&b[..n]);
            }
        });
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut answered = !answering;
        while !has(&buf, b"Quit") {
            if Instant::now() > deadline {
                let _ = child.kill();
                return None;
            }
            // A terminal's reply to the device attributes question (CSI c).
            if !answered && has(&buf, b"\x1b[c") {
                let _ = writer.write_all(b"\x1b[?62;22c");
                let _ = writer.flush();
                answered = true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let rss_kib = rss_of(pid);
        let idle_cpu = idle_for.and_then(|d| idle(pid, d));
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(50));
        let text = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
        let json = text.split("xdevcmd startup ").nth(1)?;
        let json = &json[..=json.find('}')?];
        let v: Value = serde_json::from_str(json).ok()?;
        Some(Start {
            total_ms: v["total_ms"].as_f64()?,
            check_ms: v["terminal_check_ms"].as_f64().unwrap_or(0.0),
            rss_kib,
            idle_cpu,
        })
    }

    fn rss_of(pid: u32) -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            status_kib(&format!("/proc/{pid}/status"), "VmRSS:")
        }
        #[cfg(not(target_os = "linux"))]
        {
            let out = Command::new("ps")
                .args(["-o", "rss=", "-p", &pid.to_string()])
                .output()
                .ok()?;
            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
        }
    }

    /// CPU used while idle over `d`, in per cent of one core (Linux: /proc/<pid>/stat).
    fn idle(pid: u32, d: Duration) -> Option<f64> {
        let ticks = || -> Option<u64> {
            let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            // After the command name in parentheses: field 14 utime, 15 stime.
            let rest = &s[s.rfind(')')? + 2..];
            let f: Vec<&str> = rest.split(' ').collect();
            Some(f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?)
        };
        let hz: f64 = Command::new("getconf")
            .arg("CLK_TCK")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(100.0);
        std::thread::sleep(Duration::from_secs(1));
        let a = ticks()?;
        std::thread::sleep(d);
        let b = ticks()?;
        Some((b - a) as f64 / hz / d.as_secs_f64() * 100.0)
    }
}

// ── small things ─────────────────────────────────────────────────────────────────────

fn workspace_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let section = &text[text.find("[workspace.package]")?..];
    let line = section
        .lines()
        .find(|l| l.trim_start().starts_with("version"))?;
    Some(line.split('"').nth(1)?.to_string())
}

/// The machine's load, where the system says it cheaply.
fn load() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/loadavg").ok()?;
        Some(s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Today in UTC, YYYY-MM-DD, without a date crate (days to a civil date).
fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's days-from-civil, inverted.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_p95_and_best() {
        let r = Result_ {
            unit: "ms",
            values: vec![5.0, 1.0, 3.0, 2.0, 4.0],
            rss_kib: None,
        };
        assert_eq!((r.median(), r.p95(), r.best()), (3.0, 5.0, 1.0));
        let r = Result_ {
            unit: "ms",
            values: vec![4.0, 1.0],
            rss_kib: None,
        };
        assert_eq!(r.median(), 2.5);
    }

    #[test]
    fn dates_from_days() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_405), (2025, 11, 13));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    #[test]
    fn a_host_name_is_checked_and_a_baseline_needs_one() {
        let a = |v: &[&str]| parse(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(a(&["--host", "my laptop"]).is_err());
        assert!(a(&["--save-baseline"]).is_err());
        let o = a(&["--quick", "--host", "dev-01", "--runs", "3"]).unwrap();
        assert!(o.quick && o.runs == 3 && o.host == "dev-01");
    }
}
