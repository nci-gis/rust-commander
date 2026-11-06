//! Key bindings: what a key means, rather than what a key is.
//!
//! Every binding goes through [`Action`], so `on_key` never matches a raw `KeyCode` and
//! the config file can move any key without touching the dispatch.

use std::collections::HashMap;
use std::fmt::Write as _;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    Help,
    Refresh,
    ToggleHidden,
    FocusNextPane,
    FocusTerminal,
    ToggleTerminal,
    CdTerminalHere,
    NewTerminal,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    NextSplit,
    SplitRight,
    SplitDown,
    ClosePane,
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    Zoom,
    CursorUp,
    CursorDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Open,
    EnterDir,
    Parent,
    Mark,
    MarkAndNext,
    InvertMarks,
    MarkAll,
    UnmarkAll,
    View,
    Edit,
    Copy,
    Move,
    Rename,
    MkDir,
    Delete,
    DeletePermanent,
    SortName,
    SortExt,
    SortSize,
    SortModified,
    QuickSearch,
    Filter,
    GoToPath,
    Back,
    Forward,
    History,
    Hotlist,
    OpenWith,
    QuitNoCd,
    SwapPanes,
    MirrorPane,
    MarkPattern,
    UnmarkPattern,
    CalcSizes,
    FindFiles,
    BulkRename,
    QuickView,
    NewFile,
    CopyAs,
    Symlink,
    Permissions,
    Checksum,
    UserMenu,
    Pack,
    Unpack,
    CompareDirs,
    SyncDirs,
    MenuBar,
    Usage,
}

/// One binding: its config name, where the help screen lists it, what it does, and its
/// default keys. Everything about a binding lives in this one row, so the config parser,
/// the defaults and the help screen cannot disagree.
pub struct Binding {
    pub action: Action,
    pub name: &'static str,
    pub group: &'static str,
    pub about: &'static str,
    pub keys: &'static [&'static str],
}

/// Names that used to exist. `--print-config` wrote every action name into people's
/// config files, so renaming one without an alias would make the whole file be
/// rejected on upgrade.
const ALIASES: &[(&str, Action)] = &[
    ("grow_pane", Action::ResizeRight),
    ("shrink_pane", Action::ResizeLeft),
    ("next_terminal", Action::NextTab),
    ("prev_terminal", Action::PrevTab),
];

