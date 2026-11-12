//! `config.toml`: where it lives, what it may say, and what happens when it is wrong.
//!
//! Every field is optional — a file holding one line is a valid config. A file that is
//! wrong in any way (bad TOML, a misspelt key, a colour that is not a colour) is rejected
//! **as a whole**: xdevcmd starts on the defaults and says so in the status line. A config
//! applied halfway is harder to reason about than one that was not applied at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use xdevcmd_core::color::Depth;
use xdevcmd_core::{Sort, SortKey};

use crate::keys::Keymap;
use crate::theme::Theme;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: General,
    pub files: Files,
    pub terminal: Terminal,
    pub viewer: Viewer,
    pub commands: Commands,
    pub security: Security,
    pub theme: ThemeSpec,
    /// action name -> keys. An action listed here loses its default keys.
    pub keys: BTreeMap<String, Vec<String>>,
    /// name -> directory, shown first in the hotlist (Ctrl+D). `~` is your home.
    /// Bookmarks added from inside xdevcmd are kept in its state directory instead, so
    /// xdevcmd never rewrites this file.
    pub bookmarks: BTreeMap<String, String>,
    /// The user menu (F9): commands typed into the terminal panel. Placeholders:
    /// %f %F the file under the cursor (name, path), %s %S the selection, %d this
    /// directory, %D the other pane's, %% a percent sign. Each one is quoted for
    /// your shell.
    pub menu: Vec<MenuItem>,
    /// How Enter opens files of these extensions, before the system's program: the
    /// first rule that matches wins. Shift+O lists the rules that match a file.
    pub open: Vec<OpenRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRule {
    /// Extensions, without the dot; case does not matter.
    pub ext: Vec<String>,
    /// "view" or "edit": xdevcmd's viewer, or your editor. Or a `command` instead.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: String,
    /// A program and its arguments, as a list; the file is added as one more argument.
    /// No shell reads it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// Run `command` in a terminal tab instead of as a window of its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub terminal: bool,
    /// Skip this rule where nothing can be shown on a screen (over SSH, no display),
    /// so the next rule, or the viewer, is used.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_display: bool,
    /// The systems this rule is for: "windows", "macos", "linux"; empty for all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub os: Vec<String>,
}

impl OpenRule {
    /// The program and arguments for `path`: the command as written, then the path as
    /// one more argument, whatever characters its name holds. No shell reads it.
    pub fn argv(&self, path: &Path) -> Vec<std::ffi::OsString> {
        let mut argv: Vec<std::ffi::OsString> = self.command.iter().map(Into::into).collect();
        argv.push(path.as_os_str().to_owned());
        argv
    }

    /// Whether this rule is for `ext` (lower case, without the dot) on system `os`.
    pub fn matches(&self, ext: &str, os: &str) -> bool {
        (self.os.is_empty() || self.os.iter().any(|o| o == os))
            && self.ext.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }

    /// What it does, for a menu: "view", "edit", or the command.
    pub fn label(&self) -> String {
        if self.command.is_empty() {
            self.action.clone()
        } else {
            self.command.join(" ")
        }
    }
}

