//! The terminal panel: tabs of shells, and each tab its own split layout.
//!
//! A tab reuses the same [`Node`] tree the file panes are laid out with, so splitting
//! side by side or top and bottom, closing, and dragging a boundary to resize all work
//! inside a tab without a second implementation of any of it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::layout::{Axis, Divider, Node, PaneId};
use crate::pane::TermPane;
use crate::theme;

struct Tab {
    tree: Node,
    terms: BTreeMap<PaneId, TermPane>,
    focus: PaneId,
}

impl Tab {
    fn label(&self) -> String {
        let l = self
            .terms
            .get(&self.focus)
            .map_or_else(|| "shell".to_string(), TermPane::tab_label);
        // A deep directory name must not push the + button off the bar.
        xdevcmd_core::format::elide_middle(&l, 16)
    }

    fn leaves(&self) -> Vec<PaneId> {
        let mut v = Vec::new();
        self.tree.leaves(&mut v);
        v
    }
}

/// What a click in the panel asked for. Opening a tab needs the file pane's directory,
/// which only the app knows, so that one is handed back rather than done here.
pub enum Click {
    Nothing,
    NewTab,
    CloseTab(usize),
    Selected,
}

pub struct TermPanel {
    tabs: Vec<Tab>,
    active: usize,
    next_id: PaneId,
    shell: String,
    // Where things were drawn, for clicks.
    tab_hits: Vec<(usize, Rect)>,
    close_hits: Vec<(usize, Rect)>,
    new_hit: Rect,
    rects: Vec<(PaneId, Rect)>,
    body: Rect,
}