pub const BINDINGS: &[Binding] = &[
    Binding {
        action: Action::Quit,
        name: "quit",
        group: "General",
        about: "Quit rcmd",
        keys: &["F10", "q", "Ctrl+q"],
    },
    Binding {
        action: Action::Help,
        name: "help",
        group: "General",
        about: "This screen: every key and setting",
        keys: &["F1"],
    },
    Binding {
        action: Action::Refresh,
        name: "refresh",
        group: "General",
        about: "Re-read the active pane",
        keys: &["Ctrl+r"],
    },
    Binding {
        action: Action::ToggleHidden,
        name: "toggle_hidden",
        group: "General",
        about: "Show or hide hidden files",
        keys: &["Ctrl+h"],
    },
    Binding {
        action: Action::CursorUp,
        name: "cursor_up",
        group: "Navigate",
        about: "Move up",
        keys: &["Up"],
    },
    Binding {
        action: Action::CursorDown,
        name: "cursor_down",
        group: "Navigate",
        about: "Move down",
        keys: &["Down"],
    },
    Binding {
        action: Action::PageUp,
        name: "page_up",
        group: "Navigate",
        about: "Page up",
        keys: &["PageUp"],
    },
    Binding {
        action: Action::PageDown,
        name: "page_down",
        group: "Navigate",
        about: "Page down",
        keys: &["PageDown"],
    },
    Binding {
        action: Action::Top,
        name: "top",
        group: "Navigate",
        about: "First entry",
        keys: &["Home"],
    },
    Binding {
        action: Action::Bottom,
        name: "bottom",
        group: "Navigate",
        about: "Last entry",
        keys: &["End"],
    },
    Binding {
        action: Action::Open,
        name: "open",
        group: "Navigate",
        about: "Enter a directory, or view a file",
        keys: &["Enter"],
    },
    Binding {
        action: Action::EnterDir,
        name: "enter_dir",
        group: "Navigate",
        about: "Enter the directory under the cursor",
        keys: &["Alt+Right"],
    },
    Binding {
        action: Action::Parent,
        name: "parent",
        group: "Navigate",
        about: "Go up one directory",
        keys: &["Backspace", "Alt+Left"],
    },
    Binding {
        action: Action::Mark,
        name: "mark",
        group: "Select",
        about: "Mark or unmark the entry",
        keys: &["Space"],
    },
    Binding {
        action: Action::MarkAndNext,
        name: "mark_and_next",
        group: "Select",
        about: "Mark, then move down",
        keys: &["Insert"],
    },
    Binding {
        action: Action::InvertMarks,
        name: "invert_marks",
        group: "Select",
        about: "Invert all marks",
        keys: &["*"],
    },
    Binding {
        action: Action::MarkAll,
        name: "mark_all",
        group: "Select",
        about: "Mark everything",
        keys: &["Ctrl+a"],
    },
    Binding {
        action: Action::UnmarkAll,
        name: "unmark_all",
        group: "Select",
        about: "Clear all marks",
        keys: &["Alt+u"],
    },
    Binding {
        action: Action::View,
        name: "view",
        group: "Files",
        about: "View the file",
        keys: &["F3"],
    },
    Binding {
        action: Action::Edit,
        name: "edit",
        group: "Files",
        about: "Edit the file in your editor",
        keys: &["F4"],
    },
    Binding {
        action: Action::Copy,
        name: "copy",
        group: "Files",
        about: "Copy to the other pane",
        keys: &["F5"],
    },
    Binding {
        action: Action::Move,
        name: "move",
        group: "Files",
        about: "Move to the other pane",
        keys: &["F6"],
    },
    Binding {
        action: Action::Rename,
        name: "rename",
        group: "Files",
        about: "Rename the entry under the cursor",
        keys: &["Shift+F6", "F2"],
    },
    Binding {
        action: Action::MkDir,
        name: "mkdir",
        group: "Files",
        about: "Create a directory",
        keys: &["F7"],
    },
    Binding {
        action: Action::Delete,
        name: "delete",
        group: "Files",
        about: "Delete (to the trash, by default)",
        keys: &["F8"],
    },
    Binding {
        action: Action::DeletePermanent,
        name: "delete_permanent",
        group: "Files",
        about: "Delete permanently",
        keys: &["Shift+F8"],
    },
    Binding {
        action: Action::SortName,
        name: "sort_name",
        group: "Sort",
        about: "Sort by name (again to reverse)",
        keys: &["Ctrl+F3"],
    },
    Binding {
        action: Action::SortExt,
        name: "sort_ext",
        group: "Sort",
        about: "Sort by extension",
        keys: &["Ctrl+F4"],
    },
    Binding {
        action: Action::SortSize,
        name: "sort_size",
        group: "Sort",
        about: "Sort by size",
        keys: &["Ctrl+F5"],
    },
    Binding {
        action: Action::SortModified,
        name: "sort_modified",
        group: "Sort",
        about: "Sort by date",
        keys: &["Ctrl+F6"],
    },
    Binding {
        action: Action::FocusNextPane,
        name: "focus_next_pane",
        group: "Panes",
        about: "Next file pane",
        keys: &["Tab"],
    },
    Binding {
        action: Action::SplitRight,
        name: "split_right",
        group: "Panes",
        about: "Split the pane, or the shell, side by side",
        keys: &["Alt+s"],
    },
    Binding {
        action: Action::SplitDown,
        name: "split_down",
        group: "Panes",
        about: "Split the pane, or the shell, top and bottom",
        keys: &["Alt+d"],
    },
    Binding {
        action: Action::ClosePane,
        name: "close_pane",
        group: "Panes",
        about: "Close the pane, or the focused shell",
        keys: &["Alt+w"],
    },
    Binding {
        action: Action::ResizeLeft,
        name: "resize_left",
        group: "Panes",
        about: "Move the pane's border left",
        keys: &["Alt+Shift+Left"],
    },
    Binding {
        action: Action::ResizeRight,
        name: "resize_right",
        group: "Panes",
        about: "Move the pane's border right",
        keys: &["Alt+Shift+Right"],
    },
    Binding {
        action: Action::ResizeUp,
        name: "resize_up",
        group: "Panes",
        about: "Move the pane's border up",
        keys: &["Alt+Shift+Up"],
    },
    Binding {
        action: Action::ResizeDown,
        name: "resize_down",
        group: "Panes",
        about: "Move the pane's border down",
        keys: &["Alt+Shift+Down"],
    },
    Binding {
        action: Action::Zoom,
        name: "zoom",
        group: "Panes",
        about: "Fill the screen with this pane, or put it back",
        keys: &["Alt+z"],
    },
    Binding {
        action: Action::FocusTerminal,
        name: "focus_terminal",
        group: "Terminal",
        about: "Jump into or out of the shell",
        keys: &["F12"],
    },
    Binding {
        action: Action::ToggleTerminal,
        name: "toggle_terminal",
        group: "Terminal",
        about: "Show or hide the shell",
        keys: &["Alt+t"],
    },
    Binding {
        action: Action::CdTerminalHere,
        name: "cd_terminal_here",
        group: "Terminal",
        about: "cd the shell to this pane's directory",
        keys: &["Alt+c"],
    },
    Binding {
        action: Action::NewTerminal,
        name: "new_terminal",
        group: "Terminal",
        about: "Open a new terminal tab",
        keys: &["Alt+n"],
    },
    Binding {
        action: Action::NewTab,
        name: "new_tab",
        group: "Tabs",
        about: "New tab on the same directory",
        keys: &["Ctrl+t"],
    },
    Binding {
        action: Action::CloseTab,
        name: "close_tab",
        group: "Tabs",
        about: "Close the tab",
        keys: &["Ctrl+w"],
    },
    Binding {
        action: Action::NextTab,
        name: "next_tab",
        group: "Tabs",
        about: "Next tab, of the file pane or the terminal",
        keys: &["Ctrl+PageDown"],
    },
    Binding {
        action: Action::PrevTab,
        name: "prev_tab",
        group: "Tabs",
        about: "Previous tab, of the file pane or the terminal",
        keys: &["Ctrl+PageUp"],
    },
    Binding {
        action: Action::NextSplit,
        name: "next_split",
        group: "Panes",
        about: "Next split, in the shell or the file panes",
        keys: &["Alt+o"],
    },
    Binding {
        action: Action::QuitNoCd,
        name: "quit_no_cd",
        group: "General",
        about: "Quit, leaving the shell where it was (with --cwd-file)",
        keys: &["Q"],
    },
    Binding {
        action: Action::QuickSearch,
        name: "quick_search",
        group: "Navigate",
        about: "Jump to a name as you type it; Up/Down for the next match",
        keys: &["/", "Ctrl+s"],
    },
    Binding {
        action: Action::Filter,
        name: "filter",
        group: "Navigate",
        about: "Show only names matching what you type (*.rs works too)",
        keys: &["Ctrl+f"],
    },
    Binding {
        action: Action::GoToPath,
        name: "go_to_path",
        group: "Navigate",
        about: "Type a path to go to",
        keys: &["Ctrl+l"],
    },
    Binding {
        action: Action::Back,
        name: "back",
        group: "Navigate",
        about: "Back to the previous directory",
        keys: &["Alt+,"],
    },
    Binding {
        action: Action::Forward,
        name: "forward",
        group: "Navigate",
        about: "Forward again",
        keys: &["Alt+."],
    },
    Binding {
        action: Action::History,
        name: "history",
        group: "Navigate",
        about: "Directories visited in this tab",
        keys: &["Alt+Down"],
    },
    Binding {
        action: Action::Hotlist,
        name: "hotlist",
        group: "Navigate",
        about: "Bookmarks: go to one, or bookmark this directory",
        keys: &["Ctrl+d"],
    },
    Binding {
        action: Action::OpenWith,
        name: "open_with",
        group: "Files",
        about: "Open with the system's default application",
        keys: &["o"],
    },
    Binding {
        action: Action::CalcSizes,
        name: "calc_sizes",
        group: "Files",
        about: "Work out the size of every directory shown",
        keys: &["Ctrl+Space"],
    },
    Binding {
        action: Action::MarkPattern,
        name: "mark_pattern",
        group: "Select",
        about: "Mark by pattern, e.g. *.jpg *.png",
        keys: &["+"],
    },
    Binding {
        action: Action::UnmarkPattern,
        name: "unmark_pattern",
        group: "Select",
        about: "Unmark by pattern",
        keys: &["-"],
    },
    Binding {
        action: Action::SwapPanes,
        name: "swap_panes",
        group: "Panes",
        about: "Swap this pane with the other one",
        keys: &["Ctrl+u"],
    },
    Binding {
        action: Action::MirrorPane,
        name: "mirror_pane",
        group: "Panes",
        about: "Show this directory in the other pane too",
        keys: &["Alt+="],
    },
    Binding {
        action: Action::FindFiles,
        name: "find_files",
        group: "Files",
        about: "Find files by name and content; results open in a new tab",
        keys: &["Alt+F7"],
    },
    Binding {
        action: Action::BulkRename,
        name: "bulk_rename",
        group: "Files",
        about: "Rename the selection by editing the names in $EDITOR",
        keys: &["Shift+F2"],
    },
    Binding {
        action: Action::QuickView,
        name: "quick_view",
        group: "Panes",
        about: "The other pane previews the file under the cursor",
        keys: &["Alt+q"],
    },
    Binding {
        action: Action::NewFile,
        name: "new_file",
        group: "Files",
        about: "Create a file and open it in the editor",
        keys: &["Shift+F4"],
    },
    Binding {
        action: Action::CopyAs,
        name: "copy_as",
        group: "Files",
        about: "Copy beside itself under another name",
        keys: &["Shift+F5"],
    },
    Binding {
        action: Action::Symlink,
        name: "symlink",
        group: "Files",
        about: "Link to this in the other pane's directory",
        keys: &["Ctrl+Shift+F5"],
    },
    Binding {
        action: Action::Permissions,
        name: "permissions",
        group: "Files",
        about: "Permissions: an octal mode, or read-only on Windows",
        keys: &["Alt+p"],
    },
    Binding {
        action: Action::Checksum,
        name: "checksum",
        group: "Files",
        about: "SHA-256 of the file, checked against a .sha256 beside it",
        keys: &["Alt+h"],
    },
    Binding {
        action: Action::UserMenu,
        name: "user_menu",
        group: "General",
        about: "Your commands from [[menu]], run in the terminal",
        keys: &["Shift+F9"],
    },
    Binding {
        action: Action::Pack,
        name: "pack",
        group: "Files",
        about: "Pack the selection into a .zip, .tar or .tar.gz",
        keys: &["Alt+F5"],
    },
    Binding {
        action: Action::Unpack,
        name: "unpack",
        group: "Files",
        about: "Unpack the archive under the cursor into the other pane",
        keys: &["Alt+F9"],
    },
    Binding {
        action: Action::CompareDirs,
        name: "compare_dirs",
        group: "Panes",
        about: "Compare with the other pane: mark what is newer or missing there",
        keys: &["Alt+k"],
    },
    Binding {
        action: Action::SyncDirs,
        name: "sync_dirs",
        group: "Panes",
        about: "Synchronise both panes' trees: newer wins, shown before it runs",
        keys: &["Alt+y"],
    },
    Binding {
        action: Action::MenuBar,
        name: "menu_bar",
        group: "General",
        about: "The menu bar: File, View, Help",
        keys: &["F9"],
    },
    Binding {
        action: Action::Usage,
        name: "usage",
        group: "General",
        about: "What rcmd is using: memory and its trend, CPU, threads, draw time",
        keys: &["Shift+F1"],
    },
];

