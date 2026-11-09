//! What a shell says about itself in its output: its directory (OSC 7, or Windows
//! Terminal's OSC 9;9) and where its prompt and commands begin and end (OSC 133).
//!
//! vt100 hands every OSC it does not draw to [`vt100::Callbacks::unhandled_osc`],
//! already split at `;` and with the terminator (BEL or ST) gone, however the bytes
//! were cut between reads: its parser (vte) keeps state across `process` calls. So
//! nothing here has to scan the byte stream; [`ShellReports`] is that callback.
//!
//! Anything printed into the terminal can say these things, not only the shell: `cat`
//! of a hostile file can claim a directory or a prompt. Hence the rules: a directory
//! reported here only ever moves a file pane, after it is checked to exist and to lie
//! inside the policy root; on Linux it is not used at all (`/proc` knows better); and a
//! prompt mark never overrules a running program the system can see (Linux, macOS).

use std::path::PathBuf;

/// vte keeps at most this many `;`-separated parameters of an OSC and drops the rest
/// of the text. A report with that many may have lost the end of its path.
const VTE_MAX_PARAMS: usize = 16;

/// A semantic prompt mark (OSC 133), as FinalTerm named them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// `A` (prompt starts), `B` (command line starts) or `D` (command finished): the
    /// shell is at its prompt.
    Prompt,
    /// `C`: the command line was accepted and is running.
    Running,
}

/// The directory in an OSC 7 report, `7;file://HOST/PATH`. `params` is vt100's split
/// of the OSC (the first is `7`); a path holding `;` arrives in several and is joined
/// back. Only a local HOST is accepted: empty, `localhost`, or `ours` (this machine's
/// name, compared without case). The path is percent-decoded. With `windows`,
/// `/C:/Users/me` is `C:\Users\me`. `None` for anything else, never a panic.
pub fn osc7(params: &[&[u8]], ours: &str, windows: bool) -> Option<PathBuf> {
    let (first, rest) = params.split_first()?;
    if *first != b"7" || rest.is_empty() || params.len() >= VTE_MAX_PARAMS {
        return None;
    }
    let url = rest.join(&b';');
    let after = url.strip_prefix(b"file://")?;
    let slash = after.iter().position(|&b| b == b'/')?;
    let host = std::str::from_utf8(&after[..slash]).ok()?;
    let local = host.is_empty()
        || host.eq_ignore_ascii_case("localhost")
        || (!ours.is_empty() && host.eq_ignore_ascii_case(ours));
    if !local {
        return None;
    }
    let path = percent_decode(&after[slash..])?;
    if path.contains(&0) {
        return None;
    }
    if windows {
        let text = String::from_utf8(path).ok()?;
        // `/C:/x` is how a file URL spells `C:\x`.
        let bytes = text.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':';
        if !drive {
            return None;
        }
        let mut p = text[1..].replace('/', "\\");
        if p.len() == 2 {
            p.push('\\');
        }
        return Some(PathBuf::from(p));
    }
    path_from_bytes(path)
}

/// The directory in Windows Terminal's `9;9;"C:\path"` (quotes optional). With
/// `windows`, it must be absolute the Windows way (`C:\…` or `\\server\…`); elsewhere,
/// start with `/`. `None` for anything else.
pub fn osc9_9(params: &[&[u8]], windows: bool) -> Option<PathBuf> {
    if params.len() < 3 || params[0] != b"9" || params[1] != b"9" || params.len() >= VTE_MAX_PARAMS
    {
        return None;
    }
    let raw = params[2..].join(&b';');
    let text = std::str::from_utf8(&raw).ok()?;
    let text = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text);
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }
    let b = text.as_bytes();
    let absolute = if windows {
        (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
            || text.starts_with("\\\\")
    } else {
        text.starts_with('/')
    };
    absolute.then(|| PathBuf::from(text))
}

