//! xdevcmd under a real pseudo-terminal on every system: ConPTY on Windows, a pty on
//! Linux and macOS (portable-pty either way). Its output is kept as bytes and read
//! through vt100 as a screen; keys go in as a terminal sends them. Used by kill.rs (the
//! data-safety cases, Round 036) on all three systems, and by windows_panel.rs and
//! windows_e2e.rs. tests/pty.rs keeps its own, Unix-only harness for the rest.
//!
//! Each test file uses only part of this.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub const PATIENCE: Duration = Duration::from_secs(20);
pub const ROWS: u16 = 30;
pub const COLS: u16 = 120;
pub const DOWN: &[u8] = b"\x1b[B";
pub const INSERT: &[u8] = b"\x1b[2~";
pub const ENTER: &[u8] = b"\r";
pub const ESC: &[u8] = b"\x1b";
pub const F5: &[u8] = b"\x1b[15~";
pub const F6: &[u8] = b"\x1b[17~";
pub const F10: &[u8] = b"\x1b[21~";
pub const F12: &[u8] = b"\x1b[24~";

/// A scratch directory: panes `a` and `b`, the state (session, journal, debug log) and
/// the data home (the trash) of the xdevcmd runs in it. Removed when dropped, so a test
/// can start xdevcmd again on what the last run left.
pub struct Scratch {
    pub dir: PathBuf,
}

