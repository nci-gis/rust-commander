//! Directory changes from the system, so a pane refreshes when its directory changes
//! rather than finding out by looking every second.
//!
//! Only the directories on screen are watched, each without recursion (yazi and broot
//! do the same): inotify has a per-user limit, and a pane shows one level. The
//! system's events say only that something happened; the pane re-reads its listing.
//! Polling stays, less often, for what sends no events: network drives (NFS, SMB),
//! FUSE mounts, WSL's view of Windows drives.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// A directory is reloaded once it has been quiet this long (pillar 2, A07)...
const QUIET: Duration = Duration::from_millis(100);
/// ...or once its first unreported change is this old, however busy it stays.
const MAX_WAIT: Duration = Duration::from_secs(1);

/// What the system's events have said, folded as they arrive: one entry per watched
/// directory, never a queue. An event storm (10,000 files made in a second) costs the
/// same memory as one event, and the app's tick reads at most one line per directory.
#[derive(Default)]
struct Dirty {
    /// Watched directory → (first change not yet reported, latest change).
    dirs: HashMap<PathBuf, (Instant, Instant)>,
    /// The system lost events (inotify's queue overflowed, a watch failed): every
    /// watched directory is looked at again rather than trusted.
    rescan: bool,
}

impl Dirty {
    fn note(&mut self, dir: PathBuf, now: Instant) {
        self.dirs
            .entry(dir)
            .and_modify(|(_, last)| *last = now)
            .or_insert((now, now));
    }

    /// The directories due now, taken out: quiet for [`QUIET`], or waiting since
    /// [`MAX_WAIT`]. After lost events, every one in `watched`.
    fn due(&mut self, now: Instant, watched: &HashSet<PathBuf>) -> HashSet<PathBuf> {
        if std::mem::take(&mut self.rescan) {
            self.dirs.clear();
            return watched.clone();
        }
        let ready: Vec<PathBuf> = self
            .dirs
            .iter()
            .filter(|(_, (first, last))| {
                now.duration_since(*last) >= QUIET || now.duration_since(*first) >= MAX_WAIT
            })
            .map(|(d, _)| d.clone())
            .collect();
        for d in &ready {
            self.dirs.remove(d);
        }
        ready.into_iter().collect()
    }
}

