//! The real binary in a real pty: keys and mouse go in as the bytes a terminal sends,
//! the screen comes back through vt100, and the assertions look at the disk.
//!
//! This is the layer that found the bugs `--screenshot` could not: marks surviving an
//! operation and widening the next delete, and the return from `$EDITOR` dying on a
//! cursor-position query no terminal answered in time.
//!
//! Unix only for now. On Windows the same bytes go through ConPTY, which translates
//! input into console events; whether SGR mouse sequences survive that is unverified,
//! and a test that might be measuring ConPTY rather than xdevcmd is worse than none. The
//! `--screenshot` tests in render.rs cover the same behaviour on every platform.
#![cfg(unix)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};

const ROWS: u16 = 32;
const COLS: u16 = 110;
const PATIENCE: Duration = Duration::from_secs(8);

static N: AtomicU32 = AtomicU32::new(0);

/// A scratch directory per test, holding the panes' directories and everything xdevcmd
/// might otherwise read or write outside it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("xdevcmd-pty-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("left")).unwrap();
        std::fs::create_dir_all(root.join("right")).unwrap();
        Self(root)
    }
    fn left(&self) -> PathBuf {
        self.0.join("left")
    }
    fn right(&self) -> PathBuf {
        self.0.join("right")
    }
    fn file(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::write(&p, body).unwrap();
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Session {
    parser: Arc<Mutex<vt100::Parser>>,
    /// Every byte xdevcmd wrote, for the few tests about the bytes themselves (colours);
    /// anything about text asks the screen instead.
    raw: Arc<Mutex<Vec<u8>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    /// Held for the session: dropping it (with the writer and the reader's copy) hangs
    /// up the child, which [`Session::hang_up`] does on purpose.
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    /// Tells the reader to let go of its copy of the master at its next read.
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Session {
    /// Start xdevcmd on the scratch panes, isolated from the person running the tests:
    /// no real config (their rebinds would break every key here), no real trash, and
    /// a plain `sh` in the shell pane rather than whatever their rc files do.
    fn start(s: &Scratch, editor: Option<&Path>) -> Self {
        Self::start_with(s, editor, &[s.left(), s.right()])
    }

    /// As `start`, with chosen directory arguments — none at all reopens the session.
    fn start_with(s: &Scratch, editor: Option<&Path>, dirs: &[PathBuf]) -> Self {
        Self::start_env(s, editor, dirs, &[])
    }

    /// As `start_with`, with environment variables set on top.
    fn start_env(
        s: &Scratch,
        editor: Option<&Path>,
        dirs: &[PathBuf],
        env: &[(&str, &str)],
    ) -> Self {
        Self::start_answering(s, editor, dirs, env, None)
    }

    /// As `start_env`, answering xdevcmd's start-up question about pictures (`CSI c`)
    /// with `answer`, as a terminal would, before the first frame.
    fn start_answering(
        s: &Scratch,
        editor: Option<&Path>,
        dirs: &[PathBuf],
        env: &[(&str, &str)],
        answer: Option<&'static [u8]>,
    ) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("a pty");

        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_xdevcmd"));
        for d in dirs {
            cmd.arg(d);
        }
        cmd.cwd(&s.0);
        cmd.env("TERM", "xterm-256color");
        cmd.env("XDEVCMD_CONFIG", s.0.join("no-such-config.toml"));
        cmd.env("XDG_DATA_HOME", s.0.join("data"));
        // q saves the session: into the scratch directory, never over the real one.
        cmd.env("XDEVCMD_STATE", s.0.join("session.toml"));
        cmd.env("SHELL", "/bin/sh");
        cmd.env("PS1", "$ ");
        // dash, run as root (a container, some CI), ignores an inherited PS1 and
        // prompts `# `; the tests wait for `$ `. An interactive sh reads `$ENV` after
        // setting its own prompt, so a file there has the last word.
        let rc = s.0.join("shrc");
        std::fs::write(&rc, "PS1='$ '\n").unwrap();
        cmd.env("ENV", &rc);
        cmd.env_remove("VISUAL");
        cmd.env_remove("SSH_CONNECTION");
        cmd.env_remove("SSH_TTY");
        // Run from xdevcmd's own terminal panel, the suite would be refused.
        cmd.env_remove("XDEVCMD_PANEL");
        cmd.env("DISPLAY", ":0");
        match editor {
            Some(e) => cmd.env("EDITOR", e),
            None => cmd.env_remove("EDITOR"),
        }
        for (k, v) in env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd).expect("xdevcmd starts");
        // Without dropping our copy of the slave, the reader never sees EOF.
        drop(pair.slave);

        let parser = Arc::new(Mutex::new(vt100::Parser::new(ROWS, COLS, 0)));
        let raw = Arc::new(Mutex::new(Vec::new()));
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().unwrap()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let parser = parser.clone();
            let raw = raw.clone();
            let writer = writer.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 16384];
                let mut answer = answer;
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 || stop.load(Ordering::Relaxed) {
                        break;
                    }
                    raw.lock().unwrap().extend_from_slice(&buf[..n]);
                    parser.lock().unwrap().process(&buf[..n]);
                    let asked = raw.lock().unwrap().windows(3).any(|w| w == b"\x1b[c");
                    if let Some(a) = answer.filter(|_| asked) {
                        let mut w = writer.lock().unwrap();
                        let _ = w.write_all(a);
                        let _ = w.flush();
                        answer = None;
                    }
                }
            });
        }
        let session = Self {
            parser,
            raw,
            writer,
            child,
            master: Some(pair.master),
            stop,
        };
        // The file panes are up once the key bar says Quit; --show-keys draws no panes,
        // only its own first line.
        let show_keys = dirs.iter().any(|d| d == Path::new("--show-keys"));
        session.wait_for(if show_keys {
            "xdevcmd --show-keys"
        } else {
            "Quit"
        });
        session
    }

    fn screen(&self) -> String {
        self.parser.lock().unwrap().screen().contents()
    }

    fn send(&mut self, bytes: &[u8]) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes).unwrap();
        w.flush().unwrap();
        drop(w);
        // A beat for the event loop to read it and repaint; the waits below do the
        // real synchronising.
        std::thread::sleep(Duration::from_millis(60));
        // A lone Esc is only Esc if xdevcmd reads it before the next byte: Esc then q in
        // one read is Alt+Q, as a terminal sends Alt+Q. On a loaded CI runner 60 ms was
        // not enough, and read_only_mode… pressed Alt+Q instead of quitting.
        if bytes == b"\x1b" {
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn until(&self, what: &str, done: impl Fn(&str) -> bool) {
        let start = Instant::now();
        loop {
            let s = self.screen();
            if done(&s) {
                return;
            }
            if start.elapsed() > PATIENCE {
                panic!("timed out waiting for {what}. Screen:\n{s}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_for(&self, text: &str) {
        self.until(&format!("{text:?} on screen"), |s| s.contains(text));
    }

    fn wait_gone(&self, text: &str) {
        self.until(&format!("{text:?} to go away"), |s| !s.contains(text));
    }

    /// Where `text` is drawn, as (column, row).
    fn locate(&self, text: &str) -> (u16, u16) {
        let s = self.screen();
        s.lines()
            .enumerate()
            .find_map(|(y, l)| {
                l.find(text)
                    .map(|b| (l[..b].chars().count() as u16, y as u16))
            })
            .unwrap_or_else(|| panic!("{text:?} not on screen:\n{s}"))
    }

    // SGR mouse, as xterm and every modern terminal send it: ESC [ < b ; col ; row M|m,
    // 1-based. b: 0 left, 2 right, +16 Ctrl, +32 "moving with a button held".
    fn mouse(&mut self, b: u8, x: u16, y: u16, release: bool) {
        let end = if release { 'm' } else { 'M' };
        let seq = format!("\x1b[<{b};{};{}{end}", x + 1, y + 1);
        let mut w = self.writer.lock().unwrap();
        w.write_all(seq.as_bytes()).unwrap();
        w.flush().unwrap();
        drop(w);
        std::thread::sleep(Duration::from_millis(30));
    }

    fn click(&mut self, x: u16, y: u16, b: u8) {
        self.mouse(b, x, y, false);
        self.mouse(b, x, y, true);
    }

    fn drag(&mut self, from: (u16, u16), to: (u16, u16)) {
        self.mouse(0, from.0, from.1, false);
        for s in 1..=8i32 {
            let x = from.0 as i32 + (to.0 as i32 - from.0 as i32) * s / 8;
            let y = from.1 as i32 + (to.1 as i32 - from.1 as i32) * s / 8;
            self.mouse(32, x as u16, y as u16, false);
        }
        self.mouse(0, to.0, to.1, true);
    }

    /// Close the terminal under xdevcmd, as closing its window or losing SSH does: every
    /// copy of the master goes, the reader's at its next read (xdevcmd draws often).
    fn hang_up(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        *self.writer.lock().unwrap() = Box::new(std::io::sink());
        self.master = None;
    }

    /// Press q and require a clean exit.
    fn quit(mut self) {
        self.send(b"q");
        let start = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    assert!(status.success(), "xdevcmd exited with {status:?}");
                    return;
                }
                Ok(None) if start.elapsed() > PATIENCE => {
                    let _ = self.child.kill();
                    panic!("xdevcmd did not exit on q. Screen:\n{}", self.screen());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(e) => panic!("waiting for xdevcmd: {e}"),
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
    }
}

fn wait_file(path: &Path, what: &str, ok: impl Fn(Option<String>) -> bool) {
    let start = Instant::now();
    while !ok(std::fs::read_to_string(path).ok()) {
        if start.elapsed() > PATIENCE {
            panic!("timed out waiting for {}: {what}", path.display());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

const DOWN: &[u8] = b"\x1b[B";
const INSERT: &[u8] = b"\x1b[2~";
const ENTER: &[u8] = b"\r";
const F4: &[u8] = b"\x1bOS";
const F5: &[u8] = b"\x1b[15~";
const F12: &[u8] = b"\x1b[24~";
const SHIFT_F8: &[u8] = b"\x1b[19;2~";

// ── the basics ─────────────────────────────────────────────────────────────────────

#[test]
fn starts_draws_both_panes_and_quits_cleanly() {
    let s = Scratch::new("basics");
    s.file("left/notes.txt", "x");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    let mut xdevcmd = Session::start(&s, None);

    let screen = xdevcmd.screen();
    for want in [
        "left", "right", "Name", "Size", "<DIR>", "notes", "10 Quit", "9 Menu",
    ] {
        assert!(screen.contains(want), "{want:?} missing:\n{screen}");
    }

    // The status line names the entry under the cursor, so moving it must change it.
    xdevcmd.send(DOWN);
    xdevcmd.wait_for("sub");
    xdevcmd.quit();
}

#[test]
fn the_shell_pane_runs_commands() {
    let s = Scratch::new("shell");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    // Arithmetic, so the answer cannot be mistaken for the typed command echoing back.
    xdevcmd.send(b"echo XDEVCMD_$((40+2))\r");
    xdevcmd.wait_for("XDEVCMD_42");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

// ── file operations ────────────────────────────────────────────────────────────────

#[test]
fn copying_over_a_file_asks_first_then_overwrites() {
    let s = Scratch::new("overwrite");
    s.file("left/report.txt", "NEW");
    let target = s.file("right/report.txt", "OLD");
    let mut xdevcmd = Session::start(&s, None);

    xdevcmd.send(DOWN);
    xdevcmd.send(INSERT);
    xdevcmd.send(F5);
    xdevcmd.wait_for("[Enter] go");
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("Already exists");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "OLD",
        "not before the answer"
    );

    xdevcmd.send(b"O");
    wait_file(&target, "overwritten", |c| c.as_deref() == Some("NEW"));
    assert!(
        s.left().join("report.txt").exists(),
        "a copy leaves the source"
    );

    // Marks clear once a job is done, or the next F8 would include this file too.
    xdevcmd.wait_gone("Already exists");
    xdevcmd.until("marks to clear", |scr| !scr.contains('\u{2022}'));
    xdevcmd.quit();
}

#[test]
fn a_file_made_by_another_program_shows_up_at_once() {
    let s = Scratch::new("watch");
    s.file("left/first.txt", "x");
    let xdevcmd = Session::start(&s, None);
    xdevcmd.wait_for("first ");
    // Made outside xdevcmd. The system's watcher reports it at once; polling, when
    // watching works, is ten times slower than the 1 s default, so two seconds tell
    // the two apart.
    let t = Instant::now();
    s.file("left/arrived.txt", "y");
    xdevcmd.wait_for("arrived ");
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "took {:?}",
        t.elapsed()
    );
    // Well after the watch began, so only the watcher can bring it this fast.
    std::thread::sleep(Duration::from_millis(500));
    let t = Instant::now();
    s.file("left/later.txt", "z");
    xdevcmd.wait_for("later ");
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "took {:?}",
        t.elapsed()
    );
    xdevcmd.quit();
}

/// Wait until xdevcmd has written `n` copies of `needle` in all, or fail.
fn wait_bytes(session: &Session, needle: &str, n: usize) {
    let start = Instant::now();
    loop {
        let got = String::from_utf8_lossy(&session.raw.lock().unwrap())
            .matches(needle)
            .count();
        if got >= n {
            return;
        }
        if start.elapsed() > PATIENCE {
            panic!(
                "{needle:?} written {got} times, not {n}. Screen:\n{}",
                session.screen()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn quick_view_draws_a_kitty_picture_once_and_removes_it_under_a_dialog() {
    let s = Scratch::new("kitty");
    let pic = xdevcmd_core::image::Picture {
        width: 8,
        height: 8,
        source: (8, 8),
        rgb: vec![[200, 30, 30]; 64],
    };
    std::fs::write(
        s.left().join("a.png"),
        xdevcmd_core::image::to_png(&pic).unwrap(),
    )
    .unwrap();
    let cfg = s.file("kitty.toml", "[viewer]\nimages = \"kitty\"\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", &cfg)],
    );
    x.wait_for("a ");
    x.send(DOWN);
    x.send(b"\x1bq"); // Alt+Q: quick view in the other pane
    wait_bytes(&x, "\x1b_Ga=T,f=100,", 1);
    // Drawn once: frames that change nothing do not send it again.
    std::thread::sleep(Duration::from_millis(1500));
    wait_bytes(&x, "\x1b_Ga=T,f=100,", 1);
    let sent = String::from_utf8_lossy(&x.raw.lock().unwrap())
        .matches("\x1b_Ga=T,")
        .count();
    assert_eq!(sent, 1, "sent again with nothing changed");
    // A dialog over it: the picture goes off the screen (it would be drawn over the
    // dialog), the terminal keeping it...
    x.send(b"\x1bOP"); // F1, help
    wait_bytes(&x, "\x1b_Ga=d,d=i,", 1);
    // ...and is placed again when the dialog closes, not sent again.
    x.send(b"\x1b");
    wait_bytes(&x, "\x1b_Ga=p,", 1);
    let raw = String::from_utf8_lossy(&x.raw.lock().unwrap()).into_owned();
    assert_eq!(raw.matches("\x1b_Ga=T,").count(), 1, "sent once");
    assert!(!raw.contains("\x1b_Ga=d,d=I,"), "never freed meanwhile");
    x.quit();
}

/// Ended while a dialog covers a Kitty picture (kept by the terminal, not placed): the
/// picture is freed on the way out, not left in the terminal's memory.
#[test]
fn a_kitty_picture_kept_under_a_dialog_is_freed_at_the_end() {
    let s = Scratch::new("kitty-end");
    let pic = xdevcmd_core::image::Picture {
        width: 8,
        height: 8,
        source: (8, 8),
        rgb: vec![[200, 30, 30]; 64],
    };
    std::fs::write(
        s.left().join("a.png"),
        xdevcmd_core::image::to_png(&pic).unwrap(),
    )
    .unwrap();
    let cfg = s.file("kitty.toml", "[viewer]\nimages = \"kitty\"\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", &cfg)],
    );
    x.wait_for("a ");
    x.send(DOWN);
    x.send(b"\x1bq"); // quick view
    wait_bytes(&x, "\x1b_Ga=T,f=100,", 1);
    x.send(b"\x1bOP"); // F1: help over it
    wait_bytes(&x, "\x1b_Ga=d,d=i,", 1);
    // Ended by a polite kill with the dialog still open.
    let pid = x.child.process_id().unwrap();
    std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    let start = Instant::now();
    while x.child.try_wait().unwrap().is_none() {
        assert!(start.elapsed() < PATIENCE, "still running after SIGTERM");
        std::thread::sleep(Duration::from_millis(25));
    }
    wait_bytes(&x, "\x1b_Ga=d,d=I,", 1);
}

#[test]
fn going_quickly_past_pictures_sends_only_the_one_stopped_on() {
    let s = Scratch::new("kitty-run");
    for (i, name) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
        let pic = xdevcmd_core::image::Picture {
            width: 8,
            height: 8,
            source: (8, 8),
            rgb: vec![[40 * i as u8, 30, 30]; 64],
        };
        std::fs::write(
            s.left().join(format!("{name}.png")),
            xdevcmd_core::image::to_png(&pic).unwrap(),
        )
        .unwrap();
    }
    let cfg = s.file("kitty.toml", "[viewer]\nimages = \"kitty\"\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", &cfg)],
    );
    x.wait_for("f ");
    x.send(DOWN);
    x.send(b"\x1bq"); // quick view on a.png
    wait_bytes(&x, "\x1b_Ga=T,f=100,", 1);
    // Five Downs a key-repeat apart (`send` waits 60 ms after each, under the 120 ms a
    // picture must be stayed on): past b, c, d, e to f. Without the wait each was
    // decoded and sent.
    for _ in 0..5 {
        x.send(DOWN);
    }
    x.wait_for("f.png");
    wait_bytes(&x, "\x1b_Ga=T,f=100,", 2);
    std::thread::sleep(Duration::from_millis(500));
    let raw = String::from_utf8_lossy(&x.raw.lock().unwrap()).into_owned();
    assert_eq!(
        raw.matches("\x1b_Ga=T,").count(),
        2,
        "a.png, then only f.png"
    );
    x.quit();
}

#[test]
fn quick_view_draws_iterm2_and_sixel_pictures_and_repaints_them_away() {
    for (name, start) in [("iterm2", "\x1b]1337;File=inline=1;"), ("sixel", "\x1bP")] {
        let s = Scratch::new(name);
        let pic = xdevcmd_core::image::Picture {
            width: 8,
            height: 8,
            source: (8, 8),
            rgb: vec![[30, 200, 30]; 64],
        };
        std::fs::write(
            s.left().join("a.png"),
            xdevcmd_core::image::to_png(&pic).unwrap(),
        )
        .unwrap();
        let cfg = s.file("p.toml", &format!("[viewer]\nimages = \"{name}\"\n"));
        let cfg = cfg.to_string_lossy().into_owned();
        // Sixel is drawn only where the terminal gives its cell size: this one does.
        let mut x = Session::start_answering(
            &s,
            None,
            &[s.left(), s.right()],
            &[("XDEVCMD_CONFIG", &cfg)],
            Some(b"\x1b[6;20;10t\x1b[?61;4;6;22c"),
        );
        x.wait_for("a ");
        x.send(DOWN);
        x.send(b"\x1bq");
        wait_bytes(&x, start, 1);
        // Under a dialog these pictures are cells: the screen is repainted over them, and
        // they are drawn again once it closes.
        x.send(b"\x1bOP");
        x.wait_for("Keys");
        x.send(b"\x1b");
        wait_bytes(&x, start, 2);
        x.quit();
    }
}

#[test]
fn a_terminal_that_answers_sixel_gets_sixel_with_nothing_in_the_environment() {
    // Windows Terminal over SSH: no WT_SESSION, but it answers the questions xdevcmd
    // asks at start-up — its cell size and, in its device attributes, sixel (4).
    let s = Scratch::new("probe");
    let pic = xdevcmd_core::image::Picture {
        width: 8,
        height: 8,
        source: (8, 8),
        rgb: vec![[30, 30, 200]; 64],
    };
    std::fs::write(
        s.left().join("a.png"),
        xdevcmd_core::image::to_png(&pic).unwrap(),
    )
    .unwrap();
    let mut x = Session::start_answering(
        &s,
        None,
        &[s.left(), s.right()],
        &[
            ("TERM_PROGRAM", ""),
            ("WT_SESSION", ""),
            ("KITTY_WINDOW_ID", ""),
            ("LC_TERMINAL", ""),
        ],
        Some(b"\x1b[6;20;10t\x1b[?61;4;6;22c"),
    );
    x.wait_for("a ");
    x.send(DOWN);
    x.send(b"\x1bq");
    wait_bytes(&x, "\x1bP9;1;0q", 1);
    x.quit();
}

#[test]
fn a_terminal_that_answers_sixel_but_not_its_cell_size_gets_half_blocks() {
    // Sixel drawn for a guessed cell runs over its box where the cells are smaller.
    let s = Scratch::new("probe-nocell");
    let pic = xdevcmd_core::image::Picture {
        width: 8,
        height: 8,
        source: (8, 8),
        rgb: vec![[30, 30, 200]; 64],
    };
    std::fs::write(
        s.left().join("a.png"),
        xdevcmd_core::image::to_png(&pic).unwrap(),
    )
    .unwrap();
    let mut x = Session::start_answering(
        &s,
        None,
        &[s.left(), s.right()],
        &[
            ("TERM_PROGRAM", ""),
            ("WT_SESSION", ""),
            ("KITTY_WINDOW_ID", ""),
            ("LC_TERMINAL", ""),
        ],
        Some(b"\x1b[?61;4;6;22c"),
    );
    x.wait_for("a ");
    x.send(DOWN);
    x.send(b"\x1bq");
    // Decoded and drawn (in half blocks: the title says so either way).
    x.wait_for("image 8");
    std::thread::sleep(Duration::from_millis(300));
    let raw = String::from_utf8_lossy(&x.raw.lock().unwrap()).into_owned();
    assert!(
        !raw.contains("\x1bP9;1;0q"),
        "no sixel without the cell size"
    );
    x.quit();
}

/// The terminal closed (SIGHUP): xdevcmd ends by itself and saves the session first,
/// with autosave off so only that save can have written it.
#[cfg(unix)]
#[test]
fn a_closed_terminal_saves_the_session_and_ends() {
    let s = Scratch::new("hangup");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    let cfg = s.file("hup.toml", "[general]\nautosave_secs = 0\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", &cfg)],
    );
    x.send(DOWN);
    x.send(ENTER); // into sub
    x.wait_for("/left/sub");
    let pid = x.child.process_id().expect("a pid").to_string();
    let killed = std::process::Command::new("kill")
        .args(["-HUP", &pid])
        .status()
        .unwrap();
    assert!(killed.success());
    let start = Instant::now();
    while x.child.try_wait().unwrap().is_none() {
        assert!(start.elapsed() < PATIENCE, "still running after SIGHUP");
        std::thread::sleep(Duration::from_millis(25));
    }
    let saved = std::fs::read_to_string(s.0.join("session.toml")).expect("the session was saved");
    assert!(saved.contains("sub"), "{saved}");
}

