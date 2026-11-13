//! Jobs: pumping and finishing them, quitting with them running, recovery after a crash, and undo.

use super::*;

impl App {
    /// A job is working and not waiting on a question: a screenshot (the tests) waits
    /// for it before the last frame, rather than ending the process under it.
    pub fn job_working(&self) -> bool {
        let working = |j: &Job| j.done.is_none() && j.question.is_none();
        matches!(&self.modal, Some(Modal::Job(j)) if working(j))
            || self.jobs.iter().any(working)
            // A plan being made for the open confirmation: `--screenshot` waits for it.
            || matches!(&self.modal, Some(Modal::Confirm(c)) if c.planning.is_some())
    }

    /// A plan is being made for the open confirmation (`--screenshot` lets it finish
    /// before the next key, so a test sees the same dialog however fast the disk).
    pub fn planning(&self) -> bool {
        matches!(&self.modal, Some(Modal::Confirm(c)) if c.planning.is_some())
    }

    /// The viewer has lines on screen drawn plain until its scan reaches them
    /// (`--screenshot` waits for it, as for a job).
    pub fn syntax_waiting(&self) -> bool {
        matches!(&self.modal, Some(Modal::View(v)) if v.syntax_waiting())
    }

    pub(super) fn reload_all(&mut self) {
        // Every tab, not only those on show: a tab you switch to after a move must not
        // still list the files that left it.
        for p in self.panes.values_mut() {
            if let Some(t) = p.as_tabs_mut() {
                t.reload_all();
            }
        }
    }

    /// Drain the worker's events each tick, and close the box by itself on a clean run.
    pub(super) fn pump_job(&mut self) {
        let mut finished = false;
        if let Some(Modal::Job(j)) = self.modal.as_mut() {
            j.pump();
            if let Some(s) = &j.done {
                finished = s.errors.is_empty() && !s.cancelled;
            }
        }
        if finished {
            self.finish_job();
            // A copy started by the viewer's Save as… gives the viewer back.
            self.back_to_viewer();
        }
    }

    pub(super) fn finish_job(&mut self) {
        if let Some(Modal::Job(j)) = self.modal.take() {
            if let Some(s) = &j.done {
                self.status = match &j.title {
                    Some(t) => format!(
                        "{t}: {}",
                        s.describe(j.kind).replace(" deleted", " removed")
                    ),
                    None => s.describe(j.kind),
                };
                self.note_trashed(s);
                self.note_copied(j.kind, s);
            }
        }
        // Marks are cleared once an operation has run, the way every orthodox manager
        // does it. Leaving them set means the next F8 silently includes the files from
        // the last F5 — which is how you delete something you never selected.
        for p in self.panes.values_mut() {
            if let Some(f) = p.as_files_mut() {
                f.marked.clear();
            }
        }
        self.reload_all();
    }

    pub(super) fn pump_background_jobs(&mut self) {
        let mut finished = Vec::new();
        for (i, j) in self.jobs.iter_mut().enumerate() {
            j.pump();
            if j.done.is_some() {
                finished.push(i);
            }
        }
        for i in finished.into_iter().rev() {
            let j = self.jobs.remove(i);
            if let Some(s) = &j.done {
                self.status = format!("background: {}", s.describe(j.kind));
                let s = s.clone();
                self.note_trashed(&s);
                self.note_copied(j.kind, &s);
                // A job that ended in errors comes forward so they can be read.
                if (!s.errors.is_empty() || s.failed > 0) && self.modal.is_none() {
                    self.modal = Some(Modal::Job(j));
                }
            }
            self.reload_all();
        }
        // A job waiting on a question is blocked until someone answers: bring it to
        // the front as soon as nothing else is.
        if self.modal.is_none() {
            if let Some(i) = self.jobs.iter().position(|j| j.question.is_some()) {
                let j = self.jobs.remove(i);
                self.modal = Some(Modal::Job(j));
            }
        }
    }