/// The order groups are shown in, F1 and `--print-keys` alike. A group not listed here
/// still shows, after these.
const GROUP_ORDER: &[&str] = &[
    "General", "Navigate", "Select", "Files", "Sort", "Panes", "Tabs", "Terminal",
];

/// The bindings by group, each group once, in `GROUP_ORDER`; within a group, in table
/// order. Rows were added to `BINDINGS` over time wherever they landed, so walking it
/// in order showed some groups twice.
pub fn grouped() -> Vec<(&'static str, Vec<&'static Binding>)> {
    let mut groups: Vec<&'static str> = GROUP_ORDER.to_vec();
    for b in BINDINGS {
        if !groups.contains(&b.group) {
            groups.push(b.group);
        }
    }
    groups
        .into_iter()
        .map(|g| (g, BINDINGS.iter().filter(|b| b.group == g).collect()))
        .filter(|(_, v): &(&str, Vec<&Binding>)| !v.is_empty())
        .collect()
}

/// Every key as Markdown, for `docs/keys.md`.
pub fn markdown(keymap: &Keymap) -> String {
    let mut out = String::from(
        "# Keys\n\n\
         These are the defaults. Press **F1** in rcmd to see your own, after any changes in\n\
         `[keys]` in the config. This file is generated: `rcmd --print-keys > docs/keys.md`.\n",
    );
    for (group, rows) in grouped() {
        let _ = write!(out, "\n## {group}\n\n| Keys | Does |\n|---|---|\n");
        for b in rows {
            let keys: Vec<String> = keymap
                .keys_for(b.action)
                .iter()
                .map(|k| format!("`{}`", k.replace('|', "\\|")))
                .collect();
            let keys = if keys.is_empty() {
                "—".into()
            } else {
                keys.join(" ")
            };
            let _ = writeln!(out, "| {keys} | {} |", b.about.replace('|', "\\|"));
        }
    }
    out
}