/// The mark in `133;X…`; `None` for other OSCs and for marks that say nothing about
/// whether a command runs (`P`, `L`, `E`, …).
pub fn osc133(params: &[&[u8]]) -> Option<Mark> {
    if params.len() < 2 || params[0] != b"133" {
        return None;
    }
    match params[1].first()? {
        b'A' | b'B' | b'D' => Some(Mark::Prompt),
        b'C' => Some(Mark::Running),
        _ => None,
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `%2F` and friends back to bytes; a `%` not followed by two hex digits is garbage.
fn percent_decode(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' {
            let hi = hex(*s.get(i + 1)?)?;
            let lo = hex(*s.get(i + 2)?)?;
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    Some(out)
}

#[cfg(unix)]
fn path_from_bytes(b: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(b)))
}

#[cfg(not(unix))]
fn path_from_bytes(b: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(b).ok().map(PathBuf::from)
}

/// The vt100 callbacks of one shell: the last directory and prompt mark it reported.
#[derive(Debug, Default)]
pub struct ShellReports {
    /// This machine's name, for OSC 7's host.
    host: String,
    windows: bool,
    /// The directory last reported (OSC 7 or 9;9).
    pub dir: Option<PathBuf>,
    /// Counts directory reports, so a reader can tell a new one from the one it saw.
    pub dir_seq: u64,
    /// The last prompt mark.
    pub mark: Option<Mark>,
}

impl ShellReports {
    pub fn new(host: String, windows: bool) -> Self {
        Self {
            host,
            windows,
            ..Self::default()
        }
    }

    /// One OSC, as vt100 split it.
    pub fn osc(&mut self, params: &[&[u8]]) {
        if let Some(m) = osc133(params) {
            self.mark = Some(m);
        } else if let Some(d) =
            osc7(params, &self.host, self.windows).or_else(|| osc9_9(params, self.windows))
        {
            self.dir = Some(d);
            self.dir_seq += 1;
        }
    }
}

impl vt100::Callbacks for ShellReports {
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        self.osc(params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &[u8]) -> Vec<&[u8]> {
        s.split(|&b| b == b';').collect()
    }

    fn o7(s: &str, windows: bool) -> Option<PathBuf> {
        osc7(&split(s.as_bytes()), "box", windows)
    }

    #[test]
    fn osc7_takes_only_this_machine() {
        assert_eq!(o7("7;file:///home/me", false), Some("/home/me".into()));
        assert_eq!(o7("7;file://localhost/x", false), Some("/x".into()));
        assert_eq!(o7("7;file://LocalHost/x", false), Some("/x".into()));
        assert_eq!(o7("7;file://box/x", false), Some("/x".into()));
        assert_eq!(o7("7;file://BOX/x", false), Some("/x".into()));
        assert_eq!(o7("7;file://other/x", false), None, "another machine");
        assert_eq!(osc7(&split(b"7;file:///x"), "", false), Some("/x".into()));
        assert_eq!(
            osc7(&split(b"7;file://box/x"), "", false),
            None,
            "a name when ours is unknown"
        );
    }

