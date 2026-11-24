//! `xtask manual`: the data-safety hand tests, from a tester's kit (plan v3 P5, M1).
//!
//! ```text
//! xtask manual list                 the cases, with what `xtask try` does for each
//! xtask manual show M-DS-NN         one case: setup, steps, expected, fail if
//! xtask manual log                guided: the cases, their results, a note; the
//!                                   newest saved `xtask try --check`, terminal and
//!                                   shell filled in
//! xtask manual log M-DS-NN RESULT [--check FILE] [--evidence FILE] [--issue N]
//!                  [--note TEXT] [--terminal T] [--shell S] [--to FILE]
//!                                   one row for runs.tsv, checked as `qa ledger` will
//! ```
//!
//! The cases are built in, so this works from the kit with no repository. A row is
//! appended to the repository's `runs.tsv` when run inside it, else to `runs.tsv` in
//! the current folder (made with its header), to commit later. `built_from` is the
//! commit this xtask was built from: use the xtask that came with the xdevcmd you test.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::qa::{self, COLUMNS, DISK, RUNS};

const CASES_MD: &str = include_str!("../../crates/xdevcmd/tests/manual/test-cases/data-safety.md");

/// The commit this xtask was built from (`build.rs`).
pub const COMMIT: &str = env!("XTASK_COMMIT");

/// What `xtask try` checks for the disk cases (docs/how-to-test.md, checks A–C).
fn with_try(case: &str) -> Option<&'static str> {
    match case {
        "M-DS-01" => Some("xtask try --make; --launch; --kill-when A; start again; --check"),
        "M-DS-03" => Some("xtask try --make; --launch; --kill-when B; start again; --check"),
        "M-DS-04" => Some("xtask try --make; --launch; --kill-when C; C at the dialog; --check"),
        _ => None,
    }
}

const USAGE: &str = "usage: xtask manual list
       xtask manual show M-DS-NN
       xtask manual log            (guided: asks for the cases, results and a note)
       xtask manual log M-DS-NN PASS|FAIL|WAIVED [--check FILE] [--evidence FILE]
                        [--issue N] [--note TEXT] [--terminal T] [--shell S] [--to FILE]";

