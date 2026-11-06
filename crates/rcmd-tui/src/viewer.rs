//! F3 — the viewer.
//!
//! Text or hex, over a [`Document`] that keeps the file on disk, so opening a huge log
//! is instant and scrolling to the end of it does not allocate the end of it.

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear};
use ratatui::Frame;

use rcmd_core::document::{Document, Mode, HEX_COLS};
use rcmd_core::format::{display_line, elide_middle, human_size};

use crate::theme;

pub enum Action {
    Stay,
    Close,
    /// Put this on the clipboard: `y` the selection or the top line, `Y` the screen.
    Copy(String),
}

/// What the left button is holding down.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held {
    /// The scrollbar's thumb, at this row of it.
    Bar(u16),
    /// The end of a selection of lines.
    Select,
}

pub struct Viewer {
    pub doc: Document,
    pub mode: Mode,
    /// Line index in text mode, row index in hex mode.
    pub top: usize,
    pub col: usize,
    pub wrap: bool,
    tab_width: usize,
    needle: Option<String>,
    typing: Option<String>,
    message: String,
    page: usize,
    /// Where the text was drawn, and which line each of its rows shows.
    body: Rect,
    row_lines: Vec<usize>,
    bar: Option<crate::scrollbar::Bar>,
    /// Lines chosen with the mouse: where the drag began, where it is now.
    selection: Option<(usize, usize)>,
    held: Option<Held>,
}

impl Viewer {
    pub fn open(doc: Document, wrap: bool, tab_width: usize) -> Self {
        let mode = if doc.binary { Mode::Hex } else { Mode::Text };
        let message = if doc.binary {
            "binary file — showing hex".to_string()
        } else if !doc.fully_indexed {
            "very large file — showing the first part".to_string()
        } else {
            String::new()
        };
        Self {
            doc,
            mode,
            top: 0,
            col: 0,
            wrap,
            tab_width,
            needle: None,
            typing: None,
            message,
            page: 20,
            body: Rect::ZERO,
            row_lines: Vec::new(),
            bar: None,
            selection: None,
            held: None,
        }
    }