impl Scratch {
    pub fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xdevcmd-e2e-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        Scratch { dir }
    }
    pub fn a(&self) -> PathBuf {
        self.dir.join("a")
    }
    pub fn b(&self) -> PathBuf {
        self.dir.join("b")
    }
    /// Where xdevcmd keeps its journals here (beside `XDEVCMD_STATE`).
    pub fn journal(&self) -> PathBuf {
        self.dir.join("journal")
    }
    /// Write `body` to `rel` under the scratch directory.
    pub fn file(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.dir.join(rel);
        std::fs::write(&p, body).unwrap();
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub struct Run {
    out: Arc<Mutex<Vec<u8>>>,
    screen: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    pub dir: PathBuf,
    answered: usize,
    /// A run made by `start` owns its directory; one in a [`Scratch`] does not.
    owns_dir: bool,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        if self.owns_dir {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

impl Run {
    /// xdevcmd on panes `a` and `b` of a scratch directory of its own, with `config` as
    /// its config.toml and, if given, `XDEVCMD_PANEL` set as a panel's shell sets it.
    pub fn start(tag: &str, config: &str, panel: Option<u32>) -> Run {
        let dir = std::env::temp_dir().join(format!("xdevcmd-win-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("config.toml"), config).unwrap();
        let panel = panel.map(|p| p.to_string());
        let env: Vec<(&str, &str)> = panel
            .iter()
            .map(|p| ("XDEVCMD_PANEL", p.as_str()))
            .collect();
        let mut run = Run::spawn(&dir, &[dir.join("a"), dir.join("b")], &env);
        run.owns_dir = true;
        run
    }

    /// xdevcmd in `s`, on `panes`, with `config` (TOML, may be empty) and `env` on top.
    /// Starting again in the same scratch directory finds what the last run left.
    /// Returns once the key bar is drawn: keys sent before are lost.
    pub fn start_in(s: &Scratch, panes: &[PathBuf], config: &str, env: &[(&str, &str)]) -> Run {
        std::fs::write(s.dir.join("config.toml"), config).unwrap();
        let mut run = Run::spawn(&s.dir, panes, env);
        run.wait_for("Quit");
        run
    }

    fn spawn(dir: &Path, panes: &[PathBuf], env: &[(&str, &str)]) -> Run {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_xdevcmd"));
        for p in panes {
            cmd.arg(p);
        }
        cmd.cwd(dir);
        cmd.env("XDEVCMD_CONFIG", dir.join("config.toml"));
        cmd.env("XDG_DATA_HOME", dir.join("data"));
        cmd.env("XDEVCMD_STATE", dir.join("session.toml"));
        // The plain look the tests read.
        cmd.env("XDEVCMD_THEME", "dark");
        cmd.env("TERM", "xterm-256color");
        // Run from xdevcmd's own panel, inside tmux or screen, or over SSH, xdevcmd would
        // ask or behave otherwise at start-up.
        for k in [
            "XDEVCMD_PANEL",
            "TMUX",
            "STY",
            "SSH_CONNECTION",
            "SSH_TTY",
            "VISUAL",
        ] {
            cmd.env_remove(k);
        }
        for (k, v) in env {
            cmd.env(k, v);
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
            dir: dir.to_path_buf(),
            answered: 0,
            owns_dir: false,
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
        self.answer_cursor_queries();
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
        std::thread::sleep(Duration::from_millis(60));
        // A lone Esc is only Esc if xdevcmd reads it before the next byte.
        if bytes == ESC {
            std::thread::sleep(Duration::from_millis(250));
        }
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

    /// Until `done` holds for the screen, answering the terminal's questions meanwhile.
    pub fn until(&mut self, what: &str, done: impl Fn(&str) -> bool) {
        let start = Instant::now();
        loop {
            self.answer_cursor_queries();
            let s = self.screen();
            if done(&s) {
                return;
            }
            if start.elapsed() > PATIENCE {
                let _ = self.child.kill();
                panic!("no {what}. Screen:\n{s}\nOutput:\n{:?}", self.text());
            }
            std::thread::sleep(Duration::from_millis(50));
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

    /// Until `what` is on the status line (the last two rows), not anywhere.
    pub fn wait_for_status(&mut self, what: &str) {
        self.until(&format!("{what:?} on the status line"), |s| {
            s.lines().rev().take(2).any(|l| l.contains(what))
        });
    }

    /// Until `what` is no longer on the screen.
    pub fn wait_gone(&mut self, what: &str) {
        self.until(&format!("{what:?} to go away"), |s| !s.contains(what));
    }

    /// End xdevcmd as a power cut would: nothing of it runs after. `kill -9` on Unix
    /// (portable-pty's own kill sends SIGHUP, which xdevcmd catches and tidies up
    /// after); TerminateProcess on Windows, as Task Manager's End task.
    pub fn kill_hard(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.child.process_id().expect("a pid");
            let killed = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status()
                .unwrap();
            assert!(killed.success());
        }
        #[cfg(not(unix))]
        self.child.kill().expect("TerminateProcess");
        let start = Instant::now();
        while !matches!(self.child.try_wait(), Ok(Some(_))) {
            assert!(start.elapsed() < PATIENCE, "xdevcmd did not die");
            std::thread::sleep(Duration::from_millis(25));
        }
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

    /// F10, and a clean exit.
    pub fn quit(mut self) {
        self.send(F10);
        assert_eq!(self.exit_code(), 0, "{}", self.screen());
    }
}

/// Until `path` holds what `ok` wants (`None`: not there).
pub fn wait_file(path: &Path, what: &str, ok: impl Fn(Option<String>) -> bool) {
    let start = Instant::now();
    while !ok(std::fs::read_to_string(path).ok()) {
        if start.elapsed() > PATIENCE {
            panic!("timed out waiting for {}: {what}", path.display());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A path as the journal writes it: hex of its bytes (Unix) or UTF-16 units (Windows).
pub fn hex(p: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        p.as_os_str()
            .encode_wide()
            .map(|u| format!("{u:04x}"))
            .collect()
    }
}

/// The journal's line naming this machine.
pub fn host_line() -> String {
    format!("host {}\n", hex(Path::new(&xdevcmd_core::os::hostname())))
}

/// Set `p`'s modification time `secs` seconds back.
pub fn age(p: &Path, secs: u64) {
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

/// A directory on another device than the scratch directory, for a move across
/// devices: `XDEVCMD_TEST_OTHER_DEV` (CI names one), else Linux's /dev/shm, Windows'
/// D: (GitHub's runners have one). `None`, and said, where there is none.
pub fn other_device(tag: &str) -> Option<PathBuf> {
    let tmp = std::env::temp_dir();
    let mut bases: Vec<PathBuf> = std::env::var_os("XDEVCMD_TEST_OTHER_DEV")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if cfg!(target_os = "linux") {
        bases.push("/dev/shm".into());
    }
    if cfg!(windows) {
        bases.push(r"D:\".into());
    }
    for b in bases {
        if std::fs::metadata(&b).is_ok_and(|m| m.is_dir()) && !same_device(&b, &tmp) {
            let d = b.join(format!("xdevcmd-e2e-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            if std::fs::create_dir_all(&d).is_ok() {
                return Some(d);
            }
        }
    }
    // CI sets this: a test that skipped there would pass without having run (the
    // red-team of Round 036: a skip said on stderr is not seen in a passing run).
    if std::env::var_os("XDEVCMD_REQUIRE_OTHER_DEV").is_some() {
        panic!("{tag}: no second device, and XDEVCMD_REQUIRE_OTHER_DEV is set");
    }
    eprintln!("{tag}: skipped, no second device (set XDEVCMD_TEST_OTHER_DEV)");
    None
}

fn same_device(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let dev = |p: &Path| std::fs::metadata(p).map(|m| m.dev()).ok();
        dev(a) == dev(b)
    }
    #[cfg(windows)]
    {
        let drive = |p: &Path| {
            let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            p.components()
                .next()
                .map(|c| c.as_os_str().to_ascii_uppercase())
        };
        drive(a) == drive(b)
    }
}
