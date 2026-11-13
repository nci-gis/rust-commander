//! `o`: open a file with whatever the system would open it with.
//!
//! The program is started with no terminal — stdin, stdout and stderr all null — so
//! nothing it does can scribble over xdevcmd's screen or read its keys, and it is not
//! waited for: a PDF viewer you leave open must not freeze the file manager. A thread
//! waits instead and reports a failure as a line of text.
//!
//! No shell is ever involved, so a file name is always one argument and never a
//! command. On Windows that rules out `cmd /c start`, which re-parses its command line
//! and would run `a&calc.exe`; `explorer.exe` with the path quoted is used instead.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Whether opening `path` runs it: a program, a script, a shortcut, an installer, a
/// disk image. Opening one of these is "run it", and that is asked before it happens.
/// The lists are of what each system's own "open" turns into running something; they
/// cannot be complete, which is why Enter opens without asking only
/// [`document`]s.
pub fn runs_something(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    #[cfg(windows)]
    {
        const RUN: &[&str] = &[
            "exe",
            "com",
            "bat",
            "cmd",
            "ps1",
            "psm1",
            "psd1",
            "ps1xml",
            "psc1",
            "vbs",
            "vbe",
            "js",
            "jse",
            "wsf",
            "wsh",
            "ws",
            "wsc",
            "sct",
            "msi",
            "msp",
            "msc",
            "lnk",
            "scr",
            "pif",
            "cpl",
            "hta",
            "reg",
            "url",
            "inf",
            "py",
            "pyw",
            "pyz",
            "pyzw",
            "jar",
            "jnlp",
            "appref-ms",
            "application",
            "xbap",
            "chm",
            "settingcontent-ms",
            "library-ms",
            "search-ms",
            "diagcab",
            "appx",
            "appxbundle",
            "msix",
            "msixbundle",
            "appinstaller",
            "iso",
            "img",
            "vhd",
            "vhdx",
            "website",
            "scf",
            "gadget",
            "xll",
        ];
        RUN.contains(&ext.as_str())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let exec = std::fs::metadata(path)
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
        // Launchers and what a desktop hands to an interpreter it may have: a .desktop
        // file's command, `java -jar`, Wine for Windows programs, an AppImage; on macOS
        // a .terminal file's command, a .command script, an Automator workflow.
        const RUN: &[&str] = &[
            "desktop",
            "jar",
            "jnlp",
            "exe",
            "msi",
            "com",
            "bat",
            "appimage",
            "run",
            "terminal",
            "command",
            "tool",
            "workflow",
            "action",
            "fileloc",
            "inetloc",
            "scpt",
            "scptd",
            "applescript",
            "pkg",
            "mpkg",
        ];
        exec || RUN.contains(&ext.as_str())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, ext);
        true
    }
}

/// Whether `path` is a document that the system opens in a viewer or an editor, and so
/// Enter opens without asking: pictures, sound, video, PDF, text, and office files of
/// the kinds that cannot hold macros. Anything else asks first, since what "open" does
/// with an unknown type is the system's choice, and on every system some types run
/// code. Left out on purpose: web pages and SVG (scripts in the browser, and requests
/// that tell someone the file was opened), XML (Windows hands some to Office), the
/// older .doc/.xls/.ppt and OpenDocument files (macros).
pub fn document(path: &Path) -> bool {
    const DOCS: &[&str] = &[
        // text and data
        "txt", "md", "markdown", "rst", "log", "csv", "tsv", "json", "yaml", "yml", "toml", "ini",
        "cfg", "conf", // pages
        "pdf", "epub", "djvu", "rtf", // office files that cannot hold macros
        "docx", "xlsx", "pptx", "pages", "numbers", "key", // pictures
        "png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff", "ico", "heic", "heif", "avif",
        "psd", "xcf", "raw", "cr2", "nef", "dng", // sound and video
        "mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac", "wma", "mid", "midi", "mp4",
        "m4v", "mkv", "webm", "mov", "avi", "wmv", "mpg", "mpeg", "3gp", "flv", // fonts
        "ttf", "otf", "woff", "woff2",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| DOCS.contains(&e.to_ascii_lowercase().as_str()))
        && !runs_something(path)
}