    /// The selected lines, first to last.
    fn selected(&self) -> Option<(usize, usize)> {
        self.selection.map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn line_at(&self, y: u16) -> Option<usize> {
        let row = y.checked_sub(self.body.y)?;
        self.row_lines.get(row as usize).copied()
    }

    /// The mouse: the wheel scrolls, the scrollbar drags, and in text a drag selects
    /// whole lines for `y` to copy. Your terminal's own selection (usually Shift+drag)
    /// still works for part of a line.
    pub fn on_mouse(&mut self, m: MouseEvent) {
        let (x, y) = (m.column, m.row);
        match m.kind {
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(b) = self.bar.filter(|b| b.contains(x, y)) {
                    let grab = b.grab(y);
                    self.top = b.offset_at(y, grab);
                    self.held = Some(Held::Bar(grab));
                    return;
                }
                let inside = x >= self.body.x && x < self.body.x + self.body.width;
                match self
                    .line_at(y)
                    .filter(|_| inside && self.mode == Mode::Text)
                {
                    Some(i) => {
                        self.selection = Some((i, i));
                        self.held = Some(Held::Select);
                    }
                    None => self.selection = None,
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.held {
                Some(Held::Bar(grab)) => {
                    if let Some(b) = self.bar {
                        self.top = b.offset_at(y, grab);
                    }
                }
                Some(Held::Select) => {
                    // Past the top or bottom edge, the text scrolls under the pointer.
                    let line = if y < self.body.y {
                        self.scroll(-1);
                        Some(self.top)
                    } else if y >= self.body.y + self.body.height {
                        self.scroll(1);
                        self.row_lines.last().map(|l| l + 1)
                    } else {
                        self.line_at(y)
                    };
                    let last = self.doc.line_count().saturating_sub(1);
                    if let (Some(l), Some(sel)) = (line, self.selection.as_mut()) {
                        sel.1 = l.min(last);
                    }
                }
                None => {}
            },
            MouseEventKind::Up(MouseButton::Left) => {
                let held = self.held.take();
                if let (Some(Held::Select), Some((a, b))) = (held, self.selected()) {
                    let n = b - a + 1;
                    self.message = format!(
                        "{n} line{} selected \u{2014} y copies, Esc clears",
                        if n == 1 { "" } else { "s" }
                    );
                }
            }
            _ => {}
        }
    }

    fn last(&self) -> usize {
        match self.mode {
            Mode::Text => self.doc.line_count().saturating_sub(1),
            Mode::Hex => self.doc.hex_rows().saturating_sub(1),
        }
    }

    /// Say something on the viewer's own message line, which is what shows while it
    /// covers the screen.
    pub fn note(&mut self, text: &str) {
        self.message = text.to_string();
    }

    fn scroll(&mut self, delta: isize) {
        let t = (self.top as isize + delta).clamp(0, self.last() as isize);
        self.top = t as usize;
    }

    fn search(&mut self, backwards: bool) {
        let Some(n) = self.needle.clone() else {
            self.message = "nothing to search for yet — press /".into();
            return;
        };
        match self.doc.find(&n, self.top, backwards) {
            Some(i) => {
                self.top = i;
                self.message = format!("/{n}");
            }
            None if !self.doc.fully_indexed => {
                // "no more matches" would be a lie: we never looked at the rest.
                self.message = format!(
                    "/{n} — not in the first {} lines, and this file is too large to index further",
                    self.doc.line_count()
                )
            }
            None => self.message = format!("/{n} — no more matches"),
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Action {
        if let Some(buf) = self.typing.as_mut() {
            match k.code {
                KeyCode::Char(c) => buf.push(c),
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let n = self.typing.take().unwrap_or_default();
                    if n.is_empty() {
                        self.message.clear();
                    } else {
                        self.needle = Some(n);
                        // Search from just before the current line so a match on the
                        // line you are already looking at still counts.
                        self.top = self.top.saturating_sub(1);
                        self.search(false);
                    }
                }
                KeyCode::Esc => {
                    self.typing = None;
                    self.message.clear();
                }
                _ => {}
            }
            return Action::Stay;
        }

        let page = self.page.max(1) as isize;
        match k.code {
            KeyCode::Esc if self.selection.is_some() => {
                self.selection = None;
                self.message.clear();
            }
            KeyCode::Esc | KeyCode::F(3) | KeyCode::F(10) | KeyCode::Char('q') => {
                return Action::Close
            }
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll(page),
            KeyCode::PageUp => self.scroll(-page),
            KeyCode::Home | KeyCode::Char('g') => self.top = 0,
            KeyCode::End | KeyCode::Char('G') => self.top = self.last(),
            KeyCode::Char('h') => {
                self.mode = match self.mode {
                    Mode::Text => Mode::Hex,
                    Mode::Hex => Mode::Text,
                };
                if self.mode == Mode::Text {
                    if let Err(e) = self.doc.ensure_lines() {
                        self.mode = Mode::Hex;
                        self.message = format!("cannot show as text: {e}");
                        return Action::Stay;
                    }
                }
                // The two modes count in different units, so a shared `top` would jump
                // somewhere arbitrary.
                self.top = 0;
                self.col = 0;
                self.selection = None;
                self.message = match self.mode {
                    Mode::Hex => "hex".into(),
                    Mode::Text => "text".into(),
                };
            }
            KeyCode::Char('y' | 'Y') if self.mode == Mode::Hex => {
                self.message = "copy works in text view \u{2014} h".into();
            }
            KeyCode::Char(c @ ('y' | 'Y')) => {
                let (from, n) = match (c, self.selected()) {
                    ('y', Some((a, b))) => (a, b - a + 1),
                    ('y', None) => (self.top, 1),
                    _ => (self.top, self.page.max(1)),
                };
                let text = self.doc.lines_from(from, n).join("\n");
                return Action::Copy(text);
            }
            KeyCode::Char('w') => {
                self.wrap = !self.wrap;
                self.col = 0;
                self.message = if self.wrap { "wrap on" } else { "wrap off" }.into();
            }
            KeyCode::Left => self.col = self.col.saturating_sub(8),
            KeyCode::Right if !self.wrap => self.col += 8,
            KeyCode::Char('/') => {
                self.typing = Some(String::new());
                self.message.clear();
            }
            KeyCode::Char('n') => self.search(false),
            KeyCode::Char('N') => self.search(true),
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Close
            }
            _ => {}
        }
        Action::Stay
    }

    // ── rendering ──────────────────────────────────────────────────────────────────

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);

        let kind = match self.mode {
            Mode::Text => "text",
            Mode::Hex => "hex",
        };
        let scale = match self.mode {
            Mode::Text => format!("{} lines", self.doc.line_count()),
            Mode::Hex => human_size(self.doc.len),
        };
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "view ",
                Style::new()
                    .fg(theme::t().accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                elide_middle(
                    &self.doc.path.display().to_string(),
                    area.width.saturating_sub(34) as usize,
                ),
                Style::new().fg(theme::t().fg),
            ),
            Span::styled(
                format!("  \u{b7} {kind} \u{b7} {scale} "),
                Style::new().fg(theme::t().fg_dim),
            ),
        ]);
        let block = Block::bordered()
            .border_style(Style::new().fg(theme::t().border_active))
            .style(Style::new().bg(theme::t().bg))
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height < 2 || inner.width < 8 {
            return;
        }

        let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
        self.page = body.height as usize;
        let total = match self.mode {
            Mode::Text => self.doc.line_count(),
            Mode::Hex => self.doc.hex_rows(),
        };
        self.body = body;
        self.row_lines.clear();
        self.bar = crate::scrollbar::draw(
            frame.buffer_mut(),
            crate::scrollbar::track(area, body),
            total,
            self.top,
            Style::new().fg(theme::t().accent),
        );
        match self.mode {
            Mode::Text => self.render_text(frame, body),
            Mode::Hex => self.render_hex(frame, body),
        }
        self.render_hints(
            frame,
            Rect::new(inner.x, inner.y + body.height, inner.width, 1),
        );
    }

    fn render_text(&mut self, frame: &mut Frame, area: Rect) {
        let total = self.doc.line_count();
        let gutter = (total.max(1).to_string().len() as u16).max(3) + 1;
        let text_w = area.width.saturating_sub(gutter + 1) as usize;
        if text_w == 0 {
            return;
        }

        let selected = self.selected();
        let mut row = 0u16;
        let mut i = self.top;
        while row < area.height && i < total {
            let raw = self.doc.line(i).unwrap_or_default();
            let shown = display_line(&raw, self.tab_width);
            let chars: Vec<char> = shown.chars().collect();

            let pieces: Vec<String> = if self.wrap {
                if chars.is_empty() {
                    vec![String::new()]
                } else {
                    chars.chunks(text_w).map(|c| c.iter().collect()).collect()
                }
            } else {
                vec![chars.iter().skip(self.col).take(text_w).collect::<String>()]
            };

            for (n, piece) in pieces.iter().enumerate() {
                if row >= area.height {
                    break;
                }
                let y = area.y + row;
                let num = if n == 0 {
                    format!("{:>w$} ", i + 1, w = (gutter - 1) as usize)
                } else {
                    " ".repeat(gutter as usize)
                };
                let mut spans = vec![Span::styled(num, Style::new().fg(theme::t().fg_dim))];
                spans.push(Span::styled("\u{2502}", Style::new().fg(theme::t().border)));
                spans.extend(self.highlight(piece));
                frame
                    .buffer_mut()
                    .set_line(area.x, y, &Line::from(spans), area.width);
                if selected.is_some_and(|(a, b)| i >= a && i <= b) {
                    let text = Rect::new(area.x + gutter + 1, y, area.width - gutter - 1, 1);
                    frame
                        .buffer_mut()
                        .set_style(text, Style::new().bg(theme::t().row_cursor_active));
                }
                self.row_lines.push(i);
                row += 1;
            }
            i += 1;
        }
    }

    /// Colour the search term where it appears, case-insensitively.
    fn highlight(&self, text: &str) -> Vec<Span<'static>> {
        let plain = Style::new().fg(theme::t().fg);
        let Some(needle) = self.needle.as_ref().filter(|n| !n.is_empty()) else {
            return vec![Span::styled(text.to_string(), plain)];
        };
        let hay = text.to_lowercase();
        let pin = needle.to_lowercase();
        let hit = Style::new()
            .fg(theme::t().bg)
            .bg(theme::t().mark)
            .add_modifier(Modifier::BOLD);

        let mut spans = Vec::new();
        let mut at = 0usize;
        while let Some(rel) = hay[at..].find(&pin) {
            let s = at + rel;
            let e = s + pin.len();
            if s > at {
                spans.push(Span::styled(text[at..s].to_string(), plain));
            }
            spans.push(Span::styled(text[s..e].to_string(), hit));
            at = e;
        }
        if at < text.len() {
            spans.push(Span::styled(text[at..].to_string(), plain));
        }
        spans
    }

    fn render_hex(&mut self, frame: &mut Frame, area: Rect) {
        let rows = area.height as u64;
        let start = self.top as u64 * HEX_COLS;
        let data = self.doc.bytes_at(start, (rows * HEX_COLS) as usize);

        for (r, chunk) in data.chunks(HEX_COLS as usize).enumerate() {
            let y = area.y + r as u16;
            if y >= area.y + area.height {
                break;
            }
            let offset = start + (r as u64 * HEX_COLS);

            let mut hex = String::with_capacity(50);
            for (i, b) in chunk.iter().enumerate() {
                if i == 8 {
                    hex.push(' ');
                }
                hex.push_str(&format!("{b:02x} "));
            }
            // Padded to the full width so the closing bar stays in one column on the
            // last, short row — a ragged right edge reads as a rendering bug.
            let ascii: String = chunk
                .iter()
                .map(|b| {
                    if (0x20..0x7f).contains(b) {
                        *b as char
                    } else {
                        '\u{b7}'
                    }
                })
                .chain(std::iter::repeat_n(' ', HEX_COLS as usize - chunk.len()))
                .collect();

            let spans = vec![
                Span::styled(
                    format!("{offset:08x}  "),
                    Style::new().fg(theme::t().fg_dim),
                ),
                Span::styled(format!("{hex:<50}"), Style::new().fg(theme::t().fg)),
                Span::styled("\u{2502}", Style::new().fg(theme::t().border)),
                Span::styled(ascii, Style::new().fg(theme::t().dir)),
                Span::styled("\u{2502}", Style::new().fg(theme::t().border)),
            ];
            frame
                .buffer_mut()
                .set_line(area.x, y, &Line::from(spans), area.width);
        }
    }

    fn render_hints(&self, frame: &mut Frame, area: Rect) {
        frame
            .buffer_mut()
            .set_style(area, Style::new().bg(theme::t().bg_bar));

        if let Some(buf) = &self.typing {
            let line = Line::from(vec![
                Span::styled(
                    " /",
                    Style::new()
                        .fg(theme::t().accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{buf}\u{2588}"),
                    Style::new().fg(theme::t().fg).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame
                .buffer_mut()
                .set_line(area.x, area.y, &line, area.width);
            return;
        }

        let keys: &[(&str, &str)] = &[
            ("Esc", "close"),
            ("h", "hex"),
            ("w", "wrap"),
            ("y/Y", "copy"),
            ("/", "search"),
            ("n/N", "next"),
            ("g/G", "ends"),
        ];
        let mut spans = Vec::new();
        for (k, label) in keys {
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
        if !self.message.is_empty() {
            spans.push(Span::styled(
                format!("  {}", self.message),
                Style::new().fg(theme::t().mark),
            ));
        }
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &Line::from(spans), area.width);
    }
}
