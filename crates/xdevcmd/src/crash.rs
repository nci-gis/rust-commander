//! When xdevcmd panics: what happened, written to a file the user can attach to a bug
//! report, since the terminal's scrollback is gone with the alternate screen. Nothing is
//! sent anywhere.
//!
//! A panic on the main thread ends xdevcmd: the terminal is restored and the user told.
//! One on another thread (a worker, a pty reader, a listing) does not end the process,
//! so the main loop goes on drawing: restoring the terminal under it, or printing std's
//! message over the screen, would leave it running in a broken terminal. That panic is
//! logged and the main loop says so on the status line.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;

static MAIN: OnceLock<ThreadId> = OnceLock::new();

/// Called first thing on the main thread: the hook tells it from the others.
pub fn mark_main() {
    let _ = MAIN.set(std::thread::current().id());
}

/// Whether this is the main thread (or nothing was marked, which errs towards ending).
pub fn on_main() -> bool {
    MAIN.get().is_none_or(|m| *m == std::thread::current().id())
}

/// What the panic hook does, by where the panic happened.
#[derive(Debug, PartialEq, Eq)]
pub struct Steps {
    /// Turn mouse reporting off and hand on to the previous hook (ratatui's, which
    /// restores the terminal, then std's, which prints the message).
    pub restore: bool,
    /// Say on the restored terminal where the log is.
    pub tell: bool,
    /// Leave the log's path for the main loop to show.
    pub note: bool,
}

/// The decision, kept apart from the hook so it can be tested.
pub fn on_panic(main: bool) -> Steps {
    Steps {
        restore: main,
        tell: main,
        note: !main,
    }
}

/// A background panic waiting to be shown: the log, if one could be written.
static BACKGROUND: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// From the hook, on a background thread.
pub fn note_background(log: Option<PathBuf>) {
    if let Ok(mut b) = BACKGROUND.lock() {
        *b = Some(log);
    }
}

/// For the main loop: a background panic since the last call, and its log.
pub fn take_background() -> Option<Option<PathBuf>> {
    BACKGROUND.lock().ok()?.take()
}

/// Where the issues are reported.
pub const ISSUES: &str = "https://github.com/nci-gis/rust-commander/issues";

/// How many crash logs are kept: the newest few, so they never pile up.
const KEEP: usize = 5;

/// Write `info` with the version, system and a backtrace to
/// `<state directory>/crash-<time>.log`; the path, if it could be written. `main`: the
/// panic ends xdevcmd; otherwise it was a background thread's and xdevcmd went on.
pub fn write(info: &std::panic::PanicHookInfo<'_>, main: bool) -> Option<PathBuf> {
    let dir = crate::session::default_path()?.parent()?.to_path_buf();
    write_in(&dir, &report(info, main))
}

fn report(info: &std::panic::PanicHookInfo<'_>, main: bool) -> String {
    let mut s = String::new();
    if main {
        let _ = writeln!(s, "xdevcmd {} crashed", env!("CARGO_PKG_VERSION"));
    } else {
        let _ = writeln!(
            s,
            "xdevcmd {}: a background thread panicked; xdevcmd went on",
            env!("CARGO_PKG_VERSION")
        );
    }
    let _ = writeln!(
        s,
        "system: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(s, "time: {}", chrono_free_now());
    let _ = writeln!(
        s,
        "thread: {}",
        std::thread::current().name().unwrap_or("?")
    );
    let _ = writeln!(s, "\n{info}\n");
    let _ = writeln!(s, "{}", std::backtrace::Backtrace::force_capture());
    debug_tail(&mut s, xdevcmd_core::debuglog::tail());
    s
}

/// With the debug log on: where it is and its last lines, so the report says what
/// xdevcmd was doing (they hold no names).
fn debug_tail(s: &mut String, tail: Option<(PathBuf, Vec<String>)>) {
    let Some((path, lines)) = tail else { return };
    let _ = writeln!(s, "\ndebug log: {}", path.display());
    let _ = writeln!(s, "its last {} lines:", lines.len());
    for l in lines {
        let _ = writeln!(s, "{l}");
    }
}

/// Seconds since the epoch: no date crate here, and it only orders the files.
fn chrono_free_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The log is the user's alone (`0600` on Unix): a panic's message and backtrace name
/// files and the home directory. The home directory is written `~`, so a log attached to
/// a report does not carry the user's name in every path.
fn write_in(dir: &Path, text: &str) -> Option<PathBuf> {
    use std::io::Write as _;
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!(
        "crash-{}-{}.log",
        chrono_free_now(),
        std::process::id()
    ));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let text = homes_as_tilde(text, &homes(), cfg!(windows));
    o.open(&path).ok()?.write_all(text.as_bytes()).ok()?;
    prune(dir);
    Some(path)
}

