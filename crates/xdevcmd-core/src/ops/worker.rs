use super::*;

#[derive(Debug, Clone)]
pub enum Event {
    Scanned {
        files: u64,
        bytes: u64,
    },
    Begin {
        path: PathBuf,
    },
    /// Bytes written so far, and files finished (done, skipped or failed).
    Progress {
        bytes_done: u64,
        files_done: u64,
    },
    /// `path` failed with an error that may pass (`error`): tried again after `wait`,
    /// try `attempt` of `of` (Round 031). Cleared by the next `Progress` or `Begin`.
    Retrying {
        path: PathBuf,
        error: String,
        attempt: u32,
        of: u32,
        wait: std::time::Duration,
    },
    Ask(Question),
    Done(Summary),
}

pub struct Handle {
    pub events: Receiver<Event>,
    pub(super) answers: Sender<Answer>,
    pub(super) cancel: Arc<AtomicBool>,
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
            last_tick: std::time::Instant::now(),
            sum: Summary::default(),
            moving: false,
            approved: None,
            journal: None,
            removing: Vec::new(),
            removing_bytes: 0,
            moved_file: None,
            safe: req.safe,
            recording: req.kind == Kind::Copy,
            copying: req.kind == Kind::Copy,
            plan_confirmed: req.planned.is_some(),
            trash_root: req.trash_root.clone(),
            backoff: req.backoff.clone(),
            retry: false,
            network: Default::default(),
            keeps: Default::default(),
            old_trashed: None,
            group: Group::new(),
            fast: req.fast,
            engine: crate::fastcopy::Engine::Own,
            checkpoint: req.checkpoint,
            partials: req
                .partial
                .iter()
                .map(|p| (p.dst.clone(), p.clone()))
                .collect(),
            ckpt: None,
            // Checked: nothing is taken as finished, so every folder is walked.
            done_dirs: if req.verify {
                Default::default()
            } else {
                req.done_dirs.clone()
            },
            verify: req.verify && req.existing == Existing::Resume,
            finished: Vec::new(),
            dbg: crate::debuglog::on(crate::debuglog::Topic::Jobs).then(|| {
                Box::new(Dbg {
                    job: crate::debuglog::next_job(),
                    started: std::time::Instant::now(),
                    phases: Default::default(),
                    small: Default::default(),
                    large: Default::default(),
                    batches: 0,
                    scan: Default::default(),
                    journal: Default::default(),
                    checkpoints: 0,
                    checkpoint_time: Default::default(),
                    resumed_bytes: 0,
                    dirs_done: 0,
                    dirs_skipped: 0,
                })
            }),
            phase: "start",
        };
        w.run(req);
    });

    Handle {
        events: erx,
        answers: atx,
        cancel,
    }
}

