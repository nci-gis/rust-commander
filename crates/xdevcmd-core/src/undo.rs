//! Undo the last operation, where that can be done without losing anything.
//!
//! The app records what the last operation was; when undo is asked for, [`Undo::plan`]
//! looks at the disk *now* and says what undoing would do, or why it cannot. Nothing is
//! taken on trust: a renamed file must still be where the rename put it, its old name
//! still free; a new directory must still hold nothing but directories. A copy is undone
//! by removing what it made, but only what is still as it made it (Round 031). What
//! undoing would cost data — a permanent delete — is not undone, and says so, rather than
//! undoing something older behind the user's back.

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
    /// F8: what went to the trash, and where each item went there.
    Trashed(Vec<crate::trashed::Trashed>),
    /// A copy (F5, Alt+Y): what it made, in the order made; `what` for the question
    /// ("copy of a.txt");
    /// `for_good`: files it replaced with no trash, which undo cannot give back.
    Copy {
        made: Vec<crate::ops::Made>,
        what: String,
        for_good: u64,
    },
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
    /// Put these back from the trash where they were.
    Restore(Vec<crate::trashed::Trashed>),
    /// Remove what a copy made and is still as it made it, putting back from the trash
    /// what it replaced (a job: [`unmake`]). `kept`: how many of them changed since.
    Unmake {
        made: Vec<crate::ops::Made>,
        kept: usize,
    },
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
            Undo::Trashed(t) if t.len() == 1 => {
                format!("delete of {} to the trash", name(&t[0].original))
            }
            Undo::Trashed(t) => format!("delete of {} items to the trash", t.len()),
            Undo::Copy { what, .. } => what.clone(),
            Undo::Cannot(what) => what.clone(),
        }
    }

    /// What undoing would do now, or why it cannot.
    pub fn plan(&self) -> Result<Plan, String> {
        match self {
            Undo::Cannot(what) => Err(format!("the last operation, {what}, cannot be undone")),
            Undo::Copy { made, .. } => {
                let kept = made.iter().filter(|m| !as_made(m)).count();
                if kept == made.len() {
                    return Err(if made.is_empty() {
                        "that copy made nothing new: nothing to undo".into()
                    } else {
                        "nothing that copy made is as it made it any more".into()
                    });
                }
                Ok(Plan::Unmake {
                    made: made.clone(),
                    kept,
                })
            }
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
            // What can still go back goes back; what cannot (taken name, emptied
            // trash) stays, and if nothing can, the first reason says why.
            Undo::Trashed(items) => {
                let mut why = None;
                let back: Vec<_> = items
                    .iter()
                    .filter(|t| match crate::trashed::check(t) {
                        Ok(()) => true,
                        Err(e) => {
                            why.get_or_insert(e);
                            false
                        }
                    })
                    .cloned()
                    .collect();
                if back.is_empty() {
                    return Err(why.unwrap_or_else(|| "nothing to put back".into()));
                }
                Ok(Plan::Restore(back))
            }
        }
    }
}

/// Whether what a copy made is still as it made it: the same file (kind, size, time and,
/// on Unix, inode), or for a directory, still a directory (adding to it changes its
/// time; whether it holds anything else is found when removing it).
fn as_made(m: &crate::ops::Made) -> bool {
    match crate::ops::Ident::at(&m.path) {
        Some(now) if m.ident.is_dir() => now.is_dir(),
        Some(now) => now == m.ident,
        None => false,
    }
}

