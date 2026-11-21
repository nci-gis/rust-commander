//! Safe operations' journal: what a job is in the middle of, written before it does it,
//! so that a run ended without warning (`kill -9`, a power cut) can be tidied up and
//! offered again at the next start.
//!
//! One small file per job in the state directory, `<run>-<n>.journal`, one line per
//! fact: the host, the job (its kind, sources, destination), each directory it makes
//! temporary files in (`dir`, once per directory: their names carry the run, so a later
//! start finds them there without a line per file), and rename many's temporary names
//! (`ren`, the user's own files under another name) until they are renamed on (`unren`).
//! A job that ends, however it ends while xdevcmd runs, removes its file.
//!
//! **Telling a dead run from a live one.** A job holds a lock on its journal for as long
//! as it runs, and the system lets go of it when the process ends, however it ends: a
//! journal another start can lock is a dead run's. Unix: `flock`. Windows: the journal
//! is open without sharing write access, so another start's attempt to open it for
//! writing (sharing only deletion) fails while the job runs. A date or a bare pid would
//! not do: a job blocked on a slow drive stops touching its file, and a pid is reused.
//! Where the file system has no locks (some network ones), the pid is asked, and only
//! for a journal written on this machine. A journal from another host (a home directory
//! shared over NFS) is always left alone: its job may be running there.
//!
//! **The run** is `<pid>x<start time, hex>`: it names the journal, the part files and
//! rename many's temporary names, so a later process given the same pid never matches a
//! dead run's files, nor a dead run's journal a live process's files.
//!
//! **Durable where it matters** (Round 029, after SQLite's rollback journal). At the
//! start the journal is synced, and on Unix its directory too, before the job's first
//! step. Later lines are written and flushed as they come, and synced by
//! [`Journal::commit`] right before a step that cannot be undone (a rename that replaces
//! a file, a source removed after its copy, rename many's first step): what that step
//! needs to be recovered is on disk before it is taken. A new file's temporary name is
//! not synced on its own: lost in a power cut, it leaves a part file unknown to the
//! journal, never a wrong action on the user's files.
//!
//! **A journal that fails is broken for good.** A line or a sync that fails marks it
//! broken and every later call fails too: a later line that went through would make a
//! journal with a hole in it look whole. The job is told (the error comes back) and asks
//! the user before its next step that cannot be undone.
//!
//! **A big file goes on from its last checkpoint** (Round 033). A file copied under a
//! temporary name is synced every [`crate::ops::CHECKPOINT`] bytes, and then a `ckpt`
//! line says how much of it is on disk, with the identity of its source. The tidy-up
//! keeps such a temporary file (and this journal, for the next start too) instead of
//! removing it, and Resume takes it over: the copy goes on from there if the source is
//! still the same file. Declined, both go ([`discard`]).
//!
//! `v 3` names this layout. `moved` lines came with `v 2` (Round 029); `skip`, `replace`
//! (the confirmed plan's decisions) and `done` (a source finished) with `v 3` (Round
//! 030), for Resume to carry out exactly what is left. All are unknown tags to 0.3.0's
//! tidy-up, which skips them, and this version reads older journals as before.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Journals this run offered and has not had an answer for, each with the probe that
/// holds it (red-team F1): another start finds them alive, as it does a running job's,
/// and does not offer the same job too. Let go by [`discard`] and [`forget`], or when
/// this run ends.
static HELD: std::sync::Mutex<Vec<(PathBuf, File)>> = std::sync::Mutex::new(Vec::new());

/// Tests: this run ends for the journals under `dir`, as a quit lets go of them all, so
/// the next `recover_in` there is the next start's.
#[cfg(test)]
fn next_start(dir: &Path) {
    if let Ok(mut held) = HELD.lock() {
        held.retain(|(p, _)| !p.starts_with(dir));
    }
}

/// Let go of `journal`'s probe, if this run holds it.
fn let_go(journal: &Path) {
    if let Ok(mut held) = HELD.lock() {
        held.retain(|(p, _)| p != journal);
    }
}

/// Where journals go: a directory of their own in the state directory. Without it,
/// there is no journal (nothing is recorded, nothing recovered).
pub fn init(dir: PathBuf) {
    let _ = DIR.set(dir);
}

/// This process's run: `<pid>x<start time in nanoseconds, hex>`. In every temporary
/// name xdevcmd makes, so that only this run's files are ever taken for this run's.
pub fn run_tag() -> &'static str {
    static RUN: OnceLock<String> = OnceLock::new();
    RUN.get_or_init(|| {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{}x{t:x}", std::process::id())
    })
}

/// This machine's name, to tell its journals from another's in a shared home directory.
/// Empty when the system will not say.
pub fn hostname() -> String {
    #[cfg(unix)]
    {
        rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(unix))]
    {
        std::env::var("COMPUTERNAME").unwrap_or_default()
    }
}

/// A job's journal. Removed when dropped: a job that ends while xdevcmd runs, finished,
/// cancelled or failed, leaves nothing; only a killed process leaves its file.
pub struct Journal {
    path: PathBuf,
    /// Open, and locked, for the life of the job: the lock is what says it is alive.
    file: File,
    /// Directories already written down.
    dirs: std::collections::HashSet<PathBuf>,
    /// Why a line or a sync failed: once it has, the journal is not to be trusted.
    broken: Option<String>,
    /// Lines written since the last sync.
    dirty: bool,
}

/// What trying a journal's lock found.
#[derive(Debug, PartialEq, Eq)]
enum Lock {
    /// Ours now: the run that wrote it is gone.
    Held,
    /// Someone holds it: the run is alive. Unix only: on Windows a held journal does
    /// not open at all.
    #[cfg_attr(not(unix), allow(dead_code))]
    Busy,
    /// The file system has no locks. Unix only: on Windows the open is the lock.
    #[cfg_attr(not(unix), allow(dead_code))]
    Unsupported,
}

/// Try to take `file`'s lock without waiting.
#[cfg(unix)]
fn try_lock(file: &File) -> Lock {
    use rustix::fs::{flock, FlockOperation};
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Lock::Held,
        Err(e) if e == rustix::io::Errno::WOULDBLOCK => Lock::Busy,
        // ENOLCK, EOPNOTSUPP: a file system without locks. Anything else is no better
        // an answer, and the fallback is the cautious one.
        Err(_) => Lock::Unsupported,
    }
}

/// Open a journal so that, while it is open, nobody else can open it for writing.
///
/// Windows: not `FILE_FLAG_DELETE_ON_CLOSE`. The system would remove the journal when
/// the process ends however it ends, a killed one included, and a killed run's journal
/// is exactly the one that must stay for the next start to tidy up after.
fn open_held(path: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_DELETE: readable, and the job can rename and
        // remove its own journal while it holds it; never writable by another.
        o.share_mode(0x1 | 0x4);
    }
    o.open(path)
}

/// Open someone's journal to see whether its run is gone; on Windows, the open itself
/// is the test. `None`: it cannot be opened, or (Windows) its run holds it.
fn open_probe(path: &Path) -> Option<File> {
    let mut o = OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Writing, sharing only deletion: refused (a sharing violation) while the job
        // that wrote it has it open (it shares no writing), and while another start's
        // probe has it (that shares no writing either); granted once both are gone.
        // FILE_SHARE_DELETE (0x4) is what lets this start remove the journal while it
        // still holds the probe: `remove_file` opens the file for deletion, which a
        // handle sharing nothing refuses, so the journal would never go. Removed while
        // held, the name is gone (or delete-pending, which no open is granted) before
        // the probe closes, so no other start ever gets it in between.
        o.write(true).share_mode(0x4);
    }
    o.open(path).ok()
}

/// Whether the run behind a probe opened with [`open_probe`] is gone.
fn probe_lock(file: &File) -> Lock {
    #[cfg(unix)]
    {
        try_lock(file)
    }
    #[cfg(not(unix))]
    {
        // Opened at all, with nothing shared: nobody else has it.
        let _ = file;
        Lock::Held
    }
}

