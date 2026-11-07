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
pub fn dir_size(dir: &Path, stop: &AtomicBool) -> Option<Total> {
    let mut t = Total::default();
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
                stack.push(e.path());
            } else if ft.is_file() {
                t.files += 1;
                t.bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    Some(t)
}

/// Sizes being worked out in the background, one thread per request, all cancelled
/// together when dropped.
pub struct Sizer {
    stop: Arc<AtomicBool>,
    rx: Receiver<(PathBuf, Total)>,
    tx: mpsc::Sender<(PathBuf, Total)>,
    pending: usize,
}

impl Default for Sizer {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            rx,
            tx,
            pending: 0,
        }
    }
}

impl Sizer {
    pub fn request(&mut self, dir: PathBuf) {
        let (stop, tx) = (self.stop.clone(), self.tx.clone());
        self.pending += 1;
        thread::spawn(move || {
            if let Some(t) = dir_size(&dir, &stop) {
                let _ = tx.send((dir, t));
            }
        });
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
}
