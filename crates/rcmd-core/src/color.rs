//! How many colours the terminal can show, and the nearest colour it can.
//!
//! rcmd's theme is 24-bit. A terminal without 24-bit colour does not ignore
//! `ESC[48;2;R;G;Bm`: macOS Terminal.app before macOS 26 reads R, G and B as separate
//! attributes, so (28, 42, 58) became a green background (42) and (101, 119, 139) a
//! bright red one (101). Colours are therefore sent as 24-bit only where there is
//! evidence the terminal takes them, and quantised to the 256- or 16-colour palette
//! everywhere else.
//!
//! Detection reads environment variables only. Asking the terminal is more accurate
//! but needs an answer, and a terminal that never answers costs a timeout at every
//! start — rcmd has already been bitten by a cursor query over a slow SSH link.

/// The colours a terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// 24-bit: any RGB colour.
    True,
    /// The xterm 256-colour palette.
    X256,
    /// The 16 ANSI colours, whatever the user's palette makes them.
    Ansi16,
}

impl Depth {
    /// A setting's value: `Ok(None)` is `auto`.
    pub fn parse(s: &str) -> Result<Option<Depth>, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(None),
            "truecolor" | "24bit" | "24-bit" => Ok(Some(Depth::True)),
            "256" => Ok(Some(Depth::X256)),
            "16" => Ok(Some(Depth::Ansi16)),
            _ => Err("expected auto, truecolor, 256 or 16".into()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Depth::True => "truecolor",
            Depth::X256 => "256",
            Depth::Ansi16 => "16",
        }
    }
}

/// The first build of Terminal.app with 24-bit colour (macOS 26). Below it, 24-bit
/// sequences come out as other attributes, so it gets 256 colours even when
/// `COLORTERM` says otherwise: people export `COLORTERM=truecolor` in their dotfiles.
/// From one third-party report; the round that added this asks for it to be checked.
pub const APPLE_TERMINAL_TRUECOLOR: u32 = 464;

/// Terminals known to take 24-bit colour, by `TERM_PROGRAM`.
const TRUECOLOR_PROGRAMS: &[&str] = &["iTerm.app", "WezTerm", "ghostty", "vscode"];

/// Pieces of `TERM` that name a terminal known to take 24-bit colour.
const TRUECOLOR_TERMS: &[&str] = &["kitty", "ghostty", "alacritty", "wezterm", "-direct"];

/// What `auto` means here. `var` reads an environment variable (a parameter, so tests
/// need not touch the real environment); `windows` is whether rcmd runs on Windows,
/// where the console has taken 24-bit colour since Windows 10.
///
/// 24-bit only on positive evidence, 256 otherwise. Over SSH neither `COLORTERM` nor
/// `TERM_PROGRAM` is forwarded by default, so a server reached from an old
/// Terminal.app gets 256 colours rather than the broken ones.
pub fn detect(var: impl Fn(&str) -> Option<String>, windows: bool) -> Depth {
    let get = |k: &str| var(k).unwrap_or_default();
    let program = get("TERM_PROGRAM");
    if program == "Apple_Terminal" {
        let build: u32 = get("TERM_PROGRAM_VERSION")
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        return if build >= APPLE_TERMINAL_TRUECOLOR {
            Depth::True
        } else {
            Depth::X256
        };
    }
    let term = get("TERM");
    // The Linux console: 16 colours (8, plus bold for the bright ones).
    if term == "linux" {
        return Depth::Ansi16;
    }
    let colorterm = get("COLORTERM").to_ascii_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return Depth::True;
    }
    if TRUECOLOR_PROGRAMS.contains(&program.as_str())
        || TRUECOLOR_TERMS.iter().any(|t| term.contains(t))
        || var("KITTY_WINDOW_ID").is_some()
        || var("WT_SESSION").is_some()
    {
        return Depth::True;
    }
    if windows {
        return Depth::True;
    }
    Depth::X256
}

/// The levels of the 6×6×6 cube in the 256-colour palette (indexes 16–231).
const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// xterm's default values for the 16 ANSI colours. The user's palette may differ; these
/// are only used to decide which of the 16 is nearest.
const ANSI: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2) as u32;
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// The RGB value of a 256-colour palette entry. 0–15 are the user's own, so their
/// xterm defaults stand in.
pub fn palette_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => ANSI[usize::from(i)],
        16..=231 => {
            let n = i - 16;
            (
                CUBE[usize::from(n / 36)],
                CUBE[usize::from(n / 6 % 6)],
                CUBE[usize::from(n % 6)],
            )
        }
        _ => {
            let v = 8 + 10 * (i - 232);
            (v, v, v)
        }
    }
}

/// The nearest entry of the 256-colour palette, from the cube or the grey ramp, never
/// 0–15: those are whatever the user's palette makes them.
pub fn nearest_256(rgb: (u8, u8, u8)) -> u8 {
    (16..=255u8)
        .min_by_key(|&i| distance(rgb, palette_rgb(i)))
        .unwrap_or(16)
}