impl Journal {
    /// Start a journal for a job of `kind` ("copy", "move", "delete", "extract", "pack",
    /// "sync", "rename"). `None` when journals are off; an error when the file cannot be
    /// made or synced, for the job to ask whether to go on without one. Returned synced,
    /// with its directory (Unix): on disk before the job's first step.
    pub fn begin(
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        to_trash: bool,
    ) -> Option<std::io::Result<Self>> {
        Self::begin_as(kind, sources, dest, None, to_trash)
    }

    /// [`begin`](Self::begin) for a copy given a new name (Copy As): the journal keeps
    /// the name, for a resume to use it (repo review RR04).
    pub fn begin_as(
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        rename_to: Option<&std::ffi::OsStr>,
        to_trash: bool,
    ) -> Option<std::io::Result<Self>> {
        Some(Self::begin_in_as(
            DIR.get()?,
            kind,
            sources,
            dest,
            rename_to,
            to_trash,
        ))
    }

    #[cfg(test)]
    pub(crate) fn begin_in(
        dir: &Path,
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        to_trash: bool,
    ) -> std::io::Result<Self> {
        Self::begin_in_as(dir, kind, sources, dest, None, to_trash)
    }

    pub(crate) fn begin_in_as(
        dir: &Path,
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        rename_to: Option<&std::ffi::OsStr>,
        to_trash: bool,
    ) -> std::io::Result<Self> {
        static N: AtomicU64 = AtomicU64::new(0);
        // What the seam knows the job by: its first source, else its destination.
        let about = sources
            .first()
            .map(PathBuf::as_path)
            .or(dest)
            .unwrap_or(dir);
        crate::faults::at(crate::faults::Point::JournalBegin, about)?;
        fs::create_dir_all(dir)?;
        // A journal names the user's files by their full paths: the user's to read only,
        // also when an older xdevcmd made the directory open to others.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        let n = N.fetch_add(1, Ordering::Relaxed);
        let name = format!("{}-{n}.journal", run_tag());
        // Made under a hidden name and locked before it takes its real one, so no start
        // ever sees a journal by its real name that is not yet locked.
        let new = dir.join(format!(".{name}.new"));
        let file = open_held(&new)?;
        #[cfg(test)]
        before_lock_in_test(&new);
        // Busy: another start is looking at it, has taken it for a dead run's, and is
        // removing it. Renamed unlocked, the next start would take this live job for a
        // dead one and remove its part files: no journal is better.
        #[cfg(unix)]
        if try_lock(&file) == Lock::Busy {
            let _ = fs::remove_file(&new);
            return Err(std::io::Error::other(
                "another xdevcmd starting took its journal",
            ));
        }
        let path = dir.join(name);
        if let Err(e) = fs::rename(&new, &path) {
            let _ = fs::remove_file(&new);
            return Err(e);
        }
        // From here a failure drops `j`, which removes the file.
        let mut j = Self {
            path,
            file,
            dirs: std::collections::HashSet::new(),
            broken: None,
            dirty: false,
        };
        j.line(&format!("host {}", enc(Path::new(&hostname()))), about)?;
        j.line("v 3", about)?;
        j.line(&format!("kind {kind}"), about)?;
        for s in sources {
            j.line(&format!("src {}", enc(s)), about)?;
        }
        if let Some(d) = dest {
            j.line(&format!("dest {}", enc(d)), about)?;
        }
        if let Some(n) = rename_to {
            j.line(&format!("as {}", enc(Path::new(n))), about)?;
        }
        if to_trash {
            j.line("trash", about)?;
        }
        // The file, then the name it has in its directory: without the second, a power
        // cut can lose the journal itself (SQLite syncs the directory for this).
        j.commit(about)?;
        crate::faults::sync_dir(dir)?;
        Ok(j)
    }

