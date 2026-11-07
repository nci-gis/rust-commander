//! File → Settings…: change the common settings without editing `config.toml` by hand.
//!
//! Tabs of settings, each a yes/no or a short list of choices, changed with ←/→ or
//! Space. The theme previews as it changes. Enter saves into the config file through
//! `config_edit`, which touches only the lines of what changed and keeps everything
//! else; Esc leaves everything as it was. Each row says whether it takes effect at once
//! or at the next start. Keys are rebound in the file, under `[keys]`; this screen
//! says so rather than offering a second way to do it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::config::Config;
use crate::config_edit::Value;
use crate::modal::{buttons, centered, frame_box, put};
use crate::theme;

pub const TABS: &[&str] = &["General", "Files", "Theme", "Terminal", "Keys"];

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Pick(&'static [&'static str]),
    Themes,
}

#[derive(Clone, Copy)]
pub struct Field {
    tab: usize,
    pub section: &'static str,
    pub key: &'static str,
    label: &'static str,
    help: &'static str,
    kind: Kind,
    /// Takes effect at once; otherwise at the next start.
    now: bool,
}

const fn f(
    tab: usize,
    section: &'static str,
    key: &'static str,
    label: &'static str,
    help: &'static str,
    kind: Kind,
    now: bool,
) -> Field {
    Field {
        tab,
        section,
        key,
        label,
        help,
        kind,
        now,
    }
}

const FIELDS: &[Field] = &[
    f(
        0,
        "general",
        "confirm_transfer",
        "Ask before copy and move",
        "Deleting always asks; this is about F5 and F6",
        Kind::Bool,
        true,
    ),
    f(
        0,
        "general",
        "delete_to_trash",
        "F8 moves to the trash",
        "Shift+F8 always deletes for good",
        Kind::Bool,
        true,
    ),
    f(
        0,
        "general",
        "restore_session",
        "Reopen panes and tabs",
        "The panes, tabs and directories of last time, at the next start",
        Kind::Bool,
        true,
    ),
    f(
        0,
        "general",
        "autosave_secs",
        "Save them every (seconds)",
        "0: only on quit; otherwise closing the window keeps them too",
        Kind::Pick(&["0", "5", "15", "30", "60", "300"]),
        true,
    ),
    f(
        0,
        "general",
        "menu_bar",
        "Menu bar",
        "File, View, Help on the top row; F9 opens it either way",
        Kind::Bool,
        true,
    ),
    f(
        1,
        "files",
        "enter",
        "Enter on a file",
        "open: with its program, as Total Commander; view: in xdevcmd's viewer",
        Kind::Pick(&["open", "view"]),
        true,
    ),
    f(
        1,
        "files",
        "filter",
        "Ctrl+F filter",
        "fuzzy: letters in order, best first; plain: the text as typed",
        Kind::Pick(&["fuzzy", "plain"]),
        true,
    ),
    f(
        1,
        "files",
        "show_hidden",
        "Show hidden files",
        "Ctrl+H changes it in a pane",
        Kind::Bool,
        false,
    ),
    f(
        1,
        "files",
        "sort",
        "Sort by",
        "Clicking a column header changes it in a pane",
        Kind::Pick(&["name", "ext", "size", "modified"]),
        false,
    ),
    f(
        1,
        "files",
        "dirs_first",
        "Directories first",
        "",
        Kind::Bool,
        false,
    ),
    f(
        1,
        "files",
        "auto_refresh_ms",
        "Look for changes every (ms)",
        "0: never; press Ctrl+R to refresh by hand",
        Kind::Pick(&["0", "500", "1000", "2000", "5000"]),
        true,
    ),
    f(
        1,
        "files",
        "max_panes",
        "File panes at most",
        "How many Alt+S / Alt+D may make, the two first included",
        Kind::Pick(&["2", "3", "4", "6", "8", "12", "16"]),
        true,
    ),
    f(
        1,
        "files",
        "icons",
        "File icons",
        "Needs a Nerd Font in the terminal",
        Kind::Bool,
        false,
    ),
    f(
        1,
        "files",
        "ls_colors",
        "Colours from LS_COLORS",
        "Use the colours ls uses, instead of the theme's",
        Kind::Bool,
        false,
    ),
    f(
        1,
        "files",
        "git_status",
        "Git letters",
        "Runs git: only where you trust the repositories (see security.md)",
        Kind::Bool,
        false,
    ),
    f(
        1,
        "viewer",
        "wrap",
        "Viewer wraps long lines",
        "w changes it in the viewer",
        Kind::Bool,
        true,
    ),
    f(
        1,
        "viewer",
        "tab_width",
        "Viewer tab width",
        "",
        Kind::Pick(&["2", "4", "8"]),
        true,
    ),
    f(
        2,
        "theme",
        "name",
        "Theme",
        "Built in, or themes/<name>.toml beside config.toml; ←/→ to try them",
        Kind::Themes,
        true,
    ),
    f(
        2,
        "theme",
        "colors",
        "Colours",
        "auto finds out; 256 for a terminal that shows wrong colours",
        Kind::Pick(&["auto", "truecolor", "256", "16"]),
        false,
    ),
    f(
        3,
        "terminal",
        "show",
        "Terminal panel at start",
        "F12 opens it either way",
        Kind::Bool,
        false,
    ),
    f(
        3,
        "terminal",
        "height_percent",
        "Its height (%)",
        "Drag its border to change it now",
        Kind::Pick(&["20", "25", "30", "35", "40", "50", "60"]),
        false,
    ),
    f(
        3,
        "terminal",
        "max_terminals",
        "Shells at most",
        "Across every terminal tab and split",
        Kind::Pick(&["1", "2", "3", "4", "6", "8", "12", "16"]),
        true,
    ),
];

