//! Rename many files at once by editing their names as text (Shift+F2).
//!
//! The names go into a file, one per line, your editor changes them, and the edited
//! file comes back as a plan: which file becomes what. The plan is checked as a whole
//! before anything is touched, and refused rather than half-done when something is
//! wrong — a missing line, two files given one name, a name that would replace a file
//! not being renamed.
//!
//! Swaps and chains work (`a → b`, `b → a`), because the renames happen in two steps:
//! every file first moves to a temporary name beside it, then each temporary name moves
//! to its final one. A failure in the first step puts back what had moved; in the
//! second, the file keeps a name you can see and the report says which.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::rename::validate_name;

/// The file to hand to an editor: one name per line, in the order given.
pub fn names_text(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        out.push_str(&p.file_name().unwrap_or_default().to_string_lossy());
        out.push('\n');
    }
    out
}

/// What would change: `(from, to)`, only for lines that changed.
pub fn plan(paths: &[PathBuf], edited: &str) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut lines: Vec<&str> = edited.lines().map(|l| l.trim_end_matches('\r')).collect();
    // An editor's trailing blank lines are not names.
    while lines.last().is_some_and(|l| l.is_empty()) && lines.len() > paths.len() {
        lines.pop();
    }
    if lines.len() != paths.len() {
        return Err(format!(
            "{} names for {} files \u{2014} lines must stay one per file, in order",
            lines.len(),
            paths.len()
        ));
    }
    let mut moves = Vec::new();
    for (p, line) in paths.iter().zip(&lines) {
        let old = p.file_name().unwrap_or_default().to_string_lossy();
        if *line == old {
            continue;
        }
        validate_name(line).map_err(|e| format!("{line:?}: {e}"))?;
        let dir = p.parent().unwrap_or(Path::new("."));
        moves.push((p.clone(), dir.join(line)));
    }
    // Two files given one name, compared without case: on Windows and macOS they
    // are the same file, and refusing everywhere keeps a plan meaning one thing.
    let mut seen = HashSet::new();
    let sources: HashSet<&PathBuf> = moves.iter().map(|(f, _)| f).collect();
    for (from, to) in &moves {
        let key = to.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            return Err(format!("two files would be named {}", show(to)));
        }
        // Moving onto something that exists is only fine when that something is itself
        // being renamed away, or is this very file (a change of case).
        if fs::symlink_metadata(to).is_ok() && !sources.contains(to) && !same_file(from, to) {
            return Err(format!(
                "{} already exists \u{2014} nothing renamed",
                show(to)
            ));
        }
    }
    Ok(moves)
}

fn show(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub renamed: usize,
    pub errors: Vec<String>,
}

/// Carry out a plan from [`plan`]. `safe`: safe operations for this batch (a journal
/// of the temporary names), taken from the setting by the caller.
pub fn apply(moves: &[(PathBuf, PathBuf)], safe: bool) -> Outcome {
    let mut out = Outcome::default();
    if let Err(e) = crate::policy::guard_write() {
        out.errors.push(e.to_string());
        return out;
    }
    let tag = crate::journal::run_tag();
    // Safe operations: between the two steps the files are under temporary names; a run
    // killed there has them put back at the next start.
    let mut journal = if safe {
        crate::journal::Journal::begin("rename", &[], None, false).and_then(Result::ok)
    } else {
        None
    };
    let mut parked: Vec<(PathBuf, &PathBuf, &PathBuf)> = Vec::new();
    for (i, (from, to)) in moves.iter().enumerate() {
        let dir = from.parent().unwrap_or(Path::new("."));
        let tmp = dir.join(format!(".xdevcmd-rename-{tag}-{i}"));
        if let Some(j) = journal.as_mut() {
            j.renamed(&tmp, from);
        }
        if let Err(e) = crate::noreplace::rename_noreplace(from, &tmp) {
            out.errors.push(format!("{}: {e}", show(from)));
            // Put back what had already moved: half a batch is worse than none.
            for (t, f, _) in parked.iter().rev() {
                let _ = fs::rename(t, f);
            }
            return out;
        }
        parked.push((tmp, from, to));
    }
    for (tmp, from, to) in parked {
        // Crossed out of the journal once this step is over, however it went.
        let _crossed = Unrename(journal.as_mut(), &tmp);
        // The system refuses a name that is taken in the same step as the rename, so a
        // file that appeared since the plan was made is never overwritten.
        match crate::noreplace::rename_noreplace(&tmp, to) {
            Ok(()) => out.renamed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let back = crate::noreplace::rename_noreplace(&tmp, from);
                out.errors.push(match back {
                    Ok(()) => format!(
                        "{} appeared meanwhile \u{2014} {} left as it was",
                        show(to),
                        show(from)
                    ),
                    Err(_) => format!(
                        "{} appeared meanwhile \u{2014} the file is now {}",
                        show(to),
                        tmp.display()
                    ),
                });
            }
            Err(e) => out.errors.push(format!(
                "{}: {e} \u{2014} the file is now {}",
                show(to),
                tmp.display()
            )),
        }
    }
    out
}

