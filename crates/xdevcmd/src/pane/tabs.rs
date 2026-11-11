//! Tabs for a file pane, the way Total Commander has them: Ctrl+T opens one on the same
//! directory, Ctrl+W closes it, Ctrl+PgUp/PgDn move between them.
//!
//! The bar only appears with two tabs or more, so a pane that never uses tabs keeps
//! its full height. Everything that asks "the file pane" gets the tab on show, which
//! is why almost nothing outside this file had to change.

use std::path::Path;

use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::format::elide_middle;

use crate::pane::FilePane;
use crate::theme;

pub enum Click {
    Nothing,
    New,
    Close(usize),
    Selected,
}

pub struct FileTabs {
    tabs: Vec<FilePane>,
    active: usize,
    tab_hits: Vec<(usize, Rect)>,
    close_hits: Vec<(usize, Rect)>,
    new_hit: Rect,
}

impl FileTabs {
    pub fn new(first: FilePane) -> Self {
        Self {
            tabs: vec![first],
            active: 0,
            tab_hits: Vec::new(),
            close_hits: Vec::new(),
            new_hit: Rect::ZERO,
        }
    }

    pub fn active(&self) -> &FilePane {
        &self.tabs[self.active]
    }

    pub fn active_mut(&mut self) -> &mut FilePane {
        &mut self.tabs[self.active]
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn tabs_mut(&mut self) -> &mut [FilePane] {
        &mut self.tabs
    }

    pub fn tabs(&self) -> &[FilePane] {
        &self.tabs
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Open a tab on the directory the current one shows, right after it, and show it.
    pub fn duplicate(&mut self) {
        let cur = &self.tabs[self.active];
        let tab = FilePane::new(&cur.cwd, cur.sort, cur.show_hidden);
        self.active += 1;
        self.tabs.insert(self.active, tab);
    }

    pub fn push(&mut self, tab: FilePane) {
        self.tabs.push(tab);
    }

    pub fn select(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
        }
    }

    /// Close tab `i`. False when it is the last one: a pane always shows something,
    /// and closing the pane itself is Alt+W.
    pub fn close(&mut self, i: usize) -> bool {
        if self.tabs.len() <= 1 || i >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(i);
        if self.active > i || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        true
    }

    pub fn cycle(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        self.active = (self.active as isize + delta).rem_euclid(n) as usize;
    }

    pub fn reload_all(&mut self) {
        for t in &mut self.tabs {
            t.reload();
        }
    }

    pub fn bar_button_at(&self, x: u16, y: u16) -> bool {
        let p = Position::new(x, y);
        self.new_hit.contains(p)
            || self.tab_hits.iter().any(|(_, r)| r.contains(p))
            || self.close_hits.iter().any(|(_, r)| r.contains(p))
    }

    pub fn on_click(&mut self, x: u16, y: u16) -> Click {
        let p = Position::new(x, y);
        if self.new_hit.contains(p) {
            return Click::New;
        }
        if let Some((i, _)) = self.close_hits.iter().find(|(_, r)| r.contains(p)) {
            return Click::Close(*i);
        }
        if let Some((i, _)) = self.tab_hits.iter().find(|(_, r)| r.contains(p)) {
            self.active = *i;
            return Click::Selected;
        }
        Click::Nothing
    }

    /// Whether this pane needs its tab bar on its own: more than one tab, and room.
    pub fn wants_bar(&self, area: Rect) -> bool {
        self.tabs.len() > 1 && area.height > 3
    }

    /// Draw the pane. `bar` shows the tab bar even with one tab: a pane beside one that
    /// has a bar shows its own too, so their rows stay in line.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        drop_target: bool,
        bar: bool,
    ) {
        self.tab_hits.clear();
        self.close_hits.clear();
        self.new_hit = Rect::ZERO;
        let body = if (bar || self.tabs.len() > 1) && area.height > 3 {
            self.render_bar(frame, Rect::new(area.x, area.y, area.width, 1), active);
            Rect::new(area.x, area.y + 1, area.width, area.height - 1)
        } else {
            area
        };
        self.tabs[self.active].render(frame, body, active, drop_target);
    }

    fn render_bar(&mut self, frame: &mut Frame, bar: Rect, active: bool) {
        let th = theme::t();
        frame
            .buffer_mut()
            .set_style(bar, Style::new().bg(th.bg_bar));
        let right = bar.x + bar.width;
        let mut cx = bar.x;
        let put = |frame: &mut Frame, x: u16, text: &str, style: Style| -> u16 {
            if x < right {
                frame.buffer_mut().set_line(
                    x,
                    bar.y,
                    &Line::from(Span::styled(text.to_string(), style)),
                    right - x,
                );
            }
            text.chars().count() as u16
        };
        for (i, tab) in self.tabs.iter().enumerate() {
            let style = match (i == self.active, active) {
                (true, true) => Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD),
                (true, false) => Style::new().fg(th.fg).bg(th.row_cursor),
                (false, _) => Style::new().fg(th.fg_dim),
            };
            let label = format!(" {} {} ", i + 1, tab_label(&tab.cwd));
            let w = put(frame, cx, &label, style);
            self.tab_hits.push((i, Rect::new(cx, bar.y, w, 1)));
            cx += w;
            // The last tab cannot be closed, so it has no ×.
            if self.tabs.len() > 1 {
                let w = put(frame, cx, "\u{00d7}", style.fg(th.fg_dim));
                self.close_hits.push((i, Rect::new(cx, bar.y, w, 1)));
                cx += w;
            }
            cx += 1;
        }
        let w = put(
            frame,
            cx,
            " + ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        );
        self.new_hit = Rect::new(cx, bar.y, w, 1);
    }
}

/// A directory's name, short enough that several tabs fit on a narrow pane.
pub fn tab_label(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());
    elide_middle(&name, 14)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xdevcmd_core::Sort;

    fn tabs() -> FileTabs {
        let d = std::env::temp_dir();
        FileTabs::new(FilePane::new(&d, Sort::default(), false))
    }

    #[test]
    fn duplicating_opens_the_same_directory_after_the_current_tab() {
        let mut t = tabs();
        t.duplicate();
        assert_eq!(t.len(), 2);
        assert_eq!(t.active_index(), 1);
        assert_eq!(t.tabs()[0].cwd, t.tabs()[1].cwd);
    }

    #[test]
    fn the_last_tab_cannot_be_closed() {
        let mut t = tabs();
        assert!(!t.close(0));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn closing_keeps_a_sensible_tab_active() {
        let mut t = tabs();
        t.duplicate();
        t.duplicate(); // three tabs, the third active
        assert!(t.close(2));
        assert_eq!(
            t.active_index(),
            1,
            "closing the active last tab steps left"
        );
        t.select(1);
        assert!(t.close(0));
        assert_eq!(
            t.active_index(),
            0,
            "closing one before the active keeps it in view"
        );
    }

    #[test]
    fn cycling_wraps_both_ways() {
        let mut t = tabs();
        t.duplicate();
        t.duplicate();
        t.select(0);
        t.cycle(-1);
        assert_eq!(t.active_index(), 2);
        t.cycle(1);
        assert_eq!(t.active_index(), 0);
    }
}
