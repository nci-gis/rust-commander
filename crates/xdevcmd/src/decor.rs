//! How a theme draws beyond its sixteen colours: the `[style]` table of a theme file.
//!
//! Frames (plain, rounded, thick, double), titles set in a notch of the frame, a
//! gradient from the accent colour across the screen, a lit cursor row, F keys as caps
//! or slanted segments, a shadow under dialogs. All of it is off unless a theme turns it
//! on, and off on a 16-colour terminal, where the glyphs may be missing (an old Windows
//! console) and there is no colour to blend.
//!
//! Most of it is applied after the frame is drawn ([`decorate`]): one pass over the cells
//! that have the accent's colours, so every part of the app that uses the accent (pane
//! frames, tabs, menus, dialogs, keys) gets the same light without each knowing about it.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType};
use xdevcmd_core::color::Depth;

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Borders {
    #[default]
    Plain,
    Rounded,
    Thick,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keys {
    /// The key's number in the accent colour, as always.
    #[default]
    Plain,
    /// The number on an accent background.
    Caps,
    /// The number in a slanted segment, ◢1◤, and the same for the selected tab and the
    /// menu bar's process number.
    Slant,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Decor {
    pub borders: Borders,
    /// Titles in a notch of the frame, `╭┤title├─`, instead of `╭ title ─`.
    pub notch: bool,
    /// Where the gradient ends; it starts at the accent colour, on the left.
    pub gradient: Option<Color>,
    /// The cursor row lit from its left edge, fading to the right.
    pub glow: bool,
    /// A bar, ▌, on the frame at the cursor's row.
    pub marker: bool,
    pub keys: Keys,
    /// A shadow under dialogs and menus.
    pub shadow: bool,
}

impl Decor {
    /// From a theme file's `[style]` table; `shown` names the file in errors.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        borders: Option<&str>,
        titles: Option<&str>,
        gradient: Option<&str>,
        cursor: Option<&str>,
        marker: Option<bool>,
        keys: Option<&str>,
        shadow: Option<&str>,
        shown: &str,
    ) -> Result<Decor, String> {
        let bad = |key: &str, v: &str, want: &str| {
            format!("{shown}: [style] {key} = \"{v}\": expected {want}")
        };
        let mut d = Decor::default();
        if let Some(v) = borders {
            d.borders = match v {
                "plain" => Borders::Plain,
                "rounded" => Borders::Rounded,
                "thick" => Borders::Thick,
                "double" => Borders::Double,
                _ => return Err(bad("borders", v, "plain, rounded, thick or double")),
            };
        }
        if let Some(v) = titles {
            d.notch = match v {
                "line" => false,
                "notch" => true,
                _ => return Err(bad("titles", v, "line or notch")),
            };
        }
        if let Some(v) = gradient {
            d.gradient = match v {
                "none" => None,
                _ => Some(crate::config::parse_color(v).map_err(|e| {
                    format!("{shown}: [style] gradient = \"{v}\": {e}, or \"none\"")
                })?),
            };
        }
        if let Some(v) = cursor {
            d.glow = match v {
                "block" => false,
                "glow" => true,
                _ => return Err(bad("cursor", v, "block or glow")),
            };
        }
        d.marker = marker.unwrap_or(false);
        if let Some(v) = keys {
            d.keys = match v {
                "plain" => Keys::Plain,
                "caps" => Keys::Caps,
                "slant" => Keys::Slant,
                _ => return Err(bad("keys", v, "plain, caps or slant")),
            };
        }
        if let Some(v) = shadow {
            d.shadow = match v {
                "none" => false,
                "dim" => true,
                _ => return Err(bad("shadow", v, "none or dim")),
            };
        }
        Ok(d)
    }
}

/// The terminal's colour depth, for [`get`]: the start-up code sets it once.
static DEPTH: AtomicU8 = AtomicU8::new(0);

pub fn set_depth(depth: Depth) {
    let n = match depth {
        Depth::True => 0,
        Depth::X256 => 1,
        Depth::Ansi16 => 2,
    };
    DEPTH.store(n, Ordering::Relaxed);
}

/// The active theme's decor, as this terminal can show it: none at 16 colours.
pub fn get() -> Decor {
    if DEPTH.load(Ordering::Relaxed) == 2 {
        return Decor::default();
    }
    theme::t().decor
}

/// A bordered block in the theme's frame.
pub fn bordered() -> Block<'static> {
    Block::bordered().border_type(match get().borders {
        Borders::Plain => BorderType::Plain,
        Borders::Rounded => BorderType::Rounded,
        Borders::Thick => BorderType::Thick,
        Borders::Double => BorderType::Double,
    })
}