    /// Jobs still running: ask first. True when the question is now on screen.
    pub(super) fn ask_before_quitting(&mut self) -> bool {
        if self.jobs.is_empty() {
            return false;
        }
        let n = self.jobs.len();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Quit?".into(),
            body: vec![
                format!(
                    "{n} job{} still running in the background.",
                    if n == 1 { " is" } else { "s are" }
                ),
                "Quitting cancels them; a file being written is removed, not left half done."
                    .into(),
            ],
            danger: true,
            yes: Some(("cancel them and quit".into(), Cmd::QuitAnyway)),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
        true
    }

    /// What a job about to start means for Ctrl+Z: a move can be undone; the rest say
    /// that they cannot, so an older rename is never undone behind them.
    pub(super) fn remember_job(&mut self, r: &Request) {
        use xdevcmd_core::undo::Undo;
        self.last_undo = Some(match r.kind {
            ops::Kind::Move => Undo::Move {
                sources: r.sources.clone(),
                dest: r.dest.clone().unwrap_or_default(),
                rename_to: r.rename_to.clone(),
            },
            ops::Kind::Copy => {
                self.expect_copy_undo(format!("copy of {}", Self::describe(&r.sources)));
                return;
            }
            ops::Kind::Delete if r.to_trash => {
                // Undoable once it has finished and said where each item went.
                self.trash_pending = Some(r.sources.clone());
                Undo::Cannot(TRASH_RUNNING.into())
            }
            ops::Kind::Delete => Undo::Cannot("a permanent delete".into()),
            ops::Kind::Extract => Undo::Cannot("an extraction".into()),
            ops::Kind::Pack => Undo::Cannot("packing".into()),
        });
    }