    /// Write one line about `about` (a path, for the fault seam). A journal that has
    /// failed once fails every line after.
    fn line(&mut self, s: &str, about: &Path) -> std::io::Result<()> {
        if let Some(why) = &self.broken {
            return Err(std::io::Error::other(why.clone()));
        }
        let r = crate::faults::at(crate::faults::Point::JournalLine, about)
            .and_then(|()| writeln!(self.file, "{s}"))
            .and_then(|()| self.file.flush());
        match r {
            Ok(()) => {
                self.dirty = true;
                Ok(())
            }
            Err(e) => {
                self.broken = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Sync the lines written since the last time, before the step about `about` that
    /// cannot be undone. Nothing to do when none were. A failed sync breaks the journal:
    /// its lines may be gone from the disk with the error cleared (Linux), so it is never
    /// synced again and taken for whole.
    pub fn commit(&mut self, about: &Path) -> std::io::Result<()> {
        if let Some(why) = &self.broken {
            return Err(std::io::Error::other(why.clone()));
        }
        // Passed whether or not there is anything to sync: a test can then count on it
        // before every step that cannot be undone.
        let r = crate::faults::at(crate::faults::Point::JournalSync, about).and_then(|()| {
            if !self.dirty || crate::faults::skipped(crate::faults::Point::JournalSync, about) {
                return Ok(());
            }
            self.file.sync_data()
        });
        match r {
            Ok(()) => {
                self.dirty = false;
                Ok(())
            }
            Err(e) => {
                self.broken = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Why this journal can no longer be trusted, if it cannot.
    pub fn broken(&self) -> Option<&str> {
        self.broken.as_deref()
    }

    /// About to make `p`, a temporary name: its directory is written down, once.
    pub fn part(&mut self, p: &Path) -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            if !self.dirs.contains(dir) {
                self.line(&format!("dir {}", enc(dir)), p)?;
                self.dirs.insert(dir.to_path_buf());
            }
        }
        Ok(())
    }

    /// About to rename `orig` to the temporary name `tmp` (rename many).
    pub fn renamed(&mut self, tmp: &Path, orig: &Path) -> std::io::Result<()> {
        self.line(&format!("ren {} {}", enc(tmp), enc(orig)), orig)
    }

    /// `tmp` has been renamed on: nothing to put back.
    pub fn unrenamed(&mut self, tmp: &Path) -> std::io::Result<()> {
        self.line(&format!("unren {}", enc(tmp)), tmp)
    }

    /// A move across devices: `dst`, a complete copy of `src`, is on disk, and `src` is
    /// about to be removed. Synced by the [`Journal::commit`] before the removal.
    pub fn moved(&mut self, src: &Path, dst: &Path) -> std::io::Result<()> {
        self.line(&format!("moved {} {}", enc(src), enc(dst)), src)
    }

    /// The confirmed plan's word on `dst` (Round 030): left, or replaced while it is the
    /// file the plan saw. A resume keeps it instead of asking again. Nothing for a row
    /// the job will ask about.
    pub fn decided(&mut self, dst: &Path, e: &crate::ops::Expect) -> std::io::Result<()> {
        use crate::ops::Expect;
        match e {
            Expect::Skip => self.line(&format!("skip {}", enc(dst)), dst),
            Expect::Replace(id) => self.line(&format!("replace {} {}", enc(dst), id.encode()), dst),
            Expect::Absent => Ok(()),
        }
    }

    /// About to put `dst` in the trash and rename `tmp`, its replacement, complete and
    /// synced, over it (Round 031). Killed in between, the next start finishes the
    /// rename: the new file is the one that was confirmed, and the old one is in the
    /// trash. Synced by the commit before the trash step.
    pub fn swap(&mut self, tmp: &Path, dst: &Path) -> std::io::Result<()> {
        self.line(&format!("swap {} {}", enc(tmp), enc(dst)), dst)
    }

    /// `p.bytes` of `p.part`, the copy of `p.src` that is to become `p.dst`, are on
    /// disk (synced just before): kept for a resume. Not synced on its own: lost, the
    /// copy goes on from the checkpoint before, or from the start.
    pub fn checkpoint(&mut self, p: &Partial) -> std::io::Result<()> {
        self.line(
            &format!(
                "ckpt {} {} {} {} {}",
                enc(&p.part),
                enc(&p.src),
                enc(&p.dst),
                p.ident.encode(),
                p.bytes
            ),
            &p.part,
        )
    }

    /// The job has finished with `src`, one of its sources: a resume leaves it.
    pub fn done(&mut self, src: &Path) -> std::io::Result<()> {
        self.line(&format!("done {}", enc(src)), src)
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        // Removed while still locked (the file closes after this): a start that finds
        // it must never get the lock of a job that ended normally while the name is
        // still there. One that opened it just before is told by its link count.
        let _ = fs::remove_file(&self.path);
    }
}

/// A big file cut short after a checkpoint: `bytes` of `part` are the start of `src`,
/// whose identity was `ident`, being copied to `dst`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partial {
    pub part: PathBuf,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub ident: crate::ops::Ident,
    pub bytes: u64,
}

/// What an interrupted run was doing, found at start-up, after its leftovers were
/// cleaned up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupted {
    pub kind: String,
    pub sources: Vec<PathBuf>,
    pub dest: Option<PathBuf>,
    /// The name Copy As gave the copy, in place of the source's (RR04).
    pub rename_to: Option<std::ffi::OsString>,
    pub to_trash: bool,
    /// Temporary files of copies removed.
    pub cleaned: usize,
    /// Rename many's files put back under their names.
    pub restored: usize,
    /// Rename many's files that could not be put back (their name is taken): each
    /// (temporary name, name it had).
    pub stranded: Vec<(PathBuf, PathBuf)>,
    /// A move across devices killed between a file's complete copy and the removal of
    /// its source: (source, copy), both still there. Nothing is removed: Resume leaves
    /// such a source where it is and says so.
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// The confirmed plan's decisions (Round 030): each destination left or replaced.
    pub decided: Vec<(PathBuf, crate::ops::Expect)>,
    /// Replacements finished at the start: killed after the old file went to the trash
    /// and before the new one was renamed in (Round 031). Each destination.
    pub swapped: Vec<PathBuf>,
    /// Sources the job finished with.
    pub done: Vec<PathBuf>,
    /// Big files cut short after a checkpoint, kept for Resume (Round 033).
    pub partial: Vec<Partial>,
    /// The dead run's journal, kept while the job is [offered](Self::offered) or
    /// `partial` is not empty: a later start finds it again if this one neither
    /// resumes nor discards it.
    pub journal: Option<PathBuf>,
}

impl Interrupted {
    /// Whether the start offers this job again: a copy or a move, or a delete to the
    /// trash, with sources. A permanent delete is not: it would go on with one Enter,
    /// where Shift+F8 itself takes only Y.
    pub fn offered(&self) -> bool {
        let kind = match self.kind.as_str() {
            "copy" | "move" => true,
            "delete" => self.to_trash,
            _ => false,
        };
        kind && !self.sources.is_empty()
    }
}

/// Declined: the kept temporary files of `it` and its journal go.
pub fn discard(it: &Interrupted) -> Forgotten {
    let mut f = Forgotten::default();
    if crate::policy::guard_write().is_err() {
        f.left = it.partial.iter().map(|p| p.part.clone()).collect();
        return f;
    }
    for p in &it.partial {
        if !ours(&p.part) {
            continue;
        }
        match fs::remove_file(&p.part) {
            Ok(()) => f.removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => f.removed += 1,
            Err(_) => f.left.push(p.part.clone()),
        }
    }
    // A part still there keeps its journal: the next start offers the job again, and
    // F can be tried again (repo review RR06).
    if f.left.is_empty() {
        if let Some(j) = &it.journal {
            let_go(j);
            let _ = fs::remove_file(j);
        }
    }
    f
}

/// What [`discard`] did: kept parts removed (or already gone), and those still there.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Forgotten {
    pub removed: usize,
    pub left: Vec<PathBuf>,
}

/// A resumed job has its own journal, with the kept files in it: the old one goes.
pub(crate) fn forget(journal: &Path) {
    let_go(journal);
    let _ = fs::remove_file(journal);
}

/// Only xdevcmd's own temporary names, whatever a damaged journal says.
fn ours(p: &Path) -> bool {
    p.file_name()
        .is_some_and(|n| n.to_string_lossy().contains(".xdevcmd-part-"))
}

/// Tidy up after runs that ended without warning, and say what they were doing.
pub fn recover() -> Vec<Interrupted> {
    DIR.get().map(|d| recover_in(d)).unwrap_or_default()
}

fn recover_in(dir: &Path) -> Vec<Interrupted> {
    let mut out: Vec<(std::time::SystemTime, Interrupted)> = Vec::new();
    // The engine's own wall, behind the UI's: tidying removes and renames files.
    // Read-only leaves every journal as it is, for a run that may write.
    if crate::policy::guard_write().is_err() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let host = hostname();
    for e in entries.flatten() {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // A journal still being made: `.<run>-<n>.journal.new`. Empty; removed once
        // its run is gone, left while it lives.
        if let Some(run) = name
            .strip_prefix('.')
            .and_then(|n| n.strip_suffix(".journal.new"))
            .and_then(|n| n.rsplit_once('-'))
            .map(|(run, _)| run)
        {
            if run != run_tag() {
                if let Some(_held) = claim(&path, run, None) {
                    let _ = fs::remove_file(&path);
                }
            }
            continue;
        }
        let Some(run) = name
            .strip_suffix(".journal")
            .filter(|_| !name.starts_with('.'))
            .and_then(|n| n.rsplit_once('-'))
            .map(|(run, _)| run.to_string())
        else {
            continue;
        };
        if run == run_tag() {
            continue;
        }
        // When the run last wrote it: the newest interrupted job is offered first.
        let written = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let mut text = String::new();
        // Held until the journal is gone: another start at the same moment finds it
        // locked (alive, to it), or removed, and never offers the same job again.
        let Some(held) = claim(&path, &run, Some((&mut text, &host))) else {
            continue;
        };
        // A run confined to a root (`--root`) touches nothing outside it: a journal
        // naming a place outside is left whole for a run that may go there.
        if !may_tidy(&text, crate::policy::get()) {
            continue;
        }
        let mut it = tidy(&text, &run);
        #[cfg(test)]
        while_held_in_test();
        // Kept while the start may offer the job again, so Esc leaves it for the next
        // start (#3); resuming or declining removes it. Tidying again is harmless:
        // each step checks the files are still as the line says.
        if it.partial.is_empty() && !it.offered() {
            let _ = fs::remove_file(&path);
        } else {
            it.journal = Some(path.clone());
            if let Ok(mut h) = HELD.lock() {
                h.push((path.clone(), held));
            }
        }
        out.push((written, it));
    }
    // Newest first (the owner, 2026-11-17): the job just interrupted is the one the
    // user remembers, and one left with Esc must not stay in front of it for good.
    out.sort_by_key(|o| std::cmp::Reverse(o.0));
    out.into_iter().map(|(_, it)| it).collect()
}

/// The journal at `path`, if the run that wrote it is gone, told by its lock: returned
/// open, its lock held (Unix), so that no other start takes it while this one tidies up
/// after it. Hold it until the journal is removed. `read`: read the journal into the
/// string, and count it gone only if it was written on this host (named in the second);
/// a journal with no host line is an older one of this machine.
///
/// A journal named by a bare pid (`<pid>-<n>.journal`, before the run tag: 0.2.x) was
/// written by a version that took no lock, so the lock says nothing about it and its pid
/// is asked as well: an older xdevcmd still running through an upgrade keeps its part
/// files. Windows: that version kept its journal open, which the probe sees.
fn claim(path: &Path, run: &str, read: Option<(&mut String, &str)>) -> Option<File> {
    let mut f = open_probe(path)?;
    let lock = probe_lock(&f);
    if lock == Lock::Busy {
        return None;
    }
    // Locked just as its job ended and removed it: not a dead run's.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if f.metadata().map(|m| m.nlink() == 0).unwrap_or(true) {
            return None;
        }
    }
    if let Some((text, host)) = read {
        if f.read_to_string(text).is_err() {
            return None;
        }
        let written_on = text
            .lines()
            .find_map(|l| l.strip_prefix("host "))
            .and_then(dec)
            .map(|p| p.to_string_lossy().into_owned());
        if written_on.is_some_and(|h| h != host) {
            return None;
        }
    }
    let legacy = cfg!(unix) && !run.contains('x');
    let dead = match lock {
        Lock::Held => !legacy || !pid_alive(run),
        Lock::Busy => false,
        Lock::Unsupported => !pid_alive(run),
    };
    dead.then_some(f)
}

