//! `xtask qa`: the maintainer's checks around a cut (plan v3 P2). Not in `--help`: it
//! needs the repository's git history, which a tester's kit does not have.
//!
//! ```text
//! xtask qa ledger [--rev R]   PASS / FAIL: is the hand-test evidence newer than the
//!                             last commit on a data path, for every case and system?
//! xtask qa defs               how many places implement each concept (consolidation)
//! ```
//!
//! Both only read: git, `crates/xdevcmd/tests/manual/`, the sources. They list; a
//! person decides.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files a hand test of data safety vouches for: a commit touching one of them
/// after the evidence was made leaves that evidence stale (lesson L7).
pub const DATA_PATH: &[&str] = &[
    "crates/xdevcmd-core/src/ops.rs",
    "crates/xdevcmd-core/src/journal.rs",
    "crates/xdevcmd-core/src/plan.rs",
    "crates/xdevcmd-core/src/fastcopy.rs",
    "crates/xdevcmd-core/src/noreplace.rs",
    "crates/xdevcmd-core/src/trashed.rs",
    "crates/xdevcmd-core/src/undo.rs",
    "crates/xdevcmd-core/src/archive.rs",
    "crates/xdevcmd-core/src/batch_rename.rs",
    "crates/xdevcmd-core/src/paths.rs",
    "crates/xdevcmd-core/src/policy.rs",
    "crates/xdevcmd-core/src/protect.rs",
    "crates/xdevcmd/src/app/jobs.rs",
    "crates/xdevcmd/src/app/file_ops.rs",
];

/// The systems every data-safety case needs evidence from, or a waiver.
pub const SYSTEMS: &[&str] = &["linux", "macos", "windows"];

/// The cases whose evidence the ledger requires, and where they are written.
const CASES: &str = "crates/xdevcmd/tests/manual/test-cases/data-safety.md";
/// One row per hand-test run (header in the file).
pub const RUNS: &str = "crates/xdevcmd/tests/manual/runs.tsv";
/// The columns of `RUNS`, in order.
pub const COLUMNS: &[&str] = &[
    "date_utc",
    "built_from",
    "version",
    "os",
    "terminal",
    "shell",
    "case",
    "result",
    "check",
    "evidence",
    "issue",
    "note",
];
/// Cases judged on disk by `xtask try --check`: a PASS needs evidence (its output or a
/// debug log) named.
const DISK: &[&str] = &["M-DS-01", "M-DS-03", "M-DS-04"];

pub fn run(args: &[String]) -> i32 {
    let root = match repo_root() {
        Some(r) => r,
        None => {
            eprintln!("qa: run it inside the repository (it reads git)");
            return 2;
        }
    };
    match args.first().map(String::as_str) {
        Some("ledger") => {
            let rev = match args.get(1).map(String::as_str) {
                Some("--rev") => match args.get(2) {
                    Some(r) => r.clone(),
                    None => return usage(),
                },
                None => "HEAD".into(),
                Some(_) => return usage(),
            };
            match ledger(&root, &rev) {
                Ok(l) => {
                    print!("{}", l.text());
                    i32::from(!l.pass())
                }
                Err(e) => {
                    eprintln!("qa ledger: {e}");
                    2
                }
            }
        }
        Some("defs") => {
            print!("{}", defs(&root));
            0
        }
        _ => usage(),
    }
}

fn usage() -> i32 {
    eprintln!("usage: xtask qa ledger [--rev R] | xtask qa defs");
    2
}

fn repo_root() -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// `a` is `b` or an ancestor of it.
fn is_ancestor(root: &Path, a: &str, b: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", a, b])
        .status()
        .is_ok_and(|s| s.success())
}

/// What the ledger found for one case on one system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Waived(String),
    Missing,
    /// Built before the last data-path commit, or not from this history.
    Stale(String),
    /// A PASS the machine check contradicts, or with nothing to show for it.
    Unproven(String),
    Fail(String),
}

