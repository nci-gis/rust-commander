use super::*;

impl Worker {
    /// Whether regular files `a` and `b` hold the same bytes, read side by side; a read
    /// that fails counts as different (copying again then reports the error). Stops
    /// early, as different, when the job is cancelled.
    pub(super) fn same_bytes(&mut self, a: &Path, b: &Path) -> bool {
        self.same_upto(a, b, None)
    }

    /// [`same_bytes`](Self::same_bytes) for the first `upto` bytes only, when given:
    /// both must hold that many (a kept part read again with its source, RR01).
    pub(super) fn same_upto(&mut self, a: &Path, b: &Path, upto: Option<u64>) -> bool {
        use std::io::Read;
        self.phase = "verify";
        let (Ok(mut fa), Ok(mut fb)) = (File::open(a), File::open(b)) else {
            return false;
        };
        // The box names the file and its bar moves while a big one is read (#3): the
        // bytes read are shown, not added; the file's size is added once, after.
        let _ = self.tx.send(Event::Begin {
            path: a.to_path_buf(),
        });
        let mut read = 0u64;
        let mut ba = vec![0u8; 1 << 20];
        let mut bb = vec![0u8; 1 << 20];
        loop {
            if self.stopped() {
                return false;
            }
            if self.last_tick.elapsed() >= PROGRESS_EVERY {
                self.last_tick = std::time::Instant::now();
                let _ = self.tx.send(Event::Progress {
                    bytes_done: self.bytes_done + read,
                    files_done: self.sum.done + self.sum.skipped + self.sum.failed,
                });
            }
            let want = upto.map_or(ba.len(), |u| (u - read).min(ba.len() as u64) as usize);
            if want == 0 {
                return true;
            }
            let Ok(n) = read_full(&mut fa, &mut ba[..want]) else {
                return false;
            };
            let Ok(m) = read_full(&mut fb, &mut bb[..want]) else {
                return false;
            };
            if n != m || ba[..n] != bb[..m] {
                return false;
            }
            if n == 0 {
                // Both ended: whole files match; a prefix one short of `upto` does not.
                return upto.is_none();
            }
            read += n as u64;
        }
        fn read_full(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
            let mut got = 0;
            while got < buf.len() {
                match f.read(&mut buf[got..]) {
                    Ok(0) => break,
                    Ok(k) => got += k,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(got)
        }
    }

    /// Decide what to do about an existing destination. `Some(true)` means go ahead.
    /// `src`: the file that would replace it, for the update rule; none for an archive.
    pub(super) fn resolve_conflict(
        &mut self,
        src: Option<&Path>,
        dst: &Path,
        incoming: u64,
    ) -> Option<bool> {
        let Some(now) = Ident::at(dst) else {
            return Some(true);
        };
        // #5: a file never replaces a folder, nor a folder a file — before every rule
        // below, "replace all" and the plan's word included. Skipped (the caller counts
        // it) and said; an archive's entry (no source) is a file.
        if let Some(why) = other_kind(src, dst) {
            self.note_skip(dst, why);
            return Some(false);
        }
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
        // A confirmed plan leaves what is identical (its Identical group has no other
        // choice), also the files past its cap that it kept no row for.
        if self.plan_confirmed && identical(src, dst) == Same::Yes {
            return Some(false);
        }
        if self.update {
            return Some(self.is_older(src, dst));
        }
        if self.resuming {
            match identical(src, dst) {
                Same::Dirs => return Some(true),
                Same::Yes => {
                    // Regular files only: a link is checked by its target (identical),
                    // and opening one to a FIFO or a device blocks or never ends
                    // (red-team F2). A check that Stop cut short repairs nothing (F4).
                    let regular = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_file());
                    let differs = self.verify
                        && src.is_some_and(|s| {
                            regular(s) && regular(dst) && !self.same_bytes(s, dst)
                        })
                        && !self.stopped();
                    if !differs {
                        return Some(false);
                    }
                    // Short or empty under its real name (B2): the source's copy is
                    // what the job was asked for, so it replaces it without a question,
                    // while it is still this file.
                    self.sum.repaired += 1;
                    self.agreed = Some((dst.to_path_buf(), now));
                    return Some(true);
                }
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
    pub(super) fn still_agreed(&mut self, src: Option<&Path>, dst: &Path) -> bool {
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

    /// The update rule: `dst` is replaced only when `src` is newer (beyond the precision
    /// the coarser of the two times was kept to: 2 s on FAT). Two directories merge. Without a source (an archive) it is kept.
    pub(super) fn is_older(&self, src: Option<&Path>, dst: &Path) -> bool {
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

    pub(super) fn copy_into(
        &mut self,
        src: &Path,
        dest_dir: &Path,
        rename: Option<&std::ffi::OsStr>,
    ) {
        let Some(name) = rename.or(src.file_name()) else {
            return;
        };
        let dst = dest_dir.join(name);
        if crate::plan::onto_itself(src, &dst) {
            self.sum.skipped += 1;
            return;
        }
        // Copying a directory into itself would recurse until the disk filled.
        if crate::plan::into_itself(src, &dst, dest_dir) {
            let e = io::Error::other("cannot copy a directory into itself");
            self.note_error(src, &e);
            return;
        }
        self.copy_node(src, &dst);
    }

    pub(super) fn copy_node(&mut self, src: &Path, dst: &Path) {
        self.phase = "walk";
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
            // Resume: finished by the run before, and its copy still there.
            if self.done_dirs.contains(src) && dst.is_dir() {
                self.sum.skipped += 1;
                if let Some(d) = self.dbg.as_mut() {
                    d.dirs_skipped += 1;
                }
                return;
            }
            let failed_before = self.sum.failed;
            let there = dst.symlink_metadata();
            let made = there.is_err();
            // #5: a folder never replaces a file, and a link is a file here, as the
            // plan, a move and Alt+Y take it: the table says "skipped", and nothing is
            // written through the link to wherever it points (QA 2026-11-17, P0; before,
            // the copy went into the link's folder).
            if let Some(m) = there.as_ref().ok().filter(|m| !m.is_dir()) {
                self.sum.skipped += 1;
                let why = if m.is_symlink() {
                    "is a link \u{2014} not copied through; open it to copy into it"
                } else {
                    "is a file \u{2014} a folder never replaces it"
                };
                self.note_skip(dst, why);
                return;
            }
            if let Err(e) = fs::create_dir_all(dst) {
                self.note_error(dst, &e);
                return;
            }
            if made {
                self.made(dst, None);
                if self.safe && !self.moving {
                    self.fresh.dirs.push(dst.to_path_buf());
                }
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
            // A directory made read-only would refuse its batch's renames: they go first.
            if denies_writing(&meta) {
                self.flush_group();
            }
            let _ = fs::set_permissions(dst, meta.permissions());
            self.sum.done += 1;
            if self.sum.failed == failed_before && !self.stopped() {
                self.folder_finished(src);
            }
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
    pub(super) fn moved(&mut self, src: &Path, dst: &Path, done_before: u64) {
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
    pub(super) fn remove_moved(&mut self) {
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
    pub(super) fn copy_symlink(&mut self, src: &Path, dst: &Path) {
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
                    Ok(true) => {
                        self.sum.done += 1;
                        // A link has no data, only its name: its folder is synced with
                        // the batch (B3; beta.4 red-team: a folder of links only).
                        if self.safe && !self.moving {
                            self.fresh.dirs.push(dst.to_path_buf());
                        }
                    }
                    Ok(false) => {}
                    Err(e) => self.note_error(dst, &e),
                }
            }
            Err(e) => self.note_error(src, &e),
        }
    }

    #[cfg(not(unix))]
    pub(super) fn copy_symlink(&mut self, src: &Path, _dst: &Path) {
        let e = io::Error::other("symlinks are not copied on this platform");
        self.note_error(src, &e);
    }

    pub(super) fn copy_file(&mut self, src: &Path, dst: &Path, size: u64) {
        // Tries made of this file after an error that may pass (Round 031).
        let mut tries = 0;
        loop {
            if self.stopped() {
                return;
            }
            // Free now: written only if it is still free when opened, so a file that
            // appears meanwhile is asked about, not overwritten.
            self.phase = "check";
            let mut t = self.clock();
            let free = dst.symlink_metadata().is_err();
            let decided = self.resolve_conflict(Some(src), dst, size);
            self.lap(Phase::Check, &mut t);
            match decided {
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
            let copied = self.copy_bytes(src, dst, free);
            if let (Some(d), Some(t)) = (self.dbg.as_mut(), t) {
                let lane = if size <= GROUP_FILE_MAX {
                    &mut d.small
                } else {
                    &mut d.large
                };
                lane.0 += 1;
                lane.1 += size;
                lane.2 += t.elapsed();
            }
            match copied {
                // Waiting for its batch: counted when the batch finishes it.
                Ok(true) if std::mem::take(&mut self.group.deferred) => {
                    if self.group.full() {
                        self.flush_group();
                    }
                    return;
                }
                Ok(true) => {
                    self.sum.done += 1;
                    self.advance(0);
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
                Err(e) => {
                    let network = self.on_network(dst) || self.on_network(src);
                    if transient(&e, network) && tries < self.backoff.len() {
                        let wait = self.backoff[tries];
                        tries += 1;
                        if crate::debuglog::on(crate::debuglog::Topic::Errors) {
                            crate::debuglog::Event::new("retry")
                                .num("job", self.dbg_job())
                                .file(dst)
                                .word("phase", self.phase)
                                .error(&e)
                                .num("attempt", tries as u64)
                                .num("of", self.backoff.len() as u64)
                                .ms("wait_ms", wait)
                                .write();
                        }
                        let _ = self.tx.send(Event::Retrying {
                            path: dst.to_path_buf(),
                            error: e.to_string(),
                            attempt: tries as u32,
                            of: self.backoff.len() as u32,
                            wait,
                        });
                        if self.wait(wait) {
                            continue;
                        }
                        return;
                    }
                    if crate::debuglog::on(crate::debuglog::Topic::Errors) {
                        crate::debuglog::Event::new("failed")
                            .num("job", self.dbg_job())
                            .file(dst)
                            .word("phase", self.phase)
                            .error(&e)
                            .num("tries", tries as u64 + 1)
                            .flag("network", network)
                            .write();
                    }
                    let error = if tries > 0 {
                        format!("{e} (a network error: tried {} times)", tries + 1)
                    } else {
                        e.to_string()
                    };
                    self.failed_file(dst, &e, error);
                    if self.retry {
                        self.retry = false;
                        tries = 0;
                        continue;
                    }
                    return;
                }
            }
        }
    }

    /// A file failed for good: the user says whether to try again (`retry` set), skip
    /// it, or stop the job.
    pub(super) fn failed_file(&mut self, dst: &Path, e: &io::Error, error: String) {
        match self.ask(Question::Failed {
            path: dst.to_path_buf(),
            error,
        }) {
            Answer::Retry => self.retry = true,
            Answer::Skip | Answer::SkipAll => {
                self.note_error(dst, e);
                self.sum.skipped += 1;
            }
            _ => {
                self.note_error(dst, e);
                self.abort = true;
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
    pub(super) fn copy_bytes(
        &mut self,
        src: &Path,
        dst: &Path,
        new_only: bool,
    ) -> io::Result<bool> {
        self.phase = "open";
        let mut t = self.clock();
        let meta = fs::metadata(src)?;
        let mut r = File::open(src)?;
        self.lap(Phase::Open, &mut t);
        self.phase = "create";
        let in_place = new_only && !self.safe;
        let target = if in_place {
            dst.to_path_buf()
        } else {
            part_name(dst)
        };
        self.moved_file = None;
        self.engine = crate::fastcopy::Engine::Own;
        writing_in_test(&target);
        if !in_place {
            if let Some(j) = self.journal.as_mut() {
                // A failure breaks the journal; asked about before a step that needs it.
                let _ = j.part(&target);
            }
        }
        let resumed = if in_place {
            None
        } else {
            self.take_over(src, dst, &target)
        };
        // Checkpoints: a big file under a temporary name, with a journal to keep them.
        self.ckpt = (!in_place
            && self.checkpoint > 0
            && self.journal.is_some()
            && (meta.len() > self.checkpoint || resumed.is_some()))
        .then(|| Ckpt {
            part: crate::journal::Partial {
                part: target.clone(),
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                ident: Ident::of(&meta),
                bytes: resumed.unwrap_or(0),
            },
            next: resumed.unwrap_or(0) + self.checkpoint,
        });
        // Taken over under this run's name: said at once, or a kill before the next
        // checkpoint would take it for an unfinished file and remove it.
        if let (Some(c), Some(j), Some(_)) = (&self.ckpt, self.journal.as_mut(), resumed) {
            let _ = j.checkpoint(&c.part);
        }
        // macOS clones by making the file itself, so before it is created (item 22).
        #[cfg(target_vendor = "apple")]
        let cloned = resumed.is_none() && self.fast && crate::fastcopy::clone_at(&r, &target);
        #[cfg(not(target_vendor = "apple"))]
        let cloned = false;
        let opened = if let Some(at) = resumed {
            // Taken over: what is past the checkpoint may not be on disk whole, so it
            // is cut off, and both files go on from there.
            fs::OpenOptions::new()
                .write(true)
                .open(&target)
                .and_then(|mut w| {
                    use std::io::Seek;
                    w.set_len(at)?;
                    w.seek(io::SeekFrom::Start(at))?;
                    r.seek(io::SeekFrom::Start(at))?;
                    Ok(w)
                })
                .inspect(|_| {
                    self.advance(at);
                    if let Some(d) = self.dbg.as_mut() {
                        d.resumed_bytes += at;
                    }
                })
        } else if cloned {
            fs::OpenOptions::new().write(true).open(&target)
        } else {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
        };
        let mut w = match opened {
            Ok(w) => w,
            Err(e) => {
                if cloned || resumed.is_some() {
                    let _ = fs::remove_file(&target);
                }
                self.ckpt = None;
                return Err(e);
            }
        };
        self.lap(Phase::Create, &mut t);
        self.phase = "write";
        let written = if cloned {
            crate::faults::at(crate::faults::Point::Write, &target).map(|()| {
                self.advance(meta.len());
                self.engine = crate::fastcopy::Engine::Clone;
                true
            })
        } else {
            self.write_all_from(&mut r, &mut w, &target)
        };
        self.ckpt = None;
        self.lap(Phase::Write, &mut t);
        if matches!(written, Ok(true)) {
            match self.engine {
                crate::fastcopy::Engine::Clone => self.sum.cloned += 1,
                crate::fastcopy::Engine::Kernel => self.sum.by_kernel += 1,
                crate::fastcopy::Engine::Own => {}
            }
        }
        // A new file on a network target is synced before its rename (the owner's
        // decision 3, Round 031): SMB and NFS report a failed write only at the sync or
        // the close, and a dropped handle hides the close's error. A replacement is
        // synced anyway; a move's copies are synced with their batch.
        let written = match written {
            Ok(true) if new_only && !self.moving && self.on_network(dst) => {
                self.phase = "sync";
                let r = crate::faults::sync_data(&w, &target).map(|()| true);
                self.lap(Phase::Sync, &mut t);
                r
            }
            other => other,
        };
        let kept = match written {
            Ok(true) => {
                // Permissions and mtime are part of the file as far as a user is
                // concerned; a copy that loses the executable bit broke something.
                let _ = fs::set_permissions(&target, meta.permissions());
                if let Ok(mtime) = meta.modified() {
                    let _ = w.set_times(FileTimes::new().set_modified(mtime));
                }
                self.lap(Phase::Attrs, &mut t);
                self.phase = "finish";
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
                    self.lap(Phase::Finish, &mut t);
                    if r.is_ok() {
                        if self.moving || self.on_network(dst) {
                            // A move's copies sync with their batch; a network file
                            // was synced before its rename.
                            keep(self, w);
                        } else {
                            self.fresh_file(dst, w, meta.len());
                        }
                    }
                    r
                } else if self.group_takes(dst, meta.len()) {
                    // Group commit (Round 033): synced with its batch, then finished as
                    // below ([`Worker::flush_group`]). The temporary file stays.
                    self.group.pending.push(Pending {
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        tmp: target.clone(),
                        file: w,
                        size: meta.len(),
                        agreed: self.agreed.clone(),
                    });
                    self.group.bytes += meta.len();
                    self.group.since.get_or_insert_with(std::time::Instant::now);
                    self.group.batched += 1;
                    self.group.deferred = true;
                    return Ok(true);
                } else {
                    // Removes the temporary file itself on any failure.
                    self.phase = "sync";
                    let synced = crate::faults::sync_data(&w, &target);
                    drop(w);
                    self.lap(Phase::Sync, &mut t);
                    if let Err(e) = synced {
                        let _ = fs::remove_file(&target);
                        return Err(e);
                    }
                    self.phase = "finish";
                    let r = self.finish_replace(src, &target, dst);
                    self.lap(Phase::Finish, &mut t);
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

    /// The last steps of replacing `dst` with `tmp`, complete and synced (Round 029's
    /// order): the journal's `swap` line where the old file goes to the trash, the
    /// journal on disk, the target checked once more, the old file to the trash, the
    /// rename. Whatever does not end in the rename removes `tmp`, and an old file
    /// already in the trash comes back. The file there is checked right before the
    /// rename: no portable rename compares first, so a window of microseconds is left,
    /// not the whole copy.
    pub(super) fn finish_replace(
        &mut self,
        src: &Path,
        tmp: &Path,
        dst: &Path,
    ) -> io::Result<bool> {
        let trash = self.replace_into_trash(dst);
        if trash {
            if let Some(j) = self.journal.as_mut() {
                let _ = j.swap(tmp, dst);
            }
        }
        // The journal first: its question can be open for minutes, and the look at the
        // target is the last thing before the rename.
        if !self.durable(dst) {
            let _ = fs::remove_file(tmp);
            return Ok(false);
        }
        self.rename_into(src, tmp, dst, trash)
    }

    /// The steps after the journal is on disk: the target checked once more, the old
    /// file to the trash (`trash`), the rename. See [`Worker::finish_replace`].
    pub(super) fn rename_into(
        &mut self,
        src: &Path,
        tmp: &Path,
        dst: &Path,
        trash: bool,
    ) -> io::Result<bool> {
        self.old_trashed = None;
        let r = (|| {
            if !self.still_agreed(Some(src), dst) {
                return Ok(false);
            }
            // #5, once more at the last moment: a folder made there while the file was
            // copied is never sent to the trash for it (also reached from a batch).
            // Counted and said here: the callers drop an `Ok(false)` without a word.
            if let Some(why) = other_kind(Some(src), dst) {
                self.sum.skipped += 1;
                self.note_skip(dst, why);
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
            crate::faults::at(crate::faults::Point::Rename, dst)?;
            fs::rename(tmp, dst)?;
            Ok(true)
        })();
        if !matches!(r, Ok(true)) {
            let _ = fs::remove_file(tmp);
            // The rename failed after the old file went: it comes back.
            if let Some(t) = self.old_trashed.take() {
                let _ = crate::trashed::restore(&t);
            }
        }
        r
    }

    /// Whether a replacement of `size` bytes at `dst` waits for a batch (Round 033):
    /// small, on a local drive, not moving (a move batches its own way), and not a
    /// batch's own fallback.
    pub(super) fn group_takes(&mut self, dst: &Path, size: u64) -> bool {
        !self.group.off && !self.moving && size <= GROUP_FILE_MAX && !self.on_network(dst)
    }

    /// Sync the waiting batch, then finish each of its files in order, exactly as a
    /// replacement on its own is finished ([`Worker::finish_replace`]). A file whose
    /// sync failed, or whose last steps did not end in the rename, is done again on its
    /// own from its source (a failed sync is never tried again on the same file:
    /// fsyncgate); that way asks again if the file there changed, and asks about an
    /// error that stays. Stopped: the temporary files go, and the old files stay.
    pub(super) fn flush_group(&mut self) {
        if self.group.pending.is_empty() || self.group.flushing {
            return;
        }
        self.group.flushing = true;
        let failed_before = self.sum.failed;
        let batch = std::mem::take(&mut self.group.pending);
        self.group.bytes = 0;
        self.group.since = None;
        if self.stopped() {
            for p in batch {
                drop(p.file);
                let _ = fs::remove_file(&p.tmp);
            }
            self.finished.retain(|w| !w.group);
            self.group.flushing = false;
            return;
        }
        let threads = self.group.threads_now();
        let probing = self.group.probe.is_some();
        self.phase = "sync";
        let started = std::time::Instant::now();
        let synced = sync_all(&batch, threads);
        let took = started.elapsed();
        let ms = took.as_secs_f64() * 1000.0;
        self.group.measured(batch.len(), ms);
        if let Some(d) = self.dbg.as_mut() {
            d.batches += 1;
            d.phases[Phase::Sync as usize] += took;
            crate::debuglog::Event::new("batch")
                .num("job", d.job)
                .num("files", batch.len() as u64)
                .num("bytes", batch.iter().map(|p| p.size).sum())
                .num("threads", threads as u64)
                .ms("sync_ms", took)
                .num(
                    "sync_failed",
                    synced.iter().filter(|e| e.is_some()).count() as u64,
                )
                .flag("probing", probing)
                .write();
            if probing && self.group.probe.is_none() {
                let mut e = crate::debuglog::Event::new("probe")
                    .num("job", d.job)
                    .num("chose", self.group.threads as u64);
                for (n, per) in &self.group.tried {
                    e = e.float(&format!("ms_per_file_{n}"), *per);
                }
                e.write();
            }
        }
        let mut t = self.clock();
        self.phase = "finish";
        // The journal's lines for the whole batch (a `swap` where the old file goes to
        // the trash), then one commit for them all, before any old file goes or any
        // rename: each file's order is Round 029's, the journal's sync is shared.
        let mut trash = Vec::with_capacity(batch.len());
        for (p, failed) in batch.iter().zip(&synced) {
            let to_trash = failed.is_none() && self.replace_into_trash(&p.dst);
            if to_trash {
                if let Some(j) = self.journal.as_mut() {
                    let _ = j.swap(&p.tmp, &p.dst);
                }
            }
            trash.push(to_trash);
        }
        let any = synced.iter().any(Option::is_none);
        let journal = !any || self.stopped() || self.durable(&batch[0].dst);
        for ((p, failed), to_trash) in batch.into_iter().zip(synced).zip(trash) {
            let Pending {
                src,
                dst,
                tmp,
                file,
                size,
                agreed,
            } = p;
            drop(file);
            let r = match failed {
                Some(e) => {
                    let _ = fs::remove_file(&tmp);
                    Err(e)
                }
                None if self.stopped() || !journal => {
                    let _ = fs::remove_file(&tmp);
                    Ok(false)
                }
                None => {
                    self.agreed = agreed;
                    self.rename_into(&src, &tmp, &dst, to_trash)
                }
            };
            // As `copy_file` takes the same outcomes for a file on its own.
            let again = match r {
                Ok(true) => {
                    self.sum.done += 1;
                    self.advance(0);
                    if let Some(t) = self.old_trashed.take() {
                        self.sum.replaced_to_trash += 1;
                        self.made(&dst, Some(t));
                    } else if self.recording {
                        self.sum.replaced_for_good += 1;
                    }
                    false
                }
                // Not the file agreed to any more: back to the question.
                Ok(false) => self.changed.as_deref() == Some(dst.as_path()),
                // Said and asked about: Retry writes it again from its source.
                Err(e) => {
                    self.failed_file(&dst, &e, e.to_string());
                    std::mem::take(&mut self.retry)
                }
            };
            if again && !self.stopped() {
                // Its bytes are counted again as it is written again.
                self.bytes_done = self.bytes_done.saturating_sub(size);
                self.lap(Phase::Finish, &mut t);
                self.group.off = true;
                self.copy_file(&src, &dst, size);
                self.group.off = false;
                t = self.clock();
            }
        }
        self.lap(Phase::Finish, &mut t);
        self.group.flushing = false;
        // The folders whose last files were in this batch: finished, if it went through
        // without a failure. Otherwise a resume walks them again, which is only slower.
        let ok = self.sum.failed == failed_before && !self.stopped();
        self.settle(ok, |w| &mut w.group);
    }

    /// What a folder waits on (`which` of [`Waiting`]) is through, `ok` if without a
    /// failure: the folders waiting on it are dropped if not, else freed of it, and
    /// those waiting on nothing more are told to the journal.
    fn settle(&mut self, ok: bool, which: fn(&mut Waiting) -> &mut bool) {
        let mut tell = Vec::new();
        self.finished.retain_mut(|w| {
            if !*which(w) {
                return true;
            }
            if !ok {
                return false;
            }
            *which(w) = false;
            if w.group || w.fresh {
                return true;
            }
            tell.push(std::mem::take(&mut w.dir));
            false
        });
        for d in tell {
            self.tell_finished(&d);
        }
    }

    /// A new file that took its real name, `w` its handle (B3): kept for its batch's
    /// sync, which runs once the batch is full or its first file has waited long
    /// enough. Without safe operations a new file is not synced, as before.
    pub(super) fn fresh_file(&mut self, dst: &Path, w: File, size: u64) {
        if !self.safe {
            return;
        }
        let f = &mut self.fresh;
        f.files.push((dst.to_path_buf(), w));
        f.bytes += size;
        let since = *f.since.get_or_insert_with(std::time::Instant::now);
        if f.files.len() >= FRESH_BATCH_FILES
            || f.bytes >= FRESH_BATCH_BYTES
            || since.elapsed() >= GROUP_BATCH_AGE
        {
            self.sync_fresh();
        }
    }

    /// B3: the new files and folders so far to the disk, data and names, then the
    /// sources and folders waiting on them told to the journal as done, if nothing in
    /// the batch failed. A file whose sync failed is reported (its data may be lost:
    /// fsyncgate) and nothing waiting is told, so a resume goes over it again.
    pub(super) fn sync_fresh(&mut self) {
        if self.fresh.is_empty() {
            return;
        }
        let fresh = std::mem::take(&mut self.fresh);
        let failed_before = self.sum.failed;
        let mut t = self.clock();
        // Run in the middle of another file's copy too: its step is put back after.
        let phase = std::mem::replace(&mut self.phase, "sync");
        let threads = self.group.threads_now();
        let errors = sync_each(&fresh.files, threads, |(path, file)| {
            crate::faults::sync_data(file, path)
        });
        for ((path, _), e) in fresh.files.iter().zip(errors) {
            if let Some(e) = e {
                // Never synced again (fsyncgate): the copy goes, as a move's does, and
                // it is not counted done; its folder is not told finished.
                let _ = remove_file_anyway(path);
                self.sum.done = self.sum.done.saturating_sub(1);
                self.note_error(path, &e);
            }
        }
        // The names: each folder a new file or folder went into.
        let folders: std::collections::BTreeSet<&Path> = fresh
            .files
            .iter()
            .map(|(p, _)| p.as_path())
            .chain(fresh.dirs.iter().map(PathBuf::as_path))
            .filter_map(Path::parent)
            .collect();
        for d in folders {
            if let Err(e) = crate::faults::sync_dir(d) {
                self.note_error(d, &e);
            }
        }
        drop(fresh);
        self.lap(Phase::Sync, &mut t);
        self.phase = phase;
        let ok = self.sum.failed == failed_before && !self.stopped();
        let sources = std::mem::take(&mut self.sources_waiting);
        if ok {
            for src in sources {
                if let Some(j) = self.journal.as_mut() {
                    let _ = j.done(&src);
                }
            }
        }
        self.settle(ok, |w| &mut w.fresh);
    }

    /// A folder of a copy finished without a failure: a resume need not walk it again.
    /// Told now, or once the batch holding its last files is through. Copies only: a
    /// move removes what it finished, and a resume finds nothing there to walk.
    pub(super) fn folder_finished(&mut self, src: &Path) {
        if !self.copying || self.moving || self.journal.is_none() {
            return;
        }
        let (group, fresh) = (!self.group.pending.is_empty(), !self.fresh.is_empty());
        if group || fresh {
            self.finished.push(Waiting {
                dir: src.to_path_buf(),
                group,
                fresh,
            });
        } else {
            self.tell_finished(src);
        }
    }

    pub(super) fn tell_finished(&mut self, src: &Path) {
        #[cfg(test)]
        TOLD.lock().unwrap().push(src.to_path_buf());
        if let Some(j) = self.journal.as_mut() {
            if j.done(src).is_ok() {
                if let Some(d) = self.dbg.as_mut() {
                    d.dirs_done += 1;
                }
            }
        }
    }

    /// Copy `r` into `w`, which is `path`; Ok(false) when cancelled part-way.
    /// The kept temporary files no file of this job took over: removed.
    pub(super) fn drop_partials(&mut self) {
        for (_, p) in self.partials.drain() {
            if p.part
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains(".xdevcmd-part-"))
            {
                let _ = fs::remove_file(&p.part);
            }
        }
    }

    /// Resume: the kept start of `dst`'s copy, renamed to `target` (this run's name for
    /// it), and how many bytes of it count. Only while the source is the same file it
    /// was (its size, date and identity) and the kept file holds what its checkpoint
    /// says. Otherwise the kept file goes, and the copy starts again.
    pub(super) fn take_over(&mut self, src: &Path, dst: &Path, target: &Path) -> Option<u64> {
        let p = self.partials.remove(dst)?;
        let same = p.src == src
            && Ident::at(src) == Some(p.ident)
            && p.bytes <= fs::metadata(src).map(|m| m.len()).unwrap_or(0)
            && fs::symlink_metadata(&p.part).is_ok_and(|m| m.is_file() && m.len() >= p.bytes)
            // C trusts nothing the run that died made: the kept bytes are read again
            // with the source's (repo review RR01: a damaged part was finished whole).
            && (!self.verify || self.same_upto(src, &p.part, Some(p.bytes)));
        if same && fs::rename(&p.part, target).is_ok() {
            return Some(p.bytes);
        }
        let _ = fs::remove_file(&p.part);
        None
    }

    /// `n` more bytes are in the big file being written (`w`): at a checkpoint, its data
    /// is synced and then the journal says how far it is. A failed sync fails the copy,
    /// which goes round as any write error; a failed line only breaks the journal,
    /// which is asked about before the next step that needs it.
    pub(super) fn tick(&mut self, w: &File, n: u64) -> io::Result<()> {
        // B3: new files before it wait no longer than a batch may, a big file or not.
        if self
            .fresh
            .since
            .is_some_and(|s| s.elapsed() >= GROUP_BATCH_AGE)
        {
            self.sync_fresh();
        }
        let Some(c) = self.ckpt.as_mut() else {
            return Ok(());
        };
        c.part.bytes += n;
        if c.part.bytes < c.next {
            return Ok(());
        }
        // A checkpoint is a point a resume goes on from: what was copied before it is
        // on the disk first (B3).
        if !self.fresh.is_empty() {
            self.sync_fresh();
        }
        let Some(c) = self.ckpt.as_mut() else {
            return Ok(());
        };
        let t = std::time::Instant::now();
        crate::faults::sync_data(w, &c.part.part)?;
        c.next = c.part.bytes + self.checkpoint;
        if let Some(j) = self.journal.as_mut() {
            let _ = j.checkpoint(&c.part);
        }
        if let Some(d) = self.dbg.as_mut() {
            d.checkpoints += 1;
            d.checkpoint_time += t.elapsed();
        }
        Ok(())
    }

    pub(super) fn write_all_from(
        &mut self,
        r: &mut File,
        w: &mut File,
        path: &Path,
    ) -> io::Result<bool> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.fast {
            if let Some(done) = self.kernel_copy(r, w, path)? {
                return Ok(done);
            }
        }
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
            self.tick(w, n as u64)?;
        }
        w.flush()?;
        Ok(true)
    }

    /// Linux's fast copy (item 22): a clone, else `copy_file_range`. `None`: neither
    /// would, and the file's offsets are where the kernel stopped, so read and write
    /// go on from there. A fault planted at `Write` is met once per step taken.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(super) fn kernel_copy(
        &mut self,
        r: &File,
        w: &File,
        path: &Path,
    ) -> io::Result<Option<bool>> {
        use crate::fastcopy::{Engine, Step};
        // A clone is the whole file: not for one taken over part way.
        let from_start = self.ckpt.as_ref().is_none_or(|c| c.part.bytes == 0);
        if from_start && crate::fastcopy::clone_into(r, w) {
            crate::faults::at(crate::faults::Point::Write, path)?;
            self.advance(r.metadata()?.len());
            self.engine = Engine::Clone;
            return Ok(Some(true));
        }
        let mut first = true;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Some(false));
            }
            match crate::fastcopy::kernel_step(r, w, KERNEL_CHUNK, first)? {
                Step::Copied(n) => {
                    crate::faults::at(crate::faults::Point::Write, path)?;
                    self.advance(n as u64);
                    self.tick(w, n as u64)?;
                    first = false;
                }
                Step::End => {
                    self.engine = Engine::Kernel;
                    return Ok(Some(true));
                }
                Step::Refused => return Ok(None),
            }
        }
    }

    // ── move ───────────────────────────────────────────────────────────────────────

    pub(super) fn move_into(
        &mut self,
        src: &Path,
        dest_dir: &Path,
        rename: Option<&std::ffi::OsStr>,
    ) {
        self.phase = "move";
        self.approved = None;
        let Some(name) = rename.or(src.file_name()) else {
            return;
        };
        let dst = dest_dir.join(name);
        if crate::plan::onto_itself(src, &dst) {
            self.sum.skipped += 1;
            return;
        }
        if crate::plan::into_itself(src, &dst, dest_dir) {
            let e = io::Error::other("cannot move a directory into itself");
            self.note_error(src, &e);
            return;
        }
        // A folder onto a folder is a merge, as the plan shows and as a move across
        // drives does: each entry moves into the folder there, and the source folder
        // goes once empty. A rename cannot do it: it refuses a folder that is not
        // empty (QA 2026-11-17; before, "0 moved, 1 failed").
        let real_dir = |p: &Path| p.symlink_metadata().is_ok_and(|m| m.is_dir());
        if real_dir(src) && real_dir(&dst) {
            self.merge_into(src, &dst);
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
                self.move_over(src, &dst)
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
            Err(e) if crate::os::is_cross_device(&e) => {
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

    /// Move each entry of the folder `src` into the folder `dst`, by name order, then
    /// remove `src` if that left it empty: what was skipped or failed keeps it.
    fn merge_into(&mut self, src: &Path, dst: &Path) {
        self.sum.merged = true;
        let mut entries: Vec<PathBuf> = match fs::read_dir(src) {
            Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
            Err(e) => {
                self.note_error(src, &e);
                return;
            }
        };
        entries.sort();
        for child in entries {
            if self.abort || self.cancel.load(Ordering::Relaxed) {
                return;
            }
            self.move_into(&child, dst, None);
        }
        let _ = fs::remove_dir(src);
    }

    /// A move by rename over an existing `dst` the user agreed to replace. Where the
    /// trash is on its volume, a file there goes to the trash first, as a copy's does
    /// (Round 031); then `src` takes the name only if it is still free. A rename that
    /// fails puts the old file back, so the cross-drive fallback (EXDEV) and any error
    /// leave both files as they were. Before (QA 2026-11-17, P0) the rename replaced the
    /// old file for good. A folder there is never sent: #5 refuses a file over a folder,
    /// and a folder onto a folder is not a replacement (the rename fails, nothing lost).
    pub(super) fn move_over(&mut self, src: &Path, dst: &Path) -> io::Result<()> {
        let old_is_file = dst.symlink_metadata().is_ok_and(|m| !m.is_dir());
        if !old_is_file || !self.replace_into_trash(dst) {
            return fs::rename(src, dst);
        }
        let t = crate::faults::at(crate::faults::Point::Trash, dst)
            .and_then(|()| send_to_trash(dst, self.trash_root.as_deref()))
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("the old file could not go to the trash: {e}"),
                )
            })?;
        let r = crate::faults::at(crate::faults::Point::Rename, dst)
            .and_then(|()| crate::noreplace::rename_noreplace(src, dst));
        match r {
            Ok(()) => {
                self.sum.replaced_to_trash += 1;
                Ok(())
            }
            Err(e) => {
                let _ = crate::trashed::restore(&t);
                Err(e)
            }
        }
    }
}

/// #5: why `src` may not replace `dst`, when one is a folder and the other is not. A
/// link is not a folder here (replacing a link leaves its folder alone); no source (an
/// archive's entry) is a file.
fn other_kind(src: Option<&Path>, dst: &Path) -> Option<&'static str> {
    let dir = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
    match (src.is_some_and(dir), dir(dst)) {
        (false, true) => Some("is a folder \u{2014} a file never replaces it"),
        (true, false) => Some("is a file \u{2014} a folder never replaces it"),
        _ => None,
    }
}

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
