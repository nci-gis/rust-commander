//! F1, Commands: every command, found by typing, with its key beside it (Round 024).
//!
//! The rows are built from the live keymap, the `[[menu]]` entries and the mode in
//! effect each time F1 opens, never from a hand-written list: a key rebound in
//! `config.toml` shows up here as rebound, and a command the mode forbids says why.
//! Ranking reuses the quick filter's fuzzy score (`xdevcmd_core::fuzzy`), so typing
//! here feels as it does in a pane.

use std::collections::HashSet;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::MenuItem;
use crate::keys::{self, Action, Keymap, Place};

/// Known problems by system, and what to do; F1 points there rather than repeating it.
/// At this version's tag, as Help → Documentation is (`docs.rs`).
const TROUBLESHOOTING: &str = crate::docs::docs_page!("troubleshooting.md");

/// Commands with no key and no `Action`: they live in the menus only, and are rows
/// here too, or "every command by typing" would not be true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    Settings,
    Theme,
    About,
    ConfigInUse,
    /// The right-click menu's Copy path and Copy relative path (item 48).
    CopyPath,
    CopyRelativePath,
    Documentation,
    /// The active terminal tab's menu: Log to file….
    LogTab,
    /// The active terminal tab's menu: Stop logging.
    StopLog,
}

/// What Enter on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    Act(Action),
    Extra(Extra),
    /// Entry `n` of `[[menu]]`, through the same path as Shift+F9.
    User(usize),
    /// Shown to be read, never run: a mouse gesture, the viewer's own key, a note. Why
    /// it cannot run, said when Enter is pressed on it.
    ReadOnly(&'static str),
}

pub struct Row {
    /// What the row is called ("Copy as…"), and the group it is listed under.
    pub title: String,
    pub group: String,
    /// The keys bound now, as the config writes them; empty for most read-only rows.
    pub keys: Vec<String>,
    /// What the Key column shows: the keys, shortened (`Alt+1 … Alt+9`).
    pub key_text: String,
    pub about: String,
    /// How to reach it from the shell, where that differs ("in the shell: Alt+x first").
    pub note: Option<String>,
    pub place: Place,
    pub run: Run,
    /// A key the config changed from the default.
    pub rebound: bool,
    /// Why the mode in effect (or an archive) turns it off; `None` when it can run.
    pub refused: Option<String>,
    /// The `[keys]` name, for the rebind hint; `None` for anything else.
    pub keys_name: Option<&'static str>,
    /// What recent use is remembered by: the `[keys]` name, `menu:<label>`, or the
    /// menu-only command's own word.
    pub id: String,
    /// "Group: Title", what a typed filter is scored against.
    pub name: String,
    /// `keys`, parsed once: a key typed as text is compared with these on every key.
    pub parsed: Vec<keys::Keys>,
    /// The Key column and the description, what a filter falls back to: a mouse
    /// gesture is found by its name ("double-click").
    pub hay: String,
}

impl Row {
    pub fn read_only(&self) -> bool {
        matches!(self.run, Run::ReadOnly(_))
    }

    /// Whether this row belongs where F1 was pressed.
    pub fn here(&self, ctx: Place) -> bool {
        self.place == ctx || self.place == Place::Everywhere
    }
}

/// The keys as F1 shows them: two-key bindings as written, more than three as
/// `first … last` (nine Alt+digits would push the row off the screen).
pub fn key_text(keys: &[String]) -> String {
    match keys {
        [] => String::new(),
        [first, .., last] if keys.len() > 3 => format!("{first} \u{2026} {last}"),
        _ => keys.join(" \u{b7} "),
    }
}

/// Every row, in the order the large window browses them: the bindings by group (the
/// menu-only commands at the end of General), your `[[menu]]` commands, then the
/// read-only sections. `refuse` says why the mode turns a row off.
pub fn rows(
    keymap: &Keymap,
    menu: &[MenuItem],
    refuse: impl Fn(&Run) -> Option<String>,
) -> Vec<Row> {
    let mut out = Vec::new();
    let shell_prefix = keymap.shell_prefix().map(|c| keys::key_name(c.0, c.1));
    for (group, bindings) in keys::grouped() {
        for b in bindings {
            let live = keymap.keys_for(b.action);
            let defaults: HashSet<String> = b
                .keys
                .iter()
                .filter_map(|k| keys::parse_binding(k).ok())
                .map(keys::binding_name)
                .collect();
            let rebound = live.iter().cloned().collect::<HashSet<_>>() != defaults;
            let note = if b.action.is_global() {
                Some("works in the shell".to_string())
            } else if b.action.opens_from_shell() {
                // Alt+X F1: F1 alone is the program's there.
                shell_prefix.as_ref().and_then(|p| {
                    live.iter()
                        .find(|k| k.starts_with(&format!("{p} ")))
                        .map(|k| format!("in the shell: {k}"))
                })
            } else {
                // After the prefix only single keys work: Alt+X Alt+R is not a group in
                // a group, so `Alt+r Left` is not reachable from the shell. Say which is.
                shell_prefix
                    .as_ref()
                    .filter(|_| b.action.acts_on_shell())
                    .map(|p| match live.iter().find(|k| !k.contains(' ')) {
                        Some(_) if live.iter().all(|k| !k.contains(' ')) => {
                            format!("in the shell: {p} first")
                        }
                        Some(one) => format!("in the shell: {p} {one}"),
                        None => "not from the shell".to_string(),
                    })
            };
            let run = if b.action == Action::GoTab {
                Run::ReadOnly("needs the tab's digit: press the key itself")
            } else {
                Run::Act(b.action)
            };
            out.push(Row {
                title: b.title.to_string(),
                group: group.to_string(),
                key_text: if live.is_empty() {
                    "(unbound)".to_string()
                } else {
                    key_text(&live)
                },
                keys: live,
                about: b.about.to_string(),
                note,
                place: b.action.place(),
                refused: refuse(&run),
                run,
                rebound,
                keys_name: Some(b.name),
                id: b.name.to_string(),
                name: format!("{group}: {}", b.title),
                parsed: Vec::new(),
                hay: String::new(),
            });
        }
        if group == "Files" {
            for (title, about, extra, id) in [
                (
                    "Copy path",
                    "The selected paths (or this folder's), whole, to the clipboard",
                    Extra::CopyPath,
                    "copy_path",
                ),
                (
                    "Copy relative path",
                    "The same, from the repository's root, or from this folder outside one",
                    Extra::CopyRelativePath,
                    "copy_relative_path",
                ),
            ] {
                let run = Run::Extra(extra);
                out.push(Row {
                    title: title.into(),
                    group: group.into(),
                    keys: Vec::new(),
                    key_text: String::new(),
                    about: about.into(),
                    note: None,
                    place: Place::Files,
                    refused: refuse(&run),
                    run,
                    rebound: false,
                    keys_name: None,
                    id: id.into(),
                    name: format!("{group}: {title}"),
                    parsed: Vec::new(),
                    hay: String::new(),
                });
            }
        }
        if group == "General" {
            for (title, about, extra, id) in [
                (
                    "Settings\u{2026}",
                    "The common settings, saved to config.toml",
                    Extra::Settings,
                    "settings",
                ),
                (
                    "Theme\u{2026}",
                    "Try a theme now; Settings saves it",
                    Extra::Theme,
                    "theme",
                ),
                (
                    "Config in use",
                    "The loaded config file and the settings in effect, read-only",
                    Extra::ConfigInUse,
                    "config_in_use",
                ),
                (
                    "Documentation",
                    "The docs for this version: the copy beside xdevcmd, and the link",
                    Extra::Documentation,
                    "documentation",
                ),
                (
                    "About xdevcmd",
                    "Version, licence, where releases are",
                    Extra::About,
                    "about",
                ),
            ] {
                let run = Run::Extra(extra);
                out.push(Row {
                    title: title.into(),
                    group: group.into(),
                    keys: Vec::new(),
                    key_text: String::new(),
                    about: about.into(),
                    note: None,
                    place: Place::Everywhere,
                    refused: refuse(&run),
                    run,
                    rebound: false,
                    keys_name: None,
                    id: id.into(),
                    name: format!("{group}: {title}"),
                    parsed: Vec::new(),
                    hay: String::new(),
                });
            }
        }
    }

    // The terminal tab menu's own commands, which have no key: acting on the active tab.
    if let Some(at) = out.iter().rposition(|r| r.group == "Terminal") {
        let mut rows = Vec::new();
        for (title, about, extra, id) in [
            (
                "Log tab to file\u{2026}",
                "Everything the active terminal tab prints, to a file you name (text or raw); \u{25cf} on the tab",
                Extra::LogTab,
                "log_tab",
            ),
            (
                "Stop logging tab",
                "End the active terminal tab's log; the file is flushed and closed",
                Extra::StopLog,
                "stop_log",
            ),
        ] {
            let run = Run::Extra(extra);
            rows.push(Row {
                title: title.into(),
                group: "Terminal".into(),
                keys: Vec::new(),
                key_text: String::new(),
                about: about.into(),
                note: Some("also on the tab's right-click menu".into()),
                place: Place::Everywhere,
                refused: refuse(&run),
                run,
                rebound: false,
                keys_name: None,
                id: id.into(),
                name: format!("Terminal: {title}"),
                parsed: Vec::new(),
                hay: String::new(),
            });
        }
        out.splice(at + 1..at + 1, rows);
    }

    // Your commands: their letter is the one Shift+F9's menu takes.
    let menu_key = keymap.keys_for(Action::UserMenu).into_iter().next();
    for (i, m) in menu.iter().enumerate() {
        let run = Run::User(i);
        let group = "Your commands";
        out.push(Row {
            title: m.label.clone(),
            group: group.into(),
            keys: Vec::new(),
            key_text: match &menu_key {
                Some(k) if !m.key.is_empty() => format!("{k} {}", m.key),
                _ => String::new(),
            },
            about: m.command.clone(),
            note: None,
            place: Place::Everywhere,
            refused: refuse(&run),
            run,
            rebound: false,
            keys_name: None,
            id: format!("menu:{}", m.label),
            name: format!("{group}: {}", m.label),
            parsed: Vec::new(),
            hay: String::new(),
        });
    }

    for (group, place, why, rows) in FIXED {
        for (k, what) in *rows {
            // A note's label is its name; a key's description is.
            let (title, key, about) = if *why == NOTE {
                (k.to_string(), String::new(), what.to_string())
            } else {
                (what.to_string(), k.to_string(), String::new())
            };
            out.push(Row {
                name: format!("{group}: {title}"),
                title,
                group: group.to_string(),
                keys: Vec::new(),
                key_text: key,
                about,
                note: None,
                place: *place,
                run: Run::ReadOnly(why),
                rebound: false,
                refused: None,
                keys_name: None,
                id: String::new(),
                parsed: Vec::new(),
                hay: String::new(),
            });
        }
    }
    for r in &mut out {
        r.hay = format!("{} {}", r.key_text, r.about);
        r.parsed = r
            .keys
            .iter()
            .filter_map(|k| keys::parse_binding(k).ok())
            .collect();
    }
    out
}