/// A setting's value, as the config file writes it.
pub fn get(c: &Config, f: &Field) -> String {
    match (f.section, f.key) {
        ("general", "confirm_transfer") => c.general.confirm_transfer.to_string(),
        ("general", "delete_to_trash") => c.general.delete_to_trash.to_string(),
        ("general", "restore_session") => c.general.restore_session.to_string(),
        ("general", "autosave_secs") => c.general.autosave_secs.to_string(),
        ("general", "menu_bar") => c.general.menu_bar.to_string(),
        ("files", "enter") => c.files.enter.clone(),
        ("files", "filter") => c.files.filter.clone(),
        ("files", "show_hidden") => c.files.show_hidden.to_string(),
        ("files", "sort") => c.files.sort.clone(),
        ("files", "dirs_first") => c.files.dirs_first.to_string(),
        ("files", "auto_refresh_ms") => c.files.auto_refresh_ms.to_string(),
        ("files", "max_panes") => c.files.max_panes.to_string(),
        ("files", "icons") => c.files.icons.to_string(),
        ("files", "ls_colors") => c.files.ls_colors.to_string(),
        ("files", "git_status") => c.files.git_status.to_string(),
        ("viewer", "wrap") => c.viewer.wrap.to_string(),
        ("viewer", "tab_width") => c.viewer.tab_width.to_string(),
        ("theme", "name") => c.theme.name.clone(),
        ("theme", "colors") => c.theme.colors.clone(),
        ("terminal", "show") => c.terminal.show.to_string(),
        ("terminal", "height_percent") => c.terminal.height_percent.to_string(),
        ("terminal", "max_terminals") => c.terminal.max_terminals.to_string(),
        _ => String::new(),
    }
}

/// Set a setting from the string `get` gives; a number that does not parse is ignored.
fn set(c: &mut Config, f: &Field, v: &str) {
    let b = v == "true";
    let n = || v.parse::<u64>().ok();
    match (f.section, f.key) {
        ("general", "confirm_transfer") => c.general.confirm_transfer = b,
        ("general", "delete_to_trash") => c.general.delete_to_trash = b,
        ("general", "restore_session") => c.general.restore_session = b,
        ("general", "autosave_secs") => {
            c.general.autosave_secs = n().unwrap_or(c.general.autosave_secs)
        }
        ("general", "menu_bar") => c.general.menu_bar = b,
        ("files", "enter") => c.files.enter = v.to_string(),
        ("files", "filter") => c.files.filter = v.to_string(),
        ("files", "show_hidden") => c.files.show_hidden = b,
        ("files", "sort") => c.files.sort = v.to_string(),
        ("files", "dirs_first") => c.files.dirs_first = b,
        ("files", "auto_refresh_ms") => {
            c.files.auto_refresh_ms = n().unwrap_or(c.files.auto_refresh_ms)
        }
        ("files", "max_panes") => c.files.max_panes = n().map_or(c.files.max_panes, |x| x as usize),
        ("files", "icons") => c.files.icons = b,
        ("files", "ls_colors") => c.files.ls_colors = b,
        ("files", "git_status") => c.files.git_status = b,
        ("viewer", "wrap") => c.viewer.wrap = b,
        ("viewer", "tab_width") => {
            c.viewer.tab_width = n().map_or(c.viewer.tab_width, |x| x as usize)
        }
        ("theme", "name") => c.theme.name = v.to_string(),
        ("theme", "colors") => c.theme.colors = v.to_string(),
        ("terminal", "show") => c.terminal.show = b,
        ("terminal", "height_percent") => {
            c.terminal.height_percent = n().map_or(c.terminal.height_percent, |x| x as u16)
        }
        ("terminal", "max_terminals") => {
            c.terminal.max_terminals = n().map_or(c.terminal.max_terminals, |x| x as usize)
        }
        _ => {}
    }
}