pub(super) struct Worker {
    pub(super) tx: Sender<Event>,
    pub(super) answers: Receiver<Answer>,
    pub(super) cancel: Arc<AtomicBool>,
    /// "All" answers, one per kind of question: overwriting all is not also agreeing
    /// to copy-then-delete across devices, or to delete permanently.
    pub(super) overwrite_all: bool,
    pub(super) skip_all: bool,
    /// Replace only what is older than its source; skip what is the same or newer.
    pub(super) update: bool,
    /// [`Existing::Resume`]: skip what is identical, ask about the rest.
    pub(super) resuming: bool,
    pub(super) cross_all: bool,
    pub(super) trash_all: bool,
    /// Synchronising: [`Request::expect`].
    pub(super) expect: std::collections::HashMap<PathBuf, Expect>,
    /// The file a decision to replace was about: the destination and what was there
    /// when the user was asked (what they saw) or the plan was made. Checked again
    /// right before the step that replaces it ([`Worker::still_agreed`]).
    pub(super) agreed: Option<(PathBuf, Ident)>,
    /// A destination found changed since it was agreed to: its next question says so.
    pub(super) changed: Option<PathBuf>,
    pub(super) abort: bool,
    pub(super) bytes_done: u64,
    pub(super) last_report: u64,
    /// When progress was last said.
    pub(super) last_tick: std::time::Instant,
    pub(super) sum: Summary,
    /// Copying for a move across devices: each source is removed as soon as its copy
    /// is complete, so what was skipped or failed stays where it was.
    pub(super) moving: bool,
    /// A destination the user already agreed to replace, so the copy a cross-device
    /// move falls back to does not ask about it a second time.
    pub(super) approved: Option<PathBuf>,
    /// A move across devices: copies complete and synced whose sources wait to be
    /// removed, a batch at a time ([`Worker::remove_moved`]).
    /// Each with the handle that wrote the copy, still open, to sync it through: a
    /// handle opened afresh cannot write a read-only copy, and on a network file system
    /// would not be told of a write that failed on the first (at most
    /// [`MOVE_BATCH_FILES`] are held). `None`: already synced (a replacement), or a link.
    pub(super) removing: Vec<(PathBuf, PathBuf, Option<File>)>,
    /// Their size, in bytes.
    pub(super) removing_bytes: u64,
    /// The copy [`Worker::copy_bytes`] just made while moving, still open, for
    /// [`Worker::moved`] to keep with it.
    pub(super) moved_file: Option<File>,
    /// Safe operations: what this job is in the middle of, for the next start if it is
    /// killed. Removed when the job ends.
    pub(super) journal: Option<crate::journal::Journal>,
    /// [`Request::safe`]: decides the journal and whether a new file is written under
    /// a temporary name.
    pub(super) safe: bool,
    /// A copy: what it makes is kept in [`Summary::made`], for its undo.
    pub(super) recording: bool,
    /// A copy (the kind, whatever the undo cap did to `recording`).
    pub(super) copying: bool,
    /// The job comes from a confirmed plan ([`Request::planned`]).
    pub(super) plan_confirmed: bool,
    /// [`Request::trash_root`]: tests' trash, for a replaced file too.
    pub(super) trash_root: Option<PathBuf>,
    /// [`Request::backoff`].
    pub(super) backoff: Vec<std::time::Duration>,
    /// The user answered Retry to the last failed file.
    pub(super) retry: bool,
    /// Per directory: whether it is reached over a network ([`crate::plan::is_network`]).
    pub(super) network: std::collections::HashMap<PathBuf, bool>,
    /// Per destination directory: whether a file replaced there goes to the trash.
    pub(super) keeps: std::collections::HashMap<PathBuf, bool>,
    /// The file the last replace put in the trash, for its undo.
    pub(super) old_trashed: Option<crate::trashed::Trashed>,
    /// Group commit: replacements written and waiting for their batch's sync.
    pub(super) group: Group,
    /// `[general] fast_copy` when the job started (item 22). Nothing reads it where
    /// there is no fast copy yet (Windows).
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "android", target_vendor = "apple")),
        allow(dead_code)
    )]
    pub(super) fast: bool,
    /// The engine that moved the last file's bytes.
    pub(super) engine: crate::fastcopy::Engine,
    /// Bytes between a big file's checkpoints (0: none).
    pub(super) checkpoint: u64,
    /// Resume: big files cut short after a checkpoint, by destination, not yet taken
    /// over. Whatever is left at the end goes.
    pub(super) partials: std::collections::HashMap<PathBuf, crate::journal::Partial>,
    /// The big file being written, while it has checkpoints.
    pub(super) ckpt: Option<Ckpt>,
    /// Resume: source folders finished before, skipped while their copy is there.
    pub(super) done_dirs: std::collections::HashSet<PathBuf>,
    /// [`Request::verify`]: an identical-looking destination is compared byte by byte.
    pub(super) verify: bool,
    /// Folders copied without a failure whose last files wait in the group batch: told
    /// to the journal once that batch is through without one.
    pub(super) finished: Vec<PathBuf>,
    /// The debug log's record of this job ([`crate::debuglog`]), when it logs jobs.
    pub(super) dbg: Option<Box<Dbg>>,
    /// The step under way, for the debug log's error lines.
    pub(super) phase: &'static str,
}

