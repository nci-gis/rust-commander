//! Undo the last operation, where that can be done without losing anything.
//!
//! The app records what the last operation was; when undo is asked for, [`Undo::plan`]
//! looks at the disk *now* and says what undoing would do, or why it cannot. Nothing is
//! taken on trust: a renamed file must still be where the rename put it, its old name
//! still free; a new directory must still hold nothing but directories. What undoing
//! would cost data — a copy (deleting the copies), a delete — is not undone, and says
//! so, rather than undoing something older behind the user's back.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Undo {
    /// Renames as done, each (from, to): F2, Shift+F6, rename many.
    Rename(Vec<(PathBuf, PathBuf)>),
    /// A move: the sources, the directory they went to, and the one name if renamed.
    Move {
        sources: Vec<PathBuf>,
        dest: PathBuf,
        rename_to: Option<OsString>,
    },
    /// F7: the first directory that did not exist before (a path may make several).
    MadeDir(PathBuf),
    /// Shift+F4: a new, empty file.
    MadeFile(PathBuf),
    /// The last operation was one that is not undone: "a copy", "a delete".
    Cannot(String),
}

/// What undoing does, decided against the disk as it is now.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Renames, each (from, to): the original ones turned round.
    Rename(Vec<(PathBuf, PathBuf)>),
    /// Move these back into this directory (a job, as F6 runs).
    Move { back: Vec<PathBuf>, into: PathBuf },
    /// Remove this empty directory tree, or this empty file.
    Remove(PathBuf),
}

fn there(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn name(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Nothing in `dir` but directories, all the way down; never following links.
fn only_dirs(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().all(|e| {
        e.file_type()
            .is_ok_and(|t| t.is_dir() && !t.is_symlink() && only_dirs(&e.path()))
    })
}

impl Undo {
    /// What it was, for "Undo the …?".
    pub fn describe(&self) -> String {
        match self {
            Undo::Rename(m) if m.len() == 1 => {
                format!("rename of {} to {}", name(&m[0].0), name(&m[0].1))
            }
            Undo::Rename(m) => format!("rename of {} files", m.len()),
            Undo::Move { sources, dest, .. } => {
                let what = if sources.len() == 1 {
                    name(&sources[0])
                } else {
                    format!("{} items", sources.len())
                };
                format!("move of {what} to {}", dest.display())
            }
            Undo::MadeDir(d) => format!("new directory {}", name(d)),
            Undo::MadeFile(f) => format!("new file {}", name(f)),
            Undo::Cannot(what) => what.clone(),
        }
    }

    /// What undoing would do now, or why it cannot.
    pub fn plan(&self) -> Result<Plan, String> {
        match self {
            Undo::Cannot(what) => Err(format!("the last operation, {what}, cannot be undone")),
            Undo::Rename(moves) => {
                for (from, to) in moves {
                    if !there(to) {
                        return Err(format!("{} is not there any more", name(to)));
                    }
                    if there(from) {
                        return Err(format!("{} exists again", name(from)));
                    }
                }
                Ok(Plan::Rename(
                    moves
                        .iter()
                        .rev()
                        .map(|(f, t)| (t.clone(), f.clone()))
                        .collect(),
                ))
            }
            Undo::Move {
                sources,
                dest,
                rename_to,
            } => {
                let mut parents: Vec<&Path> = sources.iter().filter_map(|s| s.parent()).collect();
                parents.dedup();
                let [into] = parents.as_slice() else {
                    return Err(
                        "those came from several directories: move them back with F6".into(),
                    );
                };
                let back: Vec<PathBuf> = sources
                    .iter()
                    .filter(|s| !there(s))
                    .filter_map(|s| {
                        let n = match (rename_to, sources.len()) {
                            (Some(r), 1) => r.clone(),
                            _ => s.file_name()?.to_os_string(),
                        };
                        let target = dest.join(n);
                        there(&target).then_some(target)
                    })
                    .collect();
                if back.is_empty() {
                    return Err("nothing of that move is where it went".into());
                }
                Ok(Plan::Move {
                    back,
                    into: into.to_path_buf(),
                })
            }
            Undo::MadeDir(d) => {
                if !d.is_dir() {
                    return Err(format!("{} is not there any more", name(d)));
                }
                if !only_dirs(d) {
                    return Err(format!("{} is not empty any more", name(d)));
                }
                Ok(Plan::Remove(d.clone()))
            }
            Undo::MadeFile(f) => match fs::symlink_metadata(f) {
                Ok(m) if m.is_file() && m.len() == 0 => Ok(Plan::Remove(f.clone())),
                Ok(_) => Err(format!("{} is not empty any more", name(f))),
                Err(_) => Err(format!("{} is not there any more", name(f))),
            },
        }
    }
}

/// Carry out a `Plan::Remove`: an empty directory tree, or an empty file. Checked again
/// here, and refused in read-only mode, as every write in the engine is.
pub fn remove(p: &Path) -> io::Result<()> {
    crate::policy::guard_write()?;
    if !crate::policy::get().allows(p) {
        return Err(io::Error::other("outside the root xdevcmd is confined to"));
    }
    let m = fs::symlink_metadata(p)?;
    if m.is_dir() {
        if !only_dirs(p) {
            return Err(io::Error::other("not empty any more"));
        }
        remove_empty_tree(p)
    } else if m.is_file() && m.len() == 0 {
        fs::remove_file(p)
    } else {
        Err(io::Error::other("not empty any more"))
    }
}

/// Remove a tree of directories bottom up, each with `remove_dir`, which refuses one
/// that is not empty: a file that appears between the check and the removal stops it,
/// rather than going with the rest.
fn remove_empty_tree(dir: &Path) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            remove_empty_tree(&e.path())?;
        }
    }
    fs::remove_dir(dir)
}

