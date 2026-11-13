//! Copy, move and delete, run on a worker thread.
//!
//! The rules this file exists to keep (see .agents/context/project.md): nothing destructive happens
//! silently, every operation is cancellable part-way through, and a filesystem that
//! misbehaves produces a question rather than a panic or a half-finished tree with no
//! explanation.
//!
//! The worker owns the filesystem work and sends [`Event`]s. When it needs a decision it
//! sends [`Event::Ask`] and *blocks* until an [`Answer`] comes back, so the UI is the only
//! thing that ever decides to overwrite or delete.

use std::fs::{self, File, FileTimes};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const BUF: usize = 256 * 1024;
/// A move across devices removes sources a batch at a time: this many files, or
/// [`MOVE_BATCH_BYTES`] of them, whichever comes first.
const MOVE_BATCH_FILES: usize = 64;
const MOVE_BATCH_BYTES: u64 = 64 << 20;
/// Report at most this often, so a directory of small files does not drown the UI.
const PROGRESS_STEP: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Move,
    Delete,
    /// Out of an archive: `archive`, the entries `inner`, below `base`, into `dest`.
    Extract,
    /// `sources` into a new archive at `dest`, its format from its name.
    Pack,
}

impl Kind {
    pub fn verb(self) -> &'static str {
        match self {
            Kind::Copy => "Copy",
            Kind::Move => "Move",
            Kind::Delete => "Delete",
            Kind::Extract => "Extract",
            Kind::Pack => "Pack",
        }
    }
}

/// What a copy or move does with a file already at the destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Existing {
    /// Ask about each one (the default).
    #[default]
    Ask,
    /// Replace it only when the source is newer; skip it when the two are the same or
    /// the destination is newer. robocopy's and rsync's `--update`.
    Update,
    /// Leave every existing file as it is.
    Skip,
    /// Replace every existing file.
    Replace,
    /// Continuing an interrupted job: skip a file that is already there and identical
    /// (same size, same date within a FAT drive's two seconds), ask about any other.
    /// Not [`Existing::Update`]: a file the user chose to keep in the first run would
    /// be replaced if its source is newer, a decision nobody made.
    Resume,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub kind: Kind,
    pub sources: Vec<PathBuf>,
    /// Destination directory. Required for copy and move, ignored for delete.
    pub dest: Option<PathBuf>,
    /// Delete to the XDG trash instead of unlinking.
    pub to_trash: bool,
    /// Override the trash location. Exists so tests do not touch the real trash.
    pub trash_root: Option<PathBuf>,
    /// Copy or move a single source under this name instead of its own: Shift+F5
    /// copies a file beside itself, which is this with `dest` its own directory.
    pub rename_to: Option<std::ffi::OsString>,
    /// Extract: the archive, the entries chosen in it, and the directory being
    /// browsed inside it ("" at the top), which is taken off each name.
    pub archive: Option<PathBuf>,
    pub inner: Vec<String>,
    pub base: String,
    /// Copy each source into its own directory (synchronising): `(source, into)`.
    /// Directories are created as needed. When set, `sources` and `dest` are unused.
    pub pairs: Vec<(PathBuf, PathBuf)>,
    /// Synchronising: what the confirmed plan expected at each destination (the path a
    /// pair's source is copied to). A destination is replaced without a question only
    /// when it is still the file the plan saw ([`Expect::Replace`]); anything else there
    /// (a file where the plan saw none, one changed since) is asked about.
    pub expect: std::collections::HashMap<PathBuf, Expect>,
    /// Copy and move: what to do with files already at the destination.
    pub existing: Existing,
    /// Files and bytes the confirmed plan already counted (Round 030): the job's
    /// progress bar uses them instead of walking the sources a second time.
    pub planned: Option<(u64, u64)>,
    /// Safe operations for this job, taken from the setting when the request is made
    /// and kept to the end: turning the setting over mid-job must not split one job
    /// between two ways of writing, and tests set it here rather than in a global
    /// other tests read.
    pub safe: bool,
}

impl Request {
    pub fn extract(archive: PathBuf, inner: Vec<String>, base: String, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Extract,
            sources: vec![archive.clone()],
            dest: Some(dest),
            archive: Some(archive),
            inner,
            base,
            ..Self::delete(Vec::new(), false)
        }
    }
    /// Copy each `(source, into, expected)`, replacing what the confirmed plan said it
    /// would, and only while it is what the plan saw.
    pub fn sync(pairs: Vec<(PathBuf, PathBuf, Expect)>) -> Self {
        let expect = pairs
            .iter()
            .filter_map(|(src, into, e)| Some((into.join(src.file_name()?), *e)))
            .collect();
        Self {
            kind: Kind::Copy,
            pairs: pairs.into_iter().map(|(s, i, _)| (s, i)).collect(),
            expect,
            ..Self::delete(Vec::new(), false)
        }
    }
    /// `dest` is the archive file to create.
    pub fn pack(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Pack,
            sources,
            dest: Some(dest),
            ..Self::delete(Vec::new(), false)
        }
    }

    pub fn copy(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Copy,
            sources,
            dest: Some(dest),
            to_trash: false,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
        }
    }
    pub fn mv(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Move,
            sources,
            dest: Some(dest),
            to_trash: false,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
        }
    }
    pub fn delete(targets: Vec<PathBuf>, to_trash: bool) -> Self {
        Self {
            kind: Kind::Delete,
            sources: targets,
            dest: None,
            to_trash,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
        }
    }
}

/// Which file a decision was about: enough to tell, right before the step it allowed,
/// that the file there is still that one. Kind, size and modification time; on Unix the
/// device and inode too, on Windows the creation time (std has no stable file index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    kind: u8,
    len: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    node: (u64, u64),
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}

impl Ident {
    pub fn of(m: &fs::Metadata) -> Self {
        let t = m.file_type();
        Self {
            kind: if t.is_symlink() {
                2
            } else if t.is_dir() {
                1
            } else if t.is_file() {
                0
            } else {
                3
            },
            len: m.len(),
            modified: m.modified().ok(),
            #[cfg(unix)]
            node: {
                use std::os::unix::fs::MetadataExt;
                (m.dev(), m.ino())
            },
            #[cfg(not(unix))]
            created: m.created().ok(),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == 1
    }

    /// What is at `p` now (a link as itself), if anything.
    pub fn at(p: &Path) -> Option<Self> {
        fs::symlink_metadata(p).ok().map(|m| Self::of(&m))
    }

    /// As one word for a journal line (no spaces): read back by [`Ident::decode`] on the
    /// same system. A time before 1970 is written as none, so it never matches again:
    /// the file is then asked about, which is the safe side.
    pub fn encode(&self) -> String {
        let t = |t: Option<std::time::SystemTime>| {
            t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or("-".to_string(), |d| {
                    format!("{}.{}", d.as_secs(), d.subsec_nanos())
                })
        };
        #[cfg(unix)]
        let rest = format!("{}:{}", self.node.0, self.node.1);
        #[cfg(not(unix))]
        let rest = t(self.created);
        format!("i1:{}:{}:{}:{rest}", self.kind, self.len, t(self.modified))
    }

    pub fn decode(s: &str) -> Option<Self> {
        let t = |s: &str| -> Option<Option<std::time::SystemTime>> {
            if s == "-" {
                return Some(None);
            }
            let (a, b) = s.split_once('.')?;
            let d = std::time::Duration::new(a.parse().ok()?, b.parse().ok()?);
            Some(Some(std::time::UNIX_EPOCH + d))
        };
        let mut w = s.split(':');
        if w.next()? != "i1" {
            return None;
        }
        let kind = w.next()?.parse().ok()?;
        let len = w.next()?.parse().ok()?;
        let modified = t(w.next()?)?;
        #[cfg(unix)]
        let id = Self {
            kind,
            len,
            modified,
            node: (w.next()?.parse().ok()?, w.next()?.parse().ok()?),
        };
        #[cfg(not(unix))]
        let id = Self {
            kind,
            len,
            modified,
            created: t(w.next()?)?,
        };
        Some(id)
    }
}

/// What a synchronisation plan saw at a destination when the user confirmed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Nothing: the copy is new there.
    Absent,
    /// This file, which the copy replaces.
    Replace(Ident),
    /// Whatever is there is left as it is (a planned skip: the user's choice for its
    /// group, or a row the plan found cannot be done).
    Skip,
}

/// Something a copy made, for undoing it (Round 031): a file, link or directory that
/// was not there before, as it was right after the copy made it. `old`: the file it
/// replaced, put in the trash on the way; undoing puts it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub path: PathBuf,
    pub ident: Ident,
    pub old: Option<crate::trashed::Trashed>,
}

/// The most a copy remembers for its undo, as the plan's own cap: past it the copy
/// cannot be undone, and says so.
pub const MADE_CAP: usize = crate::plan::CAP;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// The destination already exists. `changed`: it is not the file the user agreed
    /// to replace, or the one the plan saw (replaced, or made, since): asked again.
    Conflict {
        path: PathBuf,
        existing_bytes: u64,
        incoming_bytes: u64,
        changed: bool,
    },
    /// A move would cross a filesystem boundary, so it becomes a copy then a delete.
    CrossDevice { path: PathBuf },
    /// The trash is on another filesystem, so the only way to honour the key is to
    /// delete for real. Never assumed.
    TrashUnavailable { path: PathBuf, reason: String },
    /// Something went wrong and the user decides whether it is fatal.
    Failed { path: PathBuf, error: String },
    /// Safe operations' journal cannot be written (`error`): at the start, or before a
    /// step that cannot be undone. [`Answer::Yes`] goes on without it for the rest of the
    /// job; anything else stops the job there. Asked once per job.
    NoJournal { error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Yes for this one: overwrite, cross the device, delete for real.
    Yes,
    /// Yes, and stop asking.
    YesToAll,
    Skip,
    SkipAll,
    /// From here on: replace a destination only when the source is newer, skip the rest.
    Update,
    Retry,
    Abort,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
    pub bytes: u64,
    pub cancelled: bool,
    pub errors: Vec<String>,
    /// A delete to the trash: where each item went, for undo.
    pub trashed: Vec<crate::trashed::Trashed>,
    /// A continued move: sources left where they were, because an identical copy was
    /// already at the destination (a move across drives killed between the copy and
    /// the removal of its source). Never removed without a question.
    pub left: u64,
    /// Safe operations were on but this job's journal could not be made: why. The job ran
    /// as with safe operations off; a kill now is not tidied up at the next start.
    pub journal_off: Option<String>,
    /// A copy: what it made, in the order made (a directory before what is in it), for
    /// its undo. Empty past [`MADE_CAP`], with `made_over` set.
    pub made: Vec<Made>,
    pub made_over: bool,
    /// A copy: files it replaced with no trash to keep the old one in. Undo leaves those.
    pub replaced_for_good: u64,
    /// Files replaced whose old version went to the trash (Round 031).
    pub replaced_to_trash: u64,
}

impl Summary {
    pub fn describe(&self, kind: Kind) -> String {
        let verb = match kind {
            Kind::Copy => "copied",
            Kind::Move => "moved",
            Kind::Delete => "deleted",
            Kind::Extract => "extracted",
            Kind::Pack => "packed",
        };
        let mut s = format!("{} {verb}", self.done);
        if self.skipped > 0 {
            s.push_str(&format!(", {} skipped", self.skipped));
        }
        if self.failed > 0 {
            s.push_str(&format!(", {} failed", self.failed));
        }
        if self.replaced_to_trash > 0 {
            s.push_str(&format!(
                ", {} replaced (the old {} in the trash)",
                self.replaced_to_trash,
                if self.replaced_to_trash == 1 {
                    "one"
                } else {
                    "ones"
                }
            ));
        }
        if self.left > 0 {
            s.push_str(&format!(
                ", {} left where {} {}: already there",
                self.left,
                if self.left == 1 { "it" } else { "they" },
                if self.left == 1 { "was" } else { "were" },
            ));
        }
        if self.cancelled {
            s.push_str(" (cancelled)");
        }
        if let Some(why) = &self.journal_off {
            s.push_str(&format!("; safe operations off for this job: {why}"));
        }
        s
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Scanned { files: u64, bytes: u64 },
    Begin { path: PathBuf },
    Progress { bytes_done: u64 },
    Ask(Question),
    Done(Summary),
}

pub struct Handle {
    pub events: Receiver<Event>,
    answers: Sender<Answer>,
    cancel: Arc<AtomicBool>,
}

impl Handle {
    /// A handle with no worker behind it, fed by the caller. For tests of code that
    /// holds jobs, which should not need a filesystem to do it.
    #[doc(hidden)]
    pub fn for_test(events: Receiver<Event>) -> Self {
        let (answers, _) = channel();
        Self {
            events,
            answers,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A job that asks nothing, run by `work` on its own thread with the events'
    /// sender and the cancel flag: undoing a copy.
    pub(crate) fn spawn(
        work: impl FnOnce(Sender<Event>, Arc<AtomicBool>) + Send + 'static,
    ) -> Self {
        let (etx, events) = channel();
        let (answers, _) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || work(etx, c));
        Self {
            events,
            answers,
            cancel,
        }
    }

    pub fn answer(&self, a: Answer) {
        let _ = self.answers.send(a);
    }
    /// Ask the worker to stop at the next safe point. Never interrupts a single
    /// `write`, so a partially written file is always closed and reported.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub fn start(req: Request) -> Handle {
    let (etx, erx) = channel::<Event>();
    let (atx, arx) = channel::<Answer>();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();

    std::thread::spawn(move || {
        let mut w = Worker {
            tx: etx,
            answers: arx,
            cancel: worker_cancel,
            overwrite_all: false,
            skip_all: false,
            update: false,
            resuming: false,
            cross_all: false,
            trash_all: false,
            expect: Default::default(),
            agreed: None,
            changed: None,
            abort: false,
            bytes_done: 0,
            last_report: 0,
            sum: Summary::default(),
            moving: false,
            approved: None,
            journal: None,
            removing: Vec::new(),
            removing_bytes: 0,
            moved_file: None,
            safe: req.safe,
            recording: req.kind == Kind::Copy,
            trash_root: req.trash_root.clone(),
            keeps: Default::default(),
            old_trashed: None,
        };
        w.run(req);
    });

    Handle {
        events: erx,
        answers: atx,
        cancel,
    }
}

struct Worker {
    tx: Sender<Event>,
    answers: Receiver<Answer>,
    cancel: Arc<AtomicBool>,
    /// "All" answers, one per kind of question: overwriting all is not also agreeing
    /// to copy-then-delete across devices, or to delete permanently.
    overwrite_all: bool,
    skip_all: bool,
    /// Replace only what is older than its source; skip what is the same or newer.
    update: bool,
    /// [`Existing::Resume`]: skip what is identical, ask about the rest.
    resuming: bool,
    cross_all: bool,
    trash_all: bool,
    /// Synchronising: [`Request::expect`].
    expect: std::collections::HashMap<PathBuf, Expect>,
    /// The file a decision to replace was about: the destination and what was there
    /// when the user was asked (what they saw) or the plan was made. Checked again
    /// right before the step that replaces it ([`Worker::still_agreed`]).
    agreed: Option<(PathBuf, Ident)>,
    /// A destination found changed since it was agreed to: its next question says so.
    changed: Option<PathBuf>,
    abort: bool,
    bytes_done: u64,
    last_report: u64,
    sum: Summary,
    /// Copying for a move across devices: each source is removed as soon as its copy
    /// is complete, so what was skipped or failed stays where it was.
    moving: bool,
    /// A destination the user already agreed to replace, so the copy a cross-device
    /// move falls back to does not ask about it a second time.
    approved: Option<PathBuf>,
    /// A move across devices: copies complete and synced whose sources wait to be
    /// removed, a batch at a time ([`Worker::remove_moved`]).
    /// Each with the handle that wrote the copy, still open, to sync it through: a
    /// handle opened afresh cannot write a read-only copy, and on a network file system
    /// would not be told of a write that failed on the first (at most
    /// [`MOVE_BATCH_FILES`] are held). `None`: already synced (a replacement), or a link.
    removing: Vec<(PathBuf, PathBuf, Option<File>)>,
    /// Their size, in bytes.
    removing_bytes: u64,
    /// The copy [`Worker::copy_bytes`] just made while moving, still open, for
    /// [`Worker::moved`] to keep with it.
    moved_file: Option<File>,
    /// Safe operations: what this job is in the middle of, for the next start if it is
    /// killed. Removed when the job ends.
    journal: Option<crate::journal::Journal>,
    /// [`Request::safe`]: decides the journal and whether a new file is written under
    /// a temporary name.
    safe: bool,
    /// A copy: what it makes is kept in [`Summary::made`], for its undo.
    recording: bool,
    /// [`Request::trash_root`]: tests' trash, for a replaced file too.
    trash_root: Option<PathBuf>,
    /// Per destination directory: whether a file replaced there goes to the trash.
    keeps: std::collections::HashMap<PathBuf, bool>,
    /// The file the last replace put in the trash, for its undo.
    old_trashed: Option<crate::trashed::Trashed>,
}

impl Worker {
    fn run(&mut self, req: Request) {
        // Every kind of job writes. In read-only mode none starts, whatever asked.
        if let Err(e) = crate::policy::guard_write() {
            self.sum.failed = req.sources.len() as u64;
            self.sum.errors.push(e.to_string());
            let _ = self.tx.send(Event::Done(self.sum.clone()));
            return;
        }
        if self.safe {
            let kind = match req.kind {
                Kind::Copy if !req.pairs.is_empty() => "sync",
                Kind::Copy => "copy",
                Kind::Move => "move",
                Kind::Delete => "delete",
                Kind::Extract => "extract",
                Kind::Pack => "pack",
            };
            let sources: Vec<PathBuf> = if req.pairs.is_empty() {
                req.sources.clone()
            } else {
                req.pairs.iter().map(|(s, _)| s.clone()).collect()
            };
            match crate::journal::Journal::begin(kind, &sources, req.dest.as_deref(), req.to_trash)
            {
                Some(Ok(j)) => self.journal = Some(j),
                // Asked before anything is done (the owner's decision, Round 029): go on
                // as with safe operations off, said in the summary, or stop now.
                Some(Err(e)) if !self.go_without_journal(e.to_string()) => {
                    self.finish();
                    return;
                }
                Some(Err(_)) => {}
                None => {}
            }
        }
        if matches!(req.kind, Kind::Extract | Kind::Pack) {
            self.archive_job(&req);
            self.finish();
            return;
        }
        match req.existing {
            Existing::Ask => {}
            Existing::Update => self.update = true,
            Existing::Skip => self.skip_all = true,
            Existing::Replace => self.overwrite_all = true,
            Existing::Resume => self.resuming = true,
        }
        self.expect = req.expect.clone();
        // The plan's decisions, for a resume to keep them (Round 030): written before
        // the first step, synced with the first commit like every line.
        if req.planned.is_some() {
            if let Some(j) = self.journal.as_mut() {
                for (dst, e) in &req.expect {
                    let _ = j.decided(dst, e);
                }
            }
        }
        if !req.pairs.is_empty() {
            let sources: Vec<PathBuf> = req.pairs.iter().map(|(s, _)| s.clone()).collect();
            let (files, bytes) = scan(&sources);
            let _ = self.tx.send(Event::Scanned { files, bytes });
            for (src, into) in &req.pairs {
                if self.stopped() {
                    break;
                }
                let _ = self.tx.send(Event::Begin { path: src.clone() });
                if let Err(e) = fs::create_dir_all(into) {
                    self.note_error(into, &e);
                    continue;
                }
                self.copy_into(src, into, None);
            }
            self.finish();
            return;
        }
        let (files, bytes) = req.planned.unwrap_or_else(|| scan(&req.sources));
        let _ = self.tx.send(Event::Scanned { files, bytes });

        for src in &req.sources {
            if self.stopped() {
                break;
            }
            let _ = self.tx.send(Event::Begin { path: src.clone() });
            match req.kind {
                Kind::Copy => {
                    if let Some(d) = req.dest.as_deref() {
                        self.copy_into(src, d, req.rename_to.as_deref());
                    }
                }
                Kind::Move => {
                    if let Some(d) = req.dest.as_deref() {
                        self.move_into(src, d, req.rename_to.as_deref());
                    }
                }
                Kind::Delete => self.delete(src, req.to_trash, req.trash_root.as_deref()),
                Kind::Extract | Kind::Pack => {}
            }
            // Finished with this source (not stopped part way): a resume leaves it.
            if !self.stopped() && matches!(req.kind, Kind::Copy | Kind::Move) {
                if let Some(j) = self.journal.as_mut() {
                    let _ = j.done(src);
                }
            }
        }

        self.finish();
    }