/// The terminal lost under a running copy, the read failing (EIO) before any signal is
/// seen: signals are swallowed here, so only the error can end the loop. The copy is
/// cancelled and cleans up, the session is saved, and xdevcmd ends.
#[cfg(unix)]
#[test]
fn a_lost_terminal_cancels_cleans_and_saves_even_before_the_signal() {
    let s = Scratch::new("eio");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    // Sparse: big enough to be still copying when the terminal goes, and nothing to
    // write here unless it is copied. The copy is not sparse, so this bounds what a
    // regression (a copy never cancelled) writes to the disk: 2 GiB, not more.
    let big = std::fs::File::create(s.left().join("sub/big.bin")).unwrap();
    big.set_len(2 << 30).unwrap();
    drop(big);
    let cfg = s.file("eio.toml", "[general]\nautosave_secs = 0\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", &cfg), ("XDEVCMD_TEST_NO_SIGNALS", "1")],
    );
    x.send(DOWN);
    x.send(ENTER); // into sub
    x.wait_for("/left/sub");
    x.send(DOWN);
    x.send(F5);
    x.wait_for("[Enter] go");
    x.send(ENTER);
    x.wait_for("background");
    x.hang_up();
    let start = Instant::now();
    while x.child.try_wait().unwrap().is_none() {
        assert!(
            start.elapsed() < PATIENCE,
            "still running with its terminal gone"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let saved = std::fs::read_to_string(s.0.join("session.toml")).expect("the session was saved");
    assert!(saved.contains("sub"), "{saved}");
    let left: Vec<_> = std::fs::read_dir(s.right())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "nothing left of the copy: {left:?}");
}