/// The value as `config_edit` writes it.
pub fn value(f: &Field, v: &str) -> Value {
    match f.kind {
        Kind::Bool => Value::Bool(v == "true"),
        Kind::Pick(choices) if choices.iter().all(|c| c.parse::<i64>().is_ok()) => {
            Value::Int(v.parse().unwrap_or(0))
        }
        _ => Value::Str(v.to_string()),
    }
}

/// A pick's choices, with the value the file has among them even when it is not one
/// of the usual ones — `max_panes = 5` written by hand — in its place by size, so
/// stepping through the choices can come back to it rather than losing it.
fn with_own(choices: &[&str], own: &str) -> Vec<String> {
    let mut out: Vec<String> = choices.iter().map(|s| s.to_string()).collect();
    if out.iter().any(|c| c == own) {
        return out;
    }
    match own.parse::<i64>() {
        Ok(n) if choices.iter().all(|c| c.parse::<i64>().is_ok()) => {
            let at = choices
                .iter()
                .position(|c| c.parse::<i64>().is_ok_and(|c| c > n))
                .unwrap_or(out.len());
            out.insert(at, own.to_string());
        }
        _ => out.push(own.to_string()),
    }
    out
}

pub struct SettingsDialog {
    pub tab: usize,
    row: usize,
    /// The settings as they were when the dialog opened, and as changed.
    pub before: Config,
    pub now: Config,
    themes: Vec<String>,
    /// Why saving is not possible here (read-only modes), if it is not.
    pub cannot_save: Option<String>,
    pub buttons: Vec<Rect>,
    /// Where each tab, and each row with its value, was drawn: a click hits exactly
    /// what is on screen.
    tab_hits: Vec<Rect>,
    row_hits: Vec<RowHit>,
}

/// One row as drawn: the whole line, and its `‹` and `›` (for a check box, both are
/// the box).
struct RowHit {
    line: Rect,
    back: Rect,
    value: Rect,
}

pub enum Outcome {
    Stay,
    /// Esc: put everything back (the theme was being previewed).
    Cancel,
    /// Enter: save these settings.
    Save,
    /// The theme changed: preview it.
    Preview(String),
}

impl SettingsDialog {
    pub fn new(config: &Config, themes: Vec<String>, cannot_save: Option<String>) -> Self {
        Self {
            tab: 0,
            row: 0,
            before: config.clone(),
            now: config.clone(),
            themes,
            cannot_save,
            buttons: Vec::new(),
            tab_hits: Vec::new(),
            row_hits: Vec::new(),
        }
    }