    /// At start-up: tidy up after a run that was killed (safe operations' journal), say
    /// what was done, and offer to resume a job it was in the middle of.
    pub(super) fn recover_interrupted(&mut self) {
        if self.settings.mode.read_only() {
            return;
        }
        let found = xdevcmd_core::journal::recover();
        if found.is_empty() {
            return;
        }
        let cleaned: usize = found.iter().map(|i| i.cleaned).sum();
        let restored: usize = found.iter().map(|i| i.restored).sum();
        let stranded: Vec<_> = found.iter().flat_map(|i| i.stranded.clone()).collect();
        let mut body = vec!["The last run of xdevcmd ended in the middle of a job.".to_string()];
        if cleaned > 0 {
            body.push(format!(
                "{cleaned} unfinished temporary file{} removed.",
                plural(cleaned)
            ));
        }
        if restored > 0 {
            body.push(format!(
                "{restored} file{} put back under {} own name (rename many).",
                plural(restored),
                if restored == 1 { "its" } else { "their" }
            ));
        }
        let swapped: Vec<_> = found.iter().flat_map(|i| i.swapped.clone()).collect();
        if !swapped.is_empty() {
            let n = swapped.len();
            body.push(format!(
                "{n} replacement{} finished: the new file{} put in place, the old one{} in the trash:",
                plural(n),
                if n == 1 { "" } else { "s" },
                if n == 1 { " is" } else { "s are" },
            ));
            for p in swapped.iter().take(3) {
                body.push(format!("  {}", tidy(p)));
            }
        }
        let both: Vec<_> = found.iter().flat_map(|i| i.moved.clone()).collect();
        if !both.is_empty() {
            let n = both.len();
            body.push(format!(
                "{n} file{} of a move had been copied completely, and {} source{} still there:",
                plural(n),
                if n == 1 { "its" } else { "their" },
                if n == 1 { " is" } else { "s are" },
            ));
            for (src, dst) in both.iter().take(3) {
                body.push(format!("  {}  and  {}", tidy(src), tidy(dst)));
            }
            body.push("Both are kept; nothing was removed.".into());
        }
        for (tmp, orig) in stranded.iter().take(3) {
            body.push(format!(
                "Could not put back {} as {} (the name is taken).",
                tidy(tmp),
                tidy(orig)
            ));
        }
        // A permanent delete is not offered again: it would go on with one Enter, where
        // Shift+F8 itself takes only Y.
        let resumable = found.into_iter().find(|i| {
            let kind_ok = match i.kind.as_str() {
                "copy" | "move" => true,
                "delete" => i.to_trash,
                _ => false,
            };
            kind_ok && !i.sources.is_empty()
        });
        let yes = resumable.map(|it| {
            let what = match it.kind.as_str() {
                "copy" => "a copy",
                "move" => "a move",
                _ => "a delete",
            };
            let to = it
                .dest
                .as_deref()
                .map(|d| format!(" to {}", tidy(d)))
                .unwrap_or_default();
            body.push(String::new());
            body.push(format!(
                "It was {what} of {}{to}.",
                Self::describe(&it.sources)
            ));
            body.push("Continue it? What was done is skipped; a file cut short starts".into());
            body.push("again; you are asked about any other file already there.".into());
            ("continue".to_string(), Cmd::Resume(Box::new(it)))
        });
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Interrupted".into(),
            body,
            danger: false,
            yes,
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Run an interrupted job again: what it finished (identical at the destination) is
    /// skipped and anything else already there is asked about, never decided; a move or
    /// delete on the sources still there.
    pub(super) fn resume(&mut self, it: xdevcmd_core::journal::Interrupted) {
        // What it finished is left; what is gone cannot be done again.
        let still: Vec<PathBuf> = it
            .sources
            .iter()
            .filter(|p| !it.done.contains(p) && std::fs::symlink_metadata(p).is_ok())
            .cloned()
            .collect();
        if still.is_empty() {
            self.status = "nothing left to do: every source has gone".into();
            return;
        }
        let mut req = match (it.kind.as_str(), it.dest) {
            ("copy", Some(d)) => Request::copy(still, d),
            ("move", Some(d)) => Request::mv(still, d),
            ("delete", _) if it.to_trash => Request::delete(still, true),
            _ => return,
        };
        req.existing = ops::Existing::Resume;
        // The plan's decisions stand (Round 030): what the user chose to leave is left,
        // what they chose to replace is replaced while it is still that file.
        req.expect = it.decided.iter().cloned().collect();
        let kind = req.kind;
        self.remember_job(&req);
        self.modal = Some(Modal::Job(Job::new(kind, ops::start(req))));
    }

    /// Keep the system watching the directories on screen, and note the panes whose
    /// directory it says changed; they re-read on the next quiet moment.
    pub(super) fn watch_panes(&mut self, shown: &[PaneId], every: Duration) {
        let Some(w) = self.watcher.as_mut() else {
            return;
        };
        let mut dirs = HashSet::new();
        let mut owners: Vec<(PaneId, PathBuf)> = Vec::new();
        if !every.is_zero() {
            for id in shown {
                let Some(f) = self.panes.get(id).and_then(Pane::as_files) else {
                    continue;
                };
                if f.archive.is_some() || f.results.is_some() {
                    continue;
                }
                // The system reports canonical paths (macOS's /tmp is /private/tmp).
                let canon = self
                    .canonical
                    .entry(f.cwd.clone())
                    .or_insert_with(|| f.cwd.canonicalize().unwrap_or_else(|_| f.cwd.clone()))
                    .clone();
                dirs.insert(canon.clone());
                owners.push((*id, canon));
            }
        }
        let new = w.sync(&dirs);
        let changed = w.changed();
        for (id, canon) in owners {
            if changed.contains(&canon) {
                self.to_reload.insert(id);
            } else if new.contains(&canon) {
                self.to_check.insert(id);
            }
        }
        // Forget what is no longer shown, so the map does not grow without end.
        self.canonical.retain(|_, c| dirs.contains(c));
    }

    /// A delete to the trash has finished: if it is still the last operation, undo
    /// can now put back what it moved.
    pub(super) fn note_trashed(&mut self, s: &ops::Summary) {
        let Some(sources) = &self.trash_pending else {
            return;
        };
        let ours = !s.trashed.is_empty() && s.trashed.iter().all(|t| sources.contains(&t.original));
        let still_last = matches!(&self.last_undo, Some(xdevcmd_core::undo::Undo::Cannot(w)) if w == TRASH_RUNNING);
        if ours && still_last {
            self.last_undo = Some(xdevcmd_core::undo::Undo::Trashed(s.trashed.clone()));
            self.trash_pending = None;
        }
    }

    /// A copy is starting: once it ends, what it made is the undo; until then undo says
    /// it is still running, so nothing older is undone behind it.
    pub(super) fn expect_copy_undo(&mut self, what: String) {
        self.copy_pending = Some(what);
        self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot(COPY_RUNNING.into()));
    }

