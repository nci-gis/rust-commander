//! A real shell, running in a real pty, drawn as a widget.
//!
//! Three threads per terminal, which is the shape portable-pty pushes you into: one
//! waiting on the child, one draining its output into the vt100 parser, one feeding it
//! input. They all end when the pty closes.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::theme;

/// Smallest grid vt100 is ever given. With one row, a line that wraps underflows a row
/// index inside vt100 (grid.rs, `prev_pos.row -= scrolled`) and panics the reader
/// thread, freezing that shell without a word. Reproduced at 80x24: split the
/// terminal top and bottom, run anything with a long line of output. The grid may be
/// taller than the pane showing it; it may never be one row.
const MIN_ROWS: u16 = 2;
const MIN_COLS: u16 = 2;

/// Set by any shell's output thread when the screen it feeds has changed; the main loop
/// redraws only when something changed, so this is how a shell's output gets drawn.
static OUTPUT: AtomicBool = AtomicBool::new(false);

/// Whether any shell printed something since the last call.
pub fn take_output() -> bool {
    OUTPUT.swap(false, Ordering::Relaxed)
}

pub struct TermPane {
    parser: Arc<RwLock<vt100::Parser>>,
    tx: Sender<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    size: (u16, u16),
    exited: Arc<AtomicBool>,
    label: String,
    /// The shell's process id, once it has started; 0 until then.
    pid: Arc<AtomicU32>,
    /// What the shell is doing, re-read at most twice a second rather than on every
    /// frame: a few /proc reads per tab per frame add up across tabs and splits.
    activity: RefCell<(Option<Instant>, Option<String>)>,
    /// Started for one program ("Run it?" → in a terminal tab), not a shell: the pane
    /// stays when it ends, so its output can still be read.
    keep: bool,
    /// How the process ended, once it has: "exit 0", "exit 2".
    ended: Arc<std::sync::Mutex<Option<String>>>,
}

/// The cursor-position query a program sends to its terminal (DSR 6).
const CPR_QUERY: &[u8] = b"\x1b[6n";

/// How many cursor-position queries `chunk` holds, counting one split across the end
/// of the previous chunk, whose last bytes `tail` keeps between calls.
fn cursor_queries(tail: &mut Vec<u8>, chunk: &[u8]) -> usize {
    let mut joined = std::mem::take(tail);
    joined.extend_from_slice(chunk);
    let n = joined
        .windows(CPR_QUERY.len())
        .filter(|w| *w == CPR_QUERY)
        .count();
    // Keep what could be the start of a query cut off by the read; never a whole one,
    // which was counted already.
    let keep = joined.len().min(CPR_QUERY.len() - 1);
    tail.extend_from_slice(&joined[joined.len() - keep..]);
    n
}

