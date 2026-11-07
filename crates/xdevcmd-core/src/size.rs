//! A directory's size, which a listing never knows: finding it means walking the tree.
//!
//! Runs on its own thread so the UI never waits on it, and stops when asked — pointing
//! it at `/` and then changing directory must not leave a walk grinding on behind you.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Total {
    pub files: u64,
    pub bytes: u64,
    /// Directories that could not be read. Their contents are missing from `bytes`,
    /// so the number is a lower bound and the UI says so.
    pub unreadable: u64,
}

/// Walk `dir` without following symlinks — a link to `/` inside it must not make it
/// "the size of the disk" — and count every regular file's length. `None` if `stop`
/// was set before it finished.
///
/// On Unix a file with several hard links counts once, as `du` does, and the walk stays
/// on `dir`'s own file system (`du -x`): a mount point inside — a network share, `/proc`
/// under `/` — is not walked into, where it could hang or count what is not there.
pub fn dir_size(dir: &Path, stop: &AtomicBool) -> Option<Total> {
    let mut t = Total::default();
    #[cfg(unix)]
    let (root_dev, mut linked) = {
        use std::os::unix::fs::MetadataExt;
        (
            fs::symlink_metadata(dir).map(|m| m.dev()).ok(),
            std::collections::HashSet::<(u64, u64)>::new(),
        )
    };
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let Ok(it) = fs::read_dir(&d) else {
            t.unreadable += 1;
            continue;
        };
        for e in it.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let here = e.metadata().map(|m| m.dev()).ok();
                    if root_dev.is_some() && here != root_dev {
                        continue;
                    }
                }
                stack.push(e.path());
            } else if ft.is_file() {
                let Ok(m) = e.metadata() else { continue };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if m.nlink() > 1 && !linked.insert((m.dev(), m.ino())) {
                        continue;
                    }
                }
                t.files += 1;
                t.bytes += m.len();
            }
        }
    }
    Some(t)
}

/// Sizes being worked out in the background by a few threads (as many as the machine
/// has cores, at most four) taking requests in turn: Ctrl+Space on a directory of
/// thousands of folders queues thousands of walks, not thousands of threads. All are
/// cancelled together when dropped.
pub struct Sizer {
    stop: Arc<AtomicBool>,
    rx: Receiver<(PathBuf, Total)>,
    tx: mpsc::Sender<(PathBuf, Total)>,
    jobs: Option<mpsc::Sender<PathBuf>>,
    pending: usize,
}

impl Default for Sizer {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            rx,
            tx,
            jobs: None,
            pending: 0,
        }
    }
}

impl Sizer {
    pub fn request(&mut self, dir: PathBuf) {
        let jobs = self.jobs.get_or_insert_with(|| {
            let (jobs, queue) = mpsc::channel::<PathBuf>();
            let queue = Arc::new(std::sync::Mutex::new(queue));
            let n = thread::available_parallelism()
                .map_or(2, |n| n.get())
                .clamp(1, 4);
            for _ in 0..n {
                let (queue, stop, tx) = (queue.clone(), self.stop.clone(), self.tx.clone());
                thread::spawn(move || loop {
                    // The lock is held only to take the next request.
                    let next = queue.lock().map(|q| q.recv());
                    let Ok(Ok(dir)) = next else { return };
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Some(t) = dir_size(&dir, &stop) {
                        let _ = tx.send((dir, t));
                    }
                });
            }
            jobs
        });
        if jobs.send(dir).is_ok() {
            self.pending += 1;
        }
    }

    /// Results that have arrived since the last call.
    pub fn poll(&mut self) -> Vec<(PathBuf, Total)> {
        let got: Vec<_> = self.rx.try_iter().collect();
        self.pending = self.pending.saturating_sub(got.len());
        got
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// Stop every walk in flight and start over with none.
    pub fn cancel(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        *self = Self::default();
    }
}

impl Drop for Sizer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-size-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn counts_every_file_below_including_nested() {
        let d = scratch("nested");
        fs::create_dir_all(d.join("a/b/c")).unwrap();
        fs::write(d.join("one"), vec![0u8; 100]).unwrap();
        fs::write(d.join("a/two"), vec![0u8; 20]).unwrap();
        fs::write(d.join("a/b/c/three"), vec![0u8; 3]).unwrap();
        let t = dir_size(&d, &AtomicBool::new(false)).unwrap();
        assert_eq!((t.files, t.bytes, t.unreadable), (3, 123, 0));
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_somewhere_huge_is_not_followed() {
        let d = scratch("link");
        fs::write(d.join("f"), b"12345").unwrap();
        std::os::unix::fs::symlink("/", d.join("root")).unwrap();
        let t = dir_size(&d, &AtomicBool::new(false)).unwrap();
        assert_eq!((t.files, t.bytes), (1, 5));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_stop_already_set_returns_nothing() {
        let d = scratch("stop");
        assert_eq!(dir_size(&d, &AtomicBool::new(true)), None);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_sizer_delivers_on_its_own_thread() {
        let d = scratch("sizer");
        fs::write(d.join("f"), vec![0u8; 7]).unwrap();
        let mut s = Sizer::default();
        s.request(d.clone());
        assert!(s.busy());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let got = loop {
            let g = s.poll();
            if !g.is_empty() || std::time::Instant::now() > deadline {
                break g;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(
            got,
            vec![(
                d.clone(),
                Total {
                    files: 1,
                    bytes: 7,
                    unreadable: 0
                }
            )]
        );
        assert!(!s.busy());
        let _ = fs::remove_dir_all(&d);
    }
    #[cfg(unix)]
    #[test]
    fn a_file_with_several_hard_links_counts_once() {
        let d = scratch("hardlinks");
        fs::write(d.join("f"), vec![0u8; 1000]).unwrap();
        fs::hard_link(d.join("f"), d.join("g")).unwrap();
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::hard_link(d.join("f"), d.join("sub/h")).unwrap();
        let t = dir_size(&d, &AtomicBool::new(false)).unwrap();
        assert_eq!((t.files, t.bytes), (1, 1000));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn many_requests_are_all_answered_by_a_few_threads() {
        let d = scratch("pool");
        let mut s = Sizer::default();
        for i in 0..40 {
            let sub = d.join(format!("d{i}"));
            fs::create_dir_all(&sub).unwrap();
            fs::write(sub.join("f"), vec![0u8; i]).unwrap();
            s.request(sub);
        }
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while got.len() < 40 && std::time::Instant::now() < deadline {
            got.extend(s.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.len(), 40);
        assert!(!s.busy());
        let total: u64 = got.iter().map(|(_, t)| t.bytes).sum();
        assert_eq!(total, (0..40).sum::<u64>());
        let _ = fs::remove_dir_all(&d);
    }
}
