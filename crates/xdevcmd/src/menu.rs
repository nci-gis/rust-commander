//! A popup menu, drawn where the pointer is. Used for right-click and for the
//! "copy or move?" question a drag-and-drop ends in.
//!
//! Items carry a [`Cmd`] rather than a closure: most are an [`Action`], so a menu entry
//! does exactly what its key does, through the same code path.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::ops;

use crate::keys::Action;
use crate::theme;

pub enum Cmd {
    Act(Action),
    /// Put the selected paths on the clipboard through the terminal (OSC 52).
    CopyPath,
    /// The end of a drag-and-drop: the sources and destination are already known.
    Transfer {
        kind: ops::Kind,
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    /// Show this directory in the focused pane (history, hotlist).
    GoTo(PathBuf),
    /// Show this kept list in a file pane (the history and lists menu).
    ShowList(crate::lists::Id),
    /// `P` in a shell's history: add its paths to this list, or to a new one.
    AddToList(Option<crate::lists::Id>),
    AddBookmark(PathBuf),
    RemoveBookmark(PathBuf),
    /// Open with the system's application, already confirmed.
    OpenFile(PathBuf),
    /// Run a program in a terminal tab of its own, by argv, in this directory.
    RunInTerminal {
        argv: Vec<std::ffi::OsString>,
        cwd: PathBuf,
    },
    /// Rule `n` of `[[open]]` on this file (Shift+O).
    ApplyRule(usize, PathBuf),
    /// A checked plan from Shift+F2: each file and its new path.
    BatchRename(Vec<(PathBuf, PathBuf)>),
    /// Entry `n` of `[[menu]]`.
    UserCommand(usize),
    /// Put this text on the clipboard (OSC 52).
    CopyText(String),
    /// Save as… from the viewer, onto a file that exists: the user said replace it.
    SaveReplacing(PathBuf),
    /// The menu on a terminal tab (its `TabId`, which outlives a neighbour closing):
    /// its history in the viewer.
    TabHistory(crate::pane::terms::TabId),
    /// The menu on a terminal tab: Save as… for its history, without the viewer.
    TabSaveHistory(crate::pane::terms::TabId),
    /// The menu on a terminal tab: a list of the paths in its last command's output.
    TabList(crate::pane::terms::TabId),
    /// The menu on a terminal tab: close it, every shell in it.
    TabClose(crate::pane::terms::TabId),
    /// A confirmed synchronisation plan: each source and the directory it goes into.
    Sync(Vec<(PathBuf, PathBuf)>),
    /// Quit even though jobs are still running: they are cancelled first.
    QuitAnyway,
    /// Quit now, though cancelled jobs have not finished stopping (a hung drive).
    QuitNow,
    /// Run an interrupted job again (safe operations' journal), "replace only if older".
    Resume(xdevcmd_core::journal::Interrupted),
    /// Help → About.
    About,
    /// Help → Commands…: F1, opened at its large size.
    Commands,
    /// Help → Config in use: the loaded config file, read-only.
    ConfigInUse,
    /// Help → Documentation: the local copy of `docs/` and the link at this version.
    Documentation,
    /// Read this file in the viewer (the local docs' index), as F3 would.
    ViewFile(PathBuf),
    /// View → Theme…: the list of themes.
    ThemeMenu,
    /// File → Settings….
    Settings,
    /// Undo the last operation, confirmed.
    Undo,
    /// Switch to this theme now (not saved: Settings saves it).
    SetTheme(String),
    /// The question before typing into a shell: open a new shell in the pane's
    /// directory instead (and type there, for names).
    ShellNew(Box<crate::pane::terms::ShellSend>),
    /// The same question: type into that shell after all.
    ShellSendHere(Box<crate::pane::terms::ShellSend>),
}

pub struct Item {
    pub label: String,
    pub hint: String,
    /// `None` on an enabled item means "close the menu", which is what Cancel is.
    pub cmd: Option<Cmd>,
    pub enabled: bool,
    pub separator: bool,
}

impl Item {
    pub fn act(
        label: impl Into<String>,
        hint: impl Into<String>,
        a: Action,
        enabled: bool,
    ) -> Self {
        Self::cmd(label, hint, Cmd::Act(a), enabled)
    }
    pub fn cmd(label: impl Into<String>, hint: impl Into<String>, c: Cmd, enabled: bool) -> Self {
        Self {
            label: label.into(),
            hint: hint.into(),
            cmd: Some(c),
            enabled,
            separator: false,
        }
    }
    pub fn cancel(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: hint.into(),
            cmd: None,
            enabled: true,
            separator: false,
        }
    }
    pub fn sep() -> Self {
        Self {
            label: String::new(),
            hint: String::new(),
            cmd: None,
            enabled: false,
            separator: true,
        }
    }
    fn selectable(&self) -> bool {
        self.enabled && !self.separator
    }
}

