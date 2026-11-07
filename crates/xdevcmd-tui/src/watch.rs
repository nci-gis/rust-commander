//! Directory changes from the system, so a pane refreshes when its directory changes
//! rather than finding out by looking every second.
//!
//! Only the directories on screen are watched, each without recursion (yazi and broot
//! do the same): inotify has a per-user limit, and a pane shows one level. The
//! system's events say only that something happened; the pane re-reads its listing.
//! Polling stays, less often, for what sends no events: network drives (NFS, SMB),
//! FUSE mounts, WSL's view of Windows drives.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

pub struct Watcher {
    inner: RecommendedWatcher,
    events: Receiver<notify::Result<notify::Event>>,
    watched: HashSet<PathBuf>,
}

impl Watcher {
    /// `None` where the system cannot watch at all; polling then does everything.
    pub fn new() -> Option<Self> {
        let (tx, events) = channel();
        let inner = notify::recommended_watcher(move |e| {
            let _ = tx.send(e);
        })
        .ok()?;
        Some(Self {
            inner,
            events,
            watched: HashSet::new(),
        })
    }

    /// Watch exactly `dirs`: start on the new ones, stop on those no longer shown. A
    /// directory that cannot be watched (out of inotify watches, gone) is simply left to
    /// polling. Returns the directories newly watched: what changed in them before the
    /// watch began was not seen, so they are worth one look.
    pub fn sync(&mut self, dirs: &HashSet<PathBuf>) -> HashSet<PathBuf> {
        let gone: Vec<PathBuf> = self.watched.difference(dirs).cloned().collect();
        for d in gone {
            let _ = self.inner.unwatch(&d);
            self.watched.remove(&d);
        }
        let mut new = HashSet::new();
        for d in dirs {
            if self.watched.contains(d) {
                continue;
            }
            if self.inner.watch(d, RecursiveMode::NonRecursive).is_ok() {
                self.watched.insert(d.clone());
                new.insert(d.clone());
            }
        }
        new
    }

    pub fn watching(&self, dir: &Path) -> bool {
        self.watched.contains(dir)
    }

    /// The watched directories something happened in since the last call. Reading a
    /// file (an access event) is not a change.
    pub fn changed(&mut self) -> HashSet<PathBuf> {
        let mut out = HashSet::new();
        for e in self.events.try_iter() {
            let Ok(e) = e else { continue };
            if matches!(e.kind, EventKind::Access(_)) {
                continue;
            }
            for p in &e.paths {
                if self.watched.contains(p) {
                    out.insert(p.clone());
                }
                if let Some(dir) = p.parent().filter(|d| self.watched.contains(*d)) {
                    out.insert(dir.to_path_buf());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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