    fn stopped(&self) -> bool {
        self.abort || self.cancel.load(Ordering::Relaxed)
    }

    /// Whether the file `dst` replaces goes to the trash first (Round 031): where the
    /// trash is on the same volume ([`crate::plan::trash_keeps`]), asked once per
    /// directory. Tests give their own trash, and always use it.
    fn replace_into_trash(&mut self, dst: &Path) -> bool {
        if self.trash_root.is_some() {
            return true;
        }
        // A unit test without its own trash never touches the real one.
        if cfg!(test) {
            return false;
        }
        let dir = dst.parent().unwrap_or(Path::new("")).to_path_buf();
        *self
            .keeps
            .entry(dir)
            .or_insert_with_key(|d| crate::plan::trash_keeps(d) == Some(true))
    }

    /// A copy made `path` (replacing `old`, if it went to the trash): kept for its undo,
    /// as it is now. Past the cap, nothing more is kept and the copy cannot be undone.
    fn made(&mut self, path: &Path, old: Option<crate::trashed::Trashed>) {
        if !self.recording {
            return;
        }
        if self.sum.made.len() >= MADE_CAP {
            self.recording = false;
            self.sum.made_over = true;
            self.sum.made = Vec::new();
            return;
        }
        if let Some(ident) = Ident::at(path) {
            self.sum.made.push(Made {
                path: path.to_path_buf(),
                ident,
                old,
            });
        }
    }

    /// The job is over: say how it went. A journal that broke on a line no step needed
    /// synced (a new file's) is said here, since nobody was asked about it.
    fn finish(&mut self) {
        self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
        self.sum.bytes = self.bytes_done;
        if let Some(why) = self.journal.as_ref().and_then(|j| j.broken()) {
            self.sum.journal_off = Some(format!("its journal failed: {why}"));
        }
        let _ = self.tx.send(Event::Done(self.sum.clone()));
    }

    /// Before a step that cannot be undone, about `about`: the journal's lines are on
    /// disk. A journal that cannot be (now or before) is asked about, once: go on
    /// without it, or stop here (`false`; the job ends).
    fn durable(&mut self, about: &Path) -> bool {
        // Stopped (Stop at the journal's question, Abort): nothing more that cannot be
        // undone, with or without a journal: a batch's sources are kept.
        if self.abort {
            return false;
        }
        let Some(j) = self.journal.as_mut() else {
            return true;
        };
        let Err(e) = j.commit(about) else {
            return true;
        };
        self.go_without_journal(e.to_string())
    }

    /// Ask whether to go on without the journal, which cannot be written (`why`). Its
    /// file goes either way: half a journal is not one.
    fn go_without_journal(&mut self, why: String) -> bool {
        self.journal = None;
        match self.ask(Question::NoJournal { error: why.clone() }) {
            Answer::Yes | Answer::YesToAll => {
                self.sum.journal_off = Some(why);
                true
            }
            _ => {
                self.abort = true;
                false
            }
        }
    }

    /// Extract or pack, through `archive.rs`, with this worker answering its hooks so
    /// the progress bar, the overwrite question and cancel all behave as for a copy.
    fn archive_job(&mut self, req: &Request) {
        let Some(dest) = req.dest.clone() else { return };
        let result = match req.kind {
            Kind::Extract => {
                let Some(archive) = req.archive.clone() else {
                    return;
                };
                match crate::archive::list(&archive) {
                    Ok(l) => {
                        let chosen = crate::archive::expand(&l.items, &req.inner);
                        let files = chosen
                            .iter()
                            .filter(|i| i.kind == crate::archive::ItemKind::File);
                        let (n, bytes) = files.fold((0u64, 0u64), |(n, b), i| (n + 1, b + i.size));
                        let _ = self.tx.send(Event::Scanned { files: n, bytes });
                    }
                    Err(e) => {
                        self.note_error(&archive, &e);
                        return;
                    }
                }
                crate::archive::extract(&archive, &req.inner, &req.base, &dest, self)
            }
            _ => {
                let (files, bytes) = scan(&req.sources);
                let _ = self.tx.send(Event::Scanned { files, bytes });
                // The archive is one new file: an existing one is asked about first.
                match self.resolve_conflict(None, &dest, bytes) {
                    Some(true) => crate::archive::pack(&req.sources, &dest, self),
                    _ => Ok(()),
                }
            }
        };
        if let Err(e) = result {
            self.note_error(&dest, &e);
        } else if req.kind == Kind::Pack && !self.stopped() && dest.exists() {
            self.sum.done = self.sum.done.max(1);
        }
    }

    fn ask(&mut self, q: Question) -> Answer {
        if self.tx.send(Event::Ask(q)).is_err() {
            return Answer::Abort;
        }
        // The UI has gone away if this fails, and continuing to touch the filesystem
        // without anyone able to stop us is the one thing we must not do.
        self.answers.recv().unwrap_or(Answer::Abort)
    }

    fn note_error(&mut self, path: &Path, e: &io::Error) {
        self.sum.failed += 1;
        if self.sum.errors.len() < 20 {
            self.sum.errors.push(format!("{}: {e}", path.display()));
        }
    }

    fn advance(&mut self, n: u64) {
        self.bytes_done += n;
        if self.bytes_done - self.last_report >= PROGRESS_STEP {
            self.last_report = self.bytes_done;
            let _ = self.tx.send(Event::Progress {
                bytes_done: self.bytes_done,
            });
        }
    }

    /// Decide what to do about an existing destination. `Some(true)` means go ahead.
    /// `src`: the file that would replace it, for the update rule; none for an archive.
    fn resolve_conflict(&mut self, src: Option<&Path>, dst: &Path, incoming: u64) -> Option<bool> {
        let Some(now) = Ident::at(dst) else {
            return Some(true);
        };
        if self.approved.as_deref() == Some(dst) {
            // Agreed already, at the move's question: that file is still the one.
            self.approved = None;
            return Some(true);
        }
        self.agreed = None;
        // "Replace all" is a rule, not a file: applied to whatever is there.
        if self.overwrite_all {
            return Some(true);
        }
        // The plan's word holds for the file it saw, and for nothing else. A skip is
        // safe whatever is there now.
        if self.expect.get(dst) == Some(&Expect::Skip) {
            return Some(false);
        }
        match self.expect.get(dst) {
            Some(Expect::Replace(id)) if *id == now && self.changed.as_deref() != Some(dst) => {
                self.agreed = Some((dst.to_path_buf(), now));
                return Some(true);
            }
            Some(_) => self.changed = Some(dst.to_path_buf()),
            None => {}
        }
        if self.skip_all {
            return Some(false);
        }
        if self.update {
            return Some(self.is_older(src, dst));
        }
        if self.resuming {
            match identical(src, dst) {
                Same::Dirs => return Some(true),
                Same::Yes => return Some(false),
                Same::No => {}
            }
        }
        let changed = self.changed.as_deref() == Some(dst);
        self.changed = None;
        // What the user is shown is what they agree to: kept, and compared again
        // before the file is replaced.
        let answer = self.ask(Question::Conflict {
            path: dst.to_path_buf(),
            existing_bytes: now.len,
            incoming_bytes: incoming,
            changed,
        });
        if answer == Answer::Yes {
            self.agreed = Some((dst.to_path_buf(), now));
        }
        match answer {
            Answer::Yes => Some(true),
            Answer::YesToAll => {
                self.overwrite_all = true;
                Some(true)
            }
            Answer::Skip => Some(false),
            Answer::SkipAll => {
                self.skip_all = true;
                Some(false)
            }
            Answer::Update => {
                self.update = true;
                Some(self.is_older(src, dst))
            }
            _ => {
                self.abort = true;
                None
            }
        }
    }

    /// Right before replacing `dst`: whether it is still the file that was agreed to (an
    /// answer, the plan), or still one the update rule replaces. If not, it is marked
    /// changed and `false`: the caller goes back to the question.
    fn still_agreed(&mut self, src: Option<&Path>, dst: &Path) -> bool {
        let now = Ident::at(dst);
        let ok = match &self.agreed {
            // Kept, not taken: a move that falls back to copying checks it again.
            Some((p, id)) if p == dst => now == Some(*id),
            // A rule: re-applied to what is there now.
            _ => self.overwrite_all || !self.update || self.is_older(src, dst),
        };
        if !ok {
            self.changed = Some(dst.to_path_buf());
        }
        ok
    }

    /// The update rule: `dst` is replaced only when `src` is newer (beyond a FAT drive's
    /// two seconds). Two directories merge. Without a source (an archive) it is kept.
    fn is_older(&self, src: Option<&Path>, dst: &Path) -> bool {
        let (Some(s), Ok(d)) = (
            src.and_then(|s| fs::symlink_metadata(s).ok()),
            fs::symlink_metadata(dst),
        ) else {
            return false;
        };
        if s.is_dir() && d.is_dir() {
            return true;
        }
        crate::compare::newer_than(&s, &d) == std::cmp::Ordering::Greater
    }

    // ── copy ───────────────────────────────────────────────────────────────────────

    fn copy_into(&mut self, src: &Path, dest_dir: &Path, rename: Option<&std::ffi::OsStr>) {
        let Some(name) = rename.or(src.file_name()) else {
            return;
        };
        let dst = dest_dir.join(name);
        if dst == src {
            self.sum.skipped += 1;
            return;
        }
        // Copying a directory into itself would recurse until the disk filled.
        if src.is_dir() && dst.starts_with(src) {
            let e = io::Error::other("cannot copy a directory into itself");
            self.note_error(src, &e);
            return;
        }
        self.copy_node(src, &dst);
    }

    fn copy_node(&mut self, src: &Path, dst: &Path) {
        if self.stopped() {
            return;
        }
        let meta = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => {
                self.note_error(src, &e);
                return;
            }
        };

        if let Some(kind) = crate::entry::special_kind(&meta.file_type()) {
            // Copying a FIFO's "contents" blocks on a read nothing will satisfy, and a
            // blocked read cannot be cancelled. A device's contents may never end.
            let e = io::Error::other(format!("{kind} \u{2014} not copied"));
            self.note_error(src, &e);
            return;
        }
        if meta.file_type().is_symlink() {
            let before = self.sum.done;
            let new = dst.symlink_metadata().is_err();
            self.copy_symlink(src, dst);
            if self.sum.done > before {
                if new {
                    self.made(dst, None);
                } else if self.recording {
                    self.sum.replaced_for_good += 1;
                }
            }
            self.moved(src, dst, before);
        } else if meta.is_dir() {
            let made = dst.symlink_metadata().is_err();
            if let Err(e) = fs::create_dir_all(dst) {
                self.note_error(dst, &e);
                return;
            }
            if made {
                self.made(dst, None);
            }
            // Moving: a directory made here keeps its name in a power cut before any
            // source under it is removed (its files sync their own names).
            if self.moving && made {
                if let Err(e) = dst.parent().map_or(Ok(()), crate::faults::sync_dir) {
                    self.note_error(dst, &e);
                    return;
                }
            }
            let entries = match fs::read_dir(src) {
                Ok(it) => it,
                Err(e) => {
                    self.note_error(src, &e);
                    return;
                }
            };
            for ent in entries.flatten() {
                if self.stopped() {
                    return;
                }
                self.copy_node(&ent.path(), &dst.join(ent.file_name()));
            }
            let _ = fs::set_permissions(dst, meta.permissions());
            self.sum.done += 1;
            // Moving: the directory goes only once empty, that is when everything in
            // it was copied and removed; anything skipped keeps it.
            if self.moving {
                // Its files' sources first: the directory goes only once empty.
                self.remove_moved();
            }
            if self.moving && !self.stopped() {
                let _ = crate::faults::at(crate::faults::Point::RemoveSource, src)
                    .and_then(|()| fs::remove_dir(src));
            }
        } else {
            let before = self.sum.done;
            self.copy_file(src, dst, meta.len());
            self.moved(src, dst, before);
        }
    }

