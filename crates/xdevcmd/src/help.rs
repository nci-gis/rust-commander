//! F1 — every key and every setting, as they are right now.
//!
//! Built from the live keymap and the loaded config, never from a hand-written list, so
//! a key rebound in `config.toml` shows up here as rebound. A help screen that describes
//! the defaults to someone who changed them is worse than none.

use std::collections::HashSet;

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use ratatui::Frame;

use crate::config::{self, Config, Outcome, Settings};
use crate::theme;
use crate::viewer::Action as Close;

/// Known problems by system, and what to do; F1 points there rather than repeating it.
const TROUBLESHOOTING: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/main/docs/troubleshooting.md"
);

const TABS: [&str; 2] = ["Keys", "Settings"];

/// A line of a page and where its continuation lines start when it is wrapped: under
/// the description, for a key's row; under the line's own indent otherwise.
pub struct Row(Line<'static>, usize);

impl From<Line<'static>> for Row {
    fn from(line: Line<'static>) -> Self {
        let text = line.to_string();
        let indent = text.len() - text.trim_start_matches(' ').len();
        Row(line, indent)
    }
}

pub struct Help {
    tab: usize,
    scroll: [usize; 2],
    pages: [Vec<Row>; 2],
    /// The pages wrapped to the width they were last drawn at, and that width: what is
    /// shown, scrolled and counted.
    shown: [Vec<Line<'static>>; 2],
    shown_width: u16,
    height: usize,
    tab_rects: [Rect; 2],
}

impl Help {
    pub fn new(settings: &Settings, outcome: &Outcome) -> Self {
        let pages = [keys_page(settings), settings_page(settings, outcome)];
        let shown = pages
            .each_ref()
            .map(|p| p.iter().map(|r| r.0.clone()).collect());
        Self {
            tab: 0,
            scroll: [0, 0],
            pages,
            shown,
            shown_width: 0,
            height: 20,
            tab_rects: [Rect::ZERO; 2],
        }
    }

    fn scroll_by(&mut self, delta: isize) {
        let max = self.shown[self.tab].len().saturating_sub(self.height) as isize;
        let s = &mut self.scroll[self.tab];
        *s = (*s as isize + delta).clamp(0, max) as usize;
    }

    pub fn on_mouse(&mut self, m: MouseEvent) -> Close {
        match m.kind {
            MouseEventKind::ScrollDown => self.scroll_by(3),
            MouseEventKind::ScrollUp => self.scroll_by(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                let p = Position::new(m.column, m.row);
                if let Some(i) = self.tab_rects.iter().position(|r| r.contains(p)) {
                    self.tab = i;
                }
            }
            _ => {}
        }
        Close::Stay
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Close {
        let max = self.shown[self.tab].len().saturating_sub(self.height);
        let s = &mut self.scroll[self.tab];
        match k.code {
            KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('q') => return Close::Close,
            KeyCode::Tab | KeyCode::Right | KeyCode::Left | KeyCode::BackTab => {
                self.tab = 1 - self.tab;
            }
            KeyCode::Char('1') => self.tab = 0,
            KeyCode::Char('2') => self.tab = 1,
            KeyCode::Down | KeyCode::Char('j') => *s = (*s + 1).min(max),
            KeyCode::Up | KeyCode::Char('k') => *s = s.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => *s = (*s + self.height).min(max),
            KeyCode::PageUp => *s = s.saturating_sub(self.height),
            KeyCode::Home | KeyCode::Char('g') => *s = 0,
            KeyCode::End | KeyCode::Char('G') => *s = max,
            _ => {}
        }
        Close::Stay
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let th = theme::t();
        frame.render_widget(Clear, area);
        let block = crate::decor::bordered()
            .border_style(Style::new().fg(th.border_active))
            .style(Style::new().bg(th.bg).fg(th.fg))
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "help",
                    Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ]));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        crate::decor::notch(frame.buffer_mut(), area);
        if inner.height < 4 || inner.width < 20 {
            return;
        }

        // Drawn by hand rather than with the Tabs widget, so each tab's rectangle is
        // known exactly and a click on it can be matched.
        let mut cx = inner.x + 1;
        for (i, name) in TABS.iter().enumerate() {
            let label = format!(" {name} ");
            let w = label.chars().count() as u16;
            let style = if i == self.tab {
                Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(th.fg_dim)
            };
            frame
                .buffer_mut()
                .set_line(cx, inner.y, &Line::styled(label, style), w);
            self.tab_rects[i] = Rect::new(cx, inner.y, w, 1);
            cx += w + 1;
        }

