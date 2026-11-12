//! The palette, and the built-in themes. The default, xdevtheme, is dark with a teal
//! accent that marks what has focus, turned into a light across the screen.
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
    /// How it draws beyond its colours: frames, gradient, keys ([`crate::decor`]).
    pub decor: crate::decor::Decor,
}

/// The theme a config that names none gets.
pub const DEFAULT: &str = "xdevtheme";

impl Default for Theme {
    /// xdevtheme: xdevcmd's teal turned into a light that runs across the screen to
    /// violet, rounded frames with the titles in a notch, a lit cursor row, slanted keys.
    fn default() -> Self {
        use crate::decor::{Borders, Cursor, Decor, Gradient, Keys, Shadow, Titles};
        let rgb = Color::Rgb;
        Self {
            bg: rgb(7, 9, 15),
            bg_panel: rgb(11, 14, 23),
            bg_bar: rgb(16, 20, 42),
            border: rgb(35, 43, 71),
            border_active: rgb(46, 230, 208),
            accent: rgb(46, 230, 208),
            fg: rgb(215, 222, 240),
            fg_dim: rgb(107, 118, 153),
            dir: rgb(125, 162, 255),
            exec: rgb(94, 240, 160),
            link: rgb(192, 139, 255),
            archive: rgb(255, 179, 92),
            err: rgb(255, 95, 135),
            mark: rgb(255, 224, 102),
            row_cursor: rgb(21, 26, 44),
            row_cursor_active: rgb(29, 42, 79),
            decor: Decor {
                borders: Borders::Rounded,
                titles: Titles::Notch,
                gradient: Some(Gradient {
                    mid: None,
                    end: rgb(164, 107, 255),
                }),
                cursor: Cursor::Glow,
                marker: true,
                keys: Keys::Slant,
                shadow: Shadow::Dim,
                ..Decor::default()
            },
        }
    }
}