/// Safe operations on, but the journal cannot be made (its directory's name is taken by a
/// file): the copy is done, and the summary says it ran without safe operations.
#[test]
fn a_job_without_its_journal_says_so() {
    let s = Scratch::new("no-journal");
    s.file("left/a.txt", "a");
    s.file("journal", "a file where the journal directory goes");
    let mut x = Session::start(&s, None);
    x.send(DOWN);
    x.send(F5);
    x.wait_for("[Enter] go");
    x.send(ENTER);
    wait_file(&s.right().join("a.txt"), "copied", |c| c.is_some());
    x.wait_for("safe operations off for this job");
    x.quit();
}

/// A panic on a background thread is logged and xdevcmd goes on: still drawn, no
/// "crashed" over the screen, and q still quits.
#[test]
fn a_background_panic_is_logged_and_xdevcmd_goes_on() {
    let s = Scratch::new("bg-panic");
    let x = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_TEST_PANIC_THREAD", "1")],
    );
    x.wait_for("a background task failed");
    let screen = x.screen();
    assert!(screen.contains("10 Quit"), "{screen}");
    assert!(!screen.contains("crashed"), "{screen}");
    assert!(!screen.contains("panicked"), "{screen}");
    let logs: Vec<_> = std::fs::read_dir(&s.0)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
        .collect();
    assert_eq!(logs.len(), 1, "one crash log");
    let log = std::fs::read_to_string(logs[0].path()).unwrap();
    assert!(log.contains("xdevcmd went on"), "{log}");
    assert!(log.contains("thread: test-panic"), "{log}");
    x.quit();
}

/// The journal's first line, as a run on this machine writes it.
#[cfg(unix)]
fn host_line() -> String {
    format!(
        "host {}\n",
        hex(Path::new(&xdevcmd_core::journal::hostname()))
    )
}

/// A path as the journal writes it: hex of its bytes.
#[cfg(unix)]
fn hex(p: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A run killed in the middle of a copy left its journal and a temporary file: the next
/// start removes the file, says so, and offers to continue; Enter does.
#[cfg(unix)]
#[test]
fn an_interrupted_copy_is_tidied_up_and_resumed_at_the_next_start() {
    let s = Scratch::new("resume");
    let (src, dst) = (s.left(), s.right());
    std::fs::write(src.join("a.txt"), "a").unwrap();
    std::fs::write(src.join("b.txt"), "b").unwrap();
    // a.txt was copied (a copy keeps the date, and identical is what is skipped
    // unasked); b.txt was cut short, under its temporary name.
    std::fs::write(dst.join("a.txt"), "a").unwrap();
    let date = std::fs::metadata(src.join("a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(dst.join("a.txt"))
        .unwrap()
        .set_modified(date)
        .unwrap();
    let part = dst.join(".b.txt.xdevcmd-part-2147483646-0");
    std::fs::write(&part, "half").unwrap();
    let journal = s.0.join("journal");
    std::fs::create_dir_all(&journal).unwrap();
    std::fs::write(
        journal.join("2147483646-0.journal"),
        format!(
            "{}kind copy\nsrc {}\nsrc {}\ndest {}\npart {}\n",
            host_line(),
            hex(&src.join("a.txt")),
            hex(&src.join("b.txt")),
            hex(&dst),
            hex(&part)
        ),
    )
    .unwrap();
    let mut x = Session::start(&s, None);
    x.wait_for("Interrupted");
    x.wait_for("unfinished temporary file removed");
    assert!(!part.exists(), "the temporary file is gone");
    x.send(ENTER); // continue
                   // The job is over once its summary is on the status line: b.txt appears earlier,
                   // at its rename, while the worker still holds the journal and the job box (which
                   // ignores q) is still up.
    x.wait_for("1 copied");
    assert_eq!(std::fs::read_to_string(dst.join("b.txt")).unwrap(), "b");
    let start = Instant::now();
    while std::fs::read_dir(&journal).unwrap().next().is_some() {
        assert!(start.elapsed() < PATIENCE, "the journal is gone");
        std::thread::sleep(Duration::from_millis(25));
    }
    x.quit();
}

/// Continuing an interrupted copy never replaces a file the user kept in the first run:
/// a destination that differs from its (newer) source is asked about, and Skip keeps
/// it. With the old rule (update) the newer source replaced it unasked.
#[cfg(unix)]
#[test]
fn resume_from_app_does_not_replace_a_kept_newer_source_file() {
    let s = Scratch::new("resume-kept");
    let (src, dst) = (s.left(), s.right());
    std::fs::write(src.join("a.txt"), "the source, newer").unwrap();
    std::fs::write(dst.join("a.txt"), "kept").unwrap();
    let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(dst.join("a.txt"))
        .unwrap()
        .set_modified(hour_ago)
        .unwrap();
    let journal = s.0.join("journal");
    std::fs::create_dir_all(&journal).unwrap();
    std::fs::write(
        journal.join("2147483646-0.journal"),
        format!(
            "{}kind copy\nsrc {}\ndest {}\n",
            host_line(),
            hex(&src.join("a.txt")),
            hex(&dst),
        ),
    )
    .unwrap();
    let mut x = Session::start(&s, None);
    x.wait_for("Interrupted");
    x.send(ENTER); // continue
    x.wait_for("overwrite all");
    x.send(b"s");
    x.wait_for("1 skipped");
    assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "kept");
    x.quit();
}

/// An interrupted permanent delete is cleaned up after but never offered again: one
/// Enter would go on, where Shift+F8 itself takes only Y.
#[cfg(unix)]
#[test]
fn an_interrupted_permanent_delete_is_not_offered_again() {
    let s = Scratch::new("resume-del");
    let victim = s.file("left/keep.txt", "k");
    let journal = s.0.join("journal");
    std::fs::create_dir_all(&journal).unwrap();
    std::fs::write(
        journal.join("2147483646-0.journal"),
        format!("{}kind delete\nsrc {}\n", host_line(), hex(&victim)),
    )
    .unwrap();
    let mut x = Session::start(&s, None);
    x.wait_for("Interrupted");
    assert!(!x.screen().contains("Continue it?"), "{}", x.screen());
    x.send(ENTER);
    std::thread::sleep(Duration::from_millis(300));
    assert!(victim.exists(), "Enter deleted nothing");
    x.quit();
}

#[test]
fn shift_f8_warns_then_deletes_for_real() {
    let s = Scratch::new("delete");
    let victim = s.file("left/victim.txt", "x");
    let mut xdevcmd = Session::start(&s, None);

    xdevcmd.send(DOWN);
    xdevcmd.send(INSERT);
    xdevcmd.send(SHIFT_F8);
    // The thing the Python harness could never match: its byte stream had no spaces.
    xdevcmd.wait_for("This cannot be undone.");
    // Enter, out of habit, does not do it: Y does.
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("press Y to delete permanently");
    assert!(victim.exists(), "not on Enter");

    xdevcmd.send(b"y");
    wait_file(&victim, "deleted", |c| c.is_none());
    assert!(
        !s.0.join("data/Trash").exists(),
        "a permanent delete must not go through the trash"
    );
    // The file goes before the progress box closes, and that box ignores q: wait for
    // the result to reach the status line, or the q is swallowed (it was, once, on CI).
    xdevcmd.wait_for("1 deleted");
    xdevcmd.quit();
}

// ── F4 ─────────────────────────────────────────────────────────────────────────────

#[test]
fn f4_hands_the_terminal_to_the_editor_and_takes_it_back() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("editor");
    let target = s.file("left/draft.txt", "before\n");
    // A stand-in editor that proves which file it got.
    let editor = s.file(
        "fake-editor.sh",
        "#!/bin/sh\nprintf 'EDITED\\n' >> \"$1\"\n",
    );
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut xdevcmd = Session::start(&s, Some(&editor));
    xdevcmd.send(DOWN);
    xdevcmd.send(F4);
    wait_file(&target, "edited", |c| {
        c.is_some_and(|c| c.contains("EDITED"))
    });
    assert!(std::fs::read_to_string(&target)
        .unwrap()
        .starts_with("before"));

    // Coming back used to die on a cursor-position query this pty never answers —
    // the same silence a slow SSH link produces. Being here at all is the assertion.
    xdevcmd.wait_for("closed");
    xdevcmd.wait_for("10 Quit");
    xdevcmd.quit();
}

#[test]
fn keys_typed_while_the_editor_runs_reach_the_editor() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("editor-keys");
    let target = s.file("left/draft.txt", "");
    let ready = s.0.join("ready");
    // An editor that reads a line from the terminal, typed before it reads (an editor
    // still starting): whatever reads the terminal meanwhile takes the line, so
    // xdevcmd's input thread must not be reading while the editor runs.
    let editor = s.file(
        "line-editor.sh",
        &format!(
            "#!/bin/sh\n: > '{}'\nsleep 1\nIFS= read -r line\nprintf '%s\\n' \"$line\" >> \"$1\"\n",
            ready.display()
        ),
    );
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut xdevcmd = Session::start(&s, Some(&editor));
    xdevcmd.send(DOWN);
    xdevcmd.send(F4);
    wait_file(&ready, "the editor started", |c| c.is_some());
    xdevcmd.send(b"typed by hand\r");
    wait_file(&target, "the line typed", |c| {
        c.is_some_and(|c| c.contains("typed by hand"))
    });
    xdevcmd.wait_for("closed");
    xdevcmd.wait_for("10 Quit");
    xdevcmd.quit();
}

