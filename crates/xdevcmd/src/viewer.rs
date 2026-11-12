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
use ratatui::widgets::Clear;
use ratatui::Frame;

use xdevcmd_core::document::{Document, Mode, HEX_COLS};
use xdevcmd_core::format::{display_line, elide_middle, human_size};
use xdevcmd_core::syntax::{self, Highlighter, Span as Piece};

use crate::theme;

pub enum Action {
    Stay,
    Close,
    /// Put this on the clipboard: `y` the selection or the top line, `Y` the screen.
    Copy(String),
    /// F2: write what is shown to a file the user names (the app asks where).
    SaveAs,
    /// A path chosen in a shell's history: go there, view it or edit it.
    Link(LinkAct, crate::links::Link),
    /// `p` (a new list) or `P` (add to one) in a shell's history: these paths.
    MakeList(ListRequest, bool),
}

/// Paths taken from a shell's history for a list: resolved, not yet looked up.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRequest {
    pub paths: Vec<std::path::PathBuf>,
    /// Each line's candidates, for the list's first check to choose between: a line
    /// that names a file whole is that file (B03).
    pub groups: Vec<xdevcmd_core::find::Group>,
    /// "sh · tab 1 · rg -l foo", or "… · lines 12–40".
    pub source: String,
    /// The directory the first line's relative paths were taken from.
    pub base: Option<std::path::PathBuf>,
}

impl ListRequest {
    /// How many paths it offers, before any is looked up: the paths found in the
    /// lines. A line's whole is one choice with them, decided when the list is
    /// checked; only when no line holds a path found by the rules is each line that
    /// may name something whole counted, as one.
    pub fn count(&self) -> usize {
        let wholes: std::collections::HashSet<&std::path::PathBuf> =
            self.groups.iter().flat_map(|(w, _)| w).collect();
        let mut seen = std::collections::HashSet::new();
        let parts = self
            .paths
            .iter()
            .filter(|p| !wholes.contains(p) && seen.insert(*p))
            .count();
        if parts > 0 {
            parts
        } else {
            self.groups.len()
        }
    }
}

/// What to do with a path chosen in a shell's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAct {
    /// Enter: the file pane goes there.
    Go,
    /// F3: view the file at its line.
    View,
    /// F4: edit the file at its line.
    Edit,
}

/// What is being typed on the viewer's last row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prompt {
    Search,
    /// `:` or Ctrl+G: a line, `50%`, or in hex an offset.
    Line,
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
    typing: Option<(Prompt, String)>,
    message: String,
    page: usize,
    /// Where the text was drawn, and which line each of its rows shows.
    body: Rect,
    row_lines: Vec<usize>,
    bar: Option<crate::scrollbar::Bar>,
    /// Lines chosen with the mouse: where the drag began, where it is now.
    selection: Option<(usize, usize)>,
    held: Option<Held>,
    /// Show the last screenful at the next draw, once the screen's height is known: a
    /// shell's history opens where the shell was, at its end.
    to_end: bool,
    /// A shell's history: the paths in it that exist, and the one chosen.
    pub links: Option<crate::links::Links>,
    /// Where the text starts on screen, and the character each row starts at.
    text_x: u16,
    row_starts: Vec<usize>,
    /// A press on a path, and where: its release on the same cell is a click on it.
    pressed_link: Option<((usize, usize), (u16, u16))>,
    /// The chosen path's description in its parts, while the message is still it: drawn
    /// to fit, the path cut in its middle.
    said: Option<crate::links::Said>,
    /// Colours for the file's language, when it has one and colouring is on.
    syntax: Option<Highlighter>,
    /// The last frame drew a line plain because the scan had not reached it, and how
    /// far the scan was then: when it moves on, the frame is drawn again.
    syntax_waiting: Option<(usize, bool)>,
}

/// Files up to this size are coloured in F3; bigger ones are drawn plain.
pub const COLOUR_MAX: u64 = 64 << 20;
/// Quick view colours files up to this size, without a background scan.
pub const QUICK_COLOUR_MAX: u64 = 1 << 20;