/// The name `[[open]] os` uses for the system xdevcmd runs on.
pub fn this_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuItem {
    /// One character that picks it in the menu.
    pub key: String,
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Ask before F5 / F6 start. Deleting always asks; that is not configurable.
    pub confirm_transfer: bool,
    /// What plain F8 does. Shift+F8 deletes permanently regardless.
    pub delete_to_trash: bool,
    /// Reopen the panes, tabs and directories that were open at the last quit.
    /// Directories given on the command line always win.
    pub restore_session: bool,
    /// The File | View | Help bar on the top row. F9 opens it either way.
    pub menu_bar: bool,
    /// Save the session this often when it has changed, in seconds, so closing the
    /// window instead of quitting keeps it. 0 turns it off (it is still saved on
    /// quit); otherwise 5 to 3600.
    pub autosave_secs: u64,
    /// Safe operations: every copy (F5, F6 across drives, sync, extract, pack) writes
    /// under a temporary name and renames at the end, and a journal of each job lets the
    /// next start clean up after a run that was killed and offer to resume it. Off, a
    /// new file is written under its own name. See docs/security.md.
    pub safe_operations: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            confirm_transfer: true,
            delete_to_trash: true,
            restore_session: true,
            menu_bar: true,
            autosave_secs: 5,
            safe_operations: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Files {
    pub show_hidden: bool,
    /// name | ext | size | modified
    pub sort: String,
    pub reverse: bool,
    pub dirs_first: bool,
    /// How many file panes Alt+S / Alt+D may create, the starting two included. 2-16.
    pub max_panes: usize,
    /// How often a pane checks its directory for changes, in milliseconds. 0 turns
    /// it off (Ctrl+R still refreshes); otherwise 100 to 60000.
    pub auto_refresh_ms: u64,
    /// Colour names the way `ls` does, from $LS_COLORS, instead of the theme's.
    pub ls_colors: bool,
    /// Nerd Font icons before names. Needs a Nerd Font in the terminal.
    pub icons: bool,
    /// A column of git status letters (M ? A D R U). Runs git in each directory you
    /// open; a repository's own config can make git run programs (filters), so turn
    /// it on only where you trust the repositories. Never runs in restricted mode.
    pub git_status: bool,
    /// What Enter does on a file: "open" it with its program, as Total Commander does
    /// (an `[[open]]` rule first, else the system's), or "view" it (F3).
    pub enter: String,
    /// Ctrl+F's filter: "fuzzy", the letters in order, best matches first; or "plain",
    /// the text as typed anywhere in the name. Wildcards (`*.rs`) work in both.
    pub filter: String,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort: "name".into(),
            reverse: false,
            dirs_first: true,
            max_panes: 3,
            auto_refresh_ms: 1000,
            ls_colors: false,
            icons: false,
            git_status: false,
            enter: "open".into(),
            filter: "fuzzy".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Terminal {
    pub show: bool,
    /// Share of the height the shell pane takes, 10–90.
    pub height_percent: u16,
    /// Empty means $SHELL, or %COMSPEC% on Windows.
    pub shell: String,
    /// How many shells may run at once, across every tab and split. 1-16.
    pub max_terminals: usize,
    /// Lines of history each shell keeps, for the wheel and Shift+F12. 100-9999 (the
    /// owner's ceiling, 2026-11-09): a full history of lines as wide as the window costs
    /// lines × columns × a cell's size (Settings shows the figure); vt100 0.18 trims a
    /// line's trailing blanks once it scrolls into history, so short lines cost less.
    /// Read when a shell starts; vt100 cannot change a history's length in place.
    pub scrollback: usize,
    /// Alt+C, typing names (Alt+Enter) and the user menu send to the shell without asking when it is
    /// surely at its prompt; false asks every time (owner, 2026-11-09: default true).
    pub send_when_idle: bool,
    /// The file pane used last goes to the shell's directory when the shell's `cd`
    /// changes it, at the prompt only; never the other way. Off by default.
    pub follow: bool,
}

/// The range `[terminal] scrollback` takes.
pub const SCROLLBACK: std::ops::RangeInclusive<usize> = 100..=9999;

impl Default for Terminal {
    fn default() -> Self {
        Self {
            show: true,
            height_percent: 30,
            shell: String::new(),
            max_terminals: 3,
            scrollback: 5000,
            send_when_idle: true,
            follow: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Viewer {
    pub wrap: bool,
    pub tab_width: usize,
    /// How quick view draws pictures: "auto" (the terminal's own pictures where it can
    /// draw them, kitty, Ghostty, iTerm2, WezTerm, Windows Terminal, foot; half blocks
    /// elsewhere), "kitty", "iterm2", "sixel", or "blocks" (half blocks always).
    pub images: String,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            wrap: true,
            tab_width: 4,
            images: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Commands {
    /// Empty means $VISUAL, then $EDITOR, then vi (notepad on Windows). May carry
    /// arguments: "code -w".
    pub editor: String,
    /// What `o` opens a file with. Empty means the system's own: xdg-open on Linux,
    /// open on macOS, the file association on Windows. May carry arguments.
    pub opener: String,
}

/// How much xdevcmd may do. A command-line flag can make this stricter, never looser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Security {
    /// normal | read_only | restricted
    ///   read_only:  xdevcmd writes nothing — no copy, move, delete, rename, mkdir, edit,
    ///               and no session file. The shell still works; typing is yours.
    ///   restricted: read_only, plus no shell, no external editor, no clipboard.
    pub mode: String,
    /// Navigation never leaves this directory, through symlinks included. Empty means
    /// anywhere. It confines xdevcmd, not a shell: use restricted for that.
    pub root: String,
}

impl Default for Security {
    fn default() -> Self {
        Self {
            mode: "normal".into(),
            root: String::new(),
        }
    }
}

/// Ordered by strictness, so combining a config and a flag is `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Mode {
    #[default]
    Normal,
    ReadOnly,
    Restricted,
}

impl Mode {
    pub fn read_only(self) -> bool {
        self >= Mode::ReadOnly
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::ReadOnly => "read-only",
            Mode::Restricted => "restricted",
        }
    }
}

/// Colours as written in the file: "#rrggbb", "reset" for the terminal's own colour,
/// or "0"–"255" for the indexed palette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeSpec {
    /// The theme: a built-in one (xdevtheme, dark, light, ansi, blue, high-contrast) or a file
    /// `themes/<name>.toml` beside this config. The colours below change it further.
    pub name: String,
    /// How many colours the terminal shows: "auto", "truecolor", "256" or "16".
    /// `XDEVCMD_COLORS` in the environment overrides it.
    pub colors: String,
    pub background: String,
    pub panel: String,
    pub bar: String,
    pub border: String,
    pub border_active: String,
    pub accent: String,
    pub text: String,
    pub text_dim: String,
    pub directory: String,
    pub executable: String,
    pub symlink: String,
    pub archive: String,
    pub error: String,
    pub marked: String,
    pub cursor: String,
    pub cursor_active: String,
}

impl Default for ThemeSpec {
    fn default() -> Self {
        let t = Theme::default();
        let s = |c: Color| color_name(c);
        Self {
            name: crate::theme::DEFAULT.into(),
            colors: "auto".into(),
            background: s(t.bg),
            panel: s(t.bg_panel),
            bar: s(t.bg_bar),
            border: s(t.border),
            border_active: s(t.border_active),
            accent: s(t.accent),
            text: s(t.fg),
            text_dim: s(t.fg_dim),
            directory: s(t.dir),
            executable: s(t.exec),
            symlink: s(t.link),
            archive: s(t.archive),
            error: s(t.err),
            marked: s(t.mark),
            cursor: s(t.row_cursor),
            cursor_active: s(t.row_cursor_active),
        }
    }
}

/// Everything the app needs, checked and converted.
#[derive(Debug, Clone)]
pub struct Settings {
    pub config: Config,
    pub theme: Theme,
    /// The theme for a 16-colour terminal: [`Theme::ansi`] with the colours the user
    /// set on top.
    pub theme16: Theme,
    /// `[theme] colors`; `None` is `auto`.
    pub colors: Option<Depth>,
    /// The colours this terminal shows, as start-up decided (`main`): what View →
    /// Theme… builds a theme for.
    pub depth: Depth,
    pub keymap: Keymap,
    pub sort: Sort,
    /// The mode in effect — after the command line has had its say.
    pub mode: Mode,
    /// The canonical root directory, if navigation is confined.
    pub root: Option<PathBuf>,
    /// The theme in use: `XDEVCMD_THEME`, else `[theme] name`, else the default when
    /// that is not there.
    pub theme_name: String,
    /// `[theme] name` named a theme that is not there; the default is in use.
    pub theme_note: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        resolve(Config::default()).expect("the defaults must resolve")
    }
}

/// Turn a parsed file into settings, or say exactly what is wrong with it.
pub fn resolve(config: Config) -> Result<Settings, String> {
    let sort_key = match config.files.sort.as_str() {
        "name" => SortKey::Name,
        "ext" => SortKey::Ext,
        "size" => SortKey::Size,
        "modified" => SortKey::Modified,
        other => {
            return Err(format!(
                "[files] sort = \"{other}\": expected name, ext, size or modified"
            ))
        }
    };
    if !(10..=90).contains(&config.terminal.height_percent) {
        return Err(format!(
            "[terminal] height_percent = {}: expected 10 to 90",
            config.terminal.height_percent
        ));
    }
    // Two is the floor because the app starts with two panes and F5/F6 copy between
    // them; sixteen is far past usable on any screen, and catches a typo like 300.
    if !(2..=16).contains(&config.files.max_panes) {
        return Err(format!(
            "[files] max_panes = {}: expected 2 to 16",
            config.files.max_panes
        ));
    }
    if !(1..=16).contains(&config.terminal.max_terminals) {
        return Err(format!(
            "[terminal] max_terminals = {}: expected 1 to 16 (to start with no shell, use show = false)",
            config.terminal.max_terminals
        ));
    }
    if !SCROLLBACK.contains(&config.terminal.scrollback) {
        return Err(format!(
            "[terminal] scrollback = {}: expected {} to {} lines",
            config.terminal.scrollback,
            SCROLLBACK.start(),
            SCROLLBACK.end()
        ));
    }
    let a = config.general.autosave_secs;
    if a != 0 && !(5..=3600).contains(&a) {
        return Err(format!(
            "[general] autosave_secs = {a}: expected 0 (off) or 5 to 3600"
        ));
    }
    let ms = config.files.auto_refresh_ms;
    if ms != 0 && !(100..=60_000).contains(&ms) {
        return Err(format!(
            "[files] auto_refresh_ms = {ms}: expected 0 (off) or 100 to 60000"
        ));
    }
    if !(1..=16).contains(&config.viewer.tab_width) {
        return Err(format!(
            "[viewer] tab_width = {}: expected 1 to 16",
            config.viewer.tab_width
        ));
    }

    let colors = Depth::parse(&config.theme.colors)
        .map_err(|e| format!("[theme] colors = \"{}\": {e}", config.theme.colors))?;
    // XDEVCMD_THEME, for this run, then [theme] name. A theme that is not there (its
    // file deleted, or a name mistyped) or that does not read is not a reason to drop
    // the rest of the config: the default theme stands in, and says so. A name that is
    // a path is an error, as always.
    let theme_name = std::env::var("XDEVCMD_THEME")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| config.theme.name.clone());
    if !is_theme_name(&theme_name) {
        return Err(format!("[theme] name = \"{theme_name}\": not a theme name"));
    }
    let (base, theme_note) = match named_theme(&theme_name) {
        Ok(t) => (t, None),
        // Short, for the status line: --check-config gives the whole story.
        Err(e) if e.contains("no such theme") => (
            Theme::default(),
            Some(format!(
                "theme \"{theme_name}\" is not there \u{2014} {} in use; View \u{203a} Theme\u{2026} picks another",
                crate::theme::DEFAULT
            )),
        ),
        Err(e) => (
            Theme::default(),
            Some(format!("{e} \u{2014} {} in use", crate::theme::DEFAULT)),
        ),
    };
    let theme = build_theme(&config.theme, &base)?;
    // On a 16-colour terminal the default theme gives way to the one made of the 16
    // colours; a theme someone chose is kept, its colours fitted to the 16.
    let theme16 = if theme_note.is_some() || plain_by_default(&theme_name) {
        build_theme(&config.theme, &Theme::ansi())?
    } else {
        theme
    };

    let mode = match config.security.mode.as_str() {
        "normal" => Mode::Normal,
        "read_only" | "read-only" => Mode::ReadOnly,
        "restricted" => Mode::Restricted,
        other => {
            return Err(format!(
                "[security] mode = \"{other}\": expected normal, read_only or restricted"
            ))
        }
    };
    let root = match config.security.root.trim() {
        "" => None,
        r => match xdevcmd_core::paths::canonical(r) {
            Ok(p) if p.is_dir() => Some(p),
            Ok(_) => return Err(format!("[security] root = \"{r}\": not a directory")),
            Err(e) => return Err(format!("[security] root = \"{r}\": {e}")),
        },
    };

    let mut keys_seen = std::collections::HashSet::new();
    for (i, m) in config.menu.iter().enumerate() {
        let n = i + 1;
        if m.key.chars().count() != 1 {
            return Err(format!(
                "[[menu]] #{n} key = \"{}\": expected one character",
                m.key
            ));
        }
        if !keys_seen.insert(m.key.clone()) {
            return Err(format!("[[menu]] #{n}: key \"{}\" is used twice", m.key));
        }
        if m.command.trim().is_empty() || m.label.trim().is_empty() {
            return Err(format!(
                "[[menu]] #{n}: label and command must not be empty"
            ));
        }
    }

    if !matches!(
        config.viewer.images.as_str(),
        "auto" | "kitty" | "iterm2" | "sixel" | "blocks"
    ) {
        return Err(format!(
            "[viewer] images = \"{}\": expected auto, kitty, iterm2, sixel or blocks",
            config.viewer.images
        ));
    }
    if !matches!(config.files.filter.as_str(), "fuzzy" | "plain") {
        return Err(format!(
            "[files] filter = \"{}\": expected fuzzy or plain",
            config.files.filter
        ));
    }
    if !matches!(config.files.enter.as_str(), "open" | "view") {
        return Err(format!(
            "[files] enter = \"{}\": expected open or view",
            config.files.enter
        ));
    }
    for (i, r) in config.open.iter().enumerate() {
        let n = i + 1;
        if r.ext.is_empty() || r.ext.iter().any(|e| e.trim().is_empty() || e.contains('.')) {
            return Err(format!(
                "[[open]] #{n}: ext lists extensions without the dot, e.g. [\"pdf\"]"
            ));
        }
        match (r.action.as_str(), r.command.is_empty()) {
            ("view" | "edit", true) => {}
            ("", false) => {
                if r.command[0].trim().is_empty() {
                    return Err(format!(
                        "[[open]] #{n}: command starts with an empty program"
                    ));
                }
            }
            ("", true) => {
                return Err(format!(
                    "[[open]] #{n}: needs action = \"view\" or \"edit\", or a command"
                ))
            }
            (a, true) => {
                return Err(format!(
                    "[[open]] #{n} action = \"{a}\": expected view or edit"
                ))
            }
            (_, false) => {
                return Err(format!(
                    "[[open]] #{n}: action and command together; it is one or the other"
                ))
            }
        }
        if r.terminal && r.command.is_empty() {
            return Err(format!("[[open]] #{n}: terminal = true needs a command"));
        }
        if let Some(o) =
            r.os.iter()
                .find(|o| !matches!(o.as_str(), "windows" | "macos" | "linux"))
        {
            return Err(format!(
                "[[open]] #{n} os: \"{o}\": expected windows, macos or linux"
            ));
        }
    }

    let mut keymap = Keymap::default();
    keymap.apply(&config.keys)?;

    let sort = Sort {
        key: sort_key,
        reverse: config.files.reverse,
        dirs_first: config.files.dirs_first,
    };
    Ok(Settings {
        config,
        theme,
        theme16,
        colors,
        depth: Depth::True,
        keymap,
        sort,
        mode,
        root,
        theme_name: if theme_note.is_some() {
            crate::theme::DEFAULT.to_string()
        } else {
            theme_name
        },
        theme_note,
    })
}

