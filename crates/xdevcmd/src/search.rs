//! Quick search (`/`) and the quick filter (Ctrl+F): a line typed at the bottom of a
//! pane.
//!
//! Search moves the cursor to the first name that starts with what is typed — or,
//! failing that, contains it — and Up/Down step through every name that contains it.
//! The filter hides what does not match, and Enter leaves it in force, shown in the
//! pane's title, until Esc or leaving the directory. With `*` or `?` in it, either one
//! is a pattern instead (`*.rs`).
//!
//! Any key the line does not use ends it and then does what it always does, as in
//! Total Commander: type `src`, press F5, and the copy starts.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::glob::{has_wildcards, Pattern};
use xdevcmd_core::Entry;

use crate::layout::PaneId;
use crate::line_edit::LineEdit;
use crate::theme;

pub struct Search {
    pub pane: PaneId,
    pub edit: LineEdit,
    /// Filter rather than jump.
    pub filter: bool,
    /// The filter that was on before, put back by Esc.
    pub before: Option<String>,
    /// Whether anything matched the text as it stands.
    pub found: bool,
}

impl Search {
    pub fn new(pane: PaneId, filter: bool, before: Option<String>) -> Self {
        let edit = LineEdit::new(if filter {
            before.as_deref().unwrap_or("")
        } else {
            ""
        });
        Self {
            pane,
            edit,
            filter,
            before,
            found: true,
        }
    }

    pub fn text(&self) -> String {
        self.edit.text()
    }
}

/// Every row that contains `text` (or matches it, as a pattern), `..` excluded.
pub fn matches(entries: &[Entry], text: &str) -> Vec<usize> {
    if text.is_empty() {
        return Vec::new();
    }
    let pattern = has_wildcards(text).then(|| Pattern::new(text));
    let needle = text.to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.is_parent())
        .filter(|(_, e)| match &pattern {
            Some(p) => p.matches(&e.name, e.is_dir()),
            None => e.name.to_lowercase().contains(&needle),
        })
        .map(|(i, _)| i)
        .collect()
}

/// Where typing `text` should put the cursor: the first name starting with it, else
/// the first containing it.
pub fn first(entries: &[Entry], text: &str) -> Option<usize> {
    let hits = matches(entries, text);
    let needle = text.to_lowercase();
    hits.iter()
        .copied()
        .find(|i| entries[*i].name.to_lowercase().starts_with(&needle))
        .or(hits.first().copied())
}

/// The next (or previous) hit after `from`, wrapping round.
pub fn step(entries: &[Entry], text: &str, from: usize, forward: bool) -> Option<usize> {
    let hits = matches(entries, text);
    if hits.is_empty() {
        return None;
    }
    Some(if forward {
        hits.iter().copied().find(|i| *i > from).unwrap_or(hits[0])
    } else {
        hits.iter()
            .rev()
            .copied()
            .find(|i| *i < from)
            .unwrap_or(hits[hits.len() - 1])
    })
}

/// Draw the line over the bottom border of the pane at `area`.
pub fn render(frame: &mut Frame, area: Rect, s: &Search, count: usize) {
    if area.height < 3 || area.width < 12 {
        return;
    }
    let th = theme::t();
    let y = area.y + area.height - 1;
    let x = area.x + 1;
    let w = area.width.saturating_sub(2);
    let label = if s.filter { " filter " } else { " / " };
    let tail = if s.edit.chars().is_empty() {
        String::new()
    } else if s.found {
        format!(" {count} ")
    } else {
        " no match ".to_string()
    };
    let room = (w as usize).saturating_sub(label.chars().count() + tail.chars().count() + 1);
    let chars = s.edit.chars();
    let start = chars.len().saturating_sub(room.saturating_sub(1));
    let typed: String = chars[start..].iter().collect();
    let mut spans = vec![
        Span::styled(
            label,
            Style::new()
                .fg(th.bg)
                .bg(th.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {typed}"),
            Style::new()
                .fg(th.fg)
                .bg(th.bg_bar)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::new().fg(th.bg).bg(th.fg)),
    ];
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = (w as usize).saturating_sub(used + tail.chars().count());
    spans.push(Span::styled(" ".repeat(pad), Style::new().bg(th.bg_bar)));
    spans.push(Span::styled(
        tail,
        Style::new()
            .fg(if s.found { th.fg_dim } else { th.err })
            .bg(th.bg_bar),
    ));
    frame.buffer_mut().set_line(x, y, &Line::from(spans), w);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use xdevcmd_core::Kind;

    fn e(name: &str) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: if name == ".." {
                Kind::Parent
            } else {
                Kind::File
            },
            size: 0,
            modified: None,
            hidden: false,
            symlink: false,
            error: None,
        }
    }

    #[test]
    fn a_prefix_wins_over_a_match_further_in() {
        let v = vec![e(".."), e("my-readme"), e("README.md"), e("src")];
        assert_eq!(first(&v, "rea"), Some(2), "README starts with it");
        assert_eq!(first(&v, "me"), Some(1), "only contained: the first such");
        assert_eq!(first(&v, "zzz"), None);
        assert_eq!(first(&v, ".."), None, "never the parent row");
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let v = vec![e("a1"), e("b"), e("a2"), e("a3")];
        assert_eq!(step(&v, "a", 0, true), Some(2));
        assert_eq!(step(&v, "a", 3, true), Some(0), "wraps to the top");
        assert_eq!(step(&v, "a", 0, false), Some(3), "wraps to the bottom");
    }

    #[test]
    fn wildcards_make_it_a_pattern() {
        let v = vec![e("main.rs"), e("lib.rsx"), e("Cargo.toml")];
        assert_eq!(matches(&v, "*.rs"), vec![0]);
        assert_eq!(
            matches(&v, ".rs"),
            vec![0, 1],
            "plain text: anywhere in the name"
        );
    }
}