#[cfg(test)]
thread_local! {
    /// Run between tidying up after a journal and removing it: a second start, in tests.
    static WHILE_HELD: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
type PathHook = Option<Box<dyn FnOnce(&Path)>>;

#[cfg(test)]
thread_local! {
    /// Run once a journal is made and before it is locked: another start, in tests.
    static BEFORE_LOCK: std::cell::RefCell<PathHook> = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn before_lock_in_test(p: &Path) {
    if let Some(f) = BEFORE_LOCK.with(|h| h.borrow_mut().take()) {
        f(p);
    }
}

#[cfg(test)]
fn while_held_in_test() {
    if let Some(f) = WHILE_HELD.with(|h| h.borrow_mut().take()) {
        f();
    }
}

/// No locks on this file system, or a journal from before locks: whether the run's
/// process still exists, which a reused pid can make look alive (cautious) but never
/// dead. Unix only; elsewhere the open is the lock.
fn pid_alive(run: &str) -> bool {
    let Some(pid) = run.split('x').next().and_then(|p| p.parse::<u32>().ok()) else {
        return true;
    };
    #[cfg(unix)]
    {
        rustix::process::Pid::from_raw(pid as i32)
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Undo what one journal, left by run `run`, says was left half done.
/// Whether every place tidying would touch — the directories searched for part files,
/// the part files, both names of each rename — is inside `policy`'s root. A file that is
/// gone is judged by its directory.
fn may_tidy(text: &str, policy: &crate::policy::Policy) -> bool {
    if policy.root.is_none() {
        return true;
    }
    // A path that is gone may lean on its parent; one that exists answers for
    // itself, so a link in the root that leads out of it is outside (RR03).
    let inside = |p: &Path| policy.allows_at(p);
    // Every path a line names, not only the ones tidy touches: a journal that
    // reaches outside is left whole and not offered (RR02).
    text.lines().all(|line| {
        let mut w = line.split(' ');
        let paths = match w.next() {
            Some(
                "src" | "dest" | "dir" | "part" | "gone" | "unren" | "skip" | "replace" | "done",
            ) => 1,
            Some("ren" | "moved" | "swap") => 2,
            Some("ckpt") => 3,
            _ => 0,
        };
        w.take(paths).all(|t| dec(t).is_some_and(|p| inside(&p)))
    })
}

fn tidy(text: &str, run: &str) -> Interrupted {
    let mut it = Interrupted {
        kind: String::new(),
        sources: Vec::new(),
        dest: None,
        rename_to: None,
        to_trash: false,
        cleaned: 0,
        restored: 0,
        stranded: Vec::new(),
        moved: Vec::new(),
        decided: Vec::new(),
        done: Vec::new(),
        swapped: Vec::new(),
        partial: Vec::new(),
        journal: None,
    };
    let mut bad_name = false;
    let mut ckpts: Vec<Partial> = Vec::new();
    let mut swaps: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut parts: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut renames: Vec<(PathBuf, PathBuf)> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("ckpt ") {
            let w: Vec<&str> = rest.split(' ').collect();
            if let [part, src, dst, id, bytes] = w[..] {
                if let (Some(part), Some(src), Some(dst), Some(ident), Ok(bytes)) = (
                    dec(part),
                    dec(src),
                    dec(dst),
                    crate::ops::Ident::decode(id),
                    bytes.parse(),
                ) {
                    // The last word on a file wins.
                    ckpts.retain(|c| c.part != part);
                    ckpts.push(Partial {
                        part,
                        src,
                        dst,
                        ident,
                        bytes,
                    });
                }
            }
            continue;
        }
        let mut w = line.split(' ');
        let tag = w.next().unwrap_or_default();
        let a = w.next().and_then(dec);
        let b = w.next().and_then(dec);
        match (tag, a, b) {
            // Not `line[5..]`: a journal cut short after "kind" panicked every start.
            ("kind", _, _) => it.kind = line.strip_prefix("kind ").unwrap_or("").to_string(),
            ("src", Some(p), _) => it.sources.push(p),
            ("dest", Some(p), _) => it.dest = Some(p),
            ("as", None, _) => bad_name = true,
            // One plain name, or the job is not trusted (RR04).
            ("as", Some(p), _) => {
                let mut c = p.components();
                match (c.next(), c.next()) {
                    (Some(std::path::Component::Normal(n)), None) => {
                        it.rename_to = Some(n.to_os_string())
                    }
                    _ => bad_name = true,
                }
            }
            ("trash", _, _) => it.to_trash = true,
            ("dir", Some(d), _) => dirs.push(d),
            ("part", Some(p), _) => parts.push(p),
            ("gone", Some(p), _) => parts.retain(|x| *x != p),
            ("ren", Some(t), Some(o)) => renames.push((t, o)),
            ("unren", Some(t), _) => renames.retain(|(x, _)| *x != t),
            ("moved", Some(s), Some(d)) => it.moved.push((s, d)),
            ("skip", Some(p), _) => it.decided.push((p, crate::ops::Expect::Skip)),
            ("replace", Some(p), _) => {
                // The third word is the file's identity, not a path.
                if let Some(id) = line.split(' ').nth(2).and_then(crate::ops::Ident::decode) {
                    it.decided.push((p, crate::ops::Expect::Replace(id)));
                }
            }
            ("done", Some(p), _) => it.done.push(p),
            ("swap", Some(t), Some(d)) => swaps.push((t, d)),
            _ => {}
        }
    }
    // A Copy As whose name is not one plain name: its leftovers are tidied as any,
    // but it is not offered as a copy to run again (RR04).
    if bad_name {
        it.kind.clear();
        it.rename_to = None;
    }
    // Before the temporary files go: one whose destination is empty is the complete,
    // synced replacement of a file now in the trash. Renamed in, never over anything.
    for (tmp, dst) in swaps {
        let ours = tmp
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains(".xdevcmd-part-"));
        if ours
            && fs::symlink_metadata(&dst).is_err()
            && fs::symlink_metadata(&tmp).is_ok_and(|m| m.is_file())
            && crate::noreplace::rename_noreplace(&tmp, &dst).is_ok()
        {
            it.swapped.push(dst);
        }
    }
    // Checkpointed files are kept for Resume, while there is a job to resume and the
    // file holds at least what its line says. Anything else goes with the rest.
    let resumable = matches!(it.kind.as_str(), "copy" | "move") && !it.sources.is_empty();
    for c in ckpts {
        let holds = fs::symlink_metadata(&c.part).is_ok_and(|m| m.is_file() && m.len() >= c.bytes);
        if !ours(&c.part) || !holds {
            continue;
        }
        if resumable && c.bytes > 0 {
            it.partial.push(c);
        } else {
            parts.push(c.part);
        }
    }
    // The temporary files that run made: `.name.xdevcmd-part-<run>-<n>`. The whole
    // run, with the dash after it: a pid alone would match another run given it later.
    let mark = format!(".xdevcmd-part-{run}-");
    for d in dirs {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().contains(&mark) {
                parts.push(e.path());
            }
        }
    }
    for p in parts {
        if it.partial.iter().any(|k| k.part == p) {
            continue;
        }
        if ours(&p)
            && fs::symlink_metadata(&p).is_ok_and(|m| m.is_file())
            && fs::remove_file(&p).is_ok()
        {
            it.cleaned += 1;
        }
    }
    // What was copied and not yet removed: told, never acted on.
    it.moved
        .retain(|(s, d)| fs::symlink_metadata(s).is_ok() && fs::symlink_metadata(d).is_ok());
    for (tmp, orig) in renames {
        if fs::symlink_metadata(&tmp).is_err() {
            continue;
        }
        match crate::noreplace::rename_noreplace(&tmp, &orig) {
            Ok(()) => it.restored += 1,
            Err(_) => it.stranded.push((tmp, orig)),
        }
    }
    it
}

/// A path as hex of its bytes (UTF-16 units on Windows): any name, no escaping rules.
fn enc(p: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        p.as_os_str()
            .encode_wide()
            .map(|u| format!("{u:04x}"))
            .collect()
    }
    #[cfg(not(any(unix, windows)))]
    {
        p.to_string_lossy()
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

fn dec(s: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bytes: Option<Vec<u8>> = (0..s.len())
            .step_by(2)
            .map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .collect();
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes?)))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let units: Option<Vec<u16>> = (0..s.len())
            .step_by(4)
            .map(|i| {
                s.get(i..i + 4)
                    .and_then(|h| u16::from_str_radix(h, 16).ok())
            })
            .collect();
        Some(PathBuf::from(std::ffi::OsString::from_wide(&units?)))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let bytes: Option<Vec<u8>> = (0..s.len())
            .step_by(2)
            .map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .collect();
        String::from_utf8(bytes?).ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-journal-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A journal as a dead process would have left it: written under another pid.
    fn orphan(jdir: &Path, text: &str) {
        // A pid no live process has: the largest a 32-bit pid_t allows.
        fs::write(jdir.join("2147483646-0.journal"), text).unwrap();
    }

    #[test]
    fn a_journal_cut_short_anywhere_or_full_of_garbage_never_panics() {
        let d = dir("prefix");
        let part = d.join(".a.txt.xdevcmd-part-2147483646-0");
        let text = format!(
            "kind copy\nsrc {}\ndest {}\ndir {}\npart {}\ngone {}\nren {} {}\nunren {}\ntrash\n",
            enc(&d.join("a.txt")),
            enc(&d),
            enc(&d),
            enc(&part),
            enc(&part),
            enc(&d.join(".xdevcmd-rename-2147483646-0")),
            enc(&d.join("b.txt")),
            enc(&d.join(".xdevcmd-rename-2147483646-0")),
        );
        // A power cut can stop the file at any byte; the text is ASCII, so every
        // prefix is a str.
        for end in 0..=text.len() {
            tidy(&text[..end], "2147483646");
        }
        for junk in [
            "kind",
            "kind ",
            "src",
            "src zz",
            "src 4",
            "dir",
            "ren 41",
            "ren 41 zz",
            "unren",
            "part",
            "gone",
            "\u{0}",
            " ",
            "  kind copy",
            "trash extra words",
        ] {
            tidy(junk, "2147483646");
            tidy(&format!("{junk}\n{junk}\n"), "2147483646");
        }
        assert_eq!(tidy("kind", "1").kind, "");
        let _ = fs::remove_dir_all(&d);
    }

    /// A run tag no live process has: the largest pid a 32-bit pid_t allows.
    const DEAD: &str = "2147483646x1";

    /// Journals hold full paths: the user's alone, file and directory, even where the
    /// directory was left open by an older run.
    #[cfg(unix)]
    #[test]
    fn a_journal_and_its_directory_are_the_users_alone() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("private");
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&j.path), 0o600, "the journal");
        assert_eq!(mode(&d), 0o700, "its directory");
        drop(j);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_journal_held_by_a_running_job_is_left_alone_whatever_pid_it_names() {
        let d = dir("held");
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false).unwrap();
        // The same lock under a dead pid's name: a pid check would recover it.
        let other = d.join(format!("{DEAD}-0.journal"));
        fs::rename(&j.path, &other).unwrap();
        assert!(recover_in(&d).is_empty(), "held: alive");
        assert!(other.exists());
        drop(j);
        // Unheld now: a dead run's.
        let got = recover_in(&d);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "copy");
        // Kept while the copy is offered again (#3); declining removes it.
        let _ = discard(&got[0]);
        assert!(!other.exists());
        let _ = fs::remove_dir_all(&d);
    }

    /// The owner, 2026-11-17: with two jobs interrupted, the start offered the one read
    /// first from the folder (the oldest, often), and Esc kept it in front, so the job
    /// just interrupted was never offered. Newest first.
    #[test]
    fn interrupted_jobs_come_newest_first() {
        // (seconds of the journal named -0, of the one named -1) → the first offered.
        for (tag, t0, t1, first) in [("newest-a", 1, 2, "src1"), ("newest-b", 2, 1, "src0")] {
            let d = dir(tag);
            for (n, secs) in [(0, t0), (1, t1)] {
                let p = d.join(format!("{DEAD}-{n}.journal"));
                let text = format!(
                    "host {}\nkind copy\nsrc {}\n",
                    enc(Path::new(&hostname())),
                    enc(&d.join(format!("src{n}")))
                );
                fs::write(&p, text).unwrap();
                let f = fs::File::options().write(true).open(&p).unwrap();
                let at = std::time::Duration::from_secs(1_700_000_000 + secs * 1000);
                f.set_modified(std::time::UNIX_EPOCH + at).unwrap();
            }
            let got = recover_in(&d);
            assert_eq!(got.len(), 2, "{tag}");
            assert_eq!(got[0].sources[0], d.join(first), "{tag}: newest first");
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn a_journal_from_another_host_is_left_alone() {
        let d = dir("host");
        let text = format!(
            "host {}\nkind copy\nsrc {}\n",
            enc(Path::new("elsewhere-не-здесь")),
            enc(&d.join("a"))
        );
        let other = d.join(format!("{DEAD}-0.journal"));
        fs::write(&other, text).unwrap();
        assert!(recover_in(&d).is_empty(), "its job may be running there");
        assert!(other.exists());
        // The same, written here: recovered.
        let text = format!(
            "host {}\nkind copy\nsrc {}\n",
            enc(Path::new(&hostname())),
            enc(&d.join("a"))
        );
        fs::write(&other, text).unwrap();
        assert_eq!(recover_in(&d).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_later_run_given_the_same_pid_keeps_its_part_files() {
        let d = dir("same-pid");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let dead = d.join(format!(".a.xdevcmd-part-{DEAD}-0"));
        let live = d.join(".b.xdevcmd-part-2147483646x2-0");
        fs::write(&dead, "x").unwrap();
        fs::write(&live, "x").unwrap();
        fs::write(
            jdir.join(format!("{DEAD}-0.journal")),
            format!("kind copy\ndir {}\n", enc(&d)),
        )
        .unwrap();
        let got = recover_in(&jdir);
        assert_eq!(got[0].cleaned, 1);
        assert!(!dead.exists(), "the dead run's part file is removed");
        assert!(live.exists(), "same pid, another run: not touched");
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_journal_still_being_made_is_removed_only_once_its_run_is_gone() {
        let d = dir("new");
        let unheld = d.join(format!(".{DEAD}-0.journal.new"));
        let held = d.join(format!(".{DEAD}-1.journal.new"));
        fs::write(&unheld, "").unwrap();
        let f = open_held(&held).unwrap();
        assert_eq!(try_lock(&f), Lock::Held);
        assert!(recover_in(&d).is_empty());
        assert!(!unheld.exists(), "left by a run that died making it");
        assert!(held.exists(), "its run is making it");
        drop(f);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn legacy_journal_of_a_live_pid_is_left_alone() {
        // Written by 0.2.x, still running through an upgrade: named by a bare pid (ours
        // stands for a live one), no host line, no lock.
        let d = dir("legacy");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let pid = std::process::id();
        let part = d.join(format!(".a.xdevcmd-part-{pid}-0"));
        fs::write(&part, "half").unwrap();
        let journal = jdir.join(format!("{pid}-0.journal"));
        fs::write(&journal, format!("kind copy\ndir {}\n", enc(&d))).unwrap();
        assert!(recover_in(&jdir).is_empty(), "its run is alive");
        assert!(part.exists(), "a live run's part file is never removed");
        assert!(journal.exists());
        // The same from a pid no process has: recovered.
        fs::rename(&journal, jdir.join("2147483646-0.journal")).unwrap();
        let dead = d.join(".b.xdevcmd-part-2147483646-0");
        fs::write(&dead, "half").unwrap();
        assert_eq!(recover_in(&jdir).len(), 1);
        assert!(!dead.exists());
        assert!(part.exists(), "not the dead run's");
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_journal_another_start_holds_as_it_is_made_is_given_up() {
        let d = dir("taken");
        let holder = std::rc::Rc::new(std::cell::RefCell::new(None));
        let keep = holder.clone();
        // Another start probes the new journal before it is locked, and holds it.
        BEFORE_LOCK.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |p: &Path| {
                let f = open_probe(p).unwrap();
                assert_eq!(try_lock(&f), Lock::Held);
                *keep.borrow_mut() = Some(f);
            }))
        });
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false);
        assert!(
            j.is_err(),
            "a journal not locked by its job would look dead"
        );
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0, "nothing left behind");
        drop(holder);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn two_starts_at_once_offer_an_interrupted_job_once() {
        let d = dir("twice");
        orphan(&d, "kind copy\n");
        let second = std::rc::Rc::new(std::cell::Cell::new(None));
        let seen = second.clone();
        let jdir = d.clone();
        // Another start, while this one is between tidying up and removing the journal.
        WHILE_HELD.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || seen.set(Some(recover_in(&jdir).len()))))
        });
        assert_eq!(recover_in(&d).len(), 1, "the first start offers it");
        assert_eq!(
            second.get(),
            Some(0),
            "the second, at the same moment, does not"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn paths_survive_the_journal_whatever_they_hold() {
        for p in ["/a b/c\nd", "/tài liệu/ảnh.png", "C:\\x"] {
            assert_eq!(dec(&enc(Path::new(p))), Some(PathBuf::from(p)));
        }
    }

    #[test]
    fn a_finished_job_leaves_no_journal_and_a_live_one_is_left_alone() {
        let d = dir("live");
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false).unwrap();
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        // Ours, and running: not recovered.
        assert!(recover_in(&d).is_empty());
        drop(j);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_journal_reaching_outside_the_root_is_not_tidied() {
        let d = dir("root");
        let inside = d.join("in");
        let outside = d.join("out");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let root = crate::policy::Policy {
            read_only: false,
            root: Some(crate::paths::canonical(&inside).unwrap()),
        };
        let journal = |at: &Path| {
            format!(
                "kind copy\ndir {}\npart {}\nren {} {}\n",
                enc(at),
                enc(&at.join(".a.xdevcmd-part-9-0")),
                enc(&at.join(".xdevcmd-rename-9-0")),
                enc(&at.join("a")),
            )
        };
        assert!(
            may_tidy(&journal(&inside), &root),
            "all inside, gone files too"
        );
        assert!(!may_tidy(&journal(&outside), &root));
        // One rename whose old name is outside is enough.
        let mixed = format!(
            "kind copy\nren {} {}\n",
            enc(&inside.join(".xdevcmd-rename-9-0")),
            enc(&outside.join("a"))
        );
        assert!(!may_tidy(&mixed, &root));
        assert!(may_tidy(
            &journal(&outside),
            &crate::policy::Policy::default()
        ));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn every_path_a_journal_names_is_held_to_the_root() {
        // Repo review RR02: only dir/part/ren/swap were checked, so a journal whose
        // other lines name a place outside the root was still offered and tidied.
        let d = dir("rootall");
        let inside = d.join("in");
        let outside = d.join("out");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let root = crate::policy::Policy {
            read_only: false,
            root: Some(crate::paths::canonical(&inside).unwrap()),
        };
        let (i, o) = (inside.join("a"), outside.join("a"));
        let one = ["src", "dest", "gone", "unren", "skip", "done"];
        for kind in one {
            let line = |p: &Path| format!("kind copy\n{kind} {}\n", enc(p));
            assert!(may_tidy(&line(&i), &root), "{kind} inside");
            assert!(!may_tidy(&line(&o), &root), "{kind} outside");
        }
        let replace = format!("kind copy\nreplace {} 7\n", enc(&o));
        assert!(!may_tidy(&replace, &root), "replace outside");
        let moved = format!("kind move\nmoved {} {}\n", enc(&i), enc(&o));
        assert!(!may_tidy(&moved, &root), "moved to outside");
        for at in 0..3 {
            let mut ps = [i.clone(), i.clone(), i.clone()];
            ps[at] = o.clone();
            let ckpt = format!(
                "kind copy\nckpt {} {} {} 7 4\n",
                enc(&ps[0]),
                enc(&ps[1]),
                enc(&ps[2])
            );
            assert!(!may_tidy(&ckpt, &root), "ckpt path {at} outside");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_root_to_outside_is_not_inside_by_its_parent() {
        // Repo review RR03: a path that exists but resolves outside passed because
        // its parent is inside. Only a path that is gone may lean on its parent.
        let d = dir("rootlink");
        let inside = d.join("in");
        let outside = d.join("out");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = inside.join("door");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let root = crate::policy::Policy {
            read_only: false,
            root: Some(crate::paths::canonical(&inside).unwrap()),
        };
        let text = format!("kind copy\ndir {}\n", enc(&link));
        assert!(!may_tidy(&text, &root), "a dir that leads outside");
        let gone = format!(
            "kind copy\npart {}\n",
            enc(&inside.join(".a.xdevcmd-part-9-0"))
        );
        assert!(may_tidy(&gone, &root), "a gone file inside still may");
        // Pinned from the red-team: a gone file under the escaping link, and a dangling
        // link (it exists, so it answers for itself, and it does not resolve).
        let under = format!(
            "kind copy\npart {}\n",
            enc(&link.join(".a.xdevcmd-part-9-0"))
        );
        assert!(!may_tidy(&under, &root), "a gone file under the link");
        let dangling = inside.join("loose");
        std::os::unix::fs::symlink(outside.join("nothing"), &dangling).unwrap();
        let text = format!("kind copy\ndest {}\n", enc(&dangling));
        assert!(!may_tidy(&text, &root), "a dangling link");
        let _ = fs::remove_dir_all(&d);
    }

    /// F says what it removed, and a kept part it could not remove keeps the journal,
    /// so the next start offers the job again (repo review RR06: F said done whatever
    /// happened, and counted the parts it was given).
    #[test]
    fn forgetting_reports_what_was_removed_and_keeps_the_journal_when_a_part_stays() {
        let d = dir("forget");
        let src = d.join("src");
        fs::write(&src, "abc").unwrap();
        let gone_ok = d.join(".a.xdevcmd-part-9-0");
        fs::write(&gone_ok, "ab").unwrap();
        // A part that cannot be removed as a file (a folder by that name, not empty).
        let stuck = d.join(".b.xdevcmd-part-9-1");
        fs::create_dir_all(stuck.join("x")).unwrap();
        let journal = d.join("dead.journal");
        fs::write(&journal, "kind copy\n").unwrap();
        let part = |p: &Path| Partial {
            part: p.to_path_buf(),
            src: src.clone(),
            dst: d.join("a"),
            ident: crate::ops::Ident::at(&src).unwrap(),
            bytes: 2,
        };
        let mut it = tidy("kind copy\n", "x");
        it.partial = vec![part(&gone_ok), part(&stuck)];
        it.journal = Some(journal.clone());
        let f = discard(&it);
        assert_eq!(f.removed, 1);
        assert_eq!(f.left, vec![stuck.clone()]);
        assert!(!gone_ok.exists());
        assert!(journal.exists(), "kept: the next start offers it again");
        // All gone (one already was): the journal goes too.
        fs::remove_dir_all(&stuck).unwrap();
        let f = discard(&it);
        assert_eq!((f.removed, f.left.len()), (2, 0));
        assert!(!journal.exists());
        let _ = fs::remove_dir_all(&d);
    }

    /// Copy As keeps the name it was given across a kill: the journal says it, and the
    /// job found again asks for it, not the source's name (repo review RR04). A name
    /// that is not one plain name is not trusted: the job is not resumed as a copy.
    #[test]
    fn copy_as_keeps_its_new_name_across_a_kill() {
        let d = dir("copy-as");
        let j = Journal::begin_in_as(
            &d,
            "copy",
            &[d.join("a.txt")],
            Some(&d),
            Some(std::ffi::OsStr::new("b c.txt")),
            false,
        )
        .unwrap();
        let text = fs::read_to_string(&j.path).unwrap();
        let it = tidy(&text, "x");
        assert_eq!(
            it.rename_to.as_deref(),
            Some(std::ffi::OsStr::new("b c.txt"))
        );
        let t = format!("{text}as zz-not-hex\n");
        assert!(tidy(&t, "x").kind.is_empty(), "a name that does not decode");
        for bad in ["../b", "x/b", ".."] {
            let t = format!("{text}as {}\n", enc(Path::new(bad)));
            let it = tidy(&t, "x");
            assert!(
                it.rename_to.is_none() && it.kind.is_empty(),
                "{bad}: {it:?}"
            );
        }
        drop(j);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_dead_runs_parts_are_removed_and_renames_put_back() {
        let d = dir("dead");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let part = d.join(".big.iso.xdevcmd-part-99-0");
        let done = d.join(".ok.txt.xdevcmd-part-99-1");
        // Found by its directory and the dead pid in its name, with no line of its own;
        // another live run's part file in the same directory is left alone.
        let by_dir = d.join(".c.bin.xdevcmd-part-2147483646-5");
        let other_run = d.join(".c.bin.xdevcmd-part-1-5");
        fs::write(&by_dir, "x").unwrap();
        fs::write(&other_run, "x").unwrap();
        let user = d.join("not-ours.txt");
        fs::write(&part, "half").unwrap();
        fs::write(&user, "keep").unwrap();
        // Rename many had moved photo.jpg to its temporary name when it died.
        let tmp = d.join(".xdevcmd-rename-99-0");
        fs::write(&tmp, "photo").unwrap();
        let text = format!(
            "kind copy\nsrc {}\ndest {}\ndir {}\npart {}\npart {}\ngone {}\npart {}\nren {} {}\n",
            enc(&d.join("src/big.iso")),
            enc(&d),
            enc(&d),
            enc(&part),
            enc(&done),
            enc(&done),
            enc(&user),
            enc(&tmp),
            enc(&d.join("photo.jpg")),
        );
        orphan(&jdir, &text);
        let got = recover_in(&jdir);
        assert_eq!(got.len(), 1);
        let it = &got[0];
        assert_eq!(it.kind, "copy");
        assert_eq!(it.sources, vec![d.join("src/big.iso")]);
        assert_eq!(it.dest.as_deref(), Some(d.as_path()));
        assert_eq!((it.cleaned, it.restored), (2, 1));
        assert!(!part.exists(), "the part file of the dead run is gone");
        assert!(!by_dir.exists(), "found by its directory");
        assert!(other_run.exists(), "another run's part file is not touched");
        assert!(
            user.exists(),
            "a name that is not xdevcmd's own is never removed"
        );
        assert_eq!(fs::read_to_string(d.join("photo.jpg")).unwrap(), "photo");
        // Kept while the copy is offered again (#3); declining removes it.
        assert!(it.journal.is_some());
        let _ = discard(it);
        assert_eq!(
            fs::read_dir(&jdir).unwrap().count(),
            0,
            "the journal is gone once declined"
        );
        let _ = fs::remove_dir_all(&d);
    }

    /// A big file cut short after a checkpoint (Round 033): its temporary file and the
    /// journal stay, for Resume, also at the next start; one shorter than its line says,
    /// or of a job that cannot be resumed, goes as before. Declined, both go.
    #[test]
    fn a_checkpointed_part_is_kept_with_its_journal_until_discarded() {
        let d = dir("ckpt");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let src = d.join("big.iso");
        fs::write(&src, vec![7u8; 4096]).unwrap();
        let id = crate::ops::Ident::at(&src).unwrap();
        let kept = d.join(".big.iso.xdevcmd-part-2147483646-0");
        fs::write(&kept, vec![7u8; 3000]).unwrap();
        let short = d.join(".short.iso.xdevcmd-part-2147483646-1");
        fs::write(&short, vec![7u8; 10]).unwrap();
        let plain = d.join(".small.txt.xdevcmd-part-2147483646-2");
        fs::write(&plain, "half").unwrap();
        let line = |part: &Path, bytes: u64| {
            format!(
                "ckpt {} {} {} {} {bytes}\n",
                enc(part),
                enc(&src),
                enc(&d.join("out").join("big.iso")),
                id.encode()
            )
        };
        let text = format!(
            "host {}\nv 3\nkind copy\nsrc {}\ndest {}\ndir {}\n{}{}{}",
            enc(Path::new(&hostname())),
            enc(&src),
            enc(&d.join("out")),
            enc(&d),
            line(&kept, 1024),
            line(&kept, 2048),
            line(&short, 2048),
        );
        orphan(&jdir, &text);
        let got = recover_in(&jdir);
        assert_eq!(got.len(), 1);
        let it = &got[0];
        assert_eq!(it.partial.len(), 1, "{:?}", it.partial);
        assert_eq!(it.partial[0].part, kept);
        assert_eq!(it.partial[0].bytes, 2048, "the last checkpoint counts");
        assert_eq!(it.partial[0].ident, id);
        assert!(kept.exists(), "kept for Resume");
        assert!(!short.exists(), "shorter than its checkpoint: removed");
        assert!(!plain.exists(), "no checkpoint: removed as before");
        let journal = it.journal.clone().expect("kept");
        assert!(journal.exists());
        // Neither resumed nor declined: the next start offers it again.
        next_start(&jdir);
        let again = recover_in(&jdir);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].partial, it.partial);
        let _ = discard(&again[0]);
        assert!(!kept.exists() && !journal.exists(), "declined: both go");
        // A delete keeps nothing: there is no copy to go on with.
        fs::write(&kept, vec![7u8; 3000]).unwrap();
        orphan(&jdir, &text.replace("kind copy", "kind delete"));
        let got = recover_in(&jdir);
        assert!(got[0].partial.is_empty() && got[0].journal.is_none());
        assert!(!kept.exists());
        assert_eq!(fs::read_dir(&jdir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&d);
    }

    /// A journal written by 0.3.0 (no `v` line, no `moved`) is tidied up as before; one
    /// of this version, with lines 0.3.0 does not know, is read by the same rules.
    #[test]
    fn a_journal_of_0_3_0_is_still_tidied_and_new_tags_are_skipped() {
        let d = dir("v030");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let part = d.join(".a.xdevcmd-part-2147483646-0");
        fs::write(&part, "half").unwrap();
        let old = format!(
            "host {}\nkind copy\nsrc {}\ndest {}\ndir {}\n",
            enc(Path::new(&hostname())),
            enc(&d.join("a")),
            enc(&d),
            enc(&d),
        );
        orphan(&jdir, &old);
        let got = recover_in(&jdir);
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].kind.as_str(), got[0].cleaned), ("copy", 1));
        assert!(!part.exists());
        let new = format!(
            "{old}v 2\nmoved {} {}\nzzz unknown\n",
            enc(&d.join("a")),
            enc(&d.join("b"))
        );
        let it = tidy(&new, "2147483646");
        assert_eq!(it.kind, "copy");
        assert!(
            it.moved.is_empty(),
            "neither file is there: nothing to tell"
        );
        assert_eq!(it.sources, vec![d.join("a")]);
        let _ = fs::remove_dir_all(&d);
    }

    /// A journal begun is on disk with its header; a line or a sync that fails breaks it
    /// for good, so a later line that would go through is refused too.
    #[test]
    fn a_journal_that_failed_once_stays_broken() {
        let d = dir("broken");
        let mut j =
            Journal::begin_in(&d, "copy", &[d.join("broken-src")], Some(&d), false).unwrap();
        let text = fs::read_to_string(&j.path).unwrap();
        assert!(text.contains("\nv 3\n"), "{text}");
        let _f = crate::faults::fail(
            crate::faults::Point::JournalSync,
            "broken-part",
            1,
            crate::faults::Fault::Os(5),
        );
        j.part(&d.join("sub/.x.broken-part")).unwrap();
        assert!(j.commit(&d.join("broken-part")).is_err());
        assert!(j.broken().is_some());
        assert!(
            j.part(&d.join("other/.y")).is_err(),
            "a later line is refused"
        );
        assert!(
            j.commit(&d.join("anything")).is_err(),
            "and never synced again"
        );
        let _ = fs::remove_dir_all(&d);
    }

    /// A move killed between a file's synced copy and its source's removal: the next
    /// start says both are there, and removes neither.
    #[test]
    fn a_move_killed_before_removing_a_source_says_both_are_there() {
        let d = dir("moved");
        let (src, dst, gone) = (d.join("a"), d.join("b"), d.join("c"));
        fs::write(&src, "x").unwrap();
        fs::write(&dst, "x").unwrap();
        let text = format!(
            "host {}\nv 2\nkind move\nsrc {}\nmoved {} {}\nmoved {} {}\n",
            enc(Path::new(&hostname())),
            enc(&src),
            enc(&src),
            enc(&dst),
            enc(&gone),
            enc(&dst),
        );
        let it = tidy(&text, "2147483646");
        assert_eq!(
            it.moved,
            vec![(src.clone(), dst.clone())],
            "the source of c is gone"
        );
        assert!(src.exists() && dst.exists(), "neither removed");
        let _ = fs::remove_dir_all(&d);
    }

    /// Killed after the old file went to the trash and before the new one was renamed
    /// in: the next start renames the complete new file in. If the old one is still
    /// there (killed before the trash step), the temporary file just goes.
    #[test]
    fn a_replacement_killed_after_the_trash_is_finished_at_the_next_start() {
        let d = dir("swap");
        let run = "2147483645";
        let tmp1 = d.join(format!(".a.xdevcmd-part-{run}-1"));
        let tmp2 = d.join(format!(".b.xdevcmd-part-{run}-2"));
        fs::write(&tmp1, "new a").unwrap();
        fs::write(&tmp2, "new b").unwrap();
        fs::write(d.join("b"), "old b").unwrap();
        let text = format!(
            "host {}\nv 3\nkind copy\ndir {}\nswap {} {}\nswap {} {}\n",
            enc(Path::new(&hostname())),
            enc(&d),
            enc(&tmp1),
            enc(&d.join("a")),
            enc(&tmp2),
            enc(&d.join("b")),
        );
        let it = tidy(&text, run);
        assert_eq!(it.swapped, vec![d.join("a")]);
        assert_eq!(fs::read_to_string(d.join("a")).unwrap(), "new a");
        assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "old b");
        assert!(!tmp1.exists() && !tmp2.exists());
        let _ = fs::remove_dir_all(&d);
    }

    /// The plan's decisions and the sources finished come back from a dead run's
    /// journal, for Resume; a damaged identity is dropped, not guessed.
    #[test]
    fn decisions_and_finished_sources_come_back_from_a_dead_run() {
        use crate::ops::{Expect, Ident};
        let d = dir("decided");
        fs::write(d.join("there"), "x").unwrap();
        let id = Ident::at(&d.join("there")).unwrap();
        let mut j =
            Journal::begin_in(&d, "copy", &[d.join("a"), d.join("b")], Some(&d), false).unwrap();
        j.decided(&d.join("left"), &Expect::Skip).unwrap();
        j.decided(&d.join("there"), &Expect::Replace(id)).unwrap();
        j.decided(&d.join("new"), &Expect::Absent).unwrap();
        j.done(&d.join("a")).unwrap();
        let text = fs::read_to_string(&j.path).unwrap();
        let it = tidy(
            &format!("{text}replace {} i1:junk\n", enc(&d.join("bad"))),
            "x",
        );
        assert_eq!(it.kind, "copy");
        assert_eq!(
            it.decided,
            vec![
                (d.join("left"), Expect::Skip),
                (d.join("there"), Expect::Replace(id)),
            ]
        );
        assert_eq!(it.done, vec![d.join("a")]);
        drop(j);
        let _ = fs::remove_dir_all(&d);
    }

    /// A job the start offers again (a copy, a move, a delete to the trash) keeps its
    /// journal until it is resumed or declined: Esc leaves it for the next start, as the
    /// dialog says. Without a kept part it was removed at once, so Esc lost it (#3).
    #[test]
    fn a_job_offered_again_keeps_its_journal_until_answered() {
        let d = dir("offered");
        let jdir = d.join("j");
        fs::create_dir_all(&jdir).unwrap();
        let src = enc(&d.join("a"));
        let dst = enc(&d.join("out"));
        for (text, kept) in [
            (format!("kind copy\nsrc {src}\ndest {dst}\n"), true),
            (format!("kind move\nsrc {src}\ndest {dst}\n"), true),
            (format!("kind delete\nsrc {src}\ntrash\n"), true),
            (format!("kind delete\nsrc {src}\n"), false),
            ("kind copy\n".to_string(), false),
        ] {
            orphan(&jdir, &text);
            let got = recover_in(&jdir);
            assert_eq!(got.len(), 1, "{text}");
            assert_eq!(got[0].journal.is_some(), kept, "{text}");
            // While this run has it unanswered, another start does not offer it too
            // (red-team F1); the next start does.
            assert!(recover_in(&jdir).is_empty(), "held: {text}");
            next_start(&jdir);
            let again = recover_in(&jdir);
            assert_eq!(again.len(), usize::from(kept), "offered again: {text}");
            if let Some(it) = again.first() {
                let _ = discard(it);
            }
            assert_eq!(fs::read_dir(&jdir).unwrap().count(), 0, "{text}");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_identity_reads_back_as_itself() {
        use crate::ops::Ident;
        let d = dir("ident");
        fs::write(d.join("f"), "x").unwrap();
        fs::create_dir(d.join("sub")).unwrap();
        for p in [d.join("f"), d.join("sub")] {
            let id = Ident::at(&p).unwrap();
            assert_eq!(Ident::decode(&id.encode()), Some(id), "{}", id.encode());
            assert!(!id.encode().contains(' '));
        }
        for junk in [
            "",
            "i1",
            "i2:0:1:-:1:2",
            "i1:x:1:-:1:2",
            "i1:0:1:5:1:2",
            "i1:0:1:-",
        ] {
            assert_eq!(Ident::decode(junk), None, "{junk}");
        }
        let _ = fs::remove_dir_all(&d);
    }
}
