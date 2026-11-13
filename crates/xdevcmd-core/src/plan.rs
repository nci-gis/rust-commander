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

use std::collections::hash_map::{Entry, HashMap};
use std::ffi::{OsStr, OsString};
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
    /// A name the destination cannot hold (Windows' rules, on Windows or on a FAT,
    /// exFAT or NTFS drive): why.
    BadName(&'static str),
    /// A name longer than the destination allows: 255 bytes, or 255 UTF-16 units where
    /// Windows' rules apply.
    TooLong,
    /// A file of 4 GiB or more, onto a FAT drive, which cannot hold one.
    TooBig,
}

/// Something the user is told before the job, and asked about: data that would be lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warn {
    /// The destination ignores case and already holds, or is also getting, this other
    /// name that differs only in case: the two would be one file there.
    CaseClash(OsString),
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
    pub warn: Option<Warn>,
}

impl Row {
    /// Whether the user has something to decide about this row, or to be told.
    pub fn needs_deciding(&self) -> bool {
        self.blocked.is_some() || self.warn.is_some() || self.class.conflicts()
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
    /// Rows with a [`Warn`].
    pub warned: u64,
    /// New rows counted but not kept, past [`CAP`].
    pub dropped: u64,
    /// Bytes the job writes at the destination (what is identical is not copied).
    pub writes: u64,
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
    /// What the destination is and allows, as far as it could be told.
    pub target: Target,
}

impl Plan {
    /// Whether anything in it needs a decision before the job starts; when nothing
    /// does, the F5 dialog is the one it always was.
    pub fn needs_deciding(&self) -> bool {
        self.totals.conflicts > 0
            || self.totals.blocked > 0
            || self.totals.warned > 0
            || self.target.writable == Some(false)
            || self.short_of_space().is_some()
    }

    /// The bytes the destination needs free: none for a move on one device (a rename).
    pub fn needed(&self) -> u64 {
        if self.kind == Kind::Move && self.same_device == Some(true) {
            0
        } else {
            self.totals.writes
        }
    }

    /// `(needed, free)` when the destination has less free than the job writes.
    pub fn short_of_space(&self) -> Option<(u64, u64)> {
        let free = self.target.free?;
        let needed = self.needed();
        (needed > free).then_some((needed, free))
    }
}

/// The largest file a FAT drive holds.
const FAT_MAX: u64 = u32::MAX as u64;

/// What kind of file system a destination is, as far as names and sizes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fs {
    /// FAT12/16/32: files under 4 GiB, Windows' names, case ignored.
    Fat,
    Exfat,
    Ntfs,
    Other,
    /// It could not be told (Windows, for beta.1: owner, 2026-11-13).
    Unknown,
}

/// What a destination is and allows. Each `None` is "unknown", never "fine".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub fs: Fs,
    /// Bytes free for this user.
    pub free: Option<u64>,
    pub writable: Option<bool>,
    /// Whether two names that differ only in case are one file there.
    pub ignores_case: Option<bool>,
}

impl Target {
    /// Nothing known.
    pub const UNKNOWN: Target = Target {
        fs: Fs::Unknown,
        free: None,
        writable: None,
        ignores_case: None,
    };

    /// Whether Windows' rules for names apply there.
    pub fn windows_names(&self) -> bool {
        cfg!(windows) || matches!(self.fs, Fs::Fat | Fs::Exfat | Fs::Ntfs)
    }
}

/// Look at the directory `dest`: its file system, free space, whether this user can
/// write there, whether it ignores case. Reads only.
pub fn target(dest: &Path) -> Target {
    let fs = fs_kind(dest);
    Target {
        fs,
        free: free_space(dest),
        writable: writable(dest),
        ignores_case: ignores_case(dest, fs),
    }
}