// ── mouse ──────────────────────────────────────────────────────────────────────────

#[test]
fn dragging_a_file_onto_the_other_pane_copies_it_after_asking() {
    let s = Scratch::new("drop");
    for n in ["apple.txt", "cherry.txt"] {
        s.file(&format!("left/{n}"), n);
    }
    let mut xdevcmd = Session::start(&s, None);

    let from = xdevcmd.locate("cherry");
    let (border, _) = xdevcmd.locate("\u{2510}\u{250c}");
    xdevcmd.drag(from, (border + 20, from.1));
    xdevcmd.wait_for("Copy cherry.txt here");
    assert!(
        !s.right().join("cherry.txt").exists(),
        "nothing until asked"
    );

    let (x, y) = xdevcmd.locate("Copy cherry.txt here");
    xdevcmd.click(x + 1, y, 0);
    wait_file(&s.right().join("cherry.txt"), "copied", |c| c.is_some());
    assert!(s.left().join("cherry.txt").exists(), "copy, not move");
    assert!(
        !s.right().join("apple.txt").exists(),
        "only the grabbed file"
    );
    xdevcmd.wait_for("1 copied"); // the progress box ignores q until it has closed
    xdevcmd.quit();
}

#[test]
fn ctrl_click_marks_and_the_mark_is_what_f5_copies() {
    let s = Scratch::new("ctrl-click");
    for n in ["apple.txt", "berry.txt"] {
        s.file(&format!("left/{n}"), n);
    }
    let mut xdevcmd = Session::start(&s, None);

    let (x, y) = xdevcmd.locate("apple");
    xdevcmd.click(x, y, 16);
    xdevcmd.wait_for("\u{2022}apple");
    // Move the cursor away: if F5 still copies apple, the mark did it, not the cursor.
    xdevcmd.send(DOWN);
    xdevcmd.send(F5);
    xdevcmd.wait_for("[Enter] go");
    xdevcmd.send(ENTER);
    wait_file(&s.right().join("apple.txt"), "copied", |c| c.is_some());
    assert!(
        !s.right().join("berry.txt").exists(),
        "the cursor's file stays put"
    );
    xdevcmd.wait_for("1 copied"); // the progress box ignores q until it has closed
    xdevcmd.quit();
}

#[test]
fn dragging_the_border_resizes_the_panes() {
    let s = Scratch::new("resize");
    let mut xdevcmd = Session::start(&s, None);
    let (border, _) = xdevcmd.locate("\u{2510}\u{250c}");
    xdevcmd.drag((border, 10), (30, 10));
    xdevcmd.wait_for("left 2");
    let (moved, _) = xdevcmd.locate("\u{2510}\u{250c}");
    assert!(moved < border, "the boundary should follow the pointer");
    xdevcmd.quit();
}

// ── terminal tabs and splits ───────────────────────────────────────────────────────

const ALT_N: &[u8] = b"\x1bn";
/// Inside the shell xdevcmd takes only F12's family (keymap A); its terminal keys are
/// reached through this prefix: Alt+X, then the key.
const ALT_X: &[u8] = b"\x1bx";
const ALT_S: &[u8] = b"\x1bs";
const CTRL_PAGE_UP: &[u8] = b"\x1b[5;5~";

#[test]
fn each_terminal_tab_is_its_own_shell() {
    let s = Scratch::new("tabs");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.send(b"echo ONE_$((0+1))\r");
    xdevcmd.wait_for("ONE_1");

    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.wait_gone("ONE_1");
    xdevcmd.send(b"echo TWO_$((1+1))\r");
    xdevcmd.wait_for("TWO_2");

    // Back to the first tab: its own screen, untouched by what ran in the second.
    xdevcmd.send(ALT_X);
    xdevcmd.send(CTRL_PAGE_UP);
    xdevcmd.wait_for("ONE_1");
    xdevcmd.wait_gone("TWO_2");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

#[test]
fn typing_exit_closes_that_terminal_and_the_last_one_closes_the_panel() {
    let s = Scratch::new("exit");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_S);
    xdevcmd.wait_for("split 2/2");

    xdevcmd.send(b"exit\r");
    xdevcmd.wait_for("shell exited");
    xdevcmd.until("the split to go", |scr| !scr.contains("split 2/2"));

    xdevcmd.send(b"exit\r");
    xdevcmd.until("the panel to go", |scr| !scr.contains('\u{2302}'));
    // Focus is back on the files, so q quits rather than reaching a shell.
    xdevcmd.quit();
}

#[test]
fn the_wheel_scrolls_back_through_shell_history() {
    let s = Scratch::new("scrollback");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.send(b"seq 1 300\r");
    xdevcmd.wait_for("300");

    let (_, bar_y) = xdevcmd.locate("\u{2302} 1");
    for _ in 0..10 {
        // SGR wheel-up is button 64.
        xdevcmd.mouse(64, 10, bar_y + 3, false);
    }
    xdevcmd.wait_for("lines back");

    // Typing returns to the live screen, or you would type into one you cannot see.
    xdevcmd.send(b"x");
    xdevcmd.wait_gone("lines back");
    xdevcmd.send(b"\x15"); // Ctrl+U clears the stray x
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Runs `setup` in the shell, then reads one line and shows it with `cat -v`, so what a
/// wheel turn sent the program is on screen. Returns what it showed.
fn wheel_into_program(tag: &str, setup: &str, reset: &str) -> String {
    let s = Scratch::new(tag);
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    // "rea" "dy" apart, or the typed command itself would say it is ready.
    let cmd = format!(
        "stty -echo; printf '{setup}'; printf '%s%s\\n' rea dy; read -r l; \
         printf '{reset}'; stty echo; echo \"got:$l\" | cat -v\r"
    );
    xdevcmd.send(cmd.as_bytes());
    xdevcmd.wait_for("ready");
    let (_, bar_y) = xdevcmd.locate("\u{2302} 1");
    xdevcmd.mouse(64, 10, bar_y + 3, false);
    xdevcmd.send(b"\r");
    xdevcmd.wait_for("got:^[");
    let screen = xdevcmd.screen();
    xdevcmd.send(F12);
    xdevcmd.quit();
    screen
}

#[test]
fn the_wheel_goes_to_a_program_that_asked_for_the_mouse() {
    // As Claude Code, vim and htop ask: button presses, reported the SGR way.
    let screen = wheel_into_program(
        "wheel-mouse",
        "\\033[?1000h\\033[?1006h",
        "\\033[?1000l\\033[?1006l",
    );
    assert!(screen.contains("got:^[[<64;"), "{screen}");
}

#[test]
fn the_wheel_is_arrow_keys_to_a_full_screen_program_without_the_mouse() {
    // As less and man run: the other screen, no mouse.
    let screen = wheel_into_program("wheel-arrows", "\\033[?1049h", "\\033[?1049l");
    assert!(screen.contains("got:^[[A^[[A^[[A"), "{screen}");
}

#[test]
fn xdevcmd_will_not_start_inside_its_own_terminal_panel() {
    let s = Scratch::new("nested");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    let cmd = format!("'{}'\r", env!("CARGO_BIN_EXE_xdevcmd"));
    xdevcmd.send(cmd.as_bytes());
    xdevcmd.wait_for("already running");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

// ── rename ─────────────────────────────────────────────────────────────────────────

#[test]
fn shift_f6_and_f2_rename_through_real_key_bytes() {
    let s = Scratch::new("rename");
    s.file("left/draft.md", "text");
    let mut xdevcmd = Session::start(&s, None);

    xdevcmd.send(DOWN);
    xdevcmd.send(b"\x1b[17;2~"); // Shift+F6, as xterm sends it
    xdevcmd.wait_for("[Enter] rename");
    xdevcmd.send(b"final");
    xdevcmd.send(ENTER);
    wait_file(&s.left().join("final.md"), "renamed", |c| {
        c.as_deref() == Some("text")
    });
    assert!(!s.left().join("draft.md").exists());

    xdevcmd.send(b"\x1bOQ"); // F2
    xdevcmd.wait_for("[Enter] rename");
    xdevcmd.send(b"\x1b"); // Esc cancels: nothing changes
    xdevcmd.wait_gone("[Enter] rename");
    assert!(s.left().join("final.md").exists());
    xdevcmd.quit();
}

#[test]
fn alt_arrows_navigate_through_real_key_bytes() {
    let s = Scratch::new("alt-arrows");
    std::fs::create_dir(s.left().join("deeper")).unwrap();
    s.file("left/deeper/inside.txt", "x");
    let mut xdevcmd = Session::start(&s, None);

    xdevcmd.send(DOWN);
    xdevcmd.send(b"\x1b[1;3C"); // Alt+Right, as xterm sends it
    xdevcmd.wait_for("inside");
    xdevcmd.send(b"\x1b[1;3D"); // Alt+Left
    xdevcmd.wait_gone("inside");
    xdevcmd.wait_for("deeper");

    // Resizing moved to Alt+Shift+arrows: ESC [1;4C, a rarer modifier code. If it did
    // not parse, keyboard resizing would silently stop working.
    let (before, _) = xdevcmd.locate("\u{2510}\u{250c}");
    xdevcmd.send(b"\x1b[1;4C");
    xdevcmd.until("the pane to grow", |scr| {
        scr.lines()
            .find_map(|l| l.find("\u{2510}\u{250c}").map(|b| l[..b].chars().count()))
            .is_some_and(|x| x as u16 > before)
    });
    xdevcmd.quit();
}

// ── tab names ──────────────────────────────────────────────────────────────────────

/// Linux only: the names come from /proc. Elsewhere tabs keep the shell's name.
#[cfg(target_os = "linux")]
#[test]
fn a_tab_is_named_by_what_its_shell_is_doing() {
    let s = Scratch::new("tab-names");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    // Idle at a prompt: the shell's directory.
    xdevcmd.wait_for("\u{2302} 1 left");

    xdevcmd.send(b"cd /tmp\r");
    xdevcmd.wait_for("\u{2302} 1 tmp");

    // A program in the foreground: its name, and back again when it ends.
    xdevcmd.send(b"sleep 30\r");
    xdevcmd.wait_for("\u{2302} 1 sleep");
    xdevcmd.send(b"\x03"); // Ctrl+C
    xdevcmd.wait_for("\u{2302} 1 tmp");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

// ── hostile input ──────────────────────────────────────────────────────────────────
//
// Each of these was an attack that worked before it was fixed.

const ALT_C: &[u8] = b"\x1bc";

#[test]
fn a_directory_name_cannot_run_a_command_through_alt_c() {
    let s = Scratch::new("inject");
    // Ctrl+U kills the typed line, Enter runs what follows. Before the fix this
    // created PWNED: readline acts on both before the shell sees the quotes.
    std::fs::create_dir(s.left().join("evil\u{15}touch PWNED\r")).unwrap();
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(F12);
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.send(ALT_C);
    // The disk first, because that is the harm; the message second.
    std::thread::sleep(Duration::from_millis(800));
    let pwned: Vec<_> = walk(&s.0)
        .into_iter()
        .filter(|p| p.ends_with("PWNED"))
        .collect();
    assert!(pwned.is_empty(), "a command ran: {pwned:?}");
    xdevcmd.wait_for("control character");
    xdevcmd.quit();
}

/// Linux reads /proc, macOS asks ps. Windows cannot tell yet.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn alt_c_does_not_type_into_a_program_that_owns_the_terminal() {
    let s = Scratch::new("busy");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    // Stand-in for a sudo password prompt or an editor: whatever owns the terminal
    // would receive the typed cd.
    xdevcmd.send(b"cat > typed.txt\r");
    xdevcmd.wait_for("\u{2302} 1 cat");
    xdevcmd.send(F12);
    xdevcmd.send(ALT_C);
    xdevcmd.wait_for("cat is running in the terminal");
    xdevcmd.send(F12);
    xdevcmd.send(b"\x04"); // Ctrl+D ends cat
    std::thread::sleep(Duration::from_millis(300));
    let typed = std::fs::read_to_string(s.left().join("typed.txt")).unwrap_or_default();
    assert!(!typed.contains("cd "), "the cd went into cat: {typed:?}");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

const ALT_G: &[u8] = b"\x1bg";

/// Linux reads /proc, macOS asks lsof (not run here; enabled so CI on macOS runs it).
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn alt_g_takes_the_pane_to_the_shells_directory() {
    let s = Scratch::new("altg");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    s.file("left/sub/inside-sub.txt", "x");
    s.file("left/at-left.txt", "x");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"cd sub\r");
    std::thread::sleep(Duration::from_millis(300));
    xdevcmd.send(F12);
    xdevcmd.send(ALT_G);
    xdevcmd.wait_for("inside-sub");
    xdevcmd.wait_for("the shell's directory:");
    assert!(!xdevcmd.screen().contains("at-left"));
    // Again: nothing to do, and said so.
    xdevcmd.send(ALT_G);
    xdevcmd.wait_for("already in the shell's directory");
    xdevcmd.quit();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn alt_g_refuses_while_a_program_runs_and_when_the_directory_is_gone() {
    let s = Scratch::new("altg-no");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    s.file("left/at-left.txt", "x");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"cd sub && sleep 30\r");
    xdevcmd.wait_for("\u{2302} 1 sleep");
    xdevcmd.send(F12);
    xdevcmd.send(ALT_G);
    xdevcmd.wait_for("sleep is running in the shell");
    assert!(xdevcmd.screen().contains("at-left"), "pane unchanged");
    xdevcmd.send(F12);
    xdevcmd.send(b"\x03"); // Ctrl+C ends sleep
    xdevcmd.send(b"rmdir ../sub\r");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!s.left().join("sub").exists());
    xdevcmd.send(F12);
    xdevcmd.send(ALT_G);
    xdevcmd.wait_for("is not there any more");
    assert!(xdevcmd.screen().contains("at-left"), "pane unchanged");
    xdevcmd.quit();
}

#[test]
fn f3_on_a_fifo_says_so_instead_of_freezing() {
    let s = Scratch::new("fifo");
    let made = std::process::Command::new("mkfifo")
        .arg(s.left().join("README.md"))
        .status()
        .unwrap();
    assert!(made.success());
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(DOWN);
    xdevcmd.send(b"\x1bOR"); // F3
                             // Before: File::open on the FIFO never returned and nothing redrew again.
    xdevcmd.wait_for("FIFO");
    xdevcmd.quit();
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            }
            out.push(p);
        }
    }
    out
}

