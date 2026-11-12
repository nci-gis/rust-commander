//! Open dialogs' keys and actions, quick search, go to, filters, history and the hotlist.

use super::*;

impl App {
    pub(super) fn on_modal_key(&mut self, k: KeyEvent) {
        let mut act = ModalAct::None;

        // F1 over the viewer, which takes every other key: F1 there was nothing before.
        if matches!(self.modal, Some(Modal::View(_)))
            && self.settings.keymap.action_for(k) == Some(Action::Help)
        {
            return self.open_commands(commands::Size::Small, false);
        }

        match self.modal.as_mut() {
            Some(Modal::Menu(menu)) => match menu.on_key(k) {
                MenuOutcome::Stay => {}
                MenuOutcome::Close => act = ModalAct::Close,
                MenuOutcome::Run(c) => act = ModalAct::Run(c),
                MenuOutcome::Side(d) => act = ModalAct::Bar(d),
            },
            Some(Modal::Usage) => {
                if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Confirm(c)) => match k.code {
                KeyCode::Tab => c.next_destination(),
                // What cannot be undone is never one Enter away: Enter is what a hand
                // presses out of habit. Y, or a click on the button, says it.
                KeyCode::Enter if c.permanent => {
                    self.status = "press Y to delete permanently, Esc to cancel".into();
                }
                KeyCode::Char('u' | 'U') if c.existing_line.is_some() => c.next_existing(),
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => act = ModalAct::Start,
                KeyCode::Esc | KeyCode::Char('n' | 'N') => act = ModalAct::Close,
                _ => {}
            },
            Some(Modal::Input(i)) => match k.code {
                KeyCode::Esc => act = ModalAct::Close,
                KeyCode::Enter => act = ModalAct::Apply,
                KeyCode::Tab if i.kind == InputKind::GoTo => complete_path(i),
                _ => {
                    if i.edit.on_key(k) {
                        i.error = None;
                    }
                }
            },
            Some(Modal::View(v)) => match v.on_key(k) {
                viewer::Action::Close => act = ModalAct::Close,
                viewer::Action::Copy(text) => {
                    // Not Run(…): that closes the viewer first, and copying should not.
                    self.copy_text(text);
                    let said = self.status.clone();
                    if let Some(Modal::View(v)) = self.modal.as_mut() {
                        v.note(&said);
                    }
                    return;
                }
                viewer::Action::SaveAs => return self.begin_save_as(),
                viewer::Action::Link(a, l) => act = ModalAct::Link(a, l),
                viewer::Action::MakeList(req, add) => return self.make_list(req, add),
                viewer::Action::Stay => {}
            },
            Some(Modal::Config(c)) => {
                if matches!(c.on_key(k), crate::config_view::Act::Close) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Commands(c)) => {
                let could_hint = c.option_hint;
                act = match c.on_key(k) {
                    commands::Outcome::Stay => ModalAct::None,
                    commands::Outcome::Close => ModalAct::Close,
                    commands::Outcome::Run(row) => ModalAct::Palette(row),
                };
                if could_hint && !c.option_hint {
                    self.option_hinted = true;
                }
            }
            Some(Modal::Settings(d)) => {
                let o = d.on_key(k);
                act = self.settings_outcome(o);
            }
            Some(Modal::Rename(d)) => match d.on_key(k) {
                rename_dialog::Outcome::Stay => {}
                rename_dialog::Outcome::Close => act = ModalAct::Close,
                rename_dialog::Outcome::Rename(m) => act = ModalAct::Run(Cmd::BatchRename(m)),
                rename_dialog::Outcome::Editor(t) => act = ModalAct::BatchEditor(t),
            },
            Some(Modal::Find(d)) => match d.on_key(k) {
                find_dialog::Outcome::Stay => {}
                find_dialog::Outcome::Close => act = ModalAct::Close,
                find_dialog::Outcome::Start => act = ModalAct::StartFind,
            },
            Some(Modal::Ask(a)) => match k.code {
                KeyCode::Char(c)
                    if a.also
                        .as_ref()
                        .is_some_and(|(key, _, _)| key.eq_ignore_ascii_case(&c)) =>
                {
                    act = match a.also.take() {
                        Some((_, _, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    }
                }
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    act = match a.yes.take() {
                        Some((_, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    }
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => act = ModalAct::Close,
                _ => {}
            },
            Some(Modal::Job(j)) => {
                if let Some(q) = j.question.take() {
                    match modal::answer_for(&q, k.code) {
                        Some(a) => j.answer(a),
                        None => j.question = Some(q),
                    }
                } else if j.done.is_some() {
                    if matches!(k.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ')) {
                        act = ModalAct::Finish;
                    }
                } else if k.code == KeyCode::Esc {
                    j.handle.cancel();
                } else if matches!(k.code, KeyCode::Char('b' | 'B')) {
                    act = ModalAct::Background;
                }
            }
            None => {}
        }

        self.modal_act(act);
    }

    pub(super) fn modal_act(&mut self, act: ModalAct) {
        self.modal_act_inner(act);
        self.back_to_viewer();
    }

    /// Once Save as…'s dialogs are all closed (asked, answered, or the copy done), the
    /// viewer it was started from comes back, with what happened on its message line.
    pub(super) fn back_to_viewer(&mut self) {
        if self.modal.is_some() {
            return;
        }
        if let Some(mut v) = self.save_from.take() {
            if std::mem::take(&mut self.save_only) {
                return;
            }
            if !self.status.is_empty() {
                v.note(&self.status);
            }
            self.modal = Some(Modal::View(v));
        }
    }

    pub(super) fn modal_act_inner(&mut self, act: ModalAct) {
        match act {
            ModalAct::Link(a, l) => self.history_link(a, l),
            ModalAct::Run(c) => {
                self.modal = None;
                self.perform_cmd(c);
            }
            ModalAct::Close if matches!(self.modal, Some(Modal::Commands(_))) => {
                // F1 gives back what it was opened over (the viewer).
                if let Some(Modal::Commands(mut c)) = self.modal.take() {
                    self.modal = c.under.take().map(|u| *u);
                }
            }
            ModalAct::Palette(row) => self.run_command_row(row),
            ModalAct::Close => {
                // A file viewed from a shell's history: back to that history.
                // So does the menu `P` opened over it, cancelled.
                let viewing = matches!(self.modal, Some(Modal::View(_)))
                    || self.pending_list.take().is_some();
                self.modal = self.return_to.take().filter(|_| viewing).map(Modal::View);
                // Not while Save as… holds the viewer: it still shows that file.
                if self.save_from.is_none() {
                    if let Some(t) = self.view_temp.take() {
                        let _ = std::fs::remove_dir_all(&t);
                    }
                }
            }
            ModalAct::Start => {
                if let Some(Modal::Confirm(c)) = self.modal.take() {
                    let kind = c.request.kind;
                    self.remember_job(&c.request);
                    self.modal = Some(Modal::Job(Job::new(kind, ops::start(c.request))));
                }
            }
            ModalAct::Apply => {
                if let Some(Modal::Input(i)) = self.modal.take() {
                    self.apply_input(i);
                }
            }
            ModalAct::Finish => self.finish_job(),
            ModalAct::Bar(d) => {
                let at = match &self.modal {
                    Some(Modal::Menu(m)) => m.bar.unwrap_or(0),
                    _ => 0,
                };
                let n = crate::app::BAR.len() as isize;
                self.open_bar((at as isize + d).rem_euclid(n) as usize);
            }
            ModalAct::Background => {
                if let Some(Modal::Job(j)) = self.modal.take() {
                    self.jobs.push(j);
                    // The job has its list of files; marks now would only be carried
                    // into the next operation by mistake, as finish_job knows.
                    for p in self.panes.values_mut() {
                        if let Some(f) = p.as_files_mut() {
                            f.marked.clear();
                        }
                    }
                    self.status =
                        "running in the background \u{2014} it comes back if it has a question"
                            .into();
                }
            }
            ModalAct::SaveSettings => self.save_settings(),
            ModalAct::BatchEditor(text) => {
                if let Some(Modal::Rename(d)) = self.modal.take() {
                    self.begin_batch(d.files, &text);
                }
            }
            ModalAct::StartFind => {
                if let Some(Modal::Find(d)) = self.modal.take() {
                    let sort = self.settings.sort;
                    let tab = FilePane::results(d.query(), d.what(), sort);
                    if let Some(t) = self.focused_tabs() {
                        t.push(tab);
                        let last = t.len() - 1;
                        t.select(last);
                    }
                    self.status = "searching \u{2014} Esc stops it, Ctrl+W closes the tab".into();
                }
            }
            ModalAct::None => {}
        }
    }

    pub(super) fn begin_search(&mut self, filter: bool) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let before = f.filter.clone();
        self.search = Some(Search::new(self.focus, filter, before));
        self.status = if filter {
            "type to filter \u{b7} Enter keeps it \u{b7} Esc puts it back".into()
        } else {
            "type to jump \u{b7} \u{2191}\u{2193} next match \u{b7} Enter or Esc to stop".into()
        };
    }

    /// A key while the search line is open. False means the line did not want it: the
    /// search has ended and the key goes on to do what it normally does.
    pub(super) fn on_search_key(&mut self, k: KeyEvent) -> bool {
        let Some(s) = self.search.as_mut() else {
            return false;
        };
        let id = s.pane;
        let f = self.panes.get_mut(&id).and_then(Pane::as_files_mut);
        let Some(f) = f else {
            self.search = None;
            return false;
        };
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc => {
                if s.filter {
                    f.set_filter(s.before.take());
                }
                self.search = None;
                self.status.clear();
                return true;
            }
            KeyCode::Enter => {
                self.search = None;
                self.status.clear();
                return true;
            }
            KeyCode::Up | KeyCode::Down => {
                let forward = k.code == KeyCode::Down;
                if s.filter {
                    f.move_cursor(if forward { 1 } else { -1 });
                } else if let Some(i) = search::step(&f.entries, &s.text(), f.cursor, forward) {
                    f.go_to(i);
                }
                return true;
            }
            // Ctrl+A and Ctrl+U are the line's own (select all, delete to the start);
            // any other chord is a command.
            _ if alt || (ctrl && !matches!(k.code, KeyCode::Char('a' | 'u'))) => {}
            _ => {
                if s.edit.on_key(k) {
                    let text = s.text();
                    if s.filter {
                        f.set_filter(Some(text.clone()));
                        s.found = text.is_empty() || f.entries.iter().any(|e| !e.is_parent());
                    } else {
                        match search::first(&f.entries, &text) {
                            Some(i) => {
                                f.go_to(i);
                                s.found = true;
                            }
                            None => s.found = text.is_empty(),
                        }
                    }
                    return true;
                }
            }
        }
        self.search = None;
        self.status.clear();
        false
    }

    pub(super) fn begin_goto(&mut self) {
        let dir = self.focused_cwd();
        let mut start = dir.display().to_string();
        if !start.ends_with(std::path::MAIN_SEPARATOR) {
            start.push(std::path::MAIN_SEPARATOR);
        }
        self.modal = Some(Modal::Input(Input {
            title: "Go to".into(),
            label: "a directory, or a file to land on \u{b7} ~ is home \u{b7} Tab completes".into(),
            edit: LineEdit::new(&start),
            kind: InputKind::GoTo,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn begin_pattern(&mut self, mark: bool) {
        if self.focused_files().is_none() {
            return;
        }
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: if mark { "Mark" } else { "Unmark" }.into(),
            label: "names matching \u{b7} *.jpg *.png \u{b7} a trailing / for directories".into(),
            // Selected, so typing replaces it and Enter alone means everything.
            edit: LineEdit::for_rename("*", true),
            kind: InputKind::Pattern { mark },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Where a menu that belongs to the focused pane opens: just inside its corner.
    pub(super) fn pane_anchor(&self) -> (u16, u16) {
        self.rects
            .iter()
            .find(|(id, _)| *id == self.focus)
            .map(|(_, r)| (r.x + 2, r.y + 1))
            .unwrap_or((2, 1))
    }

    pub(super) fn open_history(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let list = f.history();
        let kept = crate::lists::all();
        if list.is_empty() && kept.is_empty() {
            self.status = "no history in this tab yet".into();
            return;
        }
        // The lists first (the active one at the top), then the directories.
        let here = f.list_id();
        let mut items: Vec<Item> = kept
            .iter()
            .filter(|l| Some(l.id) != here)
            .map(|l| {
                let label = format!(
                    "list \u{b7} {} \u{b7} {}",
                    xdevcmd_core::format::elide_middle(&l.source, 50),
                    l.rows.len()
                );
                (label, Cmd::ShowList(l.id))
            })
            .chain(
                list.iter()
                    .take(20)
                    .map(|p| (tidy(p), Cmd::GoTo(p.clone()))),
            )
            .enumerate()
            .map(|(i, (label, cmd))| Item::cmd(label, digit(i), cmd, true))
            .collect();
        if items.is_empty() {
            self.status = "no history in this tab yet".into();
            return;
        }
        let lists = kept.iter().filter(|l| Some(l.id) != here).count();
        if lists > 0 && lists < items.len() {
            items.insert(lists, Item::sep());
        }
        self.modal = Some(Modal::Menu(Menu::new(
            Some(
                if lists > 0 {
                    "History and lists"
                } else {
                    "History"
                }
                .into(),
            ),
            items,
            self.pane_anchor(),
        )));
    }

    pub(super) fn open_hotlist(&mut self) {
        if self.focused_files().is_none() {
            return;
        }
        let cwd = self.focused_cwd();
        let mut items = Vec::new();
        let mut n = 0;
        for (name, path) in &self.settings.config.bookmarks {
            let p = expand_tilde(path);
            items.push(Item::cmd(
                format!("{name}  \u{b7}  {}", tidy(&p)),
                digit(n),
                Cmd::GoTo(p),
                true,
            ));
            n += 1;
        }
        if !items.is_empty() && !self.bookmarks.is_empty() {
            items.push(Item::sep());
        }
        for p in &self.bookmarks {
            items.push(Item::cmd(tidy(p), digit(n), Cmd::GoTo(p.clone()), true));
            n += 1;
        }
        if !items.is_empty() {
            items.push(Item::sep());
        }
        let writable = !self.settings.mode.read_only();
        if self.bookmarks.contains(&cwd) {
            items.push(Item::cmd(
                format!("Remove {}", tidy(&cwd)),
                "",
                Cmd::RemoveBookmark(cwd),
                writable,
            ));
        } else {
            items.push(Item::cmd(
                format!("Bookmark {}", tidy(&cwd)),
                "",
                Cmd::AddBookmark(cwd),
                writable,
            ));
        }
        items.push(Item::cancel("Cancel", "Esc"));
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Hotlist".into()),
            items,
            self.pane_anchor(),
        )));
    }

    pub(super) fn save_bookmarks(&mut self, done: String) {
        self.status = match bookmarks::save(&self.bookmarks) {
            Ok(()) => done,
            Err(e) => format!("bookmarks not saved: {e}"),
        };
    }
}