    /// A copy has ended: if it is still the last operation, undo can now remove what it
    /// made. Past the cap it says it cannot.
    pub(super) fn note_copied(&mut self, kind: ops::Kind, s: &ops::Summary) {
        use xdevcmd_core::undo::Undo;
        if kind != ops::Kind::Copy {
            return;
        }
        let Some(what) = self.copy_pending.take() else {
            return;
        };
        if !matches!(&self.last_undo, Some(Undo::Cannot(w)) if w == COPY_RUNNING) {
            return;
        }
        self.last_undo = Some(if s.made_over {
            Undo::Cannot(format!(
                "a copy of more than {} items",
                xdevcmd_core::ops::MADE_CAP
            ))
        } else {
            Undo::Copy {
                made: s.made.clone(),
                what,
                for_good: s.replaced_for_good,
            }
        });
    }

    /// Ctrl+Z: say what undoing would do, against the disk as it is now, and ask.
    pub(super) fn ask_undo(&mut self) {
        let Some(u) = &self.last_undo else {
            self.status = "nothing to undo".into();
            return;
        };
        let plan = match u.plan() {
            Ok(p) => p,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        use xdevcmd_core::undo::{Plan, Undo};
        let what = match &plan {
            Plan::Rename(m) => m
                .iter()
                .take(8)
                .map(|(f, t)| {
                    format!(
                        "{}  \u{2192}  {}",
                        Self::describe(std::slice::from_ref(f)),
                        Self::describe(std::slice::from_ref(t))
                    )
                })
                .collect::<Vec<_>>(),
            Plan::Move { back, into } => vec![format!(
                "move {} back to {}",
                Self::describe(back),
                tidy(into)
            )],
            Plan::Remove(p) => vec![format!("remove {}, still empty", tidy(p))],
            Plan::Unmake { made, kept } => {
                let n = made.len() - kept;
                let back = made.iter().filter(|m| m.old.is_some()).count();
                let mut lines = vec![format!(
                    "remove the {n} item{} it made, as it made {}",
                    if n == 1 { "" } else { "s" },
                    if n == 1 { "it" } else { "them" }
                )];
                if back > 0 {
                    lines.push(format!("put back from the trash the {back} it replaced"));
                }
                if *kept > 0 {
                    lines.push(format!("keep {kept} changed since the copy"));
                }
                if let Undo::Copy { for_good, .. } = u {
                    if *for_good > 0 {
                        lines.push(format!(
                            "{for_good} file{} it replaced had no trash: those stay as copied",
                            if *for_good == 1 { "" } else { "s" }
                        ));
                    }
                }
                lines
            }
            Plan::Restore(items) => {
                let mut lines: Vec<String> = items
                    .iter()
                    .take(8)
                    .map(|i| format!("put back {}", tidy(&i.original)))
                    .collect();
                if items.len() > 8 {
                    lines.push(format!("\u{2026} and {} more", items.len() - 8));
                }
                lines
            }
        };
        let mut body = vec![format!("Undo the {}?", u.describe()), String::new()];
        body.extend(what);
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Undo".into(),
            body,
            danger: false,
            yes: Some(("undo".into(), Cmd::Undo)),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Rename many: `moves`, with safe operations' journal when `safe`. `None` when its
    /// journal could not be written: nothing was renamed, and a dialog says why and
    /// offers to rename without it (`undoing`: this is an undo).
    fn rename_many(
        &mut self,
        moves: Vec<(PathBuf, PathBuf)>,
        safe: bool,
        undoing: bool,
    ) -> Option<xdevcmd_core::batch_rename::Outcome> {
        let out = xdevcmd_core::batch_rename::apply(&moves, safe);
        let Some(why) = out.no_journal else {
            self.reload_all();
            return Some(out);
        };
        self.status = "nothing renamed: safe operations cannot write their journal".into();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Rename many".into(),
            body: vec![
                "Nothing was renamed: safe operations cannot write their journal".into(),
                format!("({why})."),
                String::new(),
                "Rename many first moves every file to a temporary name. Without the".into(),
                "journal, a kill or a power cut between its two steps leaves them under".into(),
                "those names, and the next start cannot put them back.".into(),
            ],
            danger: true,
            yes: Some((
                "rename anyway".into(),
                Cmd::BatchRenameAnyway { moves, undoing },
            )),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
        None
    }

    /// Shift+F2's plan, carried out; it becomes the one step undo takes back.
    pub(super) fn batch_rename(&mut self, moves: Vec<(PathBuf, PathBuf)>, safe: bool) {
        let Some(out) = self.rename_many(moves.clone(), safe, false) else {
            return;
        };
        self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(moves));
        self.status = match (out.renamed, out.errors.first()) {
            (n, None) => format!("renamed {n}"),
            (n, Some(e)) => format!("renamed {n}, {} failed \u{2014} {e}", out.errors.len()),
        };
    }

    /// Undo of a rename many: the names put back.
    pub(super) fn undo_rename(&mut self, moves: Vec<(PathBuf, PathBuf)>, safe: bool) {
        let what = self
            .last_undo
            .as_ref()
            .map(|u| u.describe())
            .unwrap_or_else(|| "rename".into());
        let Some(out) = self.rename_many(moves, safe, true) else {
            return;
        };
        self.status = match out.errors.first() {
            None => format!("undone: {what}"),
            Some(e) => format!(
                "undo: {} renamed back, {} failed \u{2014} {e}",
                out.renamed,
                out.errors.len()
            ),
        };
        self.last_undo = None;
    }

    /// Carry out the undo, checking the disk once more.
    pub(super) fn undo(&mut self) {
        use xdevcmd_core::undo::Plan;
        let Some(u) = self.last_undo.clone() else {
            return;
        };
        let plan = match u.plan() {
            Ok(p) => p,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        match plan {
            Plan::Rename(moves) => {
                self.undo_rename(moves, xdevcmd_core::ops::safe());
                return;
            }
            Plan::Move { back, into } => {
                let request = Request::mv(back, into);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Move, ops::start(request))));
                self.status = format!("undoing the {}", u.describe());
            }
            Plan::Unmake { made, .. } => {
                let mut job = Job::new(ops::Kind::Delete, xdevcmd_core::undo::unmake(made));
                job.title = Some("Undo the copy".into());
                self.modal = Some(Modal::Job(job));
                self.status = format!("undoing the {}", u.describe());
            }
            Plan::Remove(p) => {
                self.status = match xdevcmd_core::undo::remove(&p) {
                    Ok(()) => format!("undone: {}", u.describe()),
                    Err(e) => format!("cannot undo: {e}"),
                };
            }
            Plan::Restore(items) => {
                let mut failed = Vec::new();
                for t in &items {
                    if let Err(e) = xdevcmd_core::trashed::restore(t) {
                        failed.push(e.to_string());
                    }
                }
                self.status = match failed.first() {
                    None => format!("undone: {}", u.describe()),
                    Some(e) => format!(
                        "undo: {} put back, {} not \u{2014} {e}",
                        items.len() - failed.len(),
                        failed.len()
                    ),
                };
            }
        }
        // One step: an undone operation is not undone again.
        self.last_undo = None;
        self.reload_all();
    }
}
