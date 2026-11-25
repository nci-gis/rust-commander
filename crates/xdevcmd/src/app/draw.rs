//! Drawing: the menu bar, the frame, the status line and the key footer.

use super::*;
use crate::lang::Text;
use xdevcmd_core::format::Cells;

/// The key bar's labels, cut to fit `width` columns: the longest gives a column at a
/// time, down to 3 (two and `…`), when that keeps every key (a longer language at 80
/// columns); otherwise whole, and the caller leaves the last keys off. Each key takes
/// its digits, its label and 3 columns of spacing.
fn fit_labels(items: &[(Action, String, String)], width: usize) -> Vec<String> {
    const MIN: usize = 3;
    let mut budget: Vec<usize> = items.iter().map(|(_, _, l)| l.cells()).collect();
    let total = |b: &[usize]| -> usize {
        items
            .iter()
            .zip(b)
            .map(|((_, k, _), w)| k.cells() + w + 3)
            .sum()
    };
    // Cut only when that keeps every key; else whole labels, the last keys left off
    // (a narrow screen reads better with fewer whole words than ten stubs).
    let least: Vec<usize> = budget.iter().map(|w| (*w).min(MIN)).collect();
    if total(&least) > width {
        return items.iter().map(|(_, _, l)| l.clone()).collect();
    }
    while total(&budget) > width {
        let Some((i, _)) = budget
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > MIN)
            .max_by_key(|(i, w)| (**w, std::cmp::Reverse(*i)))
        else {
            break;
        };
        budget[i] -= 1;
    }
    items
        .iter()
        .zip(budget)
        .map(|((_, _, l), w)| {
            // "Tạo …" reads as a word left out: the space goes before the `…`.
            let cut = xdevcmd_core::format::cut_end(l, w);
            match cut.strip_suffix('\u{2026}') {
                Some(head) if head.ends_with(' ') => format!("{}\u{2026}", head.trim_end()),
                _ => cut,
            }
        })
        .collect()
}

/// The sort's word on the status line, by `SortKey::label`.
const SORT_WORDS: &[Text] = &[
    Text::new("sort.name", "name"),
    Text::new("sort.ext", "ext"),
    Text::new("sort.size", "size"),
    Text::new("sort.date", "date"),
];

