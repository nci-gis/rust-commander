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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Kitty,
    Iterm2,
    Sixel,
    /// Half blocks: two pixels a cell, in any terminal.
    Blocks,
}

/// The protocol for `setting` (`[viewer] images`): a name chooses it; "auto" asks the
/// environment, `var`.
pub fn detect(setting: &str, var: impl Fn(&str) -> Option<String>) -> Protocol {
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
    Protocol::Blocks
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

/// Kitty: raw RGB, base64, in chunks of 4096 (the protocol's limit), scaled by the
/// terminal to the cells given; C=1 keeps the cursor where it is.
fn kitty(pic: &Picture, cols: u16, rows: u16, id: u32) -> Vec<u8> {
    let raw: Vec<u8> = pic.rgb.iter().flatten().copied().collect();
    let data = base64(&raw);
    let mut out = Vec::with_capacity(data.len() + 256);
    let chunks: Vec<&[u8]> = data.as_bytes().chunks(4096).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        if i == 0 {
            out.extend_from_slice(
                format!(
                    "\x1b_Ga=T,f=24,s={},v={},c={cols},r={rows},i={id},q=2,C=1,m={more};",
                    pic.width, pic.height
                )
                .as_bytes(),
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

/// Standard base64, padded: all these protocols want it, and it is a dozen lines.
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
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

    #[test]
    fn the_terminal_is_recognised_from_its_environment() {
        assert_eq!(
            detect("auto", env(&[("TERM", "xterm-kitty")])),
            Protocol::Kitty
        );
        assert_eq!(
            detect("auto", env(&[("TERM_PROGRAM", "ghostty")])),
            Protocol::Kitty
        );
        assert_eq!(
            detect("auto", env(&[("TERM_PROGRAM", "iTerm.app")])),
            Protocol::Iterm2
        );
        // Over SSH, iTerm2 is known by LC_TERMINAL.
        assert_eq!(
            detect("auto", env(&[("LC_TERMINAL", "iTerm2")])),
            Protocol::Iterm2
        );
        assert_eq!(
            detect("auto", env(&[("TERM_PROGRAM", "WezTerm")])),
            Protocol::Iterm2
        );
        assert_eq!(detect("auto", env(&[("WT_SESSION", "x")])), Protocol::Sixel);
        assert_eq!(detect("auto", env(&[("TERM", "foot")])), Protocol::Sixel);
        // Terminal.app, VS Code, an unknown terminal: half blocks.
        assert_eq!(
            detect("auto", env(&[("TERM_PROGRAM", "Apple_Terminal")])),
            Protocol::Blocks
        );
        assert_eq!(
            detect("auto", env(&[("TERM_PROGRAM", "vscode")])),
            Protocol::Blocks
        );
        assert_eq!(detect("auto", env(&[])), Protocol::Blocks);
        // Inside tmux, half blocks, whatever is outside.
        assert_eq!(
            detect(
                "auto",
                env(&[("TERM", "xterm-kitty"), ("TMUX", "/tmp/t,1,0")])
            ),
            Protocol::Blocks
        );
        // The setting wins.
        assert_eq!(
            detect("sixel", env(&[("TERM", "xterm-kitty")])),
            Protocol::Sixel
        );
        assert_eq!(
            detect("blocks", env(&[("TERM", "xterm-kitty")])),
            Protocol::Blocks
        );
    }

    #[test]
    fn base64_is_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
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
    fn kitty_sends_raw_rgb_in_chunks_and_can_remove_it() {
        let p = pic(64, 64); // 12,288 bytes, base64 16,384: four chunks
        let s = String::from_utf8(encode(Protocol::Kitty, &p, 10, 5, 7)).unwrap();
        assert!(
            s.starts_with("\x1b_Ga=T,f=24,s=64,v=64,c=10,r=5,i=7,q=2,C=1,m=1;"),
            "{}",
            &s[..60]
        );
        assert_eq!(s.matches("\x1b_G").count(), 4);
        assert!(s.ends_with("\x1b_Gm=0;".to_string().as_str()) || s.contains("\x1b_Gm=0;"));
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
}