/// Every home folder set (on Windows `HOME` and `USERPROFILE` may both be, and
/// differ), as the rest of xdevcmd finds them.
fn homes() -> Vec<String> {
    xdevcmd_core::dirs::homes()
        .iter()
        .map(|h| h.to_string_lossy().into_owned())
        .collect()
}

/// `text` with every mention of each of `homes` written `~`, the longest first so one
/// inside another is not cut in two. `fold`: letters' case does not matter (Windows).
/// What it covers: each home as written, with `/` or `\\` between its names, followed
/// by a separator or the end of a path. Not a home spelled another way (a short 8.3
/// name, an MSYS `/c/Users/…` for `C:\\Users\\…`).
fn homes_as_tilde(text: &str, homes: &[String], fold: bool) -> String {
    let mut by_length: Vec<&String> = homes.iter().collect();
    by_length.sort_by_key(|h| std::cmp::Reverse(h.len()));
    by_length
        .into_iter()
        .fold(text.to_string(), |t, h| home_as_tilde(&t, Some(h), fold))
}

/// `text` with every mention of `home` written `~`: as written and with the other
/// slash, only where a separator or the end of a path follows it (so `/home/anna` is
/// not `/home/ann` + `a`). A home that names no user (`/`, a drive's root `C:\\`) is
/// left alone: it would turn every path into `~`.
fn home_as_tilde(text: &str, home: Option<&str>, fold: bool) -> String {
    let Some(h) = home.map(|h| h.trim_end_matches(['/', '\\'])) else {
        return text.to_string();
    };
    let drive_root = h.len() == 2 && h.ends_with(':');
    if h.len() <= 1 || drive_root {
        return text.to_string();
    }
    let mut out = text.to_string();
    for form in [h.to_string(), h.replace('\\', "/"), h.replace('/', "\\")] {
        out = replace_home(&out, &form, fold);
    }
    out
}

/// Each `home` in `text` followed by a separator or a path's end, written `~`.
fn replace_home(text: &str, home: &str, fold: bool) -> String {
    let key = |s: &str| {
        if fold {
            s.to_ascii_lowercase()
        } else {
            s.to_string()
        }
    };
    // ASCII case folding keeps every byte where it was, so offsets carry over.
    let (hay, needle) = (key(text), key(home));
    let ends_path = |c: Option<char>| {
        c.is_none_or(|c| {
            matches!(c, '/' | '\\' | ':' | '"' | '\'' | ')' | ']') || c.is_whitespace()
        })
    };
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    while let Some(at) = hay[from..].find(&needle).map(|i| from + i) {
        let end = at + needle.len();
        if ends_path(text[end..].chars().next()) {
            out.push_str(&text[from..at]);
            out.push('~');
        } else {
            out.push_str(&text[from..end]);
        }
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// Keep only the newest [`KEEP`] crash logs.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".log"))
        })
        .collect();
    logs.sort();
    let extra = logs.len().saturating_sub(KEEP);
    for p in logs.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

