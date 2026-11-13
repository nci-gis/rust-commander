//! The mouse: clicks, drags and drops, context menus, clicks on dialogs, and text selection.

use super::*;

impl App {
    pub(super) fn on_mouse(&mut self, m: MouseEvent) {
        // The mouse closes an open key group: arrows typed after a click are arrows.
        if self.group.take().is_some() {
            self.status.clear();
        }
        if self.modal.is_some() {
            return self.on_modal_mouse(m);
        }
        // A click anywhere ends a search, keeping whatever it found.
        if matches!(m.kind, MouseEventKind::Down(_)) {
            self.search = None;
        }
        let (x, y) = (m.column, m.row);
        let p = Position::new(x, y);

        // A press the program in a shell took: the rest of that gesture is its too.
        if let Some((key, r)) = self.mouse_to_shell {
            use crate::pane::term::MouseEv;
            let mods = mouse_mods(m.modifiers);
            let ev = match m.kind {
                MouseEventKind::Drag(b) => Some(MouseEv::Drag(button_number(b) | mods)),
                MouseEventKind::Up(b) => {
                    self.mouse_to_shell = None;
                    Some(MouseEv::Release(button_number(b) | mods))
                }
                MouseEventKind::Down(_) => {
                    self.mouse_to_shell = None;
                    None
                }
                _ => None,
            };
            if let Some(ev) = ev {
                if let Some(panel) = self.panel_ref() {
                    panel.mouse_to(key, r, x, y, ev);
                }
                return;
            }
        }

        match m.kind {
            MouseEventKind::Drag(MouseButton::Left) => return self.on_drag(x, y),
            MouseEventKind::Up(MouseButton::Left) => return self.on_release(x, y),
            _ => {}
        }

        if let MouseEventKind::Down(MouseButton::Left) = m.kind {
            if let Some(i) = self
                .bar_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(i, _)| *i)
            {
                return self.open_bar(i);
            }
            // The key bar is a row of buttons, as in every orthodox manager.
            if let Some(a) = self
                .footer_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(a, _)| *a)
            {
                return self.perform(a);
            }
            // A scrollbar sits on a pane's right border, which is also half of the
            // boundary beside it; where there is a scrollbar, it wins. The boundary can
            // still be dragged by the other pane's border, or above and below the bar.
            if let Some(id) = self.pane_at(x, y) {
                let grab = self
                    .panes
                    .get(&id)
                    .and_then(Pane::as_files)
                    .and_then(|f| f.bar_at(x, y));
                if let Some(grab) = grab {
                    self.focus = id;
                    if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                        f.drag_bar(y, grab);
                    }
                    self.scrolling = Some((id, grab));
                    return;
                }
            }
            // A boundary is grabbed before either pane it separates: it sits on their
            // borders, which are otherwise just part of each pane. The exception is a
            // button on the terminal's tab bar, which shares a row with that strip.
            let on_tab_button = self.panes.values().any(|p| match p {
                Pane::Term(t) => self.term_shown && t.bar_button_at(x, y),
                Pane::Files(f) => f.bar_button_at(x, y),
            });
            // Nor a directory's name in a pane's title: a pane split off below (Alt+D)
            // has its title on the boundary's row, and a click there means that directory
            // (0.2.1 took it for a drag). The rest of the row still moves the boundary.
            let shown: Vec<PaneId> = self.rects.iter().map(|(id, _)| *id).collect();
            let on_crumb = self.panes.iter().any(|(id, p)| {
                shown.contains(id) && p.as_files().is_some_and(|f| f.crumb_at(x, y).is_some())
            });
            let mut dividers = Vec::new();
            // Zoomed, the layout's borders are not on screen, so none can be grabbed.
            if !on_tab_button && !on_crumb && self.zoom.is_none() {
                self.tree.dividers(self.main, &mut dividers);
            }
            // A double-click on a boundary shares it evenly between the two panes on
            // either side, and only those two; a single press starts a drag.
            if let Some(d) = dividers.into_iter().find(|d| d.hit.contains(p)) {
                if self.register_click(x, y) {
                    self.tree.even(d.id);
                    self.status = "shared evenly between the two panes".into();
                } else {
                    self.resizing = Some((d, false));
                }
                return;
            }
            let inner = self
                .term_shown
                .then(|| self.panel().map(|p| p.dividers()))
                .flatten()
                .unwrap_or_default();
            if let Some(d) = inner.into_iter().find(|d| d.hit.contains(p)) {
                if self.register_click(x, y) {
                    if let Some(panel) = self.panel() {
                        panel.even(d.id);
                    }
                    self.status = "shared evenly between the two shells".into();
                } else {
                    self.resizing = Some((d, true));
                }
                return;
            }
        }

        let Some(id) = self.pane_at(x, y) else { return };
        let ctrl = m.modifiers.contains(KeyModifiers::CONTROL);
        let shift = m.modifiers.contains(KeyModifiers::SHIFT);

        match m.kind {
            MouseEventKind::Down(MouseButton::Left) if Some(id) == self.term_id => {
                self.focus = id;
                let click = self.panel().map(|p| p.on_click(x, y));
                match click {
                    Some(TermClick::NewTab) => self.new_terminal(),
                    Some(TermClick::CloseTab(i)) => {
                        // Closing has a side effect, so it stays out of a match guard.
                        let nothing_left = self.panel().is_some_and(|p| p.close_tab(i));
                        if nothing_left {
                            self.remove_terminal_panel();
                        }
                    }
                    // Inside a shell: the program's, when it asked for the mouse.
                    Some(TermClick::Selected) => self.press_into_shell(x, y, 0, m.modifiers),
                    _ => {}
                }
            }
            // A right-click on a terminal tab: its menu. The tab bar is xdevcmd's, so no
            // guessing whose click it is; inside a shell a right-click is the program's
            // when it asked for the mouse, and otherwise left alone.
            MouseEventKind::Down(b @ (MouseButton::Right | MouseButton::Middle))
                if Some(id) == self.term_id =>
            {
                self.focus = id;
                if let Some(i) = self.panel().and_then(|p| p.tab_at(x, y)) {
                    if b == MouseButton::Right {
                        self.open_tab_menu(i, x, y);
                    }
                } else {
                    self.press_into_shell(x, y, button_number(b), m.modifiers);
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if Some(id) == self.term_id => {
                let up = m.kind == MouseEventKind::ScrollUp;
                if let Some(p) = self.panel() {
                    p.wheel_at(x, y, up);
                }
            }
            MouseEventKind::Down(MouseButton::Left)
                if self
                    .panes
                    .get(&id)
                    .and_then(Pane::as_tabs)
                    .is_some_and(|t| t.bar_button_at(x, y)) =>
            {
                self.focus = id;
                let Some(tabs) = self.focused_tabs() else {
                    return;
                };
                match tabs.on_click(x, y) {
                    TabClick::New => tabs.duplicate(),
                    TabClick::Close(i) => {
                        tabs.close(i);
                    }
                    TabClick::Selected | TabClick::Nothing => {}
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = id;
                let double = self.register_click(x, y);
                let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) else {
                    return;
                };
                // A directory named in the title's path: go there, as Alt+G would.
                if let Some(dir) = f.crumb_at(x, y) {
                    f.set_cwd(&dir);
                    return;
                }
                if let Some(key) = f.header_at(x, y) {
                    f.set_sort(key);
                    return;
                }
                let Some(i) = f.row_at(x, y) else { return };
                if ctrl {
                    f.go_to(i);
                    f.toggle_mark();
                    return;
                }
                if shift {
                    // Most terminals keep Shift+click for their own text selection, so
                    // this often never arrives. When it does, it does the obvious thing.
                    let from = f.cursor;
                    f.mark_range(from, i);
                    f.go_to(i);
                    return;
                }
                f.go_to(i);
                if double {
                    // What Enter does: into a directory, or open the file.
                    if f.activate().is_some() {
                        self.enter_file();
                    }
                    return;
                }
                let anchor_was_marked =
                    f.entries.get(i).is_some_and(|e| f.marked.contains(&e.path));
                let marks_before = f.marked.clone();
                self.drag = Some(Drag {
                    source: id,
                    anchor: i,
                    anchor_was_marked,
                    marks_before,
                    mode: DragMode::Pending,
                    hover: None,
                });
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.focus = id;
                let row = self
                    .panes
                    .get_mut(&id)
                    .and_then(Pane::as_files_mut)
                    .map(|f| {
                        let r = f.row_at(x, y);
                        if let Some(i) = r {
                            f.go_to(i);
                        }
                        r
                    });
                if let Some(row) = row {
                    self.open_context_menu(row, x, y);
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                    f.scroll(-3);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                    f.scroll(3);
                }
            }
            _ => {}
        }
    }

    pub(super) fn on_drag(&mut self, x: u16, y: u16) {
        if let Some((id, grab)) = self.scrolling {
            if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                f.drag_bar(y, grab);
            }
            return;
        }
        if let Some((d, in_panel)) = self.resizing {
            let pct = d.pct_at(x, y);
            if in_panel {
                if let Some(p) = self.panel() {
                    p.set_pct(d.id, pct);
                }
            } else {
                self.tree.set_pct(d.id, pct);
            }
            // Both sides, because one number is ambiguous: dragging the terminal's top
            // edge, "45%" read as the terminal's share when it was the panes'.
            self.status = match d.axis {
                Axis::Cols => format!("left {pct}% \u{b7} right {}%", 100 - pct),
                Axis::Rows => format!("top {pct}% \u{b7} bottom {}%", 100 - pct),
            };
            return;
        }
        let Some(mut d) = self.drag.take() else {
            return;
        };
        let over = self.pane_at(x, y);
        let into_other =
            over.filter(|t| *t != d.source && self.panes.get(t).is_some_and(|p| !p.is_term()));

        if let Some(target) = into_other {
            // Crossing into another pane makes it a drag-and-drop. If a range had been
            // drawn from an unmarked row, that was really the start of carrying one
            // file: put the marks back how they were.
            if d.mode == DragMode::Range && !d.anchor_was_marked {
                if let Some(f) = self.panes.get_mut(&d.source).and_then(Pane::as_files_mut) {
                    f.marked = d.marks_before.clone();
                }
            }
            d.mode = DragMode::Files;
            d.hover = Some(target);
            let n = self.drag_payload(&d).len();
            self.status = format!(
                "\u{21e2} {n} item{} \u{2014} release to copy or move",
                if n == 1 { "" } else { "s" }
            );
        } else if over == Some(d.source) && d.mode != DragMode::Files && !d.anchor_was_marked {
            if let Some(f) = self.panes.get_mut(&d.source).and_then(Pane::as_files_mut) {
                let r = f.rows_rect;
                let last = f.entries.len().saturating_sub(1);
                // Dragging past the top or bottom edge scrolls, so a range can be longer
                // than the pane is tall.
                let row = if y < r.y {
                    f.scroll(-1);
                    f.offset
                } else if y >= r.y + r.height {
                    f.scroll(1);
                    (f.offset + r.height as usize).saturating_sub(1)
                } else {
                    f.offset + (y - r.y) as usize
                }
                .min(last);
                f.marked = d.marks_before.clone();
                f.mark_range(d.anchor, row);
                f.go_to(row);
                d.mode = DragMode::Range;
            }
        } else {
            d.hover = None;
        }
        self.drag = Some(d);
    }

    /// What a drag-and-drop carries: the selection if a marked row was grabbed,
    /// otherwise the one file that was.
    pub(super) fn drag_payload(&self, d: &Drag) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&d.source).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if d.anchor_was_marked {
            let mut v: Vec<PathBuf> = f
                .entries
                .iter()
                .filter(|e| f.marked.contains(&e.path))
                .map(|e| e.path.clone())
                .collect();
            v.sort();
            v
        } else {
            f.entries
                .get(d.anchor)
                .filter(|e| !e.is_parent())
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        }
    }

    pub(super) fn on_release(&mut self, x: u16, y: u16) {
        if self.scrolling.take().is_some() {
            return;
        }
        if self.resizing.take().is_some() {
            return;
        }
        let Some(d) = self.drag.take() else { return };
        if d.mode != DragMode::Files {
            return;
        }
        let Some(target) = d.hover else {
            self.status = "drop cancelled".into();
            return;
        };
        let sources = self.drag_payload(&d);
        let Some(dest) = self
            .panes
            .get(&target)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
        else {
            return;
        };
        if sources.is_empty() {
            self.status = "nothing to drop".into();
            return;
        }
        if let Some(why) = self.refused(Action::Copy) {
            self.status = why;
            return;
        }
        let from_archive = self
            .panes
            .get(&d.source)
            .and_then(Pane::as_files)
            .is_some_and(|f| f.archive.is_some());
        let into_virtual = self
            .panes
            .get(&target)
            .and_then(Pane::as_files)
            .is_some_and(|f| f.archive.is_some() || f.results.is_some());
        if from_archive || into_virtual {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status =
                format!("archives and results do not take drops \u{2014} {k} extracts or copies");
            return;
        }
        let src_dir = self
            .panes
            .get(&d.source)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone());
        if src_dir.as_ref() == Some(&dest) {
            self.status = "that pane shows the same directory \u{2014} nothing to copy".into();
            return;
        }
        let what = Self::describe(&sources);
        let name = dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| dest.display().to_string());
        // Copy or move is asked, never guessed from a modifier the terminal may not
        // even pass through. The answer is the confirmation; overwrites still ask.
        let items = vec![
            Item::cmd(
                format!("Copy {what} here"),
                "",
                Cmd::Transfer {
                    kind: ops::Kind::Copy,
                    sources: sources.clone(),
                    dest: dest.clone(),
                },
                true,
            ),
            Item::cmd(
                format!("Move {what} here"),
                "",
                Cmd::Transfer {
                    kind: ops::Kind::Move,
                    sources,
                    dest,
                },
                true,
            ),
            Item::sep(),
            Item::cancel("Cancel", "Esc"),
        ];
        self.status = format!("drop into {name}?");
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Drop into {name}")),
            items,
            (x, y),
        )));
    }

    /// Shift+F10: the right-click menu for the row under the cursor, opened at that
    /// row, beside the name, as a file manager opens it for the selected item.
    pub(super) fn context_menu_at_cursor(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let r = f.rows_rect;
        if r.height == 0 {
            return;
        }
        let shown = f.cursor.saturating_sub(f.offset).min(r.height as usize - 1) as u16;
        let (row, x, y) = (f.cursor, r.x + 2, r.y + shown);
        self.open_context_menu(Some(row), x, y);
        if let Some(Modal::Menu(m)) = self.modal.as_mut() {
            m.row = Some(y);
        }
    }

    pub(super) fn open_context_menu(&mut self, row: Option<usize>, x: u16, y: u16) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let entry = row
            .and_then(|i| f.entries.get(i))
            .filter(|e| !e.is_parent())
            .cloned();
        let any_marks = !f.marked.is_empty();
        let hidden = f.show_hidden;
        let entry_marked = entry.as_ref().is_some_and(|e| f.marked.contains(&e.path));
        let title = match &entry {
            Some(e) => e.name.clone(),
            None => f
                .cwd
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| f.cwd.display().to_string()),
        };
        let sel = self.selection();
        let has_sel = !sel.is_empty();
        let other = self.other_pane_cwd().is_some();
        let n = sel.len();
        // Hints come from the keymap, so the menu teaches the keys you actually have.
        let k = |a| self.footer_key(a).unwrap_or_default();
        let many = if n > 1 {
            format!(" {n} items")
        } else {
            String::new()
        };

        let mut items = Vec::new();
        if let Some(e) = &entry {
            if e.is_dir() {
                items.push(Item::act("Open", k(Action::Open), Action::Open, true));
            } else {
                items.push(Item::act("View", k(Action::View), Action::View, true));
                items.push(Item::act("Edit", k(Action::Edit), Action::Edit, true));
                items.push(Item::act(
                    "Open with system app",
                    k(Action::OpenWith),
                    Action::OpenWith,
                    true,
                ));
                items.push(Item::act(
                    "Open with\u{2026}",
                    k(Action::OpenWithMenu),
                    Action::OpenWithMenu,
                    true,
                ));
                items.push(Item::act(
                    "Checksum (SHA-256)",
                    k(Action::Checksum),
                    Action::Checksum,
                    true,
                ));
            }
            items.push(Item::act(
                "Copy as\u{2026}",
                k(Action::CopyAs),
                Action::CopyAs,
                true,
            ));
            items.push(Item::act(
                "Link in other pane",
                k(Action::Symlink),
                Action::Symlink,
                other,
            ));
            items.push(Item::act(
                "Permissions\u{2026}",
                k(Action::Permissions),
                Action::Permissions,
                true,
            ));
            items.push(Item::act("Rename", k(Action::Rename), Action::Rename, true));
            items.push(Item::sep());
        }
        items.push(Item::act(
            format!("Copy{many} to other pane"),
            k(Action::Copy),
            Action::Copy,
            has_sel && other,
        ));
        items.push(Item::act(
            format!("Move{many} to other pane"),
            k(Action::Move),
            Action::Move,
            has_sel && other,
        ));
        items.push(Item::act(
            format!("Delete{many}"),
            k(Action::Delete),
            Action::Delete,
            has_sel,
        ));
        items.push(Item::act(
            "Delete permanently",
            k(Action::DeletePermanent),
            Action::DeletePermanent,
            has_sel,
        ));
        items.push(Item::sep());
        if entry.is_some() {
            let label = if entry_marked { "Unmark" } else { "Mark" };
            items.push(Item::act(label, k(Action::Mark), Action::Mark, true));
        }
        items.push(Item::act(
            "Mark all",
            k(Action::MarkAll),
            Action::MarkAll,
            true,
        ));
        items.push(Item::act(
            "Clear marks",
            k(Action::UnmarkAll),
            Action::UnmarkAll,
            any_marks,
        ));
        items.push(Item::sep());
        items.push(Item::act(
            "New directory",
            k(Action::MkDir),
            Action::MkDir,
            true,
        ));
        items.push(Item::act(
            "New tab",
            k(Action::NewTab),
            Action::NewTab,
            true,
        ));
        items.push(Item::cmd("Copy path", "", Cmd::CopyPath, true));
        items.push(Item::act(
            "Terminal here",
            k(Action::CdTerminalHere),
            Action::CdTerminalHere,
            self.term_id.is_some(),
        ));
        items.push(Item::act(
            "Type into the shell",
            k(Action::TypeNames),
            Action::TypeNames,
            self.term_id.is_some() && has_sel,
        ));
        items.push(Item::act(
            "Refresh",
            k(Action::Refresh),
            Action::Refresh,
            true,
        ));
        let hide = if hidden {
            "Hide hidden files"
        } else {
            "Show hidden files"
        };
        items.push(Item::act(
            hide,
            k(Action::ToggleHidden),
            Action::ToggleHidden,
            true,
        ));

        let restricted = self.settings.mode == Mode::Restricted;
        for it in &mut items {
            match &it.cmd {
                Some(Cmd::Act(a)) if self.refused(*a).is_some() => it.enabled = false,
                Some(Cmd::CopyPath) if restricted => it.enabled = false,
                _ => {}
            }
        }
        self.modal = Some(Modal::Menu(Menu::new(Some(title), items, (x, y))));
    }

    pub(super) fn on_modal_mouse(&mut self, m: MouseEvent) {
        let p = Position::new(m.column, m.row);
        let left = matches!(m.kind, MouseEventKind::Down(MouseButton::Left));
        let hit = |rects: &[Rect], i: usize| rects.get(i).is_some_and(|r| r.contains(p));
        let mut act = ModalAct::None;
        // A click on a menu-bar title switches menus, whatever menu is open.
        if left && matches!(self.modal, Some(Modal::Menu(_)) | Some(Modal::Usage)) {
            if let Some(i) = self
                .bar_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(i, _)| *i)
            {
                return self.open_bar(i);
            }
        }
        if left && matches!(self.modal, Some(Modal::Usage)) {
            return self.modal_act(ModalAct::Close);
        }

        match self.modal.as_mut() {
            Some(Modal::Menu(menu)) => match menu.on_mouse(m) {
                MenuOutcome::Stay => {}
                MenuOutcome::Close => act = ModalAct::Close,
                MenuOutcome::Run(c) => act = ModalAct::Run(c),
                MenuOutcome::Side(d) => act = ModalAct::Bar(d),
            },
            Some(Modal::Confirm(c)) if left => {
                if hit(&c.buttons, 0) {
                    act = ModalAct::Start;
                } else if hit(&c.buttons, 1) {
                    act = ModalAct::Close;
                } else if hit(&c.buttons, 2) {
                    c.next_destination();
                }
            }
            Some(Modal::Input(i)) if left => {
                if hit(&i.buttons, 0) {
                    act = ModalAct::Apply;
                } else if hit(&i.buttons, 1) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Job(j)) if left => {
                if j.question.is_some() {
                    let answer = j
                        .answer_buttons
                        .iter()
                        .find(|(_, r)| r.contains(p))
                        .map(|(a, _)| *a);
                    if let Some(a) = answer {
                        j.answer(a);
                    }
                } else if hit(&j.buttons, 0) {
                    if j.done.is_some() {
                        act = ModalAct::Finish;
                    } else {
                        j.handle.cancel();
                    }
                } else if hit(&j.buttons, 1) && j.done.is_none() {
                    act = ModalAct::Background;
                }
            }
            Some(Modal::View(v)) => {
                if let viewer::Action::Link(a, l) = v.on_mouse(m) {
                    act = ModalAct::Link(a, l);
                }
            }
            Some(Modal::Rename(d)) if left => {
                act = if hit(&d.buttons, 0) {
                    match d.confirm() {
                        rename_dialog::Outcome::Rename(m) => ModalAct::Run(Cmd::BatchRename(m)),
                        _ => ModalAct::None,
                    }
                } else if hit(&d.buttons, 1) {
                    match d.to_editor() {
                        rename_dialog::Outcome::Editor(t) => ModalAct::BatchEditor(t),
                        _ => ModalAct::None,
                    }
                } else if hit(&d.buttons, 2) {
                    d.next_field(false);
                    ModalAct::None
                } else if hit(&d.buttons, 3) {
                    ModalAct::Close
                } else {
                    ModalAct::None
                };
            }
            Some(Modal::Find(d)) if left => {
                if hit(&d.buttons, 0) {
                    act = ModalAct::StartFind;
                } else if hit(&d.buttons, 1) {
                    d.field = 1 - d.field;
                } else if hit(&d.buttons, 2) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Ask(a)) if left => {
                // [yes] [also] [cancel], or [yes] [cancel].
                let cancel = if a.also.is_some() { 2 } else { 1 };
                if hit(&a.buttons, 0) {
                    act = match a.yes.take() {
                        Some((_, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    };
                } else if cancel == 2 && hit(&a.buttons, 1) {
                    act = match a.also.take() {
                        Some((_, _, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    };
                } else if hit(&a.buttons, cancel) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Config(c)) => c.on_mouse(m),
            Some(Modal::Commands(c)) => {
                act = match c.on_mouse(m) {
                    commands::Outcome::Stay => ModalAct::None,
                    commands::Outcome::Close => ModalAct::Close,
                    commands::Outcome::Run(row) => ModalAct::Palette(row),
                };
            }
            Some(Modal::Settings(d)) => {
                let o = d.on_mouse(m);
                act = self.settings_outcome(o);
            }
            _ => {}
        }
        self.modal_act(act);
    }

    pub(super) fn register_click(&mut self, x: u16, y: u16) -> bool {
        let now = Instant::now();
        let double = matches!(
            self.last_click,
            Some((t, px, py)) if px == x && py == y && now.duration_since(t) < DOUBLE_CLICK
        );
        self.last_click = if double { None } else { Some((now, x, y)) };
        double
    }

    /// What the keys act on: everything marked, or the row under the cursor.
    /// The selection in the order the pane shows it, which is what a counter counts.
    pub(super) fn selection_in_view(&self) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if f.marked.is_empty() {
            return self.selection();
        }
        f.entries
            .iter()
            .filter(|e| f.marked.contains(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }

    pub(super) fn selection(&self) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if f.marked.is_empty() {
            return f
                .current()
                .filter(|e| !e.is_parent())
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default();
        }
        let mut v: Vec<PathBuf> = f
            .entries
            .iter()
            .filter(|e| f.marked.contains(&e.path))
            .map(|e| e.path.clone())
            .collect();
        v.sort();
        v
    }
}