impl Theme {
    /// The dark theme before xdevtheme: one teal accent, plain lines, nothing more.
    pub fn dark() -> Self {
        let rgb = Color::Rgb;
        Self {
            bg: rgb(10, 14, 20),
            bg_panel: rgb(13, 18, 25),
            bg_bar: rgb(17, 23, 32),
            border: rgb(37, 49, 63),
            border_active: rgb(0, 214, 190),
            accent: rgb(0, 214, 190),
            fg: rgb(199, 212, 227),
            fg_dim: rgb(101, 119, 139),
            dir: rgb(122, 186, 255),
            exec: rgb(126, 231, 135),
            link: rgb(197, 164, 255),
            archive: rgb(255, 196, 99),
            err: rgb(255, 110, 110),
            mark: rgb(255, 214, 102),
            row_cursor: rgb(28, 42, 58),
            row_cursor_active: rgb(0, 74, 82),
            decor: crate::decor::Decor::default(),
        }
    }

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
            decor: crate::decor::Decor::default(),
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
pub(crate) fn ansi(i: u8) -> Color {
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
/// these (`base = "dark"`); built-in names cannot be taken by a file. The first is the
/// default.
pub const BUILT_IN: &[&str] = &[
    "xdevtheme",
    "dark",
    "light",
    "ansi",
    "blue",
    "high-contrast",
];

/// A built-in theme by name.
pub fn built_in(name: &str) -> Option<Theme> {
    let rgb = Color::Rgb;
    Some(match name {
        "xdevtheme" => Theme::default(),
        "dark" => Theme::dark(),
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
            decor: crate::decor::Decor::default(),
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
            decor: crate::decor::Decor::default(),
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
            decor: crate::decor::Decor::default(),
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

/// The colour of a piece of source in the viewer: a slot the theme already has, so every
/// theme, built in or a file, colours source without new keys (Round 028).
pub fn syntax_style(kind: xdevcmd_core::syntax::Kind) -> Style {
    syntax_style_in(&t(), kind)
}

/// [`syntax_style`] under theme `th`.
pub fn syntax_style_in(th: &Theme, kind: xdevcmd_core::syntax::Kind) -> Style {
    use xdevcmd_core::syntax::Kind as K;
    let fg = match kind {
        K::Comment => th.fg_dim,
        K::String | K::Added => th.exec,
        K::Number => th.archive,
        K::Keyword | K::Tag => th.link,
        K::Type | K::Key => th.dir,
        K::Function | K::Heading => th.accent,
        K::Meta | K::Warning => th.mark,
        K::Removed | K::Error => th.err,
    };
    let style = Style::new().fg(fg);
    match kind {
        K::Heading | K::Error => style.add_modifier(Modifier::BOLD),
        _ => style,
    }
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
    fn the_example_theme_files_load_and_keep_their_rows_apart() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        let mut seen = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "toml") {
                continue;
            }
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            assert!(
                !BUILT_IN.contains(&name.as_str()),
                "{name} hides a built-in"
            );
            let text = std::fs::read_to_string(&p).unwrap();
            let t = crate::config::theme_from_text(&text, &name).unwrap();
            // Printed (`--print-theme`) and read back, it is the same theme.
            let base = if crate::decor::is_light(t.bg) {
                "light"
            } else {
                "dark"
            };
            let printed = crate::config::theme_text(t, &name, "theme", base);
            let back = crate::config::theme_from_text(&printed, &name).unwrap();
            assert_eq!(back, t, "{name} printed and read back\n{printed}");
            for depth in [Depth::True, Depth::X256] {
                for (a, ca, b, cb) in apart(&t) {
                    assert_ne!(
                        fit(ca, depth),
                        fit(cb, depth),
                        "{name} at {depth:?}: {a} and {b} become the same colour"
                    );
                }
            }
            seen += 1;
        }
        assert!(
            seen >= 3,
            "the example themes are missing from {}",
            dir.display()
        );
    }

    /// WCAG's contrast ratio of two 24-bit colours.
    fn contrast(a: Color, b: Color) -> f64 {
        let lum = |c: Color| {
            let Color::Rgb(r, g, b) = c else {
                panic!("{c:?} is not a 24-bit colour")
            };
            let ch = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.039_28 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// As a 256-colour terminal shows it: the palette entry's colour.
    fn at_256(c: Color) -> Color {
        match fit(c, Depth::X256) {
            Color::Indexed(i) => {
                let (r, g, b) = xdevcmd_core::color::palette_rgb(i);
                Color::Rgb(r, g, b)
            }
            c => c,
        }
    }

    /// The colour `syntax_style` gives `kind` under theme `t`.
    fn slot_for(t: &Theme, kind: xdevcmd_core::syntax::Kind) -> Color {
        syntax_style_in(t, kind).fg.unwrap()
    }

    /// The light theme is for reading on white: every name at 4.5:1 or more on the
    /// panel and on the unfocused cursor row, the titles and keys on the bars, and the
    /// frames at 3:1, at true colour and at 256 colours (Terminal.app has no more).
    #[test]
    fn xdevtheme_light_reads_on_its_own_background() {
        use xdevcmd_core::syntax::Kind as K;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../themes/xdevtheme-light.toml");
        let text = std::fs::read_to_string(path).unwrap();
        let t = crate::config::theme_from_text(&text, "xdevtheme-light").unwrap();
        let texts = [
            ("text", t.fg),
            ("text_dim", t.fg_dim),
            ("directory", t.dir),
            ("executable", t.exec),
            ("symlink", t.link),
            ("archive", t.archive),
            ("error", t.err),
            ("marked", t.mark),
            ("accent", t.accent),
        ];
        let mut pairs: Vec<(&str, Color, &str, Color, f64)> = Vec::new();
        for (name, c) in texts {
            pairs.push((name, c, "panel", t.bg_panel, 4.5));
            pairs.push((name, c, "cursor", t.row_cursor, 4.5));
        }
        for (name, c) in [("text", t.fg), ("text_dim", t.fg_dim), ("accent", t.accent)] {
            pairs.push((name, c, "bar", t.bg_bar, 4.5));
        }
        // The viewer draws on `bg`, and colours source with these slots
        // (`syntax_style`): every kind of piece at 4.5:1 there too.
        for kind in [
            K::Comment,
            K::String,
            K::Number,
            K::Keyword,
            K::Type,
            K::Function,
            K::Meta,
            K::Key,
            K::Tag,
            K::Heading,
            K::Added,
            K::Removed,
            K::Error,
            K::Warning,
        ] {
            pairs.push(("syntax", slot_for(&t, kind), "viewer", t.bg, 4.5));
            // And under a selection of lines in the viewer.
            pairs.push((
                "syntax",
                slot_for(&t, kind),
                "selection",
                t.row_cursor_active,
                4.5,
            ));
        }
        pairs.push(("border", t.border, "panel", t.bg_panel, 3.0));
        pairs.push(("border_active", t.border_active, "panel", t.bg_panel, 3.0));
        for (a, ca, b, cb, want) in pairs {
            for (depth, ca, cb) in [
                ("true colour", ca, cb),
                ("256 colours", at_256(ca), at_256(cb)),
            ] {
                let got = contrast(ca, cb);
                assert!(
                    got >= want,
                    "{a} on {b} at {depth}: {got:.2}:1, under {want}:1"
                );
            }
        }
    }

    #[test]
    fn a_printed_theme_reads_back_as_the_same_theme() {
        for name in BUILT_IN {
            let text = crate::config::render_theme(name).unwrap();
            let back = crate::config::theme_from_text(&text, name).unwrap();
            let want = built_in(name).unwrap();
            if *name == "ansi" {
                // Its 16 named colours come back as palette entries 0-15, which a
                // 16-colour terminal, the only one it is for, shows the same.
                let at16 = |t: &Theme| -> Vec<Color> {
                    apart(t)
                        .into_iter()
                        .flat_map(|(_, a, _, b)| [fit(a, Depth::Ansi16), fit(b, Depth::Ansi16)])
                        .collect()
                };
                assert_eq!(at16(&back), at16(&want), "{name}");
            } else {
                assert_eq!(back, want, "{name}");
            }
        }
        assert!(crate::config::render_theme("nope").is_err());
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