const NOTE: &str = "a note to read";
const VIEWER_KEY: &str = "the viewer's own key: press it there";
const GESTURE: &str = "a mouse gesture";

/// The read-only sections: what the viewer and the mouse do, and the notes F1 always
/// had. Group, where it applies, why it cannot run, and (key, what) rows.
#[allow(clippy::type_complexity)]
const FIXED: &[(&str, Place, &str, &[(&str, &str)])] = &[
    (
        "Viewer",
        Place::Viewer,
        VIEWER_KEY,
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
        Place::Everywhere,
        GESTURE,
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
        "Safe operations",
        Place::Everywhere,
        NOTE,
        &[
            (
                "Covered",
                "F5 copy, F6 move, Shift+F5 copy as, Alt+Y sync, extract, Alt+F5 pack, Shift+F2 rename many (Settings \u{2192} General; on by default)",
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
                "A replaced file stays whole until its copy is done; closing the terminal stops jobs, cleans up and saves the session",
            ),
            (
                "Not covered",
                "Shift+F8 (nothing to keep), F4 (the editor writes)",
            ),
        ],
    ),
    (
        "Two-key bindings",
        Place::Everywhere,
        NOTE,
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

/// One line of the list as F1 shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A group's name, in the large window with an empty filter.
    Heading(String),
    /// A sentence to read above the groups: keymap A's rule.
    Text(String),
    /// The dim line between the commands for here and the rest.
    Elsewhere,
    /// Row `row`; `hits` are the characters of its shown name the filter matched;
    /// `tagged` shows the name with its group ("Files: Copy").
    Row {
        row: usize,
        hits: Vec<usize>,
        tagged: bool,
    },
}

/// How near a row is to where F1 was pressed, for ranking: 0 this place's own (the
/// viewer's Close in the viewer), 1 a command that works everywhere (Zoom), 2 the
/// rest. A read-only row that applies everywhere (a mouse gesture, a note) counts as
/// the rest: it is never what someone typing a command's name wants first.
fn nearness(r: &Row, ctx: Place) -> u8 {
    if r.place == ctx {
        0
    } else if r.place == Place::Everywhere && !r.read_only() {
        1
    } else {
        2
    }
}

/// Index of `id` in the recent list, most recent first; `usize::MAX` when absent.
fn recency(recent: &[String], id: &str) -> usize {
    if id.is_empty() {
        return usize::MAX;
    }
    recent.iter().position(|r| r == id).unwrap_or(usize::MAX)
}

/// A query that is a key ("f5", "alt+j", "Ctrl+PageDown"): two characters or more, so
/// a letter typed as the start of a word is not taken for the key `q`.
fn as_key(query: &str) -> Option<keys::Keys> {
    let q = query.trim();
    if q.chars().count() < 2 {
        return None;
    }
    keys::parse_binding(q).ok()
}

/// The rows a filter finds, best first, as one flat list:
/// 0. a key typed as text that a row has, exactly;
/// 1. the name ("Group: Title") matched as words are typed: word starts and runs;
/// 2. only the description matched, so: below every such name match;
/// 3. the name's letters matched scattered ("trash" in "Terminal: Show…"), last.
///
/// Within a tier: the rows for where F1 was pressed first, then those for everywhere,
/// then the rest ([`nearness`]), so "close" in the viewer is the viewer's Close (its own
/// key, read-only) and never Close pane, which would close the viewer and then the pane;
/// then what can run before what is only to read (a mouse gesture that says "Copy or
/// move there" is not the Copy someone typing "copy" in the shell wants); then by
/// score; then recently run first, then a row for here (a mouse gesture works in any
/// pane), then table order. A description matched only by scattered letters is not a
/// match: in a sentence, almost anything is.
pub fn rank(rows: &[Row], query: &str, ctx: Place, recent: &[String]) -> Vec<Item> {
    use xdevcmd_core::fuzzy;
    let key = as_key(query);
    let q = fuzzy::Query::new(query);
    // Sorted by (tier, nearness, read-only, best score, recency, not here, table order).
    type Found = (
        (u8, u8, bool, std::cmp::Reverse<i32>, usize, bool, usize),
        Vec<usize>,
    );
    let mut found: Vec<Found> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let by_key = key.is_some_and(|k| r.parsed.contains(&k));
        let (tier, score, hits) = if let Some((s, hits)) = q.score_positions(&r.name) {
            let tier = if by_key {
                0
            } else if fuzzy::is_words(&r.name, &hits) {
                1
            } else {
                3
            };
            (tier, s, hits)
        } else if by_key {
            (0, 0, Vec::new())
        } else if let Some((s, hits)) = q.score_positions(&r.hay) {
            if !fuzzy::is_words(&r.hay, &hits) {
                continue;
            }
            (2, s, Vec::new())
        } else {
            continue;
        };
        let key = (
            tier,
            nearness(r, ctx),
            r.read_only(),
            std::cmp::Reverse(score),
            recency(recent, &r.id),
            !r.here(ctx),
            i,
        );
        found.push((key, hits));
    }
    found.sort_by_key(|f| f.0);
    found
        .into_iter()
        .map(|((.., row), hits)| Item::Row {
            row,
            hits,
            tagged: true,
        })
        .collect()
}

