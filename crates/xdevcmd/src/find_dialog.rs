//! Alt+F7: what to look for. Two lines — names and, optionally, text inside — and Tab
//! between them. The search itself runs in `xdevcmd_core::find` and its results open in a
//! new tab of the pane.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::find::Query;

use crate::line_edit::LineEdit;
use crate::modal::{buttons, centered, frame_box, put};
use crate::theme;

pub struct FindDialog {
    pub root: PathBuf,
    pub names: LineEdit,
    pub text: LineEdit,
    /// 0: names, 1: text.
    pub field: usize,
    pub hidden: bool,
    pub buttons: Vec<Rect>,
}

pub enum Outcome {
    Stay,
    Close,
    Start,
}

impl FindDialog {
    pub fn new(root: PathBuf, hidden: bool) -> Self {
        Self {
            root,
            // Selected, so typing a pattern replaces it and Enter alone lists everything.
            names: LineEdit::for_rename("*", true),
            text: LineEdit::new(""),
            field: 0,
            hidden,
            buttons: Vec::new(),
        }
    }

    pub fn query(&self) -> Query {
        Query {
            root: self.root.clone(),
            names: self.names.text(),
            text: self.text.text(),
            hidden: self.hidden,
        }
    }

    /// How the results tab describes this search.
    pub fn what(&self) -> String {
        let names = self.names.text();
        let names = if names.trim().is_empty() {
            "*".into()
        } else {
            names
        };
        match self.text.text() {
            t if t.is_empty() => names,
            t => format!("{names} containing \"{t}\""),
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Outcome {
        match k.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::Enter => Outcome::Start,
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                self.field = 1 - self.field;
                Outcome::Stay
            }
            _ => {
                let e = if self.field == 0 {
                    &mut self.names
                } else {
                    &mut self.text
                };
                e.on_key(k);
                Outcome::Stay
            }
        }
    }
}

pub(crate) fn field_line(edit: &LineEdit, focused: bool, width: usize) -> Line<'static> {
    let th = theme::t();
    let chars = edit.chars();
    let cur = edit.cursor();
    let sel = edit.selection();
    let start = if cur >= width { cur + 1 - width } else { 0 };
    let mut spans = Vec::new();
    for idx in start..(start + width) {
        let at_cursor = focused && idx == cur && sel.is_none();
        let Some(c) = chars.get(idx).copied().or(at_cursor.then_some(' ')) else {
            break;
        };
        let style = if at_cursor {
            Style::new().fg(th.bg).bg(th.fg)
        } else if focused && sel.is_some_and(|(s, e)| idx >= s && idx < e) {
            Style::new().fg(th.bg).bg(th.accent)
        } else if focused {
            Style::new().fg(th.fg).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(th.fg_dim)
        };
        spans.push(Span::styled(c.to_string(), style));
    }
    Line::from(spans)
}

pub fn render(frame: &mut Frame, area: Rect, d: &mut FindDialog) {
    let th = theme::t();
    let r = centered(area, 64, 9);
    let inner = frame_box(frame, r, "Find files", th.accent);
    put(
        frame,
        inner,
        0,
        &format!("in {}", d.root.display()),
        Style::new().fg(th.fg_dim),
    );
    let w = inner.width as usize;
    let rows = [
        ("names  (*.rs *.toml, empty: all)", &d.names),
        ("containing  (empty: any file)", &d.text),
    ];
    for (i, (label, edit)) in rows.iter().enumerate() {
        let y = 1 + i as u16 * 2;
        let style = if d.field == i {
            Style::new().fg(th.accent)
        } else {
            Style::new().fg(th.fg_dim)
        };
        put(frame, inner, y, label, style);
        if inner.height > y + 1 {
            frame.buffer_mut().set_line(
                inner.x,
                inner.y + y + 1,
                &field_line(edit, d.field == i, w),
                inner.width,
            );
        }
    }
    let y = inner.y + inner.height.saturating_sub(1);
    d.buttons = buttons(
        frame,
        inner.x,
        y,
        inner.width,
        &[
            ("Enter", "find", th.accent),
            ("Tab", "next field", th.accent),
            ("Esc", "cancel", th.accent),
        ],
    );
}
