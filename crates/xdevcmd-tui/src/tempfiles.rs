//! xdevcmd's own temporary files, kept track of so none stays behind.
//!
//! Everything xdevcmd writes to the system's temporary directory — a file viewed from
//! inside an archive, the list of names for renaming many in an editor — goes in one
//! directory for the run, `xdevcmd-run-<pid>`, readable by the user only. While xdevcmd runs
//! a thread of its own refreshes a marker file there every few minutes — a thread, so
//! the marker stays fresh while an editor has the terminal for hours; when xdevcmd ends it removes the
//! directory; and when xdevcmd starts it removes the directories of runs that ended
//! without doing so (killed, crashed, the power cut), recognised by a marker not
//! refreshed for an hour. Only directories carrying the marker are ever removed, so
//! nothing of anyone else's is touched.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

const RUN_PREFIX: &str = "xdevcmd-run-";
const MARKER: &str = ".xdevcmd-alive";
/// A run whose marker is older than this has ended.
const STALE: Duration = Duration::from_secs(60 * 60);
/// How often a running xdevcmd refreshes its marker: well inside `STALE`, and often
/// enough that waking from sleep — when the thread's monotonic clock has stood still
/// and the marker's wall-clock time is hours old — leaves it stale for seconds only.
const TOUCH_EVERY: Duration = Duration::from_secs(30);

static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
/// What the directory was when this run made it (device and inode on Unix), to tell
/// it from one made later under the same name.
static IDENTITY: OnceLock<Option<(u64, u64)>> = OnceLock::new();

fn identity(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::symlink_metadata(p).ok().filter(|m| m.is_dir())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        // The temporary directory on Windows is the user's own (under the profile).
        let _ = m;
        Some((0, 0))
    }
}

/// Whether `d` is still the directory this run made: a real directory, the same one.
fn ours(d: &Path) -> bool {
    matches!((IDENTITY.get(), identity(d)), (Some(Some(a)), Some(b)) if *a == b)
}

/// This run's directory, made on first use, with the thread that keeps its marker
/// fresh. `None` if it cannot be made.
pub fn dir() -> Option<PathBuf> {
    DIR.get_or_init(|| {
        let d = make_in(&std::env::temp_dir())?;
        let _ = IDENTITY.set(identity(&d));
        let (dir, marker) = (d.clone(), d.join(MARKER));
        std::thread::spawn(move || loop {
            std::thread::sleep(TOUCH_EVERY);
            // Only in the directory this run made, which no one else can write in, and
            // without creating anything: a removed directory is never made again.
            if ours(&dir) {
                if let Ok(f) = std::fs::File::options().write(true).open(&marker) {
                    let _ = f.set_modified(SystemTime::now());
                }
            }
        });
        Some(d)
    })
    .clone()
}

/// A path for `name` in this run's directory — only while that directory is still
/// the one this run made (the same directory, not a link, not one made since under
/// the same name). If it has gone, nothing is written in its place.
pub fn file(name: &str) -> Option<PathBuf> {
    let d = dir()?;
    ours(&d).then(|| d.join(name))
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

/// Remove this run's directory: xdevcmd is ending.
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
        // Only a directory with the marker is a run of ours.
        let marker = path.join(MARKER);
        let gone = name.starts_with(RUN_PREFIX) && is_dir && marker.exists() && stale(&marker);
        if gone {
            removed += usize::from(std::fs::remove_dir_all(&path).is_ok());
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("xdevcmd-tempfiles-{tag}-{}", std::process::id()));
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
        let stranger = b.join("xdevcmd-run-unrelated");
        std::fs::create_dir_all(&stranger).unwrap();
        // Someone else's file.
        let other = b.join("notes.txt");
        std::fs::write(&other, "keep").unwrap();

        sweep_in(&b, Some(&alive), SystemTime::now());
        assert!(!ended.exists(), "an ended run goes");
        assert!(alive.exists(), "a live run stays");
        assert!(stranger.exists(), "no marker, not touched");
        assert!(other.exists(), "nothing else is touched");
        let _ = std::fs::remove_dir_all(&b);
    }
}