pub fn run(args: &[String], version: &str) -> i32 {
    match args.first().map(String::as_str) {
        Some("list") => {
            print!("{}", list(CASES_MD));
            0
        }
        Some("show") => match args.get(1).and_then(|id| show(CASES_MD, id)) {
            Some(text) => {
                println!("{text}");
                0
            }
            None => {
                eprintln!("manual show: no such case; `xtask manual list` names them");
                2
            }
        },
        Some("log") if args.len() == 1 => {
            let here = std::env::current_dir().unwrap_or_default();
            let dir = crate::tryout::sandbox_in(&here)
                .map_or(here, |root| crate::tryout::checks_dir(&root));
            let stdin = std::io::stdin();
            let mut out = std::io::stdout();
            match ask_rows(CASES_MD, &mut stdin.lock(), &mut out, &dir, &detected()) {
                Ok(rows) => {
                    let mut bad = 0;
                    for a in &rows {
                        match log(a, version) {
                            Ok(msg) => println!("{msg}"),
                            Err(e) => {
                                bad += 1;
                                eprintln!("manual log {}: {e}", a.case);
                            }
                        }
                    }
                    i32::from(bad > 0) * 2
                }
                Err(e) => {
                    eprintln!("manual log: {e}");
                    2
                }
            }
        }
        Some("log") if matches!(args.get(1).map(String::as_str), Some("--help" | "-h")) => {
            println!("{USAGE}");
            0
        }
        Some("log") => match log_args(&args[1..]) {
            Ok(a) => match log(&a, version) {
                Ok(msg) => {
                    println!("{msg}");
                    0
                }
                Err(e) => {
                    eprintln!("manual log: {e}");
                    2
                }
            },
            Err(e) => {
                eprintln!("manual log: {e}\n{USAGE}");
                2
            }
        },
        Some("--help" | "-h") => {
            println!("{USAGE}");
            0
        }
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

/// Each case: its ID, title and what `xtask try` does for it.
fn list(md: &str) -> String {
    let mut s = String::new();
    for line in md.lines().filter(|l| l.starts_with("### M-DS-")) {
        let rest = &line[4..];
        let (id, title) = rest.split_once(' ').unwrap_or((rest, ""));
        s.push_str(&format!("{id}  {title}\n"));
        if let Some(t) = with_try(id) {
            s.push_str(&format!("         with xtask: {t}\n"));
        }
    }
    s.push_str("Then: xtask manual log M-DS-NN PASS|FAIL|WAIVED …  (xtask manual log --help)\n");
    s
}

/// One case's text, from its heading to the next.
fn show(md: &str, id: &str) -> Option<String> {
    let start = md.find(&format!("### {id} "))?;
    let rest = &md[start..];
    let end = rest[4..].find("\n### ").map_or(rest.len(), |e| e + 4);
    let mut text = rest[..end].trim_end().to_string();
    if let Some(t) = with_try(id) {
        text.push_str(&format!("\n\nWith xtask: {t}"));
    }
    Some(text)
}

#[derive(Debug, Default, PartialEq)]
struct LogArgs {
    case: String,
    result: String,
    check: Option<PathBuf>,
    evidence: Option<String>,
    issue: Option<String>,
    note: Option<String>,
    terminal: Option<String>,
    shell: Option<String>,
    to: Option<PathBuf>,
}

fn log_args(args: &[String]) -> Result<LogArgs, String> {
    let mut a = LogArgs::default();
    let mut it = args.iter();
    a.case = it.next().ok_or("which case?")?.clone();
    a.result = it
        .next()
        .ok_or("which result: PASS, FAIL or WAIVED?")?
        .to_ascii_uppercase();
    while let Some(flag) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--check" => a.check = Some(PathBuf::from(val()?)),
            "--evidence" => a.evidence = Some(val()?),
            "--issue" => a.issue = Some(val()?),
            "--note" => a.note = Some(val()?),
            "--terminal" => a.terminal = Some(val()?),
            "--shell" => a.shell = Some(val()?),
            "--to" => a.to = Some(PathBuf::from(val()?)),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

/// The `xtask try --check` area a disk case is judged by (docs/how-to-test.md).
fn area_of(case: &str) -> Option<&'static str> {
    match case {
        "M-DS-01" => Some("(check A)"),
        "M-DS-03" => Some("(check B)"),
        "M-DS-04" => Some("(check C)"),
        _ => None,
    }
}

/// What `xtask try --check` said for `case`, from its saved output. Any FAIL anywhere
/// is a FAIL (a lost file is a lost file); else the case's own area (check A, B or C),
/// with the sources unchanged; for any other case, the summary.
fn check_said(text: &str, case: &str) -> Option<&'static str> {
    let state = |l: &str| -> Option<&'static str> {
        let w = l.split_whitespace().collect::<Vec<_>>();
        match w.as_slice() {
            ["PASS", ..] => Some("PASS"),
            ["FAIL", ..] | ["FAIL:", ..] => Some("FAIL"),
            ["NOT", "DONE", ..] => Some("NOT DONE"),
            _ => None,
        }
    };
    if text.lines().any(|l| state(l) == Some("FAIL")) {
        return Some("FAIL");
    }
    let of = |what: &str| text.lines().find(|l| l.contains(what)).and_then(state);
    match area_of(case) {
        Some(area) => match (of("sources unchanged"), of(area)) {
            (Some("PASS"), Some(s)) => Some(s),
            (Some(_), Some(_)) => Some("NOT DONE"),
            _ => None,
        },
        None => text.lines().rev().find_map(|l| {
            if l.starts_with("PASS:") {
                Some("PASS")
            } else if l.starts_with("no file lost or wrong so far") {
                Some("NOT DONE")
            } else {
                None
            }
        }),
    }
}

/// The terminal and shell this runs in, as well as the environment tells.
fn detected() -> (String, String) {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let terminal = match env("TERM_PROGRAM").as_str() {
        "Apple_Terminal" => "Terminal.app".to_string(),
        "" if !env("WT_SESSION").is_empty() => "Windows Terminal".into(),
        "" => env("TERM"),
        t => t.to_string(),
    };
    // PowerShell 7 puts its own modules first; cmd and Windows PowerShell do not.
    let shell = if cfg!(windows) && env("PSModulePath").contains("PowerShell\\7") {
        "pwsh".to_string()
    } else {
        let s = env("SHELL");
        let s = if s.is_empty() { env("ComSpec") } else { s };
        let name = s.rsplit(['/', '\\']).next().unwrap_or("").to_string();
        name.strip_suffix(".exe")
            .map_or(name.clone(), str::to_string)
    };
    (terminal, shell)
}