/// The phases a file goes through, as the debug log names them.
#[derive(Clone, Copy)]
pub(super) enum Phase {
    /// Is a file there, and is it to be replaced.
    Check,
    /// The source's details, and the source opened.
    Open,
    /// The new file made (or cloned, on macOS).
    Create,
    /// The bytes moved.
    Write,
    /// Permissions and the date.
    Attrs,
    /// Data synced, on its own or in a batch.
    Sync,
    /// The journal, the old file to the trash, the rename.
    Finish,
}

pub(super) const PHASES: [&str; 7] = [
    "check", "open", "create", "write", "attrs", "sync", "finish",
];

/// What the debug log is told at a job's end. Added up as the job goes: one line a
/// job, not one a file.
pub(super) struct Dbg {
    pub(super) job: u64,
    pub(super) started: std::time::Instant,
    pub(super) phases: [std::time::Duration; 7],
    /// Files of up to [`GROUP_FILE_MAX`] and larger: count, bytes, time in `copy_bytes`.
    pub(super) small: (u64, u64, std::time::Duration),
    pub(super) large: (u64, u64, std::time::Duration),
    pub(super) batches: u64,
    pub(super) scan: std::time::Duration,
    /// The journal begun and synced, before the first step.
    pub(super) journal: std::time::Duration,
    /// Big files' checkpoints taken, and the time their syncs took.
    pub(super) checkpoints: u64,
    pub(super) checkpoint_time: std::time::Duration,
    /// Bytes not copied again: big files taken over from their checkpoint.
    pub(super) resumed_bytes: u64,
    /// Folders told to the journal as finished, and folders a resume skipped.
    pub(super) dirs_done: u64,
    pub(super) dirs_skipped: u64,
}

/// The big file being written now, while it has checkpoints: see [`CHECKPOINT`].
pub(super) struct Ckpt {
    pub(super) part: crate::journal::Partial,
    /// Where the next checkpoint is due.
    pub(super) next: u64,
}

/// A replacement written under its temporary name, its handle open, waiting for the
/// batch's sync before the steps that finish it (Round 033).
pub(super) struct Pending {
    pub(super) src: PathBuf,
    pub(super) dst: PathBuf,
    pub(super) tmp: PathBuf,
    pub(super) file: File,
    pub(super) size: u64,
    /// What was agreed about `dst` when it was asked or planned ([`Worker::agreed`]).
    pub(super) agreed: Option<(PathBuf, Ident)>,
}

/// The probe under way: thread counts still to try, and what each one tried measured
/// (milliseconds a file).
pub(super) type Probe = (Vec<usize>, Vec<(usize, f64)>);

/// Group commit's state (Round 033). Measured: a sync per file costs a journal commit
/// each; syncs in flight together share one, so N files on T threads cost about N÷T
/// commits. On the owner's Windows 11 laptop 16 threads were 4.6× one thread, 32 up
/// to 8.5×; on ext4 16 were 9.7×; on macOS 8 were 2.4×.
pub(super) struct Group {
    pub(super) pending: Vec<Pending>,
    pub(super) bytes: u64,
    pub(super) since: Option<std::time::Instant>,
    /// Threads for a batch's syncs.
    pub(super) threads: usize,
    /// Files batched so far in this job.
    pub(super) batched: u64,
    /// The probe under way: thread counts still to try, and what each one measured
    /// (milliseconds a file).
    pub(super) probe: Option<Probe>,
    pub(super) probed: bool,
    /// A flush is running: a question it asks (the journal's) must not flush again.
    pub(super) flushing: bool,
    /// The file being written must not join a batch: a batch's own fallback.
    pub(super) off: bool,
    /// Set by `copy_bytes` when the file just written joined the batch.
    pub(super) deferred: bool,
    /// What the probe measured, once it chose (for the debug log).
    pub(super) tried: Vec<(usize, f64)>,
}