/// The small palette with nothing typed: what was run recently, then the commands for
/// where F1 was pressed, then everything else under a dim "elsewhere".
pub fn browse_small(rows: &[Row], ctx: Place, recent: &[String]) -> Vec<Item> {
    let item = |row| Item::Row {
        row,
        hits: Vec::new(),
        tagged: true,
    };
    let mut shown = vec![false; rows.len()];
    let mut out = Vec::new();
    for id in recent {
        if let Some(i) = rows.iter().position(|r| !r.id.is_empty() && &r.id == id) {
            if !shown[i] {
                shown[i] = true;
                out.push(item(i));
            }
        }
    }
    // Read-only rows are for reading in the large window, except in the viewer, whose
    // own keys are what someone pressing F1 there is after.
    // This place's own rows first (Copy, View in a file pane), then those that work
    // everywhere (Quit, Zoom).
    let mut order: Vec<usize> = (0..rows.len()).collect();
    // The Files group first among them: what a file pane is for.
    order.sort_by_key(|i| rows[*i].group != "Files");
    for exact in [true, false] {
        for &i in &order {
            let r = &rows[i];
            // F1 itself is not something to look for from F1.
            let wanted = if ctx == Place::Viewer {
                r.place == Place::Viewer
            } else {
                r.here(ctx) && !r.read_only() && r.run != Run::Act(Action::Help)
            };
            if wanted && (r.place == ctx) == exact && !shown[i] {
                shown[i] = true;
                out.push(item(i));
            }
        }
    }
    if shown.iter().any(|s| !s) {
        out.push(Item::Elsewhere);
        out.extend((0..rows.len()).filter(|i| !shown[*i]).map(item));
    }
    out
}

/// The large window with nothing typed: every row under its group's name, to browse.
pub fn browse_large(rows: &[Row]) -> Vec<Item> {
    let mut out = Vec::new();
    let mut group: Option<&str> = None;
    for (i, r) in rows.iter().enumerate() {
        if group != Some(r.group.as_str()) {
            group = Some(&r.group);
            out.push(Item::Heading(r.group.clone()));
        }
        out.push(Item::Row {
            row: i,
            hits: Vec::new(),
            tagged: false,
        });
    }
    out
}

// ── recent commands ───────────────────────────────────────────────────────────────

/// How many recently run commands are kept (the owner's decision 4).
pub const RECENT: usize = 8;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct RecentFile {
    #[serde(default)]
    recent: Vec<String>,
}

fn recent_path() -> Option<std::path::PathBuf> {
    crate::session::state_file("commands.toml")
}

/// Whether `id` names a command that exists now: a `[keys]` action, a menu-only
/// command, or one of `menu`'s entries by label.
fn known(id: &str, menu: &[MenuItem]) -> bool {
    keys::BINDINGS.iter().any(|b| b.name == id)
        || matches!(id, "settings" | "theme" | "about" | "config_in_use")
        || id
            .strip_prefix("menu:")
            .is_some_and(|l| menu.iter().any(|m| m.label == l))
}

/// The commands run recently, most recent first, kept beside the session. Names no
/// longer known (a `[[menu]]` entry removed, an action renamed) are dropped; a missing
/// or broken file is an empty list. Commands, not files: nothing private in it.
pub fn load_recent(menu: &[MenuItem]) -> Vec<String> {
    recent_path().map_or_else(Vec::new, |p| load_recent_from(&p, menu))
}

fn load_recent_from(p: &std::path::Path, menu: &[MenuItem]) -> Vec<String> {
    // A file far larger than eight names could ever make is not ours: ignored, unread.
    if std::fs::metadata(p).map_or(true, |m| m.len() > RECENT_FILE_MAX) {
        return Vec::new();
    }
    std::fs::read_to_string(p)
        .map(|t| parse_recent(&t, menu))
        .unwrap_or_default()
}

/// The largest `commands.toml` read: eight names are a few hundred bytes.
const RECENT_FILE_MAX: u64 = 64 * 1024;

/// The list in a `commands.toml`: known names only, each once (the first, most recent,
/// kept), at most [`RECENT`]. Anything unreadable is an empty list.
fn parse_recent(text: &str, menu: &[MenuItem]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in toml::from_str::<RecentFile>(text)
        .map(|f| f.recent)
        .unwrap_or_default()
    {
        if known(&id, menu) && !out.contains(&id) {
            out.push(id);
        }
    }
    out.truncate(RECENT);
    out
}

/// `id` to the front of `recent`, once, at most [`RECENT`] kept.
pub fn remember(recent: &mut Vec<String>, id: &str) {
    if id.is_empty() {
        return;
    }
    recent.retain(|r| r != id);
    recent.insert(0, id.to_string());
    recent.truncate(RECENT);
}

pub fn save_recent(recent: &[String]) -> std::io::Result<()> {
    let p = recent_path().ok_or_else(|| std::io::Error::other("no state directory"))?;
    let text = toml::to_string_pretty(&RecentFile {
        recent: recent.to_vec(),
    })
    .map_err(std::io::Error::other)?;
    crate::session::write_private(&p, text.as_bytes())
}

// ── the palette ───────────────────────────────────────────────────────────────────

/// F1's two sizes: one component, so the filter, the selection and the scroll stay
/// when it grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// Top-centre, up to [`SMALL_ROWS`] rows: type and go.
    Small,
    /// The whole window, the groups to browse: what F1 was before.
    Large,
}

/// The small size's widest and its most rows.
pub const SMALL_WIDTH: u16 = 72;
pub const SMALL_ROWS: u16 = 12;

/// Below this the large size would be no larger than the small one, and F1 pressed
/// again closes instead (the owner's decision 2).
pub const LARGE_MIN: (u16, u16) = (90, 30);

/// Columns: Name · Key · Description from this inner width; Name · Key from
/// [`NAME_ONLY`]; under it, Name alone.
const THREE_COLUMNS: u16 = 70;
const NAME_ONLY: u16 = 30;

pub enum Outcome {
    Stay,
    Close,
    /// Run row `n`, which can run: the app closes F1 and runs it on what had focus.
    Run(usize),
}

pub struct Commands {
    pub rows: Vec<Row>,
    /// Where F1 was pressed.
    pub ctx: Place,
    recent: Vec<String>,
    edit: crate::line_edit::LineEdit,
    pub size: Size,
    items: Vec<Item>,
    /// The selected item: always an `Item::Row`, `None` when nothing matches.
    sel: Option<usize>,
    scroll: usize,
    /// List rows shown at the last draw.
    height: usize,
    /// The screen F1 is drawn on: whether the large size would be larger.
    screen: ratatui::layout::Rect,
    /// Said on the last line until the next key: why a row cannot run.
    message: Option<String>,
    intro: Vec<String>,
    /// What F1 was opened over (the viewer), given back when it closes.
    pub under: Option<Box<crate::modal::Modal>>,
    /// Opened from the Help menu, with the mouse perhaps: "Next time" names F1.
    pub from_menu: bool,
    /// Where the list and the box were drawn, for the mouse.
    list: ratatui::layout::Rect,
    frame: ratatui::layout::Rect,
    last_click: Option<(std::time::Instant, usize)>,
    /// A Mac's Option typed a character: say once how to make it Alt.
    pub option_hint: bool,
}