    /// While moving across devices: `src`'s copy `dst` just completed (the count of
    /// done items went up), so its source is to be removed. A skipped or failed copy
    /// leaves it. All of that waits for [`Worker::remove_moved`], a batch at a time.
    fn moved(&mut self, src: &Path, dst: &Path, done_before: u64) {
        if !self.moving || self.sum.done <= done_before {
            return;
        }
        let file = self.moved_file.take();
        self.removing_bytes += fs::symlink_metadata(dst).map(|m| m.len()).unwrap_or(0);
        self.removing
            .push((src.to_path_buf(), dst.to_path_buf(), file));
        if self.removing.len() >= MOVE_BATCH_FILES || self.removing_bytes >= MOVE_BATCH_BYTES {
            self.remove_moved();
        }
    }

    /// Remove the sources of the copies waiting in [`Worker::removing`]. First each
    /// copy's data is synced, then each copy's directory (its name then survives a power
    /// cut too), then the journal's `moved` lines, one sync for the batch (a kill from
    /// here on is told at the next start that both are there), and only then do the
    /// sources go. A file's source never goes before its copy is on disk. A copy that
    /// cannot be synced may not hold what was read: it is removed and its source kept.
    ///
    /// Batched (the owner's decision 4, Round 029): three syncs per file made a move of
    /// 10,000 small files to a disk eight times slower. Syncing a batch of files after
    /// writing them all lets the system write them out together.
    fn remove_moved(&mut self) {
        let batch = std::mem::take(&mut self.removing);
        self.removing_bytes = 0;
        if batch.is_empty() {
            return;
        }
        let mut kept = Vec::with_capacity(batch.len());
        for (src, dst, file) in batch {
            // Through the handle that wrote it; none for a link (no data of its own;
            // its name is synced below) or a replacement (synced before its rename).
            let synced = match &file {
                Some(f) => crate::faults::sync_data(f, &dst),
                None => Ok(()),
            };
            drop(file);
            match synced {
                Ok(()) => kept.push((src, dst)),
                Err(e) => {
                    // Never synced again (fsyncgate): this copy goes, its source stays,
                    // and a later move copies it afresh.
                    let _ = remove_file_anyway(&dst);
                    self.sum.done = self.sum.done.saturating_sub(1);
                    self.note_error(&dst, &e);
                }
            }
        }
        let mut batch: Vec<(PathBuf, PathBuf)> = kept;
        let mut dirs: Vec<PathBuf> = batch
            .iter()
            .filter_map(|(_, d)| d.parent())
            .map(Path::to_path_buf)
            .collect();
        dirs.sort();
        dirs.dedup();
        for dir in dirs {
            if let Err(e) = crate::faults::sync_dir(&dir) {
                // Its copies are complete but their names may not last: their sources stay.
                for (src, _) in batch
                    .iter()
                    .filter(|(_, d)| d.parent() == Some(dir.as_path()))
                {
                    self.note_error(src, &e);
                }
                batch.retain(|(_, d)| d.parent() != Some(dir.as_path()));
            }
        }
        let Some((first, _)) = batch.first().cloned() else {
            return;
        };
        if let Some(j) = self.journal.as_mut() {
            for (src, dst) in &batch {
                let _ = j.moved(src, dst);
            }
        }
        if !self.durable(&first) {
            return;
        }
        for (src, _) in batch {
            if let Err(e) = crate::faults::at(crate::faults::Point::RemoveSource, &src)
                .and_then(|()| remove_file_anyway(&src))
            {
                self.note_error(&src, &e);
            }
        }
    }

    #[cfg(unix)]
    fn copy_symlink(&mut self, src: &Path, dst: &Path) {
        match fs::read_link(src) {
            Ok(target) => {
                if matches!(self.resolve_conflict(Some(src), dst, 0), Some(false) | None) {
                    if !self.abort {
                        self.sum.skipped += 1;
                    }
                    return;
                }
                // A link replacing something is made under a temporary name and renamed
                // over it, like a file: removing the old entry first lost it whenever
                // making the link then failed.
                let replacing = dst.symlink_metadata().is_ok();
                let made = if replacing {
                    let tmp = part_name(dst);
                    if let Some(j) = self.journal.as_mut() {
                        let _ = j.part(&tmp);
                    }
                    let r = crate::faults::at(crate::faults::Point::Write, dst)
                        .and_then(|()| std::os::unix::fs::symlink(&target, &tmp))
                        .and_then(|()| {
                            if !self.durable(dst) {
                                return Ok(false);
                            }
                            crate::faults::at(crate::faults::Point::Rename, dst)?;
                            fs::rename(&tmp, dst).map(|()| true)
                        });
                    if !matches!(r, Ok(true)) {
                        let _ = fs::remove_file(&tmp);
                    }
                    r
                } else {
                    crate::faults::at(crate::faults::Point::Write, dst)
                        .and_then(|()| std::os::unix::fs::symlink(&target, dst))
                        .map(|()| true)
                };
                match made {
                    Ok(true) => self.sum.done += 1,
                    Ok(false) => {}
                    Err(e) => self.note_error(dst, &e),
                }
            }
            Err(e) => self.note_error(src, &e),
        }
    }

    #[cfg(not(unix))]
    fn copy_symlink(&mut self, src: &Path, _dst: &Path) {
        let e = io::Error::other("symlinks are not copied on this platform");
        self.note_error(src, &e);
    }

    fn copy_file(&mut self, src: &Path, dst: &Path, size: u64) {
        loop {
            if self.stopped() {
                return;
            }
            // Free now: written only if it is still free when opened, so a file that
            // appears meanwhile is asked about, not overwritten.
            let free = dst.symlink_metadata().is_err();
            match self.resolve_conflict(Some(src), dst, size) {
                Some(true) => {}
                Some(false) => {
                    self.sum.skipped += 1;
                    if self.moving && self.resuming && identical(Some(src), dst) == Same::Yes {
                        self.sum.left += 1;
                    }
                    self.advance(size);
                    return;
                }
                None => return,
            }
            appear_in_test(dst);
            match self.copy_bytes(src, dst, free) {
                Ok(true) => {
                    self.sum.done += 1;
                    if free {
                        self.made(dst, None);
                    } else if let Some(t) = self.old_trashed.take() {
                        self.sum.replaced_to_trash += 1;
                        self.made(dst, Some(t));
                    } else if self.recording {
                        self.sum.replaced_for_good += 1;
                    }
                    return;
                }
                // Taken since it was found free: round again, to the question.
                Err(e) if free && e.kind() == io::ErrorKind::AlreadyExists => continue,
                // Not the file agreed to any more: asked again.
                Ok(false) if self.changed.as_deref() == Some(dst) => continue,
                // Cancelled mid-file: copy_bytes removed what it wrote, and a file
                // being replaced is still the old one.
                Ok(false) => return,
                Err(e) => match self.ask(Question::Failed {
                    path: dst.to_path_buf(),
                    error: e.to_string(),
                }) {
                    Answer::Retry => continue,
                    Answer::Skip | Answer::SkipAll => {
                        self.note_error(dst, &e);
                        self.sum.skipped += 1;
                        return;
                    }
                    _ => {
                        self.note_error(dst, &e);
                        self.abort = true;
                        return;
                    }
                },
            }
        }
    }

    /// Returns Ok(false) when cancelled part-way. `new_only`: fail with `AlreadyExists`
    /// rather than replace a file at `dst`.
    ///
    /// Replacing writes beside `dst` under a temporary name and renames it over `dst`
    /// only once complete: a cancel or an error part-way leaves the old file as it
    /// was, rather than both old and new gone. With safe operations a new file goes the
    /// same way, so not even a killed run leaves a short file under a real name.
    /// Whatever was written and not kept is removed here.
    fn copy_bytes(&mut self, src: &Path, dst: &Path, new_only: bool) -> io::Result<bool> {
        let meta = fs::metadata(src)?;
        let mut r = File::open(src)?;
        let in_place = new_only && !self.safe;
        let target = if in_place {
            dst.to_path_buf()
        } else {
            part_name(dst)
        };
        self.moved_file = None;
        writing_in_test(&target);
        if !in_place {
            if let Some(j) = self.journal.as_mut() {
                // A failure breaks the journal; asked about before a step that needs it.
                let _ = j.part(&target);
            }
        }
        let mut w = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let written = self.write_all_from(&mut r, &mut w, &target);
        let kept = match written {
            Ok(true) => {
                // Permissions and mtime are part of the file as far as a user is
                // concerned; a copy that loses the executable bit broke something.
                let _ = fs::set_permissions(&target, meta.permissions());
                if let Ok(mtime) = meta.modified() {
                    let _ = w.set_times(FileTimes::new().set_modified(mtime));
                }
                // A new file is not synced here (the owner's call: new files stay
                // fast). One whose source is to be removed (a move across devices) is
                // synced with its batch, before any source goes (`remove_moved`).
                // Moving: the handle is kept, to be synced through with its batch.
                let keep = |me: &mut Self, w: File| {
                    if me.moving {
                        me.moved_file = Some(w);
                    }
                };
                if in_place {
                    keep(self, w);
                    Ok(true)
                } else if new_only {
                    // Taken only if still free: a file that appeared meanwhile is asked
                    // about (the caller goes round again on AlreadyExists).
                    let r = crate::faults::at(crate::faults::Point::Rename, dst)
                        .and_then(|()| crate::noreplace::rename_noreplace(&target, dst))
                        .map(|()| true);
                    if r.is_ok() {
                        keep(self, w);
                    }
                    r
                } else {
                    // Removes the temporary file itself on any failure. The file there is
                    // checked once more right before the rename: no portable rename
                    // compares first, so a window of microseconds is left, not the
                    // whole copy.
                    // Where the trash is on this volume the old file goes there right
                    // before the rename, so undo brings it back (Round 031). The
                    // journal's `swap` line first: killed between the two, the next
                    // start renames the complete new file in.
                    let trash = self.replace_into_trash(dst);
                    self.old_trashed = None;
                    let r = commit_replace(w, &target, dst, || {
                        if trash {
                            if let Some(j) = self.journal.as_mut() {
                                let _ = j.swap(&target, dst);
                            }
                        }
                        // The journal first: its question can be open for minutes, and
                        // the look at the target is the last thing before the rename.
                        if !(self.durable(dst) && self.still_agreed(Some(src), dst)) {
                            return Ok(false);
                        }
                        if trash {
                            let t = crate::faults::at(crate::faults::Point::Trash, dst)
                                .and_then(|()| send_to_trash(dst, self.trash_root.as_deref()))
                                .map_err(|e| {
                                    io::Error::new(
                                        e.kind(),
                                        format!("the old file could not go to the trash: {e}"),
                                    )
                                })?;
                            self.old_trashed = Some(t);
                        }
                        Ok(true)
                    });
                    // The rename failed after the old file went: it comes back.
                    if !matches!(r, Ok(true)) {
                        if let Some(t) = self.old_trashed.take() {
                            let _ = crate::trashed::restore(&t);
                        }
                    }
                    return r;
                }
            }
            other => other,
        };
        if !matches!(kept, Ok(true)) {
            let _ = fs::remove_file(&target);
        }
        kept
    }

    /// Copy `r` into `w`, which is `path`; Ok(false) when cancelled part-way.
    fn write_all_from(&mut self, r: &mut File, w: &mut File, path: &Path) -> io::Result<bool> {
        let mut buf = vec![0u8; BUF];
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crate::faults::at(crate::faults::Point::Write, path)?;
            w.write_all(&buf[..n])?;
            self.advance(n as u64);
        }
        w.flush()?;
        Ok(true)
    }

    // ── move ───────────────────────────────────────────────────────────────────────

    fn move_into(&mut self, src: &Path, dest_dir: &Path, rename: Option<&std::ffi::OsStr>) {
        self.approved = None;
        let Some(name) = rename.or(src.file_name()) else {
            return;
        };
        let dst = dest_dir.join(name);
        if dst == src {
            self.sum.skipped += 1;
            return;
        }
        if src.is_dir() && dst.starts_with(src) {
            let e = io::Error::other("cannot move a directory into itself");
            self.note_error(src, &e);
            return;
        }
        let size = fs::symlink_metadata(src).map(|m| m.len()).unwrap_or(0);
        let moved = loop {
            let free = dst.symlink_metadata().is_err();
            match self.resolve_conflict(Some(src), &dst, size) {
                Some(true) if !free => self.approved = Some(dst.clone()),
                Some(true) => {}
                Some(false) => {
                    self.sum.skipped += 1;
                    if self.resuming && identical(Some(src), &dst) == Same::Yes {
                        self.sum.left += 1;
                    }
                    return;
                }
                None => return,
            }
            // A name found free is taken only if still free, in the same step as the
            // move; one taken since goes round again, to the question.
            appear_in_test(&dst);
            let r = if free {
                crate::noreplace::rename_noreplace(src, &dst)
            } else if !self.durable(&dst) {
                return;
            } else if !self.still_agreed(Some(src), &dst) {
                // Changed since it was agreed to (or while the journal was asked
                // about): back to the question.
                self.approved = None;
                continue;
            } else {
                fs::rename(src, &dst)
            };
            match r {
                Err(e) if free && e.kind() == io::ErrorKind::AlreadyExists => continue,
                r => break r,
            }
        };

        match moved {
            Ok(()) => {
                self.sum.done += 1;
                self.advance(size);
            }
            Err(e) if is_cross_device(&e) => {
                // Not a rename at all: a copy followed by a delete, which can half-fail.
                // The user is told before it starts.
                let go = if self.cross_all {
                    true
                } else {
                    match self.ask(Question::CrossDevice {
                        path: src.to_path_buf(),
                    }) {
                        Answer::Yes => true,
                        Answer::YesToAll => {
                            self.cross_all = true;
                            true
                        }
                        Answer::Skip | Answer::SkipAll => false,
                        _ => {
                            self.abort = true;
                            return;
                        }
                    }
                };
                if !go {
                    self.sum.skipped += 1;
                    return;
                }
                // Each source is removed once its own copy is complete. Before, the
                // whole tree was removed when nothing had *failed*, and a file the
                // user chose to skip was deleted with it, never copied (0.2.0).
                self.moving = true;
                self.copy_node(src, &dst);
                self.remove_moved();
                self.moving = false;
                self.approved = None;
            }
            Err(e) => {
                self.approved = None;
                self.note_error(&dst, &e)
            }
        }
        self.approved = None;
    }

    // ── delete ─────────────────────────────────────────────────────────────────────

    fn delete(&mut self, target: &Path, to_trash: bool, trash_root: Option<&Path>) {
        if self.stopped() {
            return;
        }
        if to_trash {
            match send_to_trash(target, trash_root) {
                Ok(t) => {
                    self.sum.done += 1;
                    self.sum.trashed.push(t);
                    return;
                }
                Err(e) => {
                    let answer = if self.trash_all {
                        Answer::Yes
                    } else {
                        self.ask(Question::TrashUnavailable {
                            path: target.to_path_buf(),
                            reason: e.to_string(),
                        })
                    };
                    match answer {
                        Answer::Yes => {}
                        Answer::YesToAll => self.trash_all = true,
                        Answer::Skip | Answer::SkipAll => {
                            self.sum.skipped += 1;
                            return;
                        }
                        _ => {
                            self.abort = true;
                            return;
                        }
                    }
                }
            }
        }
        match remove_tree(target) {
            Ok(()) => self.sum.done += 1,
            Err(e) => self.note_error(target, &e),
        }
    }
}