impl Action {
    pub fn from_name(s: &str) -> Option<Action> {
        BINDINGS
            .iter()
            .find(|b| b.name == s)
            .map(|b| b.action)
            .or_else(|| ALIASES.iter().find(|(old, _)| *old == s).map(|(_, a)| *a))
    }

    /// Actions that work wherever focus is, including inside the shell pane. Everything
    /// else is passed through to the shell so it can have its own keys.
    pub fn is_global(self) -> bool {
        matches!(
            self,
            Action::FocusTerminal
                | Action::ToggleTerminal
                | Action::CdTerminalHere
                | Action::NewTerminal
                | Action::NextTab
                | Action::PrevTab
                | Action::NextSplit
                | Action::SplitRight
                | Action::SplitDown
                | Action::ClosePane
                | Action::ResizeLeft
                | Action::ResizeRight
                | Action::ResizeUp
                | Action::ResizeDown
                | Action::Zoom
        )
    }
}

pub type Chord = (KeyCode, KeyModifiers);

#[derive(Debug, Clone)]
pub struct Keymap {
    map: HashMap<Chord, Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut map = HashMap::new();
        for b in BINDINGS {
            for k in b.keys {
                let chord = parse_key(k).expect("a default binding must parse");
                map.insert(chord, b.action);
            }
        }
        Self { map }
    }
}