impl Verdict {
    fn ok(&self) -> bool {
        matches!(self, Verdict::Pass | Verdict::Waived(_))
    }
}

pub struct Ledger {
    pub rev: String,
    pub last_data: String,
    pub rows: Vec<(String, String, Verdict)>,
}

impl Ledger {
    pub fn pass(&self) -> bool {
        !self.rows.is_empty() && self.rows.iter().all(|(_, _, v)| v.ok())
    }

    pub fn text(&self) -> String {
        let mut s = format!(
            "evidence for {} (last data-path commit {}):\n",
            short(&self.rev),
            short(&self.last_data)
        );
        for (case, os, v) in &self.rows {
            let what = match v {
                Verdict::Pass => "PASS".to_string(),
                Verdict::Waived(n) => format!("waived: {n}"),
                Verdict::Missing => "MISSING: no run".to_string(),
                Verdict::Stale(n) => format!("STALE: {n}"),
                Verdict::Unproven(n) => format!("UNPROVEN: {n}"),
                Verdict::Fail(n) => format!("FAIL: {n}"),
            };
            s.push_str(&format!("  {case:<8} {os:<8} {what}\n"));
        }
        s.push_str(if self.pass() {
            "PASS: every case has fresh evidence on every system, or a waiver\n"
        } else {
            "FAIL: not ready to cut\n"
        });
        s
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

/// The case IDs of the data-safety file (`### M-DS-NN …`).
pub fn case_ids(md: &str) -> Vec<String> {
    md.lines()
        .filter_map(|l| l.strip_prefix("### "))
        .filter_map(|h| h.split_whitespace().next())
        .filter(|id| id.starts_with("M-") && id.len() == 7)
        .map(String::from)
        .collect()
}

/// One row of `RUNS` as column → value; the header and blank or `#` lines skipped.
pub fn parse_runs(tsv: &str) -> Vec<BTreeMap<&'static str, String>> {
    tsv.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("date_utc"))
        .map(|l| {
            COLUMNS
                .iter()
                .zip(l.split('\t').chain(std::iter::repeat("")))
                .map(|(k, v)| (*k, v.trim().to_string()))
                .collect()
        })
        .collect()
}

pub fn ledger(root: &Path, rev: &str) -> Result<Ledger, String> {
    let rev = git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?;
    let mut args = vec!["log", "-1", "--format=%H", rev.as_str(), "--"];
    args.extend(DATA_PATH);
    let last_data = git(root, &args)?;
    if last_data.is_empty() {
        return Err("no commit touches the data path: wrong repository?".into());
    }
    let cases_md = git(root, &["show", &format!("{rev}:{CASES}")])?;
    let runs = git(root, &["show", &format!("{rev}:{RUNS}")]).unwrap_or_default();
    let runs = parse_runs(&runs);
    let mut rows = Vec::new();
    for case in case_ids(&cases_md) {
        for os in SYSTEMS {
            // The newest row for the pair decides; rows are in the order written.
            let row = runs
                .iter()
                .rfind(|r| r["case"] == case && r["os"].starts_with(os));
            let v = match row {
                None => Verdict::Missing,
                Some(r) => judge(root, &last_data, &case, r),
            };
            rows.push((case.clone(), os.to_string(), v));
        }
    }
    Ok(Ledger {
        rev,
        last_data,
        rows,
    })
}

