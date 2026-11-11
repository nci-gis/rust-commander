//! Keys and commands: feeding input, key groups, Alt+X, actions, menu commands and copying to the clipboard.

use super::*;

impl App {
    /// Feed a key as if it had been typed. Used by `--keys` so a dialog can be put on
    /// screen and asserted on without a terminal or a human.
    pub fn feed_key(&mut self, k: KeyEvent) {
        self.on_key(k);
    }

    /// Feed a mouse event, as `--keys=click:X:Y` does.
    pub fn feed_mouse(&mut self, m: MouseEvent) {
        self.on_mouse(m);
    }

    /// On macOS, a character Option typed in a file pane, where it means nothing, says
    /// once how to make Option work as Alt.
    pub(super) fn option_hint(&mut self, k: KeyEvent) {
        if !cfg!(target_os = "macos") || self.option_hinted || self.focus_is_term() {
            return;
        }
        if let Some(c) = crate::keyfix::typed_by_option(k)
            .filter(|_| self.settings.keymap.action_for(k).is_none())
        {
            self.option_hinted = true;
            self.status = crate::keyfix::option_hint(c);
        }
    }

    /// The status line while a group is open: its first key and the keys that follow.
    /// Inside the shell only the keys that work there are offered, and the rest of
    /// what the prefix does is said in words.
    pub(super) fn group_hint(&self, first: crate::keys::Chord) -> String {
        let in_shell = self.focus_is_term();
        let keys: Vec<String> = self
            .settings
            .keymap
            .group(first)
            .iter()
            .filter(|(_, a)| !in_shell || a.is_global())
            .map(|((c, m), _)| crate::keys::key_name(*c, *m))
            .collect();
        // A group whose keys all repeat is a mode, left with Enter or Esc; any other
        // is a key to pick, after which it closes by itself.
        if self.group_repeats(first) {
            format!("{} \u{b7} Enter or Esc: done", keys.join(" "))
        } else if in_shell {
            format!(
                "then: {} \u{b7} a terminal key for this shell \u{b7} Esc: cancel",
                keys.join(" ")
            )
        } else {
            format!("then: {} \u{b7} Esc: cancel", keys.join(" "))
        }
    }

    /// Whether every key in the group `first` opens repeats.
    pub(super) fn group_repeats(&self, first: crate::keys::Chord) -> bool {
        let g = self.settings.keymap.group(first);
        !g.is_empty() && g.iter().all(|(_, a)| a.repeats())
    }

    /// The badge at the left of the status line while a group is open: `RESIZE` for a
    /// mode with a name, else the key that opened it, `ALT+R …`.
    pub(super) fn group_badge(&self) -> Option<String> {
        let first = self.group?;
        let label = match self.settings.keymap.group_name(first) {
            Some(n) if self.group_repeats(first) => n.to_uppercase(),
            _ => format!(
                "{} \u{2026}",
                crate::keys::key_name(first.0, first.1).to_uppercase()
            ),
        };
        Some(format!(" {label} "))
    }