// ── session ────────────────────────────────────────────────────────────────────────

#[test]
fn quitting_and_starting_again_brings_back_the_panes_tabs_and_directories() {
    let s = Scratch::new("session");
    std::fs::create_dir(s.left().join("sub")).unwrap();
    let mut xdevcmd = Session::start(&s, None);

    xdevcmd.send(b"\x14"); // Ctrl+T: a second tab
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER); // into sub
    xdevcmd.send(ALT_S); // a third pane
    std::thread::sleep(Duration::from_millis(200));
    xdevcmd.quit();
    assert!(
        s.0.join("session.toml").exists(),
        "q should save the session"
    );

    let mut again = Session::start_with(&s, None, &[]);
    again.wait_for(" 2 sub ");
    // The left pane has a tab bar, so its top border is one row lower than the
    // others': count the corners across the two rows below the menu bar.
    let screen = again.screen();
    let corners: usize = screen
        .lines()
        .skip(1)
        .take(2)
        .map(|l| l.matches('\u{250c}').count())
        .sum();
    assert_eq!(corners, 3, "three panes, as left:\n{screen}");
    again.send(b"\x1b[5;5~"); // Ctrl+PgUp: tab 1 is still the pane's own directory
    again.wait_for(" 1 left ");
    again.quit();
}

/// Linux only: a shell's directory is read from /proc to be saved.
#[cfg(target_os = "linux")]
#[test]
fn terminal_tabs_reopen_where_their_shells_were() {
    let s = Scratch::new("session-term");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.send(b"cd /tmp\r");
    xdevcmd.wait_for("\u{2302} 1 tmp");
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("\u{2302} 2 left");
    xdevcmd.send(F12);
    xdevcmd.quit();

    let again = Session::start_with(&s, None, &[]);
    again.wait_for("\u{2302} 1 tmp");
    again.wait_for("\u{2302} 2 left");
    again.quit();
}

// ── secure modes, through real keys ────────────────────────────────────────────────

impl Session {
    fn start_flags(s: &Scratch, flags: &[&str]) -> Self {
        let mut dirs: Vec<PathBuf> = flags.iter().map(PathBuf::from).collect();
        dirs.push(s.left());
        dirs.push(s.right());
        Self::start_with(s, None, &dirs)
    }
}

#[test]
fn read_only_mode_changes_nothing_on_disk_and_saves_no_session() {
    let s = Scratch::new("read-only");
    s.file("left/keep.txt", "keep");
    let mut xdevcmd = Session::start_flags(&s, &["--read-only"]);
    xdevcmd.wait_for("READ-ONLY");
    for keys in [
        &b"\x1b[19~"[..], // F8
        b"\x1b[19;2~",    // Shift+F8
        b"\x1b[15~",      // F5
        b"\x1b[17~",      // F6
        b"\x1bOQ",        // F2
        b"\x1b[18~",      // F7
    ] {
        xdevcmd.send(DOWN);
        xdevcmd.send(keys);
        xdevcmd.send(ENTER);
        xdevcmd.send(b"x");
        xdevcmd.send(ENTER);
        xdevcmd.send(b"\x1b");
    }
    xdevcmd.quit();
    assert_eq!(
        std::fs::read_to_string(s.left().join("keep.txt")).unwrap(),
        "keep"
    );
    let left: Vec<_> = std::fs::read_dir(s.left()).unwrap().collect();
    let right: Vec<_> = std::fs::read_dir(s.right()).unwrap().collect();
    assert_eq!(
        (left.len(), right.len()),
        (1, 0),
        "nothing created, moved or copied"
    );
    assert!(
        !s.0.join("session.toml").exists(),
        "read-only writes no session either"
    );
}

#[test]
fn restricted_mode_never_starts_a_shell() {
    let s = Scratch::new("restricted");
    let mut xdevcmd = Session::start_flags(&s, &["--restricted"]);
    xdevcmd.wait_for("RESTRICTED");
    xdevcmd.send(F12);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("the terminal is turned off");
    assert!(!xdevcmd.screen().contains('\u{2302}'));
    xdevcmd.quit();
}

// ── group A: cd on exit, and a directory that changes underneath ──────────────────

#[test]
fn q_writes_the_directory_for_the_shell_and_shift_q_does_not() {
    let s = Scratch::new("cwd-file");
    std::fs::create_dir_all(s.left().join("deeper")).unwrap();
    let out = s.0.join("cwd");
    let flag = format!("--cwd-file={}", out.display());

    let mut xdevcmd = Session::start_flags(&s, &[&flag]);
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("deeper ");
    xdevcmd.quit();
    let got = std::fs::read_to_string(&out).expect("q writes it");
    assert_eq!(
        std::path::Path::new(&got),
        std::fs::canonicalize(s.left().join("deeper")).unwrap(),
        "no newline, nothing else: a shell does cd \"$(cat file)\""
    );

    std::fs::remove_file(&out).unwrap();
    let mut xdevcmd = Session::start_flags(&s, &[&flag]);
    xdevcmd.send(b"Q");
    let start = Instant::now();
    while xdevcmd.child.try_wait().ok().flatten().is_none() {
        assert!(start.elapsed() < PATIENCE, "Q should quit");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(!out.exists(), "Q leaves the shell where it was");
}

#[test]
fn a_file_created_outside_xdevcmd_appears_without_a_key_press() {
    let s = Scratch::new("refresh");
    s.file("left/b-existing.txt", "x");
    let xdevcmd = Session::start(&s, None);
    xdevcmd.wait_for("b-existing");
    // The cursor sits on b-existing; a file sorting above it must not move it.
    let mut xdevcmd = xdevcmd;
    xdevcmd.send(DOWN);
    s.file("left/a-new.txt", "made elsewhere");
    xdevcmd.wait_for("a-new");
    let status = || {
        let sc = xdevcmd.screen();
        sc.lines().rev().nth(1).unwrap_or_default().to_string()
    };
    assert!(
        status().contains("b-existing"),
        "the cursor stayed on its file:\n{}",
        xdevcmd.screen()
    );
    // A file that grows is noticed too, not only one that appears.
    std::fs::write(s.left().join("a-new.txt"), vec![b'x'; 300_000]).unwrap();
    xdevcmd.wait_for("293 K");
    xdevcmd.quit();
}

// ── group B: bulk rename through the editor ─────────────────────────────────────────

#[test]
fn shift_f2_renames_what_the_editor_changed_after_asking() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("bulk");
    s.file("left/a.txt", "A");
    s.file("left/b.txt", "B");
    s.file("left/c.txt", "C");
    // An "editor" that swaps a and b and leaves c: the swap only works because the
    // renames go through temporary names first.
    let editor = s.file(
        "swap-editor.sh",
        "#!/bin/sh\nsed -e 's/^a.txt$/TMP/' -e 's/^b.txt$/a.txt/' -e 's/^TMP$/b.txt/' \"$1\" > \"$1.new\" && mv \"$1.new\" \"$1\"\n",
    );
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut xdevcmd = Session::start(&s, Some(&editor));
    xdevcmd.send(b"\x01"); // Ctrl+A: mark all
    xdevcmd.send(b"\x1b[1;2Q"); // Shift+F2
    xdevcmd.wait_for("edit in editor");
    xdevcmd.send(b"\x1bOS"); // F4: the editor route
    xdevcmd.wait_for("Rename 2 files");
    xdevcmd.wait_for("a.txt  \u{2192}  b.txt");
    assert_eq!(
        std::fs::read_to_string(s.left().join("a.txt")).unwrap(),
        "A",
        "nothing renamed before Enter"
    );
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("renamed 2");
    let read = |n: &str| std::fs::read_to_string(s.left().join(n)).unwrap();
    assert_eq!(
        (read("a.txt"), read("b.txt"), read("c.txt")),
        ("B".into(), "A".into(), "C".into())
    );
    let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with(&format!(
                "xdevcmd-rename-{}-",
                xdevcmd.child.process_id().unwrap_or(0)
            ))
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "the list of names is deleted afterwards"
    );
    xdevcmd.quit();
}

