//! File operations: copy, move, delete, mkdir, rename, rename many, links, permissions, checksums.

use super::*;

impl App {
    /// Where a copy or move from the focused pane could go, best first: the other file
    /// panes by how recently they had focus, then any never focused, one entry per
    /// directory, and a directory equal to the source's own last. With two panes this
    /// is just "the other pane", as it always was.
    pub(super) fn transfer_targets(&self) -> Vec<PathBuf> {
        let src = self.focused_cwd();
        let mut panes: Vec<PaneId> = self
            .file_panes()
            .into_iter()
            .filter(|id| *id != self.focus)
            .collect();
        let rank = |id: &PaneId| self.mru.iter().position(|m| m == id).unwrap_or(usize::MAX);
        panes.sort_by_key(rank);
        let mut dirs: Vec<PathBuf> = Vec::new();
        for id in panes {
            // A pane browsing an archive or showing search results is not a place
            // files can be copied into.
            let into = self
                .panes
                .get(&id)
                .and_then(Pane::as_files)
                .filter(|f| f.archive.is_none() && f.results.is_none());
            if let Some(f) = into {
                if !dirs.contains(&f.cwd) {
                    dirs.push(f.cwd.clone());
                }
            }
        }
        // Stable sort: recency order is kept within each group.
        dirs.sort_by_key(|d| *d == src);
        dirs
    }

    /// The pane a copy or move to `dir` lands in, as [`transfer_targets`] ranks them:
    /// the other file pane focused last that shows `dir`. Drawn lit while the dialog
    /// is open, so with three panes or more the destination is seen, not guessed.
    pub(super) fn transfer_pane(&self, dir: &Path) -> Option<PaneId> {
        let rank = |id: &PaneId| self.mru.iter().position(|m| m == id).unwrap_or(usize::MAX);
        let mut panes: Vec<PaneId> = self
            .file_panes()
            .into_iter()
            .filter(|id| *id != self.focus)
            .collect();
        panes.sort_by_key(rank);
        panes.into_iter().find(|id| {
            self.panes
                .get(id)
                .and_then(Pane::as_files)
                .is_some_and(|f| f.archive.is_none() && f.results.is_none() && f.cwd == dir)
        })
    }

    pub(super) fn other_pane_cwd(&self) -> Option<PathBuf> {
        self.transfer_targets().into_iter().next()
    }

    /// "1 file", "3 files" — the singular matters because these strings sit in a
    /// confirmation for something irreversible, and sloppy copy reads as untrustworthy.
    pub(super) fn count_label(files: u64, bytes: u64) -> String {
        // Whole texts, never an English word inside a translated one (beta.4 red-team).
        let count = if files == 1 {
            tr!("file_ops.one_file", "1 file").into_owned()
        } else {
            tr!("file_ops.n_files", "{files} files", files = files)
        };
        if bytes < 1024 {
            tr!(
                "file_ops.count_bytes",
                "{count} \u{2022} {bytes} bytes",
                count = count,
                bytes = bytes
            )
        } else {
            tr!(
                "file_ops.count_size",
                "{count} \u{2022} {size}",
                count = count,
                size = human_size(bytes)
            )
        }
    }