/// After a block from [`bordered`] is drawn over `area`: set its title in a notch of
/// the top border. The spaces either side of the title become the notch's walls, so
/// nothing moves and clicks on the title still land where they did.
pub fn notch(buf: &mut Buffer, area: Rect) {
    if !get().notch || area.width < 6 || area.height < 2 {
        return;
    }
    let (left, right) = match get().borders {
        Borders::Thick => ("┫", "┣"),
        Borders::Double => ("╡", "╞"),
        _ => ("┤", "├"),
    };
    let line = match get().borders {
        Borders::Thick => "━",
        Borders::Double => "═",
        _ => "─",
    };
    let y = area.y;
    let end = area.x + area.width - 1;
    let sym = |buf: &Buffer, x: u16| buf.cell((x, y)).map_or("", |c| c.symbol()).to_string();
    if sym(buf, area.x + 1) != " " || sym(buf, area.x + 2) == line {
        return;
    }
    let style = buf.cell((area.x, y)).map(|c| c.style()).unwrap_or_default();
    // The last space before the frame's line resumes closes the notch.
    let mut close = None;
    let mut x = area.x + 2;
    while x < end {
        if sym(buf, x) == " " && (sym(buf, x + 1) == line || x + 1 == end) {
            close = Some(x);
            break;
        }
        x += 1;
    }
    let Some(close) = close else { return };
    for (x, s) in [(area.x + 1, left), (close, right)] {
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_symbol(s).set_style(style);
        }
    }
}

/// A shadow to the right of and below `area`, a box drawn over the screen.
pub fn shadow(buf: &mut Buffer, area: Rect) {
    if !get().shadow {
        return;
    }
    let screen = buf.area;
    let (right, bottom) = (area.x + area.width, area.y + area.height);
    let mut cells: Vec<(u16, u16)> = (area.y + 1..=bottom)
        .flat_map(|y| [(right, y), (right + 1, y)])
        .collect();
    cells.extend((area.x + 2..right).map(|x| (x, bottom)));
    for (x, y) in cells {
        if x >= screen.x + screen.width || y >= screen.y + screen.height {
            continue;
        }
        if let Some(c) = buf.cell_mut((x, y)) {
            let (fg, bg) = (darken(c.fg), darken(c.bg));
            c.set_fg(fg).set_bg(bg);
        }
    }
}

fn darken(c: Color) -> Color {
    match c {
        Color::Rgb(..) => mix(c, Color::Rgb(0, 0, 0), 0.6),
        Color::Reset => Color::Rgb(0, 0, 0),
        c => c,
    }
}

/// Mix two colours; anything not a 24-bit colour is left as `a`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => a,
    }
}

/// The gradient's colour at column `x` of a screen `width` wide.
fn along(d: &Decor, x: u16, width: u16) -> Option<Color> {
    let end = d.gradient?;
    let t = f32::from(x) / f32::from(width.max(2) - 1);
    Some(mix(theme::t().accent, end, t))
}