fn judge(root: &Path, last_data: &str, case: &str, r: &BTreeMap<&'static str, String>) -> Verdict {
    let result = r["result"].to_ascii_uppercase();
    if result == "WAIVED" {
        return if r["note"].is_empty() {
            Verdict::Unproven("a waiver needs its reason in note".into())
        } else {
            Verdict::Waived(r["note"].clone())
        };
    }
    let built = r["built_from"].trim_end_matches("-dirty");
    if built.is_empty() || built == "-" {
        return Verdict::Stale("built_from not recorded".into());
    }
    if r["built_from"].ends_with("-dirty") {
        return Verdict::Stale(format!(
            "{} was built from uncommitted code",
            r["built_from"]
        ));
    }
    if !is_ancestor(root, last_data, built) {
        return Verdict::Stale(format!(
            "built from {} , before data-path commit {}",
            short(built),
            short(last_data)
        ));
    }
    match result.as_str() {
        "PASS" => {
            if r["check"].eq_ignore_ascii_case("FAIL") {
                Verdict::Unproven("PASS written, but --check said FAIL".into())
            } else if DISK.contains(&case) && (r["evidence"].is_empty() || r["evidence"] == "-") {
                Verdict::Unproven("a disk case needs --check output or a debug log named".into())
            } else {
                Verdict::Pass
            }
        }
        "FAIL" => Verdict::Fail(format!("{} {}", r["issue"], r["note"]).trim().to_string()),
        other => Verdict::Unproven(format!("result {other:?} is not PASS, FAIL or WAIVED")),
    }
}

/// Concepts and the marks of an implementation of each: a line holding one counts as
/// one place. Consolidation (plan v3 H-03..H-08) should bring each count down; the
/// marks are kept here, beside the ledger, so the count is the same every time.
pub const CONCEPTS: &[(&str, &[&str])] = &[
    (
        "root confinement",
        &[
            "fn allows(",
            "fn may_tidy(",
            "root.is_some() &&",
            ".root.as_ref()",
        ],
    ),
    (
        "path identity",
        &[
            "fn same_entry(",
            "fn same_file(",
            "fn into_itself(",
            "fn within(",
        ],
    ),
    (
        "atomic write",
        &[
            "fn save_bytes(",
            "fn write_private(",
            "fn write_atomic(",
            ".persist(",
        ],
    ),
    (
        "refusal of writes",
        &["fn refused(", "is turned off", "fn writes("],
    ),
    (
        "control characters in names",
        &["fn control_char(", "is_control()", "fn validate_name("],
    ),
    (
        "home and state folders",
        &[
            "fn home_dir(",
            "\"USERPROFILE\"",
            "\"XDG_STATE_HOME\"",
            "\"LOCALAPPDATA\"",
        ],
    ),
    (
        "journal kinds",
        &[
            "format!(\"src ",
            "format!(\"dest ",
            "format!(\"ckpt ",
            "Some(\"ckpt\")",
            "(\"done\"",
        ],
    ),
];