#[test]
fn shift_f9_types_a_menu_command_into_the_shell_with_names_quoted() {
    let s = Scratch::new("usermenu");
    s.file("left/it's here.txt", "x");
    s.file(
        "config.toml",
        "[[menu]]\nkey = \"e\"\nlabel = \"say it\"\ncommand = \"echo GOT %s DONE\"\n",
    );
    let flag = format!("--config={}", s.0.join("config.toml").display());
    let mut xdevcmd = Session::start_flags(&s, &[&flag]);
    xdevcmd.send(DOWN);
    xdevcmd.send(b"\x1b[20;2~"); // Shift+F9
    xdevcmd.wait_for("say it");
    xdevcmd.send(b"e");
    xdevcmd.wait_for("GOT it's here.txt DONE");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

// ── long-running: does memory settle? ───────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn rss_of(pid: u32) -> Option<u64> {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
        .map(|kb| kb * 1024)
}

/// Drive xdevcmd through everything that allocates — listings, tabs, the viewer, search,
/// filter, quick view, sizes, find, the usage view, terminal output — over and over,
/// sampling its resident memory once a second. After a warm-up, the second half of the
/// run must not sit meaningfully above the first: memory that only ever goes up is a
/// leak, however slow.
///
/// Off by default (it takes minutes). Run it with:
///   XDEVCMD_SOAK_SECS=600 cargo test --release --test pty -- --ignored soak
/// The samples are written to `soak-rss.csv` in cargo's test scratch directory.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "long-running; see the doc comment"]
fn soak_memory_settles_instead_of_growing() {
    let secs: u64 = std::env::var("XDEVCMD_SOAK_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let s = Scratch::new("soak");
    for i in 0..300 {
        s.file(&format!("left/f{i:03}.txt"), &"line\n".repeat(i % 50 + 1));
    }
    for d in ["sub-a", "sub-b"] {
        std::fs::create_dir_all(s.left().join(d).join("deeper")).unwrap();
        s.file(&format!("left/{d}/deeper/x.txt"), "x");
    }
    let mut xdevcmd = Session::start(&s, None);
    let pid = xdevcmd.child.process_id().expect("a pid");
    let start = Instant::now();
    let mut samples: Vec<(f64, u64)> = Vec::new();
    let mut last_sample = Instant::now() - Duration::from_secs(2);
    let mut cycles = 0u64;
    let steps: &[&[u8]] = &[
        b"\x1b[B",
        b"\x1b[B",
        b"\r",
        b"\x7f", // into sub-a and back
        b"\x1b[F",
        b"\x1bOR",
        b"\x1b", // End, F3, Esc
        b"\x14",
        b"\x17", // Ctrl+T, Ctrl+W
        b"/",
        b"f",
        b"1",
        b"\r", // quick search
        b"\x06",
        b".",
        b"t",
        b"x",
        b"t",
        b"\r",
        b"\x1b", // filter, then off
        b"\x1bq",
        b"\x1b[B",
        b"\x1bq", // quick view on, move, off
        b" ",
        b"\x00", // Space (size), Ctrl+Space
        b"\x1b[1;2P",
        b"\x1b", // Shift+F1 usage, Esc
        b"\x1b[20~",
        b"\x1b", // F9 menu, Esc
        b"\x1b[24~",
        b"seq 1 3000\r",
        b"\x1b[24~", // shell output into scrollback
        b"\x1b[18;3~",
        b"\r",
        b"\x17",   // Alt+F7 find, close the tab
        b"\x1b[H", // Home
    ];
    while start.elapsed() < Duration::from_secs(secs) {
        for step in steps {
            xdevcmd.send(step);
            if last_sample.elapsed() >= Duration::from_secs(1) {
                last_sample = Instant::now();
                if let Some(b) = rss_of(pid) {
                    samples.push((start.elapsed().as_secs_f64(), b));
                }
            }
        }
        cycles += 1;
    }
    let csv: String = std::iter::once("seconds,rss_bytes".to_string())
        .chain(samples.iter().map(|(t, b)| format!("{t:.1},{b}")))
        .collect::<Vec<_>>()
        .join("\n");
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("soak-rss.csv");
    let _ = std::fs::write(&out, csv);
    xdevcmd.quit();

    // A fifth of the run is warm-up: caches filling, the allocator finding its size.
    let warm = samples.len() / 5;
    let rest = &samples[warm..];
    assert!(rest.len() >= 20, "too few samples: {}", samples.len());
    let median = |v: &[(f64, u64)]| {
        let mut b: Vec<u64> = v.iter().map(|s| s.1).collect();
        b.sort_unstable();
        b[b.len() / 2]
    };
    let (first, second) = rest.split_at(rest.len() / 2);
    let (m1, m2) = (median(first), median(second));
    let max = samples.iter().map(|s| s.1).max().unwrap_or(0);
    eprintln!(
        "soak: {secs}s, {cycles} cycles, {} samples \u{b7} median {} MiB then {} MiB \u{b7} max {} MiB \u{b7} {}",
        samples.len(),
        m1 >> 20,
        m2 >> 20,
        max >> 20,
        out.display()
    );
    // 10 % plus 2 MiB of slack: allocators do not hand memory back on a schedule.
    assert!(
        m2 <= m1 + m1 / 10 + (2 << 20),
        "memory kept growing: median {} KiB in the first half after warm-up, {} KiB in the second",
        m1 >> 10,
        m2 >> 10
    );
}

/// Windows found this: ConPTY asks its terminal where the cursor is (ESC[6n) before it
/// lets the shell start, and an unanswered query left cmd.exe dead. The same query from
/// a program on Unix must be answered too: here `dd` waits for the reply's first bytes,
/// which never came before.
#[test]
fn the_terminal_answers_where_the_cursor_is() {
    let s = Scratch::new("cpr");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"stty -icanon -echo; printf '\\033[6n'; dd bs=1 count=2 2>/dev/null | od -An -c; stty sane\r");
    xdevcmd.wait_for("033   [");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// The shell's keys are the shell's: Ctrl+R (history search), Tab (completion) and
/// Ctrl+Space (set mark) reach the program in the panel as a terminal sends them.
#[test]
fn ctrl_r_tab_and_ctrl_space_reach_the_program_in_the_panel() {
    let s = Scratch::new("shell-keys");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"stty raw -echo; dd bs=1 count=3 2>/dev/null | od -An -c; stty sane\r");
    std::thread::sleep(Duration::from_millis(300));
    // Ctrl+R, Tab, Ctrl+Space, as a terminal sends them.
    xdevcmd.send(b"\x12");
    xdevcmd.send(b"\t");
    xdevcmd.send(b"\x00");
    xdevcmd.wait_for("022  \\t  \\0");
    // Still in the panel: none of them was taken by xdevcmd.
    assert!(
        xdevcmd.screen().contains("F12 back to files"),
        "{}",
        xdevcmd.screen()
    );
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Keymap A (owner, 2026-11-09): with the panel focused, xdevcmd takes only F12's family
/// (F12, Shift+F12, and Alt+X for their two-key forms). Every other Alt+letter, every
/// Alt+digit, every Ctrl+letter and F1…F11 reach the program, byte for byte as a
/// terminal sent them, and none of them does anything of xdevcmd's: the shell keeps
/// focus, no tab, split or panel change. Before, Alt+N, Alt+S, Alt+D, Alt+W, Alt+O,
/// Alt+T, Alt+C, Alt+Z, Alt+R and Alt+1…9 were xdevcmd's inside the shell, and no F-key
/// reached it at all.
#[test]
fn inside_the_shell_only_f12s_family_is_xdevcmds() {
    let s = Scratch::new("keymap-a");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    let mut keys: Vec<Vec<u8>> = Vec::new();
    for c in (b'a'..=b'z').filter(|c| *c != b'x') {
        keys.push(vec![0x1b, c]);
    }
    for d in b'1'..=b'9' {
        keys.push(vec![0x1b, d]);
    }
    for c in 1u8..=26 {
        keys.push(vec![c]);
    }
    for f in [
        &b"\x1bOP"[..],
        b"\x1bOQ",
        b"\x1bOR",
        b"\x1bOS",
        b"\x1b[15~",
        b"\x1b[17~",
        b"\x1b[18~",
        b"\x1b[19~",
        b"\x1b[20~",
        b"\x1b[21~",
        b"\x1b[23~",
        b"\x1b[17;2~", // Shift+F6, rename in the panes
    ] {
        keys.push(f.to_vec());
    }
    let expected: Vec<u8> = keys.concat();
    let cmd = format!(
        "stty raw -echo; dd bs=1 count={} of=keys.bin 2>/dev/null; stty sane; echo GOT_ALL\r",
        expected.len()
    );
    xdevcmd.send(cmd.as_bytes());
    std::thread::sleep(Duration::from_millis(300));
    for k in &keys {
        // One key per write, as a person types them: ESC and its letter together is
        // Alt+letter, as a terminal sends it.
        let mut w = xdevcmd.writer.lock().unwrap();
        w.write_all(k).unwrap();
        w.flush().unwrap();
        drop(w);
        std::thread::sleep(Duration::from_millis(15));
    }
    xdevcmd.wait_for("GOT_ALL");
    let got = std::fs::read(s.left().join("keys.bin")).unwrap();
    assert_eq!(
        got, expected,
        "every key reached the program as it was sent"
    );
    let screen = xdevcmd.screen();
    assert!(
        screen.contains("F12 back to files"),
        "the shell kept focus\n{screen}"
    );
    assert!(screen.contains("terminal 1/1"), "no tab opened\n{screen}");
    assert!(!screen.contains("split"), "no split\n{screen}");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// The prefix inside the shell: Alt+X, then a terminal key, acts on the shell (here
/// Alt+N, a new tab); Alt+X then a key that means nothing to xdevcmd there sends both
/// to the program, as if xdevcmd had not looked.
#[test]
fn alt_x_is_the_prefix_inside_the_shell_and_gives_back_what_it_does_not_use() {
    let s = Scratch::new("prefix");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd
        .send(b"stty raw -echo; dd bs=1 count=3 of=keys.bin 2>/dev/null; stty sane; echo GOT_IT\r");
    std::thread::sleep(Duration::from_millis(300));
    xdevcmd.send(ALT_X);
    xdevcmd.wait_for("ALT+X");
    xdevcmd.send(b"q");
    xdevcmd.wait_for("GOT_IT");
    assert_eq!(std::fs::read(s.left().join("keys.bin")).unwrap(), b"\x1bxq");
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Inside the shell only Esc cancels the prefix. Enter after Alt+X is the program's like
/// any other key xdevcmd does not use there: it receives `ESC x` and then Enter, instead
/// of losing both.
#[test]
fn alt_x_then_enter_in_the_shell_reaches_the_program() {
    let s = Scratch::new("prefix-enter");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd
        .send(b"stty raw -echo; dd bs=1 count=4 of=keys.bin 2>/dev/null; stty sane; echo GOT_IT\r");
    std::thread::sleep(Duration::from_millis(300));
    // Alt+X, Esc: cancelled, nothing sent.
    xdevcmd.send(ALT_X);
    xdevcmd.wait_for("ALT+X");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("ALT+X");
    xdevcmd.send(b"z");
    // Alt+X, Enter: both to the program.
    xdevcmd.send(ALT_X);
    xdevcmd.wait_for("ALT+X");
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("GOT_IT");
    assert_eq!(
        std::fs::read(s.left().join("keys.bin")).unwrap(),
        b"z\x1bx\r"
    );
    xdevcmd.send(F12);
    xdevcmd.quit();
}

const SHIFT_F12: &[u8] = b"\x1b[24;2~";

/// Shift+F12: the shell's history in the viewer, at its end; Home goes to its first
/// line; a drag and y put those lines on the clipboard (OSC 52); Esc is back in the
/// shell, typing into it.
#[test]
fn shift_f12_reads_the_shells_history_in_the_viewer() {
    let s = Scratch::new("history");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"seq 1 300\r");
    xdevcmd.wait_for("300");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("history \u{b7} sh \u{b7} tab 1");
    // The command line is the history's line 1, so seq's 300 is line 301; the prompt
    // after it is the last line, on the viewer's last row.
    xdevcmd.wait_for("301 \u{2502}300");
    xdevcmd.wait_for("302 \u{2502}$");
    xdevcmd.wait_for("in memory only");
    // Nothing of the shell's output was written anywhere xdevcmd keeps files.
    let written: Vec<_> = walk(&s.0)
        .into_iter()
        .filter(|p| std::fs::read(p).is_ok_and(|b| b.windows(8).any(|w| w == b"299\n300\n")))
        .collect();
    assert!(written.is_empty(), "history on disk: {written:?}");
    xdevcmd.send(b"\x1b[H"); // Home
    xdevcmd.wait_for("  1 \u{2502}$ seq 1 300");
    let (x, y) = xdevcmd.locate("  2 \u{2502}1");
    let (_, y3) = xdevcmd.locate("  3 \u{2502}2");
    assert_eq!(y3, y + 1);
    xdevcmd.drag((x + 6, y), (x + 6, y + 1));
    xdevcmd.wait_for("2 lines selected");
    xdevcmd.send(b"y");
    xdevcmd.wait_for("copied 2 lines");
    let raw = String::from_utf8_lossy(&xdevcmd.raw.lock().unwrap()).into_owned();
    // base64 of "1\n2".
    assert!(
        raw.contains("\x1b]52;c;MQoy\x07"),
        "OSC 52 with the two lines"
    );
    // The first Esc lets go of the selection, as always in the viewer; the second
    // closes it.
    xdevcmd.send(b"\x1b");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(b"echo BACK_$((1+1))\r");
    xdevcmd.wait_for("BACK_2");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// A full-screen program on the other screen (htop, vim) leaves vt100 no history to
/// give: the viewer shows that screen and says so, instead of counting it as history.
#[test]
fn shift_f12_over_a_full_screen_program_says_it_is_that_screen_only() {
    let s = Scratch::new("history-alt");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"printf '\\033[?1049hON_THE_%s\\n' OTHER; read x; printf '\\033[?1049l'\r");
    xdevcmd.wait_for("ON_THE_OTHER");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("a full-screen program is running");
    xdevcmd.wait_for("its screen only");
    assert!(!xdevcmd.screen().contains("in memory only"));
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(ENTER); // ends `read`, back to the shell's own screen
    xdevcmd.wait_gone("ON_THE_OTHER");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// The history view is held in memory: with HOME and TMPDIR inside the scratch
/// directory, so that nothing xdevcmd could write lands anywhere unchecked, no file
/// holds the shell's output after Shift+F12 and Esc, nor after quitting.
#[test]
fn history_is_never_written_outside_the_shell() {
    let s = Scratch::new("history-nowhere");
    let home = s.0.join("home");
    let tmp = s.0.join("tmp");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    let (h, t) = (home.to_string_lossy(), tmp.to_string_lossy());
    let mut xdevcmd = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("HOME", h.as_ref()), ("TMPDIR", t.as_ref())],
    );
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    // Only the output holds MARK_42 and seq's run of lines; the command line does not.
    xdevcmd.send(b"echo MARK_$((6*7)); seq 1 300\r");
    xdevcmd.wait_for("300");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    let holds = |p: &PathBuf| {
        std::fs::read(p).is_ok_and(|b| {
            b.windows(7).any(|w| w == b"MARK_42") || b.windows(8).any(|w| w == b"299\n300\n")
        })
    };
    let written: Vec<_> = walk(&s.0).into_iter().filter(holds).collect();
    assert!(written.is_empty(), "history on disk: {written:?}");
    xdevcmd.send(F12);
    xdevcmd.quit();
    let written: Vec<_> = walk(&s.0).into_iter().filter(holds).collect();
    assert!(
        written.is_empty(),
        "history on disk after quitting: {written:?}"
    );
}

const F2: &[u8] = b"\x1bOQ";

/// F2 in the history view: offered as xdevcmd-history-<date>-<time>.txt in the shell's
/// directory, written there readable by its owner only; F2 to a name that exists asks
/// first, and Esc keeps the file that was there.
#[test]
fn f2_saves_the_history_owner_only_and_asks_before_replacing() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("save-history");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"echo SAVED_$((40+2))\r");
    xdevcmd.wait_for("SAVED_42");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(F2);
    xdevcmd.wait_for("Save as");
    xdevcmd.wait_for("xdevcmd-history-");
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("saved to");
    xdevcmd.wait_for("view history"); // back in the viewer
    let saved: Vec<PathBuf> = std::fs::read_dir(s.left())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().contains("xdevcmd-history-"))
        .collect();
    // Linux knows the shell's directory; elsewhere it is the file pane's, the same here.
    assert_eq!(saved.len(), 1, "{saved:?}");
    let text = std::fs::read_to_string(&saved[0]).unwrap();
    assert!(text.contains("SAVED_42\n"), "{text:?}");
    let mode = std::fs::metadata(&saved[0]).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    // The same name again: asked, and Esc keeps the first.
    let first = std::fs::read(&saved[0]).unwrap();
    xdevcmd.send(F2);
    xdevcmd.wait_for("Save as");
    xdevcmd.send(b"\x15"); // Ctrl+U clears the line
    xdevcmd.send(saved[0].to_string_lossy().as_bytes());
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("Already exists");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("Already exists");
    xdevcmd.wait_for("view history");
    assert_eq!(std::fs::read(&saved[0]).unwrap(), first, "kept as it was");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// A shell in a directory whose name is not UTF-8: Save as… cannot offer that name as
