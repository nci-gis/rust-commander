//! Pictures drawn by the terminal itself, at its own resolution, where it can: the Kitty
//! graphics protocol (kitty, Ghostty), iTerm2's inline images (iTerm2, WezTerm) and
//! sixel (Windows Terminal 1.22+, foot, xterm). Everywhere else, and in screenshots,
//! quick view keeps its half blocks.
//!
//! These are a few escape sequences each, written here rather than taken from a crate
//! (the owner's choice, 2026-11-08): only sixel's palette quantising comes from
//! `icy_sixel`. The picture is decoded and scaled by xdevcmd's own code, as for the
//! half blocks.

use xdevcmd_core::image::Picture;

use crate::app::base64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Kitty,
    Iterm2,
    Sixel,
    /// Half blocks: two pixels a cell, in any terminal.
    Blocks,
}

/// What the terminal said when asked at start-up ([`probe`]): whether it draws Kitty
/// pictures or sixel, and a cell's size in pixels (height, width). Asking works over
/// SSH, where the environment variables that name the terminal mostly do not arrive.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Probe {
    pub kitty: bool,
    pub sixel: bool,
    pub cell: Option<(u16, u16)>,
    /// Its name and version, if it says (XTVERSION): "kitty(0.32.2)", "iTerm2 3.5.5".
    pub version: Option<String>,
    /// Its secondary device attributes (DA2): its kind and version numbers.
    pub da2: Option<(u32, u32)>,
}