/// Whether the viewer colours source: the setting, unless `NO_COLOR` says no colour.
pub fn colours_on(setting: bool) -> bool {
    setting && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
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
            to_end: false,
            links: None,
            text_x: 0,
            row_starts: Vec::new(),
            pressed_link: None,
            said: None,
            syntax: None,
            syntax_waiting: None,
        }
    }

    /// Colour the file by its language when `on` ([`colours_on`]), it is text on disk,
    /// and at most `max` bytes. `scan`: work out the state through the whole file in
    /// the background, so a jump anywhere is coloured (F3); without it, only lines near
    /// the top are (quick view). A document held in memory (a shell's history) is never
    /// coloured.
    pub fn with_syntax(mut self, on: bool, max: u64, scan: bool) -> Self {
        if !on || self.doc.binary || self.doc.in_memory {
            return self;
        }
        let name = self
            .doc
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let first = self.doc.line(0).unwrap_or_default();
        let Some(lang) = syntax::detect(&name, &first) else {
            return self;
        };
        if self.doc.len > max {
            if self.message.is_empty() {
                self.message = format!(
                    "{} file over {} \u{2014} not coloured",
                    lang.name(),
                    human_size(max)
                );
            }
            return self;
        }
        let mut h = Highlighter::new(lang);
        if scan && self.doc.line_count() > syntax::scan::EVERY {
            h.scan_file(&self.doc.path.clone(), self.doc.line_count());
        }
        self.syntax = Some(h);
        self
    }

    /// The language the text is coloured as, if it is.
    pub fn language(&self) -> Option<&'static str> {
        self.syntax.as_ref().map(|h| h.lang().name())
    }

    /// True when lines were drawn plain for want of the scan and the scan has moved on
    /// since: time to draw again.
    pub fn syntax_moved(&self) -> bool {
        match (&self.syntax_waiting, &self.syntax) {
            (Some(seen), Some(h)) => h.progress() != *seen,
            _ => false,
        }
    }

    /// Lines on screen are waiting for the scan.
    pub fn syntax_waiting(&self) -> bool {
        self.syntax_waiting.is_some() && self.syntax.as_ref().is_some_and(|h| !h.progress().1)
    }

    /// What the message line says now.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The paths in a shell's history, offered by Tab.
    pub fn with_links(mut self, links: crate::links::Links) -> Self {
        self.links = Some(links);
        self
    }

    /// Put line `n` (from 1) at the top, as F3 from a history does for a path's line.
    pub fn show_line(&mut self, n: u32) {
        self.top = (n as usize).saturating_sub(1).min(self.last());
        self.message = format!("line {n}");
    }

    /// Line `i` as it is drawn: tabs spread out, control characters made visible.
    fn shown(&mut self, i: usize) -> String {
        display_line(&self.doc.line(i).unwrap_or_default(), self.tab_width)
    }

    /// Tab and Shift+Tab: the next path that exists, after (or before) the chosen one,
    /// or from the top of the screen. Lines are looked at as needed, a few screens at a
    /// time, so a long history costs nothing until Tab goes through it.
    fn next_link(&mut self, forward: bool) {
        const LOOK: usize = 2000;
        let total = self.doc.line_count();
        if total == 0 || self.links.is_none() {
            return;
        }
        let (mut i, mut k) = match self.links.as_ref().and_then(|l| l.selected) {
            Some((i, k)) => (i, Some(k)),
            None if forward => (self.top, None),
            None => ((self.top + self.page).min(total - 1), None),
        };
        for _ in 0..LOOK {
            let shown = self.shown(i);
            let Some(links) = self.links.as_mut() else {
                return;
            };
            let n = links.on_line(i, &shown).len();
            let next = match (k, forward) {
                (None, true) if n > 0 => Some(0),
                (None, false) if n > 0 => Some(n - 1),
                (Some(j), true) if j + 1 < n => Some(j + 1),
                (Some(j), false) if j > 0 => Some(j - 1),
                _ => None,
            };
            if let Some(j) = next {
                links.selected = Some((i, j));
                let said = links.describe_chosen();
                self.note_path(said);
                // Into view, a few lines from the edge.
                let page = self.page.max(1);
                if i < self.top || i >= self.top + page {
                    self.top = i.saturating_sub(page / 3);
                }
                return;
            }
            k = None;
            match forward {
                true if i + 1 < total => i += 1,
                false if i > 0 => i -= 1,
                _ => break,
            }
        }
        self.message = if forward {
            "no more paths below \u{2014} Shift+Tab goes back"
        } else {
            "no more paths above"
        }
        .into();
    }

    /// `p` and `P`: the paths in the selected lines; with none selected, in the output
    /// of the command the chosen path is in, else of the last command (prompt marks
    /// say where each begins). Without either, nothing is guessed.
    pub fn list_request(&mut self, add: bool) -> Action {
        let total = self.doc.line_count();
        let Some(links) = self.links.as_ref() else {
            return Action::Stay;
        };
        if let Some(p) = &links.remote {
            self.message = format!(
                "{p} is running in this shell: its output names another machine's files \u{2014} no list made"
            );
            return Action::Stay;
        }
        let picked = match self.selected() {
            Some((a, b)) => Some((a..b + 1, None, format!("lines {}\u{2013}{}", a + 1, b + 1))),
            None => links
                .selected
                .and_then(|(i, _)| links.command_at(i, total))
                .or_else(|| links.last_command(total))
                .map(|(r, typed)| (r, typed, String::new())),
        };
        let Some((lines, typed, mut what)) = picked else {
            self.message =
                "drag over the lines to take paths from (no prompt marks here: docs/shell.md)"
                    .into();
            return Action::Stay;
        };
        if let Some(t) = typed {
            what = command_text(&self.shown(t));
        }
        let mut paths = Vec::new();
        let mut groups = Vec::new();
        for i in lines.clone() {
            let shown = self.shown(i);
            if let Some(l) = self.links.as_ref() {
                let (wholes, parts) = l.line_paths(i, &shown);
                paths.extend(wholes.iter().cloned());
                paths.extend(parts.iter().cloned());
                if !wholes.is_empty() {
                    groups.push((wholes, parts));
                }
            }
        }
        if paths.is_empty() {
            self.message = "no paths in those lines".into();
            return Action::Stay;
        }
        let links = self.links.as_ref().expect("checked above");
        let source = [links.label.as_str(), what.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" \u{b7} ");
        let base = links.base_dir(lines.start);
        Action::MakeList(
            ListRequest {
                paths,
                groups,
                source,
                base,
            },
            add,
        )
    }

    /// The link under (`x`, `y`), as (line, which of its links).
    fn link_at(&mut self, x: u16, y: u16) -> Option<(usize, usize)> {
        let row = usize::from(y.checked_sub(self.body.y)?);
        let line = *self.row_lines.get(row)?;
        let at = self.row_starts.get(row)? + usize::from(x.checked_sub(self.text_x)?);
        let shown = self.shown(line);
        let links = self.links.as_mut()?;
        let k = links
            .on_line(line, &shown)
            .iter()
            .position(|l| l.chars.contains(&at))?;
        Some((line, k))
    }

    /// Open at the end: the last line on the last row, as the shell showed it.
    pub fn show_end(&mut self) {
        self.to_end = true;
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
    pub fn on_mouse(&mut self, m: MouseEvent) -> Action {
        let (x, y) = (m.column, m.row);
        // A path in a shell's history: a click chooses it, a click on the chosen one
        // goes there. A click, not a press: a drag that starts on a path selects lines.
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) if self.mode == Mode::Text => {
                self.pressed_link = self.link_at(x, y).map(|hit| (hit, (x, y)));
            }
            MouseEventKind::Drag(_) => {
                if self.pressed_link.is_some_and(|(_, at)| at != (x, y)) {
                    self.pressed_link = None;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((hit, _)) = self.pressed_link.take() {
                    self.held = None;
                    self.selection = None;
                    let links = self.links.as_mut().expect("a link was hit");
                    if links.selected == Some(hit) {
                        if let Some(l) = links.chosen().cloned() {
                            return Action::Link(LinkAct::Go, l);
                        }
                    }
                    links.selected = Some(hit);
                    let said = links.describe_chosen();
                    self.note_path(said);
                    return Action::Stay;
                }
            }
            _ => {}
        }
        match m.kind {
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(b) = self.bar.filter(|b| b.contains(x, y)) {
                    let grab = b.grab(y);
                    self.top = b.offset_at(y, grab);
                    self.held = Some(Held::Bar(grab));
                    return Action::Stay;
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
        Action::Stay
    }

    fn last(&self) -> usize {
        match self.mode {
            Mode::Text => self.doc.line_count().saturating_sub(1),
            Mode::Hex => self.doc.hex_rows().saturating_sub(1),
        }
    }

    /// Say something that names a path: drawn to fit, the path cut in its middle so
    /// its name and what follows stay on the line.
    pub fn note_path(&mut self, said: crate::links::Said) {
        self.message = said.text();
        self.said = Some(said);
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

    /// `:` / Ctrl+G, then Enter: go to what was typed. In text, a line number (`40`) or a
    /// share of the document (`50%`) puts that line at the top; in hex, a byte offset,
    /// `0x1f00` or decimal, or a share. Past the end goes to the end and says so.
    fn go_to(&mut self, typed: &str) {
        let t = typed.trim();
        let last = self.last();
        let rows = last + 1;
        let unit = match self.mode {
            Mode::Text => "a line",
            Mode::Hex => "an offset",
        };
        let target: Option<u64> = if let Some(p) = t.strip_suffix('%') {
            number(p.trim(), 10).map(|p| (rows as u64 * p.min(100) / 100).saturating_sub(1))
        } else if self.mode == Mode::Hex {
            let (digits, radix) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(h) => (h, 16),
                None => (t, 10),
            };
            number(digits, radix).map(|off| off / HEX_COLS)
        } else {
            number(t, 10).map(|n| n.saturating_sub(1))
        };
        let Some(row) = target else {
            self.message = format!("not {unit}: {t} \u{2014} a number, or 50%");
            return;
        };
        self.selection = None;
        if row > last as u64 {
            self.top = last;
            self.message = match self.mode {
                Mode::Text => format!("{t} is past the end \u{2014} line {rows} is the last"),
                Mode::Hex => format!(
                    "{t} is past the end \u{2014} the file is {} bytes",
                    self.doc.len
                ),
            };
        } else {
            self.top = row as usize;
            self.message.clear();
            // Lines count from 1; 0 goes to the first and says why it is not line 0.
            if self.mode == Mode::Text && number(t, 10) == Some(0) {
                self.message = "lines start at 1 \u{2014} this is line 1".into();
            }
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Action {
        if let Some((prompt, buf)) = self.typing.as_mut() {
            let prompt = *prompt;
            match k.code {
                KeyCode::Char(c) => buf.push(c),
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter if prompt == Prompt::Line => {
                    let typed = self.typing.take().map(|(_, b)| b).unwrap_or_default();
                    if typed.trim().is_empty() {
                        self.message.clear();
                    } else {
                        self.go_to(&typed);
                    }
                }
                KeyCode::Enter => {
                    let n = self.typing.take().map(|(_, b)| b).unwrap_or_default();
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
        // A shell's history: Tab goes through its paths, and the chosen one is acted on.
        if self.links.is_some() && self.mode == Mode::Text {
            let chosen = self.links.as_ref().and_then(|l| l.chosen().cloned());
            match (k.code, chosen) {
                (KeyCode::Tab, _) => {
                    self.next_link(true);
                    return Action::Stay;
                }
                (KeyCode::BackTab, _) => {
                    self.next_link(false);
                    return Action::Stay;
                }
                (KeyCode::Esc, Some(_)) => {
                    if let Some(l) = self.links.as_mut() {
                        l.selected = None;
                    }
                    self.message.clear();
                    return Action::Stay;
                }
                (KeyCode::Enter, Some(l)) => return Action::Link(LinkAct::Go, l),
                (KeyCode::F(3 | 4), Some(l)) if l.dir => {
                    self.message = "a directory \u{2014} Enter goes there".into();
                    return Action::Stay;
                }
                (KeyCode::F(3), Some(l)) => return Action::Link(LinkAct::View, l),
                (KeyCode::F(4), Some(l)) => return Action::Link(LinkAct::Edit, l),
                (KeyCode::Enter, None) => {
                    self.message = "Tab chooses a path in the output".into();
                    return Action::Stay;
                }
                (KeyCode::Char('p'), _) => return self.list_request(false),
                (KeyCode::Char('P'), _) => return self.list_request(true),
                _ => {}
            }
        }
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
            KeyCode::F(2) => return Action::SaveAs,
            KeyCode::Char(':') => self.typing = Some((Prompt::Line, String::new())),
            // Ctrl+G before `g`: the arm below matches 'g' whatever the modifiers, and
            // Ctrl+G (go to, as in the panes) must not be taken as Home.
            KeyCode::Char('g') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.typing = Some((Prompt::Line, String::new()));
            }
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
                self.typing = Some((Prompt::Search, String::new()));
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
        let scale = match (self.mode, self.language()) {
            (Mode::Text, Some(lang)) => format!("{lang} \u{b7} {} lines", self.doc.line_count()),
            (Mode::Text, None) => format!("{} lines", self.doc.line_count()),
            (Mode::Hex, _) => human_size(self.doc.len),
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
        let block = crate::decor::bordered()
            .border_style(Style::new().fg(theme::t().border_active))
            .style(Style::new().bg(theme::t().bg))
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        crate::decor::notch(frame.buffer_mut(), area);
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
        if std::mem::take(&mut self.to_end) {
            self.top = total.saturating_sub(body.height as usize);
        }
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
        self.text_x = area.x + gutter + 1;
        self.row_starts.clear();
        let mut waiting = false;
        let mut row = 0u16;
        let mut i = self.top;
        while row < area.height && i < total {
            let raw = self.doc.line(i).unwrap_or_default();
            let shown = display_line(&raw, self.tab_width);
            let chars: Vec<char> = shown.chars().collect();
            let pieces_of = match self.syntax.as_mut() {
                Some(h) => {
                    let got = h.spans_for(&mut self.doc, i, &raw);
                    waiting |= got.is_none();
                    got.unwrap_or_default()
                }
                None => Vec::new(),
            };
            let styles = char_styles(&raw, &pieces_of, self.tab_width, chars.len());
            let hits = self.hits(&chars);
            let chosen = self.links.as_ref().and_then(|l| l.selected);
            let links: Vec<(std::ops::Range<usize>, bool)> = match self.links.as_mut() {
                Some(l) => l
                    .on_line(i, &shown)
                    .iter()
                    .enumerate()
                    .map(|(k, link)| (link.chars.clone(), chosen == Some((i, k))))
                    .collect(),
                None => Vec::new(),
            };

            // Each row's piece of the line, as the character it starts at and how many.
            let pieces: Vec<(usize, usize)> = if self.wrap {
                if chars.is_empty() {
                    vec![(0, 0)]
                } else {
                    (0..chars.len())
                        .step_by(text_w)
                        .map(|s| (s, text_w.min(chars.len() - s)))
                        .collect()
                }
            } else {
                let s = self.col.min(chars.len());
                vec![(s, text_w.min(chars.len() - s))]
            };

            for (n, &(from, len)) in pieces.iter().enumerate() {
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
                spans.extend(self.highlight(
                    &chars[from..from + len],
                    &styles[from..from + len],
                    &hits[from..from + len],
                ));
                frame
                    .buffer_mut()
                    .set_line(area.x, y, &Line::from(spans), area.width);
                if selected.is_some_and(|(a, b)| i >= a && i <= b) {
                    // The selection's background, except under the search term's hits,
                    // which stay on top.
                    let x0 = area.x + gutter + 1;
                    let w = area.width - gutter - 1;
                    let buf = frame.buffer_mut();
                    for dx in 0..w {
                        if hits.get(from + usize::from(dx)).copied().unwrap_or(false)
                            && usize::from(dx) < len
                        {
                            continue;
                        }
                        buf.set_style(
                            Rect::new(x0 + dx, y, 1, 1),
                            Style::new().bg(theme::t().row_cursor_active),
                        );
                    }
                }
                // The paths on this piece of the line: underlined, the chosen one lit.
                let start = if self.wrap { n * text_w } else { self.col };
                for (span, chosen) in &links {
                    let from = span.start.max(start);
                    let to = span.end.min(start + len);
                    if from >= to {
                        continue;
                    }
                    let style = if *chosen {
                        Style::new()
                            .fg(theme::t().bg)
                            .bg(theme::t().accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::new()
                            .fg(theme::t().accent)
                            .add_modifier(Modifier::UNDERLINED)
                    };
                    let x = self.text_x + (from - start) as u16;
                    frame
                        .buffer_mut()
                        .set_style(Rect::new(x, y, (to - from) as u16, 1), style);
                }
                self.row_starts.push(start);
                self.row_lines.push(i);
                row += 1;
            }
            i += 1;
        }
        self.syntax_waiting = match (&self.syntax, waiting) {
            (Some(h), true) => Some(h.progress()),
            _ => None,
        };
    }

    /// Which characters of a line are the search term, case-insensitively.
    fn hits(&self, chars: &[char]) -> Vec<bool> {
        let mut hits = vec![false; chars.len()];
        if let Some(needle) = self.needle.as_ref().filter(|n| !n.is_empty()) {
            let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
            let pin: Vec<char> = needle.chars().map(fold).collect();
            let hay: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
            let mut at = 0;
            while at + pin.len() <= hay.len() {
                if hay[at..at + pin.len()] == pin[..] {
                    hits[at..at + pin.len()].fill(true);
                    at += pin.len();
                } else {
                    at += 1;
                }
            }
        }
        hits
    }

    /// A row's characters in their colours (the language's, else plain), with the
    /// search term's hits on top.
    fn highlight(&self, chars: &[char], styles: &[Style], hits: &[bool]) -> Vec<Span<'static>> {
        let hit = Style::new()
            .fg(theme::t().bg)
            .bg(theme::t().mark)
            .add_modifier(Modifier::BOLD);
        let styles: Vec<Style> = styles
            .iter()
            .zip(hits)
            .map(|(&s, &h)| if h { hit } else { s })
            .collect();
        let mut spans = Vec::new();
        let mut start = 0;
        for k in 1..=chars.len() {
            if k == chars.len() || styles[k] != styles[start] {
                spans.push(Span::styled(
                    chars[start..k].iter().collect::<String>(),
                    styles[start],
                ));
                start = k;
            }
        }
        if spans.is_empty() {
            spans.push(Span::styled(String::new(), Style::new().fg(theme::t().fg)));
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

        if let Some((prompt, buf)) = &self.typing {
            let (lead, hint) = match (prompt, self.mode) {
                (Prompt::Search, _) => (" /", ""),
                (Prompt::Line, Mode::Text) => (" go to line: ", "   40 \u{b7} 50% \u{b7} Esc"),
                (Prompt::Line, Mode::Hex) => (
                    " go to offset: ",
                    "   0x1f00 \u{b7} 8000 \u{b7} 50% \u{b7} Esc",
                ),
            };
            let line = Line::from(vec![
                Span::styled(
                    lead,
                    Style::new()
                        .fg(theme::t().accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{buf}\u{2588}"),
                    Style::new().fg(theme::t().fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(hint, Style::new().fg(theme::t().fg_dim)),
            ]);
            frame
                .buffer_mut()
                .set_line(area.x, area.y, &line, area.width);
            return;
        }

        let paths: &[(&str, &str)] = &[("Esc", "close"), ("Tab", "paths"), ("p", "list")];
        let plain: &[(&str, &str)] = &[("Esc", "close")];
        let keys_lead = if self.links.is_some() { paths } else { plain };
        let rest: &[(&str, &str)] = &[
            ("h", "hex"),
            ("w", "wrap"),
            ("y/Y", "copy"),
            ("/", "search"),
            ("n/N", "next"),
            ("g/G", "ends"),
            (":", "line"),
            ("F2", "save"),
        ];
        let keys: Vec<(&str, &str)> = keys_lead.iter().chain(rest).copied().collect();
        // A message is what just happened (a refusal, "past the end"): it must be read,
        // so keys are dropped from the end, down to Esc, until it fits beside them.
        let width = |n: usize| -> usize {
            keys[..n]
                .iter()
                .map(|(k, l)| k.chars().count() + l.chars().count() + 3)
                .sum()
        };
        let room = area.width as usize;
        // A chosen path's description, cut to fit beside Esc, the path in its middle.
        let message = match &self.said {
            Some(s) if s.text() == self.message => s.fit(room.saturating_sub(width(1) + 2)),
            _ => self.message.clone(),
        };
        let said = if message.is_empty() {
            0
        } else {
            message.chars().count() + 2
        };
        let mut shown = keys.len();
        while shown > 1 && width(shown) + said > room {
            shown -= 1;
        }
        let keys = &keys[..shown];
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
        if !message.is_empty() {
            spans.push(Span::styled(
                format!("  {message}"),
                Style::new().fg(theme::t().mark),
            ));
        }
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &Line::from(spans), area.width);
    }
}

/// The style of each character of `raw` as [`display_line`] shows it (`shown` of
/// them): a tab is as many spaces as it spreads to, each in the tab's style.
fn char_styles(raw: &str, pieces: &[Piece], tab: usize, shown: usize) -> Vec<Style> {
    let plain = Style::new().fg(theme::t().fg);
    if pieces.is_empty() {
        return vec![plain; shown];
    }
    let mut out = Vec::with_capacity(shown);
    let mut p = 0;
    for (at, c) in raw.char_indices() {
        while p < pieces.len() && pieces[p].end <= at {
            p += 1;
        }
        let style = match pieces.get(p) {
            Some(s) if s.start <= at => theme::syntax_style(s.kind),
            _ => plain,
        };
        let n = if c == '\t' {
            tab - (out.len() % tab)
        } else {
            1
        };
        out.extend(std::iter::repeat_n(style, n));
    }
    out.resize(shown, plain);
    out
}

/// Digits in `radix` as a number; more than a `u64` holds is the largest one, which is
/// past the end of anything, rather than "not a number".
fn number(digits: &str, radix: u32) -> Option<u64> {
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    Some(u64::from_str_radix(digits, radix).unwrap_or(u64::MAX))
}

/// A command as typed, from its line in a history: what follows the prompt (`$ `,
/// `# `, `> `, `% `, `❯ `), at most 40 characters.
fn command_text(line: &str) -> String {
    let after = ["$ ", "# ", "> ", "% ", "\u{276f} "]
        .iter()
        .filter_map(|p| line.find(p).map(|at| (at, p.len())))
        .min_by_key(|(at, _)| *at)
        .map_or(line, |(at, n)| &line[at + n..]);
    xdevcmd_core::format::elide_middle(after.trim(), 40)
}

#[cfg(test)]
mod tests {
    use super::command_text;
    use super::Viewer;
    use xdevcmd_core::document::Document;

    /// A shell's history is held in memory and never coloured, whatever it is called.
    #[test]
    fn a_document_in_memory_is_never_coloured() {
        let doc = Document::from_bytes("history.rs", b"fn main() {}\n".to_vec());
        let v = Viewer::open(doc, true, 4).with_syntax(true, u64::MAX, true);
        assert_eq!(v.language(), None);
    }

    #[test]
    fn a_file_over_the_cap_is_plain_and_says_so() {
        let dir = std::env::temp_dir().join(format!("xdevcmd-viewer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("big.rs");
        std::fs::write(&p, "fn main() {}\n".repeat(10)).unwrap();
        let v = Viewer::open(Document::open(&p).unwrap(), true, 4).with_syntax(true, 100, true);
        assert_eq!(v.language(), None);
        assert!(v.message().contains("not coloured"), "{}", v.message());
        let v = Viewer::open(Document::open(&p).unwrap(), true, 4).with_syntax(true, 1000, true);
        assert_eq!(v.language(), Some("Rust"));
        let v = Viewer::open(Document::open(&p).unwrap(), true, 4).with_syntax(false, 1000, true);
        assert_eq!(v.language(), None, "off by setting");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_command_is_what_follows_the_prompt() {
        assert_eq!(command_text("$ rg -l foo"), "rg -l foo");
        assert_eq!(command_text("user@box:~/src$ make"), "make");
        assert_eq!(command_text("PS C:\\src> dir /b /s"), "dir /b /s");
        let long = "$ printf '\\033]133;C\\007%s/%s\\n' a x.txt; printf '\\033]133;A\\007'";
        let got = command_text(long);
        assert!(got.starts_with("printf"), "{got}");
        assert_eq!(got.chars().count(), 40);
    }
}