pub struct Watcher {
    inner: RecommendedWatcher,
    dirty: Arc<Mutex<Dirty>>,
    /// Shared with the event thread, which keeps only events about these.
    watched: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Watcher {
    /// `None` where the system cannot watch at all; polling then does everything.
    pub fn new() -> Option<Self> {
        let dirty = Arc::new(Mutex::new(Dirty::default()));
        let watched = Arc::new(Mutex::new(HashSet::new()));
        let (d, w) = (dirty.clone(), watched.clone());
        // Locks are always taken `watched` first, then `dirty` (here, in `sync` and in
        // `changed`), so the event thread and the app never wait on each other.
        let inner = notify::recommended_watcher(move |e: notify::Result<notify::Event>| {
            let now = Instant::now();
            let Ok(watched) = w.lock() else { return };
            let Ok(mut dirty) = d.lock() else { return };
            let e = match e {
                Ok(e) => e,
                Err(_) => {
                    dirty.rescan = true;
                    return;
                }
            };
            if e.need_rescan() {
                dirty.rescan = true;
                return;
            }
            // Reading a file (an access event) is not a change.
            if matches!(e.kind, EventKind::Access(_)) {
                return;
            }
            for p in &e.paths {
                if watched.contains(p) {
                    dirty.note(p.clone(), now);
                }
                if let Some(dir) = p.parent().filter(|d| watched.contains(*d)) {
                    dirty.note(dir.to_path_buf(), now);
                }
            }
        })
        .ok()?;
        Some(Self {
            inner,
            dirty,
            watched,
        })
    }

    /// Watch exactly `dirs`: start on the new ones, stop on those no longer shown. A
    /// directory that cannot be watched (out of inotify watches, gone) is simply left to
    /// polling. Returns the directories newly watched: what changed in them before the
    /// watch began was not seen, so they are worth one look.
    pub fn sync(&mut self, dirs: &HashSet<PathBuf>) -> HashSet<PathBuf> {
        // No lock is held while notify works: its watch and unwatch wait for its own
        // event thread, which may be in our callback waiting for `watched`.
        let now: HashSet<PathBuf> = self.watched.lock().map(|w| w.clone()).unwrap_or_default();
        for d in now.difference(dirs) {
            let _ = self.inner.unwatch(d);
            if let Ok(mut w) = self.watched.lock() {
                w.remove(d);
                if let Ok(mut dirty) = self.dirty.lock() {
                    dirty.dirs.remove(d);
                }
            }
        }
        let mut new = HashSet::new();
        for d in dirs.difference(&now) {
            if self.inner.watch(d, RecursiveMode::NonRecursive).is_ok() {
                if let Ok(mut w) = self.watched.lock() {
                    w.insert(d.clone());
                }
                new.insert(d.clone());
            }
        }
        new
    }

    pub fn watching(&self, dir: &Path) -> bool {
        self.watched.lock().is_ok_and(|w| w.contains(dir))
    }

    /// The watched directories that changed and are due a reload now: quiet for 100 ms,
    /// or changing for a second already. All of them after the system lost events.
    pub fn changed(&mut self) -> HashSet<PathBuf> {
        let watched = self.watched.lock().map(|w| w.clone()).unwrap_or_default();
        self.dirty
            .lock()
            .map(|mut d| d.due(Instant::now(), &watched))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(dirs: &[&str]) -> HashSet<PathBuf> {
        dirs.iter().map(PathBuf::from).collect()
    }

    /// A07: a burst is reported once it is quiet, a storm at least once a second, and
    /// lost events mean every watched directory again.
    #[test]
    fn changes_wait_for_quiet_but_never_past_max_wait_and_lost_events_rescan() {
        let watched = set(&["/a", "/b"]);
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut d = Dirty::default();
        d.note("/a".into(), ms(0));
        d.note("/a".into(), ms(50));
        assert!(d.due(ms(100), &watched).is_empty(), "50 ms quiet only");
        assert_eq!(d.due(ms(150), &watched), set(&["/a"]));
        assert!(d.due(ms(160), &watched).is_empty(), "reported once");
        // A storm: a change every 50 ms never lets it be quiet; it is still reported
        // within a second of its first change.
        let mut reported = None;
        for i in 0..40 {
            let now = ms(1000 + i * 50);
            d.note("/b".into(), now);
            if !d.due(now, &watched).is_empty() {
                reported = Some(i * 50);
                break;
            }
        }
        assert_eq!(reported, Some(1000));
        // The system lost events: everything watched, once.
        d.rescan = true;
        assert_eq!(d.due(ms(5000), &watched), watched);
        assert!(d.due(ms(5001), &watched).is_empty());
    }

    #[test]
    fn a_new_file_in_a_watched_directory_is_reported_and_others_are_not() {
        let Some(mut w) = Watcher::new() else { return };
        let root = std::env::temp_dir().join(format!("xdevcmd-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (seen, unseen) = (root.join("seen"), root.join("unseen"));
        std::fs::create_dir_all(&seen).unwrap();
        std::fs::create_dir_all(&unseen).unwrap();
        // Canonical, as the system reports it (macOS's /tmp is /private/tmp).
        let seen = seen.canonicalize().unwrap();
        w.sync(&HashSet::from([seen.clone()]));
        assert!(w.watching(&seen));
        std::fs::write(unseen.join("x"), "x").unwrap();
        std::fs::write(seen.join("new.txt"), "x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = HashSet::new();
        while !got.contains(&seen) && Instant::now() < deadline {
            got.extend(w.changed());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(got.contains(&seen), "{got:?}");
        assert!(!got.iter().any(|p| p.ends_with("unseen")), "{got:?}");
        // No longer shown: no longer watched.
        w.sync(&HashSet::new());
        assert!(!w.watching(&seen));
        let _ = std::fs::remove_dir_all(&root);
    }
}
