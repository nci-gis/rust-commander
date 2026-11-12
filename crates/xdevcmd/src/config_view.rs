//! Help → Config in use: the loaded config file, as it is in effect, read-only.
//!
//! Built from the loaded config, never from a hand-written list: what is shown is what
//! xdevcmd is using, a `•` on each value that differs from the default, and whether the
//! file was loaded at all (a rejected file is IGNORED, with why). A filter keeps the
//! lines that match and the section each is in. F1 (Commands) has the keys.

use std::collections::HashSet;

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::config::{self, Config, Outcome, Settings};
use crate::line_edit::LineEdit;
use crate::theme;

/// A line of the page: its text, the section it is in (its heading's index, or its
/// own if it is one), and where its continuation lines start when it is wrapped.
struct Entry {
    line: Line<'static>,
    text: String,
    section: usize,
    heading: bool,
    hang: usize,
}

pub struct ConfigView {
    entries: Vec<Entry>,
    edit: LineEdit,
    scroll: usize,
    height: usize,
    /// The entries the filter keeps, wrapped to the width last drawn at.
    shown: Vec<Line<'static>>,
    shown_for: Option<(String, u16)>,
}

pub enum Act {
    Stay,
    Close,
}

impl ConfigView {
    pub fn new(settings: &Settings, outcome: &Outcome) -> Self {
        Self {
            entries: page(settings, outcome),
            edit: LineEdit::new(""),
            scroll: 0,
            height: 20,
            shown: Vec::new(),
            shown_for: None,
        }
    }

    /// The entries the filter keeps: a matching line and its section's heading; a
    /// matching heading, its whole section.
    fn kept(&self) -> Vec<&Entry> {
        let q = self.edit.text().to_lowercase();
        let q = q.trim();
        if q.is_empty() {
            return self.entries.iter().collect();
        }
        let hit: HashSet<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.text.to_lowercase().contains(q))
            .map(|(i, _)| i)
            .collect();
        let whole: HashSet<usize> = hit
            .iter()
            .filter(|i| self.entries[**i].heading)
            .copied()
            .collect();
        let sections: HashSet<usize> = hit.iter().map(|i| self.entries[*i].section).collect();
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                hit.contains(i)
                    || whole.contains(&e.section) && !e.text.trim().is_empty()
                    || e.heading && sections.contains(i)
            })
            .map(|(_, e)| e)
            .collect()
    }

    fn max_scroll(&self) -> usize {
        self.shown.len().saturating_sub(self.height)
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Act {
        match k.code {
            KeyCode::Esc if !self.edit.text().is_empty() => {
                self.edit = LineEdit::new("");
                self.scroll = 0;
            }
            KeyCode::Esc => return Act::Close,
            KeyCode::Down => self.scroll = (self.scroll + 1).min(self.max_scroll()),
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll = (self.scroll + self.height).min(self.max_scroll()),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.height),
            KeyCode::Home if self.edit.text().is_empty() => self.scroll = 0,
            KeyCode::End if self.edit.text().is_empty() => self.scroll = self.max_scroll(),
            // Enter on a setting opening Settings at it is for later (0.5).
            KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab => {}
            _ => {
                if self.edit.on_key(k) {
                    self.scroll = 0;
                }
            }
        }
        Act::Stay
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollDown => self.scroll = (self.scroll + 3).min(self.max_scroll()),
            MouseEventKind::ScrollUp => self.scroll = self.scroll.saturating_sub(3),
            _ => {}
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let th = theme::t();
        let inner = crate::modal::frame_box(frame, area, "Config in use", th.accent);
        if inner.height < 4 || inner.width < 20 {
            return;
        }
        // The filter line, as F1's.
        let mut spans = vec![Span::styled(
            "\u{203a} ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        )];
        let text = self.edit.text();
        spans.push(Span::styled(
            text.clone(),
            Style::new().fg(th.fg).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" ", Style::new().fg(th.bg).bg(th.fg)));
        if text.is_empty() {
            spans.push(Span::styled(
                " type to filter: a setting, a value, a [section]",
                Style::new().fg(th.fg_dim),
            ));
        }
        frame
            .buffer_mut()
            .set_line(inner.x, inner.y, &Line::from(spans), inner.width);

        let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 3);
        self.height = usize::from(body.height);
        let want = (text, body.width);
        if self.shown_for.as_ref() != Some(&want) {
            let width = usize::from(body.width);
            self.shown = self
                .kept()
                .into_iter()
                .flat_map(|e| crate::wrap::line(&e.line, width, e.hang))
                .collect();
            self.shown_for = Some(want);
        }
        self.scroll = self.scroll.min(self.max_scroll());
        if self.shown.is_empty() {
            frame.buffer_mut().set_line(
                body.x,
                body.y,
                &Line::styled("  nothing matches", Style::new().fg(th.fg_dim)),
                body.width,
            );
        }
        for (i, line) in self
            .shown
            .iter()
            .skip(self.scroll)
            .take(self.height)
            .enumerate()
        {
            frame
                .buffer_mut()
                .set_line(body.x, body.y + i as u16, line, body.width);
        }

        let y = inner.y + inner.height - 1;
        let more = if self.max_scroll() > 0 {
            format!(
                "   {}/{}",
                (self.scroll + self.height).min(self.shown.len()),
                self.shown.len()
            )
        } else {
            String::new()
        };
        let hint = format!(
            " \u{2191}\u{2193} scroll \u{b7} Esc {} \u{b7} read-only: File \u{2192} Settings\u{2026} changes the common ones{more}",
            if self.edit.text().is_empty() {
                "close"
            } else {
                "clear"
            }
        );
        frame.buffer_mut().set_style(
            Rect::new(inner.x, y, inner.width, 1),
            Style::new().bg(th.bg_bar),
        );
        frame.buffer_mut().set_line(
            inner.x,
            y,
            &Line::styled(hint, Style::new().fg(th.fg_dim)),
            inner.width,
        );
    }
}