pub enum Outcome {
    Stay,
    Close,
    Run(Cmd),
    /// Left or Right in a menu opened from the menu bar: the neighbouring menu.
    Side(isize),
}

pub struct Menu {
    title: Option<String>,
    items: Vec<Item>,
    selected: Option<usize>,
    at: (u16, u16),
    /// Where things landed on the last draw, for hit-testing.
    frame: Rect,
    rows: Vec<(usize, Rect)>,
    /// The first item shown, when there are more than fit.
    top: usize,
    /// Set for a menu opened from the menu bar: which one it is.
    pub bar: Option<usize>,
    /// Opened from the keyboard for this screen row: below it when there is room, else
    /// above it, never over it. A menu opened by the mouse has none.
    pub row: Option<u16>,
}

impl Menu {
    pub fn new(title: Option<String>, items: Vec<Item>, at: (u16, u16)) -> Self {
        let selected = items.iter().position(Item::selectable);
        Self {
            title,
            items,
            selected,
            at,
            frame: Rect::ZERO,
            rows: Vec::new(),
            top: 0,
            bar: None,
            row: None,
        }
    }

    /// A drop-down from the menu bar: no title, Left/Right move to its neighbours.
    pub fn dropdown(bar: usize, items: Vec<Item>, at: (u16, u16)) -> Self {
        Self {
            bar: Some(bar),
            ..Self::new(None, items, at)
        }
    }

    fn step(&mut self, dir: isize) {
        let n = self.items.len() as isize;
        if n == 0 {
            return;
        }
        let mut i = self.selected.map(|s| s as isize).unwrap_or(-dir.signum());
        for _ in 0..n {
            i = (i + dir).rem_euclid(n);
            if self.items[i as usize].selectable() {
                self.selected = Some(i as usize);
                return;
            }
        }
    }