impl crate::archive::Hooks for Worker {
    fn stopped(&self) -> bool {
        Worker::stopped(self)
    }
    fn begin(&mut self, path: &Path) {
        let _ = self.tx.send(Event::Begin {
            path: path.to_path_buf(),
        });
    }
    fn conflict(&mut self, dst: &Path, incoming: u64) -> Option<bool> {
        self.resolve_conflict(None, dst, incoming)
    }
    fn part(&mut self, p: &Path, made: bool) {
        // Only what is made is written down: a later start removes only what is
        // still there.
        if let Some(j) = self.journal.as_mut().filter(|_| made) {
            let _ = j.part(p);
        }
    }
    fn durable(&mut self, dst: &Path) -> bool {
        Worker::durable(self, dst)
    }
    fn recheck(&mut self, dst: &Path) -> io::Result<()> {
        if self.still_agreed(None, dst) {
            return Ok(());
        }
        self.changed = None;
        Err(io::Error::other(
            "it changed since you answered \u{2014} kept as it is",
        ))
    }
    fn wrote(&mut self, bytes: u64) {
        self.advance(bytes);
    }
    fn done(&mut self) {
        self.sum.done += 1;
    }
    fn skipped(&mut self, path: &Path, why: &str) {
        self.sum.skipped += 1;
        if self.sum.errors.len() < 20 {
            self.sum.errors.push(format!("{}: {why}", path.display()));
        }
    }
    fn failed(&mut self, path: &Path, e: &io::Error) {
        self.note_error(path, e);
    }
}

// ── free functions ─────────────────────────────────────────────────────────────────

/// What [`Existing::Resume`] makes of a destination that is there.
#[derive(Debug, PartialEq, Eq)]
enum Same {
    /// Two directories: they merge, as with every other rule.
    Dirs,
    /// The same file: what the interrupted run finished.
    Yes,
    /// Anything else, an unknown date included: the user is asked.
    No,
}

/// Whether `dst` is what copying `src` would have made: the same size and date (a copy
/// keeps the date), or a link to the same target. Without a source (an archive) or
/// with a date the system will not give, it is not taken for the same.
fn identical(src: Option<&Path>, dst: &Path) -> Same {
    let (Some(src), Ok(d)) = (src, fs::symlink_metadata(dst)) else {
        return Same::No;
    };
    let Ok(s) = fs::symlink_metadata(src) else {
        return Same::No;
    };
    if s.is_dir() && d.is_dir() {
        return Same::Dirs;
    }
    if s.file_type().is_symlink() && d.file_type().is_symlink() {
        return match (fs::read_link(src), fs::read_link(dst)) {
            (Ok(a), Ok(b)) if a == b => Same::Yes,
            _ => Same::No,
        };
    }
    let dated = s.modified().is_ok() && d.modified().is_ok();
    if s.is_file()
        && d.is_file()
        && s.len() == d.len()
        && dated
        && crate::compare::newer_than(&s, &d) == std::cmp::Ordering::Equal
    {
        Same::Yes
    } else {
        Same::No
    }
}

/// Count what the job is about to touch, so a progress bar means something.
pub fn scan(sources: &[PathBuf]) -> (u64, u64) {
    scan_until(sources, &AtomicBool::new(false)).unwrap_or((0, 0))
}

/// As [`scan`], stopping when `stop` is set: `None` then. A count of a large tree on a
/// slow disk takes seconds, and the dialog asking about it must not wait for it.
pub fn scan_until(sources: &[PathBuf], stop: &AtomicBool) -> Option<(u64, u64)> {
    fn walk(p: &Path, files: &mut u64, bytes: &mut u64, stop: &AtomicBool) -> bool {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(p) else {
            return true;
        };
        if meta.file_type().is_symlink() {
            *files += 1;
        } else if meta.is_dir() {
            if let Ok(it) = fs::read_dir(p) {
                for e in it.flatten() {
                    if !walk(&e.path(), files, bytes, stop) {
                        return false;
                    }
                }
            }
        } else {
            *files += 1;
            *bytes += meta.len();
        }
        true
    }
    let (mut f, mut b) = (0, 0);
    for s in sources {
        if !walk(s, &mut f, &mut b, stop) {
            return None;
        }
    }
    Some((f, b))
}

/// [`scan_until`] on its own thread. Dropping the handle stops it.
pub struct Count {
    pub rx: Receiver<(u64, u64)>,
    stop: Arc<AtomicBool>,
}

impl Drop for Count {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn count(sources: Vec<PathBuf>) -> Count {
    Count::spawn(move |stop| scan_until(&sources, stop))
}

impl Count {
    /// Any count, run on its own thread: `work` checks the flag and gives up with
    /// `None` when it is set.
    pub fn spawn(work: impl FnOnce(&AtomicBool) -> Option<(u64, u64)> + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            if let Some(n) = work(&flag) {
                let _ = tx.send(n);
            }
        });
        Count { rx, stop }
    }
}

pub fn remove_tree(p: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(p)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
}

/// `[general] safe_operations`: every copy writes under a temporary name and renames at
/// the end, and jobs keep a journal (journal.rs) so a run killed outright is tidied up
/// at the next start. Off, a new file is written under its own name, as before; a
/// replacement goes through a temporary name either way. Read once per job, into
/// [`Request::safe`], never by a running worker.
static SAFE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_safe(on: bool) {
    SAFE.store(on, Ordering::Relaxed);
}

pub fn safe() -> bool {
    SAFE.load(Ordering::Relaxed)
}

/// The one way xdevcmd replaces a file (Round 029). `tmp`, complete and still open as
/// `f`, is synced to the disk (a close error, or an NFS or SMB write that failed, is told
/// here, not lost when the file is dropped), then `ready` runs (the journal's line
/// synced, the target looked at once more) and may stop it with `Ok(false)`, and only
/// then is `tmp` renamed over `dst`. Without the sync, a power cut after the rename can
/// leave `dst` empty or short where ext4's defaults do not happen to save it.
///
/// Whatever does not end in the rename removes `tmp`, and `dst` is as it was. A sync
/// that failed is never tried again on the same file: Linux may have dropped the data
/// and cleared the error ("fsyncgate", PostgreSQL 2018), so the caller writes the file
/// again from its source.
pub(crate) fn commit_replace(
    f: File,
    tmp: &Path,
    dst: &Path,
    ready: impl FnOnce() -> io::Result<bool>,
) -> io::Result<bool> {
    let r = (|| {
        crate::faults::sync_data(&f, tmp)?;
        drop(f);
        if !ready()? {
            return Ok(false);
        }
        crate::faults::at(crate::faults::Point::Rename, dst)?;
        fs::rename(tmp, dst)?;
        Ok(true)
    })();
    if !matches!(r, Ok(true)) {
        let _ = fs::remove_file(tmp);
    }
    r
}

/// Remove the file `p`. On Windows a read-only file cannot be removed until its
/// read-only attribute is cleared: a moved read-only source, or a read-only copy that
/// failed its sync, would otherwise stay.
fn remove_file_anyway(p: &Path) -> io::Result<()> {
    let r = fs::remove_file(p);
    #[cfg(windows)]
    if let Err(e) = &r {
        if e.kind() == io::ErrorKind::PermissionDenied {
            if let Ok(m) = fs::symlink_metadata(p) {
                let mut perm = m.permissions();
                if perm.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    if fs::set_permissions(p, perm).is_ok() {
                        return fs::remove_file(p);
                    }
                }
            }
        }
    }
    r
}

/// A temporary name beside `dst` to write a replacement under: hidden, and unique to
/// this run (pid and start time, [`crate::journal::run_tag`]) and call, so two jobs
/// never share one and a later process given the same pid never takes it for its own.
pub(crate) fn part_name(dst: &Path) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = dst
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = dst.parent().unwrap_or(Path::new("."));
    dir.join(format!(
        ".{name}.xdevcmd-part-{}-{n}",
        crate::journal::run_tag()
    ))
}

/// Tests put a path here to have a file appear at it between the check that found the
/// name free and the write: the moment no test could otherwise reach.
#[cfg(test)]
static APPEAR: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
fn appear_in_test(dst: &Path) {
    let mut paths = APPEAR.lock().unwrap();
    if let Some(i) = paths.iter().position(|p| p == dst) {
        paths.remove(i);
        fs::write(dst, "theirs").unwrap();
    }
}

#[cfg(not(test))]
fn appear_in_test(_: &Path) {}

/// Every name a copy opened to write, for tests to see which way a job wrote.
#[cfg(test)]
static WRITING: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
fn writing_in_test(target: &Path) {
    WRITING.lock().unwrap().push(target.to_path_buf());
}

#[cfg(not(test))]
fn writing_in_test(_: &Path) {}

fn is_cross_device(e: &io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(18) // EXDEV
    }
    #[cfg(windows)]
    {
        e.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = e;
        false
    }
}

/// Move one entry to wherever this platform keeps deleted files, and say where it went.
pub fn send_to_trash(path: &Path, root: Option<&Path>) -> io::Result<crate::trashed::Trashed> {
    if let Some(base) = root {
        // Tests pass a temp directory here, so a test run never puts anything in the
        // real trash of whoever happens to be running it.
        return trash_into(base, path, None);
    }
    #[cfg(target_os = "linux")]
    {
        let home = default_trash_base()
            .ok_or_else(|| io::Error::other("no HOME, so no trash directory"))?;
        match volume_trash(path, &home)? {
            None => trash_into(&home, path, None),
            Some((base, topdir)) => trash_into(&base, path, Some(&topdir)),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use crate::trashed::{Place, Trashed};
        let when = std::time::SystemTime::now();
        let at = crate::trashed::mac_trash(path)?;
        Ok(Trashed {
            original: path.to_path_buf(),
            when,
            place: Place::At(at),
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        use crate::trashed::{Place, Trashed};
        let when = std::time::SystemTime::now();
        ::trash::delete(path).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Trashed {
            original: path.to_path_buf(),
            when,
            place: Place::RecycleBin,
        })
    }
}

/// The freedesktop trash for a path on another file system than the home trash: the
/// spec's `$topdir/.Trash/$uid` when an administrator made `.Trash` (a real, sticky
/// directory), else `$topdir/.Trash-$uid`, made here, private. `None` for the same file
/// system as `home`, whose trash is used.
#[cfg(target_os = "linux")]
fn volume_trash(path: &Path, home: &Path) -> io::Result<Option<(PathBuf, PathBuf)>> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let dir = path.parent().unwrap_or(Path::new("/"));
    let dev = fs::metadata(dir)?.dev();
    // The home trash may not exist yet: its nearest existing ancestor decides.
    let home_dev = home
        .ancestors()
        .find_map(|a| fs::metadata(a).ok())
        .map(|m| m.dev());
    if home_dev == Some(dev) {
        return Ok(None);
    }
    // The top of the mount: the last ancestor still on this device.
    let mut topdir = dir.to_path_buf();
    while let Some(up) = topdir.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => topdir = up.to_path_buf(),
            _ => break,
        }
    }
    let uid = rustix::process::getuid().as_raw();
    let shared = topdir.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&shared) {
        let sticky = m.permissions().mode() & 0o1000 != 0;
        if m.is_dir() && !m.file_type().is_symlink() && sticky {
            let mine = shared.join(uid.to_string());
            if fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&mine)
                .is_ok()
            {
                return Ok(Some((mine, topdir)));
            }
        }
    }
    let own = topdir.join(format!(".Trash-{uid}"));
    if let Ok(m) = fs::symlink_metadata(&own) {
        if !m.is_dir() || m.file_type().is_symlink() || m.uid() != uid {
            return Err(io::Error::other(format!(
                "{} is not a trash directory of yours",
                own.display()
            )));
        }
    } else {
        fs::DirBuilder::new().mode(0o700).create(&own)?;
    }
    Ok(Some((own, topdir)))
}

/// Whether a file in the directory `dir` can go to a trash on its own file system:
/// the home trash on the same one, or one [`volume_trash`] would use or may make at the
/// top of its mount. Reads only: nothing is made here (the planner asks it).
#[cfg(target_os = "linux")]
pub(crate) fn volume_trash_usable(dir: &Path) -> Option<bool> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let home = default_trash_base()?;
    let dev = fs::metadata(dir).ok()?.dev();
    let home_dev = home
        .ancestors()
        .find_map(|a| fs::metadata(a).ok())
        .map(|m| m.dev());
    if home_dev == Some(dev) {
        return Some(true);
    }
    let mut topdir = dir.to_path_buf();
    while let Some(up) = topdir.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => topdir = up.to_path_buf(),
            _ => break,
        }
    }
    let uid = rustix::process::getuid().as_raw();
    let usable_dir = |p: &Path| {
        rustix::fs::access(
            p,
            rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
        )
        .is_ok()
    };
    let shared = topdir.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&shared) {
        let sticky = m.permissions().mode() & 0o1000 != 0;
        if m.is_dir() && !m.file_type().is_symlink() && sticky {
            let mine = shared.join(uid.to_string());
            if mine.is_dir() && usable_dir(&mine) || !mine.exists() && usable_dir(&shared) {
                return Some(true);
            }
        }
    }
    let own = topdir.join(format!(".Trash-{uid}"));
    Some(match fs::symlink_metadata(&own) {
        Ok(m) => m.is_dir() && !m.file_type().is_symlink() && m.uid() == uid,
        Err(_) => usable_dir(&topdir),
    })
}

/// Where deleted files go on Linux: `$XDG_DATA_HOME/Trash`, per the freedesktop spec.
pub fn default_trash_base() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("Trash"))
}