/// text, so it offers the file pane's directory instead of a mangled one.
#[test]
fn save_as_from_a_shell_in_a_non_utf8_directory_offers_the_panes() {
    let s = Scratch::new("save-non-utf8");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    // Some filesystems (APFS) refuse such a name; then the shell stays where it is and
    // the pane's directory is offered anyway.
    xdevcmd
        .send(b"mkdir \"$(printf 'odd\\377')\" && cd \"$(printf 'odd\\377')\"; echo IN_$((1+1))\r");
    xdevcmd.wait_for("IN_2");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(F2);
    xdevcmd.wait_for("Save as");
    let want = format!("left{}xdevcmd-history-", std::path::MAIN_SEPARATOR);
    xdevcmd.wait_for(&want);
    assert!(
        !xdevcmd.screen().contains('\u{fffd}'),
        "{}",
        xdevcmd.screen()
    );
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("Save as");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// F2 on a file in the viewer is an ordinary copy under the name given, and the viewer
/// comes back afterwards.
#[test]
fn f2_on_a_viewed_file_copies_it_under_the_new_name() {
    let s = Scratch::new("save-file");
    s.file("left/notes.txt", "keep these\n");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(DOWN);
    xdevcmd.send(b"\x1b[13~"); // F3
    xdevcmd.wait_for("keep these");
    xdevcmd.send(F2);
    xdevcmd.wait_for("Save as");
    xdevcmd.send(b"\x15");
    xdevcmd.send(s.right().join("copy.txt").to_string_lossy().as_bytes());
    xdevcmd.send(ENTER);
    wait_file(&s.right().join("copy.txt"), "the copy", |b| {
        b.as_deref() == Some("keep these\n")
    });
    // A clean copy closes its box by itself, and the viewer is back.
    xdevcmd.wait_for("view ");
    xdevcmd.wait_for("1 copied");
    xdevcmd.wait_for("keep these");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("keep these");
    xdevcmd.quit();
}

/// Read-only mode: the history can be read, not saved.
#[test]
fn read_only_mode_refuses_saving_the_history() {
    let s = Scratch::new("save-ro");
    let mut xdevcmd = Session::start_flags(&s, &["--read-only"]);
    xdevcmd.wait_for("READ-ONLY");
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(F2);
    xdevcmd.wait_for("creating files is turned off");
    assert!(!xdevcmd.screen().contains("Save as"));
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
    let left: Vec<_> = std::fs::read_dir(s.left()).unwrap().collect();
    assert!(left.is_empty(), "nothing written");
}

/// `[terminal] scrollback = 200`: a shell keeps 200 lines of history and its screen, so
/// `seq 1 500` loses its start, which the default 5000 keeps.
#[test]
fn a_shell_keeps_the_history_scrollback_says() {
    let s = Scratch::new("scrollback-200");
    let cfg = s.file("short.toml", "[terminal]\nscrollback = 200\n");
    let cfg = cfg.to_string_lossy().into_owned();
    let mut xdevcmd = Session::start_env(
        &s,
        None,
        &[s.left(), s.right()],
        &[("XDEVCMD_CONFIG", cfg.as_str())],
    );
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"seq 1 500\r");
    xdevcmd.wait_for("500");
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("in memory only");
    let screen = xdevcmd.screen();
    let n: usize = screen
        .split(" lines")
        .next()
        .and_then(|t| t.rsplit(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("a line count in the title:\n{screen}"));
    // 200 of history and the screen's rows (about ten here), no more.
    assert!((200..=240).contains(&n), "{n} lines\n{screen}");
    xdevcmd.send(b"\x1b[H");
    xdevcmd.wait_for("  1 \u{2502}");
    assert!(
        !xdevcmd.screen().contains("$ seq 1 500"),
        "the start is gone"
    );
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// The menu on a terminal tab acts on the tab clicked, not the one in focus: View history
/// shows the first tab's output while the second has focus, and Close tab closes the
/// first and leaves the second.
#[test]
fn the_tab_menu_acts_on_the_tab_clicked() {
    let s = Scratch::new("tab-menu");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"echo ONE_$((0+1))\r");
    xdevcmd.wait_for("ONE_1");
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.send(b"echo TWO_$((1+1))\r");
    xdevcmd.wait_for("TWO_2");

    let (x, y) = xdevcmd.locate("\u{2302} 1");
    xdevcmd.click(x + 1, y, 2);
    xdevcmd.wait_for("Terminal 1");
    let (hx, hy) = xdevcmd.locate("View history");
    xdevcmd.click(hx, hy, 0);
    xdevcmd.wait_for("history \u{b7} sh \u{b7} tab 1");
    xdevcmd.wait_for("ONE_1");
    assert!(!xdevcmd.screen().contains("TWO_2"), "{}", xdevcmd.screen());
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");

    let (x, y) = xdevcmd.locate("\u{2302} 1");
    xdevcmd.click(x + 1, y, 2);
    xdevcmd.wait_for("Close tab");
    let (cx, cy) = xdevcmd.locate("Close tab");
    xdevcmd.click(cx, cy, 0);
    xdevcmd.wait_for("terminal 1/1");
    xdevcmd.wait_for("TWO_2");
    assert!(!xdevcmd.screen().contains("ONE_1"), "{}", xdevcmd.screen());
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Shells are reaped every turn, also while a tab's menu is open: when an earlier tab
/// ends under the menu, the menu still acts on the tab it was opened on, which has moved
/// one place to the left, not on the tab that moved into its old place.
#[test]
fn the_tab_menu_follows_its_tab_when_an_earlier_one_ends() {
    let s = Scratch::new("tab-menu-reap");
    let go = s.0.join("GO");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    // Tab 1 ends when the test says so.
    let wait = format!(
        "while [ ! -e '{}' ]; do sleep 0.05; done; exit\r",
        go.display()
    );
    xdevcmd.send(wait.as_bytes());
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.send(b"echo TWO_$((1+1))\r");
    xdevcmd.wait_for("TWO_2");
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 3/3");
    xdevcmd.send(b"echo THREE_$((1+2))\r");
    xdevcmd.wait_for("THREE_3");

    let (x, y) = xdevcmd.locate("\u{2302} 2");
    xdevcmd.click(x + 1, y, 2);
    xdevcmd.wait_for("Terminal 2");
    std::fs::write(&go, "").unwrap();
    // Tab 1 is gone (the menu covers the bar; the status line counts the tabs).
    xdevcmd.wait_for("terminal 2/2");
    let (hx, hy) = xdevcmd.locate("View history");
    xdevcmd.click(hx, hy, 0);
    xdevcmd.wait_for("in memory only");
    xdevcmd.wait_for("TWO_2");
    assert!(
        !xdevcmd.screen().contains("THREE_3"),
        "{}",
        xdevcmd.screen()
    );
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Save history as… from the tab menu goes straight to the question; once saved, no
/// viewer is left behind.
#[test]
fn save_history_from_the_tab_menu_leaves_no_viewer() {
    let s = Scratch::new("tab-save");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"echo KEEP_$((2+3))\r");
    xdevcmd.wait_for("KEEP_5");
    let (x, y) = xdevcmd.locate("\u{2302} 1");
    xdevcmd.click(x + 1, y, 2);
    let (sx, sy) = xdevcmd.locate("Save history as");
    xdevcmd.click(sx, sy, 0);
    xdevcmd.wait_for("Save as");
    xdevcmd.send(b"\x15");
    xdevcmd.send(s.right().join("tab.txt").to_string_lossy().as_bytes());
    xdevcmd.send(ENTER);
    wait_file(&s.right().join("tab.txt"), "the saved history", |t| {
        t.is_some_and(|t| t.contains("KEEP_5"))
    });
    xdevcmd.wait_gone("Save as");
    assert!(
        !xdevcmd.screen().contains("view history"),
        "{}",
        xdevcmd.screen()
    );
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Save history as… from a tab's menu saves that tab's text, not the focused tab's.
#[test]
fn tab_menu_save_history_uses_clicked_tab() {
    let s = Scratch::new("tab-save-clicked");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"echo ONE_$((0+1))\r");
    xdevcmd.wait_for("ONE_1");
    xdevcmd.send(ALT_X);
    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.send(b"echo TWO_$((1+1))\r");
    xdevcmd.wait_for("TWO_2");
    let (x, y) = xdevcmd.locate("\u{2302} 1");
    xdevcmd.click(x + 1, y, 2);
    xdevcmd.wait_for("Terminal 1");
    let (sx, sy) = xdevcmd.locate("Save history as");
    xdevcmd.click(sx, sy, 0);
    xdevcmd.wait_for("Save as");
    let dest = s.right().join("first.txt");
    xdevcmd.send(b"\x15");
    xdevcmd.send(dest.to_string_lossy().as_bytes());
    xdevcmd.send(ENTER);
    wait_file(&dest, "tab 1's history", |t| {
        t.is_some_and(|t| t.contains("ONE_1"))
    });
    let text = std::fs::read_to_string(&dest).unwrap();
    assert!(
        !text.contains("TWO_2"),
        "the clicked tab's, not the focused one's: {text:?}"
    );
    xdevcmd.wait_gone("Save as");
    xdevcmd.send(F12);
    xdevcmd.quit();
}

/// Terminal.app sends Shift+F12 as `ESC [34~` (xterm's F20; crossterm numbers it F17),
/// and Alt+X F12 stands for Shift+F12 where neither arrives: both open the history.
#[test]
fn terminal_apps_shift_f12_and_alt_x_f12_open_the_history_too() {
    let s = Scratch::new("history-keys");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.wait_for("$ ");
    xdevcmd.send(b"\x1b[34~");
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    xdevcmd.send(ALT_X);
    xdevcmd.send(F12);
    xdevcmd.wait_for("in memory only");
    xdevcmd.send(b"\x1b");
    xdevcmd.wait_gone("view history");
    // From the files it opens the shell the panel last had in focus.
    xdevcmd.send(F12);
    xdevcmd.send(SHIFT_F12);
    xdevcmd.wait_for("history \u{b7} sh \u{b7} tab 1");
    xdevcmd.send(b"\x1b");
    xdevcmd.quit();
}

/// Reported from Windows: closing the window instead of pressing q lost the session,
/// since it was written only on quit and a closed window kills xdevcmd without asking.
/// It is saved when it changes now, so a kill keeps what was open.
#[test]
fn a_killed_xdevcmd_still_reopens_where_it_was() {
    let s = Scratch::new("killed");
    std::fs::create_dir(s.left().join("deeper")).unwrap();
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(b"\x14"); // Ctrl+T: a second tab
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER); // into deeper
    xdevcmd.wait_for(" 2 deeper ");
    // Autosave runs every 5 s by default: allow it two rounds.
    let file = s.0.join("session.toml");
    let start = Instant::now();
    while !std::fs::read_to_string(&file).is_ok_and(|t| t.contains("deeper")) {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "saved without quitting"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = xdevcmd.child.kill();
    drop(xdevcmd);

    let again = Session::start_with(&s, None, &[]);
    again.wait_for(" 2 deeper ");
    again.quit();
}

/// What xdevcmd writes while starting, in an environment that looks like this terminal.
fn first_frame_bytes(tag: &str, env: &[(&str, &str)]) -> String {
    let s = Scratch::new(tag);
    std::fs::write(s.left().join("a.txt"), "a").unwrap();
    let session = Session::start_env(&s, None, &[s.left(), s.right()], env);
    // Let the shell pane print its prompt too: its colours go the same way.
    std::thread::sleep(Duration::from_millis(300));
    let bytes = String::from_utf8_lossy(&session.raw.lock().unwrap()).into_owned();
    session.quit();
    bytes
}

#[test]
fn an_old_terminal_app_is_never_sent_24_bit_colour() {
    // Terminal.app before macOS 26 reads 48;2;R;G;B as five attributes: the cursor
    // row (28, 42, 58) came out with a green background. COLORTERM is set to show it
    // does not win: people export it in their dotfiles.
    let old = [
        ("TERM_PROGRAM", "Apple_Terminal"),
        ("TERM_PROGRAM_VERSION", "455.1"),
        ("COLORTERM", "truecolor"),
        ("XDEVCMD_COLORS", ""),
    ];
    let bytes = first_frame_bytes("old-terminal-app", &old);
    assert!(
        !bytes.contains("38;2;") && !bytes.contains("48;2;"),
        "24-bit colour sent to an old Terminal.app"
    );
    assert!(
        bytes.contains("48;5;"),
        "and the 256-colour palette used instead"
    );

    // The same start in a terminal that says it takes 24-bit colour gets it.
    let new = [
        ("TERM_PROGRAM", "iTerm.app"),
        ("COLORTERM", "truecolor"),
        ("XDEVCMD_COLORS", ""),
    ];
    assert!(first_frame_bytes("truecolor-terminal", &new).contains("48;2;"));
}

#[test]
fn xdevcmd_colors_overrides_what_the_terminal_seems_to_be() {
    let env = [
        ("TERM_PROGRAM", "iTerm.app"),
        ("COLORTERM", "truecolor"),
        ("XDEVCMD_COLORS", "16"),
    ];
    let bytes = first_frame_bytes("xdevcmd-colors-16", &env);
    assert!(!bytes.contains("38;2;") && !bytes.contains("48;2;"));
    // crossterm sends even the 16 ANSI colours as `38;5;N`; at 16 colours N is the
    // user's own palette, 0 to 15, and nothing above.
    let mut seen = 0;
    for part in bytes.split(";5;").skip(1) {
        let n: u32 = part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        assert!(n < 16, "palette entry {n} sent to a 16-colour terminal");
        seen += 1;
    }
    assert!(seen > 0, "some colour was sent");
}

#[test]
fn a_program_run_in_a_terminal_tab_keeps_its_output_after_it_ends() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("run-in-tab");
    s.file("left/hello.sh", "#!/bin/sh\necho hello from the tab\n");
    let script = s.left().join("hello.sh");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("Run it?");
    // Cancel is first; the terminal tab is the next item.
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("hello from the tab");
    xdevcmd.wait_for("hello.sh ended (exit 0)");
    // Enter closes the finished program's tab, and nothing else.
    xdevcmd.send(ENTER);
    xdevcmd.wait_gone("hello from the tab");
    xdevcmd.quit();
}

#[cfg(unix)]
#[test]
fn alt_c_does_not_type_into_a_program_run_in_a_terminal_tab() {
    // A program tab has no shell: its process is the program, which once made it look
    // like a shell at its prompt, so Alt+C typed a cd into it.
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("busy-tab");
    s.file(
        "left/ask.sh",
        "#!/bin/sh\necho waiting\nread x\necho \"GOT:$x\" > typed.txt\n",
    );
    let script = s.left().join("ask.sh");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("Run it?");
    xdevcmd.send(DOWN);
    xdevcmd.send(ENTER);
    xdevcmd.wait_for("waiting");
    xdevcmd.send(F12);
    xdevcmd.send(ALT_C);
    xdevcmd.wait_for("ask.sh is running in the terminal");
    xdevcmd.send(F12);
    xdevcmd.send(b"done\r");
    xdevcmd.wait_for("ask.sh ended");
    let typed = std::fs::read_to_string(s.left().join("typed.txt")).unwrap_or_default();
    assert_eq!(typed.trim(), "GOT:done", "the cd went into the program");
    xdevcmd.send(ENTER);
    xdevcmd.quit();
}

#[test]
fn show_keys_prints_each_key_and_click_and_what_it_would_do() {
    let s = Scratch::new("show-keys");
    let mut xdevcmd = Session::start_flags(&s, &["--show-keys"]);
    xdevcmd.send(b"\x1b[18~"); // F7
    xdevcmd.wait_for("F7");
    xdevcmd.wait_for("mkdir");
    xdevcmd.send(b"\x1bj"); // Alt+J
    xdevcmd.wait_for("history");
    xdevcmd.mouse(0, 9, 4, false);
    xdevcmd.wait_for("Down(Left) at column 9, row 4");
    // Esc twice ends it, as the first line says.
    xdevcmd.send(b"\x1b");
    xdevcmd.send(b"\x1b");
    let start = Instant::now();
    loop {
        match xdevcmd.child.try_wait() {
            Ok(Some(st)) => {
                assert!(st.success(), "{st:?}");
                break;
            }
            _ if start.elapsed() > PATIENCE => panic!("--show-keys did not end on Esc Esc"),
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}