/// Crosses a temporary name out of the journal when its step is over: renamed on, put
/// back, or left (and reported).
struct Unrename<'a>(Option<&'a mut crate::journal::Journal>, &'a Path);

impl Drop for Unrename<'_> {
    fn drop(&mut self) {
        if let Some(j) = self.0.as_mut() {
            j.unrenamed(self.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
        let d = std::env::temp_dir().join(format!("xdevcmd-batch-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let paths = names
            .iter()
            .map(|n| {
                fs::write(d.join(n), n.as_bytes()).unwrap();
                d.join(n)
            })
            .collect();
        (d, paths)
    }

    fn body(p: &Path) -> String {
        fs::read_to_string(p).unwrap()
    }

    #[test]
    fn unchanged_lines_are_left_alone_and_changed_ones_renamed() {
        let (d, paths) = dir("basic", &["a.txt", "b.txt", "c.txt"]);
        let moves = plan(&paths, "a.txt\nbee.txt\nc.txt\n").unwrap();
        assert_eq!(moves, vec![(d.join("b.txt"), d.join("bee.txt"))]);
        assert_eq!(apply(&moves, false).renamed, 1);
        assert_eq!(body(&d.join("bee.txt")), "b.txt");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_swap_works_because_of_the_two_steps() {
        let (d, paths) = dir("swap", &["a", "b"]);
        let moves = plan(&paths, "b\na\n").unwrap();
        assert_eq!(
            apply(&moves, false),
            Outcome {
                renamed: 2,
                errors: vec![]
            }
        );
        assert_eq!(
            (body(&d.join("a")), body(&d.join("b"))),
            ("b".into(), "a".into())
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_wrong_line_count_is_refused() {
        let (d, paths) = dir("count", &["a", "b"]);
        let e = plan(&paths, "a\n").unwrap_err();
        assert!(e.contains("1 names for 2 files"), "{e}");
        assert!(
            plan(&paths, "a\nb\n\n\n").is_ok(),
            "trailing blank lines are fine"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn two_files_one_name_is_refused_even_by_case() {
        let (d, paths) = dir("dupe", &["a", "b"]);
        assert!(plan(&paths, "x\nx\n").unwrap_err().contains("two files"));
        assert!(plan(&paths, "x\nX\n").unwrap_err().contains("two files"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_name_that_would_replace_another_file_is_refused() {
        let (d, paths) = dir("exists", &["a", "b", "keep"]);
        let e = plan(&paths[..2], "keep\nb\n").unwrap_err();
        assert!(e.contains("keep already exists"), "{e}");
        assert_eq!(body(&d.join("keep")), "keep", "untouched");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_that_appears_after_the_plan_is_never_overwritten() {
        let (d, p) = dir("appears", &["a", "b"]);
        let moves = vec![(p[0].clone(), d.join("x")), (p[1].clone(), d.join("y"))];
        // Another program creates x between the plan and the renaming.
        fs::write(d.join("x"), "theirs").unwrap();
        let out = apply(&moves, false);
        assert_eq!(out.renamed, 1, "{:?}", out.errors);
        assert!(
            out.errors[0].contains("appeared meanwhile"),
            "{:?}",
            out.errors
        );
        assert_eq!(body(&d.join("x")), "theirs");
        assert_eq!(body(&p[0]), "a", "a is back under its own name");
        assert_eq!(body(&d.join("y")), "b");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_slash_or_an_empty_line_is_not_a_name() {
        let (d, paths) = dir("bad", &["a", "b"]);
        assert!(plan(&paths, "sub/a\nb\n").is_err());
        assert!(plan(&paths, "\nb\n").is_err());
        let _ = fs::remove_dir_all(&d);
    }
}