/// Why the system's program cannot open anything from here; `None` when it can. The
/// one place that decides, for `o`, Enter and the `[[open]]` rules.
///
/// Over SSH, on any system, there is no screen in front of the user: `open` and
/// `explorer.exe` would show the file on the remote machine's own screen, or fail. On
/// Linux and the BSDs without a display, xdg-open falls back to a browser — possibly a
/// text one, inside this very terminal.
pub fn no_screen() -> Option<String> {
    let set = |v: &str| std::env::var_os(v).is_some_and(|s| !s.is_empty());
    if set("SSH_CONNECTION") || set("SSH_TTY") {
        return Some("over SSH there is no screen here to open it on".into());
    }
    if cfg!(all(unix, not(target_os = "macos"))) && !set("DISPLAY") && !set("WAYLAND_DISPLAY") {
        return Some("no desktop session to open it in (no DISPLAY)".into());
    }
    None
}

/// Whether `path` is a program that runs in a terminal: something "Run it?" can start
/// in a terminal tab, by its path alone, with its output on screen.
pub fn console_program(path: &Path) -> bool {
    #[cfg(windows)]
    {
        path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "exe" | "com" | "bat" | "cmd"
            )
        })
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let desktop = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("desktop"));
        !desktop
            && std::fs::metadata(path)
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

/// The command that opens `path`: the configured one, or the platform's own. `Err`
/// says why nothing can be opened here.
pub fn command(path: &Path, configured: &str) -> Result<Command, String> {
    let mut parts = configured.split_whitespace();
    if let Some(program) = parts.next() {
        let mut c = Command::new(program);
        c.args(parts).arg(path);
        return Ok(c);
    }
    platform(path)
}

fn platform(path: &Path) -> Result<Command, String> {
    if let Some(why) = no_screen() {
        return Err(format!("{why} \u{2014} set [commands] opener"));
    }
    system(path)
}

#[cfg(target_os = "macos")]
fn system(path: &Path) -> Result<Command, String> {
    let mut c = Command::new("open");
    c.arg("--").arg(path);
    Ok(c)
}

