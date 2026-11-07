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
        cmd.env_remove("VISUAL");
        cmd.env_remove("SSH_CONNECTION");
        cmd.env_remove("SSH_TTY");
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
        {
            let parser = parser.clone();
            let raw = raw.clone();
            let writer = writer.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 16384];
                let mut answer = answer;
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 {
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
        // Keep the master alive for the session: dropping it hangs up the child.
        std::mem::forget(pair.master);

        let session = Self {
            parser,
            raw,
            writer,
            child,
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
    wait_bytes(&x, "\x1b_Ga=T,f=24,", 1);
    // Drawn once: frames that change nothing do not send it again.
    std::thread::sleep(Duration::from_millis(1500));
    wait_bytes(&x, "\x1b_Ga=T,f=24,", 1);
    let sent = String::from_utf8_lossy(&x.raw.lock().unwrap())
        .matches("\x1b_Ga=T,")
        .count();
    assert_eq!(sent, 1, "sent again with nothing changed");
    // A dialog over it: the picture goes (it would be drawn over the dialog)...
    x.send(b"\x1bOP"); // F1, help
    wait_bytes(&x, "\x1b_Ga=d,d=I,", 1);
    // ...and comes back when the dialog closes.
    x.send(b"\x1b");
    wait_bytes(&x, "\x1b_Ga=T,f=24,", 2);
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
        let mut x = Session::start_env(
            &s,
            None,
            &[s.left(), s.right()],
            &[("XDEVCMD_CONFIG", &cfg)],
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
const ALT_S: &[u8] = b"\x1bs";
const CTRL_PAGE_UP: &[u8] = b"\x1b[5;5~";

#[test]
fn each_terminal_tab_is_its_own_shell() {
    let s = Scratch::new("tabs");
    let mut xdevcmd = Session::start(&s, None);
    xdevcmd.send(F12);
    xdevcmd.send(b"echo ONE_$((0+1))\r");
    xdevcmd.wait_for("ONE_1");

    xdevcmd.send(ALT_N);
    xdevcmd.wait_for("terminal 2/2");
    xdevcmd.wait_gone("ONE_1");
    xdevcmd.send(b"echo TWO_$((1+1))\r");
    xdevcmd.wait_for("TWO_2");

    // Back to the first tab: its own screen, untouched by what ran in the second.
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
