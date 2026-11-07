//! The palette. Dark, high-contrast, a single teal accent that marks what has focus.
//!
//! Set at startup from `[theme]` in the config, then read everywhere through [`t`]. A
//! process-wide value rather than a parameter threaded through every render function:
//! there is one theme at a time. It can be swapped while running ([`set`], View →
//! Theme…); `t` hands out a copy, sixteen colours, so a frame never sees half of one
//! theme and half of another.

use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use xdevcmd_core::color::Depth;
use xdevcmd_core::{Entry, Kind};

#[derive(Debug, Clone, Copy, PartialEq)]
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
    use xdevcmd_core::color::{nearest_16, nearest_256, palette_rgb};
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

/// The themes xdevcmd has built in, by name. A theme file in `themes/` starts from one of
/// these (`base = "dark"`); built-in names cannot be taken by a file.
pub const BUILT_IN: &[&str] = &["dark", "light", "ansi", "blue", "high-contrast"];

/// A built-in theme by name.
pub fn built_in(name: &str) -> Option<Theme> {
    let rgb = Color::Rgb;
    Some(match name {
        "dark" => Theme::default(),
        "ansi" => Theme::ansi(),
        "light" => Theme {
            bg: rgb(246, 247, 249),
            bg_panel: rgb(255, 255, 255),
            bg_bar: rgb(230, 234, 239),
            border: rgb(190, 197, 206),
            border_active: rgb(0, 122, 112),
            accent: rgb(0, 122, 112),
            fg: rgb(28, 33, 40),
            fg_dim: rgb(105, 114, 125),
            dir: rgb(0, 84, 190),
            exec: rgb(0, 120, 50),
            link: rgb(125, 55, 175),
            archive: rgb(160, 90, 0),
            err: rgb(196, 30, 30),
            mark: rgb(176, 110, 0),
            row_cursor: rgb(224, 229, 236),
            row_cursor_active: rgb(188, 226, 222),
        },
        // Blue panels, cyan text, yellow marks: the DOS-era commanders' look.
        "blue" => Theme {
            bg: rgb(0, 0, 128),
            bg_panel: rgb(0, 0, 160),
            bg_bar: rgb(0, 128, 128),
            border: rgb(85, 255, 255),
            border_active: rgb(255, 255, 255),
            accent: rgb(255, 255, 85),
            fg: rgb(85, 255, 255),
            fg_dim: rgb(160, 170, 200),
            dir: rgb(255, 255, 255),
            exec: rgb(85, 255, 85),
            link: rgb(255, 170, 255),
            archive: rgb(255, 85, 255),
            err: rgb(255, 85, 85),
            mark: rgb(255, 255, 85),
            row_cursor: rgb(0, 0, 100),
            row_cursor_active: rgb(0, 110, 110),
        },
        "high-contrast" => Theme {
            bg: rgb(0, 0, 0),
            bg_panel: rgb(0, 0, 0),
            bg_bar: rgb(38, 38, 38),
            border: rgb(200, 200, 200),
            border_active: rgb(255, 215, 0),
            accent: rgb(255, 215, 0),
            fg: rgb(255, 255, 255),
            fg_dim: rgb(190, 190, 190),
            dir: rgb(110, 180, 255),
            exec: rgb(100, 255, 100),
            link: rgb(255, 130, 255),
            archive: rgb(255, 200, 0),
            err: rgb(255, 90, 90),
            mark: rgb(255, 255, 0),
            row_cursor: rgb(64, 64, 64),
            row_cursor_active: rgb(0, 80, 160),
        },
        _ => return None,
    })
}

static THEME: RwLock<Option<Theme>> = RwLock::new(None);

/// Install the theme at start-up.
pub fn init(theme: Theme) {
    set(theme);
}

/// Swap the theme while running; the next frame draws with it.
pub fn set(theme: Theme) {
    if let Ok(mut t) = THEME.write() {
        *t = Some(theme);
    }
}

/// The active theme, or the default one if nothing installed it (unit tests).
pub fn t() -> Theme {
    THEME.read().ok().and_then(|t| *t).unwrap_or_default()
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
    fn every_built_in_theme_keeps_its_rows_apart_at_full_and_256_colours() {
        for name in BUILT_IN {
            let t = built_in(name).unwrap();
            let depths: &[Depth] = if *name == "ansi" {
                &[Depth::Ansi16]
            } else {
                &[Depth::True, Depth::X256]
            };
            for depth in depths {
                for (a, ca, b, cb) in apart(&t) {
                    if ca == Color::Reset && cb == Color::Reset {
                        continue;
                    }
                    assert_ne!(
                        fit(ca, *depth),
                        fit(cb, *depth),
                        "{name} at {depth:?}: {a} and {b} become the same colour"
                    );
                }
            }
        }
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