    fn choose(&mut self, i: usize) -> Outcome {
        let Some(item) = self.items.get_mut(i) else {
            return Outcome::Stay;
        };
        if !item.selectable() {
            return Outcome::Stay;
        }
        match item.cmd.take() {
            Some(c) => Outcome::Run(c),
            None => Outcome::Close,
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Outcome {
        match k.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::Left if self.bar.is_some() => Outcome::Side(-1),
            KeyCode::Right if self.bar.is_some() => Outcome::Side(1),
            KeyCode::Up => {
                self.step(-1);
                Outcome::Stay
            }
            KeyCode::Down | KeyCode::Tab => {
                self.step(1);
                Outcome::Stay
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.selected {
                Some(i) => self.choose(i),
                None => Outcome::Close,
            },
            // An item whose hint is a single character is chosen by typing it: the
            // hotlist numbers its entries 1-9.
            KeyCode::Char(c) => {
                let hit = self.items.iter().position(|it| {
                    it.selectable() && it.hint.chars().count() == 1 && it.hint.starts_with(c)
                });
                match hit {
                    Some(i) => self.choose(i),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent) -> Outcome {
        let p = Position::new(m.column, m.row);
        let under = self
            .rows
            .iter()
            .find(|(_, r)| r.contains(p))
            .map(|(i, _)| *i);
        match m.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                if let Some(i) = under.filter(|i| self.items[*i].selectable()) {
                    self.selected = Some(i);
                }
                Outcome::Stay
            }
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right) => {
                if !self.frame.contains(p) {
                    // Clicking away from a menu dismisses it, as everywhere else.
                    return Outcome::Close;
                }
                match under {
                    Some(i) => self.choose(i),
                    None => Outcome::Stay,
                }
            }
            _ => Outcome::Stay,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, screen: Rect) {
        let th = theme::t();
        let label_w = self
            .items
            .iter()
            .map(|i| i.label.chars().count())
            .max()
            .unwrap_or(0);
        let hint_w = self
            .items
            .iter()
            .map(|i| i.hint.chars().count())
            .max()
            .unwrap_or(0);
        let title_w = self.title.as_ref().map_or(0, |t| t.chars().count() + 4);
        let inner_w = (label_w + hint_w + 5).max(title_w).max(22) as u16;
        let w = (inner_w + 2).min(screen.width);
        let h = (self.items.len() as u16 + 2).min(screen.height);

        // Open at the pointer, but pull back inside the screen near its edges. From the
        // keyboard, below its row, or above it where the bottom has no room.
        let x = self.at.0.min(screen.x + screen.width.saturating_sub(w));
        let bottom = screen.y + screen.height;
        let y = match self.row {
            Some(row) if row + 1 + h > bottom && row >= screen.y + h => row - h,
            Some(row) => (row + 1).min(bottom.saturating_sub(h)),
            None => self.at.1.min(bottom.saturating_sub(h)),
        };
        let r = Rect::new(x, y, w, h);
        self.frame = r;

        crate::modal::clear_overlay(frame, r);
        let mut block = crate::decor::bordered()
            .border_style(Style::new().fg(th.accent))
            .style(Style::new().bg(th.bg_panel).fg(th.fg));
        if let Some(t) = &self.title {
            block = block.title(Line::from(Span::styled(
                format!(" {t} "),
                Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
            )));
        }
        let inner = block.inner(r);
        frame.render_widget(block, r);
        crate::decor::notch(frame.buffer_mut(), r);

        // Taller than the screen: scroll so the selected item is always on it.
        let visible = inner.height as usize;
        if let Some(s) = self.selected {
            if s < self.top {
                self.top = s;
            } else if visible > 0 && s >= self.top + visible {
                self.top = s + 1 - visible;
            }
        }
        self.top = self.top.min(self.items.len().saturating_sub(visible));
        self.rows.clear();
        for (i, item) in self.items.iter().enumerate().skip(self.top) {
            let row_y = inner.y + (i - self.top) as u16;
            if row_y >= inner.y + inner.height {
                break;
            }
            let row = Rect::new(inner.x, row_y, inner.width, 1);
            self.rows.push((i, row));
            let buf = frame.buffer_mut();
            if item.separator {
                let line = "\u{2500}".repeat(inner.width as usize);
                buf.set_line(
                    row.x,
                    row.y,
                    &Line::styled(line, Style::new().fg(th.border)),
                    row.width,
                );
                continue;
            }
            let picked = self.selected == Some(i);
            let (fg, bg) = match (picked, item.enabled) {
                (true, _) => (th.bg, th.accent),
                (false, true) => (th.fg, th.bg_panel),
                (false, false) => (th.fg_dim, th.bg_panel),
            };
            buf.set_style(row, Style::new().bg(bg));
            let gap = (inner.width as usize)
                .saturating_sub(item.label.chars().count() + item.hint.chars().count() + 2);
            let line = Line::from(vec![
                Span::styled(format!(" {}", item.label), Style::new().fg(fg)),
                Span::raw(" ".repeat(gap)),
                Span::styled(
                    format!("{} ", item.hint),
                    Style::new().fg(if picked { th.bg } else { th.fg_dim }),
                ),
            ]);
            buf.set_line(row.x, row.y, &line, row.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn menu() -> Menu {
        Menu::new(
            None,
            vec![
                Item::act("View", "F3", Action::View, true),
                Item::sep(),
                Item::act("Copy", "F5", Action::Copy, false),
                Item::act("Delete", "F8", Action::Delete, true),
                Item::cancel("Cancel", "Esc"),
            ],
            (0, 0),
        )
    }

    #[test]
    fn arrows_skip_separators_and_disabled_items() {
        let mut m = menu();
        assert_eq!(m.selected, Some(0));
        m.on_key(key(KeyCode::Down));
        assert_eq!(
            m.selected,
            Some(3),
            "must skip the separator and the disabled Copy"
        );
        m.on_key(key(KeyCode::Down));
        assert_eq!(m.selected, Some(4));
        m.on_key(key(KeyCode::Down));
        assert_eq!(m.selected, Some(0), "wraps around");
        m.on_key(key(KeyCode::Up));
        assert_eq!(m.selected, Some(4));
    }

    #[test]
    fn enter_runs_the_item_and_cancel_closes() {
        let mut m = menu();
        assert!(matches!(
            m.on_key(key(KeyCode::Enter)),
            Outcome::Run(Cmd::Act(Action::View))
        ));

        let mut m = menu();
        m.selected = Some(4);
        assert!(matches!(m.on_key(key(KeyCode::Enter)), Outcome::Close));
    }

    #[test]
    fn a_disabled_item_cannot_be_chosen() {
        let mut m = menu();
        assert!(matches!(m.choose(2), Outcome::Stay));
        assert!(matches!(m.choose(1), Outcome::Stay), "nor a separator");
    }
}
