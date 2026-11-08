//! When xdevcmd panics: what happened, written to a file the user can attach to a bug
//! report, since the terminal's scrollback is gone with the alternate screen. Nothing is
//! sent anywhere.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where the issues are reported.
pub const ISSUES: &str = "https://github.com/nci-gis/rust-commander/issues";

/// How many crash logs are kept: the newest few, so they never pile up.
const KEEP: usize = 5;

/// Write `info` with the version, system and a backtrace to
/// `<state directory>/crash-<time>.log`; the path, if it could be written.
pub fn write(info: &std::panic::PanicHookInfo<'_>) -> Option<PathBuf> {
    let dir = crate::session::default_path()?.parent()?.to_path_buf();
    write_in(&dir, &report(info))
}

fn report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "xdevcmd {} crashed", env!("CARGO_PKG_VERSION"));
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