        let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 3);
        self.height = body.height as usize;
        if self.shown_width != body.width {
            // The first line on screen stays first, as near as wrapping allows.
            let tops = self.scroll.map(|s| s as f64);
            let before = self.shown.each_ref().map(|p| p.len().max(1) as f64);
            self.shown_width = body.width;
            let width = usize::from(body.width);
            self.shown = self.pages.each_ref().map(|p| {
                p.iter()
                    .flat_map(|Row(line, hang)| crate::wrap::line(line, width, *hang))
                    .collect()
            });
            for t in 0..2 {
                self.scroll[t] = (tops[t] / before[t] * self.shown[t].len() as f64) as usize;
            }
        }
        let page = &self.shown[self.tab];
        let max = page.len().saturating_sub(self.height);
        self.scroll[self.tab] = self.scroll[self.tab].min(max);
        for (i, line) in page
            .iter()
            .skip(self.scroll[self.tab])
            .take(body.height as usize)
            .enumerate()
        {
            frame
                .buffer_mut()
                .set_line(body.x, body.y + i as u16, line, body.width);
        }

        let hint = Line::from(vec![
            Span::styled(
                " Tab",
                Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch  ", Style::new().fg(th.fg_dim)),
            Span::styled(
                "\u{2191}\u{2193}",
                Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" scroll  ", Style::new().fg(th.fg_dim)),
            Span::styled(
                "Esc",
                Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" close", Style::new().fg(th.fg_dim)),
            Span::styled(
                if max > 0 {
                    format!(
                        "   {}/{}",
                        (self.scroll[self.tab] + self.height).min(page.len()),
                        page.len()
                    )
                } else {
                    String::new()
                },
                Style::new().fg(th.fg_dim),
            ),
        ]);
        let y = inner.y + inner.height - 1;
        frame.buffer_mut().set_style(
            Rect::new(inner.x, y, inner.width, 1),
            Style::new().bg(th.bg_bar),
        );
        frame.buffer_mut().set_line(inner.x, y, &hint, inner.width);
    }
}

fn heading(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {text}"),
        Style::new()
            .fg(theme::t().accent)
            .add_modifier(Modifier::BOLD),
    ))
}

/// The key column of F1: wide enough for "Alt+Shift+Right · Alt+r Right".
const KEYS_WIDTH: usize = 30;