impl Group {
    pub(super) fn new() -> Self {
        Self {
            pending: Vec::new(),
            bytes: 0,
            since: None,
            threads: group_default_threads(),
            batched: 0,
            probe: None,
            probed: false,
            flushing: false,
            off: false,
            deferred: false,
            tried: Vec::new(),
        }
    }

    /// The threads the next batch syncs on.
    pub(super) fn threads_now(&self) -> usize {
        self.probe
            .as_ref()
            .and_then(|(left, _)| left.first().copied())
            .unwrap_or(self.threads)
    }

    /// The files the next batch holds: four rounds of its threads, at least 16 while
    /// probing (one sample) and 64 otherwise.
    pub(super) fn cap(&self) -> usize {
        let t = self.threads_now();
        if self.probe.is_some() {
            (4 * t).max(16)
        } else {
            (4 * t).max(64)
        }
    }

    pub(super) fn full(&self) -> bool {
        self.pending.len() >= self.cap()
            || self.bytes >= GROUP_BATCH_BYTES
            || self.since.is_some_and(|s| s.elapsed() >= GROUP_BATCH_AGE)
    }

    /// A batch of `files` synced in `ms`: the probe's measurement, and its choice once
    /// every count is tried. A probe starts once [`GROUP_PROBE_AFTER`] files went by.
    pub(super) fn measured(&mut self, files: usize, ms: f64) {
        if let Some((left, got)) = self.probe.as_mut() {
            if files > 0 {
                got.push((left.remove(0), ms / files as f64));
            }
            if left.is_empty() {
                if let Some(best) = got.iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
                    self.threads = best.0;
                }
                self.tried = std::mem::take(got);
                self.probe = None;
                self.probed = true;
            }
        } else if !self.probed && self.batched >= GROUP_PROBE_AFTER {
            // The default first, then the others: what the default measures here is
            // what the others are compared with.
            let mut order = vec![self.threads];
            order.extend(GROUP_PROBE.iter().filter(|n| **n != self.threads));
            self.probe = Some((order, Vec::new()));
        }
    }
}

/// The thread count group commit starts with (Round 033: the CI sweep and the owner's
/// laptop): 8 on macOS, where more did not help; 16 elsewhere.
pub(super) fn group_default_threads() -> usize {
    if cfg!(target_os = "macos") {
        8
    } else {
        16
    }
}

/// Sync every pending file's data on `threads` threads, each taking the next file: per
/// file, the error if its sync failed.
pub(super) fn sync_all(batch: &[Pending], threads: usize) -> Vec<Option<io::Error>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<io::Error>> = (0..batch.len()).map(|_| None).collect();
    let failed: Vec<(usize, io::Error)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads.clamp(1, batch.len().max(1)))
            .map(|_| {
                s.spawn(|| {
                    let mut bad = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(p) = batch.get(i) else { break };
                        if let Err(e) = crate::faults::sync_data(&p.file, &p.tmp) {
                            bad.push((i, e));
                        }
                    }
                    bad
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_default())
            .collect()
    });
    for (i, e) in failed {
        out[i] = Some(e);
    }
    out
}

