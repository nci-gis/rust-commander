//! Tabs for a file pane, the way Total Commander has them: Ctrl+T opens one on the same
//! directory, Ctrl+W closes it, Ctrl+PgUp/PgDn move between them.
//!
//! The bar only appears with two tabs or more, so a pane that never uses tabs keeps
//! its full height. A pane holds at most [`MAX_TABS`]. When they do not fit, the bar
//! either stays one row and shows the tabs around the selected one, with `‹k` and `k›`
//! for those left out, or wraps onto up to [`MAX_ROWS`] rows first (`[files] tab_bar`). Everything that asks "the file pane" gets the tab on show, which
//! is why almost nothing outside this file had to change.

use std::path::Path;
use xdevcmd_core::format::Cells;

use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::format::elide_middle;

use crate::pane::FilePane;
use crate::theme;

/// Tabs a file pane holds at most, as `[terminal] max_terminals` allows shells.
pub const MAX_TABS: usize = 16;

/// Rows a wrapping tab bar takes at most; past them it shows a window, as one row does.
pub const MAX_ROWS: u16 = 3;

/// Whether the tab bar wraps (`[files] tab_bar = "multi-line"`), set from the config at
/// start and when Settings saves.
static WRAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_wrap(on: bool) {
    WRAP.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn max_rows() -> u16 {
    if WRAP.load(std::sync::atomic::Ordering::Relaxed) {
        MAX_ROWS
    } else {
        1
    }
}

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
    /// False, and nothing opened, when the pane already holds [`MAX_TABS`].
    #[must_use]
    pub fn duplicate(&mut self) -> bool {
        if self.tabs.len() >= MAX_TABS {
            return false;
        }
        let cur = &self.tabs[self.active];
        let tab = FilePane::new(&cur.cwd, cur.sort, cur.show_hidden);
        self.active += 1;
        self.tabs.insert(self.active, tab);
        true
    }

    /// Add a tab at the end (a session coming back); past [`MAX_TABS`] it is left out.
    pub fn push(&mut self, tab: FilePane) {
        if self.tabs.len() < MAX_TABS {
            self.tabs.push(tab);
        }
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

    /// The rows this pane's tab bar needs on its own in `area`: none with one tab or
    /// no room, else one, or up to [`MAX_ROWS`] when the bar wraps.
    pub fn bar_rows(&self, area: Rect) -> u16 {
        if self.tabs.len() > 1 && area.height > 3 {
            self.layout(area.width, self.room(area)).rows
        } else {
            0
        }
    }

    /// Rows the bar may take in `area`: the list keeps at least three.
    fn room(&self, area: Rect) -> u16 {
        max_rows().min(area.height.saturating_sub(3)).max(1)
    }

    /// Draw the pane. `rows` is the bar's height: a pane beside one that has a bar shows
    /// its own too, as tall, so their rows stay in line. 0 leaves it to this pane.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        drop_target: bool,
        rows: u16,
    ) {
        self.tab_hits.clear();
        self.close_hits.clear();
        self.new_hit = Rect::ZERO;
        let rows = rows.max(self.bar_rows(area));
        let body = if rows > 0 && area.height > rows + 2 {
            self.render_bar(frame, Rect::new(area.x, area.y, area.width, rows), active);
            Rect::new(area.x, area.y + rows, area.width, area.height - rows)
        } else {
            area
        };
        self.tabs[self.active].render(frame, body, active, drop_target);
    }

    /// Where each piece of the bar goes in `width` cells and at most `rows` rows.
    fn layout(&self, width: u16, rows: u16) -> Layout {
        let names: Vec<String> = self.tabs.iter().map(|t| dir_name(&t.cwd)).collect();
        layout(&names, self.active, width, rows)
    }

    fn render_bar(&mut self, frame: &mut Frame, bar: Rect, active: bool) {
        let th = theme::t();
        frame
            .buffer_mut()
            .set_style(bar, Style::new().bg(th.bg_bar));
        let plan = self.layout(bar.width, bar.height);
        let right = bar.x + bar.width;
        let put = |frame: &mut Frame, x: u16, y: u16, text: &str, style: Style| {
            if x < right {
                frame.buffer_mut().set_line(
                    x,
                    y,
                    &Line::from(Span::styled(text.to_string(), style)),
                    right - x,
                );
            }
        };
        let slant_look = crate::decor::get().keys == crate::decor::Keys::Slant;
        for p in &plan.pieces {
            let (x, y) = (bar.x + p.x, bar.y + p.row);
            let at = Rect::new(x, y, p.width.min(right.saturating_sub(x)), 1);
            match p.what {
                Piece::Tab(i) => {
                    let style = match (i == self.active, active) {
                        (true, true) => Style::new()
                            .fg(th.bg)
                            .bg(th.accent)
                            .add_modifier(Modifier::BOLD),
                        (true, false) => Style::new().fg(th.fg).bg(th.row_cursor),
                        (false, _) => Style::new().fg(th.fg_dim),
                    };
                    put(frame, x, y, &p.text, style);
                    // The selected tab of the focused pane as a slanted segment, in that
                    // look; only where all of it is on screen.
                    if i == self.active && active && slant_look && x + p.width <= right {
                        crate::decor::slant(frame.buffer_mut(), x, x + p.width - 1, y);
                    }
                    self.tab_hits.push((i, at));
                }
                Piece::Close(i) => {
                    let style = if i == self.active && active && !slant_look {
                        Style::new().fg(th.fg_dim).bg(th.accent)
                    } else if i == self.active && !active {
                        Style::new().fg(th.fg_dim).bg(th.row_cursor)
                    } else {
                        Style::new().fg(th.fg_dim)
                    };
                    put(frame, x, y, &p.text, style);
                    self.close_hits.push((i, at));
                }
                // A count of tabs left out; clicking it steps to the nearest of them.
                Piece::More(i) => {
                    put(frame, x, y, &p.text, Style::new().fg(th.accent));
                    self.tab_hits.push((i, at));
                }
                Piece::New => {
                    put(
                        frame,
                        x,
                        y,
                        &p.text,
                        Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
                    );
                    self.new_hit = at;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    Tab(usize),
    Close(usize),
    /// Tabs left out on one side; the index is the nearest of them.
    More(usize),
    New,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Placed {
    what: Piece,
    text: String,
    x: u16,
    row: u16,
    width: u16,
}

#[derive(Debug, Default)]
struct Layout {
    pieces: Vec<Placed>,
    rows: u16,
}

/// Lay the bar out: every tab at its usual length if they fit in `rows` rows, then with
/// shorter names, then only the tabs around the selected one with `‹k`/`k›` for the
/// rest. Nothing goes past `width`; the selected tab is always shown.
fn layout(names: &[String], active: usize, width: u16, rows: u16) -> Layout {
    let n = names.len();
    let rows = rows.max(1);
    for lim in [14, 8] {
        if let Some(l) = pack(names, 0, n - 1, lim, width, rows) {
            return l;
        }
    }
    // A window around the selected tab, grown one tab at a time, right then left.
    let (mut a, mut b) = (active, active);
    let fits = |a, b, lim| pack(names, a, b, lim, width, rows);
    let mut lim = 8;
    if fits(a, b, lim).is_none() {
        // Even the selected tab alone is too wide: its name takes what is left.
        let spare = width.saturating_sub(24) as usize;
        lim = spare.clamp(1, 8);
    }
    loop {
        let mut grew = false;
        if b + 1 < n && fits(a, b + 1, lim).is_some() {
            b += 1;
            grew = true;
        }
        if a > 0 && fits(a - 1, b, lim).is_some() {
            a -= 1;
            grew = true;
        }
        if !grew {
            break;
        }
    }
    fits(a, b, lim).unwrap_or_else(|| {
        // Narrower than any bar: the selected tab alone, cut at the edge.
        let text = format!(" {} ", active + 1);
        let width_t = text.cells() as u16;
        Layout {
            pieces: vec![Placed {
                what: Piece::Tab(active),
                text,
                x: 0,
                row: 0,
                width: width_t,
            }],
            rows: 1,
        }
    })
}

/// Tabs `a..=b` with names cut to `lim`, markers for the rest, then `+`, wrapped into
/// `width`: None if that takes more than `rows` rows or a piece cannot fit at all.
fn pack(names: &[String], a: usize, b: usize, lim: usize, width: u16, rows: u16) -> Option<Layout> {
    let n = names.len();
    // Units that stay on one row together: a tab and its ×.
    let mut units: Vec<Vec<(Piece, String)>> = Vec::new();
    if a > 0 {
        units.push(vec![(Piece::More(a - 1), format!("\u{2039}{a}"))]);
    }
    for (i, name) in names.iter().enumerate().take(b + 1).skip(a) {
        let label = format!(" {} {} ", i + 1, elide_middle(name, lim));
        let mut u = vec![(Piece::Tab(i), label)];
        // The last tab cannot be closed, so it has no ×.
        if n > 1 {
            u.push((Piece::Close(i), "\u{00d7}".to_string()));
        }
        units.push(u);
    }
    if b + 1 < n {
        units.push(vec![(Piece::More(b + 1), format!("{}\u{203a}", n - 1 - b))]);
    }
    units.push(vec![(Piece::New, " + ".to_string())]);
    let mut out = Layout {
        pieces: Vec::new(),
        rows: 1,
    };
    let (mut x, mut row) = (0u16, 0u16);
    for u in units {
        let w: u16 = u.iter().map(|(_, t)| t.cells() as u16).sum();
        if w > width {
            return None;
        }
        if x > 0 && x + w > width {
            row += 1;
            x = 0;
            if row >= rows {
                return None;
            }
        }
        for (what, text) in u {
            let tw = text.cells() as u16;
            out.pieces.push(Placed {
                what,
                text,
                x,
                row,
                width: tw,
            });
            x += tw;
        }
        x += 1; // a gap between units
    }
    out.rows = row + 1;
    Some(out)
}

/// A directory's name for its tab: the last part, or the whole path at a root.
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
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
        assert!(t.duplicate());
        assert_eq!(t.len(), 2);
        assert_eq!(t.active_index(), 1);
        assert_eq!(t.tabs()[0].cwd, t.tabs()[1].cwd);
    }

    #[test]
    fn a_pane_holds_at_most_sixteen_tabs() {
        let mut t = tabs();
        for _ in 1..MAX_TABS {
            assert!(t.duplicate());
        }
        assert!(!t.duplicate(), "the 17th is refused");
        assert_eq!(t.len(), MAX_TABS);
        t.push(FilePane::new(&std::env::temp_dir(), Sort::default(), false));
        assert_eq!(t.len(), MAX_TABS, "a session cannot bring back more");
    }

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("directory-{i:02}")).collect()
    }

    /// Nothing past the edge, the selected tab shown, every piece on a row it may use.
    fn check(l: &Layout, active: usize, width: u16, rows: u16) {
        assert!(l.rows >= 1 && l.rows <= rows, "{l:?}");
        for p in &l.pieces {
            assert!(p.x + p.width <= width, "{p:?} past {width}");
            assert!(p.row < l.rows, "{p:?}");
        }
        assert!(
            l.pieces.iter().any(|p| p.what == Piece::Tab(active)),
            "{l:?}"
        );
    }

    #[test]
    fn tabs_that_fit_are_all_shown_on_one_row() {
        let l = layout(&names(3), 1, 120, 1);
        check(&l, 1, 120, 1);
        assert_eq!(l.rows, 1);
        assert!(!l.pieces.iter().any(|p| matches!(p.what, Piece::More(_))));
        assert!(l.pieces.iter().any(|p| p.what == Piece::New));
    }

    #[test]
    fn one_row_shows_the_tabs_around_the_selected_one_and_counts_the_rest() {
        for active in [0, 7, 15] {
            let l = layout(&names(16), active, 60, 1);
            check(&l, active, 60, 1);
            let shown: Vec<usize> = l
                .pieces
                .iter()
                .filter_map(|p| match p.what {
                    Piece::Tab(i) => Some(i),
                    _ => None,
                })
                .collect();
            let (lo, hi) = (shown[0], *shown.last().unwrap());
            assert_eq!(shown, (lo..=hi).collect::<Vec<_>>(), "one run of tabs");
            let more: Vec<usize> = l
                .pieces
                .iter()
                .filter_map(|p| match p.what {
                    Piece::More(i) => Some(i),
                    _ => None,
                })
                .collect();
            let want: Vec<usize> = [(lo > 0).then(|| lo - 1), (hi < 15).then_some(hi + 1)]
                .into_iter()
                .flatten()
                .collect();
            assert_eq!(
                more, want,
                "a marker for each side left out, pointing next to it"
            );
        }
    }

    #[test]
    fn several_rows_wrap_before_leaving_tabs_out() {
        let l = layout(&names(8), 7, 60, 3);
        check(&l, 7, 60, 3);
        assert!(l.rows > 1, "{l:?}");
        assert!(!l.pieces.iter().any(|p| matches!(p.what, Piece::More(_))));
        let l = layout(&names(16), 3, 40, 3);
        check(&l, 3, 40, 3);
        assert!(
            l.pieces.iter().any(|p| matches!(p.what, Piece::More(_))),
            "past 3 rows, a window"
        );
    }

    #[test]
    fn a_narrow_bar_still_shows_the_selected_tab() {
        for w in [6, 12, 20, 30] {
            let l = layout(&names(16), 9, w, 1);
            check(&l, 9, w, 1);
        }
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
        assert!(t.duplicate());
        assert!(t.duplicate()); // three tabs, the third active
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
        assert!(t.duplicate());
        assert!(t.duplicate());
        t.select(0);
        t.cycle(-1);
        assert_eq!(t.active_index(), 2);
        t.cycle(1);
        assert_eq!(t.active_index(), 0);
    }
}