/// Every concept's count, with the places.
pub fn defs(root: &Path) -> String {
    let mut files = Vec::new();
    for d in ["crates/xdevcmd-core/src", "crates/xdevcmd/src"] {
        collect(&root.join(d), &mut files);
    }
    files.sort();
    let mut s = String::new();
    for (concept, marks) in CONCEPTS {
        let mut at = Vec::new();
        for f in &files {
            let Ok(text) = fs::read_to_string(f) else {
                continue;
            };
            // Tests are not implementations.
            let body = text
                .split("#[cfg(test)]\nmod tests")
                .next()
                .unwrap_or(&text);
            for (n, line) in body.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if marks.iter().any(|m| line.contains(m)) {
                    let rel = f.strip_prefix(root).unwrap_or(f);
                    at.push(format!("{}:{}", rel.display(), n + 1));
                }
            }
        }
        s.push_str(&format!("{concept}: {}\n", at.len()));
        for a in at {
            s.push_str(&format!("  {a}\n"));
        }
    }
    s
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    const CASES_MD: &str = "### M-DS-01 Kill a copy\n\n### M-DS-02 Pull a stick\n";

    fn row(
        built: &str,
        os: &str,
        case: &str,
        result: &str,
        check: &str,
        ev: &str,
        note: &str,
    ) -> String {
        format!("2026-11-20T00:00Z\t{built}\t0.4.0-beta.2\t{os}\tterm\tsh\t{case}\t{result}\t{check}\t{ev}\t-\t{note}\n")
    }

    /// The ledger fires: evidence made before a data-path commit is STALE, a PASS that
    /// --check contradicts or that names no evidence is UNPROVEN, a missing system is
    /// MISSING; fresh evidence and waivers with a reason pass (lessons L7, L15).
    #[test]
    fn the_ledger_fails_stale_contradicted_and_missing_evidence() {
        let d = std::env::temp_dir().join(format!("xtask-qa-ledger-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        sh(&d, &["init", "-q"]);
        write(&d, CASES, CASES_MD);
        write(&d, DATA_PATH[0], "v1\n");
        sh(&d, &["add", "-A"]);
        sh(&d, &["commit", "-qm", "one"]);
        let old = sh(&d, &["rev-parse", "HEAD"]);
        write(&d, DATA_PATH[0], "v2\n");
        sh(&d, &["commit", "-qam", "data path"]);
        let new = sh(&d, &["rev-parse", "HEAD"]);

        // Evidence from before the data-path commit: stale everywhere.
        let mut tsv = COLUMNS.join("\t") + "\n";
        for os in SYSTEMS {
            tsv += &row(&old, os, "M-DS-01", "PASS", "PASS", "check.txt", "");
            tsv += &row(&old, os, "M-DS-02", "PASS", "-", "-", "");
        }
        write(&d, RUNS, &tsv);
        sh(&d, &["add", "-A"]);
        sh(&d, &["commit", "-qm", "runs"]);
        let l = ledger(&d, "HEAD").unwrap();
        assert!(!l.pass(), "{}", l.text());
        assert!(
            l.rows
                .iter()
                .all(|(_, _, v)| matches!(v, Verdict::Stale(_))),
            "{}",
            l.text()
        );

        // Fresh, but: PASS against --check FAIL, a disk case with no evidence, macOS
        // missing, Windows waived with a reason.
        tsv += &row(&new, "linux", "M-DS-01", "PASS", "FAIL", "check.txt", "");
        tsv += &row(&new, "linux", "M-DS-02", "PASS", "-", "-", "");
        tsv += &row(&new, "windows", "M-DS-01", "PASS", "PASS", "-", "");
        tsv += &row(
            &new,
            "windows",
            "M-DS-02",
            "WAIVED",
            "-",
            "-",
            "no stick on this machine",
        );
        write(&d, RUNS, &tsv);
        sh(&d, &["commit", "-qam", "runs 2"]);
        let l = ledger(&d, "HEAD").unwrap();
        let v = |c: &str, o: &str| {
            l.rows
                .iter()
                .find(|r| r.0 == c && r.1 == o)
                .unwrap()
                .2
                .clone()
        };
        assert!(matches!(v("M-DS-01", "linux"), Verdict::Unproven(_)));
        assert_eq!(v("M-DS-02", "linux"), Verdict::Pass);
        assert!(matches!(v("M-DS-01", "windows"), Verdict::Unproven(_)));
        assert!(matches!(v("M-DS-02", "windows"), Verdict::Waived(_)));
        assert!(matches!(v("M-DS-01", "macos"), Verdict::Stale(_)));
        assert!(!l.pass());

        // All fresh and proven: PASS.
        let mut good = COLUMNS.join("\t") + "\n";
        for os in SYSTEMS {
            good += &row(&new, os, "M-DS-01", "PASS", "PASS", "check.txt", "");
            good += &row(&new, os, "M-DS-02", "PASS", "-", "-", "");
        }
        write(&d, RUNS, &good);
        sh(&d, &["commit", "-qam", "runs 3"]);
        let l = ledger(&d, "HEAD").unwrap();
        assert!(l.pass(), "{}", l.text());
        let _ = fs::remove_dir_all(&d);
    }

    /// The ways a wrong PASS slipped in (red-team of #51): a side branch's build whose
    /// data path differs, a symbolic `built_from`, a case heading that does not parse,
    /// a waiver from an old build, a fresh FAIL hidden by a later PASS row, and an
    /// `os` that only starts like a system.
    #[test]
    fn the_ledger_refuses_the_ways_a_wrong_pass_slipped_in() {
        let d = std::env::temp_dir().join(format!("xtask-qa-slip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        sh(&d, &["init", "-q", "-b", "main"]);
        write(&d, CASES, "### M-DS-01 Kill a copy\n");
        write(&d, DATA_PATH[0], "v1\n");
        sh(&d, &["add", "-A"]);
        sh(&d, &["commit", "-qm", "one"]);
        let base = sh(&d, &["rev-parse", "HEAD"]);
        // A side branch after it, with another data path: not what main holds.
        sh(&d, &["switch", "-q", "-c", "side"]);
        write(&d, DATA_PATH[0], "side\n");
        sh(&d, &["commit", "-qam", "side"]);
        let side = sh(&d, &["rev-parse", "HEAD"]);
        sh(&d, &["switch", "-q", "main"]);
        let runs = |rows: &str| {
            write(&d, RUNS, &(COLUMNS.join("\t") + "\n" + rows));
            sh(&d, &["add", "-A"]);
            sh(&d, &["commit", "-qm", "runs", "--allow-empty"]);
        };
        let all = |built: &str, result: &str, note: &str| {
            SYSTEMS
                .iter()
                .map(|os| row(built, os, "M-DS-01", result, "PASS", "check.txt", note))
                .collect::<String>()
        };
        let verdicts = || ledger(&d, "HEAD").unwrap();
        runs(&all(&side, "PASS", ""));
        assert!(!verdicts().pass(), "a side branch's data path passed");
        runs(&all("HEAD", "PASS", ""));
        assert!(!verdicts().pass(), "a symbolic built_from passed");
        runs(&all(&base, "PASS", ""));
        assert!(verdicts().pass(), "{}", verdicts().text());
        // A waiver from a build before a data-path change goes stale.
        write(&d, DATA_PATH[0], "v2\n");
        sh(&d, &["commit", "-qam", "data path"]);
        let new = sh(&d, &["rev-parse", "HEAD"]);
        runs(&all(&base, "WAIVED", "no machine"));
        assert!(!verdicts().pass(), "an old waiver passed");
        runs(&all(&new, "WAIVED", "-"));
        assert!(!verdicts().pass(), "a waiver with no reason passed");
        // A fresh FAIL stays, whatever row comes after it.
        let mut rows = all(&new, "PASS", "");
        rows += &row(&new, "linux", "M-DS-01", "FAIL", "FAIL", "log", "lost a file");
        rows += &row(&new, "linux", "M-DS-01", "PASS", "PASS", "check.txt", "");
        runs(&rows);
        assert!(!verdicts().pass(), "a fresh FAIL was hidden");
        // Only the system's own name counts.
        let mut rows = String::new();
        for os in ["linux (WSL on Windows 11)", "macos", "windows"] {
            rows += &row(&new, os, "M-DS-01", "PASS", "PASS", "check.txt", "");
        }
        runs(&rows);
        assert!(!verdicts().pass(), "WSL counted as Linux");
        // A case heading that does not parse is an error, never a case left out.
        write(&d, CASES, "### M-DS-01 Kill a copy\n### M-DS-08: Pull a stick\n");
        runs(&all(&new, "PASS", ""));
        assert!(ledger(&d, "HEAD").is_err(), "a case heading was dropped");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_note_with_a_tab_does_not_shift_the_columns_it_is_cut() {
        let r = parse_runs("x\tabc\tv\tlinux\tt\ts\tM-DS-01\tPASS\tPASS\tlog\t-\tnote\twith tab\n");
        assert_eq!(r[0]["note"], "note");
        assert_eq!(r[0]["case"], "M-DS-01");
    }

    /// The real files parse: every data-safety case is found, and the runs file's
    /// header is the columns.
    #[test]
    fn the_real_cases_and_runs_files_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let ids = case_ids(&fs::read_to_string(root.join(CASES)).unwrap());
        assert_eq!(ids.len(), 7, "{ids:?}");
        let runs = fs::read_to_string(root.join(RUNS)).unwrap();
        assert_eq!(runs.lines().next().unwrap(), COLUMNS.join("\t"));
    }
}
