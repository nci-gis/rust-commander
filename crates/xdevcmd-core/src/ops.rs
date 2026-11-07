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
            yes_to_all: false,
            skip_all: false,
            abort: false,
            bytes_done: 0,
            last_report: 0,
            sum: Summary::default(),
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
    yes_to_all: bool,
    skip_all: bool,
    abort: bool,
    bytes_done: u64,
    last_report: u64,
    sum: Summary,
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
        if matches!(req.kind, Kind::Extract | Kind::Pack) {
            self.archive_job(&req);
            self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
            self.sum.bytes = self.bytes_done;
            let _ = self.tx.send(Event::Done(self.sum.clone()));
            return;
        }
        self.yes_to_all = req.overwrite_confirmed;
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
                match self.resolve_conflict(&dest, bytes) {
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

    /// Decide what to do about an existing destination. `Ok(true)` means go ahead.
    fn resolve_conflict(&mut self, dst: &Path, incoming: u64) -> Option<bool> {
        if dst.symlink_metadata().is_err() {
            return Some(true);
        }
        if self.yes_to_all {
            return Some(true);
        }
        if self.skip_all {
            return Some(false);
        }
        let existing = fs::symlink_metadata(dst).map(|m| m.len()).unwrap_or(0);
        match self.ask(Question::Conflict {
            path: dst.to_path_buf(),
            existing_bytes: existing,
            incoming_bytes: incoming,
        }) {
            Answer::Yes => Some(true),
            Answer::YesToAll => {
                self.yes_to_all = true;
                Some(true)
            }
            Answer::Skip => Some(false),
            Answer::SkipAll => {
                self.skip_all = true;
                Some(false)
            }
            _ => {
                self.abort = true;
                None
            }
        }
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
            self.copy_symlink(src, dst);
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
        } else {
            self.copy_file(src, dst, meta.len());
        }
    }

    #[cfg(unix)]
    fn copy_symlink(&mut self, src: &Path, dst: &Path) {
        match fs::read_link(src) {
            Ok(target) => {
                if matches!(self.resolve_conflict(dst, 0), Some(false) | None) {
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
            match self.resolve_conflict(dst, size) {
                Some(true) => {}
                Some(false) => {
                    self.sum.skipped += 1;
                    self.advance(size);
                    return;
                }
                None => return,
            }
            match self.copy_bytes(src, dst) {
                Ok(true) => {
                    self.sum.done += 1;
                    return;
                }
                // Cancelled mid-file: the partial destination is removed rather than
                // left looking like a complete copy.
                Ok(false) => {
                    let _ = fs::remove_file(dst);
                    return;
                }
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

    /// Returns Ok(false) when cancelled part-way.
    fn copy_bytes(&mut self, src: &Path, dst: &Path) -> io::Result<bool> {
        let meta = fs::metadata(src)?;
        let mut r = File::open(src)?;
        let mut w = File::create(dst)?;
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
        }
        w.flush()?;
        // Permissions and mtime are part of the file as far as a user is concerned; a
        // copy that loses the executable bit is a copy that broke something.
        let _ = fs::set_permissions(dst, meta.permissions());
        if let Ok(mtime) = meta.modified() {
            let _ = w.set_times(FileTimes::new().set_modified(mtime));
        }
        Ok(true)
    }

    // ── move ───────────────────────────────────────────────────────────────────────

    fn move_into(&mut self, src: &Path, dest_dir: &Path, rename: Option<&std::ffi::OsStr>) {
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
        match self.resolve_conflict(&dst, size) {
            Some(true) => {}
            Some(false) => {
                self.sum.skipped += 1;
                return;
            }
            None => return,
        }

        match fs::rename(src, &dst) {
            Ok(()) => {
                self.sum.done += 1;
                self.advance(size);
            }
            Err(e) if is_cross_device(&e) => {
                // Not a rename at all: a copy followed by a delete, which can half-fail.
                // The user is told before it starts.
                let go = if self.yes_to_all {
                    true
                } else {
                    match self.ask(Question::CrossDevice {
                        path: src.to_path_buf(),
                    }) {
                        Answer::Yes => true,
                        Answer::YesToAll => {
                            self.yes_to_all = true;
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
                let before = self.sum.failed;
                self.copy_node(src, &dst);
                if self.sum.failed == before && !self.stopped() {
                    if let Err(e) = remove_tree(src) {
                        self.note_error(src, &e);
                    }
                }
            }
            Err(e) => self.note_error(&dst, &e),
        }
    }

    // ── delete ─────────────────────────────────────────────────────────────────────

    fn delete(&mut self, target: &Path, to_trash: bool, trash_root: Option<&Path>) {
        if self.stopped() {
            return;
        }
        if to_trash {
            match send_to_trash(target, trash_root) {
                Ok(()) => {
                    self.sum.done += 1;
                    return;
                }
                Err(e) => {
                    let answer = if self.yes_to_all {
                        Answer::Yes
                    } else {
                        self.ask(Question::TrashUnavailable {
                            path: target.to_path_buf(),
                            reason: e.to_string(),
                        })
                    };
                    match answer {
                        Answer::Yes => {}
                        Answer::YesToAll => self.yes_to_all = true,
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
        self.resolve_conflict(dst, incoming)
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

/// Move one entry to wherever this platform keeps deleted files.
pub fn send_to_trash(path: &Path, root: Option<&Path>) -> io::Result<()> {
    if let Some(base) = root {
        // Tests pass a temp directory here, so a test run never puts anything in the
        // real trash of whoever happens to be running it.
        return trash_into(base, path);
    }
    #[cfg(target_os = "linux")]
    {
        let base = default_trash_base()
            .ok_or_else(|| io::Error::other("no HOME, so no trash directory"))?;
        trash_into(&base, path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        ::trash::delete(path).map_err(|e| io::Error::other(e.to_string()))
    }
}

/// `$XDG_DATA_HOME/Trash`, falling back to `~/.local/share/Trash`.
#[cfg(target_os = "linux")]
pub fn default_trash_base() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("Trash"))
}

/// Move one entry into a freedesktop trash directory, writing the `.trashinfo` that
/// makes it restorable.
///
/// This is a rename, so it only works within one filesystem. Crossing one is reported
/// rather than silently turned into a permanent delete — the caller turns that into a
/// question. The spec's per-filesystem `.Trash-$uid` directories are not implemented;
/// asking is the honest answer until they are.
fn trash_into(base: &Path, path: &Path) -> io::Result<()> {
    let files = base.join("files");
    let info = base.join("info");
    fs::create_dir_all(&files)?;
    fs::create_dir_all(&info)?;

    let original = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let stem = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unnamed".to_string());

    let mut name = stem.clone();
    let mut n = 1;
    while files.join(&name).symlink_metadata().is_ok()
        || info.join(format!("{name}.trashinfo")).exists()
    {
        name = format!("{stem}.{n}");
        n += 1;
    }

    fs::rename(path, files.join(&name))?;

    let stamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(&original.to_string_lossy()),
        stamp
    );
    fs::write(info.join(format!("{name}.trashinfo")), body)?;
    Ok(())
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
