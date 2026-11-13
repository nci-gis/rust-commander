//! The terminal panel: shells and tabs, history and Save as, typing names and commands into a shell, following it.

use super::*;

impl App {
    pub(super) fn toggle_term(&mut self, area: Rect) {
        match (self.term_id, self.term_shown) {
            (Some(id), true) => {
                // Remember the height it was dragged to, so showing it again restores it.
                if let Node::Split { pct, .. } = &self.tree {
                    self.term_height = 100 - *pct;
                }
                self.tree.remove_leaf(id);
                self.term_shown = false;
                if self.focus == id {
                    self.focus = self.file_panes().first().copied().unwrap_or(0);
                }
            }
            (Some(id), false) => {
                let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                self.tree = Node::split(Axis::Rows, 100 - self.term_height, old, Node::Leaf(id));
                self.term_shown = true;
                self.focus = id;
            }
            (None, _) => {
                let cwd = self
                    .next_shell_cwd
                    .take()
                    .unwrap_or_else(|| self.focused_cwd());
                let pct = self.term_height;
                let rows = (area.height * pct / 100).saturating_sub(2).max(1);
                let cols = area.width.saturating_sub(2).max(1);
                let shell = self.settings.config.terminal.shell.clone();
                let scrollback = self.settings.config.terminal.scrollback;
                match TermPanel::new(&cwd, rows, cols, &shell, scrollback) {
                    Ok(t) => {
                        let id = self.alloc_id();
                        self.panes.insert(id, Pane::Term(Box::new(t)));
                        let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                        self.tree =
                            Node::split(Axis::Rows, 100 - self.term_height, old, Node::Leaf(id));
                        self.term_id = Some(id);
                        self.term_shown = true;
                    }
                    Err(e) => self.status = format!("no shell: {e}"),
                }
            }
        }
    }

    /// The panel has no shells left: take it out of the layout entirely, so the next
    /// F12 starts a fresh one rather than showing an empty frame.
    pub(super) fn remove_terminal_panel(&mut self) {
        if let Some(id) = self.term_id.take() {
            if self.term_shown {
                self.tree.remove_leaf(id);
            }
            self.panes.remove(&id);
            self.term_shown = false;
            if self.focus == id {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
            }
        }
    }

    pub(super) fn panel(&mut self) -> Option<&mut TermPanel> {
        match self.panes.get_mut(&self.term_id?) {
            Some(Pane::Term(p)) => Some(p),
            _ => None,
        }
    }

    /// A new terminal tab, showing the panel if it was hidden and creating it if there
    /// was none — in which case the panel's first tab is the new terminal.
    pub(super) fn new_terminal(&mut self) {
        let full = self.screen;
        match (self.term_id, self.term_shown) {
            (None, _) => self.toggle_term(full),
            (Some(_), shown) => {
                if !shown {
                    self.toggle_term(full);
                }
                if !self.terminal_room() {
                    // Still show and focus the panel: the request was "a terminal",
                    // and the ones already running are right there.
                    if let Some(id) = self.term_id {
                        self.focus = id;
                    }
                    return;
                }
                let cwd = self.focused_cwd();
                if let Some(Err(e)) = self.panel().map(|p| p.add_tab(&cwd)) {
                    self.status = format!("no shell: {e}");
                }
            }
        }
        if let Some(id) = self.term_id {
            self.focus = id;
        }
    }

    /// Whether one more shell fits under `[terminal] max_terminals`; says why if not.
    pub(super) fn terminal_room(&mut self) -> bool {
        let max = self.settings.config.terminal.max_terminals;
        let running = self.panel().map_or(0, |p| p.shell_count());
        if running >= max {
            self.status =
                format!("at most {max} terminals \u{2014} [terminal] max_terminals in config.toml");
            return false;
        }
        true
    }

