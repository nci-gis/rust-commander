//! How a theme draws beyond its sixteen colours: the `[style]` table of a theme file.
//!
//! Frames (plain, rounded, thick, double, or none: panes as cards), titles in a notch or
//! as a pill, a gradient from the accent colour across or down the screen, a lit cursor
//! row, F keys as caps or slanted segments, a shadow under dialogs, a glow around the
//! focused frame, a night sky behind it all with stars in the empty rows. All of it is
//! off unless a theme turns it on, and off on a 16-colour terminal, where the glyphs may
//! be missing (an old Windows console) and there is no colour to blend.
//!
//! Most of it is applied after the frame is drawn ([`decorate`]): one pass over the
//! cells, so every part of the app drawn in the accent's colours (pane frames, tabs,
//! menus, dialogs, keys) gets the same light without each knowing about it. The
//! programs in the terminal panels are left alone.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType};
use serde::Deserialize;
use xdevcmd_core::color::Depth;

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Borders {
    #[default]
    Plain,
    Rounded,
    Thick,
    Double,
    /// No lines: each pane a card on the background, with a header strip.
    Cards,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Titles {
    /// `╭ title ─`, as always.
    #[default]
    Line,
    /// `╭┤title├─`: in a notch of the frame.
    Notch,
    /// The focused pane's title on the gradient, a pill.
    Pill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Block,
    /// Lit from its left edge, fading to the right.
    Glow,
    /// On the gradient, with rounded ends past the pane's edges.
    Pill,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shadow {
    #[default]
    None,
    Dim,
    /// The gradient's light, faint, as from a lit box.
    Glow,
}

/// The gradient's colours after the accent, where it starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gradient {
    pub mid: Option<Color>,
    pub end: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Decor {
    pub borders: Borders,
    pub titles: Titles,
    pub gradient: Option<Gradient>,
    /// The gradient runs top to bottom instead of left to right.
    pub down: bool,
    pub cursor: Cursor,
    /// A bar, ▌, on the frame at the cursor's row.
    pub marker: bool,
    pub keys: Keys,
    pub shadow: Shadow,
    /// The focused frame lights the cells around it.
    pub glow: bool,
    /// The backgrounds take a faint aurora, from the gradient's first colour at the top
    /// to its last at the bottom.
    pub sky: bool,
    /// A few fixed stars in a file pane's empty rows.
    pub stars: bool,
}

/// One colour, or the colours after the accent.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Colours {
    One(String),
    Many(Vec<String>),
}

/// A theme file's `[style]` table, as written.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StyleFile {
    borders: Option<String>,
    titles: Option<String>,
    gradient: Option<Colours>,
    gradient_runs: Option<String>,
    cursor: Option<String>,
    marker: Option<bool>,
    keys: Option<String>,
    shadow: Option<String>,
    glow: Option<bool>,
    sky: Option<bool>,
    stars: Option<bool>,
}

