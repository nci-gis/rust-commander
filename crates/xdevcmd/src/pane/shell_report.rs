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
    // `%0A` and the like decode to control characters: no directory name has them
    // honestly, and they would reach the status line. Refused, as 9;9 refuses them.
    if path.iter().any(|&b| b < 0x20 || b == 0x7f) {
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
/// Control characters it produces are refused by the caller ([`osc7`]).
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
pub(crate) fn path_from_bytes(b: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(b)))
}

#[cfg(not(unix))]
pub(crate) fn path_from_bytes(b: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(b).ok().map(PathBuf::from)
}

/// A command the shell ran, from its `133;C` mark: the line its output starts on, and
/// the directory it ran in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// The line's number since the shell started, counting the lines the history has
    /// dropped: it never drifts as old lines go.
    pub line: u64,
    pub dir: Option<PathBuf>,
}

/// The commands kept, the newest last; older ones are forgotten.
const COMMANDS_KEPT: usize = 200;

/// Counts the line breaks written to it, and keeps nothing.
#[derive(Default)]
struct Breaks(u64);

impl std::fmt::Write for Breaks {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0 += s.bytes().filter(|&b| b == b'\n').count() as u64;
        Ok(())
    }
}

/// The vt100 callbacks of one shell: the last directory and prompt mark it reported,
/// and where each command it ran began.
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
    /// A prompt mark (A, B or D) has come: the shell sends marks. Until then a `C` is
    /// ignored — printed by `cat` of a file in a shell that sends none, it would make
    /// the shell look busy until it ended.
    pub prompted: bool,
    /// The shell's process, once started: on Linux its directory is read from `/proc`
    /// at each prompt, rather than taken from what the output claims.
    pid: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    /// The lines the history has dropped off its top so far (or `clear` erased), as
    /// line breaks: the rows vt100 hands [`vt100::Callbacks::on_scroll`], written out
    /// as a capture would write them, so a long line wrapped over rows counts once.
    dropped: u64,
    /// That capture's state: whether the last dropped row ended its line.
    stream: vt100::capture::BasicFormattedCaptureState,
    /// The commands run, oldest first.
    pub commands: std::collections::VecDeque<Command>,
    /// The shell's directory at its last prompt (`133;A`). Read then, while the shell
    /// waits for a line, not when the command starts: the output is parsed behind the
    /// shell, which may have run `cd` by the time its `133;C` is read.
    prompt_dir: Option<PathBuf>,
}

impl ShellReports {
    pub fn new(host: String, windows: bool) -> Self {
        Self {
            host,
            windows,
            ..Self::default()
        }
    }

    /// Read the shell's directory from the system when a command starts (Linux).
    pub fn with_pid(mut self, pid: std::sync::Arc<std::sync::atomic::AtomicU32>) -> Self {
        self.pid = Some(pid);
        self
    }