fn keys_page(settings: &Settings) -> Vec<Row> {
    let th = theme::t();
    let mut out = Vec::new();
    let name_width = crate::keys::BINDINGS
        .iter()
        .map(|b| b.name.len())
        .max()
        .unwrap_or(0);
    let shell_prefix = settings
        .keymap
        .shell_prefix()
        .map(|c| crate::keys::key_name(c.0, c.1));
    // Keymap A in one line, from the live keymap: what xdevcmd takes inside the shell.
    for sentence in crate::keys::shell_rule(&settings.keymap) {
        out.push(Row::from(Line::from(Span::styled(
            format!("  {sentence}"),
            Style::new().fg(th.fg_dim),
        ))));
    }
    out.push(Row::from(Line::raw("")));
    for (i, (group, rows)) in crate::keys::grouped().into_iter().enumerate() {
        if i > 0 {
            out.push(Row::from(Line::raw("")));
        }
        out.push(Row::from(heading(group)));
        for b in rows {
            let keys = settings.keymap.keys_for(b.action);
            // Nine Alt+digit keys would push the row off the screen: first … last.
            let shown = match keys.as_slice() {
                [] => "(unbound)".to_string(),
                [first, .., last] if keys.len() > 3 => format!("{first} \u{2026} {last}"),
                _ => keys.join(" \u{b7} "),
            };
            let defaults: HashSet<String> = b
                .keys
                .iter()
                .filter_map(|k| crate::keys::parse_binding(k).ok())
                .map(crate::keys::binding_name)
                .collect();
            let rebound = keys.iter().cloned().collect::<HashSet<_>>() != defaults;
            let mut spans = vec![
                Span::styled(
                    if rebound { "  \u{2022} " } else { "    " },
                    Style::new().fg(th.mark),
                ),
                Span::styled(
                    format!("{shown:<KEYS_WIDTH$} "),
                    Style::new().fg(th.fg).add_modifier(Modifier::BOLD),
                ),
                // The name before the description: names are short, so the column after
                // them starts in the same place on every row, and the description, the
                // one of varying length, comes last.
                Span::styled(
                    format!("{:<name_width$} ", b.name),
                    Style::new().fg(th.fg_dim),
                ),
                Span::styled(b.about.to_string(), Style::new().fg(th.fg)),
            ];
            if b.action.is_global() {
                spans.push(Span::styled(
                    "  works in the shell",
                    Style::new().fg(th.fg_dim),
                ));
            } else if let Some(p) = shell_prefix.as_ref().filter(|_| b.action.acts_on_shell()) {
                // After the prefix only single keys work: Alt+X Alt+R is not a group in a
                // group, so `Alt+r Left` is not reachable from the shell. Say which one is.
                let hint = match keys.iter().find(|k| !k.contains(' ')) {
                    Some(_) if keys.iter().all(|k| !k.contains(' ')) => {
                        format!("  in the shell: {p} first")
                    }
                    Some(one) => format!("  in the shell: {p} {one}"),
                    None => "  not from the shell".to_string(),
                };
                spans.push(Span::styled(hint, Style::new().fg(th.fg_dim)));
            }
            out.push(Row(Line::from(spans), 4 + KEYS_WIDTH + 1 + name_width + 1));
        }
    }

    let fixed: &[(&str, &[(&str, &str)])] = &[
        (
            "Viewer (F3) \u{2014} fixed keys",
            &[
                ("h", "Switch between text and hex"),
                ("w", "Wrap long lines on or off"),
                ("/", "Search; Enter to jump"),
                ("n \u{b7} N", "Next / previous match"),
                ("g \u{b7} G", "Top / bottom"),
                (
                    ": \u{b7} Ctrl+G",
                    "Go to a line, or 50%; in hex an offset (0x1f00)",
                ),
                ("F2", "Save as\u{2026}: a copy, or a shell's history (0600)"),
                ("Drag", "Select lines with the mouse"),
                ("y \u{b7} Y", "Copy the selection or top line / the screen"),
                ("Esc \u{b7} q", "Close the viewer"),
                (
                    "Tab \u{b7} Shift+Tab",
                    "A shell's history: the next / previous path in it",
                ),
                (
                    "Enter \u{b7} F3 \u{b7} F4",
                    "On a chosen path: go there / view / edit, at its line",
                ),
                (
                    "p \u{b7} P",
                    "A shell's history: a list of its paths / add them to a list",
                ),
            ],
        ),
        (
            "Mouse",
            &[
                ("click", "Focus a pane, select a row"),
                (
                    "double-click",
                    "What Enter does: enter a directory, open a file",
                ),
                ("Ctrl+click", "Mark or unmark a row"),
                ("drag down a pane", "Mark a range of rows"),
                ("drag to the other pane", "Copy or move there (asks which)"),
                ("drag a border", "Resize panes and the terminal"),
                (
                    "double-click a border",
                    "Share it evenly between the panes beside it",
                ),
                (
                    "path in a pane's title",
                    "Click a directory's name to go there",
                ),
                ("right-click", "Context menu"),
                ("header", "Sort by that column; again to reverse"),
                ("key bar", "Click a key to run it"),
                (
                    "terminal tab bar",
                    "Pick a tab; \u{00d7} closes it, + opens one; right-click: its menu",
                ),
                ("wheel", "Scroll; over a shell, back through its history"),
                (
                    "in a shell",
                    "Clicks, drags and wheel go to a program that asked for the mouse",
                ),
            ],
        ),
        (
            "Safe operations (Settings \u{2192} General; on by default)",
            &[
                (
                    "Covered",
                    "F5 copy, F6 move, Shift+F5 copy as, Alt+Y sync, extract, Alt+F5 pack",
                ),
                (
                    "",
                    "and Shift+F2 rename many (its files put back if a run is killed)",
                ),
                (
                    "How",
                    "A file is written under a hidden temporary name, renamed when complete",
                ),
                (
                    "If killed",
                    "The next start removes what was left and offers to continue the job",
                ),
                (
                    "Always",
                    "A replaced file stays whole until its copy is done; closing the terminal",
                ),
                ("", "stops jobs, cleans up and saves the session"),
                (
                    "Not covered",
                    "Shift+F8 (nothing to keep), F4 (the editor writes)",
                ),
            ],
        ),
        (
            "Two-key bindings, and keys that do nothing",
            &[
                (
                    "Alt+R, then \u{2190}\u{2192}\u{2191}\u{2193}",
                    "Keys written \"Alt+r Left\": Alt+R, then Left, again and again",
                ),
                (
                    "Enter \u{b7} Esc",
                    "Done resizing; so is any other key, or the mouse",
                ),
                (
                    "Other two-key bindings",
                    "Run once, then close; a badge on the status line shows one waiting",
                ),
                ("xdevcmd --show-keys", "What each key really sends"),
                ("More", TROUBLESHOOTING),
            ],
        ),
    ];
    for (title, rows) in fixed {
        out.push(Row::from(Line::raw("")));
        out.push(Row::from(heading(title)));
        for (k, what) in *rows {
            out.push(Row(
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{k:<KEYS_WIDTH$} "),
                        Style::new().fg(th.fg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(what.to_string(), Style::new().fg(th.fg)),
                ]),
                4 + KEYS_WIDTH + 1,
            ));
        }
    }
    out.push(Row::from(Line::raw("")));
    out.push(Row::from(Line::from(Span::styled(
        "  \u{2022} marks a key you changed in config.toml. Rebind under [keys], using the name after the keys.",
        Style::new().fg(th.fg_dim),
    ))));
    out
}