impl TermPane {
    /// A shell keeping `scrollback` lines of history (`[terminal] scrollback`).
    pub fn spawn(cwd: &Path, rows: u16, cols: u16, shell: &str, scrollback: usize) -> Result<Self> {
        // An explicit shell may carry arguments ("bash -l", "pwsh -NoLogo").
        let mut words = shell.split_whitespace();
        let cmd = match words.next() {
            Some(program) => {
                let mut c = CommandBuilder::new(program);
                c.args(words);
                c
            }
            None => CommandBuilder::new_default_prog(),
        };
        let label = shell
            .split_whitespace()
            .next()
            .map(str::to_string)
            .or_else(|| std::env::var("SHELL").ok())
            // Windows has no $SHELL; CommandBuilder uses %COMSPEC%, so name that.
            .or_else(|| {
                if cfg!(windows) {
                    std::env::var("COMSPEC").ok()
                } else {
                    None
                }
            })
            .and_then(|s| {
                Path::new(&s)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "shell".to_string());
        Self::start(cwd, rows, cols, cmd, label, false, scrollback)
    }

    /// One program, by its argv: nothing is split and no shell reads it, so a path with
    /// spaces or `&` is one argument. The pane stays after the program ends.
    pub fn spawn_argv(
        cwd: &Path,
        rows: u16,
        cols: u16,
        argv: &[std::ffi::OsString],
        scrollback: usize,
    ) -> Result<Self> {
        let Some((program, args)) = argv.split_first() else {
            anyhow::bail!("nothing to run");
        };
        if let Some(why) = batch_refusal(
            argv,
            cfg!(windows),
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("PATHEXT").as_deref(),
        ) {
            anyhow::bail!(why);
        }
        // A bare name is looked up here, in PATH's absolute directories only: the pty
        // library resolves a relative entry (`.`, or an empty one) against the tab's
        // directory, the one being browsed, which could run a program of that name
        // found there.
        #[cfg(unix)]
        let resolved = if Path::new(program).components().count() == 1 {
            Some(
                on_path(program, std::env::var_os("PATH").as_deref()).ok_or_else(|| {
                    anyhow::anyhow!("{}: not found in PATH", program.to_string_lossy())
                })?,
            )
        } else {
            None
        };
        #[cfg(not(unix))]
        let resolved: Option<std::path::PathBuf> = None;
        let mut cmd = CommandBuilder::new(
            resolved
                .as_deref()
                .map_or(program.as_os_str(), Path::as_os_str),
        );
        cmd.args(args);
        let label = Path::new(program).file_name().map_or_else(
            || "program".to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        Self::start(cwd, rows, cols, cmd, label, true, scrollback)
    }

    fn start(
        cwd: &Path,
        rows: u16,
        cols: u16,
        mut cmd: CommandBuilder,
        label: String,
        keep: bool,
        scrollback: usize,
    ) -> Result<Self> {
        let rows = rows.max(MIN_ROWS);
        let cols = cols.max(MIN_COLS);

        let pair = NativePtySystem::default().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        cmd.cwd(cwd);
        // vt100 understands xterm sequences; without this, programs probe for something
        // else and draw garbage into the pane.
        cmd.env("TERM", "xterm-256color");
        // Who this shell belongs to: xdevcmd started in it would run inside itself.
        cmd.env(crate::PANEL_VAR, std::process::id().to_string());

        let slave = pair.slave;
        let master = pair.master;
        let mut reader = master.try_clone_reader()?;
        let mut writer = master.take_writer()?;

        let exited = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicU32::new(0));
        let ended = Arc::new(std::sync::Mutex::new(None));
        {
            let exited = exited.clone();
            let pid = pid.clone();
            let ended = ended.clone();
            std::thread::spawn(move || {
                let how = match slave.spawn_command(cmd) {
                    Ok(mut child) => {
                        if let Some(p) = child.process_id() {
                            pid.store(p, Ordering::Relaxed);
                        }
                        match child.wait() {
                            Ok(st) => format!("exit {}", st.exit_code()),
                            Err(e) => format!("lost: {e}"),
                        }
                    }
                    Err(e) => format!("could not start: {e}"),
                };
                if let Ok(mut e) = ended.lock() {
                    *e = Some(how);
                }
                // Dropping the slave here is what lets the reader thread see EOF.
                drop(slave);
                exited.store(true, Ordering::Relaxed);
            });
        }

        let parser = Arc::new(RwLock::new(vt100::Parser::new(rows, cols, scrollback)));
        let (tx, rx) = channel::<Vec<u8>>();
        {
            let parser = parser.clone();
            let answers = tx.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut tail = Vec::new();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => {
                            OUTPUT.store(true, Ordering::Relaxed);
                            break;
                        }
                        Ok(n) => {
                            let asked = cursor_queries(&mut tail, &buf[..n]);
                            if let Ok(mut p) = parser.write() {
                                p.process(&buf[..n]);
                                OUTPUT.store(true, Ordering::Relaxed);
                                // A terminal answers "where is the cursor?". ConPTY asks
                                // it once at start-up (PSEUDOCONSOLE_INHERIT_CURSOR) and
                                // waits for the answer before the shell runs at all: on
                                // Windows, an unanswered query was a dead terminal.
                                for _ in 0..asked {
                                    let (row, col) = p.screen().cursor_position();
                                    let reply = format!("\x1b[{};{}R", row + 1, col + 1);
                                    let _ = answers.send(reply.into_bytes());
                                }
                            }
                        }
                    }
                }
            });
        }

        std::thread::spawn(move || {
            while let Ok(bytes) = rx.recv() {
                if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            parser,
            tx,
            master,
            size: (rows, cols),
            exited,
            label,
            pid,
            activity: RefCell::new((None, None)),
            keep,
            ended,
        })
    }

    /// Started for one program: kept after it ends (see `keep`).
    pub fn keeps(&self) -> bool {
        self.keep
    }

    /// How the process ended, once it has.
    pub fn ended(&self) -> Option<String> {
        self.ended.lock().ok().and_then(|e| e.clone())
    }

    /// The program running in the foreground, or the shell's directory when it is idle
    /// at a prompt — what tmux shows in its window names. Three tabs all called "bash"
    /// cannot be told apart. `None` where the system does not say (anything but Linux),
    /// and the caller falls back to the shell's name.
    pub fn activity(&self) -> Option<String> {
        let mut cache = self.activity.borrow_mut();
        let fresh = cache
            .0
            .is_some_and(|t| t.elapsed() < Duration::from_millis(500));
        if !fresh {
            let pid = self.pid.load(Ordering::Relaxed);
            *cache = (
                Some(Instant::now()),
                (pid != 0).then(|| activity_of(pid)).flatten(),
            );
        }
        cache.1.clone()
    }

    /// The shell's working directory, where the system says (Linux). Saved with the
    /// session so the next start opens the tab where it was.
    pub fn cwd(&self) -> Option<std::path::PathBuf> {
        let pid = self.pid.load(Ordering::Relaxed);
        if pid == 0 {
            return None;
        }
        #[cfg(target_os = "linux")]
        {
            std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// The shell's program name, as the tab showed it before anything ran.
    pub fn shell_name(&self) -> &str {
        &self.label
    }

    /// The program that owns the terminal when it is not the shell — an editor, a
    /// pager, a password prompt. Anything typed into the terminal goes to it, not to
    /// the shell. `None` at a prompt, and wherever the system cannot say.
    ///
    /// A tab started for one program (`keeps`) has no shell at all: its process *is*
    /// the program, so the shell-versus-foreground test would call it idle. It is
    /// always busy, with that program, and nothing is typed into it.
    pub fn foreground_program(&self) -> Option<String> {
        if self.keep {
            return Some(self.label.clone());
        }
        let pid = self.pid.load(Ordering::Relaxed);
        (pid != 0).then(|| foreground_of(pid)).flatten()
    }

    /// For the tab bar: what it is doing, else what it is.
    pub fn tab_label(&self) -> String {
        self.activity().unwrap_or_else(|| self.label.clone())
    }

    /// The wheel over this shell, `col` and `row` counted from 1 inside its border. As a
    /// terminal does it: to the program, as a mouse report, when it asked for the mouse
    /// (Claude Code, vim, htop); as arrow keys when a full-screen program did not (less,
    /// man); otherwise back through the history.
    pub fn wheel(&self, col: u16, row: u16, up: bool) {
        use vt100::{MouseProtocolEncoding as Enc, MouseProtocolMode as Mode};
        let Ok(p) = self.parser.read() else { return };
        let screen = p.screen();
        let (mode, enc) = (
            screen.mouse_protocol_mode(),
            screen.mouse_protocol_encoding(),
        );
        let (alt, app_cursor) = (screen.alternate_screen(), screen.application_cursor());
        drop(p);
        if mode != Mode::None {
            // Wheel up and down are buttons 4 and 5: 64 and 65 in the report.
            let button: u32 = if up { 64 } else { 65 };
            let bytes = match enc {
                Enc::Sgr => format!("\x1b[<{button};{col};{row}M").into_bytes(),
                Enc::Utf8 => {
                    let mut out = b"\x1b[M".to_vec();
                    for v in [button, u32::from(col), u32::from(row)] {
                        let Some(c) = char::from_u32(32 + v) else {
                            return;
                        };
                        let mut b = [0u8; 4];
                        out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                    }
                    out
                }
                Enc::Default => {
                    // One byte each: a position past 223 cannot be said.
                    if col > 223 || row > 223 {
                        return;
                    }
                    vec![
                        0x1b,
                        b'[',
                        b'M',
                        (32 + button) as u8,
                        (32 + col) as u8,
                        (32 + row) as u8,
                    ]
                }
            };
            self.send(bytes);
        } else if alt {
            let key: &[u8] = match (up, app_cursor) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            self.send(key.repeat(3));
        } else {
            self.scroll_by(if up { 3 } else { -3 });
        }
    }

    /// Look back through history; positive goes further back. vt100 clamps to what it
    /// actually kept.
    pub fn scroll_by(&self, delta: isize) {
        if let Ok(mut p) = self.parser.write() {
            let now = p.screen().scrollback() as isize;
            p.screen_mut().set_scrollback((now + delta).max(0) as usize);
        }
    }

    /// The whole history and screen as text, for the viewer (Shift+F12), and whether a
    /// full-screen program has the other screen up: vt100 keeps no history there, so
    /// the text is that screen only.
    pub fn history_text(&self) -> (String, bool) {
        self.parser
            .write()
            .map(|mut p| (history_text(&mut p), p.screen().alternate_screen()))
            .unwrap_or_default()
    }

    fn history(&self) -> usize {
        self.parser
            .write()
            .map(|mut p| history_of(&mut p))
            .unwrap_or(0)
    }

    fn scrolled(&self) -> usize {
        self.parser
            .read()
            .map(|p| p.screen().scrollback())
            .unwrap_or(0)
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(MIN_ROWS), cols.max(MIN_COLS));
        if (rows, cols) == self.size {
            return;
        }
        self.size = (rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut p) = self.parser.write() {
            // In vt100 0.16 the resize lives on the screen, not the parser.
            p.screen_mut().set_size(rows, cols);
        }
    }

    pub fn send(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(bytes);
    }

    /// Translate a key into what a terminal would have sent. Returns false for keys we
    /// deliberately keep for ourselves.
    pub fn send_key(&self, k: KeyEvent) -> bool {
        // Typing while scrolled back would type into a screen you cannot see.
        self.scroll_by(-(self.scrolled() as isize));
        let Some(bytes) = key_bytes(k) else {
            return false;
        };
        self.send(bytes);
        true
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, active: bool) {
        let back = self.scrolled();
        let state = if self.has_exited() {
            " (exited)".to_string()
        } else if back > 0 {
            format!("  \u{2191} {back} lines back")
        } else {
            String::new()
        };
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                match self.activity() {
                    Some(doing) => format!("\u{2302} {} \u{b7} {doing}{state}", self.label),
                    None => format!("\u{2302} {}{}", self.label, state),
                },
                theme::title_style(active),
            ),
            Span::raw(" "),
        ]);
        let block = Block::bordered()
            .border_style(theme::border_style(active))
            .style(Style::new().bg(theme::t().bg))
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width == 0 || inner.height == 0 {
            return;
        }
        // Only while looking back: at the live screen a thumb would always sit at the
        // bottom and say nothing.
        let history = if back > 0 { self.history() } else { 0 };
        let Ok(guard) = self.parser.read() else {
            return;
        };
        frame.render_widget(PseudoTerminal::new(guard.screen()), inner);
        drop(guard);
        // A program run in its own tab has ended: say so on its last row, and how to
        // close it, rather than leaving a screen that looks alive.
        if self.keep && self.has_exited() {
            let how = self.ended().unwrap_or_else(|| "ended".into());
            let note = format!(
                " {} ended ({how}) \u{2014} Enter or Esc closes ",
                self.label
            );
            let y = inner.y + inner.height - 1;
            frame.buffer_mut().set_string(
                inner.x,
                y,
                &note,
                Style::new()
                    .fg(theme::t().bg)
                    .bg(theme::t().mark)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );
        }
        if back > 0 {
            crate::scrollbar::draw(
                frame.buffer_mut(),
                crate::scrollbar::track(area, inner),
                history + inner.height as usize,
                history.saturating_sub(back),
                theme::border_style(active),
            );
        }
    }

    /// Inner size for a pane rectangle, accounting for the border.
    pub fn inner_size(area: Rect) -> (u16, u16) {
        (area.height.saturating_sub(2), area.width.saturating_sub(2))
    }
}