    pub(super) fn describe(sources: &[PathBuf]) -> String {
        match sources {
            [one] => one
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| one.display().to_string()),
            many => format!("{} items", many.len()),
        }
    }

    /// #2: whether a target is never deleted, moved or renamed — a root, the home or a
    /// folder above it, a path with `.`/`..` (core's [`xdevcmd_core::protect`]), or the
    /// folder the focused pane shows or one above it. Said on the status line; true when
    /// refused. The engine checks the first kinds again before a delete or a move.
    pub(super) fn guard_refused(&mut self, targets: &[PathBuf]) -> bool {
        use crate::refusal::{Guard, Refusal};
        use xdevcmd_core::protect::{self, Guarded};
        let homes = protect::homes();
        // Every folder something is in now: each file pane's (a list's own root
        // included) and each shell's (red-team of #2: not the focused pane's alone).
        let mut inside: Vec<PathBuf> = self
            .panes
            .values()
            .filter_map(Pane::as_files)
            .map(|f| f.cwd.clone())
            .collect();
        inside.extend(self.panel_ref().map(|p| p.shell_dirs()).unwrap_or_default());
        let real: Vec<PathBuf> = inside
            .iter()
            .filter_map(|d| xdevcmd_core::paths::canonical(d).ok())
            .collect();
        // Resolved only when a target's name is one of those folders' names: a link on
        // the way can make it one of them; any other name cannot be.
        let names: Vec<std::ffi::OsString> = inside
            .iter()
            .chain(&real)
            .flat_map(|d| d.ancestors())
            .filter_map(|p| p.file_name().map(|n| n.to_os_string()))
            .collect();
        for t in targets {
            let guard = match protect::protected(t, &homes) {
                Some(Guarded::Root) => Some(Guard::Root),
                Some(Guarded::Home) => Some(Guard::Home),
                Some(Guarded::NotPlain) => Some(Guard::NotPlain),
                None => {
                    let holds = inside.iter().any(|d| d.starts_with(t))
                        || t.file_name().is_some_and(|n| names.iter().any(|m| m == n))
                            && xdevcmd_core::paths::canonical(t)
                                .is_ok_and(|rt| real.iter().any(|d| d.starts_with(&rt)));
                    holds.then_some(Guard::Cwd)
                }
            };
            let Some(guard) = guard else {
                continue;
            };
            Refusal::Protected(guard).log();
            let name = xdevcmd_core::format::elide_middle(&tidy(t), 20);
            self.status = match guard {
                Guard::Cwd => {
                    tr!(
                        "file_ops.holds_a_folder_shown_or_in",
                        "{name} holds a folder shown or in a shell \u{2014} not removed",
                        name = name
                    )
                }
                Guard::NotPlain => tr!(
                    "file_ops.names_a_folder_by_or_u",
                    "{name} names a folder by . or .. \u{2014} not touched",
                    name = name
                ),
                Guard::Root | Guard::Home => tr!(
                    "file_ops.is_never_deleted_or_moved",
                    "{name} is never deleted or moved",
                    name = name
                ),
            };
            return true;
        }
        false
    }

    pub(super) fn begin_transfer(&mut self, kind: ops::Kind) {
        let sources = self.selection();
        if sources.is_empty() {
            self.status = tr!("file_ops.nothing_selected", "nothing selected").into();
            return;
        }
        if kind == ops::Kind::Move && self.guard_refused(&sources) {
            return;
        }
        let choices = self.transfer_targets();
        if let Some((file, prefix)) = self
            .focused_archive()
            .map(|a| (a.file.clone(), a.prefix.clone()))
        {
            let inner = self
                .focused_archive()
                .map(|a| a.inner(&sources))
                .unwrap_or_default();
            let Some(dest) = choices.first().cloned() else {
                self.status = tr!(
                    "file_ops.extracting_goes_into_another_pane_u",
                    "extracting goes into another pane \u{2014} Alt+S splits one"
                )
                .into();
                return;
            };
            self.confirm_extract(file, inner, prefix, dest, choices);
            return;
        }
        let Some(dest) = choices.first().cloned() else {
            self.status = tr!(
                "file_ops.no_second_pane_to_copy_to",
                "no second pane to copy to — Alt+S splits one"
            )
            .into();
            return;
        };
        let request = match kind {
            ops::Kind::Copy => Request::copy(sources.clone(), dest.clone()),
            _ => Request::mv(sources.clone(), dest.clone()),
        };
        // Overwrites still raise a question mid-job; this only skips the "are you sure"
        // before starting, which is all the setting claims to do.
        if !self.settings.config.general.confirm_transfer {
            self.remember_job(&request);
            self.modal = Some(Modal::Job(Job::new(kind, ops::start(request))));
            return;
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: crate::modal::verb_text(kind),
            body: vec![
                Self::describe(&sources),
                String::new(),
                tr!("dialog.to", "to    {dest}", dest = dest.display()),
                COUNTING.into(),
            ],
            danger: false,
            request,
            buttons: Vec::new(),
            choices,
            choice: 0,
            source_dir: Some(self.focused_cwd()),
            counting: None,
            permanent: false,
            // set_destination writes the line, after the destination's own lines.
            existing_line: Some(0),
            plans: true,
            planning: None,
            table: None,
        }));
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            c.set_destination(0);
            c.replan();
        }
    }

    pub(super) fn begin_delete(&mut self, permanent: bool) {
        let targets = self.selection();
        if targets.is_empty() {
            self.status = tr!("file_ops.nothing_selected", "nothing selected").into();
            return;
        }
        if self.guard_refused(&targets) {
            return;
        }
        let mut body = vec![Self::describe(&targets), String::new(), COUNTING.into()];
        let counting = Some((ops::count(targets.clone()), 2));
        if permanent {
            // What goes, by name: the first few, so a wrong selection shows.
            if targets.len() > 1 {
                body.push(String::new());
                for t in targets.iter().take(3) {
                    let name = t.file_name().map(|n| n.to_string_lossy().into_owned());
                    body.push(format!(
                        "  {}",
                        name.unwrap_or_else(|| t.display().to_string())
                    ));
                }
                if targets.len() > 3 {
                    body.push(tr!(
                        "file_ops.u_and_more",
                        "  \u{2026} and {v} more",
                        v = targets.len() - 3
                    ));
                }
            }
            body.push(String::new());
            body.push(
                tr!(
                    "file_ops.deleted_permanently_this_cannot_be_undone",
                    "Deleted permanently. This cannot be undone."
                )
                .into(),
            );
        } else {
            body.push(tr!("file_ops.moved_to_the_trash", "Moved to the trash.").into());
        }
        // A list's names may come from any text a shell printed: say where from, so a
        // list made of the wrong output shows before anything is deleted.
        let from = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.results.as_ref())
            .map(|r| {
                let what = xdevcmd_core::format::elide_middle(&r.what, 50);
                if r.find {
                    tr!(
                        "file_ops.found_by_find",
                        "Found by: find {what}",
                        what = what
                    )
                } else {
                    tr!(
                        "file_ops.from_the_list",
                        "From the list: {what}",
                        what = what
                    )
                }
            });
        if let Some(from) = from {
            body.push(String::new());
            body.push(from);
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: if permanent {
                tr!("file_ops.delete_permanently", "Delete permanently").into()
            } else {
                tr!("file_ops.delete", "Delete").into_owned()
            },
            body,
            danger: true,
            request: Request::delete(targets, !permanent),
            buttons: Vec::new(),
            choices: Vec::new(),
            choice: 0,
            source_dir: None,
            counting,
            permanent,
            existing_line: None,
            plans: false,
            planning: None,
            table: None,
        }));
    }

    pub(super) fn begin_mkdir(&mut self) {
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: tr!("file_ops.create_directory", "Create directory").into(),
            label: tr!("where.in", "in {dir}", dir = dir.display()),
            edit: LineEdit::new(""),
            kind: InputKind::MkDir,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn begin_rename(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()).cloned() else {
            self.status = tr!("file_ops.nothing_to_rename_here", "nothing to rename here").into();
            return;
        };
        let (label, dir) = (
            tr!("where.in", "in {dir}", dir = f.cwd.display()),
            f.cwd.clone(),
        );
        if self.guard_refused(std::slice::from_ref(&e.path)) {
            return;
        }
        self.modal = Some(Modal::Input(Input {
            title: tr!("file_ops.rename", "Rename").into(),
            label,
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::Rename {
                from: e.path.clone(),
            },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn apply_input(&mut self, i: Input) {
        let text = i.edit.text();
        // A name for something new: a control character anywhere in what was typed is
        // refused before the spaces are trimmed, or a tab or a newline at either end
        // would go with them into another name (T5 on H-08). Pack, Save as and Log to
        // file check the same in `typed_destination`.
        if matches!(
            i.kind,
            InputKind::MkDir | InputKind::NewFile | InputKind::CopyAs { .. }
        ) {
            if let Some(c) = xdevcmd_core::controls::first(&text, &[]) {
                let e = xdevcmd_core::rename::RenameError::BadChar(c);
                self.modal = Some(Modal::Input(Input {
                    error: Some(e.to_string()),
                    ..i
                }));
                return;
            }
        }
        match &i.kind {
            InputKind::SaveAs => self.apply_save_as(i),
            InputKind::LogTo { .. } => self.apply_log_to(i),
            InputKind::MkDir => {
                let name = text.trim();
                if name.is_empty() {
                    self.status = tr!("file_ops.no_name_given", "no name given").into();
                    return;
                }
                // Folders may be nested (`a/b`): each part is checked as a name where it
                // goes, Windows' rules included where they hold (W8, Round 037). An empty
                // part (`/` first, `a//b`), `.` and `..` are a way there, not a name: the
                // root check below holds `..` in.
                let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
                let bad = name
                    .split(seps)
                    .filter(|p| !p.is_empty() && *p != "." && *p != "..")
                    .find_map(|p| xdevcmd_core::rename::name_for(&i.dir, p).err());
                if let Some(e) = bad {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                let first = xdevcmd_core::undo::first_missing(&target);
                // Under --root: the one check of a path to make (no `..` in what is still
                // to make: create_dir_all would follow it out).
                if !xdevcmd_core::policy::get().allows_at(&target) {
                    self.status = tr!(
                        "file_ops.outside_the_root_xdevcmd_is_confined",
                        "outside the root xdevcmd is confined to"
                    )
                    .into();
                    return;
                }
                let existed = target.exists();
                match std::fs::create_dir_all(&target) {
                    Ok(()) => {
                        if !existed {
                            self.last_undo = Some(xdevcmd_core::undo::Undo::MadeDir(first));
                        }
                        self.status = tr!("file_ops.created", "created {name}", name = name);
                        self.reload_all();
                    }
                    Err(e) => {
                        self.status = tr!("file_ops.mkdir_failed", "mkdir failed: {e}", e = e)
                    }
                }
            }
            InputKind::GoTo => {
                let typed = text.trim();
                if typed.is_empty() {
                    return;
                }
                let mut p = expand_tilde(typed);
                if p.is_relative() {
                    p = i.dir.join(p);
                }
                match std::fs::metadata(&p) {
                    Ok(m) if m.is_dir() => self.with_files(|f| f.set_cwd(&p)),
                    Ok(_) => {
                        let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
                            return;
                        };
                        let (parent, name) = (parent.to_path_buf(), name.to_os_string());
                        self.with_files(|f| f.go_to_file(&parent, &name));
                    }
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e.to_string()),
                            ..i
                        }));
                    }
                }
            }
            InputKind::NewFile => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::name_for(&i.dir, name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                // An existing file is opened, not truncated: Shift+F4 on a name that
                // is there already means "edit it".
                let made = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target);
                match made {
                    Ok(_) => {
                        self.last_undo = Some(xdevcmd_core::undo::Undo::MadeFile(target.clone()));
                        self.status = tr!("file_ops.created", "created {name}", name = name);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if target.is_dir() {
                            self.status = tr!(
                                "file_ops.is_a_directory",
                                "{name} is a directory",
                                name = name
                            );
                            return;
                        }
                    }
                    Err(e) => {
                        self.status = tr!(
                            "file_ops.cannot_create",
                            "cannot create {name}: {e}",
                            name = name,
                            e = e
                        );
                        return;
                    }
                }
                self.reload_all();
                self.with_files(|f| f.select_path(&target));
                if xdevcmd_core::policy::get().allows(&target) {
                    self.pending_edit = Some(target);
                } else {
                    // A name that exists as a link out of the root.
                    self.status = tr!(
                        "file_ops.that_file_is_outside_the_root",
                        "that file is outside the root xdevcmd is confined to"
                    )
                    .into();
                }
            }
            InputKind::CopyAs { from } => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::name_for(&i.dir, name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let mut req = Request::copy(vec![from.clone()], i.dir.clone());
                req.rename_to = Some(name.into());
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            InputKind::Permissions { targets } => {
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot(
                    tr!(
                        "file_ops.a_change_of_permissions",
                        "a change of permissions"
                    )
                    .into(),
                ));
                let setting = match xdevcmd_core::attrs::parse(&text) {
                    Ok(s) => s,
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e),
                            ..i
                        }));
                        return;
                    }
                };
                let mut done = 0;
                let mut first_err = None;
                for t in targets {
                    match xdevcmd_core::attrs::apply(t, setting) {
                        Ok(()) => done += 1,
                        Err(e) => {
                            first_err.get_or_insert_with(|| {
                                format!(
                                    "{}: {e}",
                                    t.file_name().unwrap_or_default().to_string_lossy()
                                )
                            });
                        }
                    }
                }
                self.reload_all();
                self.status = match first_err {
                    None => tr!(
                        "file_ops.permissions_set_on",
                        "permissions set on {done}",
                        done = done
                    ),
                    Some(e) => tr!(
                        "file_ops.set_on_not_on_the_rest",
                        "set on {done}, not on the rest \u{2014} {e}",
                        done = done,
                        e = e
                    ),
                };
            }
            InputKind::Pack { sources } => {
                // The one check of a typed file to make: inside the root, no control
                // character (the name is prefilled from one someone else chose), not a
                // directory, in a folder that exists.
                let dest = match self.typed_destination(&i) {
                    Ok(Some(d)) => d,
                    Ok(None) => return,
                    Err(why) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(why),
                            ..i
                        }));
                        return;
                    }
                };
                if xdevcmd_core::archive::format_of(&dest).is_none() {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(
                            tr!(
                                "file_ops.the_name_must_end_in_zip",
                                "the name must end in .zip, .tar, .tar.gz or .tgz"
                            )
                            .into(),
                        ),
                        ..i
                    }));
                    return;
                }
                let req = Request::pack(sources.clone(), dest);
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Pack, ops::start(req))));
            }
            InputKind::Pattern { mark } => {
                let mark = *mark;
                let pattern = xdevcmd_core::glob::Pattern::new(&text);
                if pattern.is_empty() {
                    self.status = tr!("file_ops.no_pattern_given", "no pattern given").into();
                    return;
                }
                let n = self
                    .focused_files()
                    .map_or(0, |f| f.mark_matching(&pattern, mark));
                self.status = match n {
                    0 => tr!(
                        "file_ops.nothing_matches",
                        "nothing matches {trim}",
                        trim = text.trim()
                    ),
                    n if mark => tr!(
                        "file_ops.marked_n_matching",
                        "marked {n} matching {pattern}",
                        n = n,
                        pattern = text.trim()
                    ),
                    n => tr!(
                        "file_ops.unmarked_n_matching",
                        "unmarked {n} matching {pattern}",
                        n = n,
                        pattern = text.trim()
                    ),
                };
            }
            InputKind::Rename { from } => match xdevcmd_core::rename::rename(from, &text) {
                Ok(to) => {
                    if to != *from {
                        let pair = [(from.clone(), to.clone())];
                        self.follow_renames(&pair);
                        self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(vec![(
                            from.clone(),
                            to.clone(),
                        )]));
                    }
                    let old = from.file_name().map(|s| s.to_string_lossy().into_owned());
                    self.status = tr!(
                        "file_ops.renamed_u",
                        "renamed {unwrap_or_default} \u{2192} {text}",
                        unwrap_or_default = old.unwrap_or_default(),
                        text = text
                    );
                    self.reload_all();
                    for p in self.panes.values_mut() {
                        if let Some(f) = p.as_files_mut() {
                            f.select_path(&to);
                        }
                    }
                }
                Err(e) => {
                    // Back on screen with the reason, and the text as typed.
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                }
            },
        }
    }

    /// F4 in the rename dialog: `text`, one name per file, into a file for the editor.
    pub(super) fn begin_batch(&mut self, files: Vec<PathBuf>, text: &str) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(list) = crate::tempfiles::file(&format!("rename-{n}.txt")) else {
            self.status = tr!(
                "file_ops.cannot_make_a_temporary_directory_for",
                "cannot make a temporary directory for the list of names"
            )
            .into();
            return;
        };
        if let Err(e) = xdevcmd_core::document::save_state(&list, text.as_bytes()) {
            self.status = tr!(
                "file_ops.cannot_write_the_list_of_names",
                "cannot write the list of names: {e}",
                e = e
            );
            return;
        }
        self.pending_batch = Some((files, list.clone()));
        self.pending_edit = Some(list);
    }

    /// The editor has closed: read the names back and ask before renaming anything.
    pub(super) fn finish_batch(&mut self, files: Vec<PathBuf>, list: &Path) {
        let edited = std::fs::read_to_string(list);
        let _ = std::fs::remove_file(list);
        let edited = match edited {
            Ok(t) => t,
            Err(e) => {
                self.status = tr!(
                    "file_ops.cannot_read_the_names_back",
                    "cannot read the names back: {e}",
                    e = e
                );
                return;
            }
        };
        let moves = match xdevcmd_core::batch_rename::plan(&files, &edited) {
            Ok(m) => m,
            Err(e) => {
                self.status = tr!(
                    "file_ops.nothing_renamed_u",
                    "nothing renamed \u{2014} {e}",
                    e = e
                );
                return;
            }
        };
        if moves.is_empty() {
            self.status = tr!("file_ops.no_names_changed", "no names changed").into();
            return;
        }
        let name = |p: &PathBuf| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let body = moves
            .iter()
            .map(|(f, t)| format!("{}  \u{2192}  {}", name(f), name(t)))
            .collect();
        let n = moves.len();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: tr!(
                "file_ops.rename_file",
                "Rename {n} file{s}",
                n = n,
                s = if n == 1 { "" } else { "s" }
            ),
            body,
            danger: false,
            yes: Some(("rename".into(), Cmd::BatchRename(moves))),
            buttons: Vec::new(),
            also: None,
            more: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    pub(super) fn begin_copy_as(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()) else {
            self.status = tr!("file_ops.nothing_to_copy_here", "nothing to copy here").into();
            return;
        };
        let dir = e
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| f.cwd.clone());
        self.modal = Some(Modal::Input(Input {
            title: tr!("file_ops.copy_as", "Copy as").into(),
            label: tr!(
                "file_ops.a_second_copy_of_beside_it",
                "a second copy of {name}, beside it",
                name = e.name
            ),
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::CopyAs {
                from: e.path.clone(),
            },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn make_symlink(&mut self) {
        self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot(
            tr!("file_ops.making_a_link", "making a link").into(),
        ));
        let Some(e) = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned())
            .filter(|e| !e.is_parent())
        else {
            self.status = tr!(
                "file_ops.nothing_to_link_to_here",
                "nothing to link to here"
            )
            .into();
            return;
        };
        let Some(dest) = self.other_pane_cwd() else {
            self.status = tr!(
                "file_ops.a_link_goes_into_the_other",
                "a link goes into the other pane \u{2014} there is none"
            )
            .into();
            return;
        };
        let link = dest.join(&e.name);
        match xdevcmd_core::attrs::symlink(&e.path, &link) {
            Ok(()) => {
                self.reload_all();
                self.status = tr!(
                    "file_ops.u",
                    "{v} \u{2192} {v2}",
                    v = tidy(&link),
                    v2 = tidy(&e.path)
                );
            }
            Err(err) => {
                self.status = tr!("file_ops.no_link_made", "no link made: {err}", err = err)
            }
        }
    }

    pub(super) fn begin_permissions(&mut self) {
        let targets = self.selection();
        let Some(first) = targets.first() else {
            self.status = tr!("file_ops.nothing_selected", "nothing selected").into();
            return;
        };
        let current = xdevcmd_core::attrs::current(first).unwrap_or_default();
        let many = if targets.len() > 1 {
            tr!(
                "file_ops.u_applies_to_all",
                " \u{b7} applies to all {targets_len}",
                targets_len = targets.len()
            )
        } else {
            String::new()
        };
        let label = format!("{}{many}", xdevcmd_core::attrs::describe(first));
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: tr!("file_ops.permissions", "Permissions").into(),
            label,
            edit: LineEdit::for_rename(&current, true),
            kind: InputKind::Permissions { targets },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn begin_checksum(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = tr!(
                "file_ops.a_checksum_is_of_a_file",
                "a checksum is of a file"
            )
            .into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = tr!(
                "file_ops.that_file_is_outside_the_root",
                "that file is outside the root xdevcmd is confined to"
            )
            .into();
            return;
        }
        // Not a FIFO or a device: reading one would never end, or never start.
        let special = std::fs::metadata(&path)
            .ok()
            .and_then(|m| xdevcmd_core::special_kind(&m.file_type()).map(str::to_string));
        if let Some(kind) = special {
            self.status = tr!(
                "file_ops.that_is_a_not_a_file",
                "that is a {kind}, not a file to checksum",
                kind = kind
            );
            return;
        }
        let p = path.clone();
        self.start_hashing_with(path, move |stop| xdevcmd_core::sha256::file(&p, stop));
    }

    /// The checksum's worker: `work` on a thread of its own, its result read by the
    /// loop. A seam for the test of a worker that dies.
    pub(super) fn start_hashing_with(
        &mut self,
        path: PathBuf,
        work: impl FnOnce(&std::sync::atomic::AtomicBool) -> std::io::Result<Option<String>>
            + Send
            + 'static,
    ) {
        let task = xdevcmd_core::task::Task::spawn(move |stop| work(stop));
        self.status = tr!(
            "file_ops.working_out_the_sha_256_of",
            "working out the SHA-256 of {v}\u{2026}",
            v = Self::describe(std::slice::from_ref(&path))
        );
        self.hashing = Some((path, task));
    }

    pub(super) fn show_checksum(&mut self, path: &Path, result: std::io::Result<Option<String>>) {
        let name = Self::describe(&[path.to_path_buf()]);
        let hex = match result {
            Ok(Some(h)) => h,
            Ok(None) => return,
            Err(e) => {
                self.status = tr!(
                    "file_ops.cannot_read",
                    "cannot read {name}: {e}",
                    name = name,
                    e = e
                );
                return;
            }
        };
        let mut body = vec![name.clone(), String::new(), format!("SHA-256  {hex}")];
        // A published checksum beside the file, in the usual places.
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut sidecars = vec![dir.join(format!("{name}.sha256"))];
        sidecars.extend(["SHA256SUMS", "sha256sums.txt", "SHA256SUMS.txt"].map(|f| dir.join(f)));
        for side in sidecars {
            let Ok(text) = std::fs::read_to_string(&side) else {
                continue;
            };
            let listed = text.lines().find(|l| {
                l.split_whitespace()
                    .nth(1)
                    .is_none_or(|n| n.trim_start_matches('*') == name)
            });
            let Some(line) = listed else { continue };
            let want = line
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let side = side
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            body.push(String::new());
            body.push(if want == hex {
                tr!("file_ops.u_matches", "\u{2714} matches {side}", side = side)
            } else {
                tr!(
                    "file_ops.u_does_not_match",
                    "\u{2718} DOES NOT MATCH {side}: {want}",
                    side = side,
                    want = want
                )
            });
            break;
        }
        let bad = body.iter().any(|l| l.contains("DOES NOT MATCH"));
        self.status.clear();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: tr!("file_ops.checksum", "Checksum").into(),
            body,
            danger: bad,
            yes: Some((
                tr!("file_ops.copy_to_clipboard", "copy to clipboard").into(),
                Cmd::CopyText(hex),
            )),
            buttons: Vec::new(),
            also: None,
            more: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }
}