impl App {
    /// What the drop-down for menu `i` holds, built from the keymap and the mode in
    /// effect, so its hints are the keys you have and what is off is greyed out.
    pub(super) fn bar_items(&self, i: usize) -> Vec<Item> {
        use Action as A;
        let file: &[Option<(Text, A)>] = &[
            Some((Text::new("menu.view", "View"), A::View)),
            Some((Text::new("menu.edit", "Edit"), A::Edit)),
            Some((
                Text::new("menu.open_with_system_app", "Open with system app"),
                A::OpenWith,
            )),
            Some((
                Text::new("menu.open_with", "Open with\u{2026}"),
                A::OpenWithMenu,
            )),
            None,
            Some((Text::new("menu.copy", "Copy"), A::Copy)),
            Some((Text::new("menu.move", "Move"), A::Move)),
            Some((Text::new("menu.copy_as", "Copy as\u{2026}"), A::CopyAs)),
            Some((Text::new("menu.rename", "Rename"), A::Rename)),
            Some((
                Text::new("menu.rename_many", "Rename many\u{2026}"),
                A::BulkRename,
            )),
            Some((
                Text::new("menu.link_in_other_pane", "Link in other pane"),
                A::Symlink,
            )),
            Some((
                Text::new("menu.permissions", "Permissions\u{2026}"),
                A::Permissions,
            )),
            Some((Text::new("menu.checksum", "Checksum"), A::Checksum)),
            None,
            Some((Text::new("menu.new_file", "New file\u{2026}"), A::NewFile)),
            Some((
                Text::new("menu.new_directory", "New directory\u{2026}"),
                A::MkDir,
            )),
            Some((Text::new("menu.pack", "Pack\u{2026}"), A::Pack)),
            Some((Text::new("menu.unpack", "Unpack"), A::Unpack)),
            None,
            Some((Text::new("menu.delete", "Delete"), A::Delete)),
            Some((
                Text::new("menu.delete_permanently", "Delete permanently"),
                A::DeletePermanent,
            )),
            None,
            Some((
                Text::new("menu.find_files", "Find files\u{2026}"),
                A::FindFiles,
            )),
            Some((
                Text::new("menu.your_commands", "Your commands\u{2026}"),
                A::UserMenu,
            )),
            None,
            Some((Text::new("menu.quit", "Quit"), A::Quit)),
        ];
        // Settings… is put in just above Quit, below.
        let view: &[Option<(Text, A)>] = &[
            Some((
                Text::new("menu.quick_search", "Quick search"),
                A::QuickSearch,
            )),
            Some((Text::new("menu.filter", "Filter\u{2026}"), A::Filter)),
            Some((
                Text::new("menu.go_to_path", "Go to path\u{2026}"),
                A::GoToPath,
            )),
            Some((Text::new("menu.back", "Back"), A::Back)),
            Some((Text::new("menu.forward", "Forward"), A::Forward)),
            Some((
                Text::new("menu.history_and_lists", "History and lists\u{2026}"),
                A::History,
            )),
            Some((Text::new("menu.last_list", "Last list"), A::ShowList)),
            Some((Text::new("menu.hotlist", "Hotlist"), A::Hotlist)),
            None,
            Some((
                Text::new("menu.hidden_files", "Hidden files"),
                A::ToggleHidden,
            )),
            Some((Text::new("menu.sort_by_name", "Sort by name"), A::SortName)),
            Some((
                Text::new("menu.sort_by_extension", "Sort by extension"),
                A::SortExt,
            )),
            Some((Text::new("menu.sort_by_size", "Sort by size"), A::SortSize)),
            Some((
                Text::new("menu.sort_by_date", "Sort by date"),
                A::SortModified,
            )),
            Some((
                Text::new("menu.directory_sizes", "Directory sizes"),
                A::CalcSizes,
            )),
            Some((Text::new("menu.refresh", "Refresh"), A::Refresh)),
            None,
            Some((Text::new("menu.quick_view", "Quick view"), A::QuickView)),
            Some((Text::new("menu.zoom", "Zoom"), A::Zoom)),
            Some((Text::new("menu.split_right", "Split right"), A::SplitRight)),
            Some((Text::new("menu.split_down", "Split down"), A::SplitDown)),
            Some((Text::new("menu.close_pane", "Close pane"), A::ClosePane)),
            Some((Text::new("menu.swap_panes", "Swap panes"), A::SwapPanes)),
            Some((
                Text::new("menu.same_directory_there", "Same directory there"),
                A::MirrorPane,
            )),
            Some((
                Text::new("menu.compare_with_other_pane", "Compare with other pane"),
                A::CompareDirs,
            )),
            Some((
                Text::new("menu.synchronise", "Synchronise\u{2026}"),
                A::SyncDirs,
            )),
            None,
            Some((Text::new("menu.terminal", "Terminal"), A::FocusTerminal)),
            Some((
                Text::new("menu.terminal_history", "Terminal history"),
                A::TerminalHistory,
            )),
            Some((
                Text::new("menu.new_terminal", "New terminal"),
                A::NewTerminal,
            )),
            Some((
                Text::new("menu.show_or_hide_terminal", "Show or hide terminal"),
                A::ToggleTerminal,
            )),
        ];
        // Commands… and Config in use are put in at the top, Documentation and About
        // at the end, below.
        let help: &[Option<(Text, A)>] = &[
            Some((Text::new("menu.usage", "Usage"), A::Usage)),
            Some((Text::new("menu.messages", "Messages"), A::Messages)),
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
                    label.get(),
                    self.footer_key(*a).unwrap_or_default(),
                    *a,
                    self.refused(*a).is_none(),
                ),
            })
            .collect();
        if i == 0 {
            // Above Quit, which is last.
            let at = items.len().saturating_sub(1);
            items.insert(
                at,
                Item::cmd(
                    tr!("menu.settings", "Settings\u{2026}"),
                    "",
                    Cmd::Settings,
                    true,
                ),
            );
        }
        if i == 1 {
            items.push(Item::sep());
            items.push(Item::cmd(
                tr!("menu.theme", "Theme\u{2026}"),
                "",
                Cmd::ThemeMenu,
                true,
            ));
        }
        if i == 2 {
            let f1 = self.footer_key(A::Help).unwrap_or_default();
            items.insert(
                0,
                Item::cmd(
                    tr!("menu.commands", "Commands\u{2026}"),
                    f1,
                    Cmd::Commands,
                    true,
                ),
            );
            items.insert(
                1,
                Item::cmd(
                    tr!("menu.config_in_use", "Config in use"),
                    "",
                    Cmd::ConfigInUse,
                    true,
                ),
            );
            items.push(Item::cmd(
                tr!("menu.documentation", "Documentation"),
                "",
                Cmd::Documentation,
                true,
            ));
            items.push(Item::sep());
            items.push(Item::cmd(
                tr!("menu.about_xdevcmd", "About xdevcmd"),
                "",
                Cmd::About,
                true,
            ));
        }
        items
    }

    pub(super) fn open_bar(&mut self, i: usize) {
        let x = self
            .hits
            .bar_hits
            .iter()
            .find(|(j, _)| *j == i)
            .map(|(_, r)| r.x)
            .unwrap_or(0);
        // Below the bar when it is on screen; at the top otherwise (F9 with it off).
        let y = u16::from(!self.hits.bar_hits.is_empty());
        self.modal = Some(Modal::Menu(Menu::dropdown(i, self.bar_items(i), (x, y))));
    }

    pub(super) fn draw_bar(&mut self, frame: &mut Frame, area: Rect) {
        self.hits.bar_hits.clear();
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
            let text = format!(" {} ", title.get());
            let w = text.cells() as u16;
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
            self.hits.bar_hits.push((i, Rect::new(x, area.y, w, 1)));
            x += w + 1;
        }
        // Which xdevcmd this is, at the right: a shell started from the terminal panel
        // names the xdevcmd it came from by this number (main.rs, `own_panel`). Dim, and
        // only where the menus leave room.
        let me = format!("xdevcmd {} ", std::process::id());
        let w = me.cells() as u16;
        let end = area.x + area.width;
        if x + 2 + w <= end {
            buf.set_line(
                end - w,
                area.y,
                &Line::from(Span::styled(me, Style::new().fg(th.fg_dim))),
                w,
            );
            // Slanted keys: the number as a segment, with a fade leading into it where
            // there is room.
            if crate::decor::get().keys == crate::decor::Keys::Slant {
                let pill = Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD);
                let lead = "\u{2591}\u{2591}\u{2592}\u{2592}\u{2593}\u{2593}\u{25e2} ";
                let lw = if x + 2 + w + 8 <= end { 8 } else { 2 };
                let lead: String = lead.chars().skip(8 - lw as usize).collect();
                let at = end - w - lw;
                let n = lead.chars().count();
                for (i, ch) in lead.chars().enumerate() {
                    let st = if i + 1 == n {
                        pill
                    } else {
                        Style::new().fg(th.accent)
                    };
                    buf.set_string(at + i as u16, area.y, ch.to_string(), st);
                }
                buf.set_style(Rect::new(end - w, area.y, w, 1), pill);
            }
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
        self.pics.under_graphic.clear();
        if let Some(r) = self.pics.graphic_shown.as_ref().map(|g| g.rect) {
            let buf = frame.buffer_mut();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    if let Some(c) = buf.cell(ratatui::layout::Position::new(x, y)) {
                        self.pics.under_graphic.push((x, y, c.clone()));
                    }
                }
            }
        }
    }

    pub(super) fn draw_frame(&mut self, frame: &mut Frame) {
        self.pics.graphic_want = None;
        self.pics.settling = false;
        let area = frame.area();
        self.hits.screen = area;
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

        self.hits.main = main;
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
        self.hits.rects = rects.clone();

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
        // While a copy or move asks where to, the pane it goes to is lit as a drop target
        // is: Tab moves the light with the destination (the owner, 2026-11-25).
        let landing = match &self.modal {
            Some(Modal::Confirm(c)) if !c.choices.is_empty() => {
                c.choices.get(c.choice).and_then(|d| self.transfer_pane(d))
            }
            _ => None,
        };
        for (id, r) in rects {
            let active = id == self.focus;
            if previewing == Some(id) {
                self.draw_quick_view(frame, r);
                continue;
            }
            match self.panes.get_mut(&id) {
                Some(Pane::Files(f)) => {
                    let target = self
                        .hits
                        .drag
                        .as_ref()
                        .is_some_and(|d| d.mode == DragMode::Files && d.hover == Some(id))
                        || landing == Some(id);
                    f.render(frame, r, active, target, barred.contains(&r.y))
                }
                Some(Pane::Term(p)) => p.render(frame, r, active),
                None => {}
            }
        }

        if let Some(s) = &self.search {
            let at = self
                .hits
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

        self.note_status();
        self.draw_walker(frame);
        self.draw_status(frame, status);
        self.draw_footer(frame, footer);

        if let Some(m) = self.modal.as_mut() {
            modal::render(frame, area, m);
        }
        if matches!(self.modal, Some(Modal::Usage)) {
            let holding = self.holding();
            crate::usage::render(frame, area, &self.usage, &holding);
        }

        // The theme's light over the whole frame, but not over what the programs in the
        // terminal panels drew.
        let screens: Vec<Rect> = self
            .hits
            .rects
            .iter()
            .filter_map(|(id, _)| match self.panes.get(id) {
                Some(Pane::Term(t)) => Some(t.screens()),
                _ => None,
            })
            .collect();
        crate::decor::decorate(frame.buffer_mut(), &screens);
    }

    /// Keep the status line's message in Messages (item 46) the first time it shows.
    /// Every message passes the frame that draws it, whoever set it.
    fn note_status(&mut self) {
        if self.status == self.noted {
            return;
        }
        self.noted.clone_from(&self.status);
        if self.status.trim().is_empty() {
            return;
        }
        if self.messages.len() == MESSAGES {
            self.messages.pop_front();
        }
        self.messages
            .push_back((std::time::SystemTime::now(), self.status.clone()));
    }

    /// Messages (Alt+M): the status line's past in the viewer, the newest at the
    /// bottom, where it opens.
    pub(super) fn show_messages(&mut self) {
        let text: String = if self.messages.is_empty() {
            tr!("draw.no_messages_yet_n", "No messages yet.\n").into()
        } else {
            self.messages
                .iter()
                .map(|(t, m)| format!("{}  {m}\n", xdevcmd_core::format::format_clock(*t)))
                .collect()
        };
        let n = self.messages.len();
        let doc = xdevcmd_core::document::Document::from_bytes("Messages", text.into_bytes());
        let v = &self.settings.config.viewer;
        let mut viewer = crate::viewer::Viewer::open(doc, v.wrap, v.tab_width);
        viewer.show_end();
        viewer.note(&tr!(
            "draw.message_of_the_last_in_memory",
            "{n} message{s} of the last {MESSAGES}, in memory only \u{2014} Esc back",
            n = n,
            s = if n == 1 { "" } else { "s" },
            MESSAGES = MESSAGES
        ));
        self.modal = Some(Modal::View(Box::new(viewer)));
    }

    /// The walker (`[general] animation`): a step a second across the whole screen, on
    /// the row of the lowest file pane's bottom edge ([`crate::walker::row`]). While it stands before a new message it is
    /// on the status line instead (`draw_status`).
    fn draw_walker(&self, frame: &mut Frame) {
        if !self.settings.config.general.animation {
            return;
        }
        let row = crate::walker::row(
            self.hits
                .rects
                .iter()
                .filter(|(id, _)| matches!(self.panes.get(id), Some(Pane::Files(_))))
                .map(|(_, r)| *r),
        );
        // The whole width: across both panes' edges (the owner, 2026-11-24).
        let screen = frame.area();
        let lane = row.map_or(0, |_| usize::from(screen.width));
        let hold = Duration::from_secs(self.settings.config.general.animation_hold_secs);
        let mut w = self.walker.borrow_mut();
        let standing = w.tick(Instant::now(), lane, &self.status, hold);
        // Not over a search or filter line, which is drawn on the same edge.
        let free = self.search.is_none();
        if let Some(y) = row.filter(|_| free && !standing && lane >= crate::walker::WIDTH) {
            frame.buffer_mut().set_string(
                screen.x + w.pos() as u16,
                y,
                w.sprite(),
                Style::new().fg(theme::t().accent),
            );
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
                    tr!("draw.u_tab", "  \u{b7}  tab {v}/{tabs_len}", v = tabs.active_index() + 1, tabs_len = tabs.len())
                } else {
                    String::new()
                };
                let sort = crate::lang::pick(SORT_WORDS, f.sort.key.label());
                tr!("draw.items_sort", " {summary}  ·  {entries_len} items  ·  sort {sort}{v}{v2}{tab}", summary = f.summary(), entries_len = f.entries.len(), sort = sort, v = if f.sort.reverse { " ▲" } else { " ▼" }, v2 = if f.show_hidden { tr!("draw.hidden", "  ·  hidden") } else { "".into() }, tab = tab)
            }
            Some(Pane::Term(p)) => tr!("draw.terminal_u_back_to_files_u", " terminal {v}/{tab_count}{v2}  \u{b7}  {unwrap_or_default} back to files  \u{b7}  {unwrap_or_default2} history", v = p.active_tab() + 1, tab_count = p.tab_count(), v2 = if p.split_count() > 1 {
                    format!("  \u{b7}  split {}/{}", p.focused_split(), p.split_count())
                } else {
                    String::new()
                }, unwrap_or_default = self.footer_key(Action::FocusTerminal).unwrap_or_default(), unwrap_or_default2 = self.footer_key(Action::TerminalHistory).unwrap_or_default()),
            None => String::new(),
        };
        let left = if self.zoom.is_some() {
            tr!("draw.u_zoomed", "{left}  \u{b7}  zoomed", left = left)
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
            tr!(
                "draw.u_u_job",
                "{left}  \u{b7}  \u{27f3} {jobs_len} job{s} {pct}%",
                left = left,
                jobs_len = self.jobs.len(),
                s = if self.jobs.len() == 1 { "" } else { "s" },
                pct = pct
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
        let badges = badge.cells() + group.as_ref().map_or(0, |g| g.cells() + 1);
        let left = format!("{}{left}", " ".repeat(badges));
        let right = format!("{} ", self.status);

        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        let avail = area.width as usize;
        // The right half (messages) wins space, but a gap of two is always kept between
        // them: with both halves long they otherwise ran together, "…historytoo small".
        let rw = right.cells().min(avail.saturating_sub(3));
        let lw = avail.saturating_sub(rw + 2);
        let gap = avail.saturating_sub(lw + rw).min(2);
        let shown_left = elide_middle(&left, lw);
        buf.set_line(
            area.x,
            area.y,
            &Line::from(Span::styled(
                shown_left.clone(),
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
        // The walker stands here, just before a new message, for its hold; it walks
        // on the file pane's bottom edge (`draw_walker`).
        let w = self.walker.borrow();
        let end = (lw + gap).saturating_sub(1);
        if self.settings.config.general.animation
            && w.standing()
            && end >= shown_left.cells() + 1 + crate::walker::WIDTH
        {
            frame.buffer_mut().set_string(
                area.x + (end - crate::walker::WIDTH) as u16,
                area.y,
                w.sprite(),
                Style::new().fg(theme::t().accent),
            );
        }
        drop(w);
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
            let x = area.x + badge.cells() as u16;
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
        // In the shell the F-keys are the shell's: a bar saying "8 Del" there lies, and
        // a click on it acted on the files behind (plan A10, UX). So the bar is three
        // buttons, with no key on them, for the mouse (the owner, 2026-11-25: way B);
        // their keys from the shell (Alt+X F1, Alt+X q) are in the docs. Quit there is
        // quit_no_cd, as Alt+X q.
        if self.focus_is_term() {
            const BUTTONS: &[(Action, Text)] = &[
                (Action::Help, Text::new("keybar.help", "Help")),
                (Action::MenuBar, Text::new("keybar.menu", "Menu")),
                (Action::QuitNoCd, Text::new("keybar.quit", "Quit")),
            ];
            let th = theme::t();
            let style = Style::new().fg(th.fg_dim);
            let mut spans = Vec::new();
            let mut hits = Vec::new();
            let mut cx = area.x;
            for (action, label) in BUTTONS {
                let text = format!(" [ {} ] ", label.get());
                let w = text.cells() as u16;
                if cx + w > area.x + area.width {
                    break;
                }
                // The click lands on the brackets and the word, not the spaces around.
                hits.push((*action, Rect::new(cx + 1, area.y, w - 2, 1)));
                cx += w;
                spans.push(Span::styled(text, style));
            }
            let buf = frame.buffer_mut();
            buf.set_style(area, Style::new().bg(th.bg_bar));
            buf.set_line(area.x, area.y, &Line::from(spans), area.width);
            self.hits.footer_hits = hits;
            return;
        }
        // Read from the keymap, not written down: after `copy = ["F2"]` a bar that still
        // says "5 Copy" is lying, and this is the line people look at most.
        // In F-key order, as in every orthodox manager. Too long for the screen: when
        // cutting the longest labels (to 3 columns with `…`) keeps every key, they are
        // cut (a longer language); else the ones that do not fit are left off whole,
        // from the end (a narrow screen).
        const KEYS: &[(Action, Text)] = &[
            (Action::Help, Text::new("keybar.help", "Help")),
            (Action::Rename, Text::new("keybar.rename", "Ren")),
            (Action::View, Text::new("keybar.view", "View")),
            (Action::Edit, Text::new("keybar.edit", "Edit")),
            (Action::Copy, Text::new("keybar.copy", "Copy")),
            (Action::Move, Text::new("keybar.move", "Move")),
            (Action::MkDir, Text::new("keybar.mkdir", "MkDir")),
            (Action::Delete, Text::new("keybar.delete", "Del")),
            (Action::MenuBar, Text::new("keybar.menu", "Menu")),
            (Action::Quit, Text::new("keybar.quit", "Quit")),
        ];
        // (action, key as shown, label): the keys this mode and keymap have.
        let items: Vec<(Action, String, String)> = KEYS
            .iter()
            .filter(|(action, _)| self.refused(*action).is_none())
            .filter_map(|(action, label)| {
                let key = self.footer_key(*action)?;
                let k = key
                    .strip_prefix('F')
                    .filter(|d| d.parse::<u8>().is_ok())
                    .unwrap_or(&key)
                    .to_string();
                Some((*action, k, label.get().into_owned()))
            })
            .collect();
        let labels = fit_labels(&items, area.width as usize);
        let mut spans = Vec::new();
        let mut hits = Vec::new();
        let mut cx = area.x;
        for ((action, k, _), label) in items.iter().zip(&labels) {
            let k = k.as_str();
            let w = (k.cells() + label.cells() + 3) as u16;
            if cx + w > area.x + area.width {
                break;
            }
            hits.push((*action, Rect::new(cx, area.y, w, 1)));
            cx += w;
            let th = theme::t();
            let number = Style::new().fg(th.accent).add_modifier(Modifier::BOLD);
            let cap = Style::new()
                .fg(th.bg)
                .bg(th.accent)
                .add_modifier(Modifier::BOLD);
            let dim = Style::new().fg(th.fg_dim);
            // The same width in each look: " 1 Help ", " 1 Help " with the 1 on the
            // accent, "◢1◤Help ".
            match crate::decor::get().keys {
                crate::decor::Keys::Plain => spans.extend([
                    Span::styled(format!(" {k}"), number),
                    Span::styled(format!(" {label} "), dim),
                ]),
                crate::decor::Keys::Caps => spans.extend([
                    Span::raw(" "),
                    Span::styled(k.to_string(), cap),
                    Span::styled(format!(" {label} "), dim),
                ]),
                crate::decor::Keys::Slant => spans.extend([
                    Span::styled("\u{25e2}", Style::new().fg(th.accent)),
                    Span::styled(k.to_string(), cap),
                    Span::styled("\u{25e4}", Style::new().fg(th.accent)),
                    Span::styled(format!("{label} "), dim),
                ]),
            }
        }
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);
        self.hits.footer_hits = hits;
    }
}
