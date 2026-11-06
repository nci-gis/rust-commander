//! The palette. Dark, high-contrast, a single teal accent that marks what has focus.
//!
//! Set once at startup from `[theme]` in the config, then read everywhere through
//! [`t`]. A process-wide value rather than a parameter threaded through every render
//! function: there is exactly one theme, and it never changes while running.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use rcmd_core::color::Depth;
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

impl Theme {
    /// The palette for a 16-colour terminal: the 16 ANSI colours, so it is the
    /// user's palette that decides what they look like, and the terminal's own
    /// background and text colour. Blue cursor rows, as in the Norton tradition.
    pub fn ansi() -> Self {
        Self {
            bg: Color::Reset,
            bg_panel: Color::Reset,
            bg_bar: Color::Reset,
            border: Color::DarkGray,
            border_active: Color::Cyan,
            accent: Color::LightCyan,
            fg: Color::Reset,
            fg_dim: Color::DarkGray,
            dir: Color::LightCyan,
            exec: Color::LightGreen,
            link: Color::LightMagenta,
            archive: Color::Yellow,
            err: Color::LightRed,
            mark: Color::LightYellow,
            row_cursor: Color::DarkGray,
            row_cursor_active: Color::Blue,
        }
    }
}

/// `c` as a terminal of this depth can show it. 24-bit colours become the nearest
/// palette entry; at 16 colours, palette entries above 15 become the nearest of the
/// first 16, which are the user's own palette. (crossterm sends those as `38;5;N`
/// too, N from 0 to 15.)
pub fn fit(c: Color, depth: Depth) -> Color {
    use rcmd_core::color::{nearest_16, nearest_256, palette_rgb};
    match (depth, c) {
        (Depth::True, _) => c,
        (Depth::X256, Color::Rgb(r, g, b)) => Color::Indexed(nearest_256((r, g, b))),
        (Depth::Ansi16, Color::Rgb(r, g, b)) => ansi(nearest_16((r, g, b))),
        (Depth::Ansi16, Color::Indexed(i)) if i > 15 => ansi(nearest_16(palette_rgb(i))),
        (Depth::Ansi16, Color::Indexed(i)) => ansi(i),
        _ => c,
    }
}

/// ANSI colour `i` (0–15) by name.
fn ansi(i: u8) -> Color {
    const NAMED: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    NAMED[usize::from(i & 15)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Slots that are drawn on top of or next to each other, and must stay apart.
    fn apart(t: &Theme) -> Vec<(&'static str, Color, &'static str, Color)> {
        vec![
            ("panel", t.bg_panel, "cursor", t.row_cursor),
            ("panel", t.bg_panel, "cursor_active", t.row_cursor_active),
            ("cursor", t.row_cursor, "cursor_active", t.row_cursor_active),
            ("panel", t.bg_panel, "bar", t.bg_bar),
            ("panel", t.bg_panel, "border", t.border),
            ("panel", t.bg_panel, "text_dim", t.fg_dim),
            ("cursor_active", t.row_cursor_active, "directory", t.dir),
            ("cursor_active", t.row_cursor_active, "marked", t.mark),
        ]
    }

    #[test]
    fn the_cursor_row_does_not_vanish_at_256_colours() {
        // The palette is coarse at the dark end: the panel, the bar and the cursor row
        // could all land on one grey.
        for (a, ca, b, cb) in apart(&Theme::default()) {
            assert_ne!(
                fit(ca, Depth::X256),
                fit(cb, Depth::X256),
                "{a} and {b} become the same colour"
            );
        }
    }

    #[test]
    fn the_16_colour_theme_keeps_its_rows_apart() {
        for (a, ca, b, cb) in apart(&Theme::ansi()) {
            if ca == Color::Reset && cb == Color::Reset {
                continue;
            }
            assert_ne!(
                fit(ca, Depth::Ansi16),
                fit(cb, Depth::Ansi16),
                "{a} and {b} become the same colour"
            );
        }
    }

    #[test]
    fn fitting_keeps_what_the_terminal_already_has() {
        assert_eq!(fit(Color::Rgb(1, 2, 3), Depth::True), Color::Rgb(1, 2, 3));
        assert_eq!(fit(Color::Reset, Depth::X256), Color::Reset);
        assert_eq!(fit(Color::Indexed(100), Depth::X256), Color::Indexed(100));
        assert_eq!(fit(Color::Blue, Depth::Ansi16), Color::Blue);
        assert_eq!(fit(Color::Rgb(255, 0, 0), Depth::X256), Color::Indexed(196));
        assert_eq!(fit(Color::Rgb(255, 0, 0), Depth::Ansi16), Color::LightRed);
        assert_eq!(fit(Color::Indexed(196), Depth::Ansi16), Color::LightRed);
        assert_eq!(fit(Color::Indexed(4), Depth::Ansi16), Color::Blue);
    }

    #[test]
    fn the_16_colour_theme_uses_names_only() {
        let t = Theme::ansi();
        for c in [
            t.bg,
            t.bg_panel,
            t.bg_bar,
            t.border,
            t.border_active,
            t.accent,
            t.fg,
            t.fg_dim,
            t.dir,
            t.exec,
            t.link,
            t.archive,
            t.err,
            t.mark,
            t.row_cursor,
            t.row_cursor_active,
        ] {
            assert!(
                !matches!(c, Color::Rgb(..) | Color::Indexed(_)),
                "{c:?} is not one of the 16"
            );
        }
    }
}