impl Worker {
    pub(super) fn run(&mut self, req: Request) {
        // Every kind of job writes. In read-only mode none starts, whatever asked.
        if let Err(e) = crate::policy::guard_write() {
            self.sum.failed = req.sources.len() as u64;
            self.sum.errors.push(e.to_string());
            let _ = self.tx.send(Event::Done(self.sum.clone()));
            return;
        }
        // The engine's own wall behind the UI's (#2): a root, the home or a folder above
        // it, or a path with `.`/`..`, is never deleted or moved, whatever asked.
        if matches!(req.kind, Kind::Delete | Kind::Move) {
            let homes = crate::protect::homes();
            let refused: Vec<String> = req
                .sources
                .iter()
                .filter_map(|s| {
                    crate::protect::protected(s, &homes)
                        .map(|_| format!("{}: never deleted or moved", s.display()))
                })
                .collect();
            if !refused.is_empty() {
                self.sum.failed = refused.len() as u64;
                self.sum.errors = refused;
                let _ = self.tx.send(Event::Done(self.sum.clone()));
                return;
            }
        }
        let kind = match req.kind {
            Kind::Copy if !req.pairs.is_empty() => "sync",
            Kind::Copy => "copy",
            Kind::Move => "move",
            Kind::Delete => "delete",
            Kind::Extract => "extract",
            Kind::Pack => "pack",
        };
        if self.safe {
            let sources: Vec<PathBuf> = if req.pairs.is_empty() {
                req.sources.clone()
            } else {
                req.pairs.iter().map(|(s, _)| s.clone()).collect()
            };
            let t = self.clock();
            let begun = crate::journal::Journal::begin_as(
                kind,
                &sources,
                req.dest.as_deref(),
                req.rename_to.as_deref(),
                req.to_trash,
            );
            if let (Some(d), Some(t)) = (self.dbg.as_mut(), t) {
                d.journal = t.elapsed();
            }
            match begun {
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
        // Resume: the big files cut short are this job's now. Written into its journal,
        // and on disk, before the old journal goes, so a kill in between still keeps
        // them. Without a journal of its own, there are no checkpoints to keep them by:
        // they are copied again from the start.
        // The folders finished before stay finished if this run dies too. A line that
        // fails only breaks the journal, asked about before a step that needs it.
        // The plan's decisions, for a resume to keep them (Round 030): written before
        // the first step, synced with the first commit like every line. A resume
        // writes them again before its old journal goes, for a second kill (repo
        // review RR05: they were lost then).
        if req.planned.is_some() || req.resumed.is_some() {
            if let Some(j) = self.journal.as_mut() {
                for (dst, e) in &req.expect {
                    let _ = j.decided(dst, e);
                }
            }
        }
        if let Some(j) = self.journal.as_mut() {
            for d in &self.done_dirs {
                if j.done(d).is_err() {
                    break;
                }
            }
        }
        if let Some(old) = &req.resumed {
            let kept = match self.journal.as_mut() {
                Some(j) => self
                    .partials
                    .values()
                    .try_for_each(|p| j.part(&p.part).and_then(|()| j.checkpoint(p)))
                    .and_then(|()| j.commit(old))
                    .is_ok(),
                None => false,
            };
            if !kept {
                self.drop_partials();
            }
            crate::journal::forget(old);
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
        if !req.pairs.is_empty() {
            let sources: Vec<PathBuf> = req.pairs.iter().map(|(s, _)| s.clone()).collect();
            let (files, bytes) = scan(&sources);
            let _ = self.tx.send(Event::Scanned { files, bytes });
            self.log_start(&req, kind, files, bytes);
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
        let t = self.dbg.as_ref().map(|_| std::time::Instant::now());
        let (files, bytes) = req
            .planned
            .unwrap_or_else(|| scan_skipping(&req.sources, &self.done_dirs));
        if let (Some(d), Some(t)) = (self.dbg.as_mut(), t) {
            d.scan = t.elapsed();
        }
        let _ = self.tx.send(Event::Scanned { files, bytes });
        self.log_start(&req, kind, files, bytes);

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
                        if let Some(name) = req.rename_to.as_deref().or(src.file_name()) {
                            let to = d.join(name);
                            if fs::symlink_metadata(src).is_err()
                                && fs::symlink_metadata(&to).is_ok()
                            {
                                self.sum.relocated.push((src.clone(), to));
                            }
                        }
                    }
                }
                Kind::Delete => self.delete(src, req.to_trash, req.trash_root.as_deref()),
                Kind::Extract | Kind::Pack => {}
            }
            // Finished with this source (not stopped part way): a resume leaves it.
            // Its batch first: a resume leaves a source marked done.
            self.flush_group();
            if !self.stopped() && matches!(req.kind, Kind::Copy | Kind::Move) {
                if let Some(j) = self.journal.as_mut() {
                    let _ = j.done(src);
                }
            }
        }

        self.finish();
    }

    pub(super) fn stopped(&self) -> bool {
        self.abort || self.cancel.load(Ordering::Relaxed)
    }