    fn rows(&self) -> Vec<&'static Field> {
        FIELDS.iter().filter(|f| f.tab == self.tab).collect()
    }

    /// The fields whose values differ from when the dialog opened.
    pub fn changed(&self) -> Vec<(&'static Field, String)> {
        FIELDS
            .iter()
            .filter(|f| get(&self.before, f) != get(&self.now, f))
            .map(|f| (f, get(&self.now, f)))
            .collect()
    }

    /// Changed settings that only take effect at the next start, by label.
    pub fn need_restart(&self) -> Vec<&'static str> {
        self.changed()
            .into_iter()
            .filter(|(f, _)| !f.now)
            .map(|(f, _)| f.label)
            .collect()
    }

    fn step(&mut self, delta: isize) -> Outcome {
        let rows = self.rows();
        let Some(f) = rows.get(self.row).copied() else {
            return Outcome::Stay;
        };
        let cur = get(&self.now, f);
        let choices: Vec<String> = match f.kind {
            Kind::Bool => vec!["false".into(), "true".into()],
            Kind::Pick(c) => with_own(c, &get(&self.before, f)),
            Kind::Themes => self.themes.clone(),
        };
        if choices.is_empty() {
            return Outcome::Stay;
        }
        let i = choices.iter().position(|c| *c == cur);
        let n = choices.len() as isize;
        let next = match i {
            Some(i) => (i as isize + delta).rem_euclid(n) as usize,
            None => 0,
        };
        set(&mut self.now, f, &choices[next]);
        if matches!(f.kind, Kind::Themes) {
            return Outcome::Preview(choices[next].clone());
        }
        Outcome::Stay
    }

    /// A click picks a tab or a row, or changes a value: `‹` steps it back, the value
    /// or `›` forward, a check box toggles. The wheel moves between rows. The buttons
    /// are Save and Cancel, or only Close where nothing can be saved.
    pub fn on_mouse(&mut self, m: MouseEvent) -> Outcome {
        let p = Position::new(m.column, m.row);
        match m.kind {
            MouseEventKind::ScrollUp => self.row = self.row.saturating_sub(1),
            MouseEventKind::ScrollDown => {
                let rows = self.rows().len();
                if rows > 0 {
                    self.row = (self.row + 1).min(rows - 1);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = |r: &Rect| r.contains(p);
                if let Some(i) = self.buttons.iter().position(hit) {
                    return match (i, self.cannot_save.is_some()) {
                        (0, false) => Outcome::Save,
                        _ => Outcome::Cancel,
                    };
                }
                if let Some(t) = self.tab_hits.iter().position(hit) {
                    if t != self.tab {
                        self.tab = t;
                        self.row = 0;
                    }
                    return Outcome::Stay;
                }
                if let Some(i) = self.row_hits.iter().position(|r| hit(&r.line)) {
                    self.row = i;
                    let r = &self.row_hits[i];
                    if hit(&r.back) {
                        return self.step(-1);
                    }
                    if hit(&r.value) {
                        return self.step(1);
                    }
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Outcome {
        let rows = self.rows().len();
        match k.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter if self.cannot_save.is_none() => return Outcome::Save,
            KeyCode::Tab => {
                self.tab = (self.tab + 1) % TABS.len();
                self.row = 0;
            }
            KeyCode::BackTab => {
                self.tab = (self.tab + TABS.len() - 1) % TABS.len();
                self.row = 0;
            }
            KeyCode::Up => self.row = self.row.saturating_sub(1),
            KeyCode::Down if rows > 0 => self.row = (self.row + 1).min(rows - 1),
            KeyCode::Left => return self.step(-1),
            KeyCode::Right | KeyCode::Char(' ') => return self.step(1),
            _ => {}
        }
        Outcome::Stay
    }
}

pub fn render(frame: &mut Frame, area: Rect, d: &mut SettingsDialog) {
    let th = theme::t();
    let r = centered(area, 86, 22);
    let inner = frame_box(frame, r, "Settings", th.accent);
    // The tabs.
    let mut spans = Vec::new();
    d.tab_hits.clear();
    d.row_hits.clear();
    let mut x = inner.x;
    for (i, t) in TABS.iter().enumerate() {
        let w = t.chars().count() as u16 + 2;
        d.tab_hits
            .push(Rect::new(x, inner.y, w, 1).intersection(inner));
        x += w + 1;
        let style = if i == d.tab {
            Style::new()
                .fg(th.bg)
                .bg(th.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(th.fg_dim)
        };
        spans.push(Span::styled(format!(" {t} "), style));
        spans.push(Span::raw(" "));
    }
    frame
        .buffer_mut()
        .set_line(inner.x, inner.y, &Line::from(spans), inner.width);

    let body_top = 2u16;
    if TABS[d.tab] == "Keys" {
        let lines = [
            "Every key is on F1, with the name to rebind it by.",
            "",
            "To change one, write it under [keys] in config.toml:",
            "    copy = [\"F5\", \"Ctrl+c\"]",
            "Two keys in a row are written with a space:",
            "    resize_left = [\"Alt+Shift+Left\", \"Alt+r Left\"]",
            "",
            "xdevcmd --print-keys lists them all; xdevcmd --show-keys shows what a key sends.",
        ];
        for (i, l) in lines.iter().enumerate() {
            put(frame, inner, body_top + i as u16, l, Style::new().fg(th.fg));
        }
    } else {
        let rows = d.rows();
        let label_w = 32usize;
        for (i, f) in rows.iter().enumerate() {
            let y = body_top + i as u16;
            if y + 4 >= inner.height {
                break;
            }
            let v = get(&d.now, f);
            let shown = match f.kind {
                Kind::Bool => if v == "true" { "[x]" } else { "[ ]" }.to_string(),
                _ => format!("\u{2039} {v} \u{203a}"),
            };
            let changed = v != get(&d.before, f);
            let when = if f.now { "" } else { "next start" };
            let line = format!(
                "{}{:<label_w$} {:<18} {when}",
                if changed { "\u{2022} " } else { "  " },
                f.label,
                shown
            );
            let style = if i == d.row {
                Style::new().fg(th.fg).bg(th.row_cursor_active)
            } else {
                Style::new().fg(th.fg)
            };
            let row = Rect::new(inner.x, inner.y + y, inner.width, 1);
            frame
                .buffer_mut()
                .set_style(row, if i == d.row { style } else { Style::new() });
            put(frame, inner, y, &line, style);
            // The value starts after the mark (2) and the label and its space.
            let vx = inner.x + 2 + label_w as u16 + 1;
            let vw = shown.chars().count() as u16;
            let value = Rect::new(vx, row.y, vw, 1).intersection(row);
            let back = match f.kind {
                Kind::Bool => Rect::default(),
                _ => Rect::new(vx, row.y, 1, 1).intersection(row),
            };
            d.row_hits.push(RowHit {
                line: row,
                back,
                value,
            });
        }
        if let Some(f) = rows.get(d.row) {
            put(
                frame,
                inner,
                inner.height.saturating_sub(3),
                f.help,
                Style::new().fg(th.fg_dim),
            );
        }
    }
    let hint = match &d.cannot_save {
        Some(why) => why.clone(),
        None => "\u{2191}\u{2193} choose  \u{2190}\u{2192} or Space change  Tab next tab".into(),
    };
    put(
        frame,
        inner,
        inner.height.saturating_sub(2),
        &hint,
        Style::new().fg(if d.cannot_save.is_some() {
            th.err
        } else {
            th.fg_dim
        }),
    );
    let y = inner.y + inner.height.saturating_sub(1);
    d.buttons = if d.cannot_save.is_some() {
        buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[("Esc", "close", th.accent)],
        )
    } else {
        buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[("Enter", "save", th.accent), ("Esc", "cancel", th.accent)],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_one_of_its_choices() {
        let base = Config::default();
        for f in FIELDS {
            if let Kind::Pick(c) = f.kind {
                let v = get(&base, f);
                assert!(
                    c.contains(&v.as_str()),
                    "{}.{}: the default {v} is not among {c:?}",
                    f.section,
                    f.key
                );
            }
        }
    }

    #[test]
    fn a_value_written_by_hand_stays_among_the_choices() {
        let c = &["2", "3", "4", "6"];
        assert_eq!(with_own(c, "4"), ["2", "3", "4", "6"]);
        assert_eq!(with_own(c, "5"), ["2", "3", "4", "5", "6"]);
        assert_eq!(with_own(c, "9"), ["2", "3", "4", "6", "9"]);
        assert_eq!(with_own(&["open", "view"], "odd"), ["open", "view", "odd"]);
    }

    #[test]
    fn every_field_reads_and_writes_its_own_setting() {
        let base = Config::default();
        for f in FIELDS {
            let v = get(&base, f);
            assert!(!v.is_empty(), "{}.{} reads nothing", f.section, f.key);
            let other = match f.kind {
                Kind::Bool => if v == "true" { "false" } else { "true" }.to_string(),
                Kind::Pick(c) => c.iter().find(|x| **x != v).unwrap().to_string(),
                Kind::Themes => if v == "light" { "dark" } else { "light" }.to_string(),
            };
            let mut c = base.clone();
            set(&mut c, f, &other);
            assert_eq!(get(&c, f), other, "{}.{} does not write", f.section, f.key);
            // And only that one setting changed.
            let moved: Vec<_> = FIELDS
                .iter()
                .filter(|g| get(&c, g) != get(&base, g))
                .collect();
            assert_eq!(moved.len(), 1, "{}.{} moved others", f.section, f.key);
        }
    }

    #[test]
    fn every_choice_is_a_value_the_config_accepts() {
        for f in FIELDS {
            let choices: Vec<String> = match f.kind {
                Kind::Bool => vec!["true".into(), "false".into()],
                Kind::Pick(c) => c.iter().map(|s| s.to_string()).collect(),
                Kind::Themes => crate::theme::BUILT_IN
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            };
            for v in choices {
                let mut c = Config::default();
                set(&mut c, f, &v);
                let text = crate::config_edit::set("", f.section, f.key, &value(f, &v));
                assert!(
                    crate::config::parse(&text).is_ok(),
                    "{}.{} = {v} is refused: {text}",
                    f.section,
                    f.key
                );
            }
        }
    }
}
