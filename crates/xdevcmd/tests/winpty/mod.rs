//! A real ConPTY for the Windows tests (windows_panel.rs, windows_e2e.rs): xdevcmd.exe
//! under portable-pty, its output kept as bytes and read through vt100 as a screen, keys
//! sent as a terminal sends them. tests/pty.rs is the Unix counterpart.
//!
//! Each test file uses only part of this.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub const PATIENCE: Duration = Duration::from_secs(20);
pub const ROWS: u16 = 30;
pub const COLS: u16 = 120;
pub const F10: &[u8] = b"\x1b[21~";
pub const F12: &[u8] = b"\x1b[24~";

pub struct Run {
    out: Arc<Mutex<Vec<u8>>>,
    screen: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    pub dir: PathBuf,
    answered: usize,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Run {
    /// xdevcmd on panes `a` and `b` of a scratch directory, with `config` as its
    /// config.toml and, if given, `XDEVCMD_PANEL` set as a panel's shell sets it.
    pub fn start(tag: &str, config: &str, panel: Option<u32>) -> Run {
        let dir = std::env::temp_dir().join(format!("xdevcmd-win-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("config.toml"), config).unwrap();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_xdevcmd"));
        cmd.arg(dir.join("a"));
        cmd.arg(dir.join("b"));
        cmd.cwd(&dir);
        cmd.env("XDEVCMD_CONFIG", dir.join("config.toml"));
        cmd.env("XDG_DATA_HOME", dir.join("data"));
        cmd.env("XDEVCMD_STATE", dir.join("session.toml"));
        // The plain look the tests read.
        cmd.env("XDEVCMD_THEME", "dark");
        match panel {
            Some(pid) => cmd.env("XDEVCMD_PANEL", pid.to_string()),
            None => cmd.env_remove("XDEVCMD_PANEL"),
        }
        let child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let out = Arc::new(Mutex::new(Vec::new()));
        let screen = Arc::new(Mutex::new(vt100::Parser::new(
            ROWS.try_into().unwrap(),
            COLS.try_into().unwrap(),
            0,
        )));
        let mut reader = pair.master.try_clone_reader().unwrap();
        let (sink, parser) = (out.clone(), screen.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buf[..n]);
                parser.lock().unwrap().process(&buf[..n]);
            }
        });
        let writer = pair.master.take_writer().unwrap();
        Run {
            out,
            screen,
            writer,
            child,
            _master: pair.master,
            dir,
            answered: 0,
        }
    }

    /// Everything xdevcmd wrote, as text (for what it prints before the screen is drawn).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    /// The screen now, as vt100 reads it.
    pub fn screen(&self) -> String {
        self.screen.lock().unwrap().screen().contents()
    }

    /// Bytes as a terminal sends them. Writing to a pty whose program has ended may
    /// fail; that is for `exit_code` to tell.
    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
        std::thread::sleep(Duration::from_millis(60));
    }

    /// ConPTY asks where the cursor is and waits for the answer; a terminal answers.
    fn answer_cursor_queries(&mut self) {
        let asked = self.text().matches("\x1b[6n").count();
        while self.answered < asked {
            let _ = self.writer.write_all(b"\x1b[1;1R");
            let _ = self.writer.flush();
            self.answered += 1;
        }
    }

    fn wait(&mut self, what: &str, look: impl Fn(&Run) -> String) {
        let start = Instant::now();
        while !look(self).contains(what) {
            self.answer_cursor_queries();
            if start.elapsed() > PATIENCE {
                let _ = self.child.kill();
                panic!(
                    "no {what:?}. Screen:\n{}\nOutput:\n{:?}",
                    self.screen(),
                    self.text()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Until `what` is in the output, drawn or not.
    pub fn wait_for_text(&mut self, what: &str) {
        self.wait(what, Run::text);
    }

    /// Until `what` is on the screen.
    pub fn wait_for(&mut self, what: &str) {
        self.wait(what, Run::screen);
    }

    pub fn exit_code(&mut self) -> u32 {
        let start = Instant::now();
        loop {
            self.answer_cursor_queries();
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.exit_code();
            }
            if start.elapsed() > PATIENCE {
                let _ = self.child.kill();
                panic!("xdevcmd did not end. Screen:\n{}", self.screen());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