    /// Whether `p` is in a directory reached over a network, asked once per directory.
    pub(super) fn on_network(&mut self, p: &Path) -> bool {
        let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
        *self
            .network
            .entry(dir)
            .or_insert_with_key(|d| crate::plan::is_network(d))
    }

    /// Wait `d`, in short steps, ending at once on Esc: false when stopped.
    pub(super) fn wait(&self, d: std::time::Duration) -> bool {
        let end = std::time::Instant::now() + d;
        loop {
            if self.stopped() {
                return false;
            }
            let now = std::time::Instant::now();
            if now >= end {
                return true;
            }
            std::thread::sleep((end - now).min(std::time::Duration::from_millis(50)));
        }
    }

    /// Whether the file `dst` replaces goes to the trash first (Round 031): where the
    /// trash is on the same volume ([`crate::plan::trash_keeps`]), asked once per
    /// directory. Tests give their own trash, and always use it.
    pub(super) fn replace_into_trash(&mut self, dst: &Path) -> bool {
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
    pub(super) fn made(&mut self, path: &Path, old: Option<crate::trashed::Trashed>) {
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
    pub(super) fn finish(&mut self) {
        // Taken over by no file of this job (skipped, changed, or the job stopped
        // first): they would never be finished.
        self.drop_partials();
        self.flush_group();
        self.sum.cancelled = self.cancel.load(Ordering::Relaxed) || self.abort;
        self.sum.bytes = self.bytes_done;
        if let Some(why) = self.journal.as_ref().and_then(|j| j.broken()) {
            self.sum.journal_off = Some(format!("its journal failed: {why}"));
        }
        self.log_end();
        let _ = self.tx.send(Event::Done(self.sum.clone()));
    }

    /// The debug log's first line for this job: its shape, no names.
    pub(super) fn log_start(&mut self, req: &Request, kind: &str, files: u64, bytes: u64) {
        let Some(job) = self.dbg.as_ref().map(|d| d.job) else {
            return;
        };
        let existing = match req.existing {
            Existing::Ask => "ask",
            Existing::Update => "update",
            Existing::Skip => "skip",
            Existing::Replace => "replace",
            Existing::Resume => "resume",
        };
        let mut e = crate::debuglog::Event::new("job_start")
            .num("job", job)
            .word("kind", kind)
            .num("sources", req.sources.len().max(req.pairs.len()) as u64)
            .num("files", files)
            .num("bytes", bytes)
            .word("existing", existing)
            .flag("planned", req.planned.is_some())
            .flag("safe", self.safe)
            .flag("fast", req.fast)
            .num("group_threads", self.group.threads as u64);
        if let Some(src) = req.sources.first().and_then(|p| p.parent()) {
            e = e.word("src_fs", &crate::plan::fs_word(src));
        }
        if let Some(d) = req.dest.as_deref() {
            e = e
                .word("dst_fs", &crate::plan::fs_word(d))
                .flag("dst_network", self.on_network(d));
        }
        e.write();
        if req.existing == Existing::Resume {
            crate::debuglog::Event::new("resume")
                .num("job", job)
                .num("sources_left", req.sources.len() as u64)
                .num("decisions_kept", req.expect.len() as u64)
                .write();
        }
    }

    /// The debug log's last line for this job: what it did and where the time went.
    pub(super) fn log_end(&mut self) {
        let Some(d) = self.dbg.take() else { return };
        let s = &self.sum;
        let mut e = crate::debuglog::Event::new("job_end")
            .num("job", d.job)
            .ms("ms", d.started.elapsed())
            .ms("scan_ms", d.scan)
            .ms("journal_ms", d.journal)
            .num("done", s.done)
            .num("skipped", s.skipped)
            .num("failed", s.failed)
            .flag("cancelled", s.cancelled)
            .num("bytes", self.bytes_done)
            .num("cloned", s.cloned)
            .num("by_kernel", s.by_kernel)
            .num("replaced_to_trash", s.replaced_to_trash)
            .flag("journal_off", s.journal_off.is_some());
        for (name, t) in PHASES.iter().zip(d.phases) {
            e = e.ms(&format!("{name}_ms"), t);
        }
        e.num("small_files", d.small.0)
            .num("small_bytes", d.small.1)
            .ms("small_ms", d.small.2)
            .num("large_files", d.large.0)
            .num("large_bytes", d.large.1)
            .ms("large_ms", d.large.2)
            .num("batches", d.batches)
            .num("checkpoints", d.checkpoints)
            .ms("checkpoint_ms", d.checkpoint_time)
            .num("resumed_bytes", d.resumed_bytes)
            .num("dirs_done", d.dirs_done)
            .num("dirs_skipped", d.dirs_skipped)
            .num("group_threads", self.group.threads as u64)
            .write();
    }

    /// The time now, when the debug log keeps time.
    pub(super) fn clock(&self) -> Option<std::time::Instant> {
        self.dbg.as_ref().map(|_| std::time::Instant::now())
    }

    /// The time since `t` added to `p`, and `t` moved on to now.
    pub(super) fn lap(&mut self, p: Phase, t: &mut Option<std::time::Instant>) {
        if let (Some(d), Some(start)) = (self.dbg.as_mut(), t.as_mut()) {
            let now = std::time::Instant::now();
            d.phases[p as usize] += now - *start;
            *start = now;
        }
    }

    /// Before a step that cannot be undone, about `about`: the journal's lines are on
    /// disk. A journal that cannot be (now or before) is asked about, once: go on
    /// without it, or stop here (`false`; the job ends).
    pub(super) fn durable(&mut self, about: &Path) -> bool {
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
    pub(super) fn go_without_journal(&mut self, why: String) -> bool {
        if crate::debuglog::on(crate::debuglog::Topic::Errors) {
            crate::debuglog::Event::new("journal_failed")
                .num("job", self.dbg_job())
                .word("phase", self.phase)
                .write();
        }
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
    pub(super) fn archive_job(&mut self, req: &Request) {
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
                    // Left as it is: counted, so the summary says so (red-team of #5).
                    Some(false) => {
                        self.sum.skipped += 1;
                        Ok(())
                    }
                    None => Ok(()),
                }
            }
        };
        if let Err(e) = result {
            self.note_error(&dest, &e);
        } else if req.kind == Kind::Pack && !self.stopped() && dest.exists() {
            self.sum.done = self.sum.done.max(1);
        }
    }

    pub(super) fn ask(&mut self, q: Question) -> Answer {
        // What is waiting in a batch is finished first: a question can stay open for
        // minutes, and what was done before it should be done.
        self.flush_group();
        let logged = crate::debuglog::on(crate::debuglog::Topic::Errors).then(|| {
            let (q_word, path) = match &q {
                Question::Conflict { path, changed, .. } if *changed => ("changed", Some(path)),
                Question::Conflict { path, .. } => ("conflict", Some(path)),
                Question::CrossDevice { path } => ("cross_device", Some(path)),
                Question::TrashUnavailable { path, .. } => ("no_trash", Some(path)),
                Question::Failed { path, .. } => ("failed", Some(path)),
                Question::NoJournal { .. } => ("no_journal", None),
            };
            (
                q_word,
                path.map(|p| crate::debuglog::file_id(p)),
                std::time::Instant::now(),
            )
        });
        if self.tx.send(Event::Ask(q)).is_err() {
            return Answer::Abort;
        }
        // The UI has gone away if this fails, and continuing to touch the filesystem
        // without anyone able to stop us is the one thing we must not do.
        let a = self.answers.recv().unwrap_or(Answer::Abort);
        if let Some((q_word, file, t)) = logged {
            let mut e = crate::debuglog::Event::new("ask")
                .num("job", self.dbg_job())
                .word("q", q_word)
                .word("phase", self.phase)
                .word("answer", &format!("{a:?}"))
                .ms("waited_ms", t.elapsed());
            if let Some(f) = file {
                e = e.num("file", f);
            }
            e.write();
        }
        a
    }

    /// This job's number in the debug log; 0 when it logs only errors.
    pub(super) fn dbg_job(&self) -> u64 {
        self.dbg.as_ref().map_or(0, |d| d.job)
    }

    pub(super) fn note_error(&mut self, path: &Path, e: &io::Error) {
        if crate::debuglog::on(crate::debuglog::Topic::Errors) {
            crate::debuglog::Event::new("error")
                .num("job", self.dbg_job())
                .file(path)
                .word("phase", self.phase)
                .error(e)
                .write();
        }
        self.sum.failed += 1;
        if self.sum.errors.len() < 20 {
            self.sum.errors.push(format!("{}: {e}", path.display()));
        }
    }

    /// `n` more bytes done (0: a file finished). Said every [`PROGRESS_STEP`] bytes,
    /// and at least every [`PROGRESS_EVERY`] while files are finishing, so a run of small
    /// files moves the bar and the ETA too (Round 031).
    pub(super) fn advance(&mut self, n: u64) {
        self.bytes_done += n;
        if self.bytes_done - self.last_report >= PROGRESS_STEP
            || self.last_tick.elapsed() >= PROGRESS_EVERY
        {
            self.last_report = self.bytes_done;
            self.last_tick = std::time::Instant::now();
            let _ = self.tx.send(Event::Progress {
                bytes_done: self.bytes_done,
                files_done: self.sum.done + self.sum.skipped + self.sum.failed,
            });
        }
    }

    /// Why a skip happened, for the summary, without counting it: the caller does.
    pub(super) fn note_skip(&mut self, path: &Path, why: &str) {
        self.sum.kind_skips += 1;
        if self.sum.errors.len() < 20 {
            self.sum.errors.push(format!("{}: {why}", path.display()));
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
        // An entry left as it is: counted here, as a copy counts its skips; the
        // archive's walk goes on without a word (red-team of #5).
        let r = self.resolve_conflict(None, dst, incoming);
        if r == Some(false) {
            self.sum.skipped += 1;
        }
        r
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

/// Count what the job is about to touch, so a progress bar means something.
pub fn scan(sources: &[PathBuf]) -> (u64, u64) {
    scan_skipping(sources, &Default::default())
}

/// As [`scan`], leaving out the folders in `skip` (a resume's finished ones).
pub(super) fn scan_skipping(
    sources: &[PathBuf],
    skip: &std::collections::HashSet<PathBuf>,
) -> (u64, u64) {
    scan_in(sources, &AtomicBool::new(false), skip).unwrap_or((0, 0))
}

/// As [`scan`], stopping when `stop` is set: `None` then. A count of a large tree on a
/// slow disk takes seconds, and the dialog asking about it must not wait for it.
pub fn scan_until(sources: &[PathBuf], stop: &AtomicBool) -> Option<(u64, u64)> {
    scan_in(sources, stop, &Default::default())
}

pub(super) fn scan_in(
    sources: &[PathBuf],
    stop: &AtomicBool,
    skip: &std::collections::HashSet<PathBuf>,
) -> Option<(u64, u64)> {
    let walk_skip = skip;
    fn walk(
        p: &Path,
        files: &mut u64,
        bytes: &mut u64,
        stop: &AtomicBool,
        skip: &std::collections::HashSet<PathBuf>,
    ) -> bool {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(p) else {
            return true;
        };
        if meta.file_type().is_symlink() {
            *files += 1;
        } else if meta.is_dir() {
            if !skip.is_empty() && skip.contains(p) {
                return true;
            }
            if let Ok(it) = fs::read_dir(p) {
                for e in it.flatten() {
                    if !walk(&e.path(), files, bytes, stop, skip) {
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
        if !walk(s, &mut f, &mut b, stop, walk_skip) {
            return None;
        }
    }
    Some((f, b))
}

/// [`scan_until`] on its own thread. Dropping the handle stops it.
pub struct Count {
    pub rx: Receiver<(u64, u64)>,
    pub(super) stop: Arc<AtomicBool>,
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
