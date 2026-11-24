//! The terminal panel: shells and tabs, history and Save as, typing names and commands into a shell, following it.

use super::*;

impl App {
    pub(super) fn toggle_term(&mut self, area: Rect) {
        match (self.shell_pane.term_id, self.shell_pane.term_shown) {
            (Some(id), true) => {
                // Remember the height it was dragged to, so showing it again restores it.
                if let Node::Split { pct, .. } = &self.tree {
                    self.shell_pane.term_height = 100 - *pct;
                }
                self.tree.remove_leaf(id);
                self.shell_pane.term_shown = false;
                if self.focus == id {
                    self.focus = self.file_panes().first().copied().unwrap_or(0);
                }
            }
            (Some(id), false) => {
                let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                self.tree = Node::split(
                    Axis::Rows,
                    100 - self.shell_pane.term_height,
                    old,
                    Node::Leaf(id),
                );
                self.shell_pane.term_shown = true;
                self.focus = id;
            }
            (None, _) => {
                let cwd = self
                    .shell_pane
                    .next_shell_cwd
                    .take()
                    .unwrap_or_else(|| self.focused_cwd());
                let pct = self.shell_pane.term_height;
                let rows = (area.height * pct / 100).saturating_sub(2).max(1);
                let cols = area.width.saturating_sub(2).max(1);
                let shell = self.settings.config.terminal.shell.clone();
                let scrollback = self.settings.config.terminal.scrollback;
                match TermPanel::new(&cwd, rows, cols, &shell, scrollback) {
                    Ok(t) => {
                        let id = self.alloc_id();
                        self.panes.insert(id, Pane::Term(Box::new(t)));
                        let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                        self.tree = Node::split(
                            Axis::Rows,
                            100 - self.shell_pane.term_height,
                            old,
                            Node::Leaf(id),
                        );
                        self.shell_pane.term_id = Some(id);
                        self.shell_pane.term_shown = true;
                    }
                    Err(e) => self.status = tr!("shell.no_shell", "no shell: {e}", e = e),
                }
            }
        }
    }

    /// The panel has no shells left: take it out of the layout entirely, so the next
    /// F12 starts a fresh one rather than showing an empty frame.
    pub(super) fn remove_terminal_panel(&mut self) {
        if let Some(id) = self.shell_pane.term_id.take() {
            if self.shell_pane.term_shown {
                self.tree.remove_leaf(id);
            }
            self.panes.remove(&id);
            self.shell_pane.term_shown = false;
            if self.focus == id {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
            }
        }
    }

    pub(super) fn panel(&mut self) -> Option<&mut TermPanel> {
        match self.panes.get_mut(&self.shell_pane.term_id?) {
            Some(Pane::Term(p)) => Some(p),
            _ => None,
        }
    }

    /// A new terminal tab, showing the panel if it was hidden and creating it if there
    /// was none — in which case the panel's first tab is the new terminal.
    pub(super) fn new_terminal(&mut self) {
        let full = self.hits.screen;
        match (self.shell_pane.term_id, self.shell_pane.term_shown) {
            (None, _) => self.toggle_term(full),
            (Some(_), shown) => {
                if !shown {
                    self.toggle_term(full);
                }
                if !self.terminal_room() {
                    // Still show and focus the panel: the request was "a terminal",
                    // and the ones already running are right there.
                    if let Some(id) = self.shell_pane.term_id {
                        self.focus = id;
                    }
                    return;
                }
                let cwd = self.focused_cwd();
                if let Some(Err(e)) = self.panel().map(|p| p.add_tab(&cwd)) {
                    self.status = tr!("shell.no_shell", "no shell: {e}", e = e);
                }
            }
        }
        if let Some(id) = self.shell_pane.term_id {
            self.focus = id;
        }
    }

    /// Whether one more shell fits under `[terminal] max_terminals`; says why if not.
    pub(super) fn terminal_room(&mut self) -> bool {
        let max = self.settings.config.terminal.max_terminals;
        let running = self.panel().map_or(0, |p| p.shell_count());
        if running >= max {
            self.status = tr!(
                "shell.at_most_terminals_u_terminal_max",
                "at most {max} terminals \u{2014} [terminal] max_terminals in config.toml",
                max = max
            );
            return false;
        }
        true
    }

    pub(super) fn cycle_terminal(&mut self, delta: isize) {
        let Some(id) = self.shell_pane.term_id else {
            self.status = tr!(
                "shell.no_terminal_yet_u_f12",
                "no terminal yet \u{2014} F12"
            )
            .into();
            return;
        };
        if !self.shell_pane.term_shown {
            let full = self.hits.screen;
            self.toggle_term(full);
        }
        if let Some(p) = self.panel() {
            p.cycle_tab(delta);
        }
        self.focus = id;
    }

    pub(super) fn next_split(&mut self) {
        if self.focus_is_term() {
            if let Some(p) = self.panel() {
                p.next_split();
            }
        } else {
            self.focus_next_file();
        }
    }

    /// Shells that have exited close themselves, as `exit` does in an editor.
    /// True when a shell went, so the screen changed.
    pub(super) fn reap_terminals(&mut self) -> bool {
        let Some(p) = self.panel() else { return false };
        let (gone, empty) = p.reap();
        // A log that ended with its shell says so, before the panel can go with it.
        let notes = p.take_notes();
        if gone > 0 {
            self.status = if gone == 1 {
                tr!("shell.shell_exited", "shell exited").into()
            } else {
                tr!("shell.shells_exited", "{gone} shells exited", gone = gone)
            };
        }
        if let Some(last) = notes.last() {
            self.status = last.clone();
        }
        if empty {
            self.remove_terminal_panel();
        }
        gone > 0 || empty
    }

