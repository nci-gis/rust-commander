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
    /// Overwrites were listed in a plan the user confirmed, so they are not asked
    /// about one by one. Only a plan that names every overwrite may set this.
    pub overwrite_confirmed: bool,
    /// Copy and move: what to do with files already at the destination.
    pub existing: Existing,
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
    /// Copy each `(source, into)`, overwriting what the confirmed plan said it would.
    pub fn sync(pairs: Vec<(PathBuf, PathBuf)>) -> Self {
        Self {
            kind: Kind::Copy,
            pairs,
            overwrite_confirmed: true,
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
            overwrite_confirmed: false,
            existing: Existing::Ask,
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
            overwrite_confirmed: false,
            existing: Existing::Ask,
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
            overwrite_confirmed: false,
            existing: Existing::Ask,
            safe: safe(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// The destination already exists.
    Conflict {
        path: PathBuf,
        existing_bytes: u64,
        incoming_bytes: u64,
    },
    /// A move would cross a filesystem boundary, so it becomes a copy then a delete.
    CrossDevice { path: PathBuf },
    /// The trash is on another filesystem, so the only way to honour the key is to
    /// delete for real. Never assumed.
    TrashUnavailable { path: PathBuf, reason: String },
    /// Something went wrong and the user decides whether it is fatal.
    Failed { path: PathBuf, error: String },
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
            planned: None,
            abort: false,
            bytes_done: 0,
            last_report: 0,
            sum: Summary::default(),
            moving: false,
            approved: None,
            journal: None,
            safe: req.safe,
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
    /// Synchronising: the destinations the confirmed plan overwrites. Only these are
    /// replaced without asking; a file that appears elsewhere is asked about.
    planned: Option<std::collections::HashSet<PathBuf>>,
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
    /// Safe operations: what this job is in the middle of, for the next start if it is
    /// killed. Removed when the job ends.
    journal: Option<crate::journal::Journal>,
    /// [`Request::safe`]: decides the journal and whether a new file is written under
    /// a temporary name.
    safe: bool,
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
            self.journal =
                crate::journal::Journal::begin(kind, &sources, req.dest.as_deref(), req.to_trash);
        }
        if matches!(req.kind, Kind::Extract | Kind::Pack) {
            self.archive_job(&req);
            self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
            self.sum.bytes = self.bytes_done;
            let _ = self.tx.send(Event::Done(self.sum.clone()));
            return;
        }
        match req.existing {
            Existing::Ask => {}
            Existing::Update => self.update = true,
            Existing::Skip => self.skip_all = true,
            Existing::Replace => self.overwrite_all = true,
            Existing::Resume => self.resuming = true,
        }
        if req.overwrite_confirmed {
            self.planned = Some(
                req.pairs
                    .iter()
                    .filter_map(|(src, into)| Some(into.join(src.file_name()?)))
                    .filter(|dst| dst.symlink_metadata().is_ok())
                    .collect(),
            );
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
            self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
            self.sum.bytes = self.bytes_done;
            let _ = self.tx.send(Event::Done(self.sum.clone()));
            return;
        }
        let (files, bytes) = scan(&req.sources);
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
        }

        self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
        self.sum.bytes = self.bytes_done;
        let _ = self.tx.send(Event::Done(self.sum.clone()));
    }

    fn stopped(&self) -> bool {
        self.abort || self.cancel.load(Ordering::Relaxed)
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
        // without anyone able to stop us is the one thing we must not do. While it
        // waits, the journal keeps showing the job is alive (on Windows a journal left
        // untouched for two minutes counts as a dead run's).
        loop {
            match self
                .answers
                .recv_timeout(std::time::Duration::from_secs(10))
            {
                Ok(a) => return a,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(j) = self.journal.as_mut() {
                        j.touch();
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Answer::Abort,
            }
        }
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
        if dst.symlink_metadata().is_err() {
            return Some(true);
        }
        if self.approved.as_deref() == Some(dst) {
            self.approved = None;
            return Some(true);
        }
        if self.overwrite_all || self.planned.as_ref().is_some_and(|p| p.contains(dst)) {
            return Some(true);
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
        let existing = fs::symlink_metadata(dst).map(|m| m.len()).unwrap_or(0);
        match self.ask(Question::Conflict {
            path: dst.to_path_buf(),
            existing_bytes: existing,
            incoming_bytes: incoming,
        }) {
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
            self.copy_symlink(src, dst);
            self.moved(src, before);
        } else if meta.is_dir() {
            if let Err(e) = fs::create_dir_all(dst) {
                self.note_error(dst, &e);
                return;
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
            if self.moving && !self.stopped() {
                let _ = fs::remove_dir(src);
            }
        } else {
            let before = self.sum.done;
            self.copy_file(src, dst, meta.len());
            self.moved(src, before);
        }
    }

    /// While moving across devices: remove `src` if its copy just completed (the count
    /// of done items went up). A skipped or failed copy leaves it.
    fn moved(&mut self, src: &Path, done_before: u64) {
        if self.moving && self.sum.done > done_before {
            if let Err(e) = fs::remove_file(src) {
                self.note_error(src, &e);
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
                let _ = fs::remove_file(dst);
                match std::os::unix::fs::symlink(&target, dst) {
                    Ok(()) => self.sum.done += 1,
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
                    return;
                }
                // Taken since it was found free: round again, to the question.
                Err(e) if free && e.kind() == io::ErrorKind::AlreadyExists => continue,
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
        writing_in_test(&target);
        if !in_place {
            if let Some(j) = self.journal.as_mut() {
                j.part(&target);
            }
        }
        let mut w = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let written = self.write_all_from(&mut r, &mut w);
        let kept = match written {
            Ok(true) => {
                // Permissions and mtime are part of the file as far as a user is
                // concerned; a copy that loses the executable bit broke something.
                let _ = fs::set_permissions(&target, meta.permissions());
                if let Ok(mtime) = meta.modified() {
                    let _ = w.set_times(FileTimes::new().set_modified(mtime));
                }
                drop(w);
                if in_place {
                    Ok(true)
                } else if new_only {
                    // Taken only if still free: a file that appeared meanwhile is asked
                    // about (the caller goes round again on AlreadyExists).
                    crate::noreplace::rename_noreplace(&target, dst).map(|()| true)
                } else {
                    fs::rename(&target, dst).map(|()| true)
                }
            }
            other => other,
        };
        if !matches!(kept, Ok(true)) {
            let _ = fs::remove_file(&target);
        }
        if !in_place {
            if let Some(j) = self.journal.as_mut() {
                j.gone(&target);
            }
        }
        kept
    }

    /// Copy `r` into `w`; Ok(false) when cancelled part-way.
    fn write_all_from(&mut self, r: &mut File, w: &mut File) -> io::Result<bool> {
        let mut buf = vec![0u8; BUF];
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n])?;
            self.advance(n as u64);
            if let Some(j) = self.journal.as_mut() {
                j.touch();
            }
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
        if let Some(j) = self.journal.as_mut() {
            if made {
                j.part(p);
            } else {
                j.gone(p);
            }
        }
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

/// A temporary name beside `dst` to write a replacement under: hidden, and unique to
/// this process and call, so two jobs never share one.
pub(crate) fn part_name(dst: &Path) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = dst
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = dst.parent().unwrap_or(Path::new("."));
    dir.join(format!(".{name}.xdevcmd-part-{}-{n}", std::process::id()))
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
    /// across devices: tmpfs at /dev/shm on Linux. None where there is no such place.
    fn other_device(tag: &str) -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let shm = Path::new("/dev/shm");
            let tmp = fs::metadata(std::env::temp_dir()).ok()?;
            let other = fs::metadata(shm).ok()?;
            if other.dev() != tmp.dev() {
                let d = shm.join(format!("xdevcmd-xdev-{tag}-{}", std::process::id()));
                let _ = fs::remove_dir_all(&d);
                fs::create_dir_all(&d).ok()?;
                return Some(d);
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = tag;
        None
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
                (root.join("a"), dst.clone()),
                (root.join("b"), dst.clone()),
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
    fn with_safe_operations_a_cancelled_new_copy_leaves_nothing_under_any_name() {
        let root = tmp("safe-cancel");
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("big"), vec![1u8; 8 << 20]).unwrap();
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
        assert!(
            file.starts_with(format!("/dev/shm/.Trash-{uid}/files")),
            "{file:?}"
        );
        let record = fs::read_to_string(info).unwrap();
        // Relative to the volume's top, as the spec prefers for these.
        assert!(record.contains("Path=xdevcmd-xdev-vol-trash-"), "{record}");
        crate::trashed::restore(&t).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"v");
        fs::remove_dir_all(&other).unwrap();
        // Leave no trash directory behind if this test made it (only empty ones go).
        let trash = PathBuf::from(format!("/dev/shm/.Trash-{uid}"));
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
        fs::write(&big, vec![7u8; 24 * 1024 * 1024]).unwrap();

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