/// The themes that give way to the 16-colour one on a terminal with 16 colours: the
/// default and dark, which nobody chose for their own colours.
fn plain_by_default(name: &str) -> bool {
    name == crate::theme::DEFAULT || name == "dark"
}

/// A theme file, `themes/<name>.toml`: a built-in to start from and the colours that
/// differ, in the same names as `[theme]`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ThemeFile {
    base: Option<String>,
    background: Option<String>,
    panel: Option<String>,
    bar: Option<String>,
    border: Option<String>,
    border_active: Option<String>,
    accent: Option<String>,
    text: Option<String>,
    text_dim: Option<String>,
    directory: Option<String>,
    executable: Option<String>,
    symlink: Option<String>,
    archive: Option<String>,
    error: Option<String>,
    marked: Option<String>,
    cursor: Option<String>,
    cursor_active: Option<String>,
    style: Option<crate::decor::StyleFile>,
}

/// The directory of the config file in use, once `load` has chosen it: with
/// `--config`, that file's, not the default one's.
static CONFIG_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The config file in use, once `load` has chosen it (it may not exist yet).
static CONFIG_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The config file xdevcmd reads, whether or not it exists: where Settings saves.
pub fn path_in_use() -> Option<PathBuf> {
    CONFIG_PATH.get().cloned().or_else(default_path)
}