/// The newest `check-*.txt` in `dir`: what `xtask try --check` saved last.
fn newest_check(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("check-") && n.ends_with(".txt")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max()
        .map(|(_, p)| p)
}

/// One line of an answer; the end of input is an empty answer.
fn answer(
    input: &mut impl std::io::BufRead,
    out: &mut impl Write,
    q: &str,
) -> Result<String, String> {
    write!(out, "{q}")
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())?;
    let mut l = String::new();
    input.read_line(&mut l).map_err(|e| e.to_string())?;
    Ok(l.trim().to_string())
}

/// The guided `manual log`: which cases, each one's result (a waiver's reason), one
/// note, the terminal and shell (`found`, to confirm). A disk case takes the newest
/// saved check in `dir`, and its result is offered from it. Nothing is written here.
fn ask_rows(
    cases_md: &str,
    input: &mut impl std::io::BufRead,
    out: &mut impl Write,
    dir: &Path,
    found: &(String, String),
) -> Result<Vec<LogArgs>, String> {
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let ids = qa::case_ids(cases_md)?;
    let title = |id: &str| {
        cases_md
            .lines()
            .find_map(|l| l.strip_prefix(&format!("### {id} ")))
            .unwrap_or("")
            .to_string()
    };
    let check = newest_check(dir);
    let text = check.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
    let said = |id: &str| text.as_deref().and_then(|t| check_said(t, id));
    match &check {
        Some(p) => {
            w(out, format!("Check file: {}", p.display()))?;
            for id in ids.iter().filter(|i| area_of(i).is_some()) {
                w(
                    out,
                    format!(
                        "  {id} {}: {}",
                        area_of(id).unwrap_or(""),
                        said(id).unwrap_or("?")
                    ),
                )?;
            }
        }
        // Only where a case is judged by `xtask try --check` (none since Round 036: CI
        // runs M-DS-01, 03 and 04).
        None if ids.iter().any(|i| area_of(i).is_some()) => w(
            out,
            format!(
                "No check-*.txt in {}: run `xtask try --check` first for M-DS-01/03/04.",
                dir.display()
            ),
        )?,
        None => {}
    }
    for (n, id) in ids.iter().enumerate() {
        w(out, format!("  {}  {id}  {}", n + 1, title(id)))?;
    }
    // Offered: the disk cases the check says were done here.
    let offered: Vec<usize> = ids
        .iter()
        .enumerate()
        .filter(|(_, id)| matches!(said(id), Some("PASS" | "FAIL")) && area_of(id).is_some())
        .map(|(n, _)| n + 1)
        .collect();
    let offer = offered
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let q = if offer.is_empty() {
        "Cases (numbers, e.g. 1,3,4): ".to_string()
    } else {
        format!("Cases (numbers, e.g. 1,3,4; Enter = {offer}): ")
    };
    let got = answer(input, out, &q)?;
    let got = if got.is_empty() { offer } else { got };
    let mut chosen = Vec::new();
    for t in got.split([',', ' ']).filter(|t| !t.is_empty()) {
        let id = t
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| ids.get(n))
            .or_else(|| ids.iter().find(|i| i.eq_ignore_ascii_case(t)))
            .ok_or(format!("{t} is not a case: 1-{}", ids.len()))?;
        if !chosen.contains(id) {
            chosen.push(id.clone());
        }
    }
    if chosen.is_empty() {
        return Err("no case chosen".into());
    }
    let mut rows = Vec::new();
    for id in &chosen {
        let default = match said(id) {
            Some(r @ ("PASS" | "FAIL")) if area_of(id).is_some() => Some(r),
            _ => None,
        };
        let q = match default {
            Some(r) => format!("{id} {} [P/F/W; Enter = {r}]: ", title(id)),
            None => format!("{id} {} [P/F/W]: ", title(id)),
        };
        let mut result = None;
        for _ in 0..3 {
            let a = answer(input, out, &q)?.to_ascii_uppercase();
            result = match (a.as_str(), default) {
                ("", Some(r)) => Some(r.to_string()),
                ("P" | "PASS", _) => Some("PASS".into()),
                ("F" | "FAIL", _) => Some("FAIL".into()),
                ("W" | "WAIVED", _) => Some("WAIVED".into()),
                _ => None,
            };
            if result.is_some() {
                break;
            }
            w(out, "  P, F or W".into())?;
        }
        let result = result.ok_or(format!("{id}: no result"))?;
        if area_of(id).is_some() && result == "PASS" && check.is_none() {
            w(
                out,
                format!("  {id} skipped: a PASS needs `xtask try --check` saved first"),
            )?;
            continue;
        }
        let mut a = LogArgs {
            case: id.clone(),
            result,
            ..LogArgs::default()
        };
        if a.result == "WAIVED" {
            let why = answer(input, out, "  why waived: ")?;
            if why.is_empty() {
                w(out, format!("  {id} skipped: a waiver needs its reason"))?;
                continue;
            }
            a.note = Some(why);
        }
        if area_of(id).is_some() {
            a.check.clone_from(&check);
        }
        rows.push(a);
    }
    if rows.is_empty() {
        return Err("nothing to write".into());
    }
    let note = answer(input, out, "Note for these rows (Enter = none): ")?;
    let t = answer(input, out, &format!("Terminal [{}]: ", found.0))?;
    let sh = answer(input, out, &format!("Shell [{}]: ", found.1))?;
    for a in &mut rows {
        if !note.is_empty() {
            a.note = Some(match a.note.take() {
                Some(why) => format!("{why}; {note}"),
                None => note.clone(),
            });
        }
        a.terminal = Some(if t.is_empty() {
            found.0.clone()
        } else {
            t.clone()
        });
        a.shell = Some(if sh.is_empty() {
            found.1.clone()
        } else {
            sh.clone()
        });
    }
    Ok(rows)
}