/// Move one entry into a freedesktop trash directory, with the `.trashinfo` that
/// makes it restorable.
///
/// As the spec says, the `.trashinfo` is made first (with `O_EXCL`, which also reserves
/// the name), then the entry moved: a crash in between leaves a record without a file,
/// which tools ignore, never a file nobody can restore. `topdir`: a per-volume trash,
/// whose records give the path relative to the volume's top.
fn trash_into(
    base: &Path,
    path: &Path,
    topdir: Option<&Path>,
) -> io::Result<crate::trashed::Trashed> {
    use crate::trashed::{Place, Trashed};
    let when = std::time::SystemTime::now();
    let files = base.join("files");
    let info = base.join("info");
    fs::create_dir_all(&files)?;
    fs::create_dir_all(&info)?;

    // The path of the entry itself, not of what a link points to: the directory is
    // made canonical, the name kept.
    let name_os = path
        .file_name()
        .ok_or_else(|| io::Error::other("that has no name to put in the trash"))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let original = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .join(name_os);
    let recorded = match topdir.and_then(|t| original.strip_prefix(t).ok()) {
        Some(rel) => rel.to_path_buf(),
        None => original.clone(),
    };
    let stem = name_os.to_string_lossy().into_owned();
    let stamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(&recorded.to_string_lossy()),
        stamp
    );

    // The first free name: its record reserved, then the entry moved in the same step
    // as the name is checked, so nothing in the trash is ever overwritten.
    let mut name = stem.clone();
    let mut n = 1;
    loop {
        let record = info.join(format!("{name}.trashinfo"));
        let reserved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&record);
        match reserved {
            Ok(mut f) => {
                f.write_all(body.as_bytes())?;
                drop(f);
                match crate::noreplace::rename_noreplace(path, &files.join(&name)) {
                    Ok(()) => {
                        return Ok(Trashed {
                            original: path.to_path_buf(),
                            when,
                            place: Place::Freedesktop {
                                file: files.join(&name),
                                info: record,
                            },
                        });
                    }
                    Err(e) => {
                        let _ = fs::remove_file(&record);
                        if e.kind() != io::ErrorKind::AlreadyExists {
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        name = format!("{stem}.{n}");
        n += 1;
    }
}

/// Percent-encoding as the trash spec wants it: path separators stay readable,
/// everything outside the unreserved set is escaped.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    static N: AtomicU32 = AtomicU32::new(0);

    fn tmp(tag: &str) -> PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("xdevcmd-ops-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Drive a job to completion, answering every question the same way.
    fn run_with(req: Request, answer: Answer) -> (Summary, u32) {
        let h = start(req);
        let mut asked = 0;
        loop {
            match h.events.recv_timeout(Duration::from_secs(20)) {
                Ok(Event::Ask(_)) => {
                    asked += 1;
                    h.answer(answer);
                }
                Ok(Event::Done(s)) => return (s, asked),
                Ok(_) => {}
                Err(e) => panic!("worker never finished: {e}"),
            }
        }
    }

    #[test]
    fn a_count_can_be_stopped_and_otherwise_arrives() {
        let root = std::env::temp_dir().join(format!("xdevcmd-ops-count-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/f"), "1234").unwrap();
        fs::write(root.join("g"), "56").unwrap();
        assert_eq!(
            scan_until(std::slice::from_ref(&root), &AtomicBool::new(true)),
            None
        );
        let c = count(vec![root.clone()]);
        let got =
            c.rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        assert_eq!(got, (2, 6));
        let _ = fs::remove_dir_all(&root);
    }

    fn run(req: Request) -> Summary {
        run_with(req, Answer::Yes).0
    }

    /// Round 031: with a trash on the volume, a replaced file goes there, after the
    /// journal is on disk and before the rename; undo removes the new one and brings
    /// the old one back.
    #[test]
    fn a_replaced_file_goes_to_the_trash_first_and_undo_brings_it_back() {
        journals();
        let d = tmp("replace-trash");
        let tag = format!("xrt-{}", std::process::id());
        fs::create_dir_all(d.join("src")).unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("src").join(&tag), "new contents").unwrap();
        fs::write(d.join("to").join(&tag), "old").unwrap();
        crate::faults::trace(&tag);
        let mut req = Request::copy(vec![d.join("src").join(&tag)], d.join("to"));
        req.trash_root = Some(d.join("trash"));
        req.safe = true;
        req.existing = Existing::Replace;
        let (s, _) = run_with(req, Answer::Yes);
        let seen = crate::faults::traced(&tag);
        let dst = d.join("to").join(&tag);
        assert_eq!(fs::read_to_string(&dst).unwrap(), "new contents");
        assert_eq!(
            (s.done, s.replaced_to_trash, s.replaced_for_good),
            (1, 1, 0),
            "{s:?}"
        );
        assert!(s
            .describe(Kind::Copy)
            .contains("1 replaced (the old one in the trash)"));
        let old = s.made[0].old.clone().expect("the old file is remembered");
        let crate::trashed::Place::Freedesktop { file, .. } = &old.place else {
            panic!("{old:?}")
        };
        assert_eq!(fs::read_to_string(file).unwrap(), "old");
        let at = |point: &str| {
            seen.iter()
                .rposition(|(p, path)| p.name() == point && path.to_string_lossy().contains(&tag))
                .unwrap_or_else(|| panic!("no {point} in {seen:?}"))
        };
        assert!(at("sync-data") < at("trash"), "{seen:?}");
        assert!(at("journal-sync") < at("trash"), "{seen:?}");
        assert!(at("trash") < at("rename"), "{seen:?}");

        let h = crate::undo::unmake(s.made.clone());
        let u = loop {
            if let Event::Done(u) = h
                .events
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
            {
                break u;
            }
        };
        assert_eq!((u.done, u.failed), (1, 0), "{u:?}");
        assert_eq!(
            fs::read_to_string(&dst).unwrap(),
            "old",
            "the old file is back"
        );
        let _ = fs::remove_dir_all(&d);
    }

    /// The rename failing after the old file went to the trash: the old file comes back
    /// under its name, and the temporary file goes.
    #[test]
    fn a_failed_rename_after_the_trash_puts_the_old_file_back() {
        let d = tmp("replace-trash-fail");
        let tag = format!("xrtf-{}", std::process::id());
        fs::create_dir_all(d.join("src")).unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("src").join(&tag), "new contents").unwrap();
        fs::write(d.join("to").join(&tag), "old").unwrap();
        let _f = crate::faults::fail(
            crate::faults::Point::Rename,
            &tag,
            1,
            crate::faults::Fault::Os(5),
        );
        let mut req = Request::copy(vec![d.join("src").join(&tag)], d.join("to"));
        req.trash_root = Some(d.join("trash"));
        req.existing = Existing::Replace;
        let (s, _) = run_with(req, Answer::Skip);
        assert_eq!(s.replaced_to_trash, 0, "{s:?}");
        assert_eq!(fs::read_to_string(d.join("to").join(&tag)).unwrap(), "old");
        let left: Vec<_> = fs::read_dir(d.join("to")).unwrap().flatten().collect();
        assert_eq!(left.len(), 1, "no temporary file left: {left:?}");
        let _ = fs::remove_dir_all(&d);
    }

    /// Round 031: a copy says what it made, a directory before what is in it, and not
    /// what it merged into, replaced or skipped.
    #[test]
    fn a_copy_reports_what_it_made_in_order() {
        let d = tmp("made");
        fs::create_dir_all(d.join("src/sub")).unwrap();
        fs::write(d.join("src/sub/new.txt"), "n").unwrap();
        fs::write(d.join("src/sub/old.txt"), "new contents").unwrap();
        fs::write(d.join("src/sub/kept.txt"), "k").unwrap();
        fs::create_dir_all(d.join("to/src/sub")).unwrap();
        fs::write(d.join("to/src/sub/old.txt"), "old").unwrap();
        fs::write(d.join("to/src/sub/kept.txt"), "mine").unwrap();
        let mut req = Request::copy(vec![d.join("src")], d.join("to"));
        req.expect
            .insert(d.join("to/src/sub/kept.txt"), Expect::Skip);
        let sum = run(req);
        let made: Vec<_> = sum.made.iter().map(|m| m.path.clone()).collect();
        assert_eq!(made, [d.join("to/src/sub/new.txt")], "{sum:?}");
        assert_eq!(sum.replaced_for_good, 1);
        assert_eq!(
            sum.made[0].ident,
            Ident::at(&d.join("to/src/sub/new.txt")).unwrap()
        );

        let e = tmp("made-dirs");
        fs::create_dir_all(e.join("a/b")).unwrap();
        fs::write(e.join("a/b/f"), "f").unwrap();
        fs::create_dir_all(e.join("to")).unwrap();
        let sum = run(Request::copy(vec![e.join("a")], e.join("to")));
        let made: Vec<_> = sum.made.iter().map(|m| m.path.clone()).collect();
        assert_eq!(made, [e.join("to/a"), e.join("to/a/b"), e.join("to/a/b/f")]);
        assert!(sum.made[0].ident.is_dir());
    }

    /// A move remembers nothing of the kind: its undo moves back.
    #[test]
    fn a_move_reports_nothing_made() {
        let d = tmp("made-move");
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("f"), "x").unwrap();
        let sum = run(Request::mv(vec![d.join("f")], d.join("to")));
        assert!(sum.made.is_empty() && !sum.made_over);
    }

    #[test]
    fn a_planned_job_does_not_walk_its_sources_again() {
        let d = tmp("planned");
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("f"), "x").unwrap();
        let mut req = Request::copy(vec![d.join("f")], d.join("to"));
        // Numbers no walk of this one file would give: they can only be the plan's.
        req.planned = Some((7, 70));
        let h = start(req);
        let scanned = loop {
            let e = h
                .events
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            if let Event::Scanned { files, bytes } = e {
                break (files, bytes);
            }
        };
        assert_eq!(scanned, (7, 70));
    }

    #[test]
    fn copies_a_tree_and_leaves_the_source_alone() {
        let root = tmp("copy-tree");
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), b"hello").unwrap();
        fs::write(src.join("nested/b.txt"), b"world").unwrap();

        let s = run(Request::copy(vec![src.clone()], dst.clone()));
        assert_eq!(s.failed, 0, "{:?}", s.errors);

        assert_eq!(fs::read(dst.join("src/a.txt")).unwrap(), b"hello");
        assert_eq!(fs::read(dst.join("src/nested/b.txt")).unwrap(), b"world");
        assert!(src.join("a.txt").exists(), "copy must not move");

        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_keeps_the_executable_bit() {
        use std::os::unix::fs::PermissionsExt;
        let root = tmp("copy-mode");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let script = root.join("run.sh");
        fs::write(&script, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        run(Request::copy(vec![script], dst.clone()));

        let mode = fs::metadata(dst.join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "a copy that drops +x has broken the file"
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_existing_destination_is_never_overwritten_without_asking() {
        let root = tmp("conflict");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("f.txt"), b"new").unwrap();
        fs::write(dst.join("f.txt"), b"old").unwrap();

        let (s, asked) = run_with(
            Request::copy(vec![root.join("f.txt")], dst.clone()),
            Answer::Skip,
        );
        assert_eq!(asked, 1, "it must ask before touching an existing file");
        assert_eq!(s.skipped, 1);
        assert_eq!(
            fs::read(dst.join("f.txt")).unwrap(),
            b"old",
            "skip must keep the original"
        );

        let (s, asked) = run_with(
            Request::copy(vec![root.join("f.txt")], dst.clone()),
            Answer::Yes,
        );
        assert_eq!(asked, 1);
        assert_eq!(s.done, 1);
        assert_eq!(fs::read(dst.join("f.txt")).unwrap(), b"new");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_file_that_appears_while_copying_or_moving_is_asked_about_not_overwritten() {
        let root = tmp("appears");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("c.txt"), b"ours").unwrap();
        fs::write(root.join("m.txt"), b"ours").unwrap();

        // dst/c.txt is free when checked, and taken just before the copy writes it.
        APPEAR.lock().unwrap().push(dst.join("c.txt"));
        let (s, asked) = run_with(
            Request::copy(vec![root.join("c.txt")], dst.clone()),
            Answer::Skip,
        );
        assert_eq!(asked, 1, "the copy asks about the file that appeared");
        assert_eq!(s.skipped, 1);
        assert_eq!(fs::read(dst.join("c.txt")).unwrap(), b"theirs");

        // The same for a move: the source stays, the newcomer is kept.
        APPEAR.lock().unwrap().push(dst.join("m.txt"));
        let (s, asked) = run_with(
            Request::mv(vec![root.join("m.txt")], dst.clone()),
            Answer::Skip,
        );
        assert_eq!(asked, 1, "the move asks about the file that appeared");
        assert_eq!(s.skipped, 1);
        assert_eq!(fs::read(dst.join("m.txt")).unwrap(), b"theirs");
        assert_eq!(fs::read(root.join("m.txt")).unwrap(), b"ours");

        // Answered Yes, it replaces it, as for any file that was there.
        APPEAR.lock().unwrap().push(dst.join("m2.txt"));
        fs::write(root.join("m2.txt"), b"ours").unwrap();
        let (s, _) = run_with(
            Request::mv(vec![root.join("m2.txt")], dst.clone()),
            Answer::Yes,
        );
        assert_eq!(s.done, 1);
        assert_eq!(fs::read(dst.join("m2.txt")).unwrap(), b"ours");

        fs::remove_dir_all(&root).unwrap();
    }

    /// A directory on another file system than the temporary directory, for moves
    /// across devices. None, and said, where there is no such place.
    fn other_device(tag: &str) -> Option<PathBuf> {
        crate::faults::places::other_device(tag)
    }

    /// Run `req`, answering each question with `answer(question)`; the summary and
    /// the questions asked.
    fn run_answering(req: Request, answer: impl Fn(&Question) -> Answer) -> (Summary, Vec<String>) {
        let h = start(req);
        let mut asked = Vec::new();
        loop {
            match h.events.recv_timeout(Duration::from_secs(20)) {
                Ok(Event::Ask(q)) => {
                    asked.push(format!("{q:?}"));
                    h.answer(answer(&q));
                }
                Ok(Event::Done(s)) => return (s, asked),
                Ok(_) => {}
                Err(e) => panic!("worker never finished: {e}"),
            }
        }
    }

    #[test]
    fn a_move_across_devices_keeps_what_was_skipped() {
        let Some(other) = other_device("skip") else {
            return;
        };
        let root = tmp("xdev-skip");
        let src = root.join("d");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a"), b"A").unwrap();
        fs::write(src.join("b"), b"only copy of b").unwrap();
        fs::write(src.join("sub/c"), b"C").unwrap();
        fs::create_dir_all(other.join("d")).unwrap();
        fs::write(other.join("d/b"), b"theirs").unwrap();
        // Merge into the existing d, copy-then-delete across devices, but skip b.
        let (s, asked) =
            run_answering(Request::mv(vec![src.clone()], other.clone()), |q| match q {
                Question::Conflict { path, .. } if path.ends_with("b") => Answer::Skip,
                _ => Answer::Yes,
            });
        assert_eq!(s.skipped, 1, "{asked:#?} {s:?}");
        assert_eq!(
            fs::read(src.join("b")).unwrap(),
            b"only copy of b",
            "b stays: it was not copied"
        );
        assert_eq!(fs::read(other.join("d/b")).unwrap(), b"theirs");
        assert!(!src.join("a").exists() && fs::read(other.join("d/a")).unwrap() == b"A");
        assert!(!src.join("sub").exists(), "a directory moved in full goes");
        assert_eq!(fs::read(other.join("d/sub/c")).unwrap(), b"C");
        assert!(src.is_dir(), "d stays, since b is still in it");
        fs::remove_dir_all(&other).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_file_moved_across_devices_onto_another_is_asked_about_once() {
        let Some(other) = other_device("once") else {
            return;
        };
        let root = tmp("xdev-once");
        fs::write(root.join("f"), b"ours").unwrap();
        fs::write(other.join("f"), b"theirs").unwrap();
        let (s, asked) = run_answering(Request::mv(vec![root.join("f")], other.clone()), |_| {
            Answer::Yes
        });
        let conflicts = asked.iter().filter(|q| q.starts_with("Conflict")).count();
        assert_eq!(conflicts, 1, "{asked:?}");
        assert_eq!(s.done, 1);
        assert_eq!(fs::read(other.join("f")).unwrap(), b"ours");
        assert!(!root.join("f").exists());
        // Skipped at the one question: nothing moves, nothing is lost.
        fs::write(root.join("g"), b"ours").unwrap();
        fs::write(other.join("g"), b"theirs").unwrap();
        let (s, _) = run_answering(
            Request::mv(vec![root.join("g")], other.clone()),
            |q| match q {
                Question::Conflict { .. } => Answer::Skip,
                _ => Answer::Yes,
            },
        );
        assert_eq!(s.skipped, 1);
        assert_eq!(fs::read(root.join("g")).unwrap(), b"ours");
        assert_eq!(fs::read(other.join("g")).unwrap(), b"theirs");
        fs::remove_dir_all(&other).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// A real "disk full" from the kernel (CI's small file system): the replacement fails,
    /// the old file is kept, and nothing is left under a temporary name.
    #[test]
    fn a_replacement_that_fills_the_disk_keeps_the_old_file() {
        let Some(small) = crate::faults::places::small_fs("full-replace") else {
            return;
        };
        let root = tmp("full-replace");
        // Sparse: bigger than the whole small file system, free to make here.
        let big = File::create(root.join("f")).unwrap();
        big.set_len(64 << 20).unwrap();
        drop(big);
        fs::write(small.join("f"), b"old").unwrap();
        let (s, asked) =
            run_answering(
                Request::copy(vec![root.join("f")], small.clone()),
                |q| match q {
                    Question::Failed { .. } => Answer::Skip,
                    _ => Answer::Yes,
                },
            );
        assert!(
            asked.iter().any(|q| q.starts_with("Failed")),
            "the full disk is asked about: {asked:?}"
        );
        assert_eq!((s.done, s.failed), (0, 1), "{s:?}");
        assert_eq!(fs::read(small.join("f")).unwrap(), b"old");
        assert_eq!(
            fs::read_dir(&small).unwrap().count(),
            1,
            "no temporary file left"
        );
        fs::remove_dir_all(&small).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// Journals for these tests, in a directory of their own (set once per test run).
    fn journals() {
        crate::journal::init(
            std::env::temp_dir().join(format!("xdevcmd-test-journals-{}", std::process::id())),
        );
    }

    /// The journal's line for a replacement cannot be written (disk full, say): the
    /// replacement is not made until the user says go on without it, and Stop keeps the
    /// old file. Before Round 029 the line's error was dropped and the file replaced.
    #[test]
    fn a_journal_that_fails_before_a_replace_is_asked_about_and_stop_keeps_the_old_file() {
        journals();
        let root = tmp("journal-line");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("jl-a1.txt"), b"new").unwrap();
        fs::write(dst.join("jl-a1.txt"), b"old").unwrap();
        // The line naming the temporary file's directory, written as the copy starts.
        let _f = crate::faults::fail(
            crate::faults::Point::JournalLine,
            ".jl-a1.txt.xdevcmd-part",
            1,
            crate::faults::Fault::Os(28),
        );
        let mut req = Request::copy(vec![root.join("jl-a1.txt")], dst.clone());
        req.safe = true;
        let (s, asked) = run_answering(req, |q| match q {
            Question::Conflict { .. } => Answer::Yes,
            _ => Answer::Abort,
        });
        assert!(
            asked.iter().any(|q| q.starts_with("NoJournal")),
            "asked before replacing: {asked:?}"
        );
        assert_eq!(fs::read(dst.join("jl-a1.txt")).unwrap(), b"old", "{s:?}");
        assert_eq!(
            fs::read_dir(&dst).unwrap().count(),
            1,
            "no temporary file left"
        );
        // Go on without it: replaced, and the summary says the job ran without one.
        drop(_f);
        let _f = crate::faults::fail(
            crate::faults::Point::JournalLine,
            ".jl-a1.txt.xdevcmd-part",
            1,
            crate::faults::Fault::Os(28),
        );
        let mut req = Request::copy(vec![root.join("jl-a1.txt")], dst.clone());
        req.safe = true;
        let (s, asked) = run_answering(req, |_| Answer::Yes);
        let asked_once = asked.iter().filter(|q| q.starts_with("NoJournal")).count();
        assert_eq!(asked_once, 1, "{asked:?}");
        assert_eq!(fs::read(dst.join("jl-a1.txt")).unwrap(), b"new");
        assert!(
            s.describe(Kind::Copy).contains("safe operations off"),
            "{s:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// The journal cannot be begun: asked before anything is done. Stop does nothing;
    /// go on copies and says it ran without one. Before Round 029 nothing was asked and
    /// only the summary said so, once the job was over.
    #[test]
    fn a_journal_that_cannot_be_begun_is_asked_about_before_anything_is_done() {
        journals();
        let root = tmp("journal-begin");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("a"), b"a").unwrap();
        let frag = root.to_string_lossy().into_owned();
        let _f = crate::faults::fail(
            crate::faults::Point::JournalBegin,
            &frag,
            1,
            crate::faults::Fault::Os(28),
        );
        let mut req = Request::copy(vec![root.join("a")], dst.clone());
        req.safe = true;
        let (s, asked) = run_answering(req, |_| Answer::Abort);
        assert_eq!(asked.len(), 1, "{asked:?}");
        assert!(asked[0].starts_with("NoJournal"), "{asked:?}");
        assert!(!dst.join("a").exists(), "stopped before anything: {s:?}");
        assert!(s.cancelled);
        drop(_f);
        let _f = crate::faults::fail(
            crate::faults::Point::JournalBegin,
            &frag,
            1,
            crate::faults::Fault::Os(28),
        );
        let mut req = Request::copy(vec![root.join("a")], dst.clone());
        req.safe = true;
        let (s, _) = run_answering(req, |_| Answer::Yes);
        assert_eq!(fs::read(dst.join("a")).unwrap(), b"a");
        assert!(
            s.describe(Kind::Copy)
                .contains("safe operations off for this job"),
            "{s:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// The points a trace saw, in order, as their names.
    fn points(fragment: &str) -> Vec<&'static str> {
        crate::faults::traced(fragment)
            .into_iter()
            .map(|(p, _)| p.name())
            .collect()
    }

    fn first(seen: &[&str], name: &str) -> usize {
        seen.iter()
            .position(|p| *p == name)
            .unwrap_or_else(|| panic!("no {name} in {seen:?}"))
    }

    /// A replacement's data is on disk before its rename, with safe operations on (the
    /// journal's line synced in between) or off (the rule holds either way: the
    /// owner's decision 1). Before Round 029 nothing was synced.
    #[test]
    fn a_replacement_is_synced_before_it_is_renamed_into_place() {
        journals();
        for safe in [false, true] {
            let root = tmp(if safe {
                "sync-order-safe"
            } else {
                "sync-order"
            });
            let dst = root.join("dst");
            fs::create_dir_all(&dst).unwrap();
            fs::write(root.join("f"), b"new").unwrap();
            fs::write(dst.join("f"), b"old").unwrap();
            let frag = root.to_string_lossy().into_owned();
            crate::faults::trace(&frag);
            let mut req = Request::copy(vec![root.join("f")], dst.clone());
            req.safe = safe;
            let (s, _) = run_with(req, Answer::Yes);
            let seen = points(&frag);
            assert_eq!(s.done, 1, "{s:?}");
            assert!(
                first(&seen, "sync-data") < first(&seen, "rename"),
                "safe {safe}: {seen:?}"
            );
            if safe {
                let (synced, renamed) = (first(&seen, "sync-data"), first(&seen, "rename"));
                assert!(
                    seen[synced..renamed].contains(&"journal-sync"),
                    "the journal's line is synced between: {seen:?}"
                );
            }
            assert_eq!(fs::read(dst.join("f")).unwrap(), b"new");
            fs::remove_dir_all(&root).unwrap();
        }
    }

    /// A sync that fails: the temporary file goes, the old file stays, and Retry writes
    /// the file again from its source instead of syncing the same one twice.
    #[test]
    fn a_failed_sync_keeps_the_old_file_and_retry_copies_again() {
        let root = tmp("sync-fail");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("f"), b"new").unwrap();
        fs::write(dst.join("f"), b"old").unwrap();
        let frag = root.to_string_lossy().into_owned();
        let _f = crate::faults::fail(
            crate::faults::Point::SyncData,
            &frag,
            1,
            crate::faults::Fault::Os(5),
        );
        let (s, asked) = run_answering(
            Request::copy(vec![root.join("f")], dst.clone()),
            |q| match q {
                Question::Failed { .. } => Answer::Skip,
                _ => Answer::Yes,
            },
        );
        assert!(asked.iter().any(|q| q.starts_with("Failed")), "{asked:?}");
        assert_eq!((s.done, s.failed), (0, 1));
        assert_eq!(fs::read(dst.join("f")).unwrap(), b"old");
        assert_eq!(
            fs::read_dir(&dst).unwrap().count(),
            1,
            "no temporary file left"
        );
        drop(_f);
        let _f = crate::faults::fail(
            crate::faults::Point::SyncData,
            &frag,
            1,
            crate::faults::Fault::Os(5),
        );
        crate::faults::trace(&frag);
        let (s, _) = run_answering(
            Request::copy(vec![root.join("f")], dst.clone()),
            |q| match q {
                Question::Failed { .. } => Answer::Retry,
                _ => Answer::Yes,
            },
        );
        let seen = points(&frag);
        assert_eq!(s.done, 1, "{s:?}");
        let order: Vec<&str> = seen
            .iter()
            .copied()
            .filter(|p| matches!(*p, "write" | "sync-data" | "rename"))
            .collect();
        assert_eq!(
            order,
            ["write", "sync-data", "write", "sync-data", "rename"],
            "written again, never synced twice"
        );
        assert_eq!(fs::read(dst.join("f")).unwrap(), b"new");
        assert_eq!(fs::read_dir(&dst).unwrap().count(), 1);
        fs::remove_dir_all(&root).unwrap();
    }

    /// Replacing with a link: making the link fails, and the old file is still there.
    /// Before Round 029 the old file was removed first and lost.
    #[cfg(unix)]
    #[test]
    fn a_link_that_cannot_be_made_does_not_lose_the_file_it_was_to_replace() {
        let root = tmp("link-replace");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        std::os::unix::fs::symlink("target", root.join("l")).unwrap();
        fs::write(dst.join("l"), b"the only copy").unwrap();
        let _f = crate::faults::fail(
            crate::faults::Point::Write,
            &dst.join("l").to_string_lossy(),
            1,
            crate::faults::Fault::Os(28),
        );
        let (s, _) = run_with(
            Request::copy(vec![root.join("l")], dst.clone()),
            Answer::Yes,
        );
        assert_eq!(s.failed, 1, "{s:?}");
        assert_eq!(fs::read(dst.join("l")).unwrap(), b"the only copy");
        drop(_f);
        let (s, _) = run_with(
            Request::copy(vec![root.join("l")], dst.clone()),
            Answer::Yes,
        );
        assert_eq!(s.done, 1, "{s:?}");
        assert_eq!(fs::read_link(dst.join("l")).unwrap(), Path::new("target"));
        assert_eq!(
            fs::read_dir(&dst).unwrap().count(),
            1,
            "no temporary link left"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// A move across devices, per file: the copy's data synced, its directory synced,
    /// the journal's `moved` line synced, and only then the source removed. Before Round
    /// 029 the source was removed right after an unsynced copy.
    #[test]
    fn a_move_across_devices_removes_a_source_only_once_its_copy_is_on_disk() {
        journals();
        let Some(other) = other_device("move-order") else {
            return;
        };
        let root = tmp("move-order");
        let tag = format!("xmvd-{}", std::process::id());
        let dir = root.join(&tag);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("f"), b"payload").unwrap();
        crate::faults::trace(&tag);
        let mut req = Request::mv(vec![dir.clone()], other.clone());
        req.safe = true;
        let (s, _) = run_with(req, Answer::Yes);
        let seen = crate::faults::traced(&tag);
        assert_eq!(s.failed, 0, "{s:?}");
        assert_eq!(fs::read(other.join(&tag).join("f")).unwrap(), b"payload");
        assert!(!dir.exists(), "moved in full");
        let at = |point: &str, file: bool| {
            seen.iter()
                .position(|(p, path)| p.name() == point && (path.ends_with("f") || !file))
                .unwrap_or_else(|| panic!("no {point} in {seen:?}"))
        };
        let gone = at("remove-source", true);
        for before in ["sync-data", "sync-dir", "journal-sync"] {
            assert!(
                at(before, false) < gone,
                "{before} before the source goes: {seen:?}"
            );
        }
        let synced_dir = seen
            .iter()
            .position(|(p, path)| p.name() == "sync-dir" && path.ends_with(&tag))
            .unwrap_or_else(|| panic!("the copy's directory is synced: {seen:?}"));
        assert!(synced_dir < gone);
        fs::remove_dir_all(&other).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// The copy's sync fails: the source stays, and the copy, which may not be what was
    /// read, is removed.
    #[test]
    fn a_move_across_devices_whose_copy_cannot_be_synced_keeps_the_source() {
        let Some(other) = other_device("move-sync-fail") else {
            return;
        };
        let root = tmp("move-sync-fail");
        fs::write(root.join("only.txt"), b"the only copy").unwrap();
        let _f = crate::faults::fail(
            crate::faults::Point::SyncData,
            &other.to_string_lossy(),
            1,
            crate::faults::Fault::Os(5),
        );
        let (s, _) = run_with(
            Request::mv(vec![root.join("only.txt")], other.clone()),
            Answer::Yes,
        );
        assert_eq!((s.done, s.failed), (0, 1), "{s:?}");
        assert_eq!(fs::read(root.join("only.txt")).unwrap(), b"the only copy");
        assert_eq!(
            fs::read_dir(&other).unwrap().count(),
            0,
            "no copy, no temporary file"
        );
        fs::remove_dir_all(&other).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// Whether this process can write to a file whatever its mode (root on Unix): a test
    /// about a read-only file then proves nothing, and says it is skipped.
    fn writes_anything(tag: &str) -> bool {
        #[cfg(unix)]
        if rustix::process::geteuid().is_root() {
            eprintln!("{tag}: skipped, root writes a read-only file; run it as another user");
            return true;
        }
        let _ = tag;
        false
    }

    /// A read-only file moved across devices: its copy is read-only as soon as it is
    /// written (the mode is copied), so syncing it through a handle opened afresh for
    /// writing failed (EACCES), the move failed, and on Windows the copy was left. The
    /// handle that wrote it is the one synced.
    #[test]
    fn a_read_only_file_moves_across_devices() {
        if writes_anything("read-only-move") {
            return;
        }
        let Some(other) = other_device("read-only-move") else {
            return;
        };
        let root = tmp("read-only-move");
        let f = root.join("ro.txt");
        fs::write(&f, b"read only").unwrap();
        let mut perm = fs::metadata(&f).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&f, perm).unwrap();
        let (s, _) = run_with(Request::mv(vec![f.clone()], other.clone()), Answer::Yes);
        assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
        assert!(!f.exists(), "the source is gone");
        assert_eq!(fs::read(other.join("ro.txt")).unwrap(), b"read only");
        assert!(fs::metadata(other.join("ro.txt"))
            .unwrap()
            .permissions()
            .readonly());
        let mut perm = fs::metadata(other.join("ro.txt")).unwrap().permissions();
        // Cleaning up: writable again so the directory can go (Windows).
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = fs::set_permissions(other.join("ro.txt"), perm);
        let _ = fs::remove_dir_all(&other);
        let _ = fs::remove_dir_all(&root);
    }

    /// A write error that surfaces only at the sync (NFS, SMB: at close or at the sync
    /// of the handle that wrote) keeps that file's source and removes its copy; the rest
    /// of the batch moves.
    #[test]
    fn a_copy_whose_sync_fails_keeps_its_source_and_the_rest_moves() {
        let Some(other) = other_device("sync-one-fails") else {
            return;
        };
        let root = tmp("sync-one-fails");
        for n in ["a", "b", "c"] {
            fs::write(root.join(n), n).unwrap();
        }
        let _f = crate::faults::fail(
            crate::faults::Point::SyncData,
            &other.join("b").to_string_lossy(),
            1,
            crate::faults::Fault::Os(5),
        );
        let srcs = vec![root.join("a"), root.join("b"), root.join("c")];
        let mut req = Request::mv(srcs, other.clone());
        req.existing = Existing::Ask;
        let (s, _) = run_with(req, Answer::YesToAll);
        assert_eq!((s.done, s.failed), (2, 1), "{s:?}");
        assert_eq!(fs::read(root.join("b")).unwrap(), b"b", "b's source stays");
        assert!(!other.join("b").exists(), "b's copy goes");
        assert!(!root.join("a").exists() && !root.join("c").exists());
        let _ = fs::remove_dir_all(&other);
        let _ = fs::remove_dir_all(&root);
    }

    /// Stop at the journal's question means nothing more that cannot be undone: a source
    /// whose copy was waiting in the batch is kept. Before, the batch was still flushed
    /// once the journal was gone (no journal: nothing to wait for), and its sources went.
    #[test]
    fn stop_at_the_journal_question_keeps_the_sources_of_a_move() {
        journals();
        let Some(other) = other_device("stop-keeps") else {
            return;
        };
        let root = tmp("stop-keeps");
        let tag = format!("stopkeep-{}", std::process::id());
        fs::create_dir_all(root.join(&tag)).unwrap();
        fs::create_dir_all(other.join(&tag)).unwrap();
        for n in ["a", "b"] {
            fs::write(root.join(&tag).join(n), b"ours").unwrap();
            fs::write(other.join(&tag).join(n), b"theirs").unwrap();
        }
        // The job's start (1), the move onto the existing directory (2), the first
        // replacement (3), the second (4) fails.
        let _f = crate::faults::fail(
            crate::faults::Point::JournalSync,
            &tag,
            4,
            crate::faults::Fault::Os(28),
        );
        let mut req = Request::mv(vec![root.join(&tag)], other.clone());
        req.safe = true;
        let (s, asked) = run_answering(req, |q| match q {
            Question::NoJournal { .. } => Answer::Abort,
            _ => Answer::YesToAll,
        });
        assert!(
            asked.iter().any(|q| q.starts_with("NoJournal")),
            "{asked:?}"
        );
        for n in ["a", "b"] {
            assert!(
                root.join(&tag).join(n).exists(),
                "{n}'s source is kept: {s:?}"
            );
        }
        let _ = fs::remove_dir_all(&other);
        let _ = fs::remove_dir_all(&root);
    }

    /// Every file of a move across devices, across batch boundaries (64, 65, 128, 129…):
    /// its copy's data, then its directory, then the journal, are synced after it was
    /// written and before ITS source goes. An off-by-one at a boundary shows here.
    #[test]
    fn every_file_of_a_batched_move_is_on_disk_before_its_own_source_goes() {
        journals();
        let Some(other) = other_device("batch-order") else {
            return;
        };
        let root = tmp("batch-order");
        let tag = format!("batchord-{}", std::process::id());
        let dir = root.join(&tag);
        fs::create_dir_all(&dir).unwrap();
        let n = 2 * MOVE_BATCH_FILES + 2;
        for i in 0..n {
            fs::write(dir.join(format!("f{i:03}")), b"x").unwrap();
        }
        crate::faults::trace(&tag);
        let mut req = Request::mv(vec![dir.clone()], other.clone());
        req.safe = true;
        let (s, _) = run_with(req, Answer::Yes);
        let seen = crate::faults::traced(&tag);
        assert_eq!(s.failed, 0, "{s:?}");
        let to_dir = other.join(&tag);
        for i in 0..n {
            let name = format!("f{i:03}");
            let at = |point: &str, path: &Path| {
                seen.iter()
                    .position(|(p, q)| p.name() == point && q == path)
                    .unwrap_or_else(|| panic!("{name}: no {point} for {}", path.display()))
            };
            let gone = at("remove-source", &dir.join(&name));
            let data = at("sync-data", &to_dir.join(&name));
            assert!(data < gone, "{name}: its data before its source goes");
            let between = |point: &str| {
                seen[data..gone]
                    .iter()
                    .any(|(p, q)| p.name() == point && (point != "sync-dir" || q == &to_dir))
            };
            assert!(
                between("sync-dir"),
                "{name}: its directory synced after its data, before its source goes"
            );
            assert!(
                between("journal-sync"),
                "{name}: the journal synced after its data, before its source goes"
            );
        }
        let _ = fs::remove_dir_all(&other);
        let _ = fs::remove_dir_all(&root);
    }

    /// The cost of the sync rules on a move across devices, measured: `n` files of
    /// `size` bytes moved with every sync made and again with the seam leaving them out.
    /// `XDEVCMD_MEASURE=<n>x<size>[,…]`, from where `XDEVCMD_MEASURE_FROM` says (default:
    /// the temporary directory) to the other device, or back with `XDEVCMD_MEASURE_BACK`.
    /// Run by hand: `cargo test -p xdevcmd-core --release -- --ignored --nocapture measure`.
    #[test]
    #[ignore = "a measurement; run by hand (Round 029, step 3)"]
    fn measure_a_move_across_devices_with_and_without_syncs() {
        journals();
        let Some(other) = other_device("measure") else {
            return;
        };
        let spec =
            std::env::var("XDEVCMD_MEASURE").unwrap_or_else(|_| "10000x1024,3x104857600".into());
        let back = std::env::var_os("XDEVCMD_MEASURE_BACK").is_some();
        for case in spec.split(',') {
            let Some((n, size)) = case.split_once('x') else {
                continue;
            };
            let (n, size): (usize, u64) = (n.parse().unwrap(), size.parse().unwrap());
            for synced in [true, false] {
                let here = tmp("measure");
                let (from, to) = if back {
                    (other.clone(), here.clone())
                } else {
                    (here.clone(), other.clone())
                };
                let src = from.join(format!("m-{n}-{size}-{synced}"));
                fs::create_dir_all(&src).unwrap();
                let chunk = vec![0x5au8; size as usize];
                for i in 0..n {
                    fs::write(src.join(format!("f{i}")), &chunk).unwrap();
                }
                let frag = src.file_name().unwrap().to_string_lossy().into_owned();
                let _skips: Vec<_> = if synced {
                    Vec::new()
                } else {
                    [
                        crate::faults::Point::SyncData,
                        crate::faults::Point::SyncDir,
                        crate::faults::Point::JournalSync,
                    ]
                    .into_iter()
                    .map(|p| crate::faults::set(p, &frag, crate::faults::Action::Skip))
                    .collect()
                };
                // What was written to make the files is not the move's to flush.
                let _ = std::process::Command::new("sync").status();
                let mut req = Request::mv(vec![src.clone()], to.clone());
                req.safe = true;
                let t = std::time::Instant::now();
                let (s, _) = run_with(req, Answer::Yes);
                let took = t.elapsed();
                assert_eq!(s.failed, 0, "{s:?}");
                eprintln!(
                    "MEASURE {n} x {size} B {} {}: {:.3} s",
                    if back { "back" } else { "out" },
                    if synced { "synced" } else { "no sync" },
                    took.as_secs_f64()
                );
                let _ = fs::remove_dir_all(to.join(src.file_name().unwrap()));
                let _ = fs::remove_dir_all(&here);
            }
        }
        let _ = fs::remove_dir_all(&other);
    }

    #[test]
    fn a_replacement_cancelled_part_way_keeps_the_old_file() {
        let root = tmp("replace-cancel");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("f"), vec![7u8; 1 << 20]).unwrap();
        fs::write(dst.join("f"), b"old").unwrap();
        let h = start(Request::copy(vec![root.join("f")], dst.clone()));
        loop {
            match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
                // Agree to overwrite, and cancel at once: the copy stops before it is
                // complete.
                Event::Ask(_) => {
                    h.cancel();
                    h.answer(Answer::Yes);
                }
                Event::Done(_) => break,
                _ => {}
            }
        }
        assert_eq!(
            fs::read(dst.join("f")).unwrap(),
            b"old",
            "the old file is intact"
        );
        let left: Vec<_> = fs::read_dir(&dst)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 1, "no temporary file left behind: {left:?}");
        // Not cancelled, the replacement lands whole.
        let (s, _) = run_with(
            Request::copy(vec![root.join("f")], dst.clone()),
            Answer::Yes,
        );
        assert_eq!(s.done, 1);
        assert_eq!(fs::read(dst.join("f")).unwrap().len(), 1 << 20);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn overwrite_all_does_not_also_agree_to_copy_then_delete() {
        let Some(other) = other_device("split") else {
            return;
        };
        let root = tmp("split-all");
        fs::write(root.join("a"), b"A").unwrap();
        fs::write(root.join("b"), b"B").unwrap();
        fs::write(other.join("a"), b"theirs").unwrap();
        // "Overwrite all" at a's question; b's copy-then-delete is still asked about.
        let (_, asked) = run_answering(
            Request::mv(vec![root.join("a"), root.join("b")], other.clone()),
            |q| match q {
                Question::Conflict { .. } => Answer::YesToAll,
                _ => Answer::Yes,
            },
        );
        let cross = asked
            .iter()
            .filter(|q| q.starts_with("CrossDevice"))
            .count();
        assert_eq!(cross, 2, "{asked:?}");
        fs::remove_dir_all(&other).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_sync_replaces_only_what_its_plan_named() {
        let root = tmp("sync-planned");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("a"), b"new a").unwrap();
        fs::write(root.join("b"), b"new b").unwrap();
        fs::write(dst.join("a"), b"old a").unwrap();
        // a is a planned overwrite; b was free, and a file takes its name meanwhile.
        APPEAR.lock().unwrap().push(dst.join("b"));
        let (_, asked) = run_with(
            Request::sync(vec![
                (
                    root.join("a"),
                    dst.clone(),
                    Expect::Replace(Ident::at(&dst.join("a")).unwrap()),
                ),
                (root.join("b"), dst.clone(), Expect::Absent),
            ]),
            Answer::Skip,
        );
        assert_eq!(
            asked, 1,
            "only b, which the plan did not name, is asked about"
        );
        assert_eq!(fs::read(dst.join("a")).unwrap(), b"new a");
        assert_eq!(fs::read(dst.join("b")).unwrap(), b"theirs");
        fs::remove_dir_all(&root).unwrap();
    }

    /// The fault test (B05): the user says Yes to replacing a file, and another program
    /// replaces that file while the copy is written. Asked again, saying so; Skip keeps
    /// the newcomer. Before Round 029 it was overwritten unasked.
    #[test]
    fn a_file_replaced_after_the_user_agreed_is_asked_about_again() {
        let root = tmp("swap-after-yes");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("f"), b"ours").unwrap();
        fs::write(dst.join("f"), b"the file the user saw").unwrap();
        // At the replacement's sync: after the answer, before the rename.
        let _g = crate::faults::set(
            crate::faults::Point::SyncData,
            &root.to_string_lossy(),
            crate::faults::Action::Swap {
                target: dst.join("f"),
                body: b"another program's file".to_vec(),
            },
        );
        let (s, asked) = run_answering(Request::copy(vec![root.join("f")], dst.clone()), |q| {
            if format!("{q:?}").contains("changed: true") {
                Answer::Skip
            } else {
                Answer::Yes
            }
        });
        assert_eq!(asked.len(), 2, "asked again: {asked:?}");
        assert!(asked[1].contains("changed: true"), "{asked:?}");
        assert_eq!(
            fs::read(dst.join("f")).unwrap(),
            b"another program's file",
            "{s:?}"
        );
        assert_eq!(
            fs::read_dir(&dst).unwrap().count(),
            1,
            "no temporary file left"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// The re-check is the last thing before the rename: the journal's question can be
    /// open for minutes, and a file replaced while it is was overwritten on a check made
    /// before it. Asked again instead.
    #[test]
    fn a_file_replaced_while_the_journal_question_is_open_is_asked_about_again() {
        journals();
        let root = tmp("swap-during-nojournal");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("sq-a1.txt"), b"ours").unwrap();
        fs::write(dst.join("sq-a1.txt"), b"seen").unwrap();
        let _f = crate::faults::fail(
            crate::faults::Point::JournalLine,
            ".sq-a1.txt.xdevcmd-part",
            1,
            crate::faults::Fault::Os(28),
        );
        let target = dst.join("sq-a1.txt");
        let mut req = Request::copy(vec![root.join("sq-a1.txt")], dst.clone());
        req.safe = true;
        let (_, asked) = run_answering(req, |q| {
            let q = format!("{q:?}");
            if q.starts_with("NoJournal") {
                // Another program replaces it while the question is open.
                fs::remove_file(&target).unwrap();
                fs::write(&target, b"replaced while asked, longer").unwrap();
                Answer::Yes
            } else if q.contains("changed: true") {
                Answer::Skip
            } else {
                Answer::Yes
            }
        });
        assert!(
            asked.iter().any(|q| q.contains("changed: true")),
            "{asked:?}"
        );
        assert_eq!(fs::read(&target).unwrap(), b"replaced while asked, longer");
        fs::remove_dir_all(&root).unwrap();
    }

    /// The same on a move within one file system, whose replace is a rename.
    #[test]
    fn a_move_onto_a_file_replaced_after_the_user_agreed_asks_again() {
        let root = tmp("swap-move");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("m"), b"ours").unwrap();
        fs::write(dst.join("m"), b"seen").unwrap();
        let target = dst.join("m");
        let mut first = true;
        let h = start(Request::mv(vec![root.join("m")], dst.clone()));
        let mut asked = Vec::new();
        loop {
            match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
                Event::Ask(q) => {
                    asked.push(format!("{q:?}"));
                    if first {
                        // Another program replaces it as the user answers.
                        fs::remove_file(&target).unwrap();
                        fs::write(&target, b"replaced meanwhile, longer").unwrap();
                        first = false;
                        h.answer(Answer::Yes);
                    } else {
                        h.answer(Answer::Skip);
                    }
                }
                Event::Done(_) => break,
                _ => {}
            }
        }
        assert_eq!(asked.len(), 2, "{asked:?}");
        assert_eq!(fs::read(&target).unwrap(), b"replaced meanwhile, longer");
        assert_eq!(
            fs::read(root.join("m")).unwrap(),
            b"ours",
            "the source stays"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// A file made, after Alt+Y's compare, where the plan saw nothing: asked about, not
    /// overwritten. Before Round 029 the job took every destination that existed when
    /// it started for one the plan named.
    #[test]
    fn a_file_made_where_the_plan_saw_none_is_asked_about() {
        let root = tmp("sync-absent");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("only-left"), b"left").unwrap();
        let req = Request::sync(vec![(root.join("only-left"), dst.clone(), Expect::Absent)]);
        // Between the compare and Enter.
        fs::write(dst.join("only-left"), b"made since").unwrap();
        let (_, asked) = run_with(req, Answer::Skip);
        assert_eq!(asked, 1);
        assert_eq!(fs::read(dst.join("only-left")).unwrap(), b"made since");
        // And a planned replacement whose file changed since the compare: asked too.
        let seen = Ident::at(&dst.join("only-left")).unwrap();
        fs::remove_file(dst.join("only-left")).unwrap();
        fs::write(dst.join("only-left"), b"changed since the plan").unwrap();
        let req = Request::sync(vec![(
            root.join("only-left"),
            dst.clone(),
            Expect::Replace(seen),
        )]);
        let (_, asked) = run_with(req, Answer::Skip);
        assert_eq!(asked, 1);
        assert_eq!(
            fs::read(dst.join("only-left")).unwrap(),
            b"changed since the plan"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// Set `p`'s modification time to `secs` after the epoch.
    fn age(p: &Path, secs: u64) {
        let t = std::time::UNIX_EPOCH + Duration::from_secs(secs);
        File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_times(FileTimes::new().set_modified(t))
            .unwrap();
    }

    #[test]
    fn update_replaces_only_what_is_older_and_skips_the_same_and_newer() {
        let root = tmp("update");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        // older: the copy there is older; same: equal times (within 2 s); newer: the
        // copy there is newer; fresh: not there at all.
        for (name, ours, theirs) in [
            ("older", 2_000_000, 1_000_000),
            ("same", 1_000_000, 1_000_001),
            ("newer", 1_000_000, 2_000_000),
        ] {
            fs::write(root.join(name), b"ours").unwrap();
            age(&root.join(name), ours);
            fs::write(dst.join(name), b"theirs").unwrap();
            age(&dst.join(name), theirs);
        }
        fs::write(root.join("fresh"), b"ours").unwrap();
        let names = ["older", "same", "newer", "fresh"];
        let mut req = Request::copy(names.iter().map(|n| root.join(n)).collect(), dst.clone());
        req.existing = Existing::Update;
        let (s, asked) = run_with(req, Answer::Abort);
        assert_eq!(asked, 0, "update never asks");
        assert_eq!((s.done, s.skipped), (2, 2));
        assert_eq!(fs::read(dst.join("older")).unwrap(), b"ours");
        assert_eq!(fs::read(dst.join("same")).unwrap(), b"theirs");
        assert_eq!(fs::read(dst.join("newer")).unwrap(), b"theirs");
        assert_eq!(fs::read(dst.join("fresh")).unwrap(), b"ours");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resuming_skips_what_is_identical_and_asks_about_the_rest() {
        let root = tmp("resume");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        // done: finished by the first run (same size, same date). kept: the user chose
        // to keep their older, different file in the first run; Update would replace
        // it, its source being newer. fresh: not there yet.
        fs::write(root.join("done"), b"ours").unwrap();
        age(&root.join("done"), 1_000_000);
        fs::write(dst.join("done"), b"ours").unwrap();
        age(&dst.join("done"), 1_000_001);
        fs::write(root.join("kept"), b"new version").unwrap();
        age(&root.join("kept"), 2_000_000);
        fs::write(dst.join("kept"), b"theirs").unwrap();
        age(&dst.join("kept"), 1_000_000);
        fs::write(root.join("fresh"), b"ours").unwrap();
        let names = ["done", "kept", "fresh"];
        let mut req = Request::copy(names.iter().map(|n| root.join(n)).collect(), dst.clone());
        req.existing = Existing::Resume;
        let (s, asked) = run_with(req, Answer::Skip);
        assert_eq!(
            asked, 1,
            "only kept is asked about; done is skipped silently"
        );
        assert_eq!(fs::read(dst.join("kept")).unwrap(), b"theirs", "kept stays");
        assert_eq!(fs::read(dst.join("fresh")).unwrap(), b"ours");
        assert_eq!((s.done, s.skipped, s.left), (1, 2, 0));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resuming_a_move_leaves_a_source_whose_copy_is_already_there_and_says_so() {
        // A move across drives killed after its copy and before removing the source.
        let root = tmp("resume-move");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("a"), b"ours").unwrap();
        age(&root.join("a"), 1_000_000);
        fs::write(dst.join("a"), b"ours").unwrap();
        age(&dst.join("a"), 1_000_000);
        let mut req = Request::mv(vec![root.join("a")], dst.clone());
        req.existing = Existing::Resume;
        let (s, asked) = run_with(req, Answer::Abort);
        assert_eq!(asked, 0);
        assert!(
            root.join("a").exists(),
            "the source is never removed unasked"
        );
        assert_eq!(s.left, 1);
        assert!(
            s.describe(Kind::Move)
                .contains("1 left where it was: already there"),
            "{}",
            s.describe(Kind::Move)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn answering_update_at_a_question_applies_it_to_the_rest() {
        let root = tmp("update-answer");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        for (name, ours, theirs) in [("a", 1_000_000, 2_000_000), ("b", 2_000_000, 1_000_000)] {
            fs::write(root.join(name), b"ours").unwrap();
            age(&root.join(name), ours);
            fs::write(dst.join(name), b"theirs").unwrap();
            age(&dst.join(name), theirs);
        }
        let req = Request::copy(vec![root.join("a"), root.join("b")], dst.clone());
        let (s, asked) = run_with(req, Answer::Update);
        assert_eq!(asked, 1, "asked once, then the rule decides");
        assert_eq!(
            fs::read(dst.join("a")).unwrap(),
            b"theirs",
            "newer there: kept"
        );
        assert_eq!(
            fs::read(dst.join("b")).unwrap(),
            b"ours",
            "older there: replaced"
        );
        assert_eq!((s.done, s.skipped), (1, 1));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_summary_says_when_its_job_ran_without_a_journal() {
        let mut s = Summary {
            done: 1,
            ..Summary::default()
        };
        assert_eq!(s.describe(Kind::Copy), "1 copied");
        s.journal_off = Some("permission denied".into());
        assert_eq!(
            s.describe(Kind::Copy),
            "1 copied; safe operations off for this job: permission denied"
        );
    }

    #[test]
    fn with_safe_operations_a_cancelled_new_copy_leaves_nothing_under_any_name() {
        let root = tmp("safe-cancel");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        // Big enough that the cancel lands before the copy ends even on a loaded machine:
        // with 8 MiB the rest was sometimes copied before the test thread ran, and a
        // copy that completes is rightly kept. Sparse, so it costs nothing to make.
        let big = File::create(root.join("big")).unwrap();
        big.set_len(1 << 30).unwrap();
        drop(big);
        let mut req = Request::copy(vec![root.join("big")], dst.clone());
        req.safe = true;
        let h = start(req);
        loop {
            match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
                // Stop it as soon as it starts writing.
                Event::Progress { .. } => h.cancel(),
                Event::Done(_) => break,
                _ => {}
            }
        }
        let left: Vec<_> = fs::read_dir(&dst)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            left.is_empty(),
            "nothing under the real name or a temporary one: {left:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn two_jobs_at_once_each_write_the_way_their_request_says() {
        let root = tmp("safe-per-job");
        let (on, off) = (root.join("on"), root.join("off"));
        fs::create_dir_all(&on).unwrap();
        fs::create_dir_all(&off).unwrap();
        let src: Vec<PathBuf> = (0..20)
            .map(|i| {
                let p = root.join(format!("f{i}"));
                fs::write(&p, vec![b'x'; 64 << 10]).unwrap();
                p
            })
            .collect();
        let mut a = Request::copy(src.clone(), on.clone());
        a.safe = true;
        let mut b = Request::copy(src, off.clone());
        b.safe = false;
        let (ha, hb) = (start(a), start(b));
        for h in [ha, hb] {
            while !matches!(
                h.events.recv_timeout(Duration::from_secs(20)).unwrap(),
                Event::Done(_)
            ) {}
        }
        let written = WRITING.lock().unwrap().clone();
        let names = |dir: &Path| -> Vec<String> {
            written
                .iter()
                .filter(|p| p.parent() == Some(dir))
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        let (safe_names, plain_names) = (names(&on), names(&off));
        assert_eq!((safe_names.len(), plain_names.len()), (20, 20));
        assert!(
            safe_names.iter().all(|n| n.contains(".xdevcmd-part-")),
            "the safe job writes every file under a temporary name: {safe_names:?}"
        );
        assert!(
            plain_names.iter().all(|n| !n.contains(".xdevcmd-part-")),
            "the other writes in place: {plain_names:?}"
        );
        assert_eq!(fs::read_dir(&on).unwrap().count(), 20);
        assert_eq!(fs::read_dir(&off).unwrap().count(), 20);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_move_on_one_filesystem_removes_the_source() {
        let root = tmp("move");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let f = root.join("f.txt");
        fs::write(&f, b"payload").unwrap();

        let s = run(Request::mv(vec![f.clone()], dst.clone()));
        assert_eq!(s.failed, 0, "{:?}", s.errors);
        assert!(!f.exists(), "the source must be gone after a move");
        assert_eq!(fs::read(dst.join("f.txt")).unwrap(), b"payload");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn delete_removes_a_whole_tree() {
        let root = tmp("delete");
        let victim = root.join("victim");
        fs::create_dir_all(victim.join("deep")).unwrap();
        fs::write(victim.join("deep/x"), b"x").unwrap();

        let s = run(Request::delete(vec![victim.clone()], false));
        assert_eq!(s.done, 1);
        assert!(!victim.exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn trash_moves_the_file_and_records_how_to_restore_it() {
        let root = tmp("trash");
        let trash_root = root.join("Trash");
        let f = root.join("note.txt");
        fs::write(&f, b"keep me").unwrap();

        let mut req = Request::delete(vec![f.clone()], true);
        req.trash_root = Some(trash_root.clone());
        let s = run(req);
        assert_eq!(s.done, 1, "{:?}", s.errors);

        assert!(!f.exists(), "the original is gone from where it was");
        assert_eq!(
            fs::read(trash_root.join("files/note.txt")).unwrap(),
            b"keep me"
        );

        let info = fs::read_to_string(trash_root.join("info/note.txt.trashinfo")).unwrap();
        assert!(info.starts_with("[Trash Info]\n"), "{info}");
        // The XDG trash is the Linux one; the absolute-path form is Unix's. Elsewhere
        // this code only runs here, in its test.
        #[cfg(unix)]
        assert!(
            info.contains("Path=/"),
            "the original path must be recorded\n{info}"
        );
        assert!(info.contains("note.txt"), "{info}");
        assert!(info.contains("DeletionDate="), "{info}");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn what_went_to_the_trash_comes_back_and_only_while_it_can() {
        let root = tmp("trash-back");
        let trash_root = root.join("Trash");
        let f = root.join("note.txt");
        fs::write(&f, b"keep me").unwrap();
        let mut req = Request::delete(vec![f.clone()], true);
        req.trash_root = Some(trash_root.clone());
        let s = run(req);
        let [t] = s.trashed.as_slice() else {
            panic!("one item trashed: {s:?}");
        };
        assert_eq!(t.original, f);
        // Its old name taken meanwhile: refused, nothing replaced.
        fs::write(&f, b"newer").unwrap();
        assert!(crate::trashed::check(t)
            .unwrap_err()
            .contains("exists again"));
        assert!(crate::trashed::restore(t).is_err());
        assert_eq!(fs::read(&f).unwrap(), b"newer");
        // Free again: back where it was, its record gone from the trash.
        fs::remove_file(&f).unwrap();
        crate::trashed::restore(t).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"keep me");
        assert!(!trash_root.join("info/note.txt.trashinfo").exists());
        assert!(!trash_root.join("files/note.txt").exists());
        // Gone from the trash: undo says so.
        assert!(crate::trashed::check(t).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_trashed_link_records_its_own_path_not_its_target() {
        let root = tmp("trash-link");
        let trash_root = root.join("Trash");
        fs::create_dir_all(root.join("elsewhere")).unwrap();
        fs::write(root.join("elsewhere/target.txt"), b"t").unwrap();
        let link = root.join("link.txt");
        std::os::unix::fs::symlink(root.join("elsewhere/target.txt"), &link).unwrap();
        let mut req = Request::delete(vec![link.clone()], true);
        req.trash_root = Some(trash_root.clone());
        run(req);
        let info = fs::read_to_string(trash_root.join("info/link.txt.trashinfo")).unwrap();
        assert!(info.contains("link.txt"), "{info}");
        assert!(!info.contains("target.txt"), "{info}");
        assert!(
            root.join("elsewhere/target.txt").exists(),
            "the target is untouched"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_on_another_volume_goes_to_that_volumes_trash_and_back() {
        let Some(other) = other_device("vol-trash") else {
            return;
        };
        let f = other.join("on-the-other-volume.txt");
        fs::write(&f, b"v").unwrap();
        let t = send_to_trash(&f, None).unwrap();
        let crate::trashed::Place::Freedesktop { file, info } = &t.place else {
            panic!("{t:?}");
        };
        let uid = rustix::process::getuid().as_raw();
        // The top of that volume: /dev/shm, or what XDEVCMD_TEST_OTHER_DEV names.
        let top = other.parent().unwrap().to_path_buf();
        assert!(
            file.starts_with(top.join(format!(".Trash-{uid}/files"))),
            "{file:?}"
        );
        let record = fs::read_to_string(info).unwrap();
        // Relative to the volume's top, as the spec prefers for these.
        assert!(record.contains("Path=xdevcmd-xdev-vol-trash-"), "{record}");
        crate::trashed::restore(&t).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"v");
        fs::remove_dir_all(&other).unwrap();
        // Leave no trash directory behind if this test made it (only empty ones go).
        let trash = top.join(format!(".Trash-{uid}"));
        let _ = fs::remove_dir(trash.join("files"));
        let _ = fs::remove_dir(trash.join("info"));
        let _ = fs::remove_dir(&trash);
    }

    #[test]
    fn trashing_two_files_of_the_same_name_does_not_lose_one() {
        let root = tmp("trash-dup");
        let trash_root = root.join("Trash");
        for dir in ["a", "b"] {
            fs::create_dir_all(root.join(dir)).unwrap();
            fs::write(root.join(dir).join("same.txt"), dir.as_bytes()).unwrap();
        }
        for dir in ["a", "b"] {
            let mut req = Request::delete(vec![root.join(dir).join("same.txt")], true);
            req.trash_root = Some(trash_root.clone());
            assert_eq!(run(req).done, 1);
        }
        assert_eq!(fs::read(trash_root.join("files/same.txt")).unwrap(), b"a");
        assert_eq!(fs::read(trash_root.join("files/same.txt.1")).unwrap(), b"b");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn copy_as_puts_a_second_copy_beside_the_first_and_still_asks_before_overwriting() {
        let root = std::env::temp_dir().join(format!("xdevcmd-ops-copyas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("tree/sub")).unwrap();
        fs::write(root.join("a.txt"), "A").unwrap();
        fs::write(root.join("tree/sub/x"), "X").unwrap();
        let mut req = Request::copy(vec![root.join("a.txt")], root.clone());
        req.rename_to = Some("b.txt".into());
        assert_eq!(run(req.clone()).done, 1);
        assert_eq!(fs::read_to_string(root.join("b.txt")).unwrap(), "A");
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "A");
        // Again: b.txt exists now, so the worker asks; Skip leaves it.
        fs::write(root.join("b.txt"), "keep").unwrap();
        let (s, asked) = run_with(req, Answer::Skip);
        assert_eq!((s.skipped, asked), (1, 1));
        assert_eq!(fs::read_to_string(root.join("b.txt")).unwrap(), "keep");
        // A directory, recursively.
        let mut req = Request::copy(vec![root.join("tree")], root.clone());
        req.rename_to = Some("tree2".into());
        run(req);
        assert_eq!(fs::read_to_string(root.join("tree2/sub/x")).unwrap(), "X");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copying_a_directory_into_itself_is_refused() {
        let root = tmp("self-copy");
        let d = root.join("d");
        fs::create_dir_all(d.join("inner")).unwrap();
        fs::write(d.join("f"), b"f").unwrap();

        let s = run(Request::copy(vec![d.clone()], d.join("inner")));
        assert_eq!(s.failed, 1, "it must refuse rather than recurse forever");
        assert_eq!(s.done, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cancelling_mid_file_removes_the_half_written_copy() {
        let root = tmp("cancel");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let big = root.join("big.bin");
        // Sparse and large, so the cancel lands mid-copy even on a loaded machine.
        File::create(&big).unwrap().set_len(1 << 30).unwrap();

        let h = start(Request::copy(vec![big], dst.clone()));
        let mut cancelled_at_progress = false;
        let summary = loop {
            match h.events.recv_timeout(Duration::from_secs(20)) {
                Ok(Event::Progress { .. }) if !cancelled_at_progress => {
                    cancelled_at_progress = true;
                    h.cancel();
                }
                Ok(Event::Done(s)) => break s,
                Ok(_) => {}
                Err(e) => panic!("worker never finished: {e}"),
            }
        };

        assert!(
            cancelled_at_progress,
            "the file was too small to cancel mid-copy"
        );
        assert!(summary.cancelled, "the summary must say it was cancelled");
        assert_eq!(summary.done, 0, "a cancelled file is not a copied file");
        assert!(
            !dst.join("big.bin").exists(),
            "a partial copy must not be left looking complete"
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn copying_a_fifo_fails_fast_instead_of_hanging() {
        let root = tmp("fifo");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        let fifo = root.join("pipe");
        assert!(std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());

        // run() panics if the worker has not finished in 20 s; before the fix it never did.
        let s = run(Request::copy(vec![fifo], dst.clone()));
        assert_eq!(s.failed, 1);
        assert!(s.errors[0].contains("FIFO"), "{:?}", s.errors);
        assert!(!dst.join("pipe").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn yes_to_all_stops_asking() {
        let root = tmp("yes-all");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        for i in 0..4 {
            fs::write(root.join(format!("f{i}")), b"new").unwrap();
            fs::write(dst.join(format!("f{i}")), b"old").unwrap();
        }
        let sources: Vec<PathBuf> = (0..4).map(|i| root.join(format!("f{i}"))).collect();

        let (s, asked) = run_with(Request::copy(sources, dst.clone()), Answer::YesToAll);
        assert_eq!(asked, 1, "after 'all' it must not ask again");
        assert_eq!(s.done, 4);
        assert_eq!(fs::read(dst.join("f3")).unwrap(), b"new");

        fs::remove_dir_all(&root).unwrap();
    }
}