/// Where theme files are looked for: `themes/` beside the config file in use, then
/// beside the xdevcmd binary (a portable copy keeps its themes with it).
pub fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let config_dir = CONFIG_DIR.get().cloned().or_else(|| {
        default_path()
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    });
    if let Some(d) = config_dir {
        dirs.push(d.join("themes"));
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
    {
        dirs.push(d.join("themes"));
    }
    dirs
}

/// Every theme that can be chosen: the built-in ones, then the files, by name.
pub fn available_themes() -> Vec<String> {
    let mut names: Vec<String> = crate::theme::BUILT_IN
        .iter()
        .map(|s| s.to_string())
        .collect();
    for d in theme_dirs() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut found: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension()? != "toml" {
                    return None;
                }
                Some(p.file_stem()?.to_string_lossy().into_owned())
            })
            .filter(|n| !names.contains(n))
            .collect();
        found.sort();
        names.extend(found);
    }
    names
}

/// Whether `name` can name a theme file: one name, never a path out of `themes/`.
fn is_theme_name(name: &str) -> bool {
    // `:` too: on Windows `C:name` would make the join leave the theme directory.
    // Windows' device names: CON.toml is the console, and reading it waits forever.
    let device = {
        let n = name.to_ascii_uppercase();
        ["CON", "PRN", "AUX", "NUL"].contains(&n.as_str())
            || ((n.starts_with("COM") || n.starts_with("LPT"))
                && n.len() == 4
                && n.as_bytes()[3].is_ascii_digit())
    };
    !(name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') || device)
}

/// The theme called `name`: built in, or read from a file in `theme_dirs`.
pub fn named_theme(name: &str) -> Result<Theme, String> {
    if let Some(t) = crate::theme::built_in(name) {
        return Ok(t);
    }
    if !is_theme_name(name) {
        return Err(format!("[theme] name = \"{name}\": not a theme name"));
    }
    let Some(path) = theme_dirs()
        .into_iter()
        .map(|d| d.join(format!("{name}.toml")))
        .find(|p| p.is_file())
    else {
        return Err(format!(
            "[theme] name = \"{name}\": no such theme; there are {}",
            available_themes().join(", ")
        ));
    };
    let shown = path.display().to_string();
    let text = std::fs::read(&path)
        .and_then(|b| decode(&b))
        .map_err(|e| format!("{shown}: {e}"))?;
    theme_from_text(&text, &shown)
}

/// A theme file's text, as `named_theme` reads it; `shown` names it in errors.
pub fn theme_from_text(text: &str, shown: &str) -> Result<Theme, String> {
    let f: ThemeFile = toml::from_str(text).map_err(|e| {
        format!(
            "{shown}: {}",
            e.to_string()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        )
    })?;
    let base_name = f.base.as_deref().unwrap_or("dark");
    let mut t = crate::theme::built_in(base_name).ok_or_else(|| {
        format!(
            "{shown}: base = \"{base_name}\": expected one of {}",
            crate::theme::BUILT_IN.join(", ")
        )
    })?;
    let set = |slot: &mut Color, field: &str, v: &Option<String>| -> Result<(), String> {
        if let Some(v) = v {
            *slot = parse_color(v).map_err(|e| format!("{shown}: {field} = \"{v}\": {e}"))?;
        }
        Ok(())
    };
    for (slot, field, v) in theme_slots(&mut t, &f) {
        set(slot, field, v)?;
    }
    if let Some(st) = &f.style {
        t.decor = crate::decor::Decor::from_file(st, shown)?;
    }
    Ok(t)
}

/// Each colour of a theme with its name in a theme file, in the order
/// `--print-theme` writes them.
fn theme_slots<'a>(
    t: &'a mut Theme,
    f: &'a ThemeFile,
) -> [(&'a mut Color, &'static str, &'a Option<String>); 16] {
    [
        (&mut t.bg, "background", &f.background),
        (&mut t.bg_panel, "panel", &f.panel),
        (&mut t.bg_bar, "bar", &f.bar),
        (&mut t.border, "border", &f.border),
        (&mut t.border_active, "border_active", &f.border_active),
        (&mut t.accent, "accent", &f.accent),
        (&mut t.fg, "text", &f.text),
        (&mut t.fg_dim, "text_dim", &f.text_dim),
        (&mut t.dir, "directory", &f.directory),
        (&mut t.exec, "executable", &f.executable),
        (&mut t.link, "symlink", &f.symlink),
        (&mut t.archive, "archive", &f.archive),
        (&mut t.err, "error", &f.error),
        (&mut t.mark, "marked", &f.marked),
        (&mut t.row_cursor, "cursor", &f.cursor),
        (&mut t.row_cursor_active, "cursor_active", &f.cursor_active),
    ]
}

/// A theme written out as a theme file, every colour listed: a start for one's own
/// (`xdevcmd --print-theme dark > themes/mine.toml`). A built-in theme, or a file in
/// `themes/`, which is printed whole, so the copy no longer needs it.
pub fn render_theme(name: &str) -> Result<String, String> {
    let (t, kind, base) = match crate::theme::built_in(name) {
        Some(t) => (t, "built-in theme", name.to_string()),
        None => {
            let t = named_theme(name).map_err(|e| format!("--print-theme {name}: {e}"))?;
            // Every colour is listed and the style is written out, so the base only
            // has to add nothing: a plain one of the same lightness.
            let base = if crate::decor::is_light(t.bg) {
                "light"
            } else {
                "dark"
            };
            (t, "theme", base.to_string())
        }
    };
    Ok(theme_text(t, name, kind, &base))
}

/// `t` as a theme file, every colour and the style written out, starting from `base`
/// (a theme with no style of its own); `kind` and `name` are for the comment on top.
pub(crate) fn theme_text(mut t: Theme, name: &str, kind: &str, base: &str) -> String {
    let f = ThemeFile::default();
    let mut out = format!(
        "# The {kind} \"{name}\", from `xdevcmd --print-theme {name}`.\n\
         # Save it as themes/<your-name>.toml beside config.toml, change the colours you\n\
         # want, and pick it in View > Theme... or with [theme] name = \"<your-name>\".\n\
         # A line you delete keeps the base theme's colour.\n\
         # Colours: \"#rrggbb\", \"reset\" (the terminal's own), or \"0\"-\"255\".\n\
         base = \"{base}\"\n"
    );
    let style = t.decor.to_toml();
    for (slot, field, _) in theme_slots(&mut t, &f) {
        out.push_str(&format!("{field} = \"{}\"\n", color_name(*slot)));
    }
    out.push_str(&style);
    out
}

/// The theme the user sees for `name`: the named theme with `[theme]`'s own colours on
/// top, as start-up builds it; for View → Theme….
pub fn theme_for(name: &str, spec: &ThemeSpec, depth: Depth) -> Result<Theme, String> {
    let base = if depth == Depth::Ansi16 && plain_by_default(name) {
        Theme::ansi()
    } else {
        named_theme(name)?
    };
    build_theme(spec, &base)
}

/// The theme `spec` describes over `base`: a colour the user left at its default
/// takes `base`'s, so the 16-colour theme keeps its own palette where nothing was set.
fn build_theme(spec: &ThemeSpec, base: &Theme) -> Result<Theme, String> {
    let default = ThemeSpec::default();
    let c = |field: &str, v: &str, d: &str, b: Color| -> Result<Color, String> {
        if v == d {
            return Ok(b);
        }
        parse_color(v).map_err(|e| format!("[theme] {field} = \"{v}\": {e}"))
    };
    let (s, d) = (spec, &default);
    Ok(Theme {
        bg: c("background", &s.background, &d.background, base.bg)?,
        bg_panel: c("panel", &s.panel, &d.panel, base.bg_panel)?,
        bg_bar: c("bar", &s.bar, &d.bar, base.bg_bar)?,
        border: c("border", &s.border, &d.border, base.border)?,
        border_active: c(
            "border_active",
            &s.border_active,
            &d.border_active,
            base.border_active,
        )?,
        accent: c("accent", &s.accent, &d.accent, base.accent)?,
        fg: c("text", &s.text, &d.text, base.fg)?,
        fg_dim: c("text_dim", &s.text_dim, &d.text_dim, base.fg_dim)?,
        dir: c("directory", &s.directory, &d.directory, base.dir)?,
        exec: c("executable", &s.executable, &d.executable, base.exec)?,
        link: c("symlink", &s.symlink, &d.symlink, base.link)?,
        archive: c("archive", &s.archive, &d.archive, base.archive)?,
        err: c("error", &s.error, &d.error, base.err)?,
        mark: c("marked", &s.marked, &d.marked, base.mark)?,
        row_cursor: c("cursor", &s.cursor, &d.cursor, base.row_cursor)?,
        row_cursor_active: c(
            "cursor_active",
            &s.cursor_active,
            &d.cursor_active,
            base.row_cursor_active,
        )?,
        decor: base.decor,
    })
}

pub fn parse_color(s: &str) -> Result<Color, String> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("reset") || s.eq_ignore_ascii_case("default") {
        return Ok(Color::Reset);
    }
    if let Ok(n) = s.parse::<u16>() {
        return u8::try_from(n)
            .map(Color::Indexed)
            .map_err(|_| "palette index must be 0 to 255".to_string());
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let v = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
        return Ok(Color::Rgb(v(0), v(2), v(4)));
    }
    Err("expected \"#rrggbb\", \"reset\", or a palette index 0-255".into())
}

