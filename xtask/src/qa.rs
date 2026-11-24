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
    // ops.rs's modules (W9): the copy, move and delete themselves.
    "crates/xdevcmd-core/src/ops",
    // The file system's kind, the journal's lock, a cross-device move (W10).
    "crates/xdevcmd-core/src/os.rs",
    // Where the journal and the session are kept (W5).
    "crates/xdevcmd-core/src/dirs.rs",
    // Which entries a copy skips (FIFOs, devices): `special_kind`.
    "crates/xdevcmd-core/src/entry.rs",
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
    "crates/xdevcmd-core/src/faults.rs",
    "crates/xdevcmd-core/src/compare.rs",
    "crates/xdevcmd-core/src/rename.rs",
    "crates/xdevcmd-core/src/attrs.rs",
    "crates/xdevcmd-core/src/document.rs",
    // Starting, forgetting and finishing jobs; input.rs and modals.rs only dispatch to it
    // (their UI changes do not make hand tests stale).
    "crates/xdevcmd/src/app/jobs.rs",
    "crates/xdevcmd/src/app/file_ops.rs",
    // A running job's box: the retry countdown, the Failed question, Esc to stop
    // (M-DS-06, 07). Only those cases list them (CASE_PATHS), so a change to the box
    // does not make the other cases stale.
    "crates/xdevcmd/src/modal.rs",
    "crates/xdevcmd/src/app/modals.rs",
    "crates/xdevcmd/src/app/archives.rs",
    "crates/xdevcmd/src/tempfiles.rs",
    // A new trash or tempfile crate changes what a delete or a save does.
    "Cargo.lock",
];

/// What each hand case vouches for (Round 036, E4; the owner: test again only when a
/// change touches it): its cells go stale when one of these changes, not when any file
/// of `DATA_PATH` does. A case not listed vouches for the whole `DATA_PATH`. Every path
/// here is also in `DATA_PATH` (a test).
pub const CASE_PATHS: &[(&str, &[&str])] = &[
    // A USB stick pulled mid-copy: the copy, its part files and syncs, the error it
    // reports, FAT's names and sizes, what is skipped.
    (
        "M-DS-02",
        &[
            "crates/xdevcmd-core/src/ops.rs",
            "crates/xdevcmd-core/src/ops",
            "crates/xdevcmd-core/src/os.rs",
            "crates/xdevcmd-core/src/plan.rs",
            "crates/xdevcmd-core/src/fastcopy.rs",
            "crates/xdevcmd-core/src/noreplace.rs",
            "crates/xdevcmd-core/src/faults.rs",
            "crates/xdevcmd-core/src/journal.rs",
            "crates/xdevcmd-core/src/entry.rs",
            "crates/xdevcmd/src/app/jobs.rs",
            "crates/xdevcmd/src/app/file_ops.rs",
            "Cargo.lock",
        ],
    ),
    // A network share taken away: the retries, the network check, the Failed question.
    (
        "M-DS-06",
        &[
            "crates/xdevcmd-core/src/ops.rs",
            "crates/xdevcmd-core/src/ops",
            "crates/xdevcmd-core/src/os.rs",
            "crates/xdevcmd-core/src/plan.rs",
            "crates/xdevcmd-core/src/fastcopy.rs",
            "crates/xdevcmd-core/src/noreplace.rs",
            "crates/xdevcmd-core/src/faults.rs",
            "crates/xdevcmd-core/src/journal.rs",
            "crates/xdevcmd/src/app/jobs.rs",
            "crates/xdevcmd/src/app/file_ops.rs",
            "crates/xdevcmd/src/modal.rs",
            "crates/xdevcmd/src/app/modals.rs",
            "Cargo.lock",
        ],
    ),
    // A hard NFS mount whose server is gone: cancelling a job held in a system call.
    (
        "M-DS-07",
        &[
            "crates/xdevcmd-core/src/ops.rs",
            "crates/xdevcmd-core/src/ops",
            "crates/xdevcmd-core/src/os.rs",
            "crates/xdevcmd-core/src/fastcopy.rs",
            "crates/xdevcmd-core/src/faults.rs",
            "crates/xdevcmd/src/app/jobs.rs",
            "crates/xdevcmd/src/app/file_ops.rs",
            "crates/xdevcmd/src/modal.rs",
            "crates/xdevcmd/src/app/modals.rs",
            "Cargo.lock",
        ],
    ),
];