pub fn os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// A field for a tab-separated row: no tab or line break inside, "-" when empty.
fn field(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect();
    if s.trim().is_empty() {
        "-".into()
    } else {
        s.trim().to_string()
    }
}

/// The row for `a`, or why it would not count (refused before it is written).
fn row(
    cases_md: &str,
    a: &LogArgs,
    version: &str,
    commit: &str,
    today: &str,
    check_text: Option<&str>,
) -> Result<String, String> {
    let ids = qa::case_ids(cases_md)?;
    if !ids.contains(&a.case) {
        return Err(format!("{} is not a case: {}", a.case, ids.join(", ")));
    }
    if !matches!(a.result.as_str(), "PASS" | "FAIL" | "WAIVED") {
        return Err(format!("result {:?} is not PASS, FAIL or WAIVED", a.result));
    }
    if commit == "unknown" || commit.ends_with("-dirty") {
        return Err(format!(
            "this xtask was built from {commit}: a run of that build is no evidence (build from a commit, or use the release's kit)"
        ));
    }
    let check = match check_text {
        Some(t) => check_said(t, &a.case).ok_or(
            "the --check file is not `xtask try --check` output (no verdict for this case)",
        )?,
        None => "-",
    };
    let evidence = a
        .evidence
        .clone()
        .or_else(|| {
            a.check
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let (t, sh) = detected();
    let terminal = a.terminal.clone().unwrap_or(t);
    let shell = a.shell.clone().unwrap_or(sh);
    let values = [
        today.to_string(),
        commit.to_string(),
        version.to_string(),
        os().to_string(),
        field(&terminal),
        field(&shell),
        a.case.clone(),
        a.result.clone(),
        check.to_string(),
        field(&evidence),
        field(a.issue.as_deref().unwrap_or("")),
        field(a.note.as_deref().unwrap_or("")),
    ];
    // Judged now as the ledger will judge it, so a row that would not count is not
    // written as if it did.
    let map: BTreeMap<&'static str, String> = COLUMNS
        .iter()
        .copied()
        .zip(values.iter().cloned())
        .collect();
    if let qa::Verdict::Unproven(why) = qa::one(&a.case, &map) {
        return Err(format!("this row would not count: {why}"));
    }
    if DISK.contains(&a.case.as_str()) && a.result == "PASS" && a.check.is_none() {
        return Err("a disk case needs --check with `xtask try --check`'s saved output".into());
    }
    Ok(values.join("\t"))
}

fn log(a: &LogArgs, version: &str) -> Result<String, String> {
    let check_text = match &a.check {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let line = row(
        CASES_MD,
        a,
        version,
        COMMIT,
        &crate::today(),
        check_text.as_deref(),
    )?;
    let to = a.to.clone().unwrap_or_else(|| match qa::repo_root() {
        Some(r) => r.join(RUNS),
        None => PathBuf::from("runs.tsv"),
    });
    append(&to, &line)?;
    Ok(format!("{line}\nwritten to {}", to.display()))
}

/// Append `line` to `to`, with the header first when the file is new.
fn append(to: &Path, line: &str) -> Result<(), String> {
    let new = !to.exists();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(to)
        .map_err(|e| format!("{}: {e}", to.display()))?;
    if new {
        writeln!(f, "{}", COLUMNS.join("\t")).map_err(|e| e.to_string())?;
    }
    writeln!(f, "{line}").map_err(|e| format!("{}: {e}", to.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cases as they were before Round 036 moved M-DS-01, 03, 04, 05 to CI: the
    /// guided log and the row rules for a disk case are tested on them, as they would
    /// serve a disk case again.
    const SEVEN: &str = "### M-DS-01 Kill a copy that replaces files\n### M-DS-02 Pull a USB stick mid-copy\n### M-DS-03 A big copy, killed past a checkpoint\n### M-DS-04 A checked continue (C)\n### M-DS-05 Move across drives, killed\n### M-DS-06 A network share pulled mid-copy\n### M-DS-07 A hard NFS mount whose server is gone\n";

    fn args(case: &str, result: &str) -> LogArgs {
        LogArgs {
            case: case.into(),
            result: result.into(),
            ..LogArgs::default()
        }
    }

    /// The guided log (the owner, 2026-11-23: a file name and every field per case was
    /// too much to type).
    #[test]
    fn the_guided_log_offers_what_the_saved_check_did_and_asks_the_rest() {
        let d = std::env::temp_dir().join(format!("xtask-guided-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let found = ("Terminal.app".to_string(), "zsh".to_string());
        let ask = |input: &str| {
            let mut out = Vec::new();
            let r = ask_rows(SEVEN, &mut input.as_bytes(), &mut out, &d, &found);
            (r, String::from_utf8(out).unwrap())
        };
        // No check saved: a disk case cannot pass, a waiver still can.
        let (r, said) = ask("1\np\n");
        assert!(
            r.is_err() && said.contains("run `xtask try --check` first"),
            "{said}"
        );
        let (r, _) = ask("6\nw\n\n");
        assert!(r.is_err(), "a waiver with no reason");
        let rows = ask("6\nw\nno share here\n\n\nbash\n").0.unwrap();
        assert_eq!(rows.len(), 1);
        let a = &rows[0];
        assert_eq!((a.case.as_str(), a.result.as_str()), ("M-DS-06", "WAIVED"));
        assert_eq!(a.note.as_deref(), Some("no share here"));
        assert_eq!(
            (a.terminal.as_deref(), a.shell.as_deref()),
            (Some("Terminal.app"), Some("bash"))
        );
        assert!(a.check.is_none());
        // A check with A and C done, B not: Enter takes 1 and 4, their results PASS.
        std::fs::write(
            d.join("check-macos-20261123-0950.txt"),
            "PASS     sources unchanged\nPASS     copy of tree/ (check C)\nNOT DONE copy of big/ (check B)\n   8 not copied yet\nPASS     replace-src onto replace-dst (check A)\nno file lost or wrong so far\n",
        )
        .unwrap();
        let (r, said) = ask("\n\n\n5 tampers\n\n\n");
        let rows = r.unwrap();
        assert!(said.contains("Enter = 1,4"), "{said}");
        assert_eq!(
            rows.iter()
                .map(|a| (a.case.as_str(), a.result.as_str()))
                .collect::<Vec<_>>(),
            [("M-DS-01", "PASS"), ("M-DS-04", "PASS")]
        );
        for a in &rows {
            assert_eq!(
                a.check.as_deref(),
                Some(d.join("check-macos-20261123-0950.txt").as_path())
            );
            assert_eq!(a.note.as_deref(), Some("5 tampers"));
            assert_eq!(a.terminal.as_deref(), Some("Terminal.app"));
            // Each row is one the ledger counts.
            let text = std::fs::read_to_string(a.check.as_ref().unwrap()).unwrap();
            assert!(row(SEVEN, a, "0.4.0", "abcdef0", "2026-11-23", Some(&text)).is_ok());
        }
        // A wrong answer is asked again; an unknown case is refused.
        let rows = ask("4\nx\nf\n\n\n\n").0.unwrap();
        assert_eq!(rows[0].result, "FAIL");
        assert!(ask("9\n").0.is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn every_case_is_listed_and_shown() {
        let ids = qa::case_ids(CASES_MD).unwrap();
        let l = list(CASES_MD);
        for id in &ids {
            assert!(l.contains(id.as_str()), "{id}");
            let s = show(CASES_MD, id).unwrap();
            assert!(
                s.starts_with(&format!("### {id} ")) && s.contains("**Expected**"),
                "{s}"
            );
        }
        assert!(show(CASES_MD, "M-DS-99").is_none());
    }

    #[test]
    fn a_row_that_would_not_count_is_refused_before_it_is_written() {
        let pass = "PASS     sources unchanged\nPASS     replace-src onto replace-dst (check A)\nPASS: every copy whole, no file lost or wrong\n";
        let fail = "PASS     sources unchanged\nFAIL     replace-src onto replace-dst (check A)\nFAIL: a file lost, short or wrong.\n";
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let r = |a: &LogArgs, commit: &str, check: Option<&str>| {
            row(SEVEN, a, "0.4.0-beta.2", commit, "2026-11-22", check)
        };
        // A disk case passes only with --check saying PASS.
        let mut a = args("M-DS-01", "PASS");
        assert!(r(&a, sha, None).is_err(), "no --check");
        a.check = Some("check-a.txt".into());
        assert!(r(&a, sha, Some(fail)).is_err(), "--check said FAIL");
        let line = r(&a, sha, Some(pass)).unwrap();
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), COLUMNS.len());
        assert_eq!(
            (cols[1], cols[6], cols[7], cols[8], cols[9]),
            (sha, "M-DS-01", "PASS", "PASS", "check-a.txt")
        );
        // A build from uncommitted code, or of no known commit, is no evidence.
        assert!(r(&a, &format!("{sha}-dirty"), Some(pass)).is_err());
        assert!(r(&a, "unknown", Some(pass)).is_err());
        // A waiver says why; an unknown case or result is refused.
        assert!(r(&args("M-DS-06", "WAIVED"), sha, None).is_err());
        let mut w = args("M-DS-06", "WAIVED");
        w.note = Some("no NFS server\there".into());
        assert!(
            r(&w, sha, None).unwrap().ends_with("no NFS server here"),
            "a tab in a note"
        );
        assert!(r(&args("M-DS-42", "PASS"), sha, None).is_err());
        assert!(r(&args("M-DS-02", "OK"), sha, None).is_err());
        // A FAIL is written: it blocks the cut, as it should.
        assert!(r(&args("M-DS-02", "FAIL"), sha, None).is_ok());
    }

    #[test]
    fn a_new_file_gets_the_header_and_the_ledger_reads_the_row() {
        let d = std::env::temp_dir().join(format!("xtask-manual-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let to = d.join("runs.tsv");
        let line = row(
            CASES_MD,
            &args("M-DS-02", "FAIL"),
            "0.4.0",
            "abcdef0",
            "2026-11-22",
            None,
        )
        .unwrap();
        append(&to, &line).unwrap();
        append(&to, &line).unwrap();
        let text = std::fs::read_to_string(&to).unwrap();
        assert!(text.starts_with("date_utc\t"));
        let rows = qa::parse_runs(&text);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0]["case"].as_str(), rows[0]["os"].as_str()),
            ("M-DS-02", os())
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn what_try_check_said_is_read_for_the_case() {
        let out = "PASS     sources unchanged\nNOT DONE copy of tree/ (check C)\n         3 not copied yet\nPASS     copy of big/ (check B)\nPASS     replace-src onto replace-dst (check A)\nno file lost or wrong so far; what is NOT DONE is not checked yet\n";
        assert_eq!(check_said(out, "M-DS-01"), Some("PASS"));
        assert_eq!(check_said(out, "M-DS-03"), Some("PASS"));
        assert_eq!(check_said(out, "M-DS-04"), Some("NOT DONE"));
        let lost = out.replace("PASS     copy of big/", "FAIL     copy of big/");
        assert_eq!(
            check_said(&lost, "M-DS-01"),
            Some("FAIL"),
            "a lost file anywhere"
        );
        assert_eq!(check_said("nothing here\n", "M-DS-01"), None);
        assert_eq!(
            check_said("PASS: every copy whole\n", "M-DS-02"),
            Some("PASS")
        );
    }
}