pub(crate) fn color_name(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(n) => n.to_string(),
        // The 16 named colours are palette entries 0-15, which is how a file writes them.
        c => (0..16u8)
            .find(|&i| crate::theme::ansi(i) == c)
            .map_or_else(|| "reset".to_string(), |i| i.to_string()),
    }
}

/// Where the config is looked for, first match wins.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("XDEVCMD_CONFIG").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(x).join("xdevcmd").join("config.toml"));
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(a).join("xdevcmd").join("config.toml"));
        }
    }
    // macOS included: terminal tools conventionally live in ~/.config, not in
    // ~/Library/Application Support, and that is where a user will look.
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/xdevcmd/config.toml"))
}

/// What happened when loading, so the UI can say it.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Loaded(PathBuf),
    NoFile,
    Rejected { path: PathBuf, error: String },
}

impl Outcome {
    pub fn message(&self) -> Option<String> {
        match self {
            Outcome::Rejected { path, error } => {
                Some(format!("config ignored — {}: {error}", path.display()))
            }
            _ => None,
        }
    }
}

/// The text of a config file, whatever a Windows tool saved it as.
fn decode(bytes: &[u8]) -> std::io::Result<String> {
    let bad = |what: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string());
    let utf16 = |rest: &[u8], le: bool| {
        let (pairs, odd) = rest.as_chunks::<2>();
        if !odd.is_empty() {
            return Err(bad("not valid UTF-16"));
        }
        let units = pairs.iter().map(|&c| {
            if le {
                u16::from_le_bytes(c)
            } else {
                u16::from_be_bytes(c)
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| bad("not valid UTF-16"))
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8(rest.to_vec()),
        [0xFF, 0xFE, rest @ ..] => return utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => return utf16(rest, false),
        _ => String::from_utf8(bytes.to_vec()),
    }
    .map_err(|_| bad("not UTF-8 text; save it as UTF-8"))
}

/// Load from `explicit`, or the default location. Never fails: a bad file produces the
/// defaults plus an [`Outcome::Rejected`] that the caller must show.
///
/// UTF-8, with or without a byte-order mark, or UTF-16 with one: Windows PowerShell 5's
/// `xdevcmd --print-config > config.toml` writes UTF-16, and Notepad has written a BOM.
pub fn load(explicit: Option<&Path>) -> (Settings, Outcome) {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => match default_path() {
            Some(p) => p,
            None => return (Settings::default(), Outcome::NoFile),
        },
    };
    if let Some(d) = path.parent() {
        let _ = CONFIG_DIR.set(d.to_path_buf());
    }
    let _ = CONFIG_PATH.set(path.clone());
    let text = match std::fs::read(&path).and_then(|b| decode(&b)) {
        Ok(t) => t,
        // A missing default file is the normal case. A missing file someone named on
        // the command line is a mistake worth pointing out.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && explicit.is_none() => {
            return (Settings::default(), Outcome::NoFile)
        }
        Err(e) => {
            return (
                Settings::default(),
                Outcome::Rejected {
                    path,
                    error: e.to_string(),
                },
            )
        }
    };
    if let Some(error) = too_open(&path) {
        return (Settings::default(), Outcome::Rejected { path, error });
    }
    match parse(&text) {
        Ok(s) => (s, Outcome::Loaded(path)),
        Err(error) => (Settings::default(), Outcome::Rejected { path, error }),
    }
}