/// Linux keeps the answer in /proc: the terminal's foreground process group in the
/// shell's `stat`, and the shell's working directory as a symlink.
#[cfg(target_os = "linux")]
fn activity_of(pid: u32) -> Option<String> {
    if let Some(program) = foreground_of(pid) {
        return Some(program);
    }
    let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).ok()?;
    Some(short_dir(&cwd))
}

#[cfg(target_os = "linux")]
fn foreground_of(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name sits in parentheses and may itself contain spaces or ')', so
    // the fields are counted from the last ')'.
    let rest = stat.get(stat.rfind(')')? + 2..)?;
    let f: Vec<&str> = rest.split_whitespace().collect();
    let pgrp: i32 = f.get(2)?.parse().ok()?;
    let tpgid: i32 = f.get(5)?.parse().ok()?;
    if tpgid > 0 && tpgid != pgrp {
        // Something else owns the terminal: name it. A process group id is its
        // leader's pid, so this reads the leader's name.
        let comm = std::fs::read_to_string(format!("/proc/{tpgid}/comm")).ok()?;
        let comm = comm.trim();
        return Some(if comm.is_empty() { "a program" } else { comm }.to_string());
    }
    None
}

/// macOS has no /proc. `ps` knows the shell's process group and its terminal's
/// foreground group, which is all the busy check needs. The working directory would
/// need lsof on every refresh, so idle tabs keep the shell's name there.
///
/// Not run on this machine (Linux); the pty test that covers it is enabled for macOS
/// so the CI runner verifies it.
#[cfg(target_os = "macos")]
fn foreground_of(pid: u32) -> Option<String> {
    let ps = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("ps").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let ids = ps(&["-o", "pgid=,tpgid=", "-p", &pid.to_string()])?;
    let mut it = ids.split_whitespace();
    let pgrp: i32 = it.next()?.parse().ok()?;
    let tpgid: i32 = it.next()?.parse().ok()?;
    if tpgid > 0 && tpgid != pgrp {
        let comm = ps(&["-o", "comm=", "-p", &tpgid.to_string()]).unwrap_or_default();
        let name = comm.rsplit('/').next().unwrap_or("").trim();
        return Some(if name.is_empty() { "a program" } else { name }.to_string());
    }
    None
}

