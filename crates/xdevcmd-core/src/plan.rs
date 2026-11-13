//! The Smart Planner's model and its scan (Round 030): what a copy or move will do to
//! every file, known before the first byte is written.
//!
//! The planner only reads. It walks the sources once, looks at what is at each
//! destination, and sorts every row into a [`Class`]: new, the same file, an older or a
//! newer one, a file over a directory and the reverse, a directory to merge into. What
//! cannot be read is a row with [`Blocked`], never a gap: a plan made from a directory
//! that could not be read must not look complete (Round 029, B01).
//!
//! The scan runs on its own thread ([`start`]) and can be stopped; the F5 dialog shows
//! its progress. Past [`CAP`] rows only the rows with something to decide are kept, the
//! rest counted.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use crate::faults::{self, Point};
use crate::ops::{Ident, Kind};

/// Past this many rows, a plan keeps only rows with a decision in them (and every
/// blocked row); the plain new files are counted, not kept (owner, 2026-11-13).
pub const CAP: usize = 200_000;

/// How often the scan reports how far it has got, in entries.
const PROGRESS_EVERY: u64 = 512;

/// What is at a row's destination, against its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Nothing there: the copy is new.
    New,
    /// The same file (size and date, or a link to the same target): nothing to do.
    Identical,
    /// A file there that is older than the source: replacing it brings it up to date.
    Older,
    /// A file there that is newer than the source: replacing it goes back in time.
    Newer,
    /// A file there of the same date but another size, or with no date to compare.
    Differs,
    /// The source is a file and a directory is there.
    FileOverDir,
    /// The source is a directory and a file is there.
    DirOverFile,
    /// Both are directories: the copy goes into it.
    Merge,
}

impl Class {
    /// Whether this row puts a file where one is: a decision to make.
    pub fn conflicts(self) -> bool {
        !matches!(self, Class::New | Class::Merge)
    }
}

/// Why a row cannot be done at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// The source (or a directory of it) could not be read: the error.
    Unreadable(String),
    /// A FIFO, a socket or a device: its contents cannot be copied.
    Special(&'static str),
    /// A directory into itself, which would recurse until the disk filled.
    IntoItself,
    /// The source is its own destination.
    SameFile,
}

/// One thing the job will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub src: PathBuf,
    pub dst: PathBuf,
    /// The source is a directory.
    pub dir: bool,
    /// The bytes it brings: a file's size, a directory's own entry 0 (its files are rows
    /// of their own), a whole subtree for [`Class::DirOverFile`].
    pub bytes: u64,
    pub class: Class,
    /// What was at the destination when the plan looked, to check again right before
    /// replacing it (Round 029's `Ident`).
    pub target: Option<Ident>,
    pub blocked: Option<Blocked>,
}

impl Row {
    /// Whether the user has something to decide about this row, or to be told.
    pub fn needs_deciding(&self) -> bool {
        self.blocked.is_some() || self.class.conflicts()
    }
}

/// The counts a plan's summary line is made of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    /// Files, links and directories the sources hold, blocked ones included.
    pub items: u64,
    /// Bytes of the files that can be copied.
    pub bytes: u64,
    /// Rows with nothing at the destination.
    pub new: u64,
    /// Rows with a file (or a directory over a file) at the destination.
    pub conflicts: u64,
    /// Rows that cannot be done.
    pub blocked: u64,
    /// New rows counted but not kept, past [`CAP`].
    pub dropped: u64,
}

/// What a copy or move will do, from one scan.
#[derive(Debug, Clone)]
pub struct Plan {
    pub kind: Kind,
    pub dest: PathBuf,
    pub rows: Vec<Row>,
    pub totals: Totals,
    /// Whether the sources are on the destination's device: a move there is a rename,
    /// elsewhere a copy and a delete. `None`: it could not be told.
    pub same_device: Option<bool>,
}

impl Plan {
    /// Whether anything in it needs a decision before the job starts; when nothing
    /// does, the F5 dialog is the one it always was.
    pub fn needs_deciding(&self) -> bool {
        self.totals.conflicts > 0 || self.totals.blocked > 0
    }
}

/// What the scan sends while it runs.
#[derive(Debug)]
pub enum Msg {
    /// How far it has got: entries seen, bytes found.
    Progress {
        items: u64,
        bytes: u64,
    },
    Done(Box<Plan>),
}