/// The nearest of the 16 ANSI colours, as an index 0–15.
pub fn nearest_16(rgb: (u8, u8, u8)) -> u8 {
    (0..16u8)
        .min_by_key(|&i| distance(rgb, ANSI[usize::from(i)]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn settings_parse() {
        assert_eq!(Depth::parse("auto"), Ok(None));
        assert_eq!(Depth::parse(""), Ok(None));
        assert_eq!(Depth::parse("TrueColor"), Ok(Some(Depth::True)));
        assert_eq!(Depth::parse("24bit"), Ok(Some(Depth::True)));
        assert_eq!(Depth::parse("256"), Ok(Some(Depth::X256)));
        assert_eq!(Depth::parse("16"), Ok(Some(Depth::Ansi16)));
        assert!(Depth::parse("8").is_err());
    }

    #[test]
    fn an_old_terminal_app_gets_256_even_when_colorterm_claims_more() {
        let e = env(&[
            ("TERM_PROGRAM", "Apple_Terminal"),
            ("TERM_PROGRAM_VERSION", "455.1"),
            ("COLORTERM", "truecolor"),
            ("TERM", "xterm-256color"),
        ]);
        assert_eq!(detect(e, false), Depth::X256);
    }

    #[test]
    fn terminal_app_from_macos_26_gets_truecolor() {
        let e = env(&[
            ("TERM_PROGRAM", "Apple_Terminal"),
            ("TERM_PROGRAM_VERSION", "464"),
        ]);
        assert_eq!(detect(e, false), Depth::True);
        let e = env(&[("TERM_PROGRAM", "Apple_Terminal")]);
        assert_eq!(detect(e, false), Depth::X256, "no version: assume old");
    }

    #[test]
    fn colorterm_is_positive_evidence() {
        let e = env(&[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]);
        assert_eq!(detect(e, false), Depth::True);
        let e = env(&[("COLORTERM", "24bit")]);
        assert_eq!(detect(e, false), Depth::True);
    }

    #[test]
    fn known_terminals_are_positive_evidence() {
        for e in [
            env(&[("TERM_PROGRAM", "iTerm.app")]),
            env(&[("TERM_PROGRAM", "WezTerm")]),
            env(&[("TERM", "xterm-kitty")]),
            env(&[("TERM", "xterm-ghostty")]),
            env(&[("KITTY_WINDOW_ID", "1")]),
            env(&[("WT_SESSION", "x")]),
        ] {
            assert_eq!(detect(e, false), Depth::True);
        }
    }

    #[test]
    fn without_evidence_it_is_256() {
        // A server reached over SSH: TERM arrives, COLORTERM and TERM_PROGRAM do not.
        let e = env(&[("TERM", "xterm-256color")]);
        assert_eq!(detect(e, false), Depth::X256);
        assert_eq!(detect(env(&[]), false), Depth::X256);
        let e = env(&[("TERM", "tmux-256color")]);
        assert_eq!(detect(e, false), Depth::X256);
    }

    #[test]
    fn the_linux_console_is_16_and_windows_is_truecolor() {
        assert_eq!(detect(env(&[("TERM", "linux")]), false), Depth::Ansi16);
        assert_eq!(detect(env(&[]), true), Depth::True);
    }

    #[test]
    fn the_palette_is_xterms() {
        assert_eq!(palette_rgb(16), (0, 0, 0));
        assert_eq!(palette_rgb(231), (255, 255, 255));
        assert_eq!(palette_rgb(196), (255, 0, 0));
        assert_eq!(palette_rgb(232), (8, 8, 8));
        assert_eq!(palette_rgb(255), (238, 238, 238));
    }

    #[test]
    fn nearest_256_finds_exact_entries_and_never_uses_the_users_16() {
        for i in 16..=255u8 {
            assert_eq!(palette_rgb(nearest_256(palette_rgb(i))), palette_rgb(i));
        }
        assert_eq!(nearest_256((255, 0, 0)), 196);
        assert_eq!(nearest_256((0, 0, 0)), 16);
        // A dark grey lands on the grey ramp, not the cube.
        assert_eq!(nearest_256((18, 18, 18)), 233);
        // The theme's teal cursor stays teal.
        assert_eq!(nearest_256((0, 74, 82)), 23);
    }

    #[test]
    fn nearest_16() {
        assert_eq!(super::nearest_16((250, 10, 10)), 9);
        assert_eq!(super::nearest_16((10, 10, 10)), 0);
        assert_eq!(super::nearest_16((250, 250, 250)), 15);
        assert_eq!(super::nearest_16((230, 230, 230)), 7);
        assert_eq!(super::nearest_16((0, 200, 200)), 6);
    }
}
