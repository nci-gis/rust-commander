//! The event loop: run, one turn of it, background messages, stopping and signals.

use super::*;

impl App {
    pub fn run(mut self, terminal: &mut Term) -> Result<()> {
        self.interactive = true;
        self.watcher = crate::watch::Watcher::new();
        self.graphics_found = crate::graphics::detect(
            &self.settings.config.viewer.images,
            |k| std::env::var(k).ok(),
            &self.probe,
        );
        self.cell_known = crate::graphics::fixed_sixel_cell(|k| std::env::var(k).ok());
        if let Some((h, w)) = self.probe.cell {
            self.cell_px = (w, h);
            self.cell_known = true;
        }
        self.measure_cells(terminal);
        let pictures = match (self.graphics, self.graphics_found) {
            (crate::graphics::Protocol::Kitty, _) => "Kitty",
            (crate::graphics::Protocol::Iterm2, _) => "iTerm2",
            (crate::graphics::Protocol::Sixel, _) => "sixel",
            (_, crate::graphics::Protocol::Sixel) => {
                "half blocks (sixel needs the cell size, which the terminal did not give)"
            }
            (crate::graphics::Protocol::Blocks, _) => "half blocks",
        };
        let name = crate::graphics::terminal_name(|k| std::env::var(k).ok(), &self.probe);
        self.usage.terminal = format!("{name} \u{b7} pictures: {pictures}");
        // The terminal closed (SIGHUP) or a polite kill (SIGTERM): the loop sees the flag
        // and ends tidily. A second signal ends xdevcmd at once, as the default would.
        #[cfg(unix)]
        for sig in [signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM] {
            // Tests: the signal is caught and dropped, so that only the lost terminal
            // itself (EIO from a read or a write) can end the loop, as when the error
            // wins the race with the flag.
            if std::env::var_os("XDEVCMD_TEST_NO_SIGNALS").is_some() {
                let swallowed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let _ = signal_hook::flag::register(sig, swallowed);
                continue;
            }
            let _ = signal_hook::flag::register_conditional_default(sig, self.hangup.clone());
            let _ = signal_hook::flag::register(sig, self.hangup.clone());
        }
        // Whatever the setting is now: what a killed run left (rename many's files under
        // temporary names above all) is put right even if safe operations were turned
        // off since. Without journals this finds nothing.
        self.recover_interrupted();
        // What a killed run left comes first; a config that failed is said on the status
        // line then, as before, and kept in Messages.
        if self.modal.is_none() {
            self.ask_about_config();
        }
        let size = terminal.size()?;
        self.boot(Rect::new(0, 0, size.width, size.height));
        // What a killed run left, tidied up, and the shell panel started, if it was open.
        self.startup.mark("shell");
        // Said once, unless start-up had something to say: where xdevcmd is running. It
        // gives way at the first key or click to the line it covers (the pane's own).
        if self.status.is_empty() && self.modal.is_none() {
            self.status = format!("running in {}", self.usage.terminal);
            self.boot_note = Some(self.status.clone());
        }

        // Drawn only when something changed: input, a shell's output, background work
        // reporting in, a job's progress; or after HEARTBEAT, whatever happened.
        let mut dirty = true;
        let mut drawn = Instant::now();
        // The terminal failing under the loop (EIO once it is closed, often before the
        // SIGHUP flag is seen): treated as the hang-up it is, jobs cancelled and cleaned
        // up and the session saved, and only then returned.
        let mut lost: Option<anyhow::Error> = None;
        while !self.quit {
            if self.hangup.load(std::sync::atomic::Ordering::Relaxed) {
                self.end_on_signal();
                break;
            }
            if let Err(e) = self.turn(terminal, &mut dirty, &mut drawn) {
                self.end_on_signal();
                lost = Some(e);
                break;
            }
        }
        // Shown, or kept by the terminal under a dialog: freed, or the terminal holds
        // the picture's data after xdevcmd is gone.
        let shown = self.graphic_shown.take().is_some();
        if (shown || self.kitty_held.take().is_some())
            && self.graphics == crate::graphics::Protocol::Kitty
        {
            let _ = terminal
                .backend_mut()
                .raw(&crate::graphics::kitty_delete(KITTY_ID));
        }
        // A log's last lines and footer reach its file before xdevcmd ends.
        self.close_logs();
        self.save_session();
        if let Some(e) = lost {
            return Err(e);
        }
        if self.quit_cd {
            self.write_cwd_file();
        }
        Ok(())
    }