impl Keymap {
    pub fn action_for(&self, k: KeyEvent) -> Option<Action> {
        self.map.get(&normalise(k)).copied()
    }

    /// Apply `[keys]` from the config. An action named there replaces all of its default
    /// bindings, so a user who rebinds `copy` does not end up with both keys live.
    pub fn apply(
        &mut self,
        table: &std::collections::BTreeMap<String, Vec<String>>,
    ) -> Result<(), String> {
        for (name, keys) in table {
            let Some(action) = Action::from_name(name) else {
                return Err(format!(
                    "[keys] has no action called \"{name}\"; see `rcmd --print-config`"
                ));
            };
            self.map.retain(|_, a| *a != action);
            for k in keys {
                let chord = parse_key(k).map_err(|e| format!("[keys] {name}: {e}"))?;
                self.map.insert(chord, action);
            }
        }
        Ok(())
    }

    /// Current bindings, grouped by action, for `--print-config`.
    pub fn to_table(&self) -> std::collections::BTreeMap<String, Vec<String>> {
        let mut out: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for b in BINDINGS {
            let mut keys: Vec<String> = self
                .map
                .iter()
                .filter(|(_, a)| **a == b.action)
                .map(|((c, m), _)| key_name(*c, *m))
                .collect();
            keys.sort();
            out.insert(b.name.to_string(), keys);
        }
        out
    }

    /// The keys currently bound to `action`, as they would be written in the config.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        let mut keys: Vec<String> = self
            .map
            .iter()
            .filter(|(_, a)| **a == action)
            .map(|((c, m), _)| key_name(*c, *m))
            .collect();
        keys.sort();
        keys
    }
}

/// A typed capital letter already carries its shift, so keep one spelling of it or the
/// lookup misses depending on how the terminal reported the key.
fn normalise(k: KeyEvent) -> Chord {
    let mut m = k.modifiers;
    let mut code = k.code;
    if let KeyCode::Char(c) = code {
        if m.contains(KeyModifiers::SHIFT) {
            code = KeyCode::Char(c.to_ascii_uppercase());
            m.remove(KeyModifiers::SHIFT);
        }
    }
    // Ctrl+letter reaches us as a control byte in a legacy terminal, which has no case:
    // crossterm reports it lowercase. A binding written "Ctrl+R" must still fire.
    if m.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = code {
            if c.is_ascii_alphabetic() {
                code = KeyCode::Char(c.to_ascii_lowercase());
            }
        }
    }
    // Terminals that speak the Kitty protocol add these; nothing here binds them.
    m.remove(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META);
    (code, m)
}