/// The first directory of `target` that does not exist yet: what F7 is about to make,
/// and so what undoing it removes.
pub fn first_missing(target: &Path) -> PathBuf {
    let mut first = target.to_path_buf();
    let mut p = target;
    while let Some(parent) = p.parent() {
        if there(parent) || parent.as_os_str().is_empty() {
            break;
        }
        first = parent.to_path_buf();
        p = parent;
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-undo-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_rename_turns_round_while_the_disk_still_says_so() {
        let d = dir("rename");
        let (a, b) = (d.join("a.txt"), d.join("b.txt"));
        fs::write(&b, "x").unwrap();
        let u = Undo::Rename(vec![(a.clone(), b.clone())]);
        assert_eq!(u.plan(), Ok(Plan::Rename(vec![(b.clone(), a.clone())])));
        fs::write(&a, "new").unwrap();
        assert!(u.plan().unwrap_err().contains("exists again"));
        fs::remove_file(&a).unwrap();
        fs::remove_file(&b).unwrap();
        assert!(u.plan().unwrap_err().contains("not there"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_move_goes_back_for_what_is_still_where_it_went() {
        let d = dir("move");
        let (src, dst) = (d.join("src"), d.join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        // x was moved; y was skipped (still in src); z was moved, then deleted.
        fs::write(dst.join("x"), "x").unwrap();
        fs::write(src.join("y"), "y").unwrap();
        let u = Undo::Move {
            sources: vec![src.join("x"), src.join("y"), src.join("z")],
            dest: dst.clone(),
            rename_to: None,
        };
        assert_eq!(
            u.plan(),
            Ok(Plan::Move {
                back: vec![dst.join("x")],
                into: src.clone()
            })
        );
        let mixed = Undo::Move {
            sources: vec![src.join("x"), dst.join("other")],
            dest: dst.clone(),
            rename_to: None,
        };
        assert!(mixed.plan().unwrap_err().contains("several directories"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_new_directory_goes_only_while_it_holds_nothing_but_directories() {
        let d = dir("mkdir");
        let made = d.join("a");
        fs::create_dir_all(made.join("b/c")).unwrap();
        assert_eq!(first_missing(&d.join("new/deeper")), d.join("new"));
        let u = Undo::MadeDir(made.clone());
        assert_eq!(u.plan(), Ok(Plan::Remove(made.clone())));
        fs::write(made.join("b/file"), "data").unwrap();
        assert!(u.plan().unwrap_err().contains("not empty"));
        assert!(remove(&made).is_err(), "and remove checks again");
        assert!(made.join("b/file").exists());
        fs::remove_file(made.join("b/file")).unwrap();
        remove(&made).unwrap();
        assert!(!made.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_new_file_goes_only_while_it_is_empty() {
        let d = dir("newfile");
        let f = d.join("notes.txt");
        fs::write(&f, "").unwrap();
        let u = Undo::MadeFile(f.clone());
        assert_eq!(u.plan(), Ok(Plan::Remove(f.clone())));
        fs::write(&f, "typed something").unwrap();
        assert!(u.plan().unwrap_err().contains("not empty"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn what_cannot_be_undone_says_so() {
        let u = Undo::Cannot("a copy".into());
        assert_eq!(
            u.plan().unwrap_err(),
            "the last operation, a copy, cannot be undone"
        );
    }
}