/// The config names programs xdevcmd runs — the shell, the editor. A file every user on
/// the machine can write is a file anyone can point those at something else, so it is
/// refused, the way ssh refuses an authorized_keys others can write.
///
/// Only world-writable is refused. Group-writable is the default on Debian and Ubuntu,
/// where umask 002 gives each user a private group, and refusing it would reject most
/// users' files for nothing. Windows permissions are ACLs and are not checked here.
#[cfg(unix)]
fn too_open(path: &Path) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode();
    (mode & 0o002 != 0).then(|| {
        format!(
            "writable by every user, and it names programs xdevcmd runs \u{2014} chmod o-w {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn too_open(_path: &Path) -> Option<String> {
    None
}

pub fn parse(text: &str) -> Result<Settings, String> {
    let cfg: Config = toml::from_str(text).map_err(|e| {
        // toml's message is good but multi-line; the status bar has one line.
        e.to_string()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    })?;
    resolve(cfg)
}

/// The full default config, every key present, ready to save and edit.
pub fn render_default() -> String {
    let cfg = Config {
        keys: Keymap::default().to_table(),
        ..Config::default()
    };
    let body = toml::to_string_pretty(&cfg).expect("the default config must serialise");
    format!(
        "# xdevcmd configuration - every setting, at its default.\n\
         # Delete what you do not change; a missing key keeps its default.\n\
         # Colours: \"#rrggbb\", \"reset\" (the terminal's own), or \"0\"-\"255\".\n\
         # [theme] colors: \"auto\", \"truecolor\", \"256\" or \"16\"; XDEVCMD_COLORS overrides it.\n\
         # Under [keys], listing an action replaces all of its default keys.\n\n{body}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        let s = parse("").unwrap();
        assert_eq!(s.config, Config::default());
    }

    #[test]
    fn the_printed_default_parses_back_to_the_defaults() {
        let s = parse(&render_default()).expect("--print-config output must load");
        let expected = Config {
            keys: Keymap::default().to_table(),
            ..Config::default()
        };
        assert_eq!(s.config, expected);
    }

    #[test]
    fn the_printed_default_is_ascii_so_no_shell_can_mangle_it() {
        // PowerShell decodes a program's output with the console's code page before
        // `>` writes it, so an em dash came out as "ΓÇö" in the user's file.
        let text = render_default();
        let odd: Vec<char> = text.chars().filter(|c| !c.is_ascii()).collect();
        assert!(odd.is_empty(), "{odd:?}");
    }

    #[test]
    fn a_config_saved_by_windows_tools_still_reads() {
        let toml = "[general]\nmenu_bar = false\n# caf\u{e9}\n";
        let utf16 = |le: bool| {
            let mut b = if le {
                vec![0xFF, 0xFE]
            } else {
                vec![0xFE, 0xFF]
            };
            for u in toml.encode_utf16() {
                b.extend(if le { u.to_le_bytes() } else { u.to_be_bytes() });
            }
            b
        };
        let bom8 = [&[0xEF, 0xBB, 0xBF][..], toml.as_bytes()].concat();
        for bytes in [toml.as_bytes().to_vec(), bom8, utf16(true), utf16(false)] {
            assert_eq!(decode(&bytes).unwrap(), toml);
        }
        assert!(decode(&[0xFF, 0xFE, 0x41]).is_err(), "odd UTF-16");
        assert!(decode(b"menu_bar = \xff").is_err(), "neither");
    }

    #[test]
    fn enter_and_open_rules_are_checked() {
        assert_eq!(parse("").unwrap().config.files.enter, "open");
        assert!(parse("[files]\nenter = \"view\"\n").is_ok());
        let e = parse("[files]\nenter = \"run\"\n").unwrap_err();
        assert!(e.contains("[files] enter"), "{e}");
        for (bad, says) in [
            ("ext = [\".pdf\"]\naction = \"view\"", "without the dot"),
            ("ext = []\naction = \"view\"", "without the dot"),
            ("ext = [\"pdf\"]", "needs action"),
            (
                "ext = [\"pdf\"]\naction = \"print\"",
                "expected view or edit",
            ),
            (
                "ext = [\"pdf\"]\naction = \"view\"\ncommand = [\"x\"]",
                "one or the other",
            ),
            (
                "ext = [\"pdf\"]\naction = \"view\"\nterminal = true",
                "needs a command",
            ),
            (
                "ext = [\"pdf\"]\ncommand = [\"x\"]\nos = [\"amiga\"]",
                "expected windows",
            ),
            ("ext = [\"pdf\"]\ncommand = [\"\"]", "empty program"),
        ] {
            let e = parse(&format!("[[open]]\n{bad}\n")).unwrap_err();
            assert!(e.contains("[[open]] #1") && e.contains(says), "{bad}: {e}");
        }
        let ok = parse("[[open]]\next = [\"PDF\"]\ncommand = [\"zathura\"]\nos = [\"linux\"]\n")
            .unwrap();
        let r = &ok.config.open[0];
        assert!(
            r.matches("pdf", "linux") && !r.matches("pdf", "macos") && !r.matches("txt", "linux")
        );
        assert_eq!(r.label(), "zathura");
    }

    #[test]
    fn a_theme_is_chosen_by_name() {
        let s = parse("[theme]\nname = \"light\"\n").unwrap();
        assert_eq!(s.theme, crate::theme::built_in("light").unwrap());
        assert_eq!(s.theme16, s.theme, "a chosen theme is kept at 16 colours");
        // The colours set in [theme] go on top of the named theme.
        let s = parse("[theme]\nname = \"blue\"\naccent = \"#ff0000\"\n").unwrap();
        assert_eq!(s.theme.accent, Color::Rgb(255, 0, 0));
        assert_eq!(s.theme.bg, crate::theme::built_in("blue").unwrap().bg);
        let e = parse("[theme]\nname = \"../evil\"\n").unwrap_err();
        assert!(e.contains("not a theme name"), "{e}");
    }

    #[test]
    fn colour_depth_is_a_setting() {
        assert_eq!(parse("").unwrap().colors, None, "auto by default");
        let s = parse("[theme]\ncolors = \"256\"\n").unwrap();
        assert_eq!(s.colors, Some(Depth::X256));
        let s = parse("[theme]\ncolors = \"16\"\n").unwrap();
        assert_eq!(s.colors, Some(Depth::Ansi16));
        let e = parse("[theme]\ncolors = \"8\"\n").unwrap_err();
        assert!(e.contains("[theme] colors"), "{e}");
    }

    #[test]
    fn the_16_colour_theme_keeps_its_palette_under_the_users_colours() {
        let s = parse("[theme]\naccent = \"#ff8800\"\n").unwrap();
        assert_eq!(s.theme16.accent, Color::Rgb(0xff, 0x88, 0x00), "set: kept");
        assert_eq!(
            s.theme16.dir,
            Theme::ansi().dir,
            "not set: the 16-colour one"
        );
        assert_eq!(s.theme.dir, Theme::default().dir);
    }

    #[test]
    fn one_line_is_a_valid_config() {
        let s = parse("[theme]\naccent = \"#ff8800\"\n").unwrap();
        assert_eq!(s.theme.accent, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(s.theme.dir, Theme::default().dir, "the rest stays default");
    }

    #[test]
    fn a_misspelt_key_is_an_error_not_silence() {
        let e = parse("[general]\nconfirm_tranfser = false\n").unwrap_err();
        assert!(e.contains("confirm_tranfser"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_hostile_file_name_reaches_an_open_rule_as_one_argument() {
        // Spaces, quotes, a ; and a $( ) that a shell would run: the program sees the
        // name exactly, as one argument, and nothing else runs.
        let d = std::env::temp_dir().join(format!("xdevcmd-openrule-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let name = "a b; touch pwned $(touch pwned2) `touch pwned3` 'q\" && echo x.txt";
        let file = d.join(name);
        std::fs::write(&file, "").unwrap();
        let out = d.join("seen");
        let rule = OpenRule {
            ext: vec!["txt".into()],
            action: String::new(),
            command: vec![
                "/bin/sh".into(),
                "-c".into(),
                format!("printf '%s|%s' \"$#\" \"$1\" > '{}'", out.display()),
                "sh".into(),
            ],
            terminal: false,
            needs_display: false,
            os: Vec::new(),
        };
        let argv = rule.argv(&file);
        assert_eq!(argv.last().unwrap(), file.as_os_str());
        let status = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&d)
            .status()
            .unwrap();
        assert!(status.success());
        let seen = std::fs::read_to_string(&out).unwrap();
        assert_eq!(seen, format!("1|{}", file.display()));
        for p in ["pwned", "pwned2", "pwned3"] {
            assert!(!d.join(p).exists(), "{p} was created: the name was run");
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn colours_accept_hex_reset_and_index() {
        assert_eq!(parse_color("#0a0b0c").unwrap(), Color::Rgb(10, 11, 12));
        assert_eq!(parse_color("0A0B0C").unwrap(), Color::Rgb(10, 11, 12));
        assert_eq!(parse_color("reset").unwrap(), Color::Reset);
        assert_eq!(parse_color("33").unwrap(), Color::Indexed(33));
        assert!(parse_color("256").is_err());
        assert!(parse_color("#12345").is_err());
        assert!(parse_color("teal").is_err());
    }

    #[test]
    fn a_bad_value_names_its_field() {
        let e = parse("[theme]\naccent = \"mauve\"\n").unwrap_err();
        assert!(e.contains("accent") && e.contains("mauve"), "{e}");

        let e = parse("[files]\nsort = \"colour\"\n").unwrap_err();
        assert!(e.contains("sort"), "{e}");

        let e = parse("[terminal]\nheight_percent = 95\n").unwrap_err();
        assert!(e.contains("height_percent"), "{e}");
    }

    #[test]
    fn limits_default_to_three_and_are_range_checked() {
        let s = parse("").unwrap();
        assert_eq!(s.config.files.max_panes, 3);
        assert_eq!(s.config.terminal.max_terminals, 3);

        let s = parse("[files]\nmax_panes = 6\n[terminal]\nmax_terminals = 8\n").unwrap();
        assert_eq!(
            (s.config.files.max_panes, s.config.terminal.max_terminals),
            (6, 8)
        );

        assert!(parse("[files]\nmax_panes = 1\n")
            .unwrap_err()
            .contains("max_panes"));
        assert!(parse("[files]\nmax_panes = 300\n")
            .unwrap_err()
            .contains("max_panes"));
        assert!(parse("[terminal]\nmax_terminals = 0\n")
            .unwrap_err()
            .contains("show = false"));
    }

    #[test]
    fn sending_to_an_idle_shell_without_asking_is_on_by_default() {
        assert!(parse("").unwrap().config.terminal.send_when_idle);
        let s = parse("[terminal]\nsend_when_idle = false\n").unwrap();
        assert!(!s.config.terminal.send_when_idle);
        assert!(render_default().contains("send_when_idle = true"));
    }

    #[test]
    fn following_the_shell_is_off_by_default() {
        assert!(!parse("").unwrap().config.terminal.follow);
        assert!(
            parse("[terminal]\nfollow = true\n")
                .unwrap()
                .config
                .terminal
                .follow
        );
        assert!(render_default().contains("follow = false"));
    }

    #[test]
    fn scrollback_is_100_to_9999_lines() {
        assert_eq!(parse("").unwrap().config.terminal.scrollback, 5000);
        for ok in [100, 9999] {
            let s = parse(&format!("[terminal]\nscrollback = {ok}\n")).unwrap();
            assert_eq!(s.config.terminal.scrollback, ok);
        }
        for bad in [99, 10000, 0] {
            let e = parse(&format!("[terminal]\nscrollback = {bad}\n")).unwrap_err();
            assert!(e.contains("scrollback") && e.contains("100 to 9999"), "{e}");
        }
    }

    #[test]
    fn security_settings_are_checked() {
        assert_eq!(parse("").unwrap().mode, Mode::Normal);
        assert_eq!(
            parse("[security]\nmode = \"read_only\"\n").unwrap().mode,
            Mode::ReadOnly
        );
        assert_eq!(
            parse("[security]\nmode = \"restricted\"\n").unwrap().mode,
            Mode::Restricted
        );
        assert!(parse("[security]\nmode = \"paranoid\"\n")
            .unwrap_err()
            .contains("mode"));
        assert!(parse("[security]\nroot = \"/definitely/not/here\"\n")
            .unwrap_err()
            .contains("root"));
        let tmp = std::env::temp_dir();
        let s = parse(&format!(
            "[security]\nroot = {:?}\n",
            tmp.display().to_string()
        ))
        .unwrap();
        assert_eq!(s.root, Some(xdevcmd_core::paths::canonical(&tmp).unwrap()));
        assert!(Mode::Restricted > Mode::ReadOnly && Mode::ReadOnly > Mode::Normal);
    }

    #[test]
    fn rebinding_through_the_file_works() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let s = parse("[keys]\ncopy = [\"F2\"]\n").unwrap();
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(s.keymap.action_for(f2), Some(crate::keys::Action::Copy));
    }

    /// Keymap A: the two actions that work inside the shell keep to F12's family, or a
    /// rebinding would take a key from the program there (Ctrl+T, or a Ctrl+X prefix).
    #[test]
    fn the_shells_own_actions_take_only_f12s_family() {
        for (action, key) in [
            ("focus_terminal", "Ctrl+t"),
            ("terminal_history", "Ctrl+x h"),
            ("terminal_history", "Alt+h"),
            ("focus_terminal", "F11"),
            ("focus_terminal", "Ctrl+F12"),
            ("terminal_history", "Alt+x h"),
        ] {
            let err = parse(&format!("[keys]\n{action} = [\"{key}\"]\n")).unwrap_err();
            assert!(err.contains(action) && err.contains(key), "{err}");
            assert!(err.contains("F12's family"), "names the rule: {err}");
            assert!(err.contains("Alt+x F12"), "says what is allowed: {err}");
        }
        for key in crate::keys::F12_FAMILY {
            let file = format!("[keys]\nfocus_terminal = [\"{key}\"]\nterminal_history = []\n");
            parse(&file).unwrap_or_else(|e| panic!("{key}: {e}"));
        }
        // Any other action may still have any key.
        parse("[keys]\nnew_terminal = [\"Ctrl+t\"]\n").unwrap();
    }

    #[test]
    fn a_rejected_file_falls_back_and_says_why() {
        let dir = std::env::temp_dir().join(format!("xdevcmd-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("bad.toml");
        std::fs::write(&p, "[theme\naccent = 1").unwrap();

        let (s, outcome) = load(Some(&p));
        assert_eq!(s.config, Config::default());
        let msg = outcome
            .message()
            .expect("a rejection must produce a message");
        assert!(msg.contains("config ignored"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_world_writable_config_is_refused_but_group_writable_is_fine() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("xdevcmd-cfg-perm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(&p, "[commands]\neditor = \"vi\"\n").unwrap();

        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o666)).unwrap();
        let (_, outcome) = load(Some(&p));
        let msg = outcome.message().expect("must be refused");
        assert!(msg.contains("chmod o-w"), "{msg}");

        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o664)).unwrap();
        assert!(
            matches!(load(Some(&p)).1, Outcome::Loaded(_)),
            "umask 002 is normal"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_default_file_is_quiet_but_a_missing_named_one_is_not() {
        let missing = std::env::temp_dir().join("xdevcmd-definitely-not-here.toml");
        let (_, outcome) = load(Some(&missing));
        assert!(matches!(outcome, Outcome::Rejected { .. }));
    }
}