impl Decor {
    /// From a theme file's `[style]`; `shown` names the file in errors.
    pub fn from_file(f: &StyleFile, shown: &str) -> Result<Decor, String> {
        fn pick<T: Copy>(
            shown: &str,
            key: &str,
            v: &Option<String>,
            options: &[(&str, T)],
            default: T,
        ) -> Result<T, String> {
            let Some(v) = v else { return Ok(default) };
            options
                .iter()
                .find(|(name, _)| name == v)
                .map(|(_, t)| *t)
                .ok_or_else(|| {
                    let names: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
                    format!(
                        "{shown}: [style] {key} = \"{v}\": expected {}",
                        names.join(", ")
                    )
                })
        }
        let colour = |v: &str| {
            crate::config::parse_color(v)
                .map_err(|e| format!("{shown}: [style] gradient = \"{v}\": {e}, or \"none\""))
        };
        let gradient = match &f.gradient {
            None => None,
            Some(Colours::One(v)) if v == "none" => None,
            Some(Colours::One(v)) => Some(Gradient {
                mid: None,
                end: colour(v)?,
            }),
            Some(Colours::Many(v)) => match v.as_slice() {
                [end] => Some(Gradient {
                    mid: None,
                    end: colour(end)?,
                }),
                [mid, end] => Some(Gradient {
                    mid: Some(colour(mid)?),
                    end: colour(end)?,
                }),
                _ => {
                    return Err(format!(
                        "{shown}: [style] gradient: one or two colours after the accent"
                    ))
                }
            },
        };
        Ok(Decor {
            borders: pick(
                shown,
                "borders",
                &f.borders,
                &[
                    ("plain", Borders::Plain),
                    ("rounded", Borders::Rounded),
                    ("thick", Borders::Thick),
                    ("double", Borders::Double),
                    ("none", Borders::Cards),
                ],
                Borders::Plain,
            )?,
            titles: pick(
                shown,
                "titles",
                &f.titles,
                &[
                    ("line", Titles::Line),
                    ("notch", Titles::Notch),
                    ("pill", Titles::Pill),
                ],
                Titles::Line,
            )?,
            gradient,
            down: pick(
                shown,
                "gradient_runs",
                &f.gradient_runs,
                &[("across", false), ("down", true)],
                false,
            )?,
            cursor: pick(
                shown,
                "cursor",
                &f.cursor,
                &[
                    ("block", Cursor::Block),
                    ("glow", Cursor::Glow),
                    ("pill", Cursor::Pill),
                ],
                Cursor::Block,
            )?,
            marker: f.marker.unwrap_or(false),
            keys: pick(
                shown,
                "keys",
                &f.keys,
                &[
                    ("plain", Keys::Plain),
                    ("caps", Keys::Caps),
                    ("slant", Keys::Slant),
                ],
                Keys::Plain,
            )?,
            shadow: pick(
                shown,
                "shadow",
                &f.shadow,
                &[
                    ("none", Shadow::None),
                    ("dim", Shadow::Dim),
                    ("glow", Shadow::Glow),
                ],
                Shadow::None,
            )?,
            glow: f.glow.unwrap_or(false),
            sky: f.sky.unwrap_or(false),
            stars: f.stars.unwrap_or(false),
        })
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

/// A bordered block in the theme's frame. Cards are drawn with plain lines, which
/// [`decorate`] turns into the card.
pub fn bordered() -> Block<'static> {
    Block::bordered().border_type(match get().borders {
        Borders::Plain | Borders::Cards => BorderType::Plain,
        Borders::Rounded => BorderType::Rounded,
        Borders::Thick => BorderType::Thick,
        Borders::Double => BorderType::Double,
    })
}

/// After a block from [`bordered`] is drawn over `area`: set its title in a notch of
/// the top border. The spaces either side of the title become the notch's walls, so
/// nothing moves and clicks on the title still land where they did.
pub fn notch(buf: &mut Buffer, area: Rect) {
    let d = get();
    if d.titles != Titles::Notch || d.borders == Borders::Cards {
        return;
    }
    if area.width < 6 || area.height < 2 {
        return;
    }
    let (left, right, line) = match d.borders {
        Borders::Thick => ("┫", "┣", "━"),
        Borders::Double => ("╡", "╞", "═"),
        _ => ("┤", "├", "─"),
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
    let d = get();
    if d.shadow == Shadow::None {
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
        let light = along(&d, x, y, screen);
        if let Some(c) = buf.cell_mut((x, y)) {
            match d.shadow {
                Shadow::Glow => {
                    let lit = mix(c.bg, light.unwrap_or(theme::t().accent), 0.22);
                    c.set_bg(lit);
                }
                _ => {
                    let (fg, bg) = (darken(c.fg), darken(c.bg));
                    c.set_fg(fg).set_bg(bg);
                }
            }
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
            let m = |x: u8, y: u8| {
                (f32::from(x) + (f32::from(y) - f32::from(x)) * t)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => a,
    }
}

/// The gradient at `t`, from 0 (the accent) to 1 (its last colour).
fn at(d: &Decor, t: f32) -> Option<Color> {
    let g = d.gradient?;
    let accent = theme::t().accent;
    let t = t.clamp(0.0, 1.0);
    Some(match g.mid {
        None => mix(accent, g.end, t),
        Some(mid) if t < 0.5 => mix(accent, mid, t * 2.0),
        Some(mid) => mix(mid, g.end, t * 2.0 - 1.0),
    })
}

/// The gradient's colour at cell (`x`, `y`) of `screen`, across or down it.
fn along(d: &Decor, x: u16, y: u16, screen: Rect) -> Option<Color> {
    let frac =
        |v: u16, from: u16, n: u16| f32::from(v.saturating_sub(from)) / f32::from(n.max(2) - 1);
    if d.down {
        at(d, frac(y, screen.y, screen.height))
    } else {
        at(d, frac(x, screen.x, screen.width))
    }
}

fn is_frame(s: &str) -> bool {
    matches!(s, "│" | "┃" | "║")
}

fn is_box(s: &str) -> bool {
    s.chars().count() == 1
        && s.chars()
            .next()
            .is_some_and(|c| ('\u{2500}'..='\u{257f}').contains(&c))
}

const TOP_LEFT: &[&str] = &["┌", "╭", "┏", "╔"];
const TOP_RIGHT: &[&str] = &["┐", "╮", "┓", "╗"];

/// A box's top edge on one row: from its corner `x0` to its corner `x1`, and whether
/// it is drawn in the focus colour.
struct Top {
    y: u16,
    x0: u16,
    x1: u16,
    focused: bool,
}

fn tops(buf: &Buffer, area: Rect, skip: &dyn Fn(u16, u16) -> bool) -> Vec<Top> {
    let th = theme::t();
    let mut out = Vec::new();
    for y in area.y..area.y + area.height {
        let mut open: Option<(u16, bool)> = None;
        for x in area.x..area.x + area.width {
            if skip(x, y) {
                open = None;
                continue;
            }
            let Some(c) = buf.cell((x, y)) else { continue };
            let s = c.symbol();
            if TOP_LEFT.contains(&s) {
                open = Some((x, c.fg == th.border_active || c.fg == th.accent));
            } else if TOP_RIGHT.contains(&s) {
                if let Some((x0, focused)) = open.take() {
                    out.push(Top {
                        y,
                        x0,
                        x1: x,
                        focused,
                    });
                }
            }
        }
    }
    out
}

/// No lines: the sides and the bottom of every box become the background, a gutter
/// between cards, and its top a header strip, lit on the focused one.
fn cards(buf: &mut Buffer, area: Rect, tops: &[Top], skip: &dyn Fn(u16, u16) -> bool, d: &Decor) {
    let th = theme::t();
    let header = |x: u16, y: u16, focused: bool| {
        if focused {
            mix(th.bg_panel, along(d, x, y, area).unwrap_or(th.accent), 0.16)
        } else {
            mix(th.bg_panel, th.fg, 0.05)
        }
    };
    for t in tops {
        for x in t.x0..=t.x1 {
            if let Some(c) = buf.cell_mut((x, t.y)) {
                if is_box(c.symbol()) && !is_frame(c.symbol()) {
                    c.set_symbol(" ");
                }
                if is_frame(c.symbol()) {
                    c.set_symbol(" ").set_bg(th.bg);
                } else {
                    c.set_bg(header(x, t.y, t.focused));
                }
            }
        }
    }
    for y in area.y..area.y + area.height {
        let mut bottom = false;
        for x in area.x..area.x + area.width {
            if skip(x, y) {
                continue;
            }
            let Some(c) = buf.cell_mut((x, y)) else {
                continue;
            };
            match c.symbol() {
                "└" | "╰" | "┗" | "╚" => {
                    bottom = true;
                    c.set_symbol(" ").set_bg(th.bg);
                }
                "┘" | "╯" | "┛" | "╝" => {
                    bottom = false;
                    c.set_symbol(" ").set_bg(th.bg);
                }
                "─" | "━" | "═" if bottom => {
                    c.set_symbol(" ").set_bg(th.bg);
                }
                s if is_frame(s) => {
                    c.set_symbol(" ").set_bg(th.bg);
                }
                _ => {}
            }
        }
    }
}

/// The focused box's title on the gradient: the bold accent text on its top edge, and
/// the spaces either side of it.
fn pills(buf: &mut Buffer, area: Rect, tops: &[Top], d: &Decor) {
    let th = theme::t();
    for t in tops.iter().filter(|t| t.focused) {
        let titled: Vec<u16> = (t.x0..=t.x1)
            .filter(|&x| {
                buf.cell((x, t.y)).is_some_and(|c| {
                    c.fg == th.accent && c.modifier.contains(Modifier::BOLD) && c.symbol() != " "
                })
            })
            .collect();
        let (Some(&a), Some(&b)) = (titled.first(), titled.last()) else {
            continue;
        };
        for x in a.saturating_sub(1).max(t.x0 + 1)..=(b + 1).min(t.x1 - 1) {
            let light = along(d, x, t.y, area).unwrap_or(th.accent);
            if let Some(c) = buf.cell_mut((x, t.y)) {
                if x == a.saturating_sub(1) || x == b + 1 {
                    if c.symbol() != " " && !is_box(c.symbol()) {
                        continue;
                    }
                    c.set_symbol(" ");
                }
                c.set_bg(light).set_fg(th.bg);
            }
        }
    }
}

/// The cursor row: one run of its colour on each row, lit or as a pill.
fn cursor(buf: &mut Buffer, area: Rect, skip: &dyn Fn(u16, u16) -> bool, d: &Decor) {
    let th = theme::t();
    for y in area.y..area.y + area.height {
        let run: Vec<u16> = (area.x..area.x + area.width)
            .filter(|&x| !skip(x, y))
            .filter(|&x| {
                buf.cell((x, y))
                    .is_some_and(|c| c.bg == th.row_cursor_active)
            })
            .collect();
        let (Some(&x0), Some(&x1)) = (run.first(), run.last()) else {
            continue;
        };
        let light = |x: u16| along(d, x, y, area).unwrap_or(th.accent);
        match d.cursor {
            Cursor::Glow => {
                let (from, to) = (
                    mix(light(x0), th.bg_panel, 0.6),
                    mix(light(x1), th.bg_panel, 0.84),
                );
                let span = f32::from((x1 - x0).max(1));
                for &x in &run {
                    let bg = mix(from, to, f32::from(x - x0) / span);
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.set_bg(bg);
                    }
                }
            }
            Cursor::Pill => {
                for &x in &run {
                    let bg = mix(light(x), th.bg_panel, 0.58);
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.set_bg(bg);
                    }
                }
                // Rounded ends on the pane's edges, but not over a dialog's text: the
                // right one only where the row meets the pane's edge.
                let edge = |buf: &Buffer, x: u16| {
                    buf.cell((x, y))
                        .is_some_and(|c| c.symbol() == " " || is_frame(c.symbol()))
                };
                let end = area.x + area.width;
                let caps = [
                    (x0 > area.x, x0.wrapping_sub(1), "▐", x0),
                    (
                        x1 + 1 < end && edge(buf, x1 + 1) && (x1 + 2 >= end || edge(buf, x1 + 2)),
                        x1 + 1,
                        "▌",
                        x1,
                    ),
                ];
                for (ok, x, s, from) in caps {
                    if !ok || !edge(buf, x) {
                        continue;
                    }
                    let fill = buf.cell((from, y)).map(|c| c.bg).unwrap_or(th.accent);
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.set_symbol(s).set_fg(fill);
                    }
                }
            }
            Cursor::Block => {}
        }
        if d.marker && d.cursor != Cursor::Pill && x0 > area.x {
            let mark = light(x0);
            if let Some(c) = buf.cell_mut((x0 - 1, y)) {
                if is_frame(c.symbol()) {
                    c.set_symbol("▌").set_fg(mark);
                }
            }
        }
    }
}

/// The pass over a drawn frame that gives it the theme's look. `skip` is what belongs
/// to the programs in the terminal panels: their colours and characters are theirs.
pub fn decorate(buf: &mut Buffer, skip: &[Rect]) {
    let d = get();
    if d == Decor::default() {
        return;
    }
    let th = theme::t();
    let area = buf.area;
    let skipped = |x: u16, y: u16| skip.iter().any(|r| r.contains(Position::new(x, y)));
    let tops = if d.borders == Borders::Cards || d.titles == Titles::Pill {
        tops(buf, area, &skipped)
    } else {
        Vec::new()
    };
    // Where the focused frames are, before the gradient changes their colour: what
    // glows.
    let lit: Vec<(u16, u16)> = if d.glow {
        (area.y..area.y + area.height.saturating_sub(2))
            .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                !skipped(x, y)
                    && buf.cell((x, y)).is_some_and(|c| {
                        is_box(c.symbol()) && (c.fg == th.border_active || c.fg == th.accent)
                    })
            })
            .collect()
    } else {
        Vec::new()
    };
    if d.borders == Borders::Cards {
        cards(buf, area, &tops, &skipped, &d);
    }
    if d.titles == Titles::Pill {
        pills(buf, area, &tops, &d);
    }
    if d.cursor != Cursor::Block || d.marker {
        cursor(buf, area, &skipped, &d);
    }
    if d.gradient.is_some() {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if skipped(x, y) {
                    continue;
                }
                let Some(g) = along(&d, x, y, area) else {
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
    if !lit.is_empty() {
        glow(buf, area, &lit, &skipped, &d);
    }
    if d.sky {
        sky(buf, area, &skipped, &d);
    }
}

/// Bloom: each focused frame cell lights the cells around it, most where it is.
fn glow(
    buf: &mut Buffer,
    area: Rect,
    lit: &[(u16, u16)],
    skip: &dyn Fn(u16, u16) -> bool,
    d: &Decor,
) {
    let (w, h) = (usize::from(area.width), usize::from(area.height));
    let mut amount = vec![0f32; w * h];
    for &(x, y) in lit {
        for dy in -1i32..=1 {
            for dx in -2i32..=2 {
                let (nx, ny) = (i32::from(x - area.x) + dx, i32::from(y - area.y) + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let a = [0.24, 0.12, 0.05][dx.unsigned_abs().max(dy.unsigned_abs()) as usize];
                let i = ny as usize * w + nx as usize;
                amount[i] = amount[i].max(a);
            }
        }
    }
    for (i, &a) in amount.iter().enumerate() {
        if a == 0.0 {
            continue;
        }
        let (x, y) = (area.x + (i % w) as u16, area.y + (i / w) as u16);
        if skip(x, y) {
            continue;
        }
        let light = along(d, x, y, area).unwrap_or(theme::t().accent);
        if let Some(c) = buf.cell_mut((x, y)) {
            let bg = mix(c.bg, light, a);
            c.set_bg(bg);
        }
    }
}

/// A night sky: the theme's own backgrounds take a faint aurora, the gradient's first
/// colour at the top and its last at the bottom.
fn sky(buf: &mut Buffer, area: Rect, skip: &dyn Fn(u16, u16) -> bool, d: &Decor) {
    let th = theme::t();
    for y in area.y..area.y + area.height {
        let tint =
            at(d, f32::from(y - area.y) / f32::from(area.height.max(2) - 1)).unwrap_or(th.accent);
        for x in area.x..area.x + area.width {
            if skip(x, y) {
                continue;
            }
            let Some(c) = buf.cell_mut((x, y)) else {
                continue;
            };
            let a = if c.bg == th.bg {
                0.13
            } else if c.bg == th.bg_panel || c.bg == th.bg_bar {
                0.075
            } else {
                continue;
            };
            let bg = mix(c.bg, tint, a);
            c.set_bg(bg);
        }
    }
}

/// Stars in a file pane's empty rows, `area`: fixed by position, so they neither move
/// nor cost anything while xdevcmd waits.
pub fn stars(buf: &mut Buffer, area: Rect) {
    let d = get();
    if !d.stars || area.width < 4 {
        return;
    }
    let th = theme::t();
    let screen = buf.area;
    for y in area.y..area.y + area.height {
        for x in area.x + 1..area.x + area.width - 1 {
            let (xx, yy) = (u32::from(x), u32::from(y));
            let h = (xx * 7919 + yy * 104_729 + xx * yy * 31) % 97;
            let (s, fg) = match h {
                0 => (
                    "✦",
                    mix(
                        along(&d, x, y, screen).unwrap_or(th.accent),
                        th.bg_panel,
                        0.35,
                    ),
                ),
                1..=3 => ("·", mix(th.fg, th.bg_panel, 0.72)),
                _ => continue,
            };
            if let Some(c) = buf.cell_mut((x, y)) {
                if c.symbol() == " " {
                    c.set_symbol(s).set_fg(fg);
                }
            }
        }
    }
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

    fn read(text: &str) -> Result<Decor, String> {
        let f: StyleFile = toml::from_str(text).map_err(|e| e.to_string())?;
        Decor::from_file(&f, "t.toml")
    }

    #[test]
    fn a_style_table_reads_and_says_what_is_wrong() {
        let d = read(
            "borders = \"rounded\"\ntitles = \"notch\"\ngradient = \"#a46bff\"\n\
             cursor = \"glow\"\nmarker = true\nkeys = \"slant\"\nshadow = \"dim\"\n",
        )
        .unwrap();
        assert_eq!(d.borders, Borders::Rounded);
        assert_eq!(d.titles, Titles::Notch);
        assert_eq!(d.keys, Keys::Slant);
        assert_eq!(d.cursor, Cursor::Glow);
        assert_eq!(d.shadow, Shadow::Dim);
        assert_eq!(
            d.gradient,
            Some(Gradient {
                mid: None,
                end: Color::Rgb(0xa4, 0x6b, 0xff)
            })
        );
        assert!(d.marker && !d.glow && !d.sky && !d.stars);
        let e = read("borders = \"wavy\"").unwrap_err();
        assert!(
            e.contains("borders = \"wavy\"") && e.contains("none"),
            "{e}"
        );
        assert!(read("keys = \"loud\"").unwrap_err().contains("slant"));
        assert!(read("colour = \"x\"").is_err(), "unknown keys are refused");
    }

    #[test]
    fn a_gradient_takes_one_or_two_colours_after_the_accent() {
        let d = read("gradient = [\"#000000\", \"#ffffff\"]\ngradient_runs = \"down\"").unwrap();
        assert_eq!(d.gradient.unwrap().mid, Some(Color::Rgb(0, 0, 0)));
        assert!(d.down);
        assert_eq!(read("gradient = \"none\"").unwrap().gradient, None);
        assert!(read("gradient = [\"#000000\", \"#111111\", \"#222222\"]").is_err());
        assert!(read("gradient = \"mauve\"").unwrap_err().contains("mauve"));
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