/// The page: the file's status, what is in effect, then the config as TOML.
fn page(settings: &Settings, outcome: &Outcome) -> Vec<Entry> {
    let th = theme::t();
    let mut out: Vec<Entry> = Vec::new();
    let mut section = 0;
    let mut push = |out: &mut Vec<Entry>, line: Line<'static>, heading: bool| {
        let text = line.to_string();
        let hang = text.len() - text.trim_start_matches(' ').len();
        if heading {
            section = out.len();
        }
        out.push(Entry {
            line,
            text,
            section,
            heading,
            hang,
        });
    };
    let heading = |text: &str| {
        Line::from(Span::styled(
            format!(" {text}"),
            Style::new()
                .fg(theme::t().accent)
                .add_modifier(Modifier::BOLD),
        ))
    };

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
    push(&mut out, heading("Config file"), true);
    push(
        &mut out,
        Line::from(vec![Span::raw("    "), Span::styled(status, style)]),
        false,
    );
    push(
        &mut out,
        Line::from(Span::styled(
            "    xdevcmd --print-config   writes every setting;   xdevcmd --check-config   validates the file",
            Style::new().fg(th.fg_dim),
        )),
        false,
    );
    push(&mut out, Line::raw(""), false);

    // What is in effect, which a command-line flag may have made stricter than the file.
    push(&mut out, heading("In effect"), true);
    let mode_style = if settings.mode.read_only() {
        Style::new().fg(th.err).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(th.fg)
    };
    push(
        &mut out,
        Line::from(vec![
            Span::styled("    mode      ", Style::new().fg(th.fg_dim)),
            Span::styled(settings.mode.label().to_string(), mode_style),
        ]),
        false,
    );
    push(
        &mut out,
        Line::from(vec![
            Span::styled("    root      ", Style::new().fg(th.fg_dim)),
            Span::styled(
                settings
                    .root
                    .as_ref()
                    .map_or_else(|| "none".to_string(), |r| r.display().to_string()),
                Style::new().fg(th.fg),
            ),
        ]),
        false,
    );
    push(&mut out, Line::raw(""), false);

    // The values as loaded, in the file's own syntax, `[keys]` included: the keys you
    // changed are here as you wrote them, and in F1 beside their commands.
    let render = |c: &Config| toml::to_string_pretty(c).unwrap_or_default();
    let defaults: HashSet<(String, String)> = sectioned(&render(&Config::default()));
    let mut current = String::new();
    for raw in render(&settings.config).lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            push(&mut out, Line::raw(""), false);
            continue;
        }
        if line.starts_with('[') {
            current = line.to_string();
            push(&mut out, heading(line), true);
            continue;
        }
        let changed = !defaults.contains(&(current.clone(), line.to_string()));
        let mut spans = vec![
            Span::styled(
                if changed { "  \u{2022} " } else { "    " },
                Style::new().fg(th.mark),
            ),
            Span::styled(line.to_string(), Style::new().fg(th.fg)),
        ];
        // A colour is easier to judge than its hex.
        if current == "[theme]" {
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
        push(&mut out, Line::from(spans), false);
    }
    push(&mut out, Line::raw(""), false);
    push(
        &mut out,
        Line::from(Span::styled(
            "  \u{2022} marks a value that differs from the default.",
            Style::new().fg(th.fg_dim),
        )),
        false,
    );
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