/// A scan on its own thread. Dropping the handle stops it.
pub struct Planning {
    pub rx: Receiver<Msg>,
    stop: Arc<AtomicBool>,
}

impl Drop for Planning {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Plan copying or moving `sources` into `dest` (a single source under `rename` when
/// given, as Shift+F5 does), on its own thread.
pub fn start(
    kind: Kind,
    sources: Vec<PathBuf>,
    dest: PathBuf,
    rename: Option<std::ffi::OsString>,
) -> Planning {
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        let progress = tx.clone();
        let mut report = |items, bytes| {
            let _ = progress.send(Msg::Progress { items, bytes });
        };
        if let Some(p) = plan_until(kind, &sources, &dest, rename.as_deref(), &flag, &mut report) {
            let _ = tx.send(Msg::Done(Box::new(p)));
        }
    });
    Planning { rx, stop }
}

/// The scan itself: `None` once `stop` is set. `progress` hears how far it has got.
pub fn plan_until(
    kind: Kind,
    sources: &[PathBuf],
    dest: &Path,
    rename: Option<&std::ffi::OsStr>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Option<Plan> {
    plan_capped(kind, sources, dest, rename, stop, progress, CAP)
}

fn plan_capped(
    kind: Kind,
    sources: &[PathBuf],
    dest: &Path,
    rename: Option<&std::ffi::OsStr>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    cap: usize,
) -> Option<Plan> {
    let mut s = Scan {
        rows: Vec::new(),
        totals: Totals::default(),
        stop,
        progress,
        cap,
        seen: 0,
    };
    for src in sources {
        let Some(name) = rename.or(src.file_name()) else {
            continue;
        };
        let dst = dest.join(name);
        if dst == *src {
            s.blocked(src, &dst, false, Blocked::SameFile);
            continue;
        }
        if src.is_dir() && dst.starts_with(src) {
            s.blocked(src, &dst, true, Blocked::IntoItself);
            continue;
        }
        if !s.node(src, &dst, false) {
            return None;
        }
    }
    (s.progress)(s.totals.items, s.totals.bytes);
    let same_device = sources.first().and_then(|src| same_device(src, dest));
    Some(Plan {
        kind,
        dest: dest.to_path_buf(),
        rows: s.rows,
        totals: s.totals,
        same_device,
    })
}

struct Scan<'a> {
    rows: Vec<Row>,
    totals: Totals,
    stop: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64, u64),
    cap: usize,
    seen: u64,
}

impl Scan<'_> {
    /// Plan `src` onto `dst`. `parent_new`: `dst`'s directory is itself new, so nothing
    /// can be there and it is not looked at. `false` once stopped.
    fn node(&mut self, src: &Path, dst: &Path, parent_new: bool) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return false;
        }
        self.seen += 1;
        if self.seen.is_multiple_of(PROGRESS_EVERY) {
            (self.progress)(self.totals.items, self.totals.bytes);
        }
        let meta = match faults::at(Point::Stat, src).and_then(|()| fs::symlink_metadata(src)) {
            Ok(m) => m,
            Err(e) => {
                self.blocked(src, dst, false, Blocked::Unreadable(e.to_string()));
                return true;
            }
        };
        let ft = meta.file_type();
        if let Some(kind) = crate::entry::special_kind(&ft) {
            self.blocked(src, dst, false, Blocked::Special(kind));
            return true;
        }
        let there = if parent_new {
            None
        } else {
            faults::at(Point::Stat, dst)
                .ok()
                .and_then(|()| fs::symlink_metadata(dst).ok())
        };
        let target = there.as_ref().map(Ident::of);
        let is_dir = ft.is_dir();
        if !is_dir {
            let class = match &there {
                None => Class::New,
                Some(d) if d.is_dir() => Class::FileOverDir,
                Some(d) => compare_files(src, &meta, dst, d),
            };
            self.totals.bytes += meta.len();
            self.push(Row {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                dir: false,
                bytes: meta.len(),
                class,
                target,
                blocked: None,
            });
            return true;
        }
        let class = match &there {
            None => Class::New,
            Some(d) if d.is_dir() => Class::Merge,
            Some(_) => Class::DirOverFile,
        };
        if class == Class::DirOverFile {
            // Nothing of it can go there; its files are not rows of their own.
            let (items, bytes) = subtree(src, self.stop);
            self.totals.bytes += bytes;
            self.totals.items += items.saturating_sub(1);
            self.push(Row {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                dir: true,
                bytes,
                class,
                target,
                blocked: None,
            });
            return !self.stop.load(Ordering::Relaxed);
        }
        let entries = match faults::at(Point::ReadDir, src).and_then(|()| fs::read_dir(src)) {
            Ok(it) => it,
            Err(e) => {
                self.blocked(src, dst, true, Blocked::Unreadable(e.to_string()));
                return true;
            }
        };
        self.push(Row {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
            dir: true,
            bytes: 0,
            class,
            target,
            blocked: None,
        });
        let new = class == Class::New;
        for ent in entries {
            match ent {
                Ok(ent) => {
                    if !self.node(&ent.path(), &dst.join(ent.file_name()), new) {
                        return false;
                    }
                }
                Err(e) => {
                    // An entry the system would not give: the directory is incomplete.
                    self.blocked(src, dst, true, Blocked::Unreadable(e.to_string()));
                }
            }
        }
        true
    }

    fn blocked(&mut self, src: &Path, dst: &Path, dir: bool, why: Blocked) {
        self.push(Row {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
            dir,
            bytes: 0,
            class: Class::New,
            target: None,
            blocked: Some(why),
        });
    }

    fn push(&mut self, row: Row) {
        self.totals.items += 1;
        if row.blocked.is_some() {
            self.totals.blocked += 1;
        } else if row.class.conflicts() {
            self.totals.conflicts += 1;
        } else if row.class == Class::New {
            self.totals.new += 1;
        }
        if self.rows.len() >= self.cap && !row.needs_deciding() {
            self.totals.dropped += 1;
            return;
        }
        self.rows.push(row);
    }
}

