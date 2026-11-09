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
    s
}

/// Seconds since the epoch: no date crate here, and it only orders the files.
fn chrono_free_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn write_in(dir: &Path, text: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!(
        "crash-{}-{}.log",
        chrono_free_now(),
        std::process::id()
    ));
    std::fs::write(&path, text).ok()?;
    prune(dir);
    Some(path)
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
        let _ = std::fs::remove_dir_all(&d);
    }
}