    /// One turn of the event loop: background work in, a frame out if anything changed,
    /// at most one event handled. An error is the terminal failing.
    /// One step of quitting with jobs (`Cmd::QuitAnyway`): pump them, answer any
    /// question with Abort, drop the finished. All gone: quit. Past [`STOP_WAIT`] with
    /// one still there (a worker stuck in a write, a hung network drive): stop waiting
    /// and ask, rather than walk away from it unasked. Whether the screen changed.
    pub(super) fn stop_step(&mut self) -> bool {
        let Some(deadline) = self.stopping else {
            return false;
        };
        for j in &mut self.jobs {
            j.pump();
            if j.question.take().is_some() {
                j.answer(xdevcmd_core::Answer::Abort);
            }
        }
        self.jobs.retain(|j| j.done.is_none());
        let n = self.jobs.len();
        if n == 0 {
            self.stopping = None;
            self.quit = true;
            self.quit_cd = true;
        } else if Instant::now() >= deadline {
            self.stopping = None;
            let safe = self.settings.config.general.safe_operations;
            self.modal = Some(Modal::Ask(modal::Ask {
                title: "Still stopping".into(),
                body: vec![
                    format!(
                        "{n} job{} has not stopped after {} seconds.",
                        if n == 1 { "" } else { "s" },
                        STOP_WAIT.as_secs()
                    ),
                    if safe {
                        "Quitting now leaves its temporary file; the next start removes it."
                    } else {
                        "Quitting now may leave a file half written."
                    }
                    .into(),
                ],
                danger: true,
                yes: Some(("quit anyway".into(), Cmd::QuitNow)),
                buttons: Vec::new(),
                also: None,
                more: None,
                footnote: None,
                pictures: Vec::new(),
            }));
        } else {
            self.status = format!(
                "stopping: waiting for {n} job{} to clean up\u{2026}",
                if n == 1 { "" } else { "s" }
            );
        }
        true
    }

    pub(super) fn turn(
        &mut self,
        terminal: &mut Term,
        dirty: &mut bool,
        drawn: &mut Instant,
    ) -> Result<()> {
        self.pump_job();
        *dirty |= self.stop_step();
        *dirty |= self.reap_terminals();
        *dirty |= self.log_notes();
        *dirty |= self.background();
        *dirty |= crate::pane::term::take_output();
        // A job's progress moves on its own, with no message to say so; a picture
        // waiting for the cursor to settle starts decoding on a later frame.
        *dirty |= matches!(self.modal, Some(Modal::Job(_))) || !self.jobs.is_empty();
        *dirty |= self.settling;
        if *dirty || drawn.elapsed() >= HEARTBEAT {
            let t0 = Instant::now();
            terminal.draw(|f| self.draw(f))?;
            self.sync_graphics(terminal)?;
            self.usage.frame(t0.elapsed());
            *drawn = Instant::now();
            *dirty = false;
            // The first frame is on screen: start-up is over. With --timings the
            // status line says how long it took, in place of where xdevcmd runs.
            if self.startup.first_frame() {
                self.usage.startup = Some((self.startup.total_text(), self.startup.steps_line()));
                if self.timings {
                    let before = self.boot_note.take();
                    if self.status.is_empty() || before.as_deref() == Some(self.status.as_str()) {
                        self.status = self.startup.line();
                        self.boot_note = Some(self.status.clone());
                        *dirty = true;
                    }
                }
            }
        }
        let input = self.input.get_or_insert_with(KeyReader::new);
        if let Some(event) = input.next(TICK)? {
            // Any event, a resize or a focus change included, may change the screen.
            *dirty = true;
            if matches!(event, Event::Key(_) | Event::Mouse(_)) {
                if let Some(note) = self.boot_note.take() {
                    if self.status == note {
                        self.status.clear();
                    }
                }
            }
            match event {
                // Quitting: the jobs are being stopped; nothing else is started.
                Event::Key(_) | Event::Mouse(_) if self.stopping.is_some() => {}
                Event::Key(k) if k.kind == KeyEventKind::Press => self.on_key(k),
                Event::Mouse(m) => self.on_mouse(m),
                // A resize clears the screen: whatever picture was there is gone, and
                // the cells may be a different size now.
                Event::Resize(..) => {
                    self.graphic_shown = None;
                    // Sent again, whole: a new transmission replaces the old one.
                    self.kitty_held = None;
                    self.measure_cells(terminal);
                }
                _ => {}
            }
        }
        if let Some(path) = self.pending_edit.take() {
            // Every way to the editor comes here — F4, Shift+F4 on a name that exists,
            // Open with… — so the root is checked here once. The list of names for
            // renaming many lives in xdevcmd's own temporary directory.
            let own = crate::tempfiles::dir().is_some_and(|d| path.starts_with(&d));
            if !own && !xdevcmd_core::policy::get().allows(&path) {
                self.status = "that file is outside the root xdevcmd is confined to".into();
                self.pending_batch = None;
                return Ok(());
            }
            // Nothing may read the terminal while the editor has it.
            if let Some(input) = self.input.as_mut() {
                input.quiet()?;
            }
            self.edit_externally(terminal, &path)?;
            if self.recheck_config.as_deref() == Some(path.as_path()) {
                self.recheck_config = None;
                self.config_checked_again(&path);
            }
            // F4 from a shell's history: the history is still up, over the status line.
            if let Some(Modal::View(v)) = self.modal.as_mut() {
                v.note(&self.status);
            }
            if let Some((files, list)) = self.pending_batch.take() {
                self.finish_batch(files, &list);
            }
        }
        Ok(())
    }