/// Two files (or links), source and destination, compared as a copy would leave them.
fn compare_files(src: &Path, s: &fs::Metadata, dst: &Path, d: &fs::Metadata) -> Class {
    use std::cmp::Ordering::*;
    if s.file_type().is_symlink() || d.file_type().is_symlink() {
        let same = s.file_type().is_symlink()
            && d.file_type().is_symlink()
            && matches!((fs::read_link(src), fs::read_link(dst)), (Ok(a), Ok(b)) if a == b);
        return if same {
            Class::Identical
        } else {
            Class::Differs
        };
    }
    let dated = s.modified().is_ok() && d.modified().is_ok();
    if !dated {
        return Class::Differs;
    }
    match crate::compare::newer_than(s, d) {
        Equal if s.len() == d.len() => Class::Identical,
        Equal => Class::Differs,
        Greater => Class::Older,
        Less => Class::Newer,
    }
}

/// Entries and bytes under `p`, itself included; what cannot be read is not counted.
fn subtree(p: &Path, stop: &AtomicBool) -> (u64, u64) {
    let mut items = 0;
    let mut bytes = 0;
    let mut todo = vec![p.to_path_buf()];
    while let Some(p) = todo.pop() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(m) = fs::symlink_metadata(&p) else {
            continue;
        };
        items += 1;
        if m.is_dir() {
            if let Ok(it) = fs::read_dir(&p) {
                todo.extend(it.flatten().map(|e| e.path()));
            }
        } else {
            bytes += m.len();
        }
    }
    (items, bytes)
}