/// Ask the terminal, before anything else reads its replies: a Kitty graphics query,
/// the cell size (`CSI 16 t`), and the device attributes (`CSI c`), which every terminal
/// answers and so marks the end. Waits at most `wait` for a terminal that answers none.
#[cfg(unix)]
pub fn probe(wait: std::time::Duration) -> Probe {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    use std::io::Write;
    use std::time::Instant;
    let mut out = std::io::stdout();
    // Kitty graphics, cell size, name and version, kind (DA2), then DA1 last: every
    // terminal answers DA1, so its reply marks the end of the others.
    let ask = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[16t\x1b[>0q\x1b[>c\x1b[c";
    if out.write_all(ask).and_then(|()| out.flush()).is_err() {
        return Probe::default();
    }
    let stdin = std::io::stdin();
    let deadline = Instant::now() + wait;
    let mut got = Vec::new();
    let mut buf = [0u8; 512];
    while !answered(&got) {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(timeout) = Timespec::try_from(left) else {
            break;
        };
        if left.is_zero() {
            break;
        }
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(n) if n > 0 => {}
            _ => break,
        }
        // Straight from the descriptor: std's Stdin would keep what it read ahead in its
        // own buffer, where crossterm, which reads the descriptor, would never see it.
        match rustix::io::read(&stdin, &mut buf) {
            Ok(n) if n > 0 => got.extend_from_slice(&buf[..n]),
            _ => break,
        }
    }
    parse_probe(&got)
}

/// Whether the device attributes reply, the last one asked for, has come.
#[cfg_attr(not(unix), allow(dead_code))]
fn answered(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(bytes);
    s.find("\x1b[?").is_some_and(|i| s[i..].contains('c'))
}

/// The terminal's replies to [`probe`]'s questions.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn parse_probe(bytes: &[u8]) -> Probe {
    let s = String::from_utf8_lossy(bytes);
    let kitty = s.contains("\x1b_Gi=31;OK");
    // Cell size: CSI 6 ; height ; width t
    let cell = s.find("\x1b[6;").and_then(|i| {
        let rest = &s[i + 4..];
        let end = rest.find('t')?;
        let mut it = rest[..end].split(';').map(|n| n.parse::<u16>().ok());
        let (h, w) = (it.next()??, it.next()??);
        (h > 0 && w > 0).then_some((h, w))
    });
    // Device attributes: CSI ? 62 ; 4 ; … c — 4 is sixel.
    let sixel = s.find("\x1b[?").is_some_and(|i| {
        let rest = &s[i + 3..];
        rest.find('c')
            .is_some_and(|end| rest[..end].split(';').any(|p| p == "4"))
    });
    // XTVERSION: DCS > | name-and-version ST
    let version = s.find("\x1bP>|").and_then(|i| {
        let rest = &s[i + 4..];
        let end = rest.find("\x1b\\")?;
        let v: String = rest[..end].chars().filter(|c| !c.is_control()).collect();
        (!v.trim().is_empty()).then(|| v.trim().to_string())
    });
    // DA2: CSI > kind ; version ; … c
    let da2 = s.find("\x1b[>").and_then(|i| {
        let rest = &s[i + 3..];
        let end = rest.find('c')?;
        let mut it = rest[..end].split(';').map(|n| n.parse::<u32>().ok());
        Some((it.next()??, it.next()??))
    });
    Probe {
        kitty,
        sixel,
        cell,
        version,
        da2,
    }
}

/// The terminal xdevcmd runs in, for the status line, Usage and About: what the
/// environment says, else what the terminal answered, else "unknown".
pub fn terminal_name(var: impl Fn(&str) -> Option<String>, probe: &Probe) -> String {
    let get = |k: &str| var(k).filter(|v| !v.is_empty());
    let term = get("TERM").unwrap_or_default();
    let named = match get("TERM_PROGRAM").as_deref() {
        Some("Apple_Terminal") => Some("Terminal.app".to_string()),
        Some("iTerm.app") => Some("iTerm2".to_string()),
        Some("vscode") => Some("VS Code".to_string()),
        Some("ghostty") => Some("Ghostty".to_string()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
    .map(|n| match get("TERM_PROGRAM_VERSION") {
        Some(v) => format!("{n} {v}"),
        None => n,
    })
    .or_else(|| get("WT_SESSION").map(|_| "Windows Terminal".to_string()))
    .or_else(|| {
        (get("KITTY_WINDOW_ID").is_some() || term.contains("kitty")).then(|| "kitty".to_string())
    })
    .or_else(|| get("KONSOLE_VERSION").map(|v| format!("Konsole {v}")))
    .or_else(|| term.starts_with("foot").then(|| "foot".to_string()))
    .or_else(|| probe.version.clone())
    .or_else(|| match probe.da2 {
        Some((41, v)) => Some(format!("xterm {v}")),
        Some((65, _)) => Some("a VTE terminal (GNOME Terminal and others)".to_string()),
        Some((77, _)) => Some("mintty".to_string()),
        Some((83, _)) => Some("screen".to_string()),
        Some((84, _)) => Some("tmux".to_string()),
        _ => None,
    })
    .or_else(|| {
        get("VTE_VERSION").map(|_| "a VTE terminal (GNOME Terminal and others)".to_string())
    });
    let name = named.unwrap_or_else(|| match term.as_str() {
        "" => "an unknown terminal".to_string(),
        t => format!("an unknown terminal (TERM={t})"),
    });
    if get("SSH_CONNECTION").is_some() || get("SSH_TTY").is_some() {
        format!("{name}, over SSH")
    } else {
        name
    }
}

/// The protocol for `setting` (`[viewer] images`): a name chooses it; "auto" asks the
/// environment, `var`, then what the terminal answered, `probe`.
pub fn detect(setting: &str, var: impl Fn(&str) -> Option<String>, probe: &Probe) -> Protocol {
    match setting {
        "kitty" => return Protocol::Kitty,
        "iterm2" => return Protocol::Iterm2,
        "sixel" => return Protocol::Sixel,
        "blocks" | "off" => return Protocol::Blocks,
        _ => {}
    }
    let has = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    let is = |k: &str, v: &str| var(k).is_some_and(|x| x == v);
    let term = var("TERM").unwrap_or_default();
    // tmux passes these sequences on only when told to: half blocks always work there.
    if has("TMUX") || term.starts_with("screen") || term.starts_with("tmux") {
        return Protocol::Blocks;
    }
    if term.contains("kitty") || has("KITTY_WINDOW_ID") {
        return Protocol::Kitty;
    }
    if is("TERM_PROGRAM", "ghostty") || term.contains("ghostty") {
        return Protocol::Kitty;
    }
    // LC_TERMINAL is set by iTerm2 and, unlike TERM_PROGRAM, usually crosses SSH.
    if is("TERM_PROGRAM", "iTerm.app") || is("LC_TERMINAL", "iTerm2") {
        return Protocol::Iterm2;
    }
    if is("TERM_PROGRAM", "WezTerm") {
        return Protocol::Iterm2;
    }
    if term.starts_with("foot") {
        return Protocol::Sixel;
    }
    // Windows Terminal draws sixel from 1.22 (2024); WT_SESSION is set in every tab.
    if has("WT_SESSION") {
        return Protocol::Sixel;
    }
    // Nothing in the environment, as over SSH: what the terminal itself said.
    if probe.kitty {
        return Protocol::Kitty;
    }
    if probe.sixel {
        return Protocol::Sixel;
    }
    Protocol::Blocks
}

/// The protocol actually used. Sixel is drawn pixel for pixel, so it needs the cell's
/// real size: with only a guess, a picture made for 10x20 cells runs over its box on
/// 8x16 ones, across the pane's border and the panel below. Without the size, half
/// blocks. Kitty and iTerm2 fit the picture to the cells they are given, and are safe.
pub fn effective(p: Protocol, cell_known: bool) -> Protocol {
    if p == Protocol::Sixel && !cell_known {
        Protocol::Blocks
    } else {
        p
    }
}

/// Whether the terminal draws sixel on a fixed cell, so its size is known without
/// asking: Windows Terminal maps sixel through a virtual 10x20 cell (VT340's), whatever
/// the font, which is xdevcmd's guess. Its tabs carry `WT_SESSION`.
pub fn fixed_sixel_cell(var: impl Fn(&str) -> Option<String>) -> bool {
    var("WT_SESSION").is_some_and(|v| !v.is_empty())
}

/// The bytes that draw `pic` with its top-left corner at the cursor, over `cols` x `rows`
/// cells. `id` names a Kitty picture so it can be removed. The cursor is left where it
/// was.
pub fn encode(p: Protocol, pic: &Picture, cols: u16, rows: u16, id: u32) -> Vec<u8> {
    match p {
        Protocol::Kitty => kitty(pic, cols, rows, id),
        Protocol::Iterm2 => iterm2(pic, cols, rows),
        Protocol::Sixel => sixel(pic),
        Protocol::Blocks => Vec::new(),
    }
}

/// Removes a Kitty picture. iTerm2 and sixel pictures are cells, gone when the cells
/// are drawn over.
pub fn kitty_delete(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\").into_bytes()
}

/// Kitty: the picture as PNG (f=100), base64, in chunks of 4096 (the protocol's limit),
/// fitted by the terminal to the cells given; C=1 keeps the cursor where it is. PNG
/// rather than raw RGB: a fraction of the bytes, which matters over SSH.
fn kitty(pic: &Picture, cols: u16, rows: u16, id: u32) -> Vec<u8> {
    let Ok(png) = xdevcmd_core::image::to_png(pic) else {
        return Vec::new();
    };
    let data = base64(&png);
    let mut out = Vec::with_capacity(data.len() + 256);
    let chunks: Vec<&[u8]> = data.as_bytes().chunks(4096).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        if i == 0 {
            out.extend_from_slice(
                format!("\x1b_Ga=T,f=100,c={cols},r={rows},i={id},q=2,C=1,m={more};").as_bytes(),
            );
        } else {
            out.extend_from_slice(format!("\x1b_Gm={more};").as_bytes());
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out
}

/// iTerm2: the picture as a PNG file, inline, fitted to the cells given.
fn iterm2(pic: &Picture, cols: u16, rows: u16) -> Vec<u8> {
    let Ok(png) = xdevcmd_core::image::to_png(pic) else {
        return Vec::new();
    };
    format!(
        "\x1b]1337;File=inline=1;size={};width={cols};height={rows};preserveAspectRatio=1:{}\x07",
        png.len(),
        base64(&png)
    )
    .into_bytes()
}

/// Sixel: drawn at the picture's own size in pixels, so `pic` must already fit the cells.
fn sixel(pic: &Picture) -> Vec<u8> {
    let rgba: Vec<u8> = pic
        .rgb
        .iter()
        .flat_map(|[r, g, b]| [*r, *g, *b, 255])
        .collect();
    icy_sixel::SixelImage::try_from_rgba(rgba, pic.width, pic.height)
        .and_then(|i| i.encode())
        .map(String::into_bytes)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    fn detect_env(setting: &str, var: impl Fn(&str) -> Option<String>) -> Protocol {
        detect(setting, var, &Probe::default())
    }

    #[test]
    fn sixel_only_where_the_cell_size_is_known() {
        assert_eq!(effective(Protocol::Sixel, false), Protocol::Blocks);
        assert_eq!(effective(Protocol::Sixel, true), Protocol::Sixel);
        for p in [Protocol::Kitty, Protocol::Iterm2, Protocol::Blocks] {
            assert_eq!(effective(p, false), p);
        }
        assert!(fixed_sixel_cell(env(&[("WT_SESSION", "x")])));
        assert!(!fixed_sixel_cell(env(&[("WT_SESSION", "")])));
        assert!(!fixed_sixel_cell(env(&[])));
    }

    #[test]
    fn what_the_terminal_answers_decides_when_the_environment_says_nothing() {
        // Windows Terminal over SSH: sixel in its device attributes, its cell size.
        let wt = parse_probe(b"\x1b[6;20;10t\x1b[?61;4;6;7;14;21;22;23;24;28;32;42c");
        assert_eq!(
            wt,
            Probe {
                kitty: false,
                sixel: true,
                cell: Some((20, 10)),
                version: None,
                da2: None,
            }
        );
        assert_eq!(detect("auto", |_| None, &wt), Protocol::Sixel);
        // kitty or Ghostty: the graphics query answered OK.
        let k = parse_probe(b"\x1b_Gi=31;OK\x1b\\\x1b[6;34;16t\x1b[?62;22c");
        assert!(k.kitty && !k.sixel);
        assert_eq!(detect("auto", |_| None, &k), Protocol::Kitty);
        // A terminal with neither: half blocks.
        let plain = parse_probe(b"\x1b[?1;2c");
        assert_eq!(detect("auto", |_| None, &plain), Protocol::Blocks);
        // tmux answers for itself: never trusted.
        assert_eq!(
            detect("auto", |k| (k == "TMUX").then(|| "x".into()), &wt),
            Protocol::Blocks
        );
        assert!(answered(b"\x1b[?62;4c") && !answered(b"\x1b[6;20;10t"));
    }

    #[test]
    fn the_terminal_is_recognised_from_its_environment() {
        assert_eq!(
            detect_env("auto", env(&[("TERM", "xterm-kitty")])),
            Protocol::Kitty
        );
        assert_eq!(
            detect_env("auto", env(&[("TERM_PROGRAM", "ghostty")])),
            Protocol::Kitty
        );
        assert_eq!(
            detect_env("auto", env(&[("TERM_PROGRAM", "iTerm.app")])),
            Protocol::Iterm2
        );
        // Over SSH, iTerm2 is known by LC_TERMINAL.
        assert_eq!(
            detect_env("auto", env(&[("LC_TERMINAL", "iTerm2")])),
            Protocol::Iterm2
        );
        assert_eq!(
            detect_env("auto", env(&[("TERM_PROGRAM", "WezTerm")])),
            Protocol::Iterm2
        );
        assert_eq!(
            detect_env("auto", env(&[("WT_SESSION", "x")])),
            Protocol::Sixel
        );
        assert_eq!(
            detect_env("auto", env(&[("TERM", "foot")])),
            Protocol::Sixel
        );
        // Terminal.app, VS Code, an unknown terminal: half blocks.
        assert_eq!(
            detect_env("auto", env(&[("TERM_PROGRAM", "Apple_Terminal")])),
            Protocol::Blocks
        );
        assert_eq!(
            detect_env("auto", env(&[("TERM_PROGRAM", "vscode")])),
            Protocol::Blocks
        );
        assert_eq!(detect_env("auto", env(&[])), Protocol::Blocks);
        // Inside tmux, half blocks, whatever is outside.
        assert_eq!(
            detect_env(
                "auto",
                env(&[("TERM", "xterm-kitty"), ("TMUX", "/tmp/t,1,0")])
            ),
            Protocol::Blocks
        );
        // The setting wins.
        assert_eq!(
            detect_env("sixel", env(&[("TERM", "xterm-kitty")])),
            Protocol::Sixel
        );
        assert_eq!(
            detect_env("blocks", env(&[("TERM", "xterm-kitty")])),
            Protocol::Blocks
        );
    }

    fn pic(w: usize, h: usize) -> Picture {
        Picture {
            width: w,
            height: h,
            source: (w, h),
            rgb: vec![[200, 10, 10]; w * h],
        }
    }

    #[test]
    fn kitty_sends_a_png_in_chunks_and_can_remove_it() {
        // Noise, so the PNG does not shrink to one chunk.
        let mut p = pic(96, 96);
        for (i, px) in p.rgb.iter_mut().enumerate() {
            *px = [(i * 7) as u8, (i * 13) as u8, (i * 31) as u8];
        }
        let s = String::from_utf8(encode(Protocol::Kitty, &p, 10, 5, 7)).unwrap();
        assert!(
            s.starts_with("\x1b_Ga=T,f=100,c=10,r=5,i=7,q=2,C=1,m=1;iVBORw0KGgo"),
            "{}",
            &s[..60]
        );
        assert!(s.matches("\x1b_G").count() > 1, "in chunks");
        assert!(s.contains("\x1b_Gm=0;"));
        assert!(s.ends_with("\x1b\\"));
        assert_eq!(
            String::from_utf8(kitty_delete(7)).unwrap(),
            "\x1b_Ga=d,d=I,i=7,q=2\x1b\\"
        );
    }

    #[test]
    fn iterm2_sends_a_png_fitted_to_the_cells() {
        let s = String::from_utf8(encode(Protocol::Iterm2, &pic(4, 4), 3, 2, 0)).unwrap();
        assert!(s.starts_with("\x1b]1337;File=inline=1;size="), "{s}");
        assert!(
            s.contains(";width=3;height=2;preserveAspectRatio=1:iVBORw0KGgo"),
            "{s}"
        );
        assert!(s.ends_with('\x07'));
    }

    #[test]
    fn sixel_is_a_dcs_sequence() {
        let s = String::from_utf8(encode(Protocol::Sixel, &pic(6, 6), 1, 1, 0)).unwrap();
        assert!(s.starts_with("\x1bP"), "{s:?}");
        assert!(s.ends_with("\x1b\\"), "{s:?}");
        assert!(encode(Protocol::Blocks, &pic(2, 2), 1, 1, 0).is_empty());
    }

    #[test]
    fn the_terminal_is_named_from_the_environment_or_its_answers() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let none = Probe::default();
        assert_eq!(
            terminal_name(
                env(&[
                    ("TERM_PROGRAM", "Apple_Terminal"),
                    ("TERM_PROGRAM_VERSION", "455")
                ]),
                &none
            ),
            "Terminal.app 455"
        );
        assert_eq!(
            terminal_name(env(&[("WT_SESSION", "x")]), &none),
            "Windows Terminal"
        );
        // Over SSH, from its answers.
        let p = parse_probe(b"\x1bP>|WezTerm 20240203\x1b\\\x1b[>1;277;0c\x1b[?62;4c");
        assert_eq!(p.version.as_deref(), Some("WezTerm 20240203"));
        assert_eq!(p.da2, Some((1, 277)));
        assert_eq!(
            terminal_name(env(&[("SSH_CONNECTION", "1 2 3 4")]), &p),
            "WezTerm 20240203, over SSH"
        );
        let x = parse_probe(b"\x1b[>41;390;0c\x1b[?65;4c");
        assert_eq!(terminal_name(env(&[]), &x), "xterm 390");
        assert_eq!(
            terminal_name(env(&[("TERM", "xterm-256color")]), &none),
            "an unknown terminal (TERM=xterm-256color)"
        );
    }
}