    pub(super) fn cycle_terminal(&mut self, delta: isize) {
        let Some(id) = self.term_id else {
            self.status = "no terminal yet \u{2014} F12".into();
            return;
        };
        if !self.term_shown {
            let full = self.screen;
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
                "shell exited".into()
            } else {
                format!("{gone} shells exited")
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
        match self.term_id {
            Some(id) if self.term_shown && self.focus != id => self.focus = id,
            Some(id) if self.term_shown => {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
                let _ = id;
            }
            _ => {
                // toggle_term leaves focus alone because startup uses it too, but F12
                // means "take me to the shell": a new one that left you typing into
                // the file panes would be a trap.
                self.toggle_term(area);
                if let Some(id) = self.term_id {
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
        if self.term_id.is_none() {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        }
        let Some(h) = self.panel().and_then(|p| p.history(tab)) else {
            self.status = "no such terminal tab".into();
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
            "a full-screen program is running \u{2014} its screen only \u{2014} Esc back".into()
        } else {
            format!(
                "{lines} line{}, in memory only \u{2014} Esc back",
                if lines == 1 { "" } else { "s" }
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
            return refuse(self, "that path is outside the root xdevcmd is confined to");
        }
        match act {
            viewer::LinkAct::Go => {
                let Some(id) = self.last_files_id() else {
                    return refuse(self, "no file pane to go there");
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
                        after: " \u{2014} Esc back to the history".into(),
                    });
                    if let Some(Modal::View(h)) = self.modal.take() {
                        self.return_to = Some(h);
                    }
                    self.modal = Some(Modal::View(Box::new(viewer)));
                }
                Err(e) => refuse(self, &format!("cannot view {}: {e}", tidy(&canonical))),
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
            self.status = "that terminal tab is gone".into();
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
            format!("{history}, F2")
        };
        let writes = self.refused(Action::CopyAs).is_none();
        // The tab's identity, not its position: shells are reaped every turn while the
        // menu is open, and an earlier tab ending would move this one's position.
        let Some(id) = self.panel().and_then(|p| p.tab_id(i)) else {
            return;
        };
        let items = vec![
            Item::cmd("View history", history, Cmd::TabHistory(id), true),
            Item::cmd(
                "Save history as\u{2026}",
                save,
                Cmd::TabSaveHistory(id),
                writes,
            ),
            match self.panel().and_then(|p| p.tab_log(i)) {
                Some(_) => Item::cmd("Stop logging", "\u{25cf}", Cmd::TabStopLog(id), true),
                None => Item::cmd("Log to file\u{2026}", "", Cmd::TabLog(id), writes),
            },
            Item::cmd(
                "List from the last command\u{2026}",
                "",
                Cmd::TabList(id),
                true,
            ),
            Item::sep(),
            Item::cmd("Close tab", "\u{00d7}", Cmd::TabClose(id), true),
        ];
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Terminal {}", i + 1)),
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
            self.status = format!("this tab is already logged to {}", l.path().display());
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
            title: format!("Log terminal {} to file", i + 1),
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
        self.status = self
            .panel()
            .and_then(|p| p.stop_log(i))
            .unwrap_or_else(|| "this tab is not being logged".into());
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
            "the history, readable by you only"
        } else {
            "a copy of this file"
        };
        self.save_from = Some(v);
        self.modal = Some(Modal::Input(Input {
            title: "Save as".into(),
            label: format!("{what}, to:"),
            edit: LineEdit::new(&dir.join(name).display().to_string()),
            kind: InputKind::SaveAs,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Where the text typed in a Save as…-style prompt points, checked as Save as…
    /// checks it: inside the root (no `..`), no control character, not a directory, in
    /// a folder that exists. `Ok(None)` when nothing was typed.
    pub(super) fn typed_destination(&self, i: &Input) -> Result<Option<PathBuf>, String> {
        let typed = i.edit.text();
        let typed = typed.trim();
        if typed.is_empty() {
            return Ok(None);
        }
        let mut dest = expand_tilde(typed);
        if dest.is_relative() {
            dest = i.dir.join(dest);
        }
        // Under --root, as F7 and Alt+F5: no `..`, and the directory it goes in must be
        // inside, links resolved.
        let climbs = Path::new(typed)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        let inside = dest
            .parent()
            .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
        if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
            return Err("outside the root xdevcmd is confined to".into());
        }
        // The prefill comes from a file's name or the shell's directory, which someone
        // else may have chosen: a control character in it is refused, as everywhere a
        // name could later be typed into a shell or pasted.
        if let Some(c) = crate::safety::control_char(typed) {
            return Err(format!(
                "the name contains a control character (U+{:04X}) \u{2014} not saved",
                c as u32
            ));
        }
        if dest.is_dir() {
            return Err("that is a directory \u{2014} add a file name".into());
        }
        // Said plainly here, before a job is started that would end in the system's
        // own words for it.
        if !dest.parent().is_some_and(Path::is_dir) {
            return Err("that folder does not exist".into());
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
                Ok(()) => self.status = format!("saved to {}", dest.display()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // The same question a copy asks, before anything is touched.
                    let on_disk = std::fs::metadata(&dest).map_or(0, |m| m.len());
                    self.modal = Some(Modal::Ask(modal::Ask {
                        title: "Already exists".into(),
                        body: vec![
                            dest.display().to_string(),
                            String::new(),
                            format!("on disk   {}", human_size(on_disk)),
                            format!("incoming  {}", human_size(len)),
                        ],
                        danger: true,
                        yes: Some(("Replace".into(), Cmd::SaveReplacing(dest))),
                        buttons: Vec::new(),
                        also: None,
                        more: None,
                        footnote: None,
                        pictures: Vec::new(),
                    }));
                }
                Err(e) => refuse(self, i, format!("cannot save: {e}")),
            }
            return;
        }
        let from = v.doc.path.clone();
        let (Some(dir), Some(name)) = (dest.parent(), dest.file_name()) else {
            return refuse(self, i, "not a file name".into());
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
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        };
        let Some(target) = panel.focused_key() else {
            return;
        };
        // A lossy name would be another directory, or none.
        let Some(text) = cwd.to_str() else {
            self.status = "the directory's name is not UTF-8 \u{2014} not typed".into();
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
            self.status = "nothing to type \u{2014} the cursor is on ..".into();
            return;
        }
        let pane_dir = self.focused_cwd();
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
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
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        };
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
            format!("Open a new shell in {} and type it there?", tidy(&s.dir))
        } else {
            format!("Open a new shell in {} instead?", tidy(&s.dir))
        };
        let what = format!(
            "To type: {}",
            xdevcmd_core::format::elide_middle(&s.what, 70)
        );
        let (title, mut body) = match &state {
            ShellState::Busy(p) => (
                "A program is running",
                vec![format!(
                    "{p} is running in this terminal \u{2014} xdevcmd does not type into a program."
                )],
            ),
            ShellState::Remote(p) => (
                "The shell is on another machine",
                vec![format!(
                    "{p} is running in this terminal \u{2014} the shell there is on another machine."
                )],
            ),
            ShellState::Unknown => (
                "Type into the shell?",
                vec![
                    "xdevcmd cannot tell whether a program is running in this shell.".into(),
                    "docs/shell.md shows the prompt snippet that lets it tell.".into(),
                ],
            ),
            ShellState::Idle if !long => (
                "Type into the shell?",
                vec![
                    "The shell is at its prompt.".into(),
                    "Settings \u{2192} Terminal can send without asking.".into(),
                ],
            ),
            ShellState::Idle => ("Type into the shell?", vec!["The shell is at its prompt.".into()]),
        };
        if long {
            body.push(format!(
                "It is {} bytes \u{2014} some shells drop a line over 4 KB without a word.",
                s.here.len()
            ));
        }
        body.push(String::new());
        body.push(what);
        body.push(new_shell);
        let s = Box::new(s);
        let (yes, also) = match decision {
            Decision::AskNewShell => (("open a new shell".to_string(), Cmd::ShellNew(s)), None),
            _ => {
                let here = s.clone();
                (
                    ("open a new shell".to_string(), Cmd::ShellNew(s)),
                    Some((
                        's',
                        "send to this shell".to_string(),
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

    /// Type `s` into its shell now, if that shell is still there.
    pub(super) fn send_here(&mut self, s: ShellSend) {
        let Some(t) = self.panel_ref().and_then(|p| p.shell(s.target)) else {
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        };
        t.type_text(&s.here, s.paste);
        self.status = format!("shell: {}", s.what);
    }

    /// "Open a new shell": a new terminal tab started in the pane's directory. For a
    /// `cd` that is the whole job; names are typed into it once its prompt is up
    /// (`pending_type`, in `background`), or not at all if it does not come up in time.
    pub(super) fn open_shell_for(&mut self, s: ShellSend) {
        if self.term_id.is_none() {
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        }
        if !self.term_shown {
            let full = self.screen;
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        let key = match self.panel().map(|p| p.add_tab(&s.dir)) {
            Some(Ok(k)) => k,
            Some(Err(e)) => {
                self.status = format!("no shell: {e}");
                return;
            }
            None => return,
        };
        match s.new_shell {
            None => self.status = format!("a new shell in {}", tidy(&s.dir)),
            Some(text) => {
                // One wait at a time: an earlier one still waiting is dropped, and said.
                let dropped = match self.pending_type.take() {
                    Some(p) => format!(" ({} for the earlier new shell: not typed)", p.what),
                    None => String::new(),
                };
                self.status = format!(
                    "a new shell in {} \u{2014} typing at its prompt{dropped}",
                    tidy(&s.dir)
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
                self.status = "the new shell ended before its prompt \u{2014} nothing typed".into();
            }
            Some((true, t)) => {
                t.type_text(&p.text, p.paste);
                self.status = format!("new shell: {}", p.what);
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
            self.follow_seen = None;
            return false;
        }
        if self.modal.is_some() || self.drag.is_some() || self.search.is_some() {
            return false;
        }
        let seen = self.follow_seen.take();
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
            self.follow_seen = Some((key, seq, last.flatten()));
            return false;
        };
        self.follow_seen = Some((key, seq, Some(dir.clone())));
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
        self.status = format!("following the shell: {}", tidy(&canonical));
        true
    }

    /// A press inside a shell, reported to the program there when it asked for the
    /// mouse (vim, htop, Claude Code); its drag and release then follow it there.
    pub(super) fn press_into_shell(&mut self, x: u16, y: u16, button: u32, mods: KeyModifiers) {
        use crate::pane::term::MouseEv;
        let ev = MouseEv::Press(button | mouse_mods(mods));
        self.mouse_to_shell = self.panel_ref().and_then(|p| p.mouse_at(x, y, ev));
    }

    /// The terminal panel, to read from.
    pub(super) fn panel_ref(&self) -> Option<&TermPanel> {
        match self.panes.get(&self.term_id?) {
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
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        };
        match panel.shell_state() {
            ShellState::Busy(p) => {
                self.status = format!(
                    "{p} is running in the shell \u{2014} its directory is read at the prompt"
                );
                return;
            }
            ShellState::Remote(p) => {
                self.status = format!(
                    "{p} is running \u{2014} the shell's directory there is on another machine"
                );
                return;
            }
            // Reading types nothing, so a shell whose state is unknown may be read.
            ShellState::Idle | ShellState::Unknown => {}
        }
        // A reported directory (OSC 7, 9;9) may be used here: it only moves a pane.
        let Some(dir) = panel.shell_dir().map(|d| d.path) else {
            self.status = "the shell's directory is not known \u{2014} see docs/shell.md".into();
            return;
        };
        if !dir.is_dir() {
            self.status = format!("the shell's directory {} is not there any more", tidy(&dir));
            return;
        }
        let canonical = xdevcmd_core::paths::canonical(&dir).unwrap_or_else(|_| dir.clone());
        if !xdevcmd_core::policy::get().allows(&canonical) {
            self.status = "the shell's directory is outside the root xdevcmd is confined to".into();
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
            self.status = "already in the shell's directory".into();
            return;
        }
        f.set_cwd(&canonical);
        self.status = format!("the shell's directory: {}", tidy(&canonical));
    }

    /// A program in a terminal tab of its own, started by argv (no shell), kept after
    /// it ends so its output can be read.
    pub(super) fn run_in_terminal(&mut self, argv: Vec<std::ffi::OsString>, cwd: PathBuf) {
        let name = argv
            .first()
            .map(|p| Self::describe(&[PathBuf::from(p)]))
            .unwrap_or_default();
        if self.refused(Action::NewTerminal).is_some() || self.settings.mode.read_only() {
            self.status = format!(
                "{} mode \u{2014} running programs is turned off",
                self.settings.mode.label()
            );
            return;
        }
        if !self.interactive {
            self.status = format!("would run {name} in a terminal tab");
            return;
        }
        let full = self.screen;
        if self.term_id.is_none() || !self.term_shown {
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        match self.panel().map(|p| p.add_program_tab(&cwd, &argv)) {
            Some(Ok(())) => self.status = format!("running {name}"),
            Some(Err(e)) => self.status = format!("cannot run {name}: {e}"),
            None => self.status = "no terminal panel".into(),
        }
        if let Some(id) = self.term_id {
            self.focus = id;
        }
    }
}

/// The Log to file prompt's label: which mode, and how to switch.
pub(super) fn log_label(raw: bool) -> String {
    if raw {
        "raw bytes, as received (Tab: text), appended to:".into()
    } else {
        "text, escapes removed (Tab: raw), appended to:".into()
    }
}