/// Whether `src` and the directory `dest` are on one device. `None`: one of them
/// cannot be looked at, or (Windows) a path without a drive or share to compare.
pub fn same_device(src: &Path, dest: &Path) -> Option<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let a = fs::symlink_metadata(src).ok()?.dev();
        let b = fs::metadata(dest).ok()?.dev();
        Some(a == b)
    }
    #[cfg(not(unix))]
    {
        let volume = |p: &Path| -> Option<std::ffi::OsString> {
            let p = crate::paths::canonical(p).ok()?;
            match p.components().next()? {
                std::path::Component::Prefix(x) => Some(x.as_os_str().to_ascii_uppercase()),
                _ => None,
            }
        };
        Some(volume(src)? == volume(dest)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-plan-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn plan(kind: Kind, sources: &[PathBuf], dest: &Path) -> Plan {
        let stop = AtomicBool::new(false);
        plan_until(kind, sources, dest, None, &stop, &mut |_, _| {}).unwrap()
    }

    fn row<'a>(p: &'a Plan, name: &str) -> &'a Row {
        p.rows
            .iter()
            .find(|r| r.src.file_name().is_some_and(|n| n == name))
            .unwrap_or_else(|| panic!("no row for {name}: {:?}", p.rows))
    }

    fn age(p: &Path, secs: u64) {
        let f = fs::File::options().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(secs))
            .unwrap();
    }

    #[test]
    fn a_tree_into_an_empty_place_is_all_new_and_counted() {
        let d = tmp("new");
        let src = d.join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), "aaaa").unwrap();
        fs::write(src.join("sub/b.txt"), "bb").unwrap();
        fs::create_dir(d.join("to")).unwrap();
        let p = plan(Kind::Copy, std::slice::from_ref(&src), &d.join("to"));
        assert_eq!(p.totals.items, 4, "{:?}", p.rows);
        assert_eq!(p.totals.new, 4);
        assert_eq!(p.totals.bytes, 6);
        assert_eq!(p.totals.conflicts, 0);
        assert!(!p.needs_deciding());
        assert_eq!(row(&p, "b.txt").dst, d.join("to/src/sub/b.txt"));
        assert_eq!(p.same_device, Some(true));
    }

    #[test]
    fn every_kind_of_destination_gets_its_class() {
        let d = tmp("classes");
        let (src, to) = (d.join("src"), d.join("to"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&to).unwrap();
        for (name, body) in [
            ("same", "x"),
            ("older", "x"),
            ("newer", "x"),
            ("differs", "xx"),
            ("fileoverdir", "x"),
        ] {
            fs::write(src.join(name), body).unwrap();
        }
        fs::create_dir_all(src.join("diroverfile")).unwrap();
        fs::write(src.join("diroverfile/in"), "123").unwrap();
        fs::create_dir_all(src.join("merge")).unwrap();
        fs::write(src.join("merge/in"), "1").unwrap();
        fs::create_dir_all(to.join("merge")).unwrap();
        fs::create_dir_all(to.join("fileoverdir")).unwrap();
        fs::write(to.join("diroverfile"), "f").unwrap();
        fs::copy(src.join("same"), to.join("same")).unwrap();
        let t = fs::metadata(src.join("same")).unwrap().modified().unwrap();
        fs::File::options()
            .write(true)
            .open(to.join("same"))
            .unwrap()
            .set_modified(t)
            .unwrap();
        fs::write(to.join("older"), "x").unwrap();
        age(&to.join("older"), 3600);
        fs::write(to.join("newer"), "x").unwrap();
        age(&src.join("newer"), 3600);
        fs::write(to.join("differs"), "x").unwrap();
        fs::File::options()
            .write(true)
            .open(to.join("differs"))
            .unwrap()
            .set_modified(
                fs::metadata(src.join("differs"))
                    .unwrap()
                    .modified()
                    .unwrap(),
            )
            .unwrap();
        let sources: Vec<PathBuf> = fs::read_dir(&src)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let p = plan(Kind::Copy, &sources, &to);
        assert_eq!(row(&p, "same").class, Class::Identical);
        assert_eq!(row(&p, "older").class, Class::Older);
        assert_eq!(row(&p, "newer").class, Class::Newer);
        assert_eq!(row(&p, "differs").class, Class::Differs);
        assert_eq!(row(&p, "fileoverdir").class, Class::FileOverDir);
        assert_eq!(row(&p, "diroverfile").class, Class::DirOverFile);
        assert_eq!(row(&p, "diroverfile").bytes, 3);
        assert_eq!(row(&p, "merge").class, Class::Merge);
        // A file inside a directory being merged is looked at; one under a directory
        // over a file is not a row of its own.
        let ins: Vec<_> = p.rows.iter().filter(|r| r.src.ends_with("in")).collect();
        assert_eq!(ins.len(), 1, "{ins:?}");
        assert_eq!(ins[0].dst, to.join("merge/in"));
        assert_eq!(ins[0].class, Class::New);
        assert!(p.needs_deciding());
        assert_eq!(p.totals.conflicts, 6);
        // What was there is kept, to check again before replacing it.
        assert_eq!(row(&p, "older").target, Ident::at(&to.join("older")));
    }

    #[test]
    fn a_directory_that_cannot_be_read_is_a_blocked_row_not_a_gap() {
        let d = tmp("unreadable");
        let src = d.join("src");
        fs::create_dir_all(src.join("locked")).unwrap();
        fs::write(src.join("locked/hidden.txt"), "x").unwrap();
        fs::write(src.join("ok.txt"), "y").unwrap();
        fs::create_dir(d.join("to")).unwrap();
        let _f = faults::fail(
            Point::ReadDir,
            "plan-unreadable",
            1,
            faults::Fault::Kind(std::io::ErrorKind::PermissionDenied),
        );
        // The first directory read is src itself: it fails; nothing under it is planned.
        let p = plan(Kind::Copy, std::slice::from_ref(&src), &d.join("to"));
        assert_eq!(p.totals.blocked, 1, "{:?}", p.rows);
        assert!(matches!(
            row(&p, "src").blocked,
            Some(Blocked::Unreadable(_))
        ));
        assert!(p.needs_deciding());
        drop(_f);
        let _g = faults::fail(
            Point::Stat,
            &src.join("ok.txt").to_string_lossy(),
            1,
            faults::Fault::Kind(std::io::ErrorKind::PermissionDenied),
        );
        let p = plan(Kind::Copy, std::slice::from_ref(&src), &d.join("to"));
        assert!(matches!(
            row(&p, "ok.txt").blocked,
            Some(Blocked::Unreadable(_))
        ));
        assert_eq!(row(&p, "hidden.txt").class, Class::New);
    }

    #[test]
    fn into_itself_and_onto_itself_are_blocked() {
        let d = tmp("itself");
        fs::create_dir_all(d.join("a")).unwrap();
        fs::write(d.join("f"), "x").unwrap();
        let p = plan(Kind::Copy, &[d.join("a")], &d.join("a"));
        assert_eq!(row(&p, "a").blocked, Some(Blocked::IntoItself));
        let p = plan(Kind::Copy, &[d.join("f")], &d);
        assert_eq!(row(&p, "f").blocked, Some(Blocked::SameFile));
    }

    #[test]
    fn a_rename_plans_the_one_source_under_its_new_name() {
        let d = tmp("rename");
        fs::write(d.join("f"), "x").unwrap();
        let stop = AtomicBool::new(false);
        let p = plan_until(
            Kind::Copy,
            &[d.join("f")],
            &d,
            Some(std::ffi::OsStr::new("g")),
            &stop,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(row(&p, "f").dst, d.join("g"));
        assert_eq!(row(&p, "f").class, Class::New);
    }

    #[test]
    fn a_stopped_scan_gives_no_plan() {
        let d = tmp("stop");
        fs::write(d.join("f"), "x").unwrap();
        let stop = AtomicBool::new(true);
        assert!(plan_until(
            Kind::Copy,
            &[d.join("f")],
            &d.join("to"),
            None,
            &stop,
            &mut |_, _| {}
        )
        .is_none());
    }

    #[test]
    fn past_the_cap_only_rows_with_a_decision_are_kept() {
        let d = tmp("cap");
        let (src, to) = (d.join("src"), d.join("to"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&to).unwrap();
        for i in 0..10 {
            fs::write(src.join(format!("f{i}")), "x").unwrap();
        }
        fs::write(to.join("f9"), "older").unwrap();
        let sources: Vec<PathBuf> = (0..10).map(|i| src.join(format!("f{i}"))).collect();
        let stop = AtomicBool::new(false);
        let p = plan_capped(Kind::Copy, &sources, &to, None, &stop, &mut |_, _| {}, 3).unwrap();
        assert_eq!(p.rows.len(), 4, "{:?}", p.rows);
        assert_eq!(p.totals.dropped, 6);
        assert_eq!(p.totals.items, 10);
        assert!(p.rows.iter().any(|r| r.src.ends_with("f9")));
    }

    #[test]
    fn the_scan_on_its_thread_reports_and_ends_with_the_plan() {
        let d = tmp("thread");
        fs::create_dir_all(d.join("src")).unwrap();
        for i in 0..(PROGRESS_EVERY + 10) {
            fs::write(d.join(format!("src/f{i}")), "x").unwrap();
        }
        let h = start(Kind::Move, vec![d.join("src")], d.join("to"), None);
        let mut progress = 0;
        let plan = loop {
            match h.rx.recv_timeout(Duration::from_secs(20)).unwrap() {
                Msg::Progress { .. } => progress += 1,
                Msg::Done(p) => break p,
            }
        };
        assert!(progress >= 1);
        assert_eq!(plan.totals.items, PROGRESS_EVERY + 11);
        assert_eq!(plan.kind, Kind::Move);
    }
}
