//! `config.toml`: where it lives, what it may say, and what happens when it is wrong.
//!
//! Every field is optional — a file holding one line is a valid config. A file that is
//! wrong in any way (bad TOML, a misspelt key, a colour that is not a colour) is rejected
//! **as a whole**: rcmd starts on the defaults and says so in the status line. A config
//! applied halfway is harder to reason about than one that was not applied at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use rcmd_core::color::Depth;
use rcmd_core::{Sort, SortKey};

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
    /// Bookmarks added from inside rcmd are kept in its state directory instead, so
    /// rcmd never rewrites this file.
    pub bookmarks: BTreeMap<String, String>,
    /// The user menu (F9): commands typed into the terminal panel. Placeholders:
    /// %f %F the file under the cursor (name, path), %s %S the selection, %d this
    /// directory, %D the other pane's, %% a percent sign. Each one is quoted for
    /// your shell.
    pub menu: Vec<MenuItem>,
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
    /// Esc as a Meta (Alt) prefix in the file panes, as in Midnight Commander: Esc then
    /// a key is Alt+key, Esc then 1-9, 0 is F1-F10. For keyboards and terminals that
    /// send no Alt or no F-keys. Esc arms only when it has nothing else to do.
    pub esc_as_meta: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            confirm_transfer: true,
            delete_to_trash: true,
            restore_session: true,
            menu_bar: true,
            autosave_secs: 5,
            esc_as_meta: true,
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
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            show: true,
            height_percent: 30,
            shell: String::new(),
            max_terminals: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Viewer {
    pub wrap: bool,
    pub tab_width: usize,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            wrap: true,
            tab_width: 4,
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

/// How much rcmd may do. A command-line flag can make this stricter, never looser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Security {
    /// normal | read_only | restricted
    ///   read_only:  rcmd writes nothing — no copy, move, delete, rename, mkdir, edit,
    ///               and no session file. The shell still works; typing is yours.
    ///   restricted: read_only, plus no shell, no external editor, no clipboard.
    pub mode: String,
    /// Navigation never leaves this directory, through symlinks included. Empty means
    /// anywhere. It confines rcmd, not a shell: use restricted for that.
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
    /// How many colours the terminal shows: "auto", "truecolor", "256" or "16".
    /// `RCMD_COLORS` in the environment overrides it.
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
    pub keymap: Keymap,
    pub sort: Sort,
    /// The mode in effect — after the command line has had its say.
    pub mode: Mode,
    /// The canonical root directory, if navigation is confined.
    pub root: Option<PathBuf>,
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
    let theme = build_theme(&config.theme, &Theme::default())?;
    let theme16 = build_theme(&config.theme, &Theme::ansi())?;

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
        r => match rcmd_core::paths::canonical(r) {
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
        keymap,
        sort,
        mode,
        root,
    })
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

fn color_name(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(n) => n.to_string(),
        _ => "reset".to_string(),
    }
}

/// Where the config is looked for, first match wins.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RCMD_CONFIG").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(x).join("rcmd").join("config.toml"));
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(a).join("rcmd").join("config.toml"));
        }
    }
    // macOS included: terminal tools conventionally live in ~/.config, not in
    // ~/Library/Application Support, and that is where a user will look.
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/rcmd/config.toml"))
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
/// `rcmd --print-config > config.toml` writes UTF-16, and Notepad has written a BOM.
pub fn load(explicit: Option<&Path>) -> (Settings, Outcome) {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => match default_path() {
            Some(p) => p,
            None => return (Settings::default(), Outcome::NoFile),
        },
    };
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

/// The config names programs rcmd runs — the shell, the editor. A file every user on
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
            "writable by every user, and it names programs rcmd runs \u{2014} chmod o-w {}",
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
        "# rcmd configuration - every setting, at its default.\n\
         # Delete what you do not change; a missing key keeps its default.\n\
         # Colours: \"#rrggbb\", \"reset\" (the terminal's own), or \"0\"-\"255\".\n\
         # [theme] colors: \"auto\", \"truecolor\", \"256\" or \"16\"; RCMD_COLORS overrides it.\n\
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
        assert_eq!(s.root, Some(rcmd_core::paths::canonical(&tmp).unwrap()));
        assert!(Mode::Restricted > Mode::ReadOnly && Mode::ReadOnly > Mode::Normal);
    }

    #[test]
    fn rebinding_through_the_file_works() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let s = parse("[keys]\ncopy = [\"F2\"]\n").unwrap();
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(s.keymap.action_for(f2), Some(crate::keys::Action::Copy));
    }

    #[test]
    fn a_rejected_file_falls_back_and_says_why() {
        let dir = std::env::temp_dir().join(format!("rcmd-cfg-{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("rcmd-cfg-perm-{}", std::process::id()));
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
        let missing = std::env::temp_dir().join("rcmd-definitely-not-here.toml");
        let (_, outcome) = load(Some(&missing));
        assert!(matches!(outcome, Outcome::Rejected { .. }));
    }
}