#[cfg(target_os = "linux")]
fn fs_kind(dest: &Path) -> Fs {
    let Ok(st) = rustix::fs::statfs(dest) else {
        return Fs::Unknown;
    };
    #[allow(clippy::unnecessary_cast)]
    match st.f_type as i64 {
        0x4d44 => Fs::Fat,
        0x2011_bab0 => Fs::Exfat,
        0x5346_544e | 0x7366_746e => Fs::Ntfs,
        // ntfs-3g and exfat-fuse are both "fuseblk": not told apart.
        0x6573_5546 => Fs::Unknown,
        _ => Fs::Other,
    }
}

#[cfg(target_os = "macos")]
fn fs_kind(dest: &Path) -> Fs {
    let Ok(st) = rustix::fs::statfs(dest) else {
        return Fs::Unknown;
    };
    let name: Vec<u8> = st
        .f_fstypename
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    match name.as_slice() {
        b"msdos" => Fs::Fat,
        b"exfat" => Fs::Exfat,
        b"ntfs" | b"ufsd_NTFS" => Fs::Ntfs,
        _ => Fs::Other,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn fs_kind(_: &Path) -> Fs {
    Fs::Unknown
}

/// Bytes free at `dest` for this user; `None` where it cannot be asked (Windows).
pub fn free_space(dest: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let st = rustix::fs::statvfs(dest).ok()?;
        Some(st.f_bavail.saturating_mul(st.f_frsize))
    }
    #[cfg(not(unix))]
    {
        let _ = dest;
        None
    }
}

fn writable(dest: &Path) -> Option<bool> {
    #[cfg(unix)]
    {
        use rustix::io::Errno;
        match rustix::fs::access(dest, rustix::fs::Access::WRITE_OK) {
            Ok(()) => Some(true),
            Err(e) if e == Errno::ACCESS || e == Errno::ROFS || e == Errno::PERM => Some(false),
            Err(_) => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dest;
        None
    }
}

/// Whether `dest` ignores case: Windows and FAT/exFAT do; elsewhere `dest` is looked up
/// under its own name with the case of its letters turned over (macOS' default volume
/// ignores case, Linux' file systems do not). `None` when its name has no letters.
fn ignores_case(dest: &Path, fs: Fs) -> Option<bool> {
    if cfg!(windows) || matches!(fs, Fs::Fat | Fs::Exfat) {
        return Some(true);
    }
    let name = dest.file_name()?.to_string_lossy().into_owned();
    let flipped: String = name
        .chars()
        .flat_map(|c| {
            if c.is_lowercase() {
                c.to_uppercase().collect::<Vec<_>>()
            } else {
                c.to_lowercase().collect::<Vec<_>>()
            }
        })
        .collect();
    if flipped == name {
        return None;
    }
    let other = dest.with_file_name(&flipped);
    if faults::found(Point::Lookup, &other) {
        return Some(true);
    }
    match (Ident::at(dest), Ident::at(&other)) {
        (Some(a), Some(b)) => Some(a == b),
        (Some(_), None) => Some(false),
        _ => None,
    }
}

/// Why `name` cannot be made at a destination, if it cannot: `windows` when Windows'
/// rules apply there.
pub fn name_problem(name: &OsStr, windows: bool) -> Option<Blocked> {
    if !windows {
        return (name.len() > 255).then_some(Blocked::TooLong);
    }
    let s = name.to_string_lossy();
    if s.chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\') || c < ' ')
    {
        return Some(Blocked::BadName(
            "it holds a character Windows does not allow (< > : \" | ? * \\ or a control character)",
        ));
    }
    if s.ends_with('.') || s.ends_with(' ') {
        return Some(Blocked::BadName("it ends with a dot or a space"));
    }
    let stem = s
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved {
        return Some(Blocked::BadName("Windows keeps that name for a device"));
    }
    (s.encode_utf16().count() > 255).then_some(Blocked::TooLong)
}

/// The names in one destination directory, folded, where the destination ignores case:
/// what is there already and what the job brings.
struct Names {
    on: bool,
    seen: HashMap<String, OsString>,
}

impl Names {
    /// `dir`'s names, read once (none when it is new, or case is not ignored).
    fn of(dir: &Path, new: bool, t: &Target) -> Self {
        let on = t.ignores_case == Some(true);
        let mut seen = HashMap::new();
        if on && !new {
            if let Ok(it) = fs::read_dir(dir) {
                for e in it.flatten() {
                    let n = e.file_name();
                    seen.insert(fold(&n), n);
                }
            }
        }
        Self { on, seen }
    }

    /// Take `name` for an incoming row: the other name it clashes with, if one does.
    fn claim(&mut self, name: &OsStr) -> Option<OsString> {
        if !self.on {
            return None;
        }
        match self.seen.entry(fold(name)) {
            Entry::Occupied(o) => (o.get().as_os_str() != name).then(|| o.get().clone()),
            Entry::Vacant(v) => {
                v.insert(name.to_os_string());
                None
            }
        }
    }
}

fn fold(n: &OsStr) -> String {
    n.to_string_lossy().to_lowercase()
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
    plan_capped(
        kind,
        sources,
        dest,
        rename,
        stop,
        progress,
        target(dest),
        CAP,
    )
}

// The scan with what `plan_until` looks up given, for tests to choose the destination.
#[allow(clippy::too_many_arguments)]
fn plan_capped(
    kind: Kind,
    sources: &[PathBuf],
    dest: &Path,
    rename: Option<&std::ffi::OsStr>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    target: Target,
    cap: usize,
) -> Option<Plan> {
    let mut s = Scan {
        rows: Vec::new(),
        totals: Totals::default(),
        stop,
        progress,
        cap,
        seen: 0,
        t: target,
    };
    let mut top = Names::of(dest, false, &target);
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
        if !s.node(src, &dst, false, &mut top) {
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
        target,
    })
}

struct Scan<'a> {
    rows: Vec<Row>,
    totals: Totals,
    stop: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64, u64),
    cap: usize,
    seen: u64,
    t: Target,
}

