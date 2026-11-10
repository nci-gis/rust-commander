//! Windows: xdevcmd started from a terminal panel asks before starting, under a real
//! ConPTY. The pty suite (tests/pty.rs) is Unix only; this is the Windows side of its
//! `xdevcmd_will_not_start_inside_its_own_terminal_panel`.
//!
//! The test process plays the panel's xdevcmd: its pid is the `XDEVCMD_PANEL` a panel's
//! shell would hand down. Windows cannot tell that panel from a new window opened from
//! it, so xdevcmd asks; Enter is no.
#![cfg(windows)]

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

const PATIENCE: Duration = Duration::from_secs(20);
const QUESTION: &str = "Start another xdevcmd here?";

struct Run {
    out: Arc<Mutex<Vec<u8>>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    _dir: std::path::PathBuf,
    answered: usize,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

impl Run {
    fn start(tag: &str) -> Run {
        let dir =
            std::env::temp_dir().join(format!("xdevcmd-winpanel-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 140,
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
        cmd.env("XDEVCMD_PANEL", std::process::id().to_string());
        let child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let out = Arc::new(Mutex::new(Vec::new()));
        let mut reader = pair.master.try_clone_reader().unwrap();
        let sink = out.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buf[..n]);
            }
        });
        let writer = pair.master.take_writer().unwrap();
        Run {
            out,
            writer,
            child,
            _master: pair.master,
            _dir: dir,
            answered: 0,
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    /// ConPTY asks where the cursor is and waits for the answer; a terminal answers.
    fn answer_cursor_queries(&mut self) {
        let asked = self.text().matches("\x1b[6n").count();
        while self.answered < asked {
            self.send(b"\x1b[1;1R");
            self.answered += 1;
        }
    }

    fn wait_for(&mut self, what: &str) {
        let start = Instant::now();
        while !self.text().contains(what) {
            self.answer_cursor_queries();
            if start.elapsed() > PATIENCE {
                let _ = self.child.kill();
                panic!("no {what:?} in:\n{:?}", self.text());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn exit_code(&mut self) -> u32 {
        let start = Instant::now();
        loop {
            self.answer_cursor_queries();
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.exit_code();
            }
            if start.elapsed() > PATIENCE {
                let _ = self.child.kill();
                panic!("xdevcmd did not end:\n{:?}", self.text());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[test]
fn started_from_a_panel_it_asks_and_enter_is_no() {
    let mut run = Run::start("no");
    run.wait_for(QUESTION);
    assert!(
        run.text()
            .contains(&format!("xdevcmd {}", std::process::id())),
        "the question names the panel's xdevcmd:\n{:?}",
        run.text()
    );
    run.send(b"\r");
    assert_eq!(run.exit_code(), 1, "{:?}", run.text());
}

#[test]
fn started_from_a_panel_yes_starts_it() {
    let mut run = Run::start("yes");
    run.wait_for(QUESTION);
    run.send(b"y\r");
    // The menu bar, drawn: it started. q quits, as in tests/pty.rs.
    run.wait_for("Help");
    run.send(b"q");
    assert_eq!(run.exit_code(), 0, "{:?}", run.text());
}
