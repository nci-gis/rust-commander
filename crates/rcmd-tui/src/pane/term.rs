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
}

impl TermPane {
    pub fn spawn(cwd: &Path, rows: u16, cols: u16, shell: &str) -> Result<Self> {
        let rows = rows.max(MIN_ROWS);
        let cols = cols.max(MIN_COLS);

        let pair = NativePtySystem::default().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        // An explicit shell may carry arguments ("bash -l", "pwsh -NoLogo").
        let mut words = shell.split_whitespace();
        let mut cmd = match words.next() {
            Some(program) => {
                let mut c = CommandBuilder::new(program);
                c.args(words);
                c
            }
            None => CommandBuilder::new_default_prog(),
        };
        cmd.cwd(cwd);
        // vt100 understands xterm sequences; without this, programs probe for something
        // else and draw garbage into the pane.
        cmd.env("TERM", "xterm-256color");

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

        let slave = pair.slave;
        let master = pair.master;
        let mut reader = master.try_clone_reader()?;
        let mut writer = master.take_writer()?;

        let exited = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicU32::new(0));
        {
            let exited = exited.clone();
            let pid = pid.clone();
            std::thread::spawn(move || {
                if let Ok(mut child) = slave.spawn_command(cmd) {
                    if let Some(p) = child.process_id() {
                        pid.store(p, Ordering::Relaxed);
                    }
                    let _ = child.wait();
                }
                // Dropping the slave here is what lets the reader thread see EOF.
                drop(slave);
                exited.store(true, Ordering::Relaxed);
            });
        }

        let parser = Arc::new(RwLock::new(vt100::Parser::new(rows, cols, 5000)));
        {
            let parser = parser.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut p) = parser.write() {
                                p.process(&buf[..n]);
                            }
                        }
                    }
                }
            });
        }

        let (tx, rx) = channel::<Vec<u8>>();
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
        })
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
    pub fn foreground_program(&self) -> Option<String> {
        let pid = self.pid.load(Ordering::Relaxed);
        (pid != 0).then(|| foreground_of(pid)).flatten()
    }

    /// For the tab bar: what it is doing, else what it is.
    pub fn tab_label(&self) -> String {
        self.activity().unwrap_or_else(|| self.label.clone())
    }

    /// Look back through history; positive goes further back. vt100 clamps to what it
    /// actually kept.
    pub fn scroll_by(&self, delta: isize) {
        if let Ok(mut p) = self.parser.write() {
            let now = p.screen().scrollback() as isize;
            p.screen_mut().set_scrollback((now + delta).max(0) as usize);
        }
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
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);

        let mut bytes: Vec<u8> = match k.code {
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
            _ => return false,
        };
        if alt {
            bytes.insert(0, 0x1b);
        }
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
        let Ok(guard) = self.parser.read() else {
            return;
        };
        frame.render_widget(PseudoTerminal::new(guard.screen()), inner);
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
