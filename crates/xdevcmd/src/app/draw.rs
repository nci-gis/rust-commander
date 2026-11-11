//! Drawing: the menu bar, the frame, the status line and the key footer.

use super::*;

impl App {
    /// What the drop-down for menu `i` holds, built from the keymap and the mode in
    /// effect, so its hints are the keys you have and what is off is greyed out.
    pub(super) fn bar_items(&self, i: usize) -> Vec<Item> {
        use Action as A;
        let file: &[Option<(&str, A)>] = &[
            Some(("View", A::View)),
            Some(("Edit", A::Edit)),
            Some(("Open with system app", A::OpenWith)),
            Some(("Open with\u{2026}", A::OpenWithMenu)),
            None,
            Some(("Copy", A::Copy)),
            Some(("Move", A::Move)),
            Some(("Copy as\u{2026}", A::CopyAs)),
            Some(("Rename", A::Rename)),
            Some(("Rename many\u{2026}", A::BulkRename)),
            Some(("Link in other pane", A::Symlink)),
            Some(("Permissions\u{2026}", A::Permissions)),
            Some(("Checksum", A::Checksum)),
            None,
            Some(("New file\u{2026}", A::NewFile)),
            Some(("New directory\u{2026}", A::MkDir)),
            Some(("Pack\u{2026}", A::Pack)),
            Some(("Unpack", A::Unpack)),
            None,
            Some(("Delete", A::Delete)),
            Some(("Delete permanently", A::DeletePermanent)),
            None,
            Some(("Find files\u{2026}", A::FindFiles)),
            Some(("Your commands\u{2026}", A::UserMenu)),
            None,
            Some(("Quit", A::Quit)),
        ];
        // Settings… is put in just above Quit, below.
        let view: &[Option<(&str, A)>] = &[
            Some(("Quick search", A::QuickSearch)),
            Some(("Filter\u{2026}", A::Filter)),
            Some(("Go to path\u{2026}", A::GoToPath)),
            Some(("Back", A::Back)),
            Some(("Forward", A::Forward)),
            Some(("History", A::History)),
            Some(("Hotlist", A::Hotlist)),
            None,
            Some(("Hidden files", A::ToggleHidden)),
            Some(("Sort by name", A::SortName)),
            Some(("Sort by extension", A::SortExt)),
            Some(("Sort by size", A::SortSize)),
            Some(("Sort by date", A::SortModified)),
            Some(("Directory sizes", A::CalcSizes)),
            Some(("Refresh", A::Refresh)),
            None,
            Some(("Quick view", A::QuickView)),
            Some(("Zoom", A::Zoom)),
            Some(("Split right", A::SplitRight)),
            Some(("Split down", A::SplitDown)),
            Some(("Close pane", A::ClosePane)),
            Some(("Swap panes", A::SwapPanes)),
            Some(("Same directory there", A::MirrorPane)),
            Some(("Compare with other pane", A::CompareDirs)),
            Some(("Synchronise\u{2026}", A::SyncDirs)),
            None,
            Some(("Terminal", A::FocusTerminal)),
            Some(("Terminal history", A::TerminalHistory)),
            Some(("New terminal", A::NewTerminal)),
            Some(("Show or hide terminal", A::ToggleTerminal)),
        ];
        let help: &[Option<(&str, A)>] = &[
            Some(("Keys and settings", A::Help)),
            Some(("Usage", A::Usage)),
        ];
        let list = match i {
            0 => file,
            1 => view,
            _ => help,
        };
        let mut items: Vec<Item> = list
            .iter()
            .map(|e| match e {
                None => Item::sep(),
                Some((label, a)) => Item::act(
                    *label,
                    self.footer_key(*a).unwrap_or_default(),
                    *a,
                    self.refused(*a).is_none(),
                ),
            })
            .collect();
        if i == 0 {
            // Above Quit, which is last.
            let at = items.len().saturating_sub(1);
            items.insert(at, Item::cmd("Settings\u{2026}", "", Cmd::Settings, true));
        }
        if i == 1 {
            items.push(Item::sep());
            items.push(Item::cmd("Theme\u{2026}", "", Cmd::ThemeMenu, true));
        }
        if i == 2 {
            items.push(Item::sep());
            items.push(Item::cmd("About xdevcmd", "", Cmd::About, true));
        }
        items
    }