impl Scan<'_> {
    /// Plan `src` onto `dst`. `parent_new`: `dst`'s directory is itself new, so nothing
    /// can be there and it is not looked at. `false` once stopped.
    fn node(&mut self, src: &Path, dst: &Path, parent_new: bool, names: &mut Names) -> bool {
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
            self.push_vetted(
                names,
                Row {
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    dir: false,
                    bytes: meta.len(),
                    class,
                    target,
                    blocked: None,
                    warn: None,
                },
            );
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
            self.push_vetted(
                names,
                Row {
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    dir: true,
                    bytes,
                    class,
                    target,
                    blocked: None,
                    warn: None,
                },
            );
            return !self.stop.load(Ordering::Relaxed);
        }
        let entries = match faults::at(Point::ReadDir, src).and_then(|()| fs::read_dir(src)) {
            Ok(it) => it,
            Err(e) => {
                self.blocked(src, dst, true, Blocked::Unreadable(e.to_string()));
                return true;
            }
        };
        let mut row = Row {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
            dir: true,
            bytes: 0,
            class,
            target,
            blocked: None,
            warn: None,
        };
        self.vet(&mut row, names);
        if row.blocked.is_some() {
            // A directory that cannot be made there: nothing under it can go either.
            let (items, _) = subtree(src, self.stop);
            self.totals.items += items.saturating_sub(1);
            self.push(row);
            return !self.stop.load(Ordering::Relaxed);
        }
        self.push(row);
        let new = class == Class::New;
        let mut inner = Names::of(dst, new, &self.t);
        for ent in entries {
            match ent {
                Ok(ent) => {
                    if !self.node(&ent.path(), &dst.join(ent.file_name()), new, &mut inner) {
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
            warn: None,
        });
    }

    /// The destination's rules on a row's name and size, then [`Scan::push`].
    fn push_vetted(&mut self, names: &mut Names, mut row: Row) {
        self.vet(&mut row, names);
        self.push(row);
    }

    fn vet(&mut self, row: &mut Row, names: &mut Names) {
        let Some(name) = row.dst.file_name() else {
            return;
        };
        if let Some(why) = name_problem(name, self.t.windows_names()) {
            row.blocked = Some(why);
            return;
        }
        if !row.dir && self.t.fs == Fs::Fat && row.bytes > FAT_MAX {
            row.blocked = Some(Blocked::TooBig);
            return;
        }
        if let Some(other) = names.claim(name) {
            row.warn = Some(Warn::CaseClash(other));
        }
    }

    fn push(&mut self, row: Row) {
        self.totals.items += 1;
        if row.blocked.is_none() && row.warn.is_some() {
            self.totals.warned += 1;
        }
        if row.blocked.is_none()
            && row.class != Class::Identical
            && (!row.dir || row.class == Class::DirOverFile)
        {
            self.totals.writes += row.bytes;
        }
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
        let p = plan_capped(
            Kind::Copy,
            &sources,
            &to,
            None,
            &stop,
            &mut |_, _| {},
            Target::UNKNOWN,
            3,
        )
        .unwrap();
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

    fn plan_on(t: Target, kind: Kind, sources: &[PathBuf], dest: &Path) -> Plan {
        let stop = AtomicBool::new(false);
        plan_capped(kind, sources, dest, None, &stop, &mut |_, _| {}, t, CAP).unwrap()
    }

    const FAT: Target = Target {
        fs: Fs::Fat,
        free: None,
        writable: Some(true),
        ignores_case: Some(true),
    };

    #[test]
    fn windows_names_are_refused_by_their_rules() {
        let bad = |n: &str| name_problem(OsStr::new(n), true);
        for n in [
            "a:b",
            "a?",
            "q\"x",
            "a|b",
            "a*",
            "a<b",
            "tab\there",
            "dot.",
            "space ",
        ] {
            assert!(matches!(bad(n), Some(Blocked::BadName(_))), "{n}");
        }
        for n in ["CON", "nul.txt", "Com1", "LPT9.log", "aux "] {
            assert!(matches!(bad(n), Some(Blocked::BadName(_))), "{n}");
        }
        for n in [
            "CONSOLE",
            "COM0",
            "COM10",
            "a.b",
            "Tiếng Việt.txt",
            "日本語",
        ] {
            assert_eq!(bad(n), None, "{n}");
        }
        assert_eq!(bad(&"é".repeat(255)), None);
        assert_eq!(bad(&"é".repeat(256)), Some(Blocked::TooLong));
        // Elsewhere only the length, in bytes.
        assert_eq!(name_problem(OsStr::new("a:b"), false), None);
        assert_eq!(
            name_problem(OsStr::new(&"é".repeat(128)), false),
            Some(Blocked::TooLong)
        );
    }

    #[test]
    fn a_fat_drive_refuses_bad_names_and_big_files_and_sees_case_clashes() {
        let d = tmp("fat");
        let (a, b, to) = (d.join("a"), d.join("b"), d.join("to"));
        for x in [&a, &b, &to] {
            fs::create_dir_all(x).unwrap();
        }
        fs::write(a.join("bad:name"), "x").unwrap();
        fs::write(a.join("X.txt"), "x").unwrap();
        fs::write(b.join("x.txt"), "y").unwrap();
        fs::write(a.join("README"), "new").unwrap();
        fs::write(to.join("Readme"), "old").unwrap();
        fs::create_dir_all(a.join("dir?/deep")).unwrap();
        fs::write(a.join("dir?/deep/f"), "z").unwrap();
        let big = fs::File::create(a.join("big")).unwrap();
        big.set_len(FAT_MAX + 1).unwrap();
        let sources = [
            a.join("bad:name"),
            a.join("X.txt"),
            b.join("x.txt"),
            a.join("README"),
            a.join("dir?"),
            a.join("big"),
        ];
        let p = plan_on(FAT, Kind::Copy, &sources, &to);
        assert!(matches!(
            row(&p, "bad:name").blocked,
            Some(Blocked::BadName(_))
        ));
        assert_eq!(row(&p, "big").blocked, Some(Blocked::TooBig));
        // A directory that cannot be made: one row, nothing under it planned.
        assert!(matches!(row(&p, "dir?").blocked, Some(Blocked::BadName(_))));
        assert!(
            !p.rows.iter().any(|r| r.src.ends_with("deep")),
            "{:?}",
            p.rows
        );
        assert_eq!(row(&p, "X.txt").warn, None);
        assert_eq!(row(&p, "x.txt").warn, Some(Warn::CaseClash("X.txt".into())));
        assert_eq!(
            row(&p, "README").warn,
            Some(Warn::CaseClash("Readme".into()))
        );
        assert_eq!(p.totals.blocked, 3);
        assert_eq!(p.totals.warned, 2);
        assert_eq!(p.totals.items, 8);
        assert!(p.needs_deciding());
        // Where case is kept apart, the same names are fine.
        let ext = Target {
            fs: Fs::Other,
            ignores_case: Some(false),
            ..FAT
        };
        let p = plan_on(ext, Kind::Copy, &sources[1..3], &to);
        assert_eq!(p.totals.warned, 0);
        assert!(!p.needs_deciding());
    }

    #[test]
    fn free_space_counts_what_is_written_and_a_move_on_one_device_needs_none() {
        let d = tmp("space");
        let (src, to) = (d.join("src"), d.join("to"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(src.join("a"), "12345").unwrap();
        fs::write(src.join("same"), "123").unwrap();
        fs::copy(src.join("same"), to.join("same")).unwrap();
        fs::File::options()
            .write(true)
            .open(to.join("same"))
            .unwrap()
            .set_modified(fs::metadata(src.join("same")).unwrap().modified().unwrap())
            .unwrap();
        let tight = Target {
            free: Some(4),
            ..Target::UNKNOWN
        };
        let sources = [src.join("a"), src.join("same")];
        let p = plan_on(tight, Kind::Copy, &sources, &to);
        // The identical file is not written.
        assert_eq!(p.needed(), 5);
        assert_eq!(p.short_of_space(), Some((5, 4)));
        assert!(p.needs_deciding());
        let p = plan_on(tight, Kind::Move, &sources[..1], &to);
        assert_eq!(p.same_device, Some(true));
        assert_eq!(p.needed(), 0);
        assert_eq!(p.short_of_space(), None);
        // The real look at a temporary directory: something free, not FAT.
        let t = target(&to);
        #[cfg(unix)]
        assert!(t.free.is_some_and(|f| f > 0));
        assert!(!matches!(t.fs, Fs::Fat));
    }

    #[cfg(unix)]
    #[test]
    fn a_destination_this_user_cannot_write_to_is_said_before_the_start() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("ro");
        let to = d.join("to");
        fs::create_dir_all(&to).unwrap();
        fs::write(d.join("f"), "x").unwrap();
        fs::set_permissions(&to, fs::Permissions::from_mode(0o555)).unwrap();
        let t = target(&to);
        fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).unwrap();
        if t.writable == Some(true) {
            eprintln!("skipped: this user (root?) writes anywhere");
            return;
        }
        assert_eq!(t.writable, Some(false));
        let p = plan_on(t, Kind::Copy, &[d.join("f")], &to);
        assert!(p.needs_deciding());
    }

    #[test]
    fn whether_case_is_ignored_is_looked_up_not_guessed() {
        let d = tmp("CaseProbe");
        let t = target(&d);
        if cfg!(windows) {
            assert_eq!(t.ignores_case, Some(true));
            return;
        }
        let real = t.ignores_case;
        assert!(real.is_some(), "{t:?}");
        // The lookup is of the whole name turned over: "XDEVCMD-PLAN-cASEpROBE-…".
        let _g = faults::set(Point::Lookup, "cASEpROBE", faults::Action::Found);
        assert_eq!(target(&d).ignores_case, Some(true));
    }
}