    #[test]
    fn osc7_decodes_and_joins() {
        assert_eq!(
            o7("7;file:///home/me/a%20b%25", false),
            Some("/home/me/a b%".into())
        );
        // vt100 splits at `;`: a path holding one comes back whole.
        assert_eq!(o7("7;file:///tmp/a;b;c", false), Some("/tmp/a;b;c".into()));
        assert_eq!(o7("7;file:///tmp/%3B", false), Some("/tmp/;".into()));
        assert_eq!(o7("7;file:///bad%2", false), None, "cut escape");
        assert_eq!(o7("7;file:///bad%zz", false), None, "not hex");
        assert_eq!(o7("7;file:///nul%00x", false), None, "NUL");
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let p = o7("7;file:///odd%FF", false).unwrap();
            assert_eq!(p.as_os_str().as_bytes(), b"/odd\xff", "not UTF-8 is kept");
        }
        let long = format!("7;file:///{}", "d/".repeat(2000));
        assert_eq!(o7(&long, false).map(|p| p.as_os_str().len()), Some(4001));
    }

    #[test]
    fn osc7_on_windows_is_a_drive_path() {
        assert_eq!(
            o7("7;file:///C:/Users/me", true),
            Some(PathBuf::from("C:\\Users\\me"))
        );
        assert_eq!(
            o7("7;file:///c%3A/x%20y", true),
            Some(PathBuf::from("c:\\x y"))
        );
        assert_eq!(o7("7;file:///D:", true), Some(PathBuf::from("D:\\")));
        assert_eq!(o7("7;file:///home/me", true), None, "no drive");
        assert_eq!(o7("7;file:///C:/%FF", true), None, "not UTF-8");
    }

    #[test]
    fn garbage_is_ignored() {
        for s in [
            "",
            "7",
            "7;",
            "7;http://x/y",
            "7;file://",
            "7;file://nohostslash",
            "8;file:///x",
            "9",
            "9;9",
            "9;9;",
            "9;9;relative",
            "133",
            "133;",
            "133;Z",
            "9;4;1;50",
        ] {
            let p = split(s.as_bytes());
            assert_eq!(osc7(&p, "box", false), None, "{s:?}");
            assert_eq!(osc9_9(&p, false), None, "{s:?}");
            assert_eq!(osc9_9(&p, true), None, "{s:?}");
            assert_eq!(osc133(&p), None, "{s:?}");
        }
        // Too many pieces: vte may have dropped the end of the path.
        let many = format!("7;file:///{}", "a;".repeat(20));
        assert_eq!(o7(&many, false), None);
        let mut r = ShellReports::new("box".into(), false);
        r.osc(&[]);
        r.osc(&[b"\xff\xfe"]);
        assert_eq!((r.dir, r.mark, r.dir_seq), (None, None, 0));
    }

    #[test]
    fn osc9_9_with_and_without_quotes() {
        let p = |s: &str, w| osc9_9(&split(s.as_bytes()), w);
        assert_eq!(
            p("9;9;\"C:\\Users\\me\"", true),
            Some("C:\\Users\\me".into())
        );
        assert_eq!(p("9;9;C:\\Users\\me", true), Some("C:\\Users\\me".into()));
        assert_eq!(p("9;9;\\\\srv\\share", true), Some("\\\\srv\\share".into()));
        assert_eq!(p("9;9;\"C:\\a;b\"", true), Some("C:\\a;b".into()));
        assert_eq!(p("9;9;/home/me", true), None);
        assert_eq!(p("9;9;/home/me", false), Some("/home/me".into()));
        assert_eq!(p("9;9;C:\\x", false), None);
    }

    #[test]
    fn prompt_marks() {
        let m = |s: &str| osc133(&split(s.as_bytes()));
        assert_eq!(m("133;A"), Some(Mark::Prompt));
        assert_eq!(m("133;A;aid=12"), Some(Mark::Prompt));
        assert_eq!(m("133;B"), Some(Mark::Prompt));
        assert_eq!(m("133;C"), Some(Mark::Running));
        assert_eq!(m("133;D;0"), Some(Mark::Prompt));
        assert_eq!(m("133;P;k=i"), None);
    }

    #[test]
    fn the_callback_keeps_the_last_report_across_split_reads() {
        let mut p =
            vt100::Parser::new_with_callbacks(5, 40, 0, ShellReports::new("box".into(), false));
        // One OSC cut in three reads, BEL-terminated, then one with ST.
        p.process(b"$ \x1b]7;file://box/tm");
        p.process(b"p/a%20");
        p.process(b"b\x07ok");
        assert_eq!(p.callbacks().dir, Some("/tmp/a b".into()));
        assert_eq!(p.callbacks().dir_seq, 1);
        p.process(b"\x1b]133;C\x1b\\");
        assert_eq!(p.callbacks().mark, Some(Mark::Running));
        p.process(b"\x1b]133;D;0\x07\x1b]7;file:///other\x1b\\");
        assert_eq!(p.callbacks().mark, Some(Mark::Prompt));
        assert_eq!(p.callbacks().dir, Some("/other".into()));
        assert_eq!(p.callbacks().dir_seq, 2);
        // The screen is drawn as before: the OSCs are not text.
        assert!(p.screen().contents().starts_with("$ ok"));
    }
}