/// Carry out a `Plan::Unmake` as a job: last made first, so a directory's files go
/// before it. Each is looked at again right before it goes: a file that changed stays;
/// a directory stays while it holds anything (`remove_dir` refuses it). A file that
/// replaced one in the trash goes first, then the old one comes back. What stayed is
/// counted as skipped and listed, with why.
pub fn unmake(made: Vec<crate::ops::Made>) -> crate::ops::Handle {
    use crate::ops::{Event, Summary};
    use std::sync::atomic::Ordering;
    crate::ops::Handle::spawn(move |tx, cancel| {
        let mut sum = Summary::default();
        let _ = tx.send(Event::Scanned {
            files: made.len() as u64,
            bytes: 0,
        });
        let note = |sum: &mut Summary, p: &Path, why: &str| {
            sum.skipped += 1;
            if sum.errors.len() < 20 {
                sum.errors.push(format!("{}: {why}", p.display()));
            }
        };
        let allowed = crate::policy::guard_write();
        for m in made.iter().rev() {
            if cancel.load(Ordering::Relaxed) {
                sum.cancelled = true;
                break;
            }
            if let Err(e) = &allowed {
                sum.failed = made.len() as u64;
                sum.errors.push(e.to_string());
                break;
            }
            if !crate::policy::get().allows(&m.path) {
                note(&mut sum, &m.path, "outside the root xdevcmd is confined to");
                continue;
            }
            let removed = match crate::ops::Ident::at(&m.path) {
                None => {
                    note(&mut sum, &m.path, "not there any more");
                    false
                }
                Some(now) if m.ident.is_dir() && now.is_dir() => match fs::remove_dir(&m.path) {
                    Ok(()) => true,
                    Err(_) => {
                        note(
                            &mut sum,
                            &m.path,
                            "kept: it holds what the copy did not make",
                        );
                        false
                    }
                },
                Some(now) if now == m.ident => match fs::remove_file(&m.path) {
                    Ok(()) => true,
                    Err(e) => {
                        sum.failed += 1;
                        if sum.errors.len() < 20 {
                            sum.errors.push(format!("{}: {e}", m.path.display()));
                        }
                        false
                    }
                },
                Some(_) => {
                    note(&mut sum, &m.path, "kept: changed since the copy");
                    false
                }
            };
            if removed {
                sum.done += 1;
                if let Some(old) = &m.old {
                    if let Err(e) = crate::trashed::restore(old) {
                        sum.failed += 1;
                        if sum.errors.len() < 20 {
                            sum.errors.push(format!(
                                "{}: the file it replaced is still in the trash: {e}",
                                m.path.display()
                            ));
                        }
                    }
                }
            } else if m.old.is_some() && sum.errors.len() < 20 {
                sum.errors.push(format!(
                    "{}: the file it replaced stays in the trash",
                    m.path.display()
                ));
            }
        }
        let _ = tx.send(Event::Done(sum));
    })
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
    fn undoing_a_move_asks_before_overwriting_what_appeared_meanwhile() {
        use crate::ops::{start, Answer, Event, Request};
        // Moving back runs the F6 job: a file that took the old name after the plan was
        // made gets the same overwrite question, and Skip keeps it.
        let run = |back: Vec<PathBuf>, into: PathBuf, answer: Answer| -> u32 {
            let h = start(Request::mv(back, into));
            let mut asked = 0;
            loop {
                match h.events.recv_timeout(std::time::Duration::from_secs(20)) {
                    Ok(Event::Ask(_)) => {
                        asked += 1;
                        h.answer(answer);
                    }
                    Ok(Event::Done(_)) => return asked,
                    Ok(_) => {}
                    Err(e) => panic!("the job never finished: {e}"),
                }
            }
        };
        let d = dir("move-asks");
        let (src, dst) = (d.join("src"), d.join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("x"), "moved").unwrap();
        let u = Undo::Move {
            sources: vec![src.join("x")],
            dest: dst.clone(),
            rename_to: None,
        };
        let Ok(Plan::Move { back, into }) = u.plan() else {
            panic!("a plan to move x back");
        };
        fs::write(src.join("x"), "theirs").unwrap();
        assert_eq!(run(back.clone(), into.clone(), Answer::Skip), 1, "it asked");
        assert_eq!(fs::read_to_string(src.join("x")).unwrap(), "theirs");
        assert_eq!(fs::read_to_string(dst.join("x")).unwrap(), "moved");
        assert_eq!(run(back, into, Answer::Yes), 1);
        assert_eq!(fs::read_to_string(src.join("x")).unwrap(), "moved");
        assert!(!dst.join("x").exists());
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

    /// Run a copy, then undo it as the app does: the plan, then the job.
    fn copy_then(
        d: &Path,
        sources: Vec<PathBuf>,
        between: impl FnOnce(),
    ) -> (Result<Plan, String>, Option<crate::ops::Summary>) {
        let sum = run_job(crate::ops::start(crate::ops::Request::copy(
            sources,
            d.join("to"),
        )));
        let u = Undo::Copy {
            made: sum.made.clone(),
            what: "x".into(),
            for_good: sum.replaced_for_good,
        };
        between();
        let plan = u.plan();
        let done = match &plan {
            Ok(Plan::Unmake { made, .. }) => Some(run_job(unmake(made.clone()))),
            _ => None,
        };
        (plan, done)
    }

    fn run_job(h: crate::ops::Handle) -> crate::ops::Summary {
        loop {
            match h
                .events
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
            {
                crate::ops::Event::Done(s) => return s,
                crate::ops::Event::Ask(_) => h.answer(crate::ops::Answer::Yes),
                _ => {}
            }
        }
    }

    /// Round 031: undoing a copy removes what it made, files then their directories.
    #[test]
    fn undoing_a_copy_removes_what_it_made() {
        let d = dir("copy");
        fs::create_dir_all(d.join("a/b")).unwrap();
        fs::write(d.join("a/b/f"), "f").unwrap();
        fs::write(d.join("g"), "g").unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        let (plan, done) = copy_then(&d, vec![d.join("a"), d.join("g")], || {});
        assert!(matches!(plan, Ok(Plan::Unmake { kept: 0, .. })), "{plan:?}");
        let done = done.unwrap();
        assert_eq!(
            (done.done, done.skipped, done.failed),
            (4, 0, 0),
            "{done:?}"
        );
        assert_eq!(fs::read_dir(d.join("to")).unwrap().count(), 0);
        assert!(
            d.join("a/b/f").exists() && d.join("g").exists(),
            "the sources stay"
        );
    }

    /// What changed since the copy stays, and so does the directory holding it; and a
    /// file dropped into a copied directory keeps that directory.
    #[test]
    fn undoing_a_copy_keeps_what_changed_since() {
        let d = dir("copy-changed");
        fs::create_dir_all(d.join("a/b")).unwrap();
        fs::write(d.join("a/b/f"), "f").unwrap();
        fs::write(d.join("a/e"), "e").unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        let to = d.join("to");
        let (plan, done) = copy_then(&d, vec![d.join("a")], || {
            fs::write(to.join("a/e"), "edited, longer").unwrap();
            fs::write(to.join("a/b/mine"), "m").unwrap();
        });
        assert!(matches!(plan, Ok(Plan::Unmake { kept: 1, .. })), "{plan:?}");
        let done = done.unwrap();
        assert_eq!(done.done, 1, "only b/f goes: {done:?}");
        assert_eq!(
            fs::read_to_string(to.join("a/e")).unwrap(),
            "edited, longer"
        );
        assert!(to.join("a/b/mine").exists());
        assert!(!to.join("a/b/f").exists());
        assert!(
            done.errors
                .iter()
                .any(|e| e.contains("changed since the copy")),
            "{done:?}"
        );
        assert!(done
            .errors
            .iter()
            .any(|e| e.contains("holds what the copy did not make")));
    }

    #[test]
    fn a_copy_with_nothing_left_as_made_says_so() {
        let d = dir("copy-gone");
        fs::write(d.join("g"), "g").unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        let to = d.join("to");
        let (plan, _) = copy_then(&d, vec![d.join("g")], || {
            fs::remove_file(to.join("g")).unwrap();
        });
        assert_eq!(
            plan.unwrap_err(),
            "nothing that copy made is as it made it any more"
        );
    }
}