/// The files `case`'s evidence vouches for.
fn paths_of(case: &str) -> &'static [&'static str] {
    CASE_PATHS
        .iter()
        .find(|(c, _)| *c == case)
        .map_or(DATA_PATH, |(_, p)| p)
}

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
pub(crate) const DISK: &[&str] = &["M-DS-01", "M-DS-03", "M-DS-04"];

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

pub(crate) fn repo_root() -> Option<PathBuf> {
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

/// The case IDs of the data-safety file (`### M-DS-NN Title`). A heading naming a case
/// in any other shape is an error: a case left out would never need evidence.
pub fn case_ids(md: &str) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for (n, line) in md.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if !(line.starts_with('#') && line.contains("M-DS-")) {
            continue;
        }
        let id = line
            .strip_prefix("### ")
            .and_then(|h| h.split_once(' '))
            .map(|(id, _)| id)
            .filter(|id| {
                id.len() == 7
                    && id.starts_with("M-DS-")
                    && id[5..].bytes().all(|b| b.is_ascii_digit())
            });
        match id {
            Some(id) => ids.push(id.to_string()),
            None => {
                return Err(format!(
                    "{CASES}:{}: not `### M-DS-NN Title`: {line}",
                    n + 1
                ))
            }
        }
    }
    if ids.is_empty() {
        return Err(format!("{CASES}: no case"));
    }
    Ok(ids)
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
    for case in case_ids(&cases_md)? {
        for os in SYSTEMS {
            let mine: Vec<_> = runs
                .iter()
                .filter(|r| r["case"] == case && r["os"].eq_ignore_ascii_case(os))
                .collect();
            rows.push((
                case.clone(),
                os.to_string(),
                decide(root, &rev, &case, &mine),
            ));
        }
    }
    Ok(Ledger {
        rev,
        last_data,
        rows,
    })
}

