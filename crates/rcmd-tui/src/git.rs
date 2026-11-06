//! `[files] git_status`: a column saying what git thinks of each file. Off by default.
//!
//! Off because it runs a program in every directory you open, and git is not
//! harmless in a repository someone else prepared: its config can name a filesystem
//! monitor or content filters, which git runs. `core.fsmonitor` is turned off on the
//! command line here; filters cannot be, so only turn this on where you trust the
//! repositories you browse. It is never run in restricted mode.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};

/// One letter per name in the directory: `M` modified, `A` added, `?` untracked,
/// `D` deleted, `R` renamed, `U` in conflict. A directory gets the strongest of what
/// is inside it.
pub type Marks = HashMap<String, char>;

pub fn start(dir: &Path) -> Receiver<Marks> {
    let (tx, rx) = mpsc::channel();
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(status(&dir).unwrap_or_default());
    });
    rx
}

fn git(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
        ])
        // Never take the index lock: a status from a file manager must not make the
        // user's own `git commit` fail with "index.lock exists".
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    c
}

fn status(dir: &Path) -> Option<Marks> {
    let top = git(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !top.status.success() {
        return None;
    }
    let top = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim_end());
    let out = git(dir)
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=normal",
            "--",
            ".",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let here = rcmd_core::paths::canonical(dir).ok()?;
    let top = rcmd_core::paths::canonical(&top).ok()?;
    Some(parse(&out.stdout, &top, &here))
}

fn rank(c: char) -> u8 {
    match c {
        'U' => 6,
        'D' => 5,
        'M' => 4,
        'R' => 3,
        'A' => 2,
        '?' => 1,
        _ => 0,
    }
}

/// `git status --porcelain=v1 -z` output, as marks for the entries of `here`.
pub fn parse(raw: &[u8], top: &Path, here: &Path) -> Marks {
    let mut marks = Marks::new();
    let mut fields = raw.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(rec) = fields.next() {
        if rec.len() < 4 {
            continue;
        }
        let (x, y) = (rec[0] as char, rec[1] as char);
        let path = String::from_utf8_lossy(&rec[3..]).into_owned();
        // A rename is followed by its old name as a field of its own.
        if x == 'R' || x == 'C' {
            fields.next();
        }
        let mark = match (x, y) {
            ('?', '?') => '?',
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => 'U',
            ('D', _) | (_, 'D') => 'D',
            ('R', _) => 'R',
            ('A', _) => 'A',
            _ => 'M',
        };
        let Ok(rel) = top.join(&path).strip_prefix(here).map(Path::to_path_buf) else {
            continue;
        };
        let Some(first) = rel.components().next() else {
            continue;
        };
        let name = first.as_os_str().to_string_lossy().into_owned();
        let slot = marks.entry(name).or_insert(mark);
        if rank(mark) > rank(*slot) {
            *slot = mark;
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_becomes_one_mark_per_entry_with_directories_taking_the_worst() {
        let raw = b" M src/main.rs\0?? notes.txt\0A  src/new.rs\0R  b.rs\0a.rs\0UU src/x.rs\0 M other/y\0";
        let top = Path::new("/repo");
        let m = parse(raw, top, top);
        assert_eq!(
            m.get("src"),
            Some(&'U'),
            "conflict outranks the rest inside"
        );
        assert_eq!(m.get("notes.txt"), Some(&'?'));
        assert_eq!(m.get("b.rs"), Some(&'R'));
        assert_eq!(m.get("a.rs"), None, "a rename's old name is not an entry");
        let inside = parse(raw, top, &top.join("src"));
        assert_eq!(inside.get("main.rs"), Some(&'M'));
        assert_eq!(inside.get("x.rs"), Some(&'U'));
        assert_eq!(inside.get("y"), None, "only what is below this directory");
    }
}