impl TermPanel {
    pub fn new(cwd: &Path, rows: u16, cols: u16, shell: &str) -> Result<Self> {
        let mut p = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
            shell: shell.to_string(),
            tab_hits: Vec::new(),
            close_hits: Vec::new(),
            new_hit: Rect::ZERO,
            rects: Vec::new(),
            body: Rect::new(0, 0, cols + 2, rows + 2),
        };
        p.add_tab(cwd)?;
        Ok(p)
    }

    fn spawn(&mut self, cwd: &Path) -> Result<(PaneId, TermPane)> {
        // Start at the size the body last had; the first draw corrects it anyway.
        let rows = self.body.height.saturating_sub(2).max(1);
        let cols = self.body.width.saturating_sub(2).max(1);
        let t = TermPane::spawn(cwd, rows, cols, &self.shell)?;
        let id = self.next_id;
        self.next_id += 1;
        Ok((id, t))
    }

    /// A tab running one program by its argv, kept after it ends; it becomes the
    /// active tab.
    pub fn add_program_tab(&mut self, cwd: &Path, argv: &[std::ffi::OsString]) -> Result<()> {
        let rows = self.body.height.saturating_sub(2).max(1);
        let cols = self.body.width.saturating_sub(2).max(1);
        let t = TermPane::spawn_argv(cwd, rows, cols, argv)?;
        let id = self.next_id;
        self.next_id += 1;
        let mut terms = BTreeMap::new();
        terms.insert(id, t);
        self.tabs.push(Tab {
            tree: Node::Leaf(id),
            terms,
            focus: id,
        });
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    /// The focused pane is a program run in a tab that has ended: Enter or Esc closes it.
    pub fn focused_program_ended(&self) -> bool {
        self.focused().is_some_and(|t| t.keeps() && t.has_exited())
    }

    pub fn add_tab(&mut self, cwd: &Path) -> Result<()> {
        let (id, t) = self.spawn(cwd)?;
        let mut terms = BTreeMap::new();
        terms.insert(id, t);
        self.tabs.push(Tab {
            tree: Node::Leaf(id),
            terms,
            focus: id,
        });
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    pub fn split(&mut self, axis: Axis, cwd: &Path) -> Result<()> {
        // Each half needs a border plus a few usable rows or columns. A shell squeezed
        // below that is useless, so say so instead of making one.
        let tab = &self.tabs[self.active];
        if let Some((_, r)) = self.rects.iter().find(|(id, _)| *id == tab.focus) {
            let (have, need, what) = match axis {
                Axis::Rows => (r.height, 2 * (2 + 3), "taller"),
                Axis::Cols => (r.width, 2 * (2 + 12), "wider"),
            };
            if have < need {
                anyhow::bail!("too small to split \u{2014} make the terminal {what} first");
            }
        }
        let (id, t) = self.spawn(cwd)?;
        let tab = &mut self.tabs[self.active];
        if tab.tree.split_leaf(tab.focus, axis, id) {
            tab.terms.insert(id, t);
            tab.focus = id;
        }
        Ok(())
    }

    /// Remove one shell. Returns true when the panel has nothing left.
    fn remove(&mut self, tab: usize, id: PaneId) -> bool {
        let t = &mut self.tabs[tab];
        // Dropping the TermPane drops the pty master, which hangs up the shell.
        t.terms.remove(&id);
        if t.terms.is_empty() {
            self.tabs.remove(tab);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            } else if self.active > tab {
                self.active -= 1;
            }
            return self.tabs.is_empty();
        }
        t.tree.remove_leaf(id);
        if t.focus == id {
            t.focus = t.leaves()[0];
        }
        false
    }

    pub fn close_focused(&mut self) -> bool {
        let id = self.tabs[self.active].focus;
        self.remove(self.active, id)
    }

    pub fn close_tab(&mut self, i: usize) -> bool {
        if i >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(i);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        } else if self.active > i {
            self.active -= 1;
        }
        self.tabs.is_empty()
    }

    /// Close every shell whose process has ended — typing `exit` closes its terminal,
    /// as in an editor. Returns how many went, and whether the panel is now empty.
    pub fn reap(&mut self) -> (usize, bool) {
        let mut gone = 0;
        for tab in (0..self.tabs.len()).rev() {
            let dead: Vec<PaneId> = self.tabs[tab]
                .terms
                .iter()
                // A program's own tab stays for its output; Enter or Esc closes it.
                .filter(|(_, t)| t.has_exited() && !t.keeps())
                .map(|(id, _)| *id)
                .collect();
            for id in dead {
                gone += 1;
                if self.remove(tab, id) {
                    return (gone, true);
                }
                if tab >= self.tabs.len() {
                    break;
                }
            }
        }
        (gone, self.tabs.is_empty())
    }

    pub fn cycle_tab(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        if n > 0 {
            self.active = (self.active as isize + delta).rem_euclid(n) as usize;
        }
    }

    pub fn next_split(&mut self) {
        let tab = &mut self.tabs[self.active];
        let leaves = tab.leaves();
        if let Some(i) = leaves.iter().position(|id| *id == tab.focus) {
            tab.focus = leaves[(i + 1) % leaves.len()];
        }
    }

    /// Each tab's directory (its focused shell's), or an empty string where unknown.
    pub fn tab_dirs(&self) -> Vec<String> {
        self.tabs
            .iter()
            // A tab started for one program is not reopened next time as a shell.
            .filter(|t| t.terms.get(&t.focus).is_none_or(|p| !p.keeps()))
            .map(|t| {
                t.terms
                    .get(&t.focus)
                    .and_then(TermPane::cwd)
                    .and_then(|p| p.to_str().map(str::to_string))
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Every running shell, across all tabs and splits — what the limit counts.
    pub fn shell_count(&self) -> usize {
        self.tabs.iter().map(|t| t.terms.len()).sum()
    }

    pub fn select_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_tab(&self) -> usize {
        self.active
    }

    pub fn split_count(&self) -> usize {
        self.tabs.get(self.active).map_or(0, |t| t.terms.len())
    }

    /// 1-based position of the focused shell among the tab's splits.
    pub fn focused_split(&self) -> usize {
        self.tabs.get(self.active).map_or(0, |tab| {
            tab.leaves()
                .iter()
                .position(|id| *id == tab.focus)
                .map_or(0, |i| i + 1)
        })
    }

    fn focused(&self) -> Option<&TermPane> {
        let tab = self.tabs.get(self.active)?;
        tab.terms.get(&tab.focus)
    }

    pub fn send_key(&self, k: KeyEvent) -> bool {
        self.focused().is_some_and(|t| t.send_key(k))
    }

    pub fn shell_name(&self) -> String {
        self.focused()
            .map(|t| t.shell_name().to_string())
            .unwrap_or_default()
    }

    /// See [`TermPane::foreground_program`].
    pub fn busy_with(&self) -> Option<String> {
        self.focused().and_then(TermPane::foreground_program)
    }

    pub fn send(&self, bytes: Vec<u8>) {
        if let Some(t) = self.focused() {
            t.send(bytes);
        }
    }

    /// Move a border between this tab's shells. False when the focused shell has no
    /// border that way inside the tab, so the caller moves the panel's own edge.
    pub fn move_boundary(&mut self, axis: Axis, delta: i16) -> bool {
        let tab = &mut self.tabs[self.active];
        tab.terms.len() > 1 && tab.tree.move_boundary(tab.focus, axis, delta)
    }

    pub fn dividers(&self) -> Vec<Divider> {
        let mut out = Vec::new();
        if let Some(tab) = self.tabs.get(self.active) {
            tab.tree.dividers(self.body, &mut out);
        }
        out
    }

    pub fn set_pct(&mut self, id: usize, pct: u16) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.tree.set_pct(id, pct);
        }
    }

    pub fn scroll_at(&mut self, x: u16, y: u16, delta: isize) {
        let p = Position::new(x, y);
        let Some(id) = self
            .rects
            .iter()
            .find(|(_, r)| r.contains(p))
            .map(|(id, _)| *id)
        else {
            return;
        };
        if let Some(t) = self.tabs[self.active].terms.get(&id) {
            t.scroll_by(delta);
        }
    }

    /// True when the point is on a tab, a close mark or the + button. The tab bar is
    /// the panel's first row, which is also half of the strip that resizes the panel,
    /// so a button has to win over the resize or no tab could ever be clicked.
    pub fn bar_button_at(&self, x: u16, y: u16) -> bool {
        let p = Position::new(x, y);
        self.new_hit.contains(p)
            || self.tab_hits.iter().any(|(_, r)| r.contains(p))
            || self.close_hits.iter().any(|(_, r)| r.contains(p))
    }

    pub fn on_click(&mut self, x: u16, y: u16) -> Click {
        let p = Position::new(x, y);
        if self.new_hit.contains(p) {
            return Click::NewTab;
        }
        if let Some((i, _)) = self.close_hits.iter().find(|(_, r)| r.contains(p)) {
            return Click::CloseTab(*i);
        }
        if let Some((i, _)) = self.tab_hits.iter().find(|(_, r)| r.contains(p)) {
            self.active = *i;
            return Click::Selected;
        }
        if let Some((id, _)) = self.rects.iter().find(|(_, r)| r.contains(p)) {
            self.tabs[self.active].focus = *id;
            return Click::Selected;
        }
        Click::Nothing
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, active: bool) {
        let th = theme::t();
        if area.height == 0 || self.tabs.is_empty() {
            return;
        }

        // ── tab bar ────────────────────────────────────────────────────────────────
        let bar = Rect::new(area.x, area.y, area.width, 1);
        frame
            .buffer_mut()
            .set_style(bar, Style::new().bg(th.bg_bar));
        self.tab_hits.clear();
        self.close_hits.clear();
        let right = area.x + area.width;
        let mut cx = area.x;
        let put = |frame: &mut Frame, x: u16, text: &str, style: Style| -> u16 {
            let w = text.chars().count() as u16;
            if x < right {
                frame.buffer_mut().set_line(
                    x,
                    bar.y,
                    &Line::from(Span::styled(text.to_string(), style)),
                    right - x,
                );
            }
            w
        };
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active;
            let style = match (is_active, active) {
                (true, true) => Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD),
                (true, false) => Style::new().fg(th.fg).bg(th.row_cursor),
                (false, _) => Style::new().fg(th.fg_dim),
            };
            let label = format!(" \u{2302} {} {} ", i + 1, tab.label());
            let w = put(frame, cx, &label, style);
            self.tab_hits.push((i, Rect::new(cx, bar.y, w, 1)));
            cx += w;
            let w = put(frame, cx, "\u{00d7}", style.fg(th.fg_dim));
            self.close_hits.push((i, Rect::new(cx, bar.y, w, 1)));
            cx += w + 1;
        }
        let w = put(
            frame,
            cx,
            " + ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        );
        self.new_hit = Rect::new(cx, bar.y, w, 1);

        // ── the active tab's shells ────────────────────────────────────────────────
        self.body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        if self.body.height == 0 {
            return;
        }
        let tab = &mut self.tabs[self.active];
        let mut rects = Vec::new();
        tab.tree.assign(self.body, &mut rects);
        for (id, r) in &rects {
            if let Some(t) = tab.terms.get_mut(id) {
                let (rows, cols) = TermPane::inner_size(*r);
                t.resize(rows, cols);
                t.render(frame, *r, active && *id == tab.focus);
            }
        }
        self.rects = rects;
    }
}