    /// The directory a command starting now runs in: on Linux the shell's, from
    /// `/proc`; elsewhere the last one the shell reported.
    fn dir_now(&self) -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        if let Some(pid) = self
            .pid
            .as_ref()
            .map(|p| p.load(std::sync::atomic::Ordering::Relaxed))
            .filter(|&p| p != 0)
        {
            return std::fs::read_link(format!("/proc/{pid}/cwd")).ok();
        }
        self.dir.clone()
    }

    /// The history's text as a capture gives it, and the number of its first line in
    /// the count [`Command::line`] uses.
    pub fn history(&self, screen: &vt100::Screen) -> (String, u64) {
        let mut basic = String::new();
        let mut state = self.stream.clone();
        let _ = screen.write_contents_formatted_basic(
            &mut basic,
            vt100::capture::BasicFormattedCaptureRange::Full(&mut state),
        );
        // The last dropped row did not end its line: the capture starts by ending it.
        let pending = basic.starts_with('\n');
        let mut text: String = vt100::capture::basic_formatted_to_plain(&basic).collect();
        if pending {
            text.remove(0);
        }
        (text, self.dropped + u64::from(pending))
    }

    /// A `133;C`: the command's output starts on the cursor's row.
    fn command_starts(&mut self, screen: &vt100::Screen) {
        let mut breaks = Breaks::default();
        let mut state = self.stream.clone();
        let _ = screen.write_contents_formatted_basic(
            &mut breaks,
            vt100::capture::BasicFormattedCaptureRange::Full(&mut state),
        );
        // The last row's line is the `breaks`-th after those dropped; the rows below the
        // cursor are blank, a line each.
        let (rows, _) = screen.size();
        let (row, _) = screen.cursor_position();
        let below = u64::from(rows.get().saturating_sub(1).saturating_sub(row));
        let line = (self.dropped + breaks.0).saturating_sub(below);
        let dir = self.prompt_dir.clone().or_else(|| self.dir_now());
        if self.commands.len() == COMMANDS_KEPT {
            self.commands.pop_front();
        }
        self.commands.push_back(Command { line, dir });
    }

    /// One OSC, as vt100 split it.
    pub fn osc(&mut self, params: &[&[u8]]) {
        if let Some(m) = osc133(params) {
            match m {
                Mark::Prompt => self.prompted = true,
                Mark::Running if !self.prompted => return,
                Mark::Running => {}
            }
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
    fn unhandled_osc(&mut self, screen: &mut vt100::Screen, params: &[&[u8]]) {
        self.osc(params);
        match osc133(params) {
            Some(Mark::Prompt) => self.prompt_dir = self.dir_now(),
            Some(Mark::Running) if self.prompted && !screen.alternate_screen() => {
                self.command_starts(screen);
            }
            _ => {}
        }
    }

    fn on_scroll(&mut self, row: vt100::capture::RowContents<'_>, alternate_screen: bool) {
        // The other screen (a full-screen program's) has no history.
        if alternate_screen {
            return;
        }
        let mut breaks = Breaks::default();
        let _ = row.write_formatted_basic(&mut breaks, &mut self.stream);
        self.dropped += breaks.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(n: u16) -> std::num::NonZeroU16 {
        std::num::NonZeroU16::new(n).expect("a size")
    }

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
        let mut p = vt100::Parser::new_with_callbacks(
            nz(5),
            nz(40),
            0,
            ShellReports::new("box".into(), false),
        );
        // One OSC cut in three reads, BEL-terminated, then one with ST.
        p.process(b"$ \x1b]7;file://box/tm");
        p.process(b"p/a%20");
        p.process(b"b\x07ok");
        assert_eq!(p.callbacks().dir, Some("/tmp/a b".into()));
        assert_eq!(p.callbacks().dir_seq, 1);
        p.process(b"\x1b]133;A\x07");
        p.process(b"\x1b]133;C\x1b\\");
        assert_eq!(p.callbacks().mark, Some(Mark::Running));
        p.process(b"\x1b]133;D;0\x07\x1b]7;file:///other\x1b\\");
        assert_eq!(p.callbacks().mark, Some(Mark::Prompt));
        assert_eq!(p.callbacks().dir, Some("/other".into()));
        assert_eq!(p.callbacks().dir_seq, 2);
        // The screen is drawn as before: the OSCs are not text.
        assert!(p.screen().contents().starts_with("$ ok"));
    }

    #[test]
    fn a_running_mark_counts_only_from_a_shell_that_sends_prompt_marks() {
        let mut r = ShellReports::new("box".into(), false);
        r.osc(&split(b"133;C"));
        assert_eq!(r.mark, None, "a C before any prompt mark");
        r.osc(&split(b"133;D;0"));
        r.osc(&split(b"133;C"));
        assert_eq!(r.mark, Some(Mark::Running));
    }

    #[test]
    fn a_control_character_in_a_reported_path_refuses_it() {
        assert_eq!(o7("7;file:///tmp/a%0Ab", false), None);
        assert_eq!(o7("7;file:///tmp/a%1Bb", false), None);
        assert_eq!(o7("7;file:///tmp/a%7Fb", false), None);
        assert_eq!(o7("7;file:///tmp/a%09b", false), None);
        assert_eq!(o7("7;file:///tmp/a%20b", false), Some("/tmp/a b".into()));
    }

    /// The bytes docs/shell.md's bash snippet printed in bash 5.2 (recorded): after a
    /// command, `cd "a b%c"`, then `cd é`.
    #[cfg(unix)]
    #[test]
    fn the_bash_snippet_in_the_docs_is_understood() {
        let mut p = vt100::Parser::new_with_callbacks(
            nz(5),
            nz(80),
            0,
            ShellReports::new("vm".into(), false),
        );
        p.process(b"\x1b]133;C\x07\x1b]133;D\x07\x1b]7;file://vm/tmp/sn/a%20b%25c\x1b\\");
        assert_eq!(p.callbacks().mark, Some(Mark::Prompt));
        p.process(b"\x1b]133;A\x07$ cd \xc3\xa9\r\n\x1b]133;C\x07");
        assert_eq!(p.callbacks().dir, Some("/tmp/sn/a b%c".into()));
        assert_eq!(p.callbacks().mark, Some(Mark::Running));
        p.process(b"\x1b]133;D\x07\x1b]7;file://vm/tmp/sn/%C3%A9\x1b\\\x1b]133;A\x07");
        assert_eq!(p.callbacks().dir, Some("/tmp/sn/\u{e9}".into()));
        assert_eq!(p.callbacks().mark, Some(Mark::Prompt));
        assert_eq!(p.callbacks().dir_seq, 2);
    }

    /// Each command's first line of output, as the history view finds it.
    fn starts(p: &vt100::Parser<ShellReports>) -> Vec<String> {
        let (text, first) = p.callbacks().history(p.screen());
        let lines: Vec<&str> = text.lines().collect();
        p.callbacks()
            .commands
            .iter()
            .filter(|c| c.line >= first)
            .map(|c| {
                lines
                    .get((c.line - first) as usize)
                    .unwrap_or(&"?")
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// A prompt, the command, its output: as a shell with the docs' snippet sends it.
    fn run(p: &mut vt100::Parser<ShellReports>, cmd: &str, out: &[&str]) {
        p.process(format!("\x1b]133;A\x07$ {cmd}\r\n\x1b]133;C\x07").as_bytes());
        for l in out {
            p.process(format!("{l}\r\n").as_bytes());
        }
        p.process(b"\x1b]133;D;0\x07");
    }

    #[test]
    fn a_command_is_found_again_at_its_first_line_of_output() {
        let mut p = vt100::Parser::new_with_callbacks(
            nz(6),
            nz(20),
            100,
            ShellReports::new("b".into(), false),
        );
        run(&mut p, "ls", &["Cargo.toml", "src"]);
        run(&mut p, "grep", &["src/a.rs:3:x"]);
        assert_eq!(starts(&p), ["Cargo.toml", "src/a.rs:3:x"]);
        // A line longer than the screen is wide wraps over rows: still one line.
        run(&mut p, "echo", &[&"w".repeat(45)]);
        run(&mut p, "pwd", &["/tmp"]);
        assert_eq!(starts(&p)[3], "/tmp");
    }

    #[test]
    fn commands_keep_their_lines_as_the_history_drops_old_ones() {
        // Room for 6 rows on screen and 4 in the history.
        let mut p = vt100::Parser::new_with_callbacks(
            nz(6),
            nz(20),
            4,
            ShellReports::new("b".into(), false),
        );
        run(&mut p, "first", &["one"]);
        for i in 0..6 {
            run(&mut p, "n", &[&format!("out{i}"), &"z".repeat(30)]);
        }
        let s = starts(&p);
        assert_eq!(s.last().map(String::as_str), Some("out5"), "{s:?}");
        assert!(
            !s.contains(&"one".to_string()),
            "dropped with its lines: {s:?}"
        );
        // `clear` erases the history; the commands before it go with it.
        p.process(b"\x1b[H\x1b[2J\x1b[3J");
        run(&mut p, "after", &["fresh"]);
        assert_eq!(starts(&p).last().map(String::as_str), Some("fresh"));
    }

    #[test]
    fn a_running_mark_from_a_shell_without_prompt_marks_records_nothing() {
        let mut p = vt100::Parser::new_with_callbacks(
            nz(6),
            nz(20),
            10,
            ShellReports::new("b".into(), false),
        );
        p.process(b"\x1b]133;C\x07text\r\n");
        assert!(p.callbacks().commands.is_empty());
    }
}