#[cfg(target_os = "macos")]
fn activity_of(pid: u32) -> Option<String> {
    foreground_of(pid)
}

/// Windows: the console's foreground process is not something std or a small crate
/// exposes. Tabs keep the shell's name, and Alt+C cannot tell a busy shell from an
/// idle one — it still refuses control characters and quotes per shell.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn activity_of(_pid: u32) -> Option<String> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn foreground_of(_pid: u32) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn short_dir(p: &Path) -> String {
    if std::env::var_os("HOME").is_some_and(|h| Path::new(&h) == p) {
        return "~".into();
    }
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

/// Everything a shell's screen holds, history first, as plain text: the lines the user
/// would read if they scrolled all the way up and then down again.
///
/// vt100 shows its history only a screen at a time (`set_scrollback`), so this walks
/// the offset from the top of the history down to 0 and takes each page's rows. Row
/// `i` of the page at offset `o` is row `history - o + i` of the whole, so each row is
/// taken once by that number: the last page overlaps the one before it unless the
/// history is a whole number of screens, and the overlap is where lines would be
/// doubled. A row that `row_wrapped` says went on is joined to the next without a line
/// break, so a long line comes back as the one line it was. Blank rows at the end
/// (below the prompt) are cut. The view's own offset is put back.
///
/// Pure over the parser; the caller holds the write lock, which pauses that shell's
/// output for as long as this runs (measured in Round 015's Do).
pub fn history_text(p: &mut vt100::Parser) -> String {
    let before = p.screen().scrollback();
    let history = history_of(p);
    let (rows, cols) = p.screen().size();
    let rows = usize::from(rows.max(1));
    let mut out = String::new();
    let mut next = 0usize; // the next row of the whole to take
    let mut offset = history;
    loop {
        p.screen_mut().set_scrollback(offset);
        let screen = p.screen();
        let first = history - offset;
        for (i, text) in screen.rows(0, cols).enumerate() {
            if first + i != next {
                continue;
            }
            next += 1;
            out.push_str(&text);
            let wrapped = u16::try_from(i).is_ok_and(|r| screen.row_wrapped(r));
            if !wrapped {
                out.push('\n');
            }
        }
        if offset == 0 {
            break;
        }
        offset = offset.saturating_sub(rows);
    }
    p.screen_mut().set_scrollback(before);
    let kept = out.trim_end_matches(['\n', ' ']).len();
    out.truncate(kept);
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// How many lines of history vt100 holds. It does not say, but it clamps a scrollback
/// offset to that length, so ask for everything and put the offset back.
fn history_of(p: &mut vt100::Parser) -> usize {
    let now = p.screen().scrollback();
    p.screen_mut().set_scrollback(usize::MAX);
    let all = p.screen().scrollback();
    p.screen_mut().set_scrollback(now);
    all
}

/// The bytes a terminal sends for `k`, as the program in the panel expects them; `None`
/// for a key with none. Ctrl+letter is its control byte (Ctrl+R is 0x12, readline's
/// history search); Ctrl+Space and Ctrl+@ are NUL (emacs' mark, zsh's
/// set-mark-command), which legacy terminals send and crossterm reports as Ctrl+Space;
/// Alt puts ESC first.
pub fn key_bytes(k: KeyEvent) -> Option<Vec<u8>> {
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if let KeyCode::F(n) = k.code {
        return function_key_bytes(n, k.modifiers);
    }
    let mut bytes: Vec<u8> = match k.code {
        KeyCode::Char(' ' | '@') if ctrl => vec![0],
        KeyCode::Char(c) if ctrl => {
            let up = c.to_ascii_uppercase() as u8;
            if up.is_ascii_uppercase() || matches!(up, b'[' | b']' | b'\\' | b'^' | b'_') {
                vec![up & 0x1f]
            } else {
                let mut b = [0u8; 4];
                c.encode_utf8(&mut b).as_bytes().to_vec()
            }
        }
        KeyCode::Char(c) => {
            let mut b = [0u8; 4];
            c.encode_utf8(&mut b).as_bytes().to_vec()
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        _ => return None,
    };
    if alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

/// F1…F12 as xterm sends them, which is what `TERM=xterm-256color` promises the
/// program: F1–F4 as `ESC O P`…`S`, the rest as `ESC [ n ~`; with Shift, Alt or Ctrl
/// the modifier rides in a parameter (`ESC [ 1 ; 2 P` is Shift+F1, `ESC [ 17 ; 5 ~`
/// Ctrl+F6), never as a leading ESC. Before keymap A (Round 015) no F-key reached the
/// panel at all; mc and htop are driven by them. `None` past F12, which has no
/// xterm sequence of its own.
fn function_key_bytes(n: u8, mods: KeyModifiers) -> Option<Vec<u8>> {
    let mut m = 1;
    if mods.contains(KeyModifiers::SHIFT) {
        m += 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        m += 2;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        m += 4;
    }
    let ss3 = match n {
        1 => Some('P'),
        2 => Some('Q'),
        3 => Some('R'),
        4 => Some('S'),
        _ => None,
    };
    if let Some(c) = ss3 {
        return Some(
            if m == 1 {
                format!("\x1bO{c}")
            } else {
                format!("\x1b[1;{m}{c}")
            }
            .into_bytes(),
        );
    }
    let code = match n {
        5 => 15,
        6 => 17,
        7 => 18,
        8 => 19,
        9 => 20,
        10 => 21,
        11 => 23,
        12 => 24,
        _ => return None,
    };
    Some(
        if m == 1 {
            format!("\x1b[{code}~")
        } else {
            format!("\x1b[{code};{m}~")
        }
        .into_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_keys_reach_the_program_as_xterm_sends_them() {
        let k = |n, m| key_bytes(KeyEvent::new(KeyCode::F(n), m));
        let none = KeyModifiers::NONE;
        assert_eq!(k(1, none), Some(b"\x1bOP".to_vec()));
        assert_eq!(k(4, none), Some(b"\x1bOS".to_vec()));
        assert_eq!(k(5, none), Some(b"\x1b[15~".to_vec()));
        assert_eq!(k(10, none), Some(b"\x1b[21~".to_vec()));
        assert_eq!(k(11, none), Some(b"\x1b[23~".to_vec()));
        assert_eq!(k(1, KeyModifiers::SHIFT), Some(b"\x1b[1;2P".to_vec()));
        assert_eq!(k(6, KeyModifiers::SHIFT), Some(b"\x1b[17;2~".to_vec()));
        assert_eq!(k(6, KeyModifiers::CONTROL), Some(b"\x1b[17;5~".to_vec()));
        assert_eq!(k(9, KeyModifiers::ALT), Some(b"\x1b[20;3~".to_vec()));
        assert_eq!(k(13, none), None);
    }

    #[test]
    fn keys_reach_the_program_as_a_terminal_sends_them() {
        let k = |code, m| key_bytes(KeyEvent::new(code, m));
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(k(KeyCode::Char('r'), ctrl), Some(vec![0x12]));
        assert_eq!(k(KeyCode::Char('R'), ctrl), Some(vec![0x12]));
        assert_eq!(k(KeyCode::Tab, KeyModifiers::NONE), Some(vec![0x09]));
        assert_eq!(k(KeyCode::Char(' '), ctrl), Some(vec![0x00]));
        assert_eq!(k(KeyCode::Char('@'), ctrl), Some(vec![0x00]));
        assert_eq!(
            k(KeyCode::Char('x'), KeyModifiers::ALT),
            Some(b"\x1bx".to_vec())
        );
        assert_eq!(
            k(KeyCode::Char(' '), KeyModifiers::NONE),
            Some(b" ".to_vec())
        );
        assert_eq!(k(KeyCode::Null, KeyModifiers::NONE), None);
    }

    fn fed(rows: u16, cols: u16, keep: usize, bytes: &[u8]) -> vt100::Parser {
        let mut p = vt100::Parser::new(rows, cols, keep);
        p.process(bytes);
        p
    }

    #[test]
    fn history_text_gives_every_line_once_in_order() {
        let body: String = (1..=300).map(|i| format!("{i}\r\n")).collect();
        let mut p = fed(10, 80, 1000, body.as_bytes());
        p.screen_mut().set_scrollback(17);
        let text = history_text(&mut p);
        let lines: Vec<&str> = text.lines().collect();
        let want: Vec<String> = (1..=300).map(|i| i.to_string()).collect();
        assert_eq!(lines, want, "none missing or doubled where the pages meet");
        assert_eq!(p.screen().scrollback(), 17, "the view stays where it was");
    }

    #[test]
    fn history_text_at_every_page_alignment() {
        // The last page lines up with the one before only when the history is a whole
        // number of screens; every remainder is where a join could double or drop.
        for n in 1..=45 {
            let body: String = (1..=n).map(|i| format!("L{i}\r\n")).collect();
            let mut p = fed(7, 20, 1000, body.as_bytes());
            let text = history_text(&mut p);
            let want: String = (1..=n).map(|i| format!("L{i}\n")).collect();
            assert_eq!(text, want, "{n} lines");
        }
    }

    #[test]
    fn a_wrapped_line_comes_back_whole() {
        let long: String = (0..250)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let body = format!("before\r\n{long}\r\nafter\r\n");
        let mut p = fed(10, 80, 100, body.as_bytes());
        let text = history_text(&mut p);
        assert_eq!(text, format!("before\n{long}\nafter\n"));
        // The same with the long line pushed into the history and cut by a page.
        let pad: String = (0..13).map(|i| format!("p{i}\r\n")).collect();
        let mut p = fed(4, 80, 100, format!("{pad}{body}").as_bytes());
        assert_eq!(
            history_text(&mut p),
            format!("{}before\n{long}\nafter\n", pad.replace("\r\n", "\n"))
        );
    }

    #[test]
    fn history_text_of_an_empty_screen_is_empty() {
        let mut p = fed(5, 20, 100, b"");
        assert_eq!(history_text(&mut p), "");
        let mut p = fed(5, 20, 100, b"$ ");
        assert_eq!(history_text(&mut p), "$\n", "trailing blanks cut");
    }

    /// Not a check: how long the walk holds the write lock on the largest history the
    /// setting allows, 9999 lines of 200 columns. `cargo test --release -- --ignored
    /// --nocapture history_text_timing`.
    #[test]
    #[ignore]
    fn history_text_timing() {
        let line: String = "x".repeat(194); // 199 columns with the number: one row each
        let body: String = (0..10_050).map(|i| format!("{i:05}{line}\r\n")).collect();
        let mut p = fed(50, 200, 9999, body.as_bytes());
        let t = std::time::Instant::now();
        let text = history_text(&mut p);
        let took = t.elapsed();
        eprintln!(
            "history_text: {} lines, {} bytes, {took:?}",
            text.lines().count(),
            text.len()
        );
    }

    #[test]
    fn history_is_measured_without_moving_the_view() {
        let mut p = vt100::Parser::new(5, 20, 100);
        assert_eq!(history_of(&mut p), 0);
        for i in 0..30 {
            p.process(format!("line {i}\r\n").as_bytes());
        }
        // 30 lines and the empty one the cursor sits on, 5 of them on screen.
        assert_eq!(history_of(&mut p), 26);
        p.screen_mut().set_scrollback(7);
        assert_eq!(history_of(&mut p), 26);
        assert_eq!(p.screen().scrollback(), 7, "the view stays where it was");
    }

    #[test]
    fn cursor_queries_are_counted_even_when_a_read_splits_one() {
        let mut tail = Vec::new();
        assert_eq!(cursor_queries(&mut tail, b"hello\x1b[6nworld"), 1);
        assert_eq!(cursor_queries(&mut tail, b"no query here"), 0);
        // ConPTY's start-up query arriving in two reads.
        assert_eq!(cursor_queries(&mut tail, b"prompt\x1b["), 0);
        assert_eq!(cursor_queries(&mut tail, b"6n"), 1);
        // Two in one read, and one ending exactly at the end is not counted again.
        assert_eq!(cursor_queries(&mut tail, b"\x1b[6n\x1b[6n"), 2);
        assert_eq!(cursor_queries(&mut tail, b"x"), 0);
        // Other sequences that look alike are not queries.
        assert_eq!(cursor_queries(&mut tail, b"\x1b[5n\x1b[6m\x1b[16n"), 0);
    }
}

/// What `cmd.exe` acts on in a command line: command separators, redirection, its
/// escape, variable expansion (`%`, and `!` where delayed expansion is on), grouping,
/// quotes, line ends, and the characters it splits arguments at besides spaces (`,`
/// `;` `=`), which the pty library does not quote.
const CMD_SPECIAL: &[char] = &[
    '&', '|', '<', '>', '^', '%', '!', '"', '(', ')', ',', ';', '=', '\r', '\n',
];

fn is_batch(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("bat") || e.eq_ignore_ascii_case("cmd"))
}

/// The first executable file named `program` in an absolute directory of `path`.
/// Empty and relative entries are skipped: they mean the current directory, which in
/// a terminal tab is the directory being browsed.
#[cfg(unix)]
fn on_path(
    program: &std::ffi::OsStr,
    path: Option<&std::ffi::OsStr>,
) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|c| {
            std::fs::metadata(c).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The batch file `program` will start, if it is one, found in the order portable-pty
/// finds it on Windows: in each `PATH` directory, the name as given and then with each
/// `PATHEXT` extension in its place; the name itself only when no directory has it. So
/// a rule's `npm` is `npm.cmd`, and its `npm.cmd` is `C:\Program Files\nodejs\npm.cmd`.
fn resolves_to_batch(
    program: &std::ffi::OsStr,
    path: Option<&std::ffi::OsStr>,
    pathext: Option<&std::ffi::OsStr>,
) -> Option<std::path::PathBuf> {
    let program = Path::new(program);
    let exts: Vec<String> = pathext
        .map(|e| {
            std::env::split_paths(e)
                .filter_map(|x| x.to_str().map(|x| x.trim_start_matches('.').to_string()))
                .collect()
        })
        .unwrap_or_else(|| vec!["EXE".into()]);
    let found = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .find_map(|dir| {
            let exact = dir.join(program);
            if exact.exists() {
                return Some(exact);
            }
            exts.iter()
                .map(|ext| dir.join(program).with_extension(ext))
                .find(|c| c.exists())
        });
    let run = found.unwrap_or_else(|| program.to_path_buf());
    is_batch(&run).then_some(run)
}

/// Why `argv` may not run in a terminal tab, if it may not. On Windows a batch file is
/// run by `cmd.exe`, which parses its whole command line again; the pty library
/// quotes an argument only when it holds a space, and nothing quotes `%`. A file
/// named `a&calc.txt` handed to a `.cmd` would start `calc`. Rather than escape for
/// `cmd.exe` — which even the standard library gets right only with care — such a
/// command is refused, and the user can run it from the shell themselves.
pub fn batch_refusal(
    argv: &[std::ffi::OsString],
    windows: bool,
    path: Option<&std::ffi::OsStr>,
    pathext: Option<&std::ffi::OsStr>,
) -> Option<String> {
    let program = argv.first()?;
    if !windows {
        return None;
    }
    let batch = resolves_to_batch(program, path, pathext)?;
    // Windows runs a batch file as `cmd /c <the command line>`, and when that line
    // starts with a quote — the program's path held a space, so it was quoted — cmd
    // may strip the first and the last quote and run `C:\Program`.
    if batch.to_string_lossy().chars().any(char::is_whitespace) {
        return Some(format!(
            "{} is a batch file in a folder whose name has a space, which cmd.exe can \
             misread \u{2014} run it from the shell yourself",
            batch.display()
        ));
    }
    argv.iter().find_map(|a| {
        let a = a.to_string_lossy();
        let c = a.chars().find(|c| CMD_SPECIAL.contains(c))?;
        let shown = match c {
            '\r' | '\n' => "a line break".to_string(),
            c => format!("\u{201c}{c}\u{201d}"),
        };
        Some(format!(
            "{} is a batch file, and {a} holds {shown}, which cmd.exe would act on \u{2014} \
             run it from the shell yourself",
            Path::new(program).display()
        ))
    })
}

#[cfg(test)]
mod batch_tests {
    #[cfg(unix)]
    #[test]
    fn a_program_is_never_found_through_a_relative_path_entry() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("xdevcmd-onpath-{}", std::process::id()));
        let (here, bin) = (root.join("browsed"), root.join("bin"));
        for d in [&here, &bin] {
            std::fs::create_dir_all(d).unwrap();
            let p = d.join("tool");
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let name = std::ffi::OsStr::new("tool");
        // `.`, an empty entry and a relative one come first; only bin/ counts.
        let path = std::env::join_paths([
            std::path::PathBuf::from("."),
            std::path::PathBuf::new(),
            std::path::PathBuf::from("browsed"),
            bin.clone(),
        ])
        .unwrap();
        assert_eq!(on_path(name, Some(&path)), Some(bin.join("tool")));
        // With only relative entries there is nothing to run.
        let rel = std::env::join_paths([".", "browsed"]).unwrap();
        assert_eq!(on_path(name, Some(&rel)), None);
        // A file that cannot be run is passed over.
        std::fs::set_permissions(bin.join("tool"), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(on_path(name, Some(&path)), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    use super::*;
    use std::ffi::OsString;

    fn argv(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_batch_file_with_a_name_cmd_would_act_on_is_refused() {
        let run = argv(&["build.cmd", "a&calc.txt"]);
        let why = batch_refusal(&run, true, None, None).expect("refused");
        assert!(
            why.contains("a&calc.txt") && why.contains("\u{201c}&\u{201d}"),
            "{why}"
        );
        for name in [
            "50%.txt",
            "a,b",
            "a;b",
            "a=b",
            "x|y",
            "a^b",
            "q\"uote",
            "(1)",
            "line\nbreak",
            "bang!",
        ] {
            assert!(
                batch_refusal(&argv(&["x.BAT", name]), true, None, None).is_some(),
                "{name}"
            );
        }
        // An ordinary name, spaces included, runs; so does anything not a batch file.
        assert_eq!(
            batch_refusal(&argv(&["x.bat", "My Report.txt"]), true, None, None),
            None
        );
        assert_eq!(
            batch_refusal(&argv(&["tool.exe", "a&calc.txt"]), true, None, None),
            None
        );
        // A batch file in a folder with a space: its quoted path is what cmd misreads.
        let spaced = argv(&["C:\\Program Files\\x\\npm.cmd", "a.txt"]);
        let why = batch_refusal(&spaced, true, None, None);
        assert!(why.is_some_and(|w| w.contains("space")));
        // A bare batch name is looked for in PATH first, as the pty library does.
        let dir = std::env::temp_dir().join(format!("xdevcmd-batch-sp ace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("npm.cmd"), "").unwrap();
        let path = std::env::join_paths([&dir]).unwrap();
        let why = batch_refusal(
            &argv(&["npm.cmd", "a.txt"]),
            true,
            Some(path.as_os_str()),
            None,
        );
        assert!(
            why.is_some_and(|w| w.contains("space")),
            "found in a folder with a space"
        );
        let _ = std::fs::remove_dir_all(&dir);
        // Only Windows runs batch files through cmd.exe.
        assert_eq!(batch_refusal(&run, false, None, None), None);
    }

    #[test]
    fn a_bare_name_that_path_turns_into_a_batch_file_counts() {
        let dir = std::env::temp_dir().join(format!("xdevcmd-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("npm.cmd"), "").unwrap();
        std::fs::write(dir.join("tool.exe"), "").unwrap();
        let path = std::env::join_paths([&dir]).unwrap();
        // Lower case: Windows writes PATHEXT in capitals but its file names ignore
        // case, which the file system this test runs on may not.
        let pathext = std::env::join_paths([".com", ".exe", ".bat", ".cmd"]).unwrap();
        let (p, e) = (Some(path.as_os_str()), Some(pathext.as_os_str()));
        assert!(
            batch_refusal(&argv(&["npm", "a&b"]), true, p, e).is_some(),
            "npm is npm.cmd"
        );
        assert_eq!(batch_refusal(&argv(&["tool", "a&b"]), true, p, e), None);
        assert_eq!(batch_refusal(&argv(&["missing", "a&b"]), true, p, e), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
