//! `xtask manual`: the data-safety hand tests, from a tester's kit (plan v3 P5, M1).
//!
//! ```text
//! xtask manual list                 the cases, with what `xtask try` does for each
//! xtask manual show M-DS-NN         one case: setup, steps, expected, fail if
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
use std::io::Write as _;
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

fn os() -> &'static str {
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
    a: &LogArgs,
    version: &str,
    commit: &str,
    today: &str,
    check_text: Option<&str>,
) -> Result<String, String> {
    let ids = qa::case_ids(CASES_MD)?;
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
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let terminal = a.terminal.clone().unwrap_or_else(|| {
        let t = env("TERM_PROGRAM");
        if !t.is_empty() {
            t
        } else if !env("WT_SESSION").is_empty() {
            "Windows Terminal".into()
        } else {
            env("TERM")
        }
    });
    let shell = a.shell.clone().unwrap_or_else(|| {
        let s = env("SHELL");
        if s.is_empty() {
            env("ComSpec")
        } else {
            s
        }
    });
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
    let line = row(a, version, COMMIT, &crate::today(), check_text.as_deref())?;
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

    fn args(case: &str, result: &str) -> LogArgs {
        LogArgs {
            case: case.into(),
            result: result.into(),
            ..LogArgs::default()
        }
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
            row(a, "0.4.0-beta.2", commit, "2026-11-22", check)
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