    /// Jump between the file panes and the shell, the way Ctrl+` works in an editor.
    pub(super) fn toggle_term_focus(&mut self, area: Rect) {
        match self.shell_pane.term_id {
            Some(id) if self.shell_pane.term_shown && self.focus != id => self.focus = id,
            Some(id) if self.shell_pane.term_shown => {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
                let _ = id;
            }
            _ => {
                // toggle_term leaves focus alone because startup uses it too, but F12
                // means "take me to the shell": a new one that left you typing into
                // the file panes would be a trap.
                self.toggle_term(area);
                if let Some(id) = self.shell_pane.term_id {
                    self.focus = id;
                }
            }
        }
    }

    /// Shift+F12: the shell's history and screen in the viewer, held in memory — a
    /// shell's output can hold tokens and customer data, so none of it is written
    /// anywhere unless the user saves it. The focused shell, or from the files the
    /// shell the panel last had in focus; `tab` names another tab's (the tab menu).
    /// The viewer is a dialog over everything, so Esc leaves focus where it was.
    pub(super) fn view_history(&mut self, tab: Option<usize>) {
        if self.shell_pane.term_id.is_none() {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = tr!(
                "shell.no_terminal_yet_u",
                "no terminal yet \u{2014} {key}",
                key = key
            );
            return;
        }
        let Some(h) = self.panel().and_then(|p| p.history(tab)) else {
            self.status = tr!("shell.no_such_terminal_tab", "no such terminal tab").into();
            return;
        };
        self.history_dir = Some(h.cwd.unwrap_or_else(|| self.focused_cwd()));
        let lines = h.text.lines().count();
        let doc = Document::from_bytes(h.title, h.text.into_bytes());
        let v = &self.settings.config.viewer;
        let mut viewer = Viewer::open(doc, v.wrap, v.tab_width);
        viewer.show_end();
        let note = if h.full_screen {
            // The shell's lines are behind the program's screen, and vt100 keeps no
            // history for that screen: say so rather than count a screenful as history.
            tr!(
                "shell.a_full_screen_program_is_running",
                "a full-screen program is running \u{2014} its screen only \u{2014} Esc back"
            )
            .into()
        } else {
            tr!(
                "shell.line_in_memory_only_u_esc",
                "{lines} line{s}, in memory only \u{2014} Esc back",
                lines = lines,
                s = if lines == 1 { "" } else { "s" }
            )
        };
        viewer.note(&note);
        let mut links = crate::links::Links::new(h.commands, h.dir_now);
        links.label = h.label;
        links.remote = h.remote;
        let viewer = viewer.with_links(links);
        self.modal = Some(Modal::View(Box::new(viewer)));
    }

