//! Find files (Alt+F7): walk a tree for names matching a pattern, optionally only
//! files containing some text, on a thread of its own.
//!
//! Results stream back as they are found, so the first hits are on screen while the
//! walk goes on, and the walk stops the moment it is cancelled. Symlinks are listed
//! when their name matches but never followed — a link to `/` must not turn a search of
//! one project into a search of the disk — and only regular files are opened, so a FIFO
//! cannot block it.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use crate::glob::Pattern;

#[derive(Debug, Clone)]
pub struct Query {
    pub root: PathBuf,
    /// Names to match. Empty means every name.
    pub names: String,
    /// Only files containing this, ignoring ASCII case. Empty means no content test.
    pub text: String,
    /// Look in hidden files and directories too.
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Found(PathBuf),
    /// How far it has got: files looked at so far.
    Progress {
        scanned: u64,
    },
    Done {
        scanned: u64,
        unreadable: u64,
    },
}

pub struct Search {
    pub rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

impl Search {
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub fn start(q: Query) -> Search {
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    thread::spawn(move || {
        let (scanned, unreadable) = walk(&q, &flag, &mut |e| tx.send(e).is_ok());
        let _ = tx.send(Event::Done {
            scanned,
            unreadable,
        });
    });
    Search { rx, stop }
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// The walk itself, synchronous, so it can be tested without threads. `emit` returns
/// false when nobody is listening any more.
pub fn walk(q: &Query, stop: &AtomicBool, emit: &mut dyn FnMut(Event) -> bool) -> (u64, u64) {
    let names = Pattern::new(if q.names.trim().is_empty() {
        "*"
    } else {
        &q.names
    });
    let needle: Vec<u8> = q.text.as_bytes().to_ascii_lowercase();
    let (mut scanned, mut unreadable) = (0u64, 0u64);
    let mut stack = vec![q.root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(it) = fs::read_dir(&dir) else {
            unreadable += 1;
            continue;
        };
        let mut children: Vec<_> = it.flatten().collect();
        // Sorted, so results arrive in a stable order and tests can rely on it.
        children.sort_by_key(|e| e.file_name());
        for e in children.into_iter().rev() {
            if stop.load(Ordering::Relaxed) {
                return (scanned, unreadable);
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if !q.hidden && is_hidden(&name) {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                stack.push(path.clone());
            }
            scanned += 1;
            if scanned % 512 == 0 && !emit(Event::Progress { scanned }) {
                return (scanned, unreadable);
            }
            if !names.matches(&name, ft.is_dir()) {
                continue;
            }
            let hit = if needle.is_empty() {
                true
            } else if ft.is_file() {
                match contains(&path, &needle, stop) {
                    Ok(found) => found,
                    Err(_) => {
                        unreadable += 1;
                        false
                    }
                }
            } else {
                false
            };
            if hit && !emit(Event::Found(path)) {
                return (scanned, unreadable);
            }
        }
    }
    (scanned, unreadable)
}

/// Whether the file holds `needle` (already lower-cased), ignoring ASCII case. Read in
/// chunks that overlap by the needle's length, so a match across a boundary is found,
/// and never more than one chunk in memory.
pub fn contains(path: &Path, needle: &[u8], stop: &AtomicBool) -> io::Result<bool> {
    const CHUNK: usize = 1 << 20;
    if needle.is_empty() {
        return Ok(true);
    }
    let mut f = File::open(path)?;
    let keep = needle.len() - 1;
    let mut buf = vec![0u8; CHUNK + keep];
    let mut have = 0usize;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let n = f.read(&mut buf[have..])?;
        if n == 0 {
            return Ok(false);
        }
        have += n;
        let window = &mut buf[..have];
        window.make_ascii_lowercase();
        if window.windows(needle.len()).any(|w| w == needle) {
            return Ok(true);
        }
        let tail = have.min(keep);
        buf.copy_within(have - tail..have, 0);
        have = tail;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-find-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("src/deep")).unwrap();
        fs::create_dir_all(d.join(".git")).unwrap();
        fs::write(d.join("README.md"), "hello WORLD").unwrap();
        fs::write(d.join("src/main.rs"), "fn main() { todo!() }").unwrap();
        fs::write(d.join("src/deep/lib.rs"), "// nothing to see").unwrap();
        fs::write(d.join(".git/config.rs"), "todo").unwrap();
        d
    }

    fn run(q: &Query) -> Vec<String> {
        let mut out = Vec::new();
        walk(q, &AtomicBool::new(false), &mut |e| {
            if let Event::Found(p) = e {
                out.push(
                    p.strip_prefix(&q.root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
            true
        });
        out.sort();
        out
    }

    fn q(root: &Path, names: &str, text: &str) -> Query {
        Query {
            root: root.to_path_buf(),
            names: names.into(),
            text: text.into(),
            hidden: false,
        }
    }

    #[test]
    fn names_match_at_any_depth_and_hidden_is_skipped() {
        let d = tree("names");
        assert_eq!(
            run(&q(&d, "*.rs", "")),
            vec!["src/deep/lib.rs", "src/main.rs"]
        );
        let mut all = q(&d, "*.rs", "");
        all.hidden = true;
        assert_eq!(run(&all).len(), 3, "hidden: .git/config.rs too");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn text_narrows_to_files_containing_it_ignoring_case() {
        let d = tree("text");
        assert_eq!(run(&q(&d, "", "TODO")), vec!["src/main.rs"]);
        assert_eq!(run(&q(&d, "*.md", "world")), vec!["README.md"]);
        assert!(run(&q(&d, "", "absent")).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_match_split_across_chunks_is_still_found() {
        let d = tree("chunk");
        let p = d.join("big.bin");
        let mut body = vec![b'.'; (1 << 20) - 3];
        body.extend_from_slice(b"NEEDLE");
        fs::write(&p, &body).unwrap();
        assert!(contains(&p, b"needle", &AtomicBool::new(false)).unwrap());
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_and_fifos_are_not_opened() {
        let d = tree("links");
        std::os::unix::fs::symlink("/", d.join("root-link")).unwrap();
        let fifo = d.join("pipe");
        let ok = std::process::Command::new("mkfifo").arg(&fifo).status();
        let t = std::time::Instant::now();
        let hits = run(&q(&d, "", "anything"));
        assert!(hits.is_empty(), "{hits:?}");
        assert!(
            t.elapsed().as_secs() < 5,
            "neither / nor the FIFO was entered"
        );
        if ok.is_ok_and(|s| s.success()) {
            assert_eq!(run(&q(&d, "pipe", "")), vec!["pipe"], "listed by name");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_cancelled_search_stops() {
        let d = tree("stop");
        let mut n = 0;
        walk(&q(&d, "", ""), &AtomicBool::new(true), &mut |_| {
            n += 1;
            true
        });
        assert_eq!(n, 0);
        let _ = fs::remove_dir_all(&d);
    }
}