pub fn parse_key(s: &str) -> Result<Chord, String> {
    let mut mods = KeyModifiers::NONE;
    let mut rest = s.trim();

    loop {
        let lower = rest.to_ascii_lowercase();
        let found = ["ctrl+", "control+", "alt+", "meta+", "shift+"]
            .iter()
            .find(|p| lower.starts_with(**p));
        let Some(prefix) = found else { break };
        mods |= match *prefix {
            "ctrl+" | "control+" => KeyModifiers::CONTROL,
            "alt+" | "meta+" => KeyModifiers::ALT,
            _ => KeyModifiers::SHIFT,
        };
        rest = &rest[prefix.len()..];
    }
    if rest.is_empty() {
        return Err(format!("\"{s}\" has modifiers but no key"));
    }

    let code = match rest.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "space" => KeyCode::Char(' '),
        // Spelled out so they can be written where a comma separates keys, as in
        // `--keys=Alt+comma,Enter`.
        "comma" => KeyCode::Char(','),
        "period" | "dot" => KeyCode::Char('.'),
        other => {
            if let Some(n) = other.strip_prefix('f').and_then(|d| d.parse::<u8>().ok()) {
                if !(1..=24).contains(&n) {
                    return Err(format!("\"{s}\": there is no F{n}"));
                }
                KeyCode::F(n)
            } else {
                let mut it = rest.chars();
                match (it.next(), it.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => return Err(format!("\"{s}\" is not a key name")),
                }
            }
        }
    };

    Ok(normalise(KeyEvent::new(code, mods)))
}

