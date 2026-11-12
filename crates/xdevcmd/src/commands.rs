//! F1, Commands: every command, found by typing, with its key beside it (Round 024).
//!
//! The rows are built from the live keymap, the `[[menu]]` entries and the mode in
//! effect each time F1 opens, never from a hand-written list: a key rebound in
//! `config.toml` shows up here as rebound, and a command the mode forbids says why.
//! Ranking reuses the quick filter's fuzzy score (`xdevcmd_core::fuzzy`), so typing
//! here feels as it does in a pane.

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::HashSet;

use crate::config::MenuItem;
use crate::keys::{self, Action, Keymap, Place};

/// Known problems by system, and what to do; F1 points there rather than repeating it.
const TROUBLESHOOTING: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/main/docs/troubleshooting.md"
);

/// Commands with no key and no `Action`: they live in the menus only, and are rows
/// here too, or "every command by typing" would not be true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    Settings,
    Theme,
    About,
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
            });
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
                });
            }
        }
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
            });
        }
    }
    for r in &mut out {
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
/// Within a tier by score; ties: recently run first, then the rows for where F1 was
/// pressed, then table order. A description matched only by scattered letters is not a
/// match: in a sentence, almost anything is.
pub fn rank(rows: &[Row], query: &str, ctx: Place, recent: &[String]) -> Vec<Item> {
    use xdevcmd_core::fuzzy;
    let key = as_key(query);
    let q = fuzzy::Query::new(query);
    let mut found: Vec<(u8, i32, usize, bool, usize, Vec<usize>)> = Vec::new();
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
        } else if let Some((s, hits)) = q.score_positions(&r.about) {
            if !fuzzy::is_words(&r.about, &hits) {
                continue;
            }
            (2, s, Vec::new())
        } else {
            continue;
        };
        found.push((tier, score, recency(recent, &r.id), !r.here(ctx), i, hits));
    }
    found.sort_by_key(|f| (f.0, std::cmp::Reverse(f.1), f.2, f.3, f.4));
    found
        .into_iter()
        .map(|(_, _, _, _, row, hits)| Item::Row {
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
    for (i, r) in rows.iter().enumerate() {
        let wanted = if ctx == Place::Viewer {
            r.place == Place::Viewer
        } else {
            r.here(ctx) && !r.read_only()
        };
        if wanted && !shown[i] {
            shown[i] = true;
            out.push(item(i));
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
        for t in ["Settings\u{2026}", "Theme\u{2026}", "About xdevcmd"] {
            assert!(r.iter().any(|r| r.title == t), "{t}");
        }
    }

    /// The list is built once per open and ranked again on every key: 2,000 rows (ten
    /// times F1's) must rank faster than a frame, in a debug build.
    #[test]
    fn ranking_two_thousand_rows_takes_under_5_ms() {
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
        assert!(best.as_millis() < 5, "{best:?} per query");
    }
}