/// Say where the log is and how to report it, once the terminal is restored.
pub fn tell(log: Option<&Path>) {
    match log {
        Some(p) => eprintln!(
            "\nxdevcmd crashed. The details are in\n  {}\nPlease report it at {ISSUES} and attach that file \
             (read it first: it may hold file names).",
            p.display()
        ),
        None => eprintln!("\nxdevcmd crashed. Please report it at {ISSUES} with the message above."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_main_thread_panic_touches_the_terminal() {
        assert_eq!(
            on_panic(true),
            Steps {
                restore: true,
                tell: true,
                note: false
            }
        );
        assert_eq!(
            on_panic(false),
            Steps {
                restore: false,
                tell: false,
                note: true
            }
        );
    }

    #[test]
    fn a_crash_log_ends_with_the_debug_logs_last_lines() {
        let mut s = String::from("crashed\n");
        debug_tail(&mut s, None);
        assert_eq!(s, "crashed\n");
        debug_tail(
            &mut s,
            Some((
                PathBuf::from("/s/debug-1-2.jsonl"),
                vec!["{\"ev\":\"batch\"}".into()],
            )),
        );
        assert!(
            s.contains("debug log: /s/debug-1-2.jsonl\nits last 1 lines:\n{\"ev\":\"batch\"}\n")
        );
    }

    #[test]
    fn crash_logs_are_written_and_only_the_newest_kept() {
        let d = std::env::temp_dir().join(format!("xdevcmd-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for i in 0..7 {
            std::fs::write(d.join(format!("crash-{}-1.log", 1000 + i)), "old").unwrap();
        }
        let p = write_in(&d, "xdevcmd 0.0.0 crashed\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "xdevcmd 0.0.0 crashed\n"
        );
        let left = std::fs::read_dir(&d).unwrap().count();
        assert_eq!(left, KEEP, "only the newest {KEEP}");
        assert!(p.exists(), "the new one is among them");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "the user's alone");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn every_home_is_written_as_a_tilde() {
        // Windows with Git Bash: HOME and USERPROFILE both set, and different. A path
        // under either keeps no user name (beta.3 review: only HOME was hidden).
        let text = r"at C:\Users\ann\src\x.rs and /c/msys/home/ann/y.rs";
        let homes = [r"C:\Users\ann".to_string(), "/c/msys/home/ann".to_string()];
        assert_eq!(
            homes_as_tilde(text, &homes, false),
            r"at ~\src\x.rs and ~/y.rs"
        );
        // One home inside the other: the longer one is replaced whole.
        let nested = ["/home/ann".to_string(), "/home/ann/win".to_string()];
        assert_eq!(homes_as_tilde("/home/ann/win/z", &nested, false), "~/z");
        assert_eq!(homes_as_tilde("/x", &[], false), "/x");
    }

    /// Beta.3 review of the fix above: a home is a whole folder name, in either slash,
    /// in any case on Windows; a drive's root is no home.
    #[test]
    fn a_home_is_matched_as_a_whole_folder_in_any_spelling_windows_allows() {
        let win = [r"C:\Users\Ann".to_string()];
        assert_eq!(homes_as_tilde(r"c:\users\ann\x", &win, true), r"~\x");
        assert_eq!(homes_as_tilde("C:/Users/Ann/z", &win, true), "~/z");
        assert_eq!(homes_as_tilde("at C:/Users/Ann", &win, true), "at ~");
        // Case matters elsewhere.
        assert_eq!(
            homes_as_tilde("/Home/Ann/x", &["/home/ann".into()], false),
            "/Home/Ann/x"
        );
        // Another user whose name starts with this one's is not touched.
        let unix = ["/home/ann".to_string()];
        assert_eq!(
            homes_as_tilde("/home/anna/x /home/ann/y", &unix, false),
            "/home/anna/x ~/y"
        );
        assert_eq!(homes_as_tilde("in /home/ann: x", &unix, false), "in ~: x");
        // A home that is a drive's root names nobody.
        assert_eq!(
            homes_as_tilde(r"C:\x and C:\y", &[r"C:\".into()], true),
            r"C:\x and C:\y"
        );
    }

    #[test]
    fn the_home_directory_is_written_as_a_tilde() {
        let text = "panicked at /home/ann/.cargo/registry/src/x.rs:1\n  in /home/ann/work\n";
        assert_eq!(
            home_as_tilde(text, Some("/home/ann"), false),
            "panicked at ~/.cargo/registry/src/x.rs:1\n  in ~/work\n"
        );
        assert_eq!(
            home_as_tilde(text, Some("/home/ann/"), false),
            home_as_tilde(text, Some("/home/ann"), false)
        );
        assert_eq!(
            home_as_tilde("/x", Some("/"), false),
            "/x",
            "a home of / says nothing"
        );
        assert_eq!(home_as_tilde("/x", None, false), "/x");
        assert_eq!(
            home_as_tilde(r"C:\Users\ann\src\x.rs", Some(r"C:\Users\ann"), false),
            r"~\src\x.rs"
        );
    }
}