pub fn key_name(code: KeyCode, mods: KeyModifiers) -> String {
    let mut out = String::new();
    if mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if mods.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if mods.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift+");
    }
    match code {
        KeyCode::Char(' ') => out.push_str("Space"),
        KeyCode::Char(c) => out.push(c),
        KeyCode::F(n) => {
            let _ = write!(out, "F{n}");
        }
        other => {
            let _ = write!(out, "{other:?}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn every_default_binding_parses_and_is_reachable() {
        let km = Keymap::default();
        for b in BINDINGS {
            assert!(!b.keys.is_empty(), "{} has no default key", b.name);
            for key in b.keys {
                let chord = parse_key(key).unwrap_or_else(|e| panic!("{}: {key}: {e}", b.name));
                assert_eq!(
                    km.action_for(KeyEvent::new(chord.0, chord.1)),
                    Some(b.action),
                    "{key} should run {}",
                    b.name
                );
            }
        }
    }

    #[test]
    fn every_action_has_exactly_one_row() {
        // A missing row means an action nobody can bind and help never mentions.
        let all = [
            Action::Quit,
            Action::Help,
            Action::Refresh,
            Action::ToggleHidden,
            Action::FocusNextPane,
            Action::FocusTerminal,
            Action::ToggleTerminal,
            Action::CdTerminalHere,
            Action::SplitRight,
            Action::SplitDown,
            Action::ClosePane,
            Action::ResizeLeft,
            Action::ResizeRight,
            Action::ResizeUp,
            Action::ResizeDown,
            Action::Zoom,
            Action::CursorUp,
            Action::CursorDown,
            Action::PageUp,
            Action::PageDown,
            Action::Top,
            Action::Bottom,
            Action::Open,
            Action::EnterDir,
            Action::Parent,
            Action::Mark,
            Action::MarkAndNext,
            Action::InvertMarks,
            Action::MarkAll,
            Action::UnmarkAll,
            Action::View,
            Action::Edit,
            Action::Copy,
            Action::Move,
            Action::Rename,
            Action::MkDir,
            Action::Delete,
            Action::DeletePermanent,
            Action::SortName,
            Action::SortExt,
            Action::SortSize,
            Action::SortModified,
            Action::NewTerminal,
            Action::NewTab,
            Action::CloseTab,
            Action::NextTab,
            Action::PrevTab,
            Action::NextSplit,
            Action::QuickSearch,
            Action::Filter,
            Action::GoToPath,
            Action::Back,
            Action::Forward,
            Action::History,
            Action::Hotlist,
            Action::OpenWith,
            Action::QuitNoCd,
            Action::SwapPanes,
            Action::MirrorPane,
            Action::MarkPattern,
            Action::UnmarkPattern,
            Action::CalcSizes,
            Action::FindFiles,
            Action::BulkRename,
            Action::QuickView,
            Action::NewFile,
            Action::CopyAs,
            Action::Symlink,
            Action::Permissions,
            Action::Checksum,
            Action::UserMenu,
            Action::Pack,
            Action::Unpack,
            Action::CompareDirs,
            Action::SyncDirs,
            Action::MenuBar,
            Action::Usage,
        ];
        for a in all {
            assert_eq!(
                BINDINGS.iter().filter(|b| b.action == a).count(),
                1,
                "{a:?}"
            );
        }
        assert_eq!(BINDINGS.len(), all.len());
    }

    #[test]
    fn no_two_actions_claim_the_same_key() {
        let mut seen: HashMap<Chord, &str> = HashMap::new();
        for b in BINDINGS {
            for key in b.keys {
                let chord = parse_key(key).unwrap();
                if let Some(other) = seen.insert(chord, b.name) {
                    panic!("{key} is bound to both {other} and {}", b.name);
                }
            }
        }
    }

    #[test]
    fn old_action_names_still_load() {
        let mut km = Keymap::default();
        let mut table = std::collections::BTreeMap::new();
        table.insert("grow_pane".to_string(), vec!["Alt+Shift+Right".to_string()]);
        table.insert(
            "shrink_pane".to_string(),
            vec!["Alt+Shift+Left".to_string()],
        );
        km.apply(&table)
            .expect("a config printed by an older rcmd must still load");
        assert_eq!(
            km.action_for(k(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT)),
            Some(Action::ResizeRight)
        );
    }

    #[test]
    fn modifiers_parse_in_any_spelling() {
        assert_eq!(
            parse_key("Ctrl+r").unwrap(),
            parse_key("control+R").unwrap()
        );
        assert_eq!(parse_key("alt+s").unwrap().1, KeyModifiers::ALT);
        assert_eq!(
            parse_key("Shift+F8").unwrap(),
            (KeyCode::F(8), KeyModifiers::SHIFT)
        );
        assert_eq!(
            parse_key("Space").unwrap(),
            (KeyCode::Char(' '), KeyModifiers::NONE)
        );
    }

    #[test]
    fn ctrl_letters_ignore_case_because_the_terminal_does() {
        let lower = parse_key("Ctrl+r").unwrap();
        assert_eq!(lower, parse_key("Ctrl+R").unwrap());
        assert_eq!(lower, (KeyCode::Char('r'), KeyModifiers::CONTROL));
    }

    #[test]
    fn a_capital_letter_has_one_spelling() {
        // The terminal may send 'G' with SHIFT or without; both must hit the binding.
        let a = parse_key("G").unwrap();
        let b = parse_key("Shift+g").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, (KeyCode::Char('G'), KeyModifiers::NONE));
    }

    #[test]
    fn bad_keys_are_rejected_with_something_readable() {
        for bad in ["", "Ctrl+", "F0", "F99", "notakey"] {
            assert!(parse_key(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn rebinding_an_action_replaces_its_defaults() {
        let mut km = Keymap::default();
        let mut table = std::collections::BTreeMap::new();
        table.insert("copy".to_string(), vec!["Ctrl+Insert".to_string()]);
        km.apply(&table).unwrap();

        assert_eq!(
            km.action_for(k(KeyCode::Insert, KeyModifiers::CONTROL)),
            Some(Action::Copy)
        );
        assert_ne!(
            km.action_for(k(KeyCode::F(5), KeyModifiers::NONE)),
            Some(Action::Copy),
            "the old key must stop doing it, or the config lied"
        );
    }

    #[test]
    fn an_unknown_action_or_key_is_an_error_not_a_shrug() {
        let mut km = Keymap::default();
        let mut table = std::collections::BTreeMap::new();
        table.insert("coppy".to_string(), vec!["F5".to_string()]);
        assert!(km.apply(&table).unwrap_err().contains("coppy"));

        let mut table = std::collections::BTreeMap::new();
        table.insert("copy".to_string(), vec!["Ctrl+Nope".to_string()]);
        assert!(km.apply(&table).unwrap_err().contains("copy"));
    }

    #[test]
    fn the_printed_table_round_trips() {
        let km = Keymap::default();
        let mut fresh = Keymap::default();
        fresh
            .apply(&km.to_table())
            .expect("what we print must parse");
        for b in BINDINGS {
            let chord = parse_key(b.keys[0]).unwrap();
            assert_eq!(
                fresh.action_for(KeyEvent::new(chord.0, chord.1)),
                Some(b.action)
            );
        }
    }
}