/// The pass over a drawn frame that gives it the theme's light: the gradient on
/// everything in the accent's colours, the lit cursor row and its marker, and a tint on
/// the menu bar. `skip` is what belongs to the programs in the terminal panels: their
/// colours are theirs.
pub fn decorate(buf: &mut Buffer, skip: &[Rect]) {
    let d = get();
    let th = theme::t();
    let area = buf.area;
    let skipped = |x: u16, y: u16| {
        skip.iter()
            .any(|r| r.contains(ratatui::layout::Position::new(x, y)))
    };
    for y in area.y..area.y + area.height {
        // The cursor row: one run of its colour, lit from the left.
        if d.glow || d.marker {
            let run: Vec<u16> = (area.x..area.x + area.width)
                .filter(|&x| !skipped(x, y))
                .filter(|&x| {
                    buf.cell((x, y))
                        .is_some_and(|c| c.bg == th.row_cursor_active)
                })
                .collect();
            if let (Some(&x0), Some(&x1)) = (run.first(), run.last()) {
                if d.glow {
                    let from = along(&d, x0, area.width).unwrap_or(th.accent);
                    let to = along(&d, x1, area.width).unwrap_or(th.accent);
                    let (from, to) = (mix(from, th.bg_panel, 0.6), mix(to, th.bg_panel, 0.84));
                    let span = f32::from((x1 - x0).max(1));
                    for &x in &run {
                        let bg = mix(from, to, f32::from(x - x0) / span);
                        if let Some(c) = buf.cell_mut((x, y)) {
                            c.set_bg(bg);
                        }
                    }
                }
                if d.marker && x0 > area.x {
                    let mark = along(&d, x0, area.width).unwrap_or(th.accent);
                    if let Some(c) = buf.cell_mut((x0 - 1, y)) {
                        if is_frame(c.symbol()) {
                            c.set_symbol("▌").set_fg(mark);
                        }
                    }
                }
            }
        }
        if d.gradient.is_none() {
            continue;
        }
        for x in area.x..area.x + area.width {
            if skipped(x, y) {
                continue;
            }
            let Some(g) = along(&d, x, area.width) else {
                break;
            };
            let Some(c) = buf.cell_mut((x, y)) else {
                continue;
            };
            if c.fg == th.accent || c.fg == th.border_active {
                c.set_fg(g);
            }
            if c.bg == th.accent {
                c.set_bg(g);
            }
            if y == area.y && c.bg == th.bg_bar {
                c.set_bg(mix(th.bg_bar, g, 0.18));
            }
        }
    }
}

fn is_frame(s: &str) -> bool {
    matches!(s, "│" | "┃" | "║")
}

/// In [`Keys::Slant`], a selected tab's label from `x0` to `x1` (inclusive) on row `y`:
/// its first and last cells, spaces, become the segment's slanted ends.
pub fn slant(buf: &mut Buffer, x0: u16, x1: u16, y: u16) {
    if get().keys != Keys::Slant || x1 <= x0 {
        return;
    }
    let th = theme::t();
    for (x, s) in [(x0, "◢"), (x1, "◤")] {
        if let Some(c) = buf.cell_mut((x, y)) {
            if c.symbol() == " " {
                c.set_symbol(s)
                    .set_style(Style::new().fg(th.accent).bg(th.bg_bar));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(borders: &str, keys: &str) -> Result<Decor, String> {
        Decor::parse(
            Some(borders),
            Some("notch"),
            Some("#a46bff"),
            Some("glow"),
            Some(true),
            Some(keys),
            Some("dim"),
            "t.toml",
        )
    }

    #[test]
    fn a_style_table_reads_and_says_what_is_wrong() {
        let d = parse("rounded", "slant").unwrap();
        assert_eq!(d.borders, Borders::Rounded);
        assert_eq!(d.keys, Keys::Slant);
        assert_eq!(d.gradient, Some(Color::Rgb(0xa4, 0x6b, 0xff)));
        assert!(d.notch && d.glow && d.marker && d.shadow);
        let e = parse("wavy", "slant").unwrap_err();
        assert!(
            e.contains("borders = \"wavy\"") && e.contains("rounded"),
            "{e}"
        );
        assert!(parse("plain", "loud").unwrap_err().contains("slant"));
    }

    #[test]
    fn mixing_goes_from_one_colour_to_the_other() {
        let (a, b) = (Color::Rgb(0, 0, 0), Color::Rgb(200, 100, 50));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color::Rgb(100, 50, 25));
        assert_eq!(mix(Color::Reset, b, 0.5), Color::Reset);
    }
}