    /// Enter, F3 or F4 on a path chosen in a shell's history. What the output said is
    /// not trusted: the path is checked as from a file pane, and the status line names
    /// it in full.
    pub(super) fn history_link(&mut self, act: viewer::LinkAct, link: crate::links::Link) {
        let canonical =
            xdevcmd_core::paths::canonical(&link.path).unwrap_or_else(|_| link.path.clone());
        let refuse = |s: &mut Self, why: &str| {
            if let Some(Modal::View(v)) = s.modal.as_mut() {
                v.note(why);
            }
        };
        if !xdevcmd_core::policy::get().allows(&canonical) {
            return refuse(
                self,
                &tr!(
                    "shell.outside_root",
                    "that path is outside the root xdevcmd is confined to"
                ),
            );
        }
        match act {
            viewer::LinkAct::Go => {
                let Some(id) = self.last_files_id() else {
                    return refuse(self, &tr!("shell.no_file_pane", "no file pane to go there"));
                };
                let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) else {
                    return;
                };
                if link.dir {
                    f.set_cwd(&canonical);
                } else if let (Some(parent), Some(name)) =
                    (canonical.parent(), canonical.file_name())
                {
                    f.go_to_file(parent, name);
                }
                self.modal = None;
                self.return_to = None;
                self.focus = id;
                self.note_focus();
                self.status = tidy(&canonical);
            }
            viewer::LinkAct::View => match Document::open(&canonical) {
                Ok(doc) => {
                    let v = &self.settings.config.viewer;
                    let mut viewer = Viewer::open(doc, v.wrap, v.tab_width).with_syntax(
                        crate::viewer::colours_on(v.highlight),
                        crate::viewer::COLOUR_MAX,
                        true,
                    );
                    if let Some(n) = link.line {
                        viewer.show_line(n);
                    }
                    viewer.note_path(crate::links::Said {
                        before: String::new(),
                        path: tidy(&canonical),
                        after: tr!(
                            "shell.u_esc_back_to_the_history",
                            " \u{2014} Esc back to the history"
                        )
                        .into(),
                    });
                    if let Some(Modal::View(h)) = self.modal.take() {
                        self.return_to = Some(h);
                    }
                    self.modal = Some(Modal::View(Box::new(viewer)));
                }
                Err(e) => refuse(
                    self,
                    &tr!(
                        "shell.cannot_view",
                        "cannot view {v}: {e}",
                        v = tidy(&canonical),
                        e = e
                    ),
                ),
            },
            viewer::LinkAct::Edit => {
                if let Some(why) = self.refused(Action::Edit) {
                    return refuse(self, &why);
                }
                // The history stays open: it is where the user comes back to.
                self.pending_edit = Some(canonical);
                self.edit_at = link.line.map(|n| (n, link.col));
            }
        }
    }

    /// Where the tab a menu was opened on is now, or a message that it has gone (its
    /// shells ended while the menu was open).
    pub(super) fn tab_still_there(&mut self, id: crate::pane::terms::TabId) -> Option<usize> {
        let i = self.panel().and_then(|p| p.tab_index(id));
        if i.is_none() {
            self.status = tr!(
                "shell.that_terminal_tab_is_gone",
                "that terminal tab is gone"
            )
            .into();
        }
        i
    }

    /// The menu on terminal tab `i` (right-click on it): only what matters (owner,
    /// 2026-11-09), each with the key that does it, so the menu also teaches the keys.
    /// New tab, split and paste are left out (the + button, Alt+N/S/D and the terminal's
    /// own paste cover them), and so is clearing the history, which is destructive and
    /// rarely wanted. Every item acts on the tab clicked, not the focused one.
    pub(super) fn open_tab_menu(&mut self, i: usize, x: u16, y: u16) {
        let history = self.footer_key(Action::TerminalHistory).unwrap_or_default();
        let save = if history.is_empty() {
            String::new()
        } else {
            tr!("shell.f2", "{history}, F2", history = history)
        };
        let writes = self.refused(Action::CopyAs).is_none();
        // The tab's identity, not its position: shells are reaped every turn while the
        // menu is open, and an earlier tab ending would move this one's position.
        let Some(id) = self.panel().and_then(|p| p.tab_id(i)) else {
            return;
        };
        let items = vec![
            Item::cmd(
                tr!("menu.view_history", "View history"),
                history,
                Cmd::TabHistory(id),
                true,
            ),
            Item::cmd(
                tr!("menu.save_history_as", "Save history as\u{2026}"),
                save,
                Cmd::TabSaveHistory(id),
                writes,
            ),
            match self.panel().and_then(|p| p.tab_log(i)) {
                Some(_) => Item::cmd(
                    tr!("menu.stop_logging", "Stop logging"),
                    "\u{25cf}",
                    Cmd::TabStopLog(id),
                    true,
                ),
                None => Item::cmd(
                    tr!("menu.log_to_file", "Log to file\u{2026}"),
                    "",
                    Cmd::TabLog(id),
                    writes,
                ),
            },
            Item::cmd(
                tr!(
                    "menu.list_from_the_last_command",
                    "List from the last command\u{2026}"
                ),
                "",
                Cmd::TabList(id),
                true,
            ),
            Item::sep(),
            Item::cmd(
                tr!("menu.close_tab", "Close tab"),
                "\u{00d7}",
                Cmd::TabClose(id),
                true,
            ),
        ];
        self.modal = Some(Modal::Menu(Menu::new(
            Some(tr!("shell.terminal", "Terminal {v}", v = i + 1)),
            items,
            (x, y),
        )));
    }

    /// The tab menu's Log to file… (and F1's row): ask where, Save as…-style. Offered
    /// as `xdevcmd-<shell>-<date>-<time>.log` in the shell's directory when the system
    /// gave it, else the pane's. Tab in the prompt switches text and raw.
    pub(super) fn begin_log(&mut self, id: crate::pane::terms::TabId) {
        // Logging is creating a file, which is what Copy as… is refused for.
        if let Some(why) = self.refused(Action::CopyAs) {
            self.status = why;
            return;
        }
        let Some(i) = self.tab_still_there(id) else {
            return;
        };
        let pane_dir = self.focused_cwd();
        let Some(p) = self.panel() else { return };
        if let Some(l) = p.tab_log(i) {
            self.status = tr!(
                "shell.this_tab_is_already_logged_to",
                "this tab is already logged to {path}",
                path = l.path().display()
            );
            return;
        }
        let shell = p.tab_shell_name(i).unwrap_or_default();
        // Only a directory the system gave: the history's rule for Save as….
        let dir = p
            .history(Some(i))
            .and_then(|h| h.cwd)
            .filter(|d| d.to_str().is_some())
            .unwrap_or(pane_dir);
        let name = xdevcmd_core::termlog::file_name(&shell);
        self.modal = Some(Modal::Input(Input {
            title: tr!(
                "shell.log_terminal_to_file",
                "Log terminal {v} to file",
                v = i + 1
            ),
            label: log_label(false),
            edit: LineEdit::new(&dir.join(name).display().to_string()),
            kind: InputKind::LogTo {
                tab: id,
                raw: false,
            },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Enter in Log to file: open the file (appended; `0600` when new) and start.
    pub(super) fn apply_log_to(&mut self, i: Input) {
        let InputKind::LogTo { tab, raw } = i.kind else {
            return;
        };
        let refuse = |app: &mut Self, i: Input, why: String| {
            app.modal = Some(Modal::Input(Input {
                error: Some(why),
                ..i
            }));
        };
        if let Some(why) = self.refused(Action::CopyAs) {
            self.status = why;
            return;
        }
        let dest = match self.typed_destination(&i) {
            Ok(Some(d)) => d,
            Ok(None) => return,
            Err(why) => return refuse(self, i, why),
        };
        let Some(n) = self.tab_still_there(tab) else {
            return;
        };
        let mode = if raw {
            xdevcmd_core::termlog::Mode::Raw
        } else {
            xdevcmd_core::termlog::Mode::Text
        };
        let Some(p) = self.panel() else { return };
        match p.start_log(n, &dest, mode) {
            Ok(said) => self.status = said,
            Err(why) => refuse(self, i, why),
        }
    }

    /// The tab menu's Stop logging (and F1's row).
    pub(super) fn stop_log(&mut self, id: crate::pane::terms::TabId) {
        let Some(i) = self.tab_still_there(id) else {
            return;
        };
        self.status = self.panel().and_then(|p| p.stop_log(i)).unwrap_or_else(|| {
            tr!(
                "shell.this_tab_is_not_being_logged",
                "this tab is not being logged"
            )
            .into()
        });
    }

    /// What the terminal panel's logs have to say, on the status line. Whether
    /// anything was said.
    pub(super) fn log_notes(&mut self) -> bool {
        let notes = self.panel().map(|p| p.take_notes()).unwrap_or_default();
        match notes.last() {
            Some(last) => {
                self.status = last.clone();
                true
            }
            None => false,
        }
    }

    /// Quitting: every log ended and flushed, each given two seconds at most. There is
    /// no screen left to say it on; a failure was said when it happened.
    pub(super) fn close_logs(&mut self) {
        if let Some(p) = self.panel() {
            let _ = p.close_logs();
        }
    }

    /// F2 in the viewer: ask where to save what it shows. History (held in memory) is
    /// offered as `xdevcmd-history-<date>-<time>.txt` in its shell's directory; a file
    /// under its own name in its own directory, to be changed. Refused as any other way
    /// of creating a file is, in read-only and restricted modes.
    pub(super) fn begin_save_as(&mut self) {
        // Saving is creating a file, which is what Copy as… is refused for.
        if let Some(why) = self.refused(Action::CopyAs) {
            if let Some(Modal::View(v)) = self.modal.as_mut() {
                v.note(&why);
            }
            self.status = why;
            return;
        }
        let Some(Modal::View(v)) = self.modal.take() else {
            return;
        };
        let (dir, name) = if v.doc.in_memory {
            (
                self.history_dir
                    .clone()
                    .unwrap_or_else(|| self.focused_cwd()),
                xdevcmd_core::document::history_file_name(),
            )
        } else {
            let p = &v.doc.path;
            (
                p.parent()
                    .map_or_else(|| self.focused_cwd(), Path::to_path_buf),
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        };
        // The prefill is text: a directory whose name is not UTF-8 would come back as
        // another, with U+FFFD in it. The pane's directory is offered instead.
        let dir = if dir.to_str().is_some() {
            dir
        } else {
            self.focused_cwd()
        };
        let what = if v.doc.in_memory {
            tr!(
                "app_shell.the_history_readable_by_you_only",
                "the history, readable by you only"
            )
        } else {
            tr!("app_shell.a_copy_of_this_file", "a copy of this file")
        };
        self.save_from = Some(v);
        self.modal = Some(Modal::Input(Input {
            title: tr!("shell.save_as", "Save as").into(),
            label: tr!("shell.to", "{what}, to:", what = what),
            edit: LineEdit::new(&dir.join(name).display().to_string()),
            kind: InputKind::SaveAs,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Where the text typed in a Save as…-style prompt points, checked as Save as…
    /// checks it: no control character, inside the root (`Policy::allows_at`), a name
    /// Rename would take, not a directory, in a folder that exists. `Ok(None)` when nothing was typed.
    pub(super) fn typed_destination(&self, i: &Input) -> Result<Option<PathBuf>, String> {
        let raw = i.edit.text();
        // The prefill comes from a file's name or the shell's directory, which someone
        // else may have chosen: a control character in it is refused, as everywhere a
        // name could later be typed into a shell or pasted. In the whole text, before
        // the trim: a tab or a newline at either end is not a space to drop (T5, H-08).
        if let Some(c) = xdevcmd_core::controls::first(&raw, &[]) {
            return Err(tr!(
                "shell.in_the_name_u_remove_it",
                "{v} in the name \u{2014} remove it",
                v = xdevcmd_core::controls::named(c)
            ));
        }
        let typed = raw.trim();
        if typed.is_empty() {
            return Ok(None);
        }
        let mut dest = expand_tilde(typed);
        if dest.is_relative() {
            dest = i.dir.join(dest);
        }
        // Under --root: the one check of a path to make (links resolved, a link left
        // there that leads out is outside, no `..` in what is still to make).
        if !xdevcmd_core::policy::get().allows_at(&dest) {
            return Err(tr!(
                "shell.outside_the_root_xdevcmd_is_confined",
                "outside the root xdevcmd is confined to"
            )
            .into());
        }
        // The name itself as Rename checks one: Windows' rules where they hold, which
        // the system would refuse in its own words once the job ran.
        if let Some(name) = dest.file_name().and_then(|n| n.to_str()) {
            let dir = dest.parent().unwrap_or(std::path::Path::new("."));
            if let Err(e) = xdevcmd_core::rename::name_for(dir, name) {
                return Err(e.to_string());
            }
        }
        if dest.is_dir() {
            return Err(tr!(
                "shell.that_is_a_directory_u_add",
                "that is a directory \u{2014} add a file name"
            )
            .into());
        }
        // Said plainly here, before a job is started that would end in the system's
        // own words for it.
        if !dest.parent().is_some_and(Path::is_dir) {
            return Err(tr!(
                "shell.that_folder_does_not_exist",
                "that folder does not exist"
            )
            .into());
        }
        Ok(Some(dest))
    }

    /// Enter in Save as…: a document in memory is written by the core
    /// (`document::save_bytes`, `0600`), asking first if the name is taken; a file is
    /// copied by an ordinary job, which asks its own question about a name taken.
    pub(super) fn apply_save_as(&mut self, i: Input) {
        let refuse = |app: &mut Self, i: Input, why: String| {
            app.modal = Some(Modal::Input(Input {
                error: Some(why),
                ..i
            }));
        };
        let dest = match self.typed_destination(&i) {
            Ok(Some(d)) => d,
            Ok(None) => return,
            Err(why) => return refuse(self, i, why),
        };
        let Some(v) = self.save_from.as_ref() else {
            return;
        };
        if let Some(bytes) = v.doc.memory_bytes() {
            let len = bytes.len() as u64;
            match xdevcmd_core::document::save_bytes(&dest, bytes, false, true) {
                Ok(()) => {
                    self.status = tr!("shell.saved_to", "saved to {dest}", dest = dest.display())
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // The same question a copy asks, before anything is touched.
                    let on_disk = std::fs::metadata(&dest).map_or(0, |m| m.len());
                    self.modal = Some(Modal::Ask(modal::Ask {
                        title: tr!("shell.already_exists", "Already exists").into(),
                        body: vec![
                            dest.display().to_string(),
                            String::new(),
                            tr!("shell.on_disk", "on disk   {v}", v = human_size(on_disk)),
                            format!("incoming  {}", human_size(len)),
                        ],
                        danger: true,
                        yes: Some((
                            tr!("shell.replace", "Replace").into(),
                            Cmd::SaveReplacing(dest),
                        )),
                        buttons: Vec::new(),
                        also: None,
                        more: None,
                        footnote: None,
                        pictures: Vec::new(),
                    }));
                }
                Err(e) => refuse(self, i, tr!("shell.cannot_save", "cannot save: {e}", e = e)),
            }
            return;
        }
        let from = v.doc.path.clone();
        let (Some(dir), Some(name)) = (dest.parent(), dest.file_name()) else {
            return refuse(
                self,
                i,
                tr!("shell.not_a_file_name", "not a file name").into(),
            );
        };
        let mut req = Request::copy(vec![from], dir.to_path_buf());
        req.rename_to = Some(name.to_os_string());
        self.remember_job(&req);
        self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
    }

    /// Send the active pane's directory to the shell. The one place the two halves of
    /// the program actually talk to each other, and the reason having both is worth it.
    /// Typed through [`App::send_to_shell`], like everything typed into a shell.
    pub(super) fn cd_terminal_here(&mut self) {
        let cwd = self.focused_cwd();
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = tr!(
                "shell.no_terminal_yet_u",
                "no terminal yet \u{2014} {key}",
                key = key
            );
            return;
        };
        let Some(target) = panel.focused_key() else {
            return;
        };
        // A lossy name would be another directory, or none.
        let Some(text) = cwd.to_str() else {
            self.status = tr!(
                "shell.the_directory_s_name_is_not",
                "the directory's name is not UTF-8 \u{2014} not typed"
            )
            .into();
            return;
        };
        let line = match crate::safety::cd_command(&panel.shell_name(), text) {
            Ok(line) => line,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        self.send_to_shell(ShellSend {
            target,
            here: line,
            new_shell: None,
            what: format!("cd {}", cwd.display()),
            dir: cwd,
            paste: false,
        });
    }

    /// Alt+Enter / Ctrl+Enter: the marked names (else the cursor's) typed into the
    /// shell's command line, each quoted for that shell, a space after the last, no
    /// Enter: the user finishes the command. Bare names where the shell is in their
    /// directory, full paths otherwise. Through [`App::send_to_shell`].
    pub(super) fn type_names(&mut self) {
        let paths = self.selection();
        if paths.is_empty() {
            self.status = tr!(
                "shell.nothing_to_type_u_the_cursor",
                "nothing to type \u{2014} the cursor is on .."
            )
            .into();
            return;
        }
        let pane_dir = self.focused_cwd();
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = tr!(
                "shell.no_terminal_yet_u",
                "no terminal yet \u{2014} {key}",
                key = key
            );
            return;
        };
        let Some(target) = panel.focused_key() else {
            return;
        };
        let shell = panel.shell_name();
        // Read only when a key asks: on macOS this runs lsof.
        let typed = bare_names_dir(panel.shell_dir());
        let build = |dir: Option<&Path>| name_words(&shell, &paths, dir);
        let (here, new_shell) = match (build(typed.as_deref()), build(Some(&pane_dir))) {
            (Ok(h), Ok(n)) => (h, n),
            (Err(why), _) | (_, Err(why)) => {
                self.status = why;
                return;
            }
        };
        let what = match paths.len() {
            1 => here.trim_end().to_string(),
            n => format!("{n} names"),
        };
        self.send_to_shell(ShellSend {
            target,
            here,
            new_shell: Some(new_shell),
            dir: pane_dir,
            paste: true,
            what,
        });
    }

    /// Everything xdevcmd types into a shell comes through here (owner, 2026-11-09), and
    /// one table decides ([`crate::pane::term::decide`]): a shell surely at its prompt
    /// gets it at once (when `[terminal] send_when_idle`, the default); a program in
    /// the terminal, or ssh, gets nothing, and the question offers a new shell instead;
    /// a shell whose state cannot be known (Windows without prompt marks), or an idle
    /// one with the setting off, asks with Send to this shell as well.
    pub(super) fn send_to_shell(&mut self, s: ShellSend) {
        let Some(state) = self
            .panel_ref()
            .and_then(|p| p.shell(s.target))
            .map(crate::pane::TermPane::state)
        else {
            self.status = tr!(
                "shell.that_shell_is_gone_u_nothing",
                "that shell is gone \u{2014} nothing typed"
            )
            .into();
            return;
        };
        // A line that ends in Enter (Alt+C's cd, a user-menu command) never goes after
        // text typed and not run: that text would run with it as more arguments (#1).
        // Names typed for the user to finish do: adding to the line is what they are for.
        if self.onto_typed_text(&s) && matches!(state, ShellState::Idle | ShellState::Unknown) {
            return self.cd_beside_typed_text(s);
        }
        let long = s.here.len() > crate::pane::term::LONG_LINE;
        let decision = crate::pane::term::decide_for(
            &state,
            self.settings.config.terminal.send_when_idle,
            s.here.len(),
        );
        if decision == Decision::Send {
            return self.send_here(s);
        }
        let new_shell = if s.new_shell.is_some() {
            tr!(
                "shell.open_a_new_shell_in_and",
                "Open a new shell in {v} and type it there?",
                v = tidy(&s.dir)
            )
        } else {
            tr!(
                "shell.open_a_new_shell_in_instead",
                "Open a new shell in {v} instead?",
                v = tidy(&s.dir)
            )
        };
        let what = tr!(
            "shell.to_type",
            "To type: {v}",
            v = xdevcmd_core::format::elide_middle(&s.what, 70)
        );
        let (title, mut body) = match &state {
            ShellState::Busy(p) => (
                tr!("app_shell.a_program_is_running", "A program is running"),
                vec![tr!("shell.is_running_in_this_terminal_u", "{p} is running in this terminal \u{2014} xdevcmd does not type into a program.", p = p)],
            ),
            ShellState::Remote(p) => (
                tr!("app_shell.the_shell_is_on_another_machine", "The shell is on another machine"),
                vec![tr!("shell.is_running_in_this_terminal_u_2", "{p} is running in this terminal \u{2014} the shell there is on another machine.", p = p)],
            ),
            ShellState::Unknown => (
                tr!("app_shell.type_into_the_shell", "Type into the shell?"),
                vec![
                    tr!("shell.xdevcmd_cannot_tell_whether_a_program", "xdevcmd cannot tell whether a program is running in this shell.").into(),
                    tr!("shell.docs_shell_md_shows_the_prompt", "docs/shell.md shows the prompt snippet that lets it tell.").into(),
                ],
            ),
            ShellState::Idle if !long => (
                tr!("app_shell.type_into_the_shell", "Type into the shell?"),
                vec![
                    tr!("shell.the_shell_is_at_its_prompt", "The shell is at its prompt.").into(),
                    tr!("shell.settings_u_terminal_can_send_without", "Settings \u{2192} Terminal can send without asking.").into(),
                ],
            ),
            ShellState::Idle => (tr!("app_shell.type_into_the_shell", "Type into the shell?"), vec![tr!("shell.the_shell_is_at_its_prompt", "The shell is at its prompt.").into()]),
        };
        if long {
            body.push(tr!(
                "shell.it_is_bytes_u_some_shells",
                "It is {here_len} bytes \u{2014} some shells drop a line over 4 KB without a word.",
                here_len = s.here.len()
            ));
        }
        body.push(String::new());
        body.push(what);
        body.push(new_shell);
        let s = Box::new(s);
        let (yes, also) = match decision {
            Decision::AskNewShell => (
                (
                    tr!("shell.open_a_new_shell", "open a new shell").into_owned(),
                    Cmd::ShellNew(s),
                ),
                None,
            ),
            _ => {
                let here = s.clone();
                (
                    (
                        tr!("shell.open_a_new_shell", "open a new shell").into_owned(),
                        Cmd::ShellNew(s),
                    ),
                    Some((
                        's',
                        tr!("shell.send_to_this_shell", "send to this shell").into_owned(),
                        Cmd::ShellSendHere(here),
                    )),
                )
            }
        };
        self.modal = Some(Modal::Ask(modal::Ask {
            title: title.into(),
            body,
            danger: false,
            yes: Some(yes),
            also,
            buttons: Vec::new(),
            more: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Whether `s` runs a line (ends in Enter) and its shell holds text typed and not run.
    fn onto_typed_text(&self, s: &ShellSend) -> bool {
        s.here.ends_with('\r')
            && self
                .panel_ref()
                .and_then(|p| p.shell(s.target))
                .is_some_and(crate::pane::TermPane::dirty)
    }

    /// A line to run, with text typed in the target shell: done in a new shell in the
    /// directory (`new_shell`'s text typed there, if any), the typed text left where it
    /// is; with no room for one, nothing, and the key that clears the line (#1).
    fn cd_beside_typed_text(&mut self, s: ShellSend) {
        let (n, before, room) = match self.panel_ref() {
            Some(p) => (
                p.tab_number(s.target).unwrap_or(1),
                p.tab_count(),
                p.shell_count() < self.settings.config.terminal.max_terminals,
            ),
            None => return,
        };
        if !room {
            let r = crate::refusal::Refusal::Busy;
            r.log();
            self.status = if cfg!(unix) {
                tr!(
                    "shell.ctrl_c_clears_shell_s_line",
                    "Ctrl+C clears shell {n}'s line \u{2014} all shell tabs in use",
                    n = n
                )
            } else {
                tr!(
                    "shell.close_a_shell_tab_u_all",
                    "close a shell tab \u{2014} all in use"
                )
                .into()
            };
            return;
        }
        let dir = xdevcmd_core::format::elide_middle(&tidy(&s.dir), 20);
        let mut s = s;
        if s.new_shell.is_some() {
            // Said again when its text is typed there, which replaces this status.
            s.what = tr!(
                "shell.u_shell_keeps_its_typed_text",
                "{what} \u{2014} shell {n} keeps its typed text",
                what = s.what,
                n = n
            );
        }
        self.open_shell_for(s);
        let after = self.panel_ref().map_or(before, TermPanel::tab_count);
        if after > before {
            crate::refusal::Refusal::DirtyLine.log();
            self.status = tr!(
                "shell.new_shell_in_u_shell_keeps",
                "new shell {after} in {dir} \u{2014} shell {n} keeps its typed text",
                after = after,
                dir = dir,
                n = n
            );
        }
    }

    /// Type `s` into its shell now, if that shell is still there.
    pub(super) fn send_here(&mut self, s: ShellSend) {
        // Again here: text may have been typed while a question was open (S, "send to
        // this shell", comes here too).
        if self.onto_typed_text(&s) {
            return self.cd_beside_typed_text(s);
        }
        let Some(t) = self.panel_ref().and_then(|p| p.shell(s.target)) else {
            self.status = tr!(
                "shell.that_shell_is_gone_u_nothing",
                "that shell is gone \u{2014} nothing typed"
            )
            .into();
            return;
        };
        t.type_text(&s.here, s.paste);
        self.status = format!("shell: {}", s.what);
    }

    /// "Open a new shell": a new terminal tab started in the pane's directory. For a
    /// `cd` that is the whole job; names are typed into it once its prompt is up
    /// (`pending_type`, in `background`), or not at all if it does not come up in time.
    pub(super) fn open_shell_for(&mut self, s: ShellSend) {
        if self.shell_pane.term_id.is_none() {
            self.status = tr!(
                "shell.that_shell_is_gone_u_nothing",
                "that shell is gone \u{2014} nothing typed"
            )
            .into();
            return;
        }
        if !self.shell_pane.term_shown {
            let full = self.hits.screen;
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        let key = match self.panel().map(|p| p.add_tab(&s.dir)) {
            Some(Ok(k)) => k,
            Some(Err(e)) => {
                self.status = tr!("shell.no_shell", "no shell: {e}", e = e);
                return;
            }
            None => return,
        };
        match s.new_shell {
            None => {
                self.status = tr!(
                    "shell.a_new_shell_in",
                    "a new shell in {v}",
                    v = tidy(&s.dir)
                )
            }
            Some(text) => {
                // One wait at a time: an earlier one still waiting is dropped, and said.
                let dropped = match self.pending_type.take() {
                    Some(p) => tr!(
                        "shell.for_the_earlier_new_shell_not",
                        " ({what} for the earlier new shell: not typed)",
                        what = p.what
                    ),
                    None => String::new(),
                };
                self.status = tr!(
                    "shell.a_new_shell_in_u_typing",
                    "a new shell in {v} \u{2014} typing at its prompt{dropped}",
                    v = tidy(&s.dir),
                    dropped = dropped
                );
                self.pending_type = Some(PendingType {
                    key,
                    text,
                    paste: s.paste,
                    what: s.what,
                    since: Instant::now(),
                });
            }
        }
    }

    /// Names waiting for a new shell's prompt: typed once it is up, dropped (and said)
    /// if the shell ends or the prompt does not come in time. True when it said so.
    pub(super) fn pump_pending_type(&mut self) -> bool {
        let Some(p) = self.pending_type.take() else {
            return false;
        };
        let ready = self
            .panel_ref()
            .and_then(|panel| panel.shell(p.key))
            .map(|t| (t.prompt_ready(), t));
        match ready {
            None => {
                self.status = tr!(
                    "shell.the_new_shell_ended_before_its",
                    "the new shell ended before its prompt \u{2014} nothing typed"
                )
                .into();
            }
            Some((true, t)) => {
                t.type_text(&p.text, p.paste);
                self.status = tr!("shell.new_shell", "new shell: {what}", what = p.what);
            }
            Some((false, _)) if p.since.elapsed() >= PROMPT_WAIT => {
                self.status = "a new shell is open, but its prompt did not show in time \u{2014} \
                               nothing typed (docs/shell.md)"
                    .into();
            }
            Some((false, _)) => {
                self.pending_type = Some(p);
                return false;
            }
        }
        true
    }

    /// `[terminal] follow`: when the focused shell's directory changes while it is at
    /// its prompt, the file pane used last goes there. Edge-triggered: the pane going
    /// somewhere else (Backspace) changes nothing until the shell's next `cd`. Never
    /// the other way. Waits while a dialog or a quick search is open; while the pane
    /// shows an archive or find results the change is let pass, not kept for later.
    /// True when it moved a pane.
    pub(super) fn follow_shell(&mut self) -> bool {
        if !self.settings.config.terminal.follow {
            self.shell_pane.follow_seen = None;
            return false;
        }
        if self.modal.is_some() || self.hits.drag.is_some() || self.search.is_some() {
            return false;
        }
        let seen = self.shell_pane.follow_seen.take();
        let Some(panel) = self.panel_ref() else {
            return false;
        };
        let Some(key) = panel.focused_key() else {
            return false;
        };
        let Some(shell) = panel.shell(key) else {
            return false;
        };
        let (mut seq, last) = match seen {
            Some((k, seq, last)) if k == key => (seq, Some(last)),
            // Another shell (a tab switched, a new one): only learn where it is.
            _ => (0, None),
        };
        let now = shell.idle_dir(&mut seq);
        let Some(dir) = now else {
            self.shell_pane.follow_seen = Some((key, seq, last.flatten()));
            return false;
        };
        self.shell_pane.follow_seen = Some((key, seq, Some(dir.clone())));
        match last {
            Some(Some(was)) if was != dir => {}
            _ => return false,
        }
        if !dir.is_dir() {
            return false;
        }
        let canonical = xdevcmd_core::paths::canonical(&dir).unwrap_or_else(|_| dir.clone());
        if !xdevcmd_core::policy::get().allows(&canonical) {
            return false;
        }
        let Some(f) = self
            .last_files_id()
            .and_then(|id| self.panes.get_mut(&id))
            .and_then(Pane::as_files_mut)
        else {
            return false;
        };
        if f.archive.is_some() || f.results.is_some() || f.cwd == canonical {
            return false;
        }
        f.set_cwd(&canonical);
        self.status = tr!(
            "shell.following_the_shell",
            "following the shell: {v}",
            v = tidy(&canonical)
        );
        true
    }

    /// A press inside a shell, reported to the program there when it asked for the
    /// mouse (vim, htop, Claude Code); its drag and release then follow it there.
    pub(super) fn press_into_shell(&mut self, x: u16, y: u16, button: u32, mods: KeyModifiers) {
        use crate::pane::term::MouseEv;
        let ev = MouseEv::Press(button | mouse_mods(mods));
        self.shell_pane.mouse_to_shell = self.panel_ref().and_then(|p| p.mouse_at(x, y, ev));
    }

    /// The terminal panel, to read from.
    pub(super) fn panel_ref(&self) -> Option<&TermPanel> {
        match self.panes.get(&self.shell_pane.term_id?) {
            Some(Pane::Term(p)) => Some(p),
            _ => None,
        }
    }

    /// The file pane used last: the focused one, else the one focused before, else
    /// the first (focus starting in the files and going straight to the shell leaves
    /// no file pane in `mru`).
    pub(super) fn last_files_id(&self) -> Option<PaneId> {
        let files = self.file_panes();
        std::iter::once(self.focus)
            .chain(self.mru.iter().copied())
            .find(|id| files.contains(id))
            .or_else(|| files.first().copied())
    }

    /// Alt+G: the file pane goes to the shell's directory, the reverse of Alt+C. It only
    /// reads, nothing is typed; still refused while a program runs in the shell (its
    /// directory is the shell's, not the program's, and may not be what is meant) or
    /// when that program is ssh (the shell that matters is on another machine).
    pub(super) fn go_shell_dir(&mut self) {
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = tr!(
                "shell.no_terminal_yet_u",
                "no terminal yet \u{2014} {key}",
                key = key
            );
            return;
        };
        match panel.shell_state() {
            ShellState::Busy(p) => {
                self.status = tr!(
                    "shell.is_running_in_the_shell_u",
                    "{p} is running in the shell \u{2014} its directory is read at the prompt",
                    p = p
                );
                return;
            }
            ShellState::Remote(p) => {
                self.status = tr!(
                    "shell.is_running_u_the_shell_s",
                    "{p} is running \u{2014} the shell's directory there is on another machine",
                    p = p
                );
                return;
            }
            // Reading types nothing, so a shell whose state is unknown may be read.
            ShellState::Idle | ShellState::Unknown => {}
        }
        // A reported directory (OSC 7, 9;9) may be used here: it only moves a pane.
        let Some(dir) = panel.shell_dir().map(|d| d.path) else {
            self.status = tr!(
                "shell.the_shell_s_directory_is_not",
                "the shell's directory is not known \u{2014} see docs/shell.md"
            )
            .into();
            return;
        };
        if !dir.is_dir() {
            self.status = tr!(
                "shell.the_shell_s_directory_is_not_2",
                "the shell's directory {v} is not there any more",
                v = tidy(&dir)
            );
            return;
        }
        let canonical = xdevcmd_core::paths::canonical(&dir).unwrap_or_else(|_| dir.clone());
        if !xdevcmd_core::policy::get().allows(&canonical) {
            self.status = tr!(
                "shell.the_shell_s_directory_is_outside",
                "the shell's directory is outside the root xdevcmd is confined to"
            )
            .into();
            return;
        }
        let Some(f) = self
            .last_files_id()
            .and_then(|id| self.panes.get_mut(&id))
            .and_then(Pane::as_files_mut)
        else {
            return;
        };
        if f.cwd == canonical && f.archive.is_none() && f.results.is_none() {
            self.status = tr!(
                "shell.already_in_the_shell_s_directory",
                "already in the shell's directory"
            )
            .into();
            return;
        }
        f.set_cwd(&canonical);
        self.status = tr!(
            "shell.the_shell_s_directory",
            "the shell's directory: {v}",
            v = tidy(&canonical)
        );
    }

    /// A program in a terminal tab of its own, started by argv (no shell), kept after
    /// it ends so its output can be read.
    pub(super) fn run_in_terminal(&mut self, argv: Vec<std::ffi::OsString>, cwd: PathBuf) {
        let name = argv
            .first()
            .map(|p| Self::describe(&[PathBuf::from(p)]))
            .unwrap_or_default();
        if self.refused(Action::NewTerminal).is_some() || self.settings.mode.read_only() {
            self.status = self.settings.mode.turned_off("running programs", "");
            return;
        }
        if !self.interactive {
            self.status = tr!(
                "shell.would_run_in_a_terminal_tab",
                "would run {name} in a terminal tab",
                name = name
            );
            return;
        }
        let full = self.hits.screen;
        if self.shell_pane.term_id.is_none() || !self.shell_pane.term_shown {
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        match self.panel().map(|p| p.add_program_tab(&cwd, &argv)) {
            Some(Ok(())) => self.status = tr!("shell.running", "running {name}", name = name),
            Some(Err(e)) => {
                self.status = tr!(
                    "shell.cannot_run",
                    "cannot run {name}: {e}",
                    name = name,
                    e = e
                )
            }
            None => self.status = tr!("shell.no_terminal_panel", "no terminal panel").into(),
        }
        if let Some(id) = self.shell_pane.term_id {
            self.focus = id;
        }
    }
}

/// The Log to file prompt's label: which mode, and how to switch.
pub(super) fn log_label(raw: bool) -> String {
    if raw {
        tr!(
            "shell.raw_bytes_as_received_tab_text",
            "raw bytes, as received (Tab: text), appended to:"
        )
        .into()
    } else {
        tr!(
            "shell.text_escapes_removed_tab_raw_appended",
            "text, escapes removed (Tab: raw), appended to:"
        )
        .into()
    }
}