/// One case on one system, from all its rows: any fresh FAIL decides; else a fresh,
/// proven PASS; else a fresh waiver; else why not. Order in the file does not matter
/// (two testers' rows merge in any order).
fn decide(root: &Path, rev: &str, case: &str, rows: &[&BTreeMap<&'static str, String>]) -> Verdict {
    if rows.is_empty() {
        return Verdict::Missing;
    }
    let mut fresh = Vec::new();
    let mut why_not = None;
    for r in rows {
        match fresh_build(root, rev, &r["built_from"], paths_of(case)) {
            Ok(()) => fresh.push(*r),
            Err(e) => why_not = Some(Verdict::Stale(e)),
        }
    }
    if let Some(r) = fresh
        .iter()
        .find(|r| r["result"].eq_ignore_ascii_case("FAIL"))
    {
        return Verdict::Fail(format!("{} {}", r["issue"], r["note"]).trim().to_string());
    }
    let verdicts: Vec<Verdict> = fresh.iter().map(|r| one(case, r)).collect();
    if verdicts.contains(&Verdict::Pass) {
        return Verdict::Pass;
    }
    if let Some(w) = verdicts.iter().find(|v| matches!(v, Verdict::Waived(_))) {
        return w.clone();
    }
    verdicts
        .into_iter()
        .find(|v| matches!(v, Verdict::Unproven(_)))
        .or(why_not)
        .unwrap_or(Verdict::Missing)
}

/// A fresh row's own verdict.
pub(crate) fn one(case: &str, r: &BTreeMap<&'static str, String>) -> Verdict {
    let result = r["result"].to_ascii_uppercase();
    let has = |k: &str| !r[k].is_empty() && r[k] != "-";
    let disk = DISK.contains(&case);
    match result.as_str() {
        "WAIVED" if has("note") => Verdict::Waived(r["note"].clone()),
        "WAIVED" => Verdict::Unproven("a waiver needs its reason in note".into()),
        "PASS" if r["check"].eq_ignore_ascii_case("FAIL") => {
            Verdict::Unproven("PASS written, but --check said FAIL".into())
        }
        "PASS" if disk && !r["check"].eq_ignore_ascii_case("PASS") => {
            Verdict::Unproven("a disk case needs `xtask try --check` to say PASS".into())
        }
        "PASS" if disk && !has("evidence") => {
            Verdict::Unproven("a disk case needs its --check output or debug log named".into())
        }
        "PASS" => Verdict::Pass,
        "FAIL" => Verdict::Fail(format!("{} {}", r["issue"], r["note"]).trim().to_string()),
        other => Verdict::Unproven(format!("result {other:?} is not PASS, FAIL or WAIVED")),
    }
}

/// `built_from` names a commit (a SHA, or `git describe`'s `…-g<sha>`), from committed
/// code, whose `paths` (the case's part of the data path) are exactly those at `rev`:
/// not a name that moves (HEAD, a branch), not a side branch with other code (red-team
/// of #51).
fn fresh_build(root: &Path, rev: &str, built: &str, paths: &[&str]) -> Result<(), String> {
    if built.ends_with("-dirty") {
        return Err(format!("{built} was built from uncommitted code"));
    }
    let hex = built.rsplit_once("-g").map_or(built, |(_, h)| h);
    let is_hex = (7..=40).contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if !is_hex {
        return Err(format!(
            "built_from {built:?} is not a commit (a SHA or git describe)"
        ));
    }
    let sha = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{hex}^{{commit}}"),
        ],
    )
    .map_err(|_| format!("{hex} is not a commit of this repository"))?;
    let mut args = vec!["diff", "--quiet", sha.as_str(), rev, "--"];
    args.extend(paths);
    let same = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(&args)
        .status()
        .is_ok_and(|s| s.success());
    if same {
        Ok(())
    } else {
        Err(format!(
            "built from {}, whose data path differs from this one",
            short(&sha)
        ))
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
        "display width and text fit",
        // Text turned into columns: one module is the target (Round 035, W2).
        &[
            "fn text_cells(",
            "fn char_cells(",
            "fn cells(",
            "fn width(s: &str)",
            "fn cut(s: &str",
            "fn cut_end(",
            "fn elide_middle(",
            "fn elided(",
            "fn fit_start(",
            "fn fit_end(",
        ],
    ),
    (
        "chars counted (a width when it sizes a column)",
        // Each `chars().count()` outside tests: a column wrong for a wide (CJK) or a
        // combining (VI) letter when it is used as a width. Not every one is a width.
        &[".chars().count()"],
    ),
    (
        "journal kinds",
        // A line's word spelled outside the table (`journal::Tag`), as the writer and
        // the reader each spelled it before H-07, and the table itself: 1 is one place.
        &[
            "format!(\"src ",
            "format!(\"dest ",
            "format!(\"ckpt ",
            "format!(\"skip ",
            "Some(\"ckpt\")",
            "(\"done\", Some",
            "Tag::Ckpt => (\"ckpt\"",
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
            let body = before_tests(&text);
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
    s.push_str(&sizes(root, &files));
    s
}

/// Files that big (the code before tests) are named by `qa defs`: one to split.
pub const BIG_FILE: usize = 2_500;

/// The fields of `struct App`, and every file whose code before its tests is over
/// `BIG_FILE` lines (a `tests.rs` is all tests) (Round 035's measures of state and file size).
fn sizes(root: &Path, files: &[PathBuf]) -> String {
    let app = fs::read_to_string(root.join("crates/xdevcmd/src/app.rs")).unwrap_or_default();
    let fields = app
        .lines()
        .skip_while(|l| !l.starts_with("pub struct App {"))
        .skip(1)
        .take_while(|l| !l.starts_with('}'))
        .filter(|l| {
            let t = l.trim_start();
            let t = t
                .strip_prefix("pub ")
                .or_else(|| t.strip_prefix("pub(crate) "))
                .unwrap_or(t);
            l.starts_with("    ")
                && !l.starts_with("     ")
                && t.split_once(':').is_some_and(|(n, _)| {
                    !n.is_empty()
                        && n.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                })
        })
        .count();
    let mut s = format!("struct App fields: {fields}\nfiles over {BIG_FILE} lines of code:\n");
    let mut big: Vec<(usize, String)> = files
        .iter()
        // A `tests.rs` is a test module moved out of its parent's file: all tests.
        .filter(|f| f.file_name().is_none_or(|n| n != "tests.rs"))
        .filter_map(|f| {
            let n = before_tests(&fs::read_to_string(f).ok()?).lines().count();
            (n > BIG_FILE).then(|| (n, f.strip_prefix(root).unwrap_or(f).display().to_string()))
        })
        .collect();
    big.sort_by(|a, b| b.cmp(a));
    for (n, f) in big {
        s.push_str(&format!("  {f}: {n}\n"));
    }
    s
}

/// The file up to its test module (`#[cfg(test)]` then `mod tests`, any indent, CRLF).
fn before_tests(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let end = (0..lines.len())
        .find(|&i| {
            lines[i].trim() == "#[cfg(test)]"
                && lines[i + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .is_some_and(|l| l.trim_start().starts_with("mod tests"))
        })
        .unwrap_or(lines.len());
    lines[..end].join("\n")
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

    /// Every data-path entry exists, and a module file split into a folder of modules
    /// names that folder too: `ops.rs` alone missed every change after W9 moved its code
    /// into `ops/` (beta.3, found while planning Round 036).
    #[test]
    fn the_data_path_names_real_files_and_the_folders_of_split_modules() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for p in DATA_PATH {
            assert!(root.join(p).exists(), "{p} is gone: name where it went");
            if let Some(stem) = p.strip_suffix(".rs") {
                if root.join(stem).is_dir() {
                    assert!(
                        DATA_PATH.contains(&stem),
                        "{p} has modules in {stem}/: list it"
                    );
                }
            }
        }
    }

    /// Each case's paths are on the data path and exist: a list naming a file the data
    /// path does not would be stale where nothing tells.
    #[test]
    fn each_cases_paths_are_on_the_data_path() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for (case, paths) in CASE_PATHS {
            for p in *paths {
                assert!(DATA_PATH.contains(p), "{case}: {p} is not in DATA_PATH");
                assert!(root.join(p).exists(), "{case}: {p} is gone");
            }
        }
    }

    /// Round 036, E4: a change to a file a case vouches for makes its cells stale; one
    /// outside them does not, while a case with no list of its own goes stale on any.
    #[test]
    fn a_hand_cell_goes_stale_only_when_its_own_files_change() {
        let d = std::env::temp_dir().join(format!("xtask-qa-percase-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        sh(&d, &["init", "-q"]);
        write(&d, CASES, "### M-DS-01 Whole\n\n### M-DS-07 Its own\n");
        let mine = "crates/xdevcmd-core/src/fastcopy.rs";
        let other = "crates/xdevcmd-core/src/trashed.rs";
        assert!(paths_of("M-DS-07").contains(&mine) && !paths_of("M-DS-07").contains(&other));
        write(&d, mine, "v1\n");
        write(&d, other, "v1\n");
        sh(&d, &["add", "-A"]);
        sh(&d, &["commit", "-qm", "one"]);
        let built = sh(&d, &["rev-parse", "HEAD"]);
        let mut tsv = COLUMNS.join("\t") + "\n";
        for os in SYSTEMS {
            tsv += &row(&built, os, "M-DS-01", "PASS", "PASS", "check.txt", "");
            tsv += &row(&built, os, "M-DS-07", "PASS", "-", "-", "");
        }
        write(&d, RUNS, &tsv);
        sh(&d, &["add", "-A"]);
        sh(&d, &["commit", "-qm", "runs"]);
        let fresh = |case: &str| {
            let l = ledger(&d, "HEAD").unwrap();
            l.rows
                .iter()
                .filter(|(c, _, _)| c == case)
                .all(|(_, _, v)| *v == Verdict::Pass)
        };
        assert!(fresh("M-DS-01") && fresh("M-DS-07"));
        // Outside M-DS-07's files: it stays; M-DS-01, with no list, goes stale.
        write(&d, other, "v2\n");
        sh(&d, &["commit", "-qam", "trash code"]);
        assert!(fresh("M-DS-07"), "{}", ledger(&d, "HEAD").unwrap().text());
        assert!(!fresh("M-DS-01"));
        // One of its own: stale.
        write(&d, mine, "v2\n");
        sh(&d, &["commit", "-qam", "copy code"]);
        assert!(!fresh("M-DS-07"), "{}", ledger(&d, "HEAD").unwrap().text());
        let _ = fs::remove_dir_all(&d);
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
        rows += &row(
            &new,
            "linux",
            "M-DS-01",
            "FAIL",
            "FAIL",
            "log",
            "lost a file",
        );
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
        write(
            &d,
            CASES,
            "### M-DS-01 Kill a copy\n### M-DS-08: Pull a stick\n",
        );
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
    fn sizes_count_app_fields_and_name_big_files_without_their_tests() {
        let root = std::env::temp_dir().join(format!("qa-sizes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let src = root.join("crates/xdevcmd/src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("app.rs"),
            "pub struct App {\n    pub a: u8,\n    b: Vec<\n        u8,\n    >,\n    // c: no\n    pub(crate) d_2: (),\n}\nstruct Other {\n    e: u8,\n}\n",
        )
        .unwrap();
        let code = "fn f() {}\n".repeat(BIG_FILE + 1);
        let tests = "#[cfg(test)]\nmod tests {\n".to_string() + &"\n".repeat(BIG_FILE);
        fs::write(src.join("big.rs"), &code).unwrap();
        fs::write(src.join("tests.rs"), &code).unwrap();
        fs::write(src.join("small.rs"), "fn g() {}\n".to_string() + &tests).unwrap();
        let files = vec![
            src.join("app.rs"),
            src.join("big.rs"),
            src.join("small.rs"),
            src.join("tests.rs"),
        ];
        let got = sizes(&root, &files);
        assert!(got.contains("struct App fields: 3\n"), "{got}");
        assert!(
            // The path as it was built and as this system writes it (Windows mixes `/`
            // from the join above with its own `\`): CI run 45252575167.
            got.contains(&format!(
                "{}: {}",
                src.join("big.rs").strip_prefix(&root).unwrap().display(),
                BIG_FILE + 1
            )),
            "{got}"
        );
        assert!(!got.contains("small.rs"), "{got}");
        assert!(!got.contains("tests.rs"), "{got}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_real_cases_and_runs_files_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let ids = case_ids(&fs::read_to_string(root.join(CASES)).unwrap()).unwrap();
        // Round 036: 01, 03, 04, 05 are run by CI (tests/kill.rs); these need a device.
        assert_eq!(ids, ["M-DS-02", "M-DS-06", "M-DS-07"], "{ids:?}");
        let runs = fs::read_to_string(root.join(RUNS)).unwrap();
        assert_eq!(runs.lines().next().unwrap(), COLUMNS.join("\t"));
    }
}