    pub(super) fn open_bar(&mut self, i: usize) {
        let x = self
            .bar_hits
            .iter()
            .find(|(j, _)| *j == i)
            .map(|(_, r)| r.x)
            .unwrap_or(0);
        // Below the bar when it is on screen; at the top otherwise (F9 with it off).
        let y = u16::from(!self.bar_hits.is_empty());
        self.modal = Some(Modal::Menu(Menu::dropdown(i, self.bar_items(i), (x, y))));
    }

    pub(super) fn draw_bar(&mut self, frame: &mut Frame, area: Rect) {
        self.bar_hits.clear();
        if area.height == 0 {
            return;
        }
        let th = theme::t();
        let open = match &self.modal {
            Some(Modal::Menu(m)) => m.bar,
            _ => None,
        };
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(th.bg_bar));
        let mut x = area.x + 1;
        for (i, title) in BAR.iter().enumerate() {
            let text = format!(" {title} ");
            let w = text.chars().count() as u16;
            if x + w > area.x + area.width {
                break;
            }
            let style = if open == Some(i) {
                Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(th.fg)
            };
            buf.set_line(x, area.y, &Line::from(Span::styled(text, style)), w);
            self.bar_hits.push((i, Rect::new(x, area.y, w, 1)));
            x += w + 1;
        }
        // Which xdevcmd this is, at the right: a shell started from the terminal panel
        // names the xdevcmd it came from by this number (main.rs, `own_panel`). Dim, and
        // only where the menus leave room.
        let me = format!("xdevcmd {} ", std::process::id());
        let w = me.chars().count() as u16;
        let end = area.x + area.width;
        if x + 2 + w <= end {
            buf.set_line(
                end - w,
                area.y,
                &Line::from(Span::styled(me, Style::new().fg(th.fg_dim))),
                w,
            );
        }
    }

    pub(super) fn holding(&self) -> crate::usage::Holding {
        let mut h = crate::usage::Holding {
            jobs: self.jobs.len(),
            ..Default::default()
        };
        for p in self.panes.values() {
            match p {
                Pane::Files(t) => {
                    h.panes += 1;
                    h.tabs += t.len();
                    h.listed += t.tabs().iter().map(|f| f.entries.len()).sum::<usize>();
                }
                Pane::Term(t) => h.shells += t.shell_count(),
            }
        }
        h
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.draw_frame(frame);
        // The cells under the picture the terminal shows, as this frame has them.
        self.under_graphic.clear();
        if let Some(r) = self.graphic_shown.as_ref().map(|g| g.rect) {
            let buf = frame.buffer_mut();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    if let Some(c) = buf.cell(ratatui::layout::Position::new(x, y)) {
                        self.under_graphic.push((x, y, c.clone()));
                    }
                }
            }
        }
    }

    pub(super) fn draw_frame(&mut self, frame: &mut Frame) {
        self.graphic_want = None;
        self.settling = false;
        let area = frame.area();
        self.screen = area;
        self.note_focus();
        for p in self.panes.values_mut() {
            if let Some(n) = p.as_files_mut().and_then(|f| f.notice.take()) {
                self.status = n;
            }
        }
        frame
            .buffer_mut()
            .set_style(area, Style::new().bg(theme::t().bg).fg(theme::t().fg));

        let bar_h = u16::from(self.settings.config.general.menu_bar && area.height >= 8);
        let [bar, main, status, footer] = Layout::vertical([
            Constraint::Length(bar_h),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        self.draw_bar(frame, bar);

        self.main = main;
        if self
            .zoom
            .is_some_and(|z| z != self.focus || !self.tree.contains(z))
        {
            self.zoom = None;
            // Do not leave "zoomed, press Alt+Z" on screen once it is not.
            if self.status.starts_with("zoomed") {
                self.status.clear();
            }
        }
        let mut rects = Vec::new();
        match self.zoom {
            Some(z) => rects.push((z, main)),
            None => self.tree.assign(main, &mut rects),
        }
        self.rects = rects.clone();

        // Quick view ends when its pane takes the focus or goes away.
        if self
            .quick_view
            .as_ref()
            .is_some_and(|(q, _)| *q == self.focus || !self.tree.contains(*q))
        {
            self.quick_view = None;
        }
        let previewing = self.quick_view.as_ref().map(|(q, _)| *q);
        // The rows where a pane starts with a tab bar: terminal panels always have one,
        // file panes when they hold more than one tab. A file pane starting on the same
        // row shows its bar too, or its list would sit a row higher than its neighbour's.
        let barred: Vec<u16> = rects
            .iter()
            .filter(|(id, r)| {
                previewing != Some(*id)
                    && match self.panes.get(id) {
                        Some(Pane::Files(f)) => f.wants_bar(*r),
                        Some(Pane::Term(_)) => true,
                        None => false,
                    }
            })
            .map(|(_, r)| r.y)
            .collect();
        for (id, r) in rects {
            let active = id == self.focus;
            if previewing == Some(id) {
                self.draw_quick_view(frame, r);
                continue;
            }
            match self.panes.get_mut(&id) {
                Some(Pane::Files(f)) => {
                    let target = self
                        .drag
                        .as_ref()
                        .is_some_and(|d| d.mode == DragMode::Files && d.hover == Some(id));
                    f.render(frame, r, active, target, barred.contains(&r.y))
                }
                Some(Pane::Term(p)) => p.render(frame, r, active),
                None => {}
            }
        }

        if let Some(s) = &self.search {
            let at = self
                .rects
                .iter()
                .find(|(id, _)| *id == s.pane)
                .map(|(_, r)| *r);
            let f = self.panes.get(&s.pane).and_then(Pane::as_files);
            if let (Some(r), Some(f)) = (at, f) {
                let count = if s.filter {
                    f.entries.iter().filter(|e| !e.is_parent()).count()
                } else {
                    search::matches(&f.entries, &s.text()).len()
                };
                search::render(frame, r, s, count);
            }
        }

        self.draw_status(frame, status);
        self.draw_footer(frame, footer);

        if let Some(m) = self.modal.as_mut() {
            modal::render(frame, area, m);
        }
        if matches!(self.modal, Some(Modal::Usage)) {
            let holding = self.holding();
            crate::usage::render(frame, area, &self.usage, &holding);
        }
    }

    pub(super) fn draw_status(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let left = match self.panes.get(&self.focus) {
            Some(Pane::Files(tabs)) => {
                let f = tabs.active();
                let tab = if tabs.len() > 1 {
                    format!("  \u{b7}  tab {}/{}", tabs.active_index() + 1, tabs.len())
                } else {
                    String::new()
                };
                let sort = match f.sort.key {
                    SortKey::Name => "name",
                    SortKey::Ext => "ext",
                    SortKey::Size => "size",
                    SortKey::Modified => "date",
                };
                format!(
                    " {}  ·  {} items  ·  sort {}{}{}{}",
                    f.summary(),
                    f.entries.len(),
                    sort,
                    if f.sort.reverse { " ▲" } else { " ▼" },
                    if f.show_hidden { "  ·  hidden" } else { "" },
                    tab
                )
            }
            Some(Pane::Term(p)) => format!(
                " terminal {}/{}{}  \u{b7}  {} back to files  \u{b7}  {} history",
                p.active_tab() + 1,
                p.tab_count(),
                if p.split_count() > 1 {
                    format!("  \u{b7}  split {}/{}", p.focused_split(), p.split_count())
                } else {
                    String::new()
                },
                self.footer_key(Action::FocusTerminal).unwrap_or_default(),
                self.footer_key(Action::TerminalHistory).unwrap_or_default()
            ),
            None => String::new(),
        };
        let left = if self.zoom.is_some() {
            format!("{left}  \u{b7}  zoomed")
        } else {
            left
        };
        let left = if self.jobs.is_empty() {
            left
        } else {
            let (done, total) = self.jobs.iter().fold((0u64, 0u64), |(d, t), j| {
                (d + j.bytes_done, t + j.total_bytes)
            });
            let pct = (done * 100).checked_div(total).unwrap_or(0);
            format!(
                "{left}  \u{b7}  \u{27f3} {} job{} {pct}%",
                self.jobs.len(),
                if self.jobs.len() == 1 { "" } else { "s" }
            )
        };
        // The mode is never something you have to remember: it sits at the left of
        // the status line in the error colour for as long as it is on.
        let badge = match self.settings.mode {
            Mode::Normal => "",
            Mode::ReadOnly => " READ-ONLY ",
            Mode::Restricted => " RESTRICTED ",
        };
        // An open two-key group is shown the same way, in the accent colour, so a key
        // waiting for its second is never a surprise.
        let group = self.group_badge();
        let badges = badge.chars().count() + group.as_ref().map_or(0, |g| g.chars().count() + 1);
        let left = format!("{}{left}", " ".repeat(badges));
        let right = format!("{} ", self.status);

        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        let avail = area.width as usize;
        // The right half (messages) wins space, but a gap of two is always kept between
        // them: with both halves long they otherwise ran together, "…historytoo small".
        let rw = right.chars().count().min(avail.saturating_sub(3));
        let lw = avail.saturating_sub(rw + 2);
        let gap = avail.saturating_sub(lw + rw).min(2);
        buf.set_line(
            area.x,
            area.y,
            &Line::from(Span::styled(
                elide_middle(&left, lw),
                Style::new().fg(theme::t().fg),
            )),
            lw as u16,
        );
        buf.set_line(
            area.x + (lw + gap) as u16,
            area.y,
            &Line::from(Span::styled(
                elide_middle(&right, rw),
                Style::new().fg(theme::t().fg_dim),
            )),
            rw as u16,
        );
        if !badge.is_empty() {
            let th = theme::t();
            frame.buffer_mut().set_line(
                area.x,
                area.y,
                &Line::from(Span::styled(
                    badge,
                    Style::new()
                        .fg(th.bg)
                        .bg(th.err)
                        .add_modifier(Modifier::BOLD),
                )),
                area.width,
            );
        }
        if let Some(g) = group {
            let th = theme::t();
            let x = area.x + badge.chars().count() as u16;
            frame.buffer_mut().set_line(
                x,
                area.y,
                &Line::from(Span::styled(
                    g,
                    Style::new()
                        .fg(th.bg)
                        .bg(th.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                area.width.saturating_sub(x - area.x),
            );
        }
    }

    pub(super) fn draw_footer(&mut self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        // Read from the keymap, not written down: after `copy = ["F2"]` a bar that still
        // says "5 Copy" is lying, and this is the line people look at most.
        // In F-key order, as in every orthodox manager. On a narrow screen the ones that
        // do not fit are left off whole, from the end, rather than cut mid-word.
        const KEYS: &[(Action, &str)] = &[
            (Action::Help, "Help"),
            (Action::Rename, "Ren"),
            (Action::View, "View"),
            (Action::Edit, "Edit"),
            (Action::Copy, "Copy"),
            (Action::Move, "Move"),
            (Action::MkDir, "MkDir"),
            (Action::Delete, "Del"),
            (Action::MenuBar, "Menu"),
            (Action::Quit, "Quit"),
        ];
        let mut spans = Vec::new();
        let mut hits = Vec::new();
        let mut cx = area.x;
        for (action, label) in KEYS {
            if self.refused(*action).is_some() {
                continue;
            }
            let Some(key) = self.footer_key(*action) else {
                continue;
            };
            let k = key
                .strip_prefix('F')
                .filter(|d| d.parse::<u8>().is_ok())
                .unwrap_or(&key);
            let w = (k.chars().count() + label.chars().count() + 3) as u16;
            if cx + w > area.x + area.width {
                break;
            }
            hits.push((*action, Rect::new(cx, area.y, w, 1)));
            cx += w;
            spans.push(Span::styled(
                format!(" {k}"),
                Style::new()
                    .fg(theme::t().accent)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(theme::t().fg_dim),
            ));
        }
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);
        self.footer_hits = hits;
    }
}