    /// A key of the open group: run it and keep the group open. `false` when the key is
    /// not one of the group's, which closes it.
    pub(super) fn in_group(&mut self, k: KeyEvent) -> bool {
        let Some(first) = self.group.take() else {
            return false;
        };
        // Inside the shell only Esc cancels: Enter is the program's like any other key
        // xdevcmd does not use there, so it goes on after the prefix.
        if self.focus_is_term() && k.code != KeyCode::Esc {
            return self.in_shell_prefix(first, k);
        }
        if matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
            self.status.clear();
            return true;
        }
        let Some(a) = self.settings.keymap.in_group(first, k) else {
            self.status.clear();
            return false;
        };
        if a.repeats() {
            self.group = Some(first);
            self.status = self.group_hint(first);
        } else {
            self.status.clear();
        }
        self.run_bound(a, k);
        // An action may have put up its own message; the group's comes back next key.
        true
    }

    /// The key after the prefix (Alt+X) inside the shell. One of the prefix's own keys
    /// that works in the shell (F12, for the history) runs; a key whose action acts on
    /// the shell (Alt+S, Alt+W, Alt+1…) runs on it; anything else was the program's all
    /// along, so the prefix and the key both go to it, as if xdevcmd had not looked.
    /// Always `true`: in the shell no key falls through to a second meaning.
    pub(super) fn in_shell_prefix(&mut self, first: crate::keys::Chord, k: KeyEvent) -> bool {
        self.status.clear();
        let keymap = &self.settings.keymap;
        let run = keymap
            .in_group(first, k)
            .filter(|a| a.is_global())
            .or_else(|| {
                keymap
                    .action_for(k)
                    .filter(|a| a.is_global() || a.acts_on_shell())
            });
        if let Some(a) = run {
            self.run_bound(a, k);
        } else if let Some(Pane::Term(t)) = self.panes.get(&self.focus) {
            t.send_key(KeyEvent::new(first.0, first.1));
            t.send_key(k);
        }
        true
    }

    /// Run `a`, which `k` was bound to.
    pub(super) fn run_bound(&mut self, a: Action, k: KeyEvent) {
        if a == Action::GoTab {
            return self.go_tab(k);
        }
        self.perform(a);
    }

    /// Alt+1…9: the tab whose number is the key's digit, in the file pane or the
    /// terminal, whichever has focus.
    pub(super) fn go_tab(&mut self, k: KeyEvent) {
        let Some(n) = (match k.code {
            KeyCode::Char(c) => c.to_digit(10).filter(|n| *n > 0),
            _ => None,
        }) else {
            self.status = "go_tab needs a key ending in a digit, 1 to 9".into();
            return;
        };
        let i = n as usize - 1;
        // The tabs of whatever has focus. A file pane with one tab shows no tab bar,
        // while the terminal's numbered tabs are on screen: say which were counted.
        let (count, whose) = if self.focus_is_term() {
            let c = self.panel().map_or(0, |p| p.tab_count());
            if let Some(p) = self.panel().filter(|_| i < c) {
                p.select_tab(i);
                return;
            }
            (c, "the terminal")
        } else {
            let c = self.focused_tabs().map_or(0, |t| t.len());
            if let Some(t) = self.focused_tabs().filter(|_| i < c) {
                t.select(i);
                return;
            }
            (c, "this file pane")
        };
        let has = if count == 1 {
            "one tab".to_string()
        } else {
            format!("{count} tabs")
        };
        let prefix = self
            .settings
            .keymap
            .shell_prefix()
            .map(|c| format!("{} ", crate::keys::key_name(c.0, c.1)))
            .unwrap_or_default();
        self.status = if whose == "this file pane" && self.term_shown {
            format!(
                "no tab {n}: {whose} has {has} \u{2014} F12 to the terminal, then {prefix}Alt+{n} picks its tabs"
            )
        } else {
            format!("no tab {n}: {whose} has {has}")
        };
    }

    pub(super) fn on_key(&mut self, k: KeyEvent) {
        if self.modal.is_some() {
            return self.on_modal_key(k);
        }
        if k.code == KeyCode::Esc {
            if let Some(c) = self.comparing.take() {
                c.stop.store(true, std::sync::atomic::Ordering::Relaxed);
                self.status = "comparison stopped".into();
                return;
            }
        }
        if self.search.is_some() && self.on_search_key(k) {
            return;
        }
        if self.in_group(k) {
            return;
        }
        self.option_hint(k);
        // The first key of a group. In the shell only the group holding F12's two-key
        // forms opens (Alt+X, for Alt+X F12); it is xdevcmd's prefix there. Any other
        // group's first key is the shell's (Alt+R is readline's revert-line).
        if self.settings.keymap.starts_group(k) {
            let first = crate::keys::Keymap::chord(k);
            let prefix = self
                .settings
                .keymap
                .group(first)
                .iter()
                .any(|(_, a)| a.is_global());
            if !self.focus_is_term() || prefix {
                self.group = Some(first);
                self.status = self.group_hint(first);
                return;
            }
        }
        let action = self.settings.keymap.action_for(k);
        // Esc in a filtered pane shows everything again: a filter left on is files you
        // cannot see, so taking it off should be the most obvious key there is.
        if action.is_none() && k.code == KeyCode::Esc && !self.focus_is_term() {
            if self.focused_files().is_some_and(|f| f.stop_search()) {
                self.status = "search stopped".into();
                return;
            }
            if let Some((_, _, name, stop)) = self.pending_view.take() {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                self.status = format!("stopped extracting {name}");
                return;
            }
            if self.focused_files().is_some_and(|f| f.stop_opening()) {
                self.status = "stopped opening the archive".into();
                return;
            }
            if let Some(dir) = self.focused_files().and_then(|f| f.stop_reading()) {
                self.status = format!("stopped reading {} \u{2014} still here", tidy(&dir));
                return;
            }
            if let Some(f) = self.focused_files().filter(|f| f.filter.is_some()) {
                f.set_filter(None);
                self.status = "filter off".into();
                return;
            }
        }

        // F12's family works from anywhere, the shell included (keymap A, owner
        // 2026-11-09). Everything else typed into the shell belongs to the shell.
        if let Some(a) = action.filter(|a| a.is_global()) {
            return self.run_bound(a, k);
        }
        if self.focus_is_term() {
            let ended = matches!(self.panes.get(&self.focus), Some(Pane::Term(t)) if t.focused_program_ended());
            if ended && matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
                let empty = self.panel().is_some_and(|p| p.close_focused());
                if empty {
                    self.remove_terminal_panel();
                }
                // Back to the files the program was started from.
                let files = self.file_panes();
                if let Some(id) = self
                    .mru
                    .iter()
                    .copied()
                    .find(|id| files.contains(id))
                    .or_else(|| files.first().copied())
                {
                    self.focus = id;
                }
                return;
            }
            if let Some(Pane::Term(t)) = self.panes.get(&self.focus) {
                t.send_key(k);
            }
            return;
        }
        if let Some(a) = action {
            self.run_bound(a, k);
        }
    }

    /// Why `a` is turned off in the mode in effect, or `None` when it is allowed. The
    /// one place that decides; the key bar, the context menu and every input path ask
    /// here, and the engine in xdevcmd-core refuses writes again behind it.
    pub(super) fn refused(&self, a: Action) -> Option<String> {
        let mode = self.settings.mode;
        let what = match a {
            Action::Copy | Action::Move => "copying and moving",
            Action::Delete | Action::DeletePermanent => "deleting",
            Action::Rename | Action::BulkRename => "renaming",
            Action::NewFile | Action::CopyAs | Action::Symlink | Action::Pack | Action::Unpack => {
                "creating files"
            }
            Action::SyncDirs => "synchronising",
            Action::Permissions => "changing permissions",
            Action::MkDir => "creating directories",
            Action::Edit => "editing",
            Action::OpenWith => "opening files in other programs",
            Action::Undo => "undoing",
            _ if mode == Mode::Restricted
                && matches!(
                    a,
                    Action::FocusTerminal
                        | Action::TerminalHistory
                        | Action::ToggleTerminal
                        | Action::CdTerminalHere
                        | Action::GoShellDir
                        | Action::TypeNames
                        | Action::NewTerminal
                        | Action::UserMenu
                ) =>
            {
                "the terminal"
            }
            _ => return None,
        };
        mode.read_only()
            .then(|| format!("{} mode \u{2014} {what} is turned off", mode.label()))
    }

    pub(super) fn perform(&mut self, a: Action) {
        if let Some(why) = self.refused(a) {
            self.status = why;
            return;
        }
        // Inside an archive only reading and extracting make sense. Writing into one
        // would mean rewriting the whole file, which xdevcmd does not do.
        if self.focused_archive().is_some()
            && matches!(
                a,
                Action::Delete
                    | Action::DeletePermanent
                    | Action::Move
                    | Action::Rename
                    | Action::BulkRename
                    | Action::MkDir
                    | Action::Edit
                    | Action::NewFile
                    | Action::CopyAs
                    | Action::Symlink
                    | Action::Permissions
                    | Action::OpenWith
                    | Action::Checksum
                    | Action::Pack
                    | Action::Unpack
                    | Action::FindFiles
                    | Action::TypeNames
            )
        {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status =
                format!("inside an archive you can view and extract ({k}) \u{2014} not that");
            return;
        }
        let full = self.screen;
        match a {
            Action::Quit => {
                if self.ask_before_quitting() {
                    return;
                }
                self.quit = true;
                self.quit_cd = true;
            }
            Action::QuitNoCd => {
                if self.ask_before_quitting() {
                    return;
                }
                self.quit = true;
                self.quit_cd = false;
            }
            Action::QuickSearch => self.begin_search(false),
            Action::Filter => self.begin_search(true),
            Action::GoToPath => self.begin_goto(),
            Action::Back => {
                if !self.focused_files().is_some_and(|f| f.go_back()) {
                    self.status = "nothing to go back to".into();
                }
            }
            Action::Forward => {
                if !self.focused_files().is_some_and(|f| f.go_forward()) {
                    self.status = "nothing to go forward to".into();
                }
            }
            Action::History => self.open_history(),
            Action::Hotlist => self.open_hotlist(),
            Action::ShowList => self.show_list(None),
            Action::OpenWith => self.open_with(),
            Action::SwapPanes => self.swap_panes(),
            Action::MirrorPane => self.mirror_pane(),
            Action::MarkPattern => self.begin_pattern(true),
            Action::UnmarkPattern => self.begin_pattern(false),
            Action::BulkRename => {
                let files = self.selection_in_view();
                if files.is_empty() {
                    self.status = "nothing selected".into();
                } else {
                    self.modal = Some(Modal::Rename(Box::new(RenameDialog::new(files))));
                }
            }
            Action::NewFile => {
                let dir = self.focused_cwd();
                self.modal = Some(Modal::Input(Input {
                    title: "New file".into(),
                    label: format!("in {} \u{b7} opens in the editor", dir.display()),
                    edit: LineEdit::new(""),
                    kind: InputKind::NewFile,
                    dir,
                    buttons: Vec::new(),
                    error: None,
                }));
            }
            Action::CopyAs => self.begin_copy_as(),
            Action::Symlink => self.make_symlink(),
            Action::Permissions => self.begin_permissions(),
            Action::Checksum => self.begin_checksum(),
            Action::UserMenu => self.open_user_menu(),
            Action::Pack => self.begin_pack(),
            Action::MenuBar => self.open_bar(0),
            Action::Usage => self.modal = Some(Modal::Usage),
            Action::ContextMenu => self.context_menu_at_cursor(),
            Action::CompareDirs => self.compare_panes(),
            Action::SyncDirs => self.begin_sync(),
            Action::Unpack => self.begin_unpack(),
            Action::QuickView => {
                if self.quick_view.take().is_some() {
                    self.status = "quick view off".into();
                } else if self.focus_is_term() {
                } else if let Some(other) = self.other_file_pane() {
                    self.quick_view = Some((other, None));
                    self.status = "quick view \u{b7} the other pane follows the cursor".into();
                } else {
                    self.status = "quick view needs a second pane".into();
                }
            }
            Action::FindFiles => {
                if let Some(f) = self.focused_files() {
                    let d = FindDialog::new(f.cwd.clone(), f.show_hidden);
                    self.modal = Some(Modal::Find(Box::new(d)));
                }
            }
            Action::CalcSizes => {
                if let Some(f) = self.focused_files() {
                    let n = f.request_all_sizes();
                    self.status = match n {
                        0 => "no directories here".into(),
                        1 => "working out the size of 1 directory".into(),
                        n => format!("working out the size of {n} directories"),
                    };
                }
            }
            Action::Help => {
                let help = Help::new(&self.settings, &self.config_outcome);
                self.modal = Some(Modal::Help(Box::new(help)));
            }
            Action::Refresh => self.with_files(|f| f.reload()),
            Action::ToggleHidden => {
                if let Some(f) = self.focused_files() {
                    f.toggle_hidden();
                    let on = f.show_hidden;
                    self.status = format!("hidden files {}", if on { "shown" } else { "hidden" });
                }
            }
            Action::FocusNextPane => self.focus_next_file(),
            Action::FocusTerminal => self.toggle_term_focus(full),
            Action::TerminalHistory => self.view_history(None),
            Action::ToggleTerminal => self.toggle_term(full),
            Action::CdTerminalHere => self.cd_terminal_here(),
            Action::GoShellDir => self.go_shell_dir(),
            Action::TypeNames => self.type_names(),
            Action::SplitRight => self.split_focused(Axis::Cols),
            Action::SplitDown => self.split_focused(Axis::Rows),
            Action::ClosePane => self.close_focused(),
            Action::ResizeLeft | Action::ResizeRight | Action::ResizeUp | Action::ResizeDown => {
                let (axis, d) = match a {
                    Action::ResizeLeft => (Axis::Cols, -5),
                    Action::ResizeRight => (Axis::Cols, 5),
                    Action::ResizeUp => (Axis::Rows, -5),
                    _ => (Axis::Rows, 5),
                };
                // A border between shells in the terminal tab first, then the layout's.
                let inside =
                    self.focus_is_term() && self.panel().is_some_and(|p| p.move_boundary(axis, d));
                if !inside && !self.tree.move_boundary(self.focus, axis, d) {
                    self.status = "no border that way".into();
                }
            }
            Action::Zoom => self.toggle_zoom(),
            Action::NewTerminal => self.new_terminal(),
            Action::NewTab => {
                if let Some(t) = self.focused_tabs() {
                    t.duplicate();
                }
            }
            Action::CloseTab => {
                let closed = self.focused_tabs().map(|t| {
                    let i = t.active_index();
                    t.close(i)
                });
                if closed == Some(false) {
                    self.status =
                        "that is the pane's only tab \u{2014} Alt+W closes the pane".into();
                }
            }
            // One pair of keys for tabs, acting on whatever has focus.
            Action::NextTab | Action::PrevTab => {
                let d = if a == Action::NextTab { 1 } else { -1 };
                if self.focus_is_term() {
                    self.cycle_terminal(d);
                } else if let Some(t) = self.focused_tabs() {
                    t.cycle(d);
                }
            }
            Action::NextSplit => self.next_split(),
            // Only from a key, which carries the tab's number: see `run_bound`.
            Action::GoTab => {}
            Action::CursorUp => self.with_files(|f| f.move_cursor(-1)),
            Action::CursorDown => self.with_files(|f| f.move_cursor(1)),
            Action::PageUp => self.with_files(|f| f.page(-1)),
            Action::PageDown => self.with_files(|f| f.page(1)),
            Action::Top => self.with_files(|f| f.go_to(0)),
            Action::Bottom => self.with_files(|f| f.go_end()),
            Action::Parent => self.with_files(|f| f.go_up()),
            Action::GoHome => {
                let home = crate::bookmarks::expand_tilde("~");
                if home.is_absolute() {
                    self.with_files(|f| f.set_cwd(&home));
                } else {
                    self.status = "no home directory: neither HOME nor USERPROFILE is set".into();
                }
            }
            Action::GoRoot => self.with_files(|f| {
                if let Some(root) = f.cwd.ancestors().last().map(Path::to_path_buf) {
                    f.set_cwd(&root);
                }
            }),
            Action::EnterDir => {
                // Only ever into a directory: Enter already views files, and a key that
                // sometimes navigates and sometimes opens a viewer is one you stop
                // trusting.
                let is_dir = self
                    .panes
                    .get(&self.focus)
                    .and_then(Pane::as_files)
                    .and_then(|f| f.current())
                    .is_some_and(|e| e.is_dir() && !e.is_parent());
                if is_dir {
                    self.with_files(|f| {
                        f.activate();
                    });
                } else {
                    self.status = "not a directory".into();
                }
            }
            Action::Open => {
                // Entering a directory navigates; on a file, Enter opens it (or views
                // it, with `[files] enter = "view"`).
                if self.focused_files().and_then(|f| f.activate()).is_some() {
                    self.enter_file();
                }
            }
            Action::OpenWithMenu => self.open_with_menu(),
            Action::Undo => self.ask_undo(),
            // Space on a directory also works out its size, as in Total Commander: the
            // moment you select a directory is the moment you want to know.
            Action::Mark => self.with_files(|f| {
                f.toggle_mark();
                if let Some(e) = f.current().filter(|e| e.is_dir() && !e.is_parent()) {
                    if f.marked.contains(&e.path) {
                        let p = e.path.clone();
                        f.request_size(&p);
                    }
                }
            }),
            Action::MarkAndNext => self.with_files(|f| {
                f.toggle_mark();
                f.move_cursor(1);
            }),
            Action::InvertMarks => self.with_files(|f| f.invert_marks()),
            Action::MarkAll => self.with_files(|f| f.mark_all(true)),
            Action::UnmarkAll => self.with_files(|f| f.mark_all(false)),
            Action::View => self.begin_view(),
            Action::Edit => self.begin_edit(),
            Action::Copy => self.begin_transfer(ops::Kind::Copy),
            Action::Move => self.begin_transfer(ops::Kind::Move),
            Action::MkDir => self.begin_mkdir(),
            Action::Rename => self.begin_rename(),
            Action::Delete => {
                let permanent = !self.settings.config.general.delete_to_trash;
                self.begin_delete(permanent);
            }
            Action::DeletePermanent => self.begin_delete(true),
            Action::SortName => self.with_files(|f| f.set_sort(SortKey::Name)),
            Action::SortExt => self.with_files(|f| f.set_sort(SortKey::Ext)),
            Action::SortSize => self.with_files(|f| f.set_sort(SortKey::Size)),
            Action::SortModified => self.with_files(|f| f.set_sort(SortKey::Modified)),
        }
    }

    pub(super) fn perform_cmd(&mut self, c: Cmd) {
        match c {
            Cmd::Act(a) => self.perform(a),
            Cmd::CopyPath if self.settings.mode == Mode::Restricted => {
                self.status = "restricted mode \u{2014} the clipboard is turned off".into();
            }
            Cmd::Transfer { .. } if self.settings.mode.read_only() => {
                self.status = format!(
                    "{} mode \u{2014} copying and moving is turned off",
                    self.settings.mode.label()
                );
            }
            Cmd::AddBookmark(_)
            | Cmd::RemoveBookmark(_)
            | Cmd::OpenFile(_)
            | Cmd::BatchRename(_)
            | Cmd::Sync(_)
                if self.settings.mode.read_only() =>
            {
                self.status = format!(
                    "{} mode \u{2014} that is turned off",
                    self.settings.mode.label()
                );
            }
            Cmd::CopyPath => self.copy_path(),
            Cmd::ShowList(id) => self.show_list(Some(id)),
            Cmd::AddToList(id) => self.add_to_list(id),
            Cmd::GoTo(p) => {
                if p.is_dir() {
                    self.with_files(|f| f.set_cwd(&p));
                } else {
                    self.status = format!("{} is not there any more", tidy(&p));
                }
            }
            Cmd::AddBookmark(p) => {
                if !self.bookmarks.contains(&p) {
                    self.bookmarks.push(p.clone());
                }
                self.save_bookmarks(format!("bookmarked {}", tidy(&p)));
            }
            Cmd::RemoveBookmark(p) => {
                self.bookmarks.retain(|b| *b != p);
                self.save_bookmarks(format!("removed the bookmark for {}", tidy(&p)));
            }
            // Checked again here: a link can be pointed elsewhere between the menu that
            // offered this and the choice.
            Cmd::OpenFile(p) if !xdevcmd_core::policy::get().allows(&p) => {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Cmd::OpenFile(p) => self.launch(&p),
            Cmd::RunInTerminal { argv, .. }
                if argv
                    .first()
                    .is_some_and(|p| !xdevcmd_core::policy::get().allows(Path::new(p))) =>
            {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Cmd::RunInTerminal { argv, cwd } => self.run_in_terminal(argv, cwd),
            Cmd::ThemeMenu => self.theme_menu(),
            Cmd::Settings => self.open_settings(),
            Cmd::Undo => self.undo(),
            Cmd::SetTheme(name) => self.set_theme(&name),
            Cmd::ShellNew(s) => self.open_shell_for(*s),
            Cmd::ShellSendHere(s) => self.send_here(*s),
            Cmd::ApplyRule(i, p) => self.apply_rule(i, p),
            Cmd::UserCommand(i) => self.run_user_command(i),
            Cmd::About => {
                self.modal = Some(Modal::Ask(modal::Ask {
                    title: format!("xdevcmd {}", env!("CARGO_PKG_VERSION")),
                    // All from [workspace.package] in Cargo.toml: one place to change.
                    body: vec![
                        env!("CARGO_PKG_DESCRIPTION").into(),
                        String::new(),
                        format!("{} licence", env!("CARGO_PKG_LICENSE")),
                        env!("CARGO_PKG_REPOSITORY").into(),
                        String::new(),
                        "No network, no telemetry.".into(),
                        // Empty in screenshots: no terminal was asked.
                        if self.usage.terminal.is_empty() {
                            String::new()
                        } else {
                            format!("Running in {}", self.usage.terminal)
                        },
                        // xdevcmd never looks for updates itself (no network): where to.
                        format!("Releases: {}/releases", env!("CARGO_PKG_REPOSITORY")),
                    ],
                    danger: false,
                    yes: None,
                    buttons: Vec::new(),
                    also: None,
                    footnote: Some(format!(
                        "Developed by {}",
                        env!("CARGO_PKG_AUTHORS").replace(':', ", ")
                    )),
                    pictures: {
                        let bg = match theme::t().bg_panel {
                            ratatui::style::Color::Rgb(r, g, b) => [r, g, b],
                            _ => [0, 0, 0],
                        };
                        xdevcmd_core::image::png_bytes(LOGO, 14, 14, bg)
                            .into_iter()
                            .collect()
                    },
                }));
            }
            Cmd::QuitAnyway => {
                // Cancelled, and waited for: a worker stops between writes and removes
                // its half-written file, which is what makes quitting now safe. The wait
                // is the main loop's (`stop_step`), so the screen says what is going on.
                for j in &self.jobs {
                    j.handle.cancel();
                }
                self.stopping = Some(Instant::now() + STOP_WAIT);
                self.stop_step();
            }
            Cmd::QuitNow => {
                self.quit = true;
                self.quit_cd = true;
            }
            Cmd::Resume(it) => self.resume(it),
            Cmd::Sync(pairs) => {
                let req = Request::sync(pairs);
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot("a synchronisation".into()));
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            Cmd::CopyText(t) => self.copy_text(t),
            Cmd::TabHistory(id) => {
                if let Some(i) = self.tab_still_there(id) {
                    self.view_history(Some(i));
                }
            }
            Cmd::TabList(id) => {
                // As `p` in the history with nothing chosen: the last command's output,
                // through a viewer that is never shown.
                if let Some(i) = self.tab_still_there(id) {
                    self.view_history(Some(i));
                    let Some(Modal::View(mut v)) = self.modal.take() else {
                        return;
                    };
                    match v.list_request(false) {
                        crate::viewer::Action::MakeList(req, _) => self.make_list(req, false),
                        _ => {
                            let key = self.footer_key(Action::TerminalHistory).unwrap_or_default();
                            self.status = format!(
                                "{} \u{2014} or {key}, drag over the lines, p",
                                v.message()
                            );
                        }
                    }
                }
            }
            Cmd::TabSaveHistory(id) => {
                // The history goes into a viewer that is never shown: Save as… holds it
                // and lets it go when its dialogs close.
                if let Some(i) = self.tab_still_there(id) {
                    self.view_history(Some(i));
                    if matches!(self.modal, Some(Modal::View(_))) {
                        self.begin_save_as();
                        self.save_only = self.save_from.is_some();
                    }
                }
            }
            Cmd::TabClose(id) => {
                if let Some(i) = self.tab_still_there(id) {
                    let nothing_left = self.panel().is_some_and(|p| p.close_tab(i));
                    if nothing_left {
                        self.remove_terminal_panel();
                    }
                }
            }
            Cmd::SaveReplacing(dest) => {
                let saved = self
                    .save_from
                    .as_ref()
                    .and_then(|v| v.doc.memory_bytes())
                    .map(|b| xdevcmd_core::document::save_bytes(&dest, b, true, true));
                self.status = match saved {
                    Some(Ok(())) => format!("saved to {}, replacing it", dest.display()),
                    Some(Err(e)) => format!("cannot save: {e}"),
                    None => "nothing to save".into(),
                };
            }
            Cmd::BatchRename(moves) => {
                let out = xdevcmd_core::batch_rename::apply(&moves, xdevcmd_core::ops::safe());
                self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(moves.clone()));
                self.reload_all();
                self.status = match (out.renamed, out.errors.first()) {
                    (n, None) => format!("renamed {n}"),
                    (n, Some(e)) => {
                        format!("renamed {n}, {} failed \u{2014} {e}", out.errors.len())
                    }
                };
            }
            Cmd::Transfer {
                kind,
                sources,
                dest,
            } => {
                let request = match kind {
                    ops::Kind::Move => Request::mv(sources, dest),
                    _ => Request::copy(sources, dest),
                };
                self.remember_job(&request);
                self.modal = Some(Modal::Job(Job::new(kind, ops::start(request))));
            }
        }
    }

    /// Put text on the clipboard with OSC 52, unless the mode forbids it or the text
    /// could act when pasted.
    pub(super) fn copy_text(&mut self, t: String) {
        if self.settings.mode == Mode::Restricted {
            // Short: it also has to fit beside the viewer's key hints.
            self.status = "restricted mode: no clipboard".into();
            return;
        }
        if let Some(why) = crate::safety::unsafe_to_paste(&t) {
            self.status = why;
            return;
        }
        if self.interactive {
            let seq = format!("\x1b]52;c;{}\x07", base64(t.as_bytes()));
            let mut out = stdout();
            let _ = out.write_all(seq.as_bytes());
            let _ = out.flush();
        }
        let lines = t.lines().count().max(1);
        self.status = match lines {
            1 => "copied 1 line to the clipboard".into(),
            n => format!("copied {n} lines to the clipboard"),
        };
    }

    /// Put the selection's paths on the clipboard with OSC 52.
    ///
    /// The terminal does the copying, so it lands on the clipboard of the machine you
    /// are sitting at — including across SSH, which nothing running on the server could
    /// do any other way. Whether it works is the terminal's call: Windows Terminal,
    /// iTerm2, kitty and WezTerm allow it; tmux needs `set-clipboard on`.
    pub(super) fn copy_path(&mut self) {
        let mut paths = self.selection();
        if paths.is_empty() {
            paths.push(self.focused_cwd());
        }
        let text = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        // Pasted into a terminal, a control character in a path runs commands just as
        // typing it would. Refuse rather than strip: a stripped path is a wrong path.
        if let Some(why) = paths
            .iter()
            .find_map(|p| crate::safety::unsafe_to_type(&p.display().to_string()))
        {
            self.status = why;
            return;
        }
        if self.interactive {
            let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
            let mut out = stdout();
            let _ = out.write_all(seq.as_bytes());
            let _ = out.flush();
        }
        self.status = match paths.as_slice() {
            [one] => format!("sent {} to the clipboard", one.display()),
            many => format!("sent {} paths to the clipboard", many.len()),
        };
    }

    /// What Settings' answer to a key or a click means here: a theme to preview, the
    /// theme put back on cancel, or the settings to save.
    pub(super) fn settings_outcome(&self, o: settings_dialog::Outcome) -> ModalAct {
        let (spec, depth) = (&self.settings.config.theme, self.settings.depth);
        let show = |name: &str| {
            if let Ok(t) = crate::config::theme_for(name, spec, depth) {
                theme::set(t);
            }
        };
        match o {
            settings_dialog::Outcome::Stay => ModalAct::None,
            settings_dialog::Outcome::Preview(name) => {
                show(&name);
                ModalAct::None
            }
            settings_dialog::Outcome::Cancel => {
                // Put back the theme the preview replaced.
                show(&self.theme_name);
                ModalAct::Close
            }
            settings_dialog::Outcome::Save => ModalAct::SaveSettings,
        }
    }

    /// The key to advertise for an action: an F-key if one is bound, since that is
    /// what the bar's layout expects, otherwise whatever is bound first.
    pub(super) fn footer_key(&self, action: Action) -> Option<String> {
        let keys = self.settings.keymap.keys_for(action);
        keys.iter()
            .find(|k| k.strip_prefix('F').is_some_and(|d| d.parse::<u8>().is_ok()))
            .or(keys.first())
            .cloned()
    }
}