    /// Work that happens between keys: directory sizes arriving, directories that
    /// changed on disk, and messages from programs started in the background. True when
    /// any of it changed what is on screen.
    pub(super) fn background(&mut self) -> bool {
        let mut changed = self.pump_pending_type();
        // The viewer drew lines plain while its scan was behind them; it has moved on.
        if let Some(Modal::View(v)) = &self.modal {
            changed |= v.syntax_moved();
        }
        changed |= self.follow_shell();
        // A thread other than this one panicked: xdevcmd goes on, and says where the
        // details went.
        if let Some(log) = crate::crash::take_background() {
            self.status = match log {
                Some(p) => format!(
                    "a background task failed \u{2014} details in {}",
                    p.display()
                ),
                None => "a background task failed (no crash log could be written)".into(),
            };
            changed = true;
        }
        // Not under an open dialog or a drag: a listing that shifted while you were
        // answering "delete these?" or dragging a row is the wrong moment to move it.
        let quiet = self.modal.is_none() && self.drag.is_none();
        let every = Duration::from_millis(self.settings.config.files.auto_refresh_ms);
        let shown: Vec<PaneId> = self.rects.iter().map(|(id, _)| *id).collect();
        self.watch_panes(&shown, every);
        for (id, p) in self.panes.iter_mut() {
            if let Some(t) = p.as_tabs_mut() {
                for f in t.tabs_mut() {
                    changed |= f.poll_sizes();
                    changed |= f.poll_git();
                    changed |= f.poll_opening();
                    changed |= f.poll_reading();
                    let was = f.results.as_ref().is_some_and(|r| r.running());
                    changed |= f.poll_results();
                    if let Some(r) = f.results.as_ref().filter(|r| was && !r.running() && r.find) {
                        let n = f.entries.iter().filter(|e| !e.is_parent()).count();
                        let skipped = match r.unreadable {
                            0 => String::new(),
                            u => format!(", {u} unreadable"),
                        };
                        self.status = format!(
                            "{n} found \u{b7} {} looked at{skipped} \u{b7} Enter goes to a file, F3 views it",
                            r.scanned
                        );
                    }
                }
                if quiet && shown.contains(id) {
                    let f = t.active_mut();
                    if self.to_reload.remove(id) {
                        f.reload();
                        changed = true;
                    } else if self.to_check.remove(id) {
                        // Just watched: a change from before the watch began.
                        changed |= f.check_now();
                    } else {
                        // Watched: the system says when it changes, and polling is only
                        // the safety net for what sends no events (network drives).
                        let watched = self
                            .watcher
                            .as_ref()
                            .zip(self.canonical.get(&f.cwd))
                            .is_some_and(|(w, c)| w.watching(c));
                        changed |= f.poll_changes(if watched { every * 10 } else { every });
                    }
                }
            }
        }
        while let Ok(n) = self.notes.1.try_recv() {
            self.status = n;
            changed = true;
        }
        let modal_before = self.modal.is_some();
        self.pump_background_jobs();
        changed |= modal_before != self.modal.is_some();
        self.autosave();
        let viewed = self
            .pending_view
            .as_ref()
            .and_then(|(rx, ..)| rx.try_recv().ok());
        if let (Some(out), Some((_, tmp, name, _))) = (viewed, self.pending_view.take()) {
            self.show_extracted(out, tmp, name);
            changed = true;
        }
        // The plan for the open confirmation: how far it has got, or the plan itself.
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            let mut done = None;
            if let Some((p, line)) = c.planning.as_ref() {
                while let Ok(m) = p.rx.try_recv() {
                    match m {
                        xdevcmd_core::plan::Msg::Progress { items, .. } => {
                            if let Some(l) = c.body.get_mut(*line) {
                                *l = format!("counting\u{2026} {items} so far");
                            }
                            changed = true;
                        }
                        xdevcmd_core::plan::Msg::Done(plan) => {
                            done = Some(plan);
                            break;
                        }
                    }
                }
            }
            if let Some(plan) = done {
                let count = Self::count_label(plan.totals.files, plan.totals.bytes);
                c.planned(plan, count);
                changed = true;
            }
        }
        // A count for the open confirmation, if it has come in.
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            let got = c.counting.as_ref().and_then(|(n, _)| n.rx.try_recv().ok());
            if let (Some((files, bytes)), Some((_, line))) = (got, c.counting.take()) {
                if let Some(l) = c.body.get_mut(line) {
                    *l = Self::count_label(files, bytes);
                }
                changed = true;
            }
        }
        changed |= self.poll_comparing();
        if let Some((key, rx)) = &self.decoding {
            if let Ok(prepared) = rx.try_recv() {
                self.quick_image = Some((key.clone(), prepared));
                self.decoding = None;
                changed = true;
            }
        }
        let viewing = matches!(self.modal, Some(Modal::Usage));
        self.usage.tick(viewing);
        let hashed = self
            .hashing
            .as_ref()
            .and_then(|(p, rx)| rx.try_recv().ok().map(|r| (p.clone(), r)));
        if let Some((path, result)) = hashed {
            self.hashing = None;
            self.show_checksum(&path, result);
            changed = true;
        }
        changed
    }

    /// One turn of the event loop without input: lets a running job report in.
    pub fn tick(&mut self) {
        self.pump_job();
        self.reap_terminals();
        self.log_notes();
        self.background();
    }

    /// The terminal is gone (or a SIGTERM came): nobody to ask and nothing to draw on.
    /// Cancel every job, give the workers up to two seconds to stop and remove what they
    /// were writing, and save the session; the caller ends the loop.
    pub(super) fn end_on_signal(&mut self) {
        if let Some(Modal::Job(j)) = &self.modal {
            j.handle.cancel();
        }
        for j in &self.jobs {
            j.handle.cancel();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.job_running_at_all() && Instant::now() < deadline {
            if let Some(Modal::Job(j)) = self.modal.as_mut() {
                j.pump();
                if j.question.take().is_some() {
                    j.answer(xdevcmd_core::Answer::Abort);
                }
            }
            for j in &mut self.jobs {
                j.pump();
                if j.question.take().is_some() {
                    j.answer(xdevcmd_core::Answer::Abort);
                }
            }
            self.jobs.retain(|j| j.done.is_none());
            std::thread::sleep(Duration::from_millis(10));
        }
        self.quit = true;
    }

    /// Any job not finished yet, waiting on a question or not.
    pub(super) fn job_running_at_all(&self) -> bool {
        matches!(&self.modal, Some(Modal::Job(j)) if j.done.is_none())
            || self.jobs.iter().any(|j| j.done.is_none())
    }
}