fn settings_page(settings: &Settings, outcome: &Outcome) -> Vec<Row> {
    let th = theme::t();
    let mut out = Vec::new();

    let (status, style) = match outcome {
        Outcome::Loaded(p) => (
            format!("loaded    {}", p.display()),
            Style::new().fg(th.exec),
        ),
        Outcome::NoFile => (
            match config::default_path() {
                Some(p) => format!("none      {}  \u{2014} defaults in use", p.display()),
                None => "no config location \u{2014} using defaults".to_string(),
            },
            Style::new().fg(th.fg_dim),
        ),
        Outcome::Rejected { path, error } => (
            format!("IGNORED   {}: {error}", path.display()),
            Style::new().fg(th.err).add_modifier(Modifier::BOLD),
        ),
    };
    out.push(Row::from(heading("Config file")));
    out.push(Row::from(Line::from(vec![
        Span::raw("    "),
        Span::styled(status, style),
    ])));
    out.push(Row::from(Line::from(Span::styled(
        "    xdevcmd --print-config   writes every setting;   xdevcmd --check-config   validates the file",
        Style::new().fg(th.fg_dim),
    ))));
    out.push(Row::from(Line::raw("")));

    // What is in effect, which a command-line flag may have made stricter than the file.
    out.push(Row::from(heading("In effect")));
    let mode_style = if settings.mode.read_only() {
        Style::new().fg(th.err).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(th.fg)
    };
    out.push(Row::from(Line::from(vec![
        Span::styled("    mode      ", Style::new().fg(th.fg_dim)),
        Span::styled(settings.mode.label().to_string(), mode_style),
    ])));
    out.push(Row::from(Line::from(vec![
        Span::styled("    root      ", Style::new().fg(th.fg_dim)),
        Span::styled(
            settings
                .root
                .as_ref()
                .map_or_else(|| "none".to_string(), |r| r.display().to_string()),
            Style::new().fg(th.fg),
        ),
    ])));
    out.push(Row::from(Line::raw("")));

    // The values as loaded, in the file's own syntax. Keys are on the other tab.
    let render = |c: &Config| {
        let mut c = c.clone();
        c.keys.clear();
        toml::to_string_pretty(&c).unwrap_or_default()
    };
    let defaults: HashSet<(String, String)> = sectioned(&render(&Config::default()));
    let mut section = String::new();
    for raw in render(&settings.config).lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            out.push(Row::from(Line::raw("")));
            continue;
        }
        if line.starts_with('[') {
            section = line.to_string();
            out.push(Row::from(heading(line)));
            continue;
        }
        let changed = !defaults.contains(&(section.clone(), line.to_string()));
        let mut spans = vec![
            Span::styled(
                if changed { "  \u{2022} " } else { "    " },
                Style::new().fg(th.mark),
            ),
            Span::styled(line.to_string(), Style::new().fg(th.fg)),
        ];
        // A colour is easier to judge than its hex.
        if section == "[theme]" {
            if let Some(v) = line
                .split_once('=')
                .map(|(_, v)| v.trim().trim_matches('"'))
            {
                if let Ok(c) = config::parse_color(v) {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        "\u{2588}\u{2588}\u{2588}\u{2588}",
                        Style::new().fg(c),
                    ));
                }
            }
        }
        out.push(Row::from(Line::from(spans)));
    }
    out.push(Row::from(Line::raw("")));
    out.push(Row::from(Line::from(Span::styled(
        "  \u{2022} marks a value that differs from the default.",
        Style::new().fg(th.fg_dim),
    ))));
    out
}

fn sectioned(toml_text: &str) -> HashSet<(String, String)> {
    let mut section = String::new();
    let mut out = HashSet::new();
    for line in toml_text.lines().map(str::trim_end) {
        if line.starts_with('[') {
            section = line.to_string();
        } else if !line.is_empty() {
            out.insert((section.clone(), line.to_string()));
        }
    }
    out
}
