//! rcmd's own temporary files, kept track of so none stays behind.
//!
//! Everything rcmd writes to the system's temporary directory — a file viewed from
//! inside an archive, the list of names for renaming many in an editor — goes in one
//! directory for the run, `rcmd-run-<pid>`, readable by the user only. While rcmd runs
//! it refreshes a marker file there every few minutes; when rcmd ends it removes the
//! directory; and when rcmd starts it removes the directories of runs that ended
//! without doing so (killed, crashed, the power cut), recognised by a marker not
//! refreshed for an hour. Only directories carrying the marker are ever removed, and
//! the two names older versions used, so nothing of anyone else's is touched.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const RUN_PREFIX: &str = "rcmd-run-";
const MARKER: &str = ".rcmd-alive";
/// A run whose marker is older than this has ended.
const STALE: Duration = Duration::from_secs(60 * 60);
/// How often a running rcmd refreshes its marker: well inside `STALE`.
const TOUCH_EVERY: Duration = Duration::from_secs(5 * 60);

static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static TOUCHED: Mutex<Option<Instant>> = Mutex::new(None);

/// This run's directory, made on first use. `None` if it cannot be made.
pub fn dir() -> Option<PathBuf> {
    DIR.get_or_init(|| make_in(&std::env::temp_dir())).clone()
}

/// A path for `name` in this run's directory.
pub fn file(name: &str) -> Option<PathBuf> {
    dir().map(|d| d.join(name))
}

/// Make a fresh run directory under `base`. `create` fails on anything already there,
/// a symlink included, so the directory is always one this run made itself.
fn make_in(base: &Path) -> Option<PathBuf> {
    let pid = std::process::id();
    for k in 0..16 {
        let name = if k == 0 {
            format!("{RUN_PREFIX}{pid}")
        } else {
            format!("{RUN_PREFIX}{pid}-{k}")
        };
        let d = base.join(name);
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        match b.create(&d) {
            Ok(()) => {
                let _ = std::fs::write(d.join(MARKER), b"");
                return Some(d);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// Refresh the marker, at most every `TOUCH_EVERY`; called from the event loop.
pub fn touch() {
    let Some(Some(d)) = DIR.get() else {
        return;
    };
    let Ok(mut last) = TOUCHED.lock() else {
        return;
    };
    if last.is_some_and(|t| t.elapsed() < TOUCH_EVERY) {
        return;
    }
    *last = Some(Instant::now());
    let _ = std::fs::write(d.join(MARKER), b"");
}

/// Remove this run's directory: rcmd is ending.
pub fn remove() {
    if let Some(Some(d)) = DIR.get() {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Remove what ended runs left in the system's temporary directory.
pub fn sweep() -> usize {
    let own = DIR.get().cloned().flatten();
    sweep_in(&std::env::temp_dir(), own.as_deref(), SystemTime::now())
}

/// What `sweep` does, in `base`, as of `now`, leaving `own` alone.
fn sweep_in(base: &Path, own: Option<&Path>, now: SystemTime) -> usize {
    let age = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
    };
    let stale = |p: &Path| age(p).is_some_and(|a| a > STALE);
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(base) else {
        return 0;
    };
    for e in entries.flatten() {
        let path = e.path();
        if own == Some(path.as_path()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        let gone = if name.starts_with(RUN_PREFIX) && is_dir {
            // Only a directory with the marker is a run of ours.
            let marker = path.join(MARKER);
            marker.exists() && stale(&marker)
        } else if (is_dir && name.starts_with("rcmd-view-"))
            || (!is_dir && name.starts_with("rcmd-rename-") && name.ends_with(".txt"))
        {
            // The names rcmd used before this module, for a file viewed from an
            // archive and for the list of names: left by a crash, never by a run.
            stale(&path)
        } else {
            false
        };
        if gone {
            let ok = if is_dir {
                std::fs::remove_dir_all(&path).is_ok()
            } else {
                std::fs::remove_file(&path).is_ok()
            };
            removed += usize::from(ok);
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rcmd-tempfiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn age_by(p: &Path, by: Duration) {
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - by).unwrap();
    }

    #[test]
    fn a_run_gets_a_private_directory_of_its_own() {
        let b = base("make");
        let d = make_in(&b).unwrap();
        assert!(d.join(MARKER).exists());
        let again = make_in(&b).unwrap();
        assert_ne!(d, again, "an existing name is never reused");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&d).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn the_sweep_removes_ended_runs_and_nothing_else() {
        let b = base("sweep");
        let hour = Duration::from_secs(3600);
        // A run that ended an hour and a half ago, and one that is alive.
        let ended = make_in(&b).unwrap();
        std::fs::write(ended.join("view-0"), "x").unwrap();
        age_by(&ended.join(MARKER), hour + hour / 2);
        let alive = make_in(&b).unwrap();
        // A directory with our prefix but no marker: not ours to judge.
        let stranger = b.join("rcmd-run-unrelated");
        std::fs::create_dir_all(&stranger).unwrap();
        // What older versions left: stale, and fresh.
        let old_view = b.join("rcmd-view-1-0");
        std::fs::create_dir_all(&old_view).unwrap();
        let old_list = b.join("rcmd-rename-1-0.txt");
        std::fs::write(&old_list, "a").unwrap();
        age_by(&old_list, hour * 2);
        let fd = std::fs::File::open(&old_view).unwrap();
        let _ = fd.set_modified(SystemTime::now() - hour * 2);
        let fresh_view = b.join("rcmd-view-2-0");
        std::fs::create_dir_all(&fresh_view).unwrap();
        // Someone else's file.
        let other = b.join("notes.txt");
        std::fs::write(&other, "keep").unwrap();

        sweep_in(&b, Some(&alive), SystemTime::now());
        assert!(!ended.exists(), "an ended run goes");
        assert!(alive.exists(), "a live run stays");
        assert!(stranger.exists(), "no marker, not touched");
        assert!(!old_list.exists(), "a stale old list goes");
        assert!(fresh_view.exists(), "a fresh old view may be in use");
        assert!(other.exists(), "nothing else is touched");
        let _ = std::fs::remove_dir_all(&b);
    }
}