/// A string cut to `w` characters, with `…` where it was cut.
/// Counted in columns, not characters: a CJK character takes two.
fn cut(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw > w - 1 {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('\u{2026}');
    out
}

/// Columns `s` takes on screen.
fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

impl Commands {
    pub fn new(
        rows: Vec<Row>,
        ctx: Place,
        recent: Vec<String>,
        intro: Vec<String>,
        size: Size,
        screen: ratatui::layout::Rect,
    ) -> Self {
        let mut c = Self {
            rows,
            ctx,
            recent,
            edit: crate::line_edit::LineEdit::new(""),
            size,
            items: Vec::new(),
            sel: None,
            scroll: 0,
            height: SMALL_ROWS as usize,
            screen,
            message: None,
            intro,
            under: None,
            from_menu: false,
            list: ratatui::layout::Rect::ZERO,
            frame: ratatui::layout::Rect::ZERO,
            last_click: None,
            option_hint: false,
        };
        c.refresh(None);
        c
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<usize> {
        match self.items.get(self.sel?) {
            Some(Item::Row { row, .. }) => Some(*row),
            _ => None,
        }
    }

    /// Whether the large size would be larger than the small one on this screen.
    pub fn can_grow(&self) -> bool {
        self.screen.width >= LARGE_MIN.0 && self.screen.height >= LARGE_MIN.1
    }

    /// The items again, for what is typed and the size; `keep` stays selected if it is
    /// still listed, else the first row is.
    fn refresh(&mut self, keep: Option<usize>) {
        let q = self.edit.text();
        self.items = if !q.trim().is_empty() {
            rank(&self.rows, &q, self.ctx, &self.recent)
        } else if self.size == Size::Small {
            browse_small(&self.rows, self.ctx, &self.recent)
        } else {
            let mut v: Vec<Item> = self.intro.iter().cloned().map(Item::Text).collect();
            v.extend(browse_large(&self.rows));
            v
        };
        let is_row = |it: &Item, want: Option<usize>| match it {
            Item::Row { row, .. } => want.is_none_or(|w| *row == w),
            _ => false,
        };
        self.sel = keep
            .and_then(|k| self.items.iter().position(|it| is_row(it, Some(k))))
            .or_else(|| self.items.iter().position(|it| is_row(it, None)));
        if keep.is_none() {
            self.scroll = 0;
        }
    }

    /// Item `i` of the list, if it is one of the first nine rows: its number.
    fn number(&self, i: usize) -> Option<usize> {
        let n = self.items[..=i]
            .iter()
            .filter(|it| matches!(it, Item::Row { .. }))
            .count();
        (matches!(self.items[i], Item::Row { .. }) && n <= 9).then_some(n)
    }

    /// The item that row number `n` (1…9) is.
    fn numbered(&self, n: usize) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| matches!(it, Item::Row { .. }))
            .nth(n.checked_sub(1)?)
            .map(|(i, _)| i)
    }

    fn step(&mut self, d: isize) {
        let rows: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| matches!(it, Item::Row { .. }))
            .map(|(i, _)| i)
            .collect();
        let Some(at) = self.sel.and_then(|s| rows.iter().position(|r| *r == s)) else {
            return;
        };
        let to = (at as isize + d).clamp(0, rows.len() as isize - 1) as usize;
        self.sel = Some(rows[to]);
        if to == 0 {
            // The first row: whatever is above it (keymap A's rule, its group's name)
            // comes into view too.
            self.scroll = 0;
        }
    }

    /// Enter on item `i`: run it, or say why it cannot.
    fn choose(&mut self, i: usize) -> Outcome {
        let Some(Item::Row { row, .. }) = self.items.get(i) else {
            return Outcome::Stay;
        };
        let row = *row;
        self.sel = Some(i);
        let r = &self.rows[row];
        if let Run::ReadOnly(why) = r.run {
            self.message = Some(if r.key_text.is_empty() {
                format!("{}: {why}", r.title)
            } else {
                format!("{}: {why}", r.key_text)
            });
            return Outcome::Stay;
        }
        if let Some(why) = &r.refused {
            self.message = Some(why.clone());
            return Outcome::Stay;
        }
        // F1's own row, chosen inside F1: the other size, not F1 closed and opened again
        // (which would only start over at the small size).
        if r.run == Run::Act(Action::Help) {
            let to = match self.size {
                Size::Small if self.can_grow() => Size::Large,
                Size::Small => {
                    self.message = Some("the screen is too small for the large size".into());
                    return Outcome::Stay;
                }
                Size::Large => Size::Small,
            };
            self.size = to;
            self.refresh(Some(row));
            return Outcome::Stay;
        }
        Outcome::Run(row)
    }

    pub fn on_key(&mut self, k: ratatui::crossterm::event::KeyEvent) -> Outcome {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};
        let had = self.message.take();
        let page = self.height.max(1) as isize;
        match k.code {
            KeyCode::Esc if !self.edit.text().is_empty() => {
                self.edit = crate::line_edit::LineEdit::new("");
                let keep = self.selected();
                self.refresh(keep);
            }
            KeyCode::Esc => return Outcome::Close,
            KeyCode::F(1) if k.modifiers.is_empty() => {
                if self.size == Size::Small && self.can_grow() {
                    self.size = Size::Large;
                    let keep = self.selected();
                    self.refresh(keep);
                } else {
                    return Outcome::Close;
                }
            }
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::PageUp => self.step(-page),
            KeyCode::PageDown => self.step(page),
            // With nothing typed there is no line to move along: the list's ends.
            KeyCode::Home if self.edit.text().is_empty() => self.step(-(self.items.len() as isize)),
            KeyCode::End if self.edit.text().is_empty() => self.step(self.items.len() as isize),
            KeyCode::Enter => {
                if let Some(i) = self.sel {
                    return self.choose(i);
                }
            }
            // Alt+1…9: the first nine rows. Digits alone are text for the filter.
            KeyCode::Char(c @ '1'..='9') if k.modifiers == KeyModifiers::ALT => {
                if let Some(i) = self.numbered(c as usize - '0' as usize) {
                    return self.choose(i);
                }
            }
            // Tab used to switch F1's tabs; there are none now, and it is no filter
            // text either.
            KeyCode::Tab | KeyCode::BackTab => self.message = had,
            _ => {
                if self.edit.on_key(k) {
                    self.refresh(None);
                    if let Some(c) = crate::keyfix::typed_by_option(k) {
                        if cfg!(target_os = "macos") && self.option_hint && self.sel.is_none() {
                            self.option_hint = false;
                            self.message = Some(crate::keyfix::option_hint(c));
                        }
                    }
                } else {
                    self.message = had;
                }
            }
        }
        Outcome::Stay
    }

    pub fn on_mouse(&mut self, m: ratatui::crossterm::event::MouseEvent) -> Outcome {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        use ratatui::layout::Position;
        let p = Position::new(m.column, m.row);
        match m.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let max = self.items.len().saturating_sub(self.height);
                self.scroll = if m.kind == MouseEventKind::ScrollDown {
                    (self.scroll + 3).min(max)
                } else {
                    self.scroll.saturating_sub(3)
                };
                // The selection comes along into view, or the next draw would scroll
                // back to it.
                let view = self.scroll..self.scroll + self.height.max(1);
                if self.sel.is_some_and(|s| !view.contains(&s)) {
                    let is_row = |i: &usize| matches!(self.items[*i], Item::Row { .. });
                    let up = self.sel.is_some_and(|s| s < view.start);
                    self.sel = if up {
                        view.clone().filter(|i| *i < self.items.len()).find(is_row)
                    } else {
                        view.clone()
                            .filter(|i| *i < self.items.len())
                            .rev()
                            .find(is_row)
                    }
                    .or(self.sel);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if !self.frame.contains(p) {
                    // Clicking away from F1 closes it, as menus do.
                    return Outcome::Close;
                }
                if !self.list.contains(p) {
                    return Outcome::Stay;
                }
                let i = self.scroll + usize::from(m.row - self.list.y);
                if !matches!(self.items.get(i), Some(Item::Row { .. })) {
                    return Outcome::Stay;
                }
                self.message = None;
                let now = std::time::Instant::now();
                let double = self.last_click.is_some_and(|(t, at)| {
                    at == i && now.duration_since(t) < std::time::Duration::from_millis(400)
                });
                self.last_click = if double { None } else { Some((now, i)) };
                self.sel = Some(i);
                if double {
                    return self.choose(i);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        use ratatui::layout::Rect;
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span};
        let th = crate::theme::t();
        self.screen = area;
        if self.size == Size::Large && !self.can_grow() {
            // The window shrank under the large size: back to the small one.
            self.size = Size::Small;
            let keep = self.selected();
            self.refresh(keep);
        }
        let box_area = match self.size {
            Size::Large => area,
            Size::Small => {
                let w = SMALL_WIDTH
                    .min(area.width.saturating_sub(4))
                    .max(area.width.min(24));
                let rows = (self.items.len() as u16).clamp(1, SMALL_ROWS);
                let h = (rows + 4).min(area.height.saturating_sub(1).max(4));
                let x = area.x + area.width.saturating_sub(w) / 2;
                // Top-centre, below the menu bar's row.
                let y = area.y + u16::from(area.height > h + 1);
                Rect::new(x, y, w, h.min(area.height))
            }
        };
        self.frame = box_area;
        let inner = crate::modal::frame_box(frame, box_area, "Commands", th.accent);
        if inner.height < 2 || inner.width < 4 {
            self.list = Rect::ZERO;
            return;
        }

        // The filter line.
        let prompt = "\u{203a} ";
        let mut spans = vec![Span::styled(
            prompt,
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        )];
        let chars = self.edit.chars();
        let cur = self.edit.cursor();
        // In columns: from the first character that still lets the cursor show.
        let room = usize::from(inner.width).saturating_sub(3);
        let cw = |c: char| c.width().unwrap_or(0);
        let mut start = cur;
        let mut used = 1; // the cursor's cell
        while start > 0 && used + cw(chars[start - 1]) <= room {
            start -= 1;
            used += cw(chars[start]);
        }
        let mut used = 0;
        for idx in start.. {
            let at_cursor = idx == cur;
            let Some(c) = chars.get(idx).copied().or(at_cursor.then_some(' ')) else {
                break;
            };
            used += cw(c).max(1);
            if used > room {
                break;
            }
            let style = if at_cursor {
                Style::new().fg(th.bg).bg(th.fg)
            } else {
                Style::new().fg(th.fg).add_modifier(Modifier::BOLD)
            };
            spans.push(Span::styled(c.to_string(), style));
        }
        if chars.is_empty() {
            spans.push(Span::styled(
                " type to find a command, or a key (f5)",
                Style::new().fg(th.fg_dim),
            ));
        }
        frame
            .buffer_mut()
            .set_line(inner.x, inner.y, &Line::from(spans), inner.width);

        // The list.
        let list = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(2),
        );
        self.list = list;
        self.height = usize::from(list.height);
        if let Some(s) = self.sel {
            if s < self.scroll {
                self.scroll = s;
            } else if self.height > 0 && s >= self.scroll + self.height {
                self.scroll = s + 1 - self.height;
            }
        }
        self.scroll = self
            .scroll
            .min(self.items.len().saturating_sub(self.height));
        let w = inner.width;
        let shown_name = |it: &Item| match it {
            Item::Row { row, tagged, .. } => {
                let r = &self.rows[*row];
                if *tagged {
                    width(&r.name)
                } else {
                    width(&r.title)
                }
            }
            _ => 0,
        };
        let name_max = self.items.iter().map(shown_name).max().unwrap_or(0);
        let key_max = self
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Row { row, .. } => Some(width(&self.rows[*row].key_text)),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        // The margin: the row's number (Alt+n), then the rebound mark.
        let margin = 4usize;
        let (name_w, key_w) = if w >= THREE_COLUMNS {
            let key_w = key_max.min(22);
            // A mouse gesture's title is a sentence: the column stops at 32.
            let name_w = name_max.min((usize::from(w) - margin) * 2 / 5).min(32);
            (name_w, key_w)
        } else if w >= NAME_ONLY {
            let key_w = key_max.min(usize::from(w) / 3);
            (usize::from(w).saturating_sub(margin + key_w + 1), key_w)
        } else {
            (usize::from(w).saturating_sub(margin), 0)
        };
        let desc_x = margin + name_w + 1 + key_w + 2;
        let elsewhere = self.items.iter().position(|it| *it == Item::Elsewhere);
        if self.items.is_empty() {
            frame.buffer_mut().set_line(
                list.x,
                list.y,
                &Line::styled("  no command matches", Style::new().fg(th.fg_dim)),
                list.width,
            );
        }
        for (i, it) in self
            .items
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(self.height)
        {
            let y = list.y + (i - self.scroll) as u16;
            let buf = frame.buffer_mut();
            match it {
                Item::Heading(h) => {
                    buf.set_line(
                        list.x,
                        y,
                        &Line::styled(
                            format!(" {h}"),
                            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
                        ),
                        list.width,
                    );
                }
                Item::Text(t) => {
                    buf.set_line(
                        list.x,
                        y,
                        &Line::styled(
                            cut(&format!("  {t}"), usize::from(w)),
                            Style::new().fg(th.fg_dim),
                        ),
                        list.width,
                    );
                }
                Item::Elsewhere => {
                    let label = " elsewhere ";
                    let side = usize::from(w).saturating_sub(label.len() + 2) / 2;
                    buf.set_line(
                        list.x,
                        y,
                        &Line::styled(
                            format!(
                                "  {}{label}{}",
                                "\u{2500}".repeat(side.min(3)),
                                "\u{2500}".repeat(side.saturating_sub(3).min(usize::from(w)))
                            ),
                            Style::new().fg(th.fg_dim),
                        ),
                        list.width,
                    );
                }
                Item::Row { row, hits, tagged } => {
                    let r = &self.rows[*row];
                    let picked = self.sel == Some(i);
                    let dim =
                        r.read_only() || r.refused.is_some() || elsewhere.is_some_and(|e| i > e);
                    let bg = if picked {
                        th.row_cursor_active
                    } else {
                        th.bg_panel
                    };
                    buf.set_style(Rect::new(list.x, y, list.width, 1), Style::new().bg(bg));
                    let base = Style::new().fg(if dim { th.fg_dim } else { th.fg });
                    let mut spans = vec![
                        Span::styled(
                            self.number(i).map_or("  ".to_string(), |n| format!("{n} ")),
                            Style::new().fg(th.fg_dim),
                        ),
                        Span::styled(
                            if r.rebound { "\u{2022} " } else { "  " },
                            Style::new().fg(th.mark),
                        ),
                    ];
                    let name = if *tagged { &r.name } else { &r.title };
                    let shown = cut(name, name_w);
                    // Cut: the `…` is not a matched letter.
                    let cut_at = if shown != *name {
                        shown.chars().count().saturating_sub(1)
                    } else {
                        usize::MAX
                    };
                    for (ci, c) in shown.chars().enumerate() {
                        let hit = ci < cut_at && hits.contains(&ci);
                        let st = if hit {
                            Style::new().fg(th.accent).add_modifier(Modifier::BOLD)
                        } else if picked {
                            base.add_modifier(Modifier::BOLD)
                        } else {
                            base
                        };
                        spans.push(Span::styled(c.to_string(), st));
                    }
                    let pad = name_w.saturating_sub(width(&shown));
                    if key_w > 0 {
                        spans.push(Span::raw(" ".repeat(pad + 1)));
                        let k = cut(&r.key_text, key_w);
                        let kst = if r.read_only() {
                            Style::new().fg(th.fg_dim)
                        } else {
                            Style::new().fg(th.fg).add_modifier(Modifier::BOLD)
                        };
                        let kpad = key_w.saturating_sub(width(&k));
                        spans.push(Span::styled(k, kst));
                        spans.push(Span::raw(" ".repeat(kpad)));
                    }
                    if w >= THREE_COLUMNS {
                        spans.push(Span::raw("  "));
                        let room = usize::from(w).saturating_sub(desc_x);
                        let (text, st) = match &r.refused {
                            Some(why) => (why.clone(), Style::new().fg(th.fg_dim)),
                            None => (
                                self.description(*row),
                                Style::new().fg(if dim { th.fg_dim } else { th.fg }),
                            ),
                        };
                        spans.push(Span::styled(cut(&text, room), st));
                    }
                    buf.set_line(list.x, y, &Line::from(spans), list.width);
                }
            }
        }

        // The last line: why a row cannot run; else the selected row's description
        // where it is not on its row; else the keys.
        let y = inner.y + inner.height - 1;
        let buf = frame.buffer_mut();
        buf.set_style(
            Rect::new(inner.x, y, inner.width, 1),
            Style::new().bg(th.bg_bar),
        );
        let (left, right) = self.last_line(w, desc_x, name_w);
        let st = if self.message.is_some() {
            Style::new().fg(th.err)
        } else {
            Style::new().fg(th.fg_dim)
        };
        let rw = width(&right);
        let room = usize::from(w).saturating_sub(if rw > 0 { rw + 3 } else { 1 });
        let left = cut(&format!(" {left}"), room);
        let gap = usize::from(w).saturating_sub(width(&left) + rw + 1);
        let mut spans = vec![Span::styled(left, st)];
        if rw > 0 && usize::from(w) > rw + 12 {
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(right, Style::new().fg(th.fg_dim)));
        }
        buf.set_line(inner.x, y, &Line::from(spans), inner.width);
    }

    /// A row's description column: what it does, and how to reach it from the shell.
    fn description(&self, row: usize) -> String {
        let r = &self.rows[row];
        match &r.note {
            Some(n) if r.about.is_empty() => n.clone(),
            Some(n) => format!("{} \u{b7} {n}", r.about),
            None => r.about.clone(),
        }
    }

    /// The last line: on the left why a row cannot run, else the selected row's
    /// description where its row has no room for it, else the keys; on the right what
    /// the left leaves out (F1 larger, or the `[keys]` name to rebind it by).
    fn last_line(&self, w: u16, desc_x: usize, name_w: usize) -> (String, String) {
        let grow = if self.size == Size::Small && self.can_grow() {
            "F1 larger"
        } else {
            ""
        };
        if let Some(m) = &self.message {
            return (m.clone(), String::new());
        }
        let sel = self.selected();
        let full = sel.map(|row| match &self.rows[row].refused {
            Some(why) => why.clone(),
            None => self.description(row),
        });
        // A row with no description whose name is a sentence (the viewer's and the
        // mouse's) cut on its row: the sentence, whole.
        if let Some(r) = sel.map(|row| &self.rows[row]) {
            let shown = match self.items.get(self.sel.unwrap_or(0)) {
                Some(Item::Row { tagged: true, .. }) => width(&r.name),
                _ => width(&r.title),
            };
            if r.about.is_empty() && shown > name_w {
                return (r.title.clone(), grow.to_string());
            }
        }
        // Narrow: the description has no column; or it was cut on its row.
        let cut_on_row = full
            .as_ref()
            .is_some_and(|d| width(d) > usize::from(w).saturating_sub(desc_x));
        if let Some(d) = full.filter(|d| !d.is_empty() && (w < THREE_COLUMNS || cut_on_row)) {
            return (d, grow.to_string());
        }
        let mut keys = vec!["Enter run", "\u{2191}\u{2193} move"];
        if !grow.is_empty() {
            keys.push(grow);
        }
        keys.push("Esc close");
        let hint = keys.join(" \u{b7} ");
        // Where to rebind it: the name `[keys]` knows it by.
        let name = sel
            .and_then(|row| self.rows[row].keys_name)
            .filter(|_| self.size == Size::Large)
            .map(|n| format!("[keys] {n}"))
            .unwrap_or_default();
        (hint, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(keymap: &Keymap) -> Vec<Row> {
        rows(keymap, &[], |_| None)
    }

    fn first_title(rows: &[Row], items: &[Item]) -> String {
        match items.first() {
            Some(Item::Row { row, .. }) => rows[*row].title.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn titles(rows: &[Row], items: &[Item]) -> Vec<String> {
        items
            .iter()
            .filter_map(|it| match it {
                Item::Row { row, .. } => Some(rows[*row].title.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn every_binding_has_a_title_and_titles_are_unique() {
        let mut seen = HashSet::new();
        for b in keys::BINDINGS {
            assert!(!b.title.is_empty(), "{} has no title", b.name);
            assert!(
                seen.insert(b.title),
                "two bindings are called {:?}",
                b.title
            );
        }
        // And every binding is a row, once.
        let r = all(&Keymap::default());
        for b in keys::BINDINGS {
            assert_eq!(
                r.iter().filter(|r| r.keys_name == Some(b.name)).count(),
                1,
                "{}",
                b.name
            );
        }
    }

    #[test]
    fn every_action_has_a_place_and_only_help_opens_from_the_shell() {
        for b in keys::BINDINGS {
            // The match is exhaustive; this says the answer is a real one.
            let _ = b.action.place();
            assert_eq!(b.action.opens_from_shell(), b.action == Action::Help);
            // F12's family stays the only keys taken inside the shell.
            assert!(!b.action.opens_from_shell() || !b.action.is_global());
        }
        assert_eq!(Action::Copy.place(), Place::Files);
        assert_eq!(Action::NewTerminal.place(), Place::Terminal);
        assert_eq!(Action::SplitRight.place(), Place::Everywhere);
    }

    #[test]
    fn a_key_typed_as_text_ranks_its_command_first() {
        let r = all(&Keymap::default());
        for (q, want) in [
            ("f5", "Copy"),
            ("F5", "Copy"),
            ("alt+j", "History and lists\u{2026}"),
            ("Shift+F6", "Rename"),
            ("alt+x f6", "Rename"),
        ] {
            assert_eq!(
                first_title(&r, &rank(&r, q, Place::Files, &[])),
                want,
                "{q}"
            );
        }
        // One letter is the start of a word, not a key: "o" is not Open with's key.
        assert!(as_key("o").is_none() && as_key("f5").is_some());
        assert_eq!(first_title(&r, &rank(&r, "o", Place::Files, &[])), "Open");
    }

    #[test]
    fn a_rebound_key_is_found_by_its_new_key_not_its_old_one() {
        let mut km = Keymap::default();
        let t: std::collections::BTreeMap<String, Vec<String>> =
            [("copy".to_string(), vec!["F2".to_string()])].into();
        km.apply(&t).unwrap();
        let r = all(&km);
        let copy = r.iter().find(|r| r.title == "Copy").unwrap();
        assert!(copy.rebound && copy.key_text == "F2");
        assert_eq!(first_title(&r, &rank(&r, "f2", Place::Files, &[])), "Copy");
        assert_ne!(first_title(&r, &rank(&r, "f5", Place::Files, &[])), "Copy");
    }

    #[test]
    fn a_title_match_ranks_above_a_description_match() {
        let r = all(&Keymap::default());
        // "trash" is a word only in Delete's description. Its letters are scattered
        // through "Terminal: Show or hide terminal", which comes after it.
        let t = titles(&r, &rank(&r, "trash", Place::Files, &[]));
        assert_eq!(t.first().map(String::as_str), Some("Delete"), "{t:?}");
        let show = t.iter().position(|x| x == "Show or hide terminal");
        assert!(show.is_some_and(|p| p > 0), "{t:?}");
        // "copy": every name that has the word comes before every description that
        // has it.
        let items = rank(&r, "copy", Place::Files, &[]);
        let named: Vec<bool> = items
            .iter()
            .map(|it| match it {
                Item::Row { row, .. } => r[*row].name.to_lowercase().contains("copy"),
                _ => unreachable!(),
            })
            .collect();
        let first_desc = named.iter().position(|n| !n).unwrap_or(named.len());
        assert!(first_desc >= 2, "{named:?}");
        assert!(named[first_desc..].iter().all(|n| !n), "{named:?}");
        assert_eq!(first_title(&r, &items), "Copy");
    }

    #[test]
    fn a_groups_name_finds_its_commands() {
        let r = all(&Keymap::default());
        let items = rank(&r, "sort", Place::Files, &[]);
        let t = titles(&r, &items);
        for want in [
            "Sort by name",
            "Sort by extension",
            "Sort by size",
            "Sort by date",
        ] {
            assert!(t[..6].contains(&want.to_string()), "{t:?}");
        }
        let items = rank(&r, "mouse", Place::Files, &[]);
        let groups: Vec<&str> = items
            .iter()
            .take(14)
            .map(|it| match it {
                Item::Row { row, .. } => r[*row].group.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert!(groups.iter().all(|g| *g == "Mouse"), "{groups:?}");
    }

    #[test]
    fn a_command_ranks_above_a_read_only_row_that_matches_as_well() {
        let r = all(&Keymap::default());
        for ctx in [Place::Files, Place::Terminal] {
            assert_eq!(
                first_title(&r, &rank(&r, "copy", ctx, &[])),
                "Copy",
                "{ctx:?}"
            );
        }
    }

    #[test]
    fn in_the_viewer_its_own_rows_come_first_and_close_pane_is_not_first() {
        let r = all(&Keymap::default());
        let t = titles(&r, &rank(&r, "close", Place::Viewer, &[]));
        assert_eq!(t[0], "Close the viewer", "{t:?}");
        let items = rank(&r, "copy", Place::Viewer, &[]);
        let Item::Row { row, .. } = &items[0] else {
            panic!()
        };
        assert_eq!(r[*row].group, "Viewer");
        // Elsewhere, Close pane is found as before.
        let t = titles(&r, &rank(&r, "close", Place::Files, &[]));
        assert!(t[..2].contains(&"Close pane".to_string()), "{t:?}");
    }

    #[test]
    fn case_is_smart_as_in_the_quick_filter() {
        let r = all(&Keymap::default());
        assert!(!rank(&r, "copy", Place::Files, &[]).is_empty());
        // A capital asks for that case: "COPY" is nowhere.
        assert!(rank(&r, "COPY", Place::Files, &[]).is_empty());
    }

    #[test]
    fn ties_go_to_recent_then_here_then_table_order() {
        let r = all(&Keymap::default());
        // The four sorts score alike for "sort by": table order first...
        let plain = titles(&r, &rank(&r, "sort by", Place::Files, &[]));
        assert_eq!(&plain[..2], ["Sort by name", "Sort by extension"]);
        // ...unless one was run recently.
        let recent = vec!["sort_size".to_string()];
        let t = titles(&r, &rank(&r, "sort by", Place::Files, &recent));
        assert_eq!(t[0], "Sort by size");
        // Where F1 was pressed beats table order: "next" from the terminal puts the
        // terminal's tab and split commands before the file pane's Next file pane.
        let t = titles(&r, &rank(&r, "next", Place::Terminal, &[]));
        let pane = t.iter().position(|x| x == "Next file pane").unwrap();
        let tab = t.iter().position(|x| x == "Next tab").unwrap();
        let tf = titles(&r, &rank(&r, "next", Place::Files, &[]));
        assert!(
            tab < pane || tf.iter().position(|x| x == "Next tab") == Some(tab),
            "{t:?}"
        );
    }

    #[test]
    fn the_small_list_starts_with_recent_then_here_then_elsewhere() {
        let r = all(&Keymap::default());
        let recent = vec!["new_terminal".to_string(), "no_such_command".to_string()];
        let items = browse_small(&r, Place::Files, &recent);
        assert_eq!(first_title(&r, &items), "New terminal", "recent first");
        let cut = items.iter().position(|i| *i == Item::Elsewhere).unwrap();
        for it in &items[1..cut] {
            let Item::Row { row, .. } = it else { panic!() };
            assert!(
                r[*row].here(Place::Files) && !r[*row].read_only(),
                "{}",
                r[*row].title
            );
        }
        // Everything is in it once.
        assert_eq!(items.len() - 1, r.len());
        // From the shell the terminal's commands are here, the files' elsewhere.
        let items = browse_small(&r, Place::Terminal, &[]);
        let cut = items.iter().position(|i| *i == Item::Elsewhere).unwrap();
        let here = titles(&r, &items[..cut]);
        assert!(here.contains(&"New terminal".to_string()));
        assert!(!here.contains(&"Copy".to_string()));
        // In the viewer, the viewer's own keys.
        let items = browse_small(&r, Place::Viewer, &[]);
        let Item::Row { row, .. } = &items[0] else {
            panic!()
        };
        assert_eq!(r[*row].group, "Viewer");
    }

    #[test]
    fn a_row_says_how_to_reach_it_from_the_shell() {
        let r = all(&Keymap::default());
        let note = |name: &str| {
            r.iter()
                .find(|r| r.keys_name == Some(name))
                .and_then(|r| r.note.clone())
        };
        assert_eq!(
            note("focus_terminal").as_deref(),
            Some("works in the shell")
        );
        assert_eq!(
            note("split_right").as_deref(),
            Some("in the shell: Alt+x first")
        );
        // Alt+R's group cannot be opened after Alt+X: the key that works is named.
        assert_eq!(
            note("resize_left").as_deref(),
            Some("in the shell: Alt+x Alt+Shift+Left")
        );
        // A pane key says nothing: inside the shell it is the program's.
        assert_eq!(note("go_shell_dir"), None);
        // F1 itself: Alt+X F1 there.
        assert_eq!(note("help").as_deref(), Some("in the shell: Alt+x F1"));
    }

    #[test]
    fn the_large_list_is_every_group_under_its_name() {
        let r = all(&Keymap::default());
        let items = browse_large(&r);
        let heads: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                Item::Heading(h) => Some(h.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(heads[0], "General");
        for g in ["Files", "Terminal", "Viewer", "Mouse"] {
            assert_eq!(heads.iter().filter(|h| **h == g).count(), 1, "{g}");
        }
    }

    #[test]
    fn read_only_rows_never_run_and_refused_rows_say_why() {
        let r = rows(&Keymap::default(), &[], |run| {
            (*run == Run::Act(Action::Copy)).then(|| "read-only mode".to_string())
        });
        for row in r
            .iter()
            .filter(|r| matches!(r.group.as_str(), "Mouse" | "Viewer"))
        {
            assert!(row.read_only(), "{}", row.title);
        }
        // Go to tab N needs its digit: shown, with its keys, but not run from here.
        let go = r.iter().find(|r| r.keys_name == Some("go_tab")).unwrap();
        assert!(go.read_only());
        assert_eq!(go.key_text, "Alt+1 \u{2026} Alt+9");
        let copy = r.iter().find(|r| r.title == "Copy").unwrap();
        assert_eq!(copy.refused.as_deref(), Some("read-only mode"));
    }

    #[test]
    fn your_menu_commands_are_rows_with_their_key() {
        let menu = vec![MenuItem {
            key: "e".into(),
            label: "say it".into(),
            command: "echo %s".into(),
        }];
        let r = rows(&Keymap::default(), &menu, |_| None);
        let row = r.iter().find(|r| r.title == "say it").unwrap();
        assert_eq!(row.group, "Your commands");
        assert_eq!(row.key_text, "Shift+F9 e");
        assert_eq!(row.run, Run::User(0));
        assert_eq!(
            first_title(&r, &rank(&r, "say", Place::Files, &[])),
            "say it"
        );
    }

    #[test]
    fn menu_only_commands_are_rows_too() {
        let r = all(&Keymap::default());
        for t in [
            "Settings\u{2026}",
            "Theme\u{2026}",
            "Config in use",
            "About xdevcmd",
        ] {
            assert!(r.iter().any(|r| r.title == t), "{t}");
        }
    }

    /// The list is built once per open and ranked again on every key: 2,000 rows (about
    /// fifteen times F1's) must rank faster than a frame. The budget is for a release
    /// build (0.28 ms measured on x86_64); a debug build gets ten times the room, since
    /// CI's ARM runner took 7.3 ms in debug (run 43212193039) for work that is fine.
    #[test]
    fn ranking_two_thousand_rows_takes_under_a_frame() {
        let base = all(&Keymap::default());
        let mut many = Vec::new();
        while many.len() < 2000 {
            for r in &base {
                many.push(Row {
                    title: r.title.clone(),
                    group: r.group.clone(),
                    keys: r.keys.clone(),
                    key_text: r.key_text.clone(),
                    about: r.about.clone(),
                    note: None,
                    place: r.place,
                    run: r.run.clone(),
                    rebound: false,
                    refused: None,
                    keys_name: r.keys_name,
                    id: r.id.clone(),
                    name: r.name.clone(),
                    parsed: r.parsed.clone(),
                    hay: r.hay.clone(),
                });
            }
        }
        many.truncate(2000);
        // The best of a few runs: the time asked about is the work's, not the
        // scheduler's on a loaded machine.
        let best = (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                for q in ["c", "cop", "files", "alt+j", "zzzz"] {
                    std::hint::black_box(rank(&many, q, Place::Files, &[]));
                }
                t.elapsed() / 5
            })
            .min()
            .unwrap();
        eprintln!("ranking 2,000 rows: {best:?} per query");
        let budget_ms = if cfg!(debug_assertions) { 50 } else { 5 };
        assert!(
            best.as_millis() < budget_ms,
            "{best:?} per query, budget {budget_ms} ms"
        );
    }

    fn palette(screen: (u16, u16)) -> Commands {
        let km = Keymap::default();
        let rows = rows(&km, &[], |r| {
            (*r == Run::Act(Action::Delete)).then(|| "read-only mode".to_string())
        });
        Commands::new(
            rows,
            Place::Files,
            Vec::new(),
            Vec::new(),
            Size::Small,
            ratatui::layout::Rect::new(0, 0, screen.0, screen.1),
        )
    }

    fn press(c: &mut Commands, code: ratatui::crossterm::event::KeyCode) -> Outcome {
        c.on_key(ratatui::crossterm::event::KeyEvent::new(
            code,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn typed(c: &mut Commands, text: &str) {
        for ch in text.chars() {
            press(c, ratatui::crossterm::event::KeyCode::Char(ch));
        }
    }

    fn title_of(c: &Commands, o: &Outcome) -> String {
        match o {
            Outcome::Run(row) => c.rows[*row].title.clone(),
            _ => "(not run)".into(),
        }
    }

    /// The owner's question (Round 033): F1 opened from inside F1 never loops. F1's own
    /// row switches the size and stays; F1 the key grows, then closes; nothing opens a
    /// second window.
    #[test]
    fn f1_inside_f1_switches_size_and_never_runs_itself() {
        use ratatui::crossterm::event::KeyCode;
        let help_item = |c: &Commands| {
            c.items
                .iter()
                .position(|it| {
                    matches!(it, Item::Row { row, .. } if c.rows[*row].run == Run::Act(Action::Help))
                })
                .expect("F1's row is listed")
        };
        let mut c = palette((118, 34));
        assert!(matches!(press(&mut c, KeyCode::F(1)), Outcome::Stay));
        assert_eq!(c.size, Size::Large);
        let i = help_item(&c);
        assert!(matches!(c.choose(i), Outcome::Stay));
        assert_eq!(c.size, Size::Small);
        let i = help_item(&c);
        assert!(matches!(c.choose(i), Outcome::Stay));
        assert_eq!(c.size, Size::Large);
        assert!(matches!(press(&mut c, KeyCode::F(1)), Outcome::Close));

        // Too small a screen for the large size: it says so and stays.
        let mut c = palette((60, 16));
        let i = help_item(&c);
        assert!(matches!(c.choose(i), Outcome::Stay));
        assert_eq!(c.size, Size::Small);
        assert!(c.message.as_deref().unwrap_or("").contains("too small"));
    }

    #[test]
    fn digits_are_text_and_alt_digits_run_that_row() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut c = palette((118, 34));
        typed(&mut c, "sort");
        // Alt+2: the second row, whatever is selected.
        let o = c.on_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT));
        assert_eq!(title_of(&c, &o), "Sort by extension");
        // A digit alone goes into the filter.
        let mut c = palette((118, 34));
        typed(&mut c, "f5");
        assert_eq!(c.edit.text(), "f5");
        let o = press(&mut c, KeyCode::Enter);
        assert_eq!(title_of(&c, &o), "Copy");
    }

    #[test]
    fn esc_clears_then_closes_and_f1_grows_keeping_the_text() {
        use ratatui::crossterm::event::KeyCode;
        let mut c = palette((118, 34));
        typed(&mut c, "sort");
        press(&mut c, KeyCode::Down);
        let picked = c.selected();
        assert!(matches!(press(&mut c, KeyCode::F(1)), Outcome::Stay));
        assert_eq!(c.size, Size::Large);
        assert_eq!(c.edit.text(), "sort");
        assert_eq!(c.selected(), picked, "the same row selected");
        assert!(matches!(press(&mut c, KeyCode::Esc), Outcome::Stay));
        assert_eq!(c.edit.text(), "");
        assert!(matches!(press(&mut c, KeyCode::Esc), Outcome::Close));
        // Large closes on F1; a small screen's small one does too.
        assert!(matches!(press(&mut c, KeyCode::F(1)), Outcome::Close));
        let mut c = palette((80, 24));
        assert!(matches!(press(&mut c, KeyCode::F(1)), Outcome::Close));
    }

    #[test]
    fn enter_on_a_row_that_cannot_run_says_why_and_stays() {
        use ratatui::crossterm::event::KeyCode;
        let mut c = palette((118, 34));
        typed(&mut c, "f8");
        assert!(matches!(press(&mut c, KeyCode::Enter), Outcome::Stay));
        assert_eq!(c.message.as_deref(), Some("read-only mode"));
        let mut c = palette((118, 34));
        typed(&mut c, "wheel");
        assert!(matches!(press(&mut c, KeyCode::Enter), Outcome::Stay));
        assert!(c.message.as_deref().unwrap().contains("a mouse gesture"));
        // The next key clears it.
        press(&mut c, KeyCode::Down);
        assert_eq!(c.message, None);
    }

    #[test]
    fn recent_keeps_eight_most_recent_first_and_drops_the_unknown() {
        let mut r = Vec::new();
        for id in ["copy", "move", "copy", "a", "b", "c", "d", "e", "f", "g"] {
            remember(&mut r, id);
        }
        assert_eq!(r.len(), RECENT);
        assert_eq!(r[0], "g");
        assert_eq!(r.iter().filter(|x| *x == "copy").count(), 1, "once");
        assert!(!r.contains(&"move".to_string()), "the oldest pushed out");
        let menu = vec![MenuItem {
            key: "e".into(),
            label: "say it".into(),
            command: "echo".into(),
        }];
        assert!(known("copy", &menu) && known("about", &menu) && known("menu:say it", &menu));
        assert!(!known("coppy", &menu) && !known("menu:gone", &menu));
    }

    /// Opening F1 builds its rows from the keymap and the config, then lists them:
    /// measured on the real list (128 rows: 87 bindings, 4 menu-only commands, 37
    /// read-only) plus twenty `[[menu]]` entries.
    #[test]
    fn opening_f1_takes_well_under_a_frame() {
        let km = Keymap::default();
        let menu: Vec<MenuItem> = (0..20)
            .map(|i| MenuItem {
                key: String::new(),
                label: format!("command {i}"),
                command: "echo %f".into(),
            })
            .collect();
        let best = (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                let r = rows(&km, &menu, |_| None);
                let n = r.len();
                let c = Commands::new(
                    r,
                    Place::Files,
                    vec!["copy".into()],
                    Vec::new(),
                    Size::Small,
                    ratatui::layout::Rect::new(0, 0, 118, 34),
                );
                std::hint::black_box(&c);
                (t.elapsed(), n)
            })
            .min()
            .unwrap();
        eprintln!("opening F1 with {} rows: {:?}", best.1, best.0);
        assert!(best.1 >= 140, "{}", best.1);
        assert!(best.0.as_millis() < 16, "{:?}", best.0);
    }

    #[test]
    fn a_recent_file_keeps_each_name_once_and_a_huge_one_is_ignored() {
        let t = "recent = [\"copy\", \"move\", \"copy\", \"nope\", \"move\", \"view\"]\n";
        assert_eq!(parse_recent(t, &[]), ["copy", "move", "view"]);
        assert!(parse_recent("not toml at all [", &[]).is_empty());
        // Over the cap the file is not read at all.
        let dir = std::env::temp_dir().join(format!("xdevcmd-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = format!("recent = [\"copy\"]\n#{}\n", "x".repeat(70 * 1024));
        std::fs::write(dir.join("commands.toml"), big).unwrap();
        let p = dir.join("commands.toml");
        assert!(
            load_recent_from(&p, &[]).is_empty(),
            "64 KB and more: ignored"
        );
        std::fs::write(&p, "recent = [\"copy\"]\n").unwrap();
        assert_eq!(load_recent_from(&p, &[]), ["copy"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cut_counts_columns_so_wide_characters_fit() {
        // Six CJK characters are twelve columns: cut to seven, five fit with the `…`.
        let c = cut("日本語のコマ", 7);
        assert_eq!(c, "日本語\u{2026}");
        assert!(width(&c) <= 7);
        assert_eq!(cut("日本語", 6), "日本語");
        assert_eq!(cut("copy", 3), "co\u{2026}");
    }
}
