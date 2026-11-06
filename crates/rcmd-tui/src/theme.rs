//! The palette. Dark, high-contrast, a single teal accent that marks what has focus.
//!
//! Set once at startup from `[theme]` in the config, then read everywhere through
//! [`t`]. A process-wide value rather than a parameter threaded through every render
//! function: there is exactly one theme, and it never changes while running.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use rcmd_core::{Entry, Kind};

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub bg_panel: Color,
    pub bg_bar: Color,
    pub border: Color,
    pub border_active: Color,
    pub accent: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub dir: Color,
    pub exec: Color,
    pub link: Color,
    pub archive: Color,
    pub err: Color,
    pub mark: Color,
    pub row_cursor: Color,
    pub row_cursor_active: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(10, 14, 20),
            bg_panel: Color::Rgb(13, 18, 25),
            bg_bar: Color::Rgb(17, 23, 32),
            border: Color::Rgb(37, 49, 63),
            border_active: Color::Rgb(0, 214, 190),
            accent: Color::Rgb(0, 214, 190),
            fg: Color::Rgb(199, 212, 227),
            fg_dim: Color::Rgb(101, 119, 139),
            dir: Color::Rgb(122, 186, 255),
            exec: Color::Rgb(126, 231, 135),
            link: Color::Rgb(197, 164, 255),
            archive: Color::Rgb(255, 196, 99),
            err: Color::Rgb(255, 110, 110),
            mark: Color::Rgb(255, 214, 102),
            row_cursor: Color::Rgb(28, 42, 58),
            row_cursor_active: Color::Rgb(0, 74, 82),
        }
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// Install the theme. Only the first call counts.
pub fn init(theme: Theme) {
    let _ = THEME.set(theme);
}

/// The active theme, or the default one if nothing installed it (unit tests).
pub fn t() -> &'static Theme {
    THEME.get_or_init(Theme::default)
}

const ARCHIVE_EXT: &[&str] = &[
    "zip", "tar", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "lz4", "lzma", "cab", "iso",
];

/// Colour for one row, before cursor and mark highlighting are layered on.
pub fn entry_style(e: &Entry) -> Style {
    let th = t();
    if e.error.is_some() {
        return Style::new().fg(th.err);
    }
    if e.symlink {
        return Style::new().fg(th.link);
    }
    match e.kind {
        Kind::Parent => Style::new().fg(th.fg_dim),
        Kind::Dir => Style::new().fg(th.dir).add_modifier(Modifier::BOLD),
        Kind::File => {
            let (_, ext) = e.stem_ext();
            let lower = ext.to_ascii_lowercase();
            if ARCHIVE_EXT.contains(&lower.as_str()) {
                Style::new().fg(th.archive)
            } else if is_executable(e) {
                Style::new().fg(th.exec)
            } else {
                Style::new().fg(th.fg)
            }
        }
    }
}

#[cfg(unix)]
pub fn is_executable(e: &Entry) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(&e.path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_executable(e: &Entry) -> bool {
    let (_, ext) = e.stem_ext();
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "exe" | "bat" | "cmd" | "ps1"
    )
}

pub fn border_style(active: bool) -> Style {
    Style::new().fg(if active {
        t().border_active
    } else {
        t().border
    })
}

pub fn title_style(active: bool) -> Style {
    if active {
        Style::new().fg(t().accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(t().fg_dim)
    }
}