#[cfg(windows)]
fn system(path: &Path) -> Result<Command, String> {
    use std::os::windows::process::CommandExt;
    // explorer.exe splits its arguments on commas unless the path is quoted, and a
    // Windows path can never contain a double quote, so quoting it is always safe.
    let mut c = Command::new("explorer.exe");
    c.raw_arg(format!("\"{}\"", path.display()));
    Ok(c)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system(path: &Path) -> Result<Command, String> {
    let mut c = Command::new("xdg-open");
    c.arg(path);
    Ok(c)
}

/// After this long without an answer from the opener, the status line says
/// "opening …" (item 45): a quick open shows no flicker.
const SHOW_AFTER: Duration = Duration::from_millis(300);
/// Some openers wait for the program they started (xdg-open with some desktops): after
/// this long the file counts as handed over, and only a failure is reported after it.
const GIVE_UP: Duration = Duration::from_secs(5);

/// Start `cmd` detached from the terminal. Nothing is said at once; on `report` come
/// "opening {what}…" if the opener is still running after 300 ms, then "opened {what}"
/// (`done` is the whole message, for "opened x with y") when it hands the file over,
/// "handed {what} to {program}" if it is still running after 5 s, or its error.
pub fn spawn(
    cmd: Command,
    what: String,
    done: String,
    report: Sender<String>,
) -> Result<(), String> {
    spawn_timed(cmd, what, done, report, SHOW_AFTER, GIVE_UP)
}

fn spawn_timed(
    mut cmd: Command,
    what: String,
    done: String,
    report: Sender<String>,
    show_after: Duration,
    give_up: Duration,
) -> Result<(), String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut said_opening = false;
        let mut handed = false;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // explorer.exe exits 1 even when it worked, so its status means
                    // nothing: it took the file.
                    let ok = status.success() || program.eq_ignore_ascii_case("explorer.exe");
                    if !ok {
                        let _ = report.send(format!("{program} could not open {what} ({status})"));
                    } else if !handed {
                        // After "handed", a later success adds nothing.
                        let _ = report.send(done);
                    }
                    return;
                }
                Ok(None) => {}
                Err(_) => return,
            }
            let took = started.elapsed();
            if !handed && took >= give_up {
                handed = true;
                let _ = report.send(format!("handed {what} to {program}"));
            } else if !said_opening && took >= show_after {
                said_opening = true;
                let _ = report.send(format!("opening {what}\u{2026}"));
            }
            // Fine-grained while someone may be watching the line, then rarely: this
            // thread lives as long as the opener does.
            std::thread::sleep(if handed {
                Duration::from_secs(1)
            } else {
                Duration::from_millis(25)
            });
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 45: what the status line hears from an opener, in order.
    #[cfg(unix)]
    fn heard(script: &str, give_up_ms: u64) -> Vec<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        spawn_timed(
            c,
            "a.pdf".into(),
            "opened a.pdf".into(),
            tx,
            Duration::from_millis(100),
            Duration::from_millis(give_up_ms),
        )
        .unwrap();
        // The thread ends with the opener (or after "handed", with it too), and with
        // it the sender: the list is complete.
        rx.iter().collect()
    }

    #[cfg(unix)]
    #[test]
    fn a_quick_open_says_opened_only_and_a_slow_one_says_opening_first() {
        assert_eq!(heard("exit 0", 2000), ["opened a.pdf"]);
        assert_eq!(
            heard("sleep 0.4", 2000),
            ["opening a.pdf\u{2026}", "opened a.pdf"]
        );
        let failed = heard("exit 3", 2000);
        assert_eq!(failed.len(), 1);
        assert!(
            failed[0].starts_with("sh could not open a.pdf"),
            "{failed:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_opener_that_waits_for_its_program_is_handed_over_and_a_late_failure_still_said() {
        assert_eq!(
            heard("sleep 1.2", 400),
            ["opening a.pdf\u{2026}", "handed a.pdf to sh"]
        );
        let late = heard("sleep 1.2; exit 4", 400);
        assert_eq!(late.len(), 3, "{late:?}");
        assert!(late[2].starts_with("sh could not open"), "{late:?}");
    }

    #[test]
    fn a_configured_opener_gets_the_path_as_one_argument() {
        let p = Path::new("/tmp/a b&c;rm -rf ~.txt");
        let c = command(p, "my-viewer --flag").unwrap();
        assert_eq!(c.get_program(), "my-viewer");
        let args: Vec<_> = c.get_args().collect();
        assert_eq!(args, vec![std::ffi::OsStr::new("--flag"), p.as_os_str()]);
    }

    #[cfg(unix)]
    #[test]
    fn executables_and_launchers_count_as_running_something() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("xdevcmd-opener-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (plain, script, desktop) = (d.join("notes.txt"), d.join("run.sh"), d.join("x.desktop"));
        for f in [&plain, &script, &desktop] {
            std::fs::write(f, "x").unwrap();
        }
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!runs_something(&plain));
        assert!(runs_something(&script));
        assert!(runs_something(&desktop));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_opener_is_reported_not_silent() {
        let (tx, rx) = std::sync::mpsc::channel();
        let c = command(Path::new("/nonexistent"), "false").unwrap();
        spawn(c, "it".into(), "opened it".into(), tx).unwrap();
        let msg = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(msg.contains("false could not open it"), "{msg}");
    }
}
