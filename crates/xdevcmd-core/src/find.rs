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

/// Events a search may have waiting for the screen (B08): past this the walk waits, so a
/// search of a whole disk cannot fill memory faster than the pane takes the rows in.
pub const QUEUE: usize = 4096;
/// Rows a search shows at most (B08). It stops there and says so: past this, a list is
/// too long to read and a narrower search is the answer.
pub const MAX_FOUND: usize = 100_000;

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
    /// A path in a list ([`list`]) that is not there any more.
    Gone(PathBuf),
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
    let (tx, rx) = mpsc::sync_channel(QUEUE);
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

/// Check a list of paths on a thread of its own: [`Event::Found`] for each that is
/// there, [`Event::Gone`] for each that is not, in the list's order, each path once.
///
/// A list is made of what a shell printed, so it can be long (a `find /`) and can name
/// anything. Nothing is looked up on another machine (`\\server\…`, `//host/…`): a
/// lookup there can hang, and the user would see a frozen screen. Those count as gone.
pub fn list(paths: Vec<PathBuf>) -> Search {
    let (tx, rx) = mpsc::sync_channel(QUEUE);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    thread::spawn(move || {
        let scanned = check(&paths, &flag, &mut |e| tx.send(e).is_ok());
        let _ = tx.send(Event::Done {
            scanned,
            unreadable: 0,
        });
    });
    Search { rx, stop }
}

/// One line's candidates, taken from a shell's output: the whole line (as shown, then
/// without trailing spaces) and the paths found in it by the `path:line` rules
/// (`found::paths_in`). The whole line, when it names something, is that thing: the
/// paths found in it are then not rows (`report:2026.txt` is one file, not `report`).
pub type Group = (Vec<PathBuf>, Vec<PathBuf>);

/// [`list`], where some rows are a line's candidates ([`Group`]): decided on the
/// check's own thread, never on the screen's, since a list can hold 10,000 lines.
pub fn list_grouped(paths: Vec<PathBuf>, groups: Vec<Group>) -> Search {
    let (tx, rx) = mpsc::sync_channel(QUEUE);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    thread::spawn(move || {
        let scanned = check_grouped(&paths, &groups, &flag, &mut |e| tx.send(e).is_ok());
        let _ = tx.send(Event::Done {
            scanned,
            unreadable: 0,
        });
    });
    Search { rx, stop }
}

/// [`list_grouped`]'s work. Each line's whole is looked at first, in order; the first
/// that is there is the line's row and the rest of its candidates go (as `Gone`, which
/// a list drops quietly for rows never checked before). A path found in lines whose
/// wholes were all there goes too; anything else is checked as [`check`] does.
pub fn check_grouped(
    paths: &[PathBuf],
    groups: &[Group],
    stop: &AtomicBool,
    emit: &mut dyn FnMut(Event) -> bool,
) -> u64 {
    let rows: std::collections::HashSet<&PathBuf> = paths.iter().collect();
    let mut decided: std::collections::HashMap<&PathBuf, bool> = Default::default();
    let mut won = vec![false; groups.len()];
    let mut scanned = 0u64;
    for (g, (wholes, _)) in groups.iter().enumerate() {
        for w in wholes.iter().filter(|w| rows.contains(w)) {
            if stop.load(Ordering::Relaxed) {
                return scanned;
            }
            if let Some(&there) = decided.get(w) {
                won[g] |= there;
                continue;
            }
            // This line is already taken whole: its later wholes are left unlooked, not
            // decided, since another line may name the same thing whole.
            if won[g] {
                continue;
            }
            scanned += 1;
            let there = !remote_name(w) && fs::symlink_metadata(w).is_ok();
            won[g] |= there;
            decided.insert(w, there);
            let e = if there {
                Event::Found(w.clone())
            } else {
                Event::Gone(w.clone())
            };
            if !emit(e) {
                return scanned;
            }
        }
    }
    // Wholes no line needed looked at: not rows (dropped quietly, never checked).
    for (wholes, _) in groups {
        for w in wholes.iter().filter(|w| rows.contains(w)) {
            if !decided.contains_key(w) {
                decided.insert(w, false);
                if !emit(Event::Gone(w.clone())) {
                    return scanned;
                }
            }
        }
    }
    // A path found in a line stands unless every line it was found in was taken whole.
    let mut beaten: std::collections::HashMap<&PathBuf, bool> = Default::default();
    for (g, (_, parts)) in groups.iter().enumerate() {
        for p in parts {
            let b = beaten.entry(p).or_insert(true);
            *b &= won[g];
        }
    }
    let rest: Vec<PathBuf> = paths
        .iter()
        .filter(|p| !decided.contains_key(p))
        .filter(|p| {
            if beaten.get(p).copied().unwrap_or(false) {
                let _ = emit(Event::Gone((*p).clone()));
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();
    scanned + check(&rest, stop, emit)
}

/// Whether `p` names a place on another machine.
pub fn remote_name(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.starts_with("\\\\") || s.starts_with("//")
}

/// [`list`]'s work, synchronous, for tests. Returns the paths looked at.
pub fn check(paths: &[PathBuf], stop: &AtomicBool, emit: &mut dyn FnMut(Event) -> bool) -> u64 {
    let mut seen = std::collections::HashSet::new();
    let mut scanned = 0u64;
    for p in paths {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if !seen.insert(p) {
            continue;
        }
        scanned += 1;
        let there = !remote_name(p) && fs::symlink_metadata(p).is_ok();
        let e = if there {
            Event::Found(p.clone())
        } else {
            Event::Gone(p.clone())
        };
        if !emit(e) || (scanned.is_multiple_of(512) && !emit(Event::Progress { scanned })) {
            break;
        }
    }
    scanned
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

    #[test]
    fn a_list_is_checked_once_each_in_its_order() {
        let d = tree("list");
        let paths = vec![
            d.join("src/main.rs"),
            d.join("gone.rs"),
            d.join("src/main.rs"),
            d.join("src"),
            PathBuf::from("//host/share/a"),
            PathBuf::from("\\\\server\\share\\a"),
        ];
        let mut got = Vec::new();
        let n = check(&paths, &AtomicBool::new(false), &mut |e| {
            got.push(e);
            true
        });
        assert_eq!(n, 5, "the duplicate is looked at once");
        assert_eq!(
            got,
            vec![
                Event::Found(d.join("src/main.rs")),
                Event::Gone(d.join("gone.rs")),
                Event::Found(d.join("src")),
                Event::Gone(PathBuf::from("//host/share/a")),
                Event::Gone(PathBuf::from("\\\\server\\share\\a")),
            ]
        );
        // Stopped: nothing more.
        let mut got = 0;
        check(&paths, &AtomicBool::new(true), &mut |_| {
            got += 1;
            true
        });
        assert_eq!(got, 0);
        // Through the thread, as the pane uses it.
        let s = list(paths);
        let events: Vec<Event> = s.rx.iter().collect();
        assert!(matches!(
            events.last(),
            Some(Event::Done { scanned: 5, .. })
        ));
        let _ = fs::remove_dir_all(&d);
    }

    /// A line's whole decides only when it is there: then its parts go (quietly, as
    /// `Gone`); otherwise the whole goes and its parts are checked as before. A part
    /// that another line, not taken whole, also named stays.
    #[test]
    fn a_whole_line_that_exists_wins_over_the_paths_found_in_it() {
        let d = std::env::temp_dir().join(format!("xdevcmd-find-whole-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        for n in ["a b.txt", "b.txt", "c.txt"] {
            fs::write(d.join(n), "x").unwrap();
        }
        let p = |n: &str| d.join(n);
        // "a b.txt": there whole. "x c.txt": not, so c.txt is checked as today. "b.txt"
        // is found in both lines' parts... and in a third, not taken whole.
        let groups: Vec<Group> = vec![
            (vec![p("a b.txt")], vec![p("a"), p("b.txt")]),
            (vec![p("x c.txt")], vec![p("x"), p("c.txt")]),
        ];
        let rows = vec![
            p("a b.txt"),
            p("a"),
            p("b.txt"),
            p("x c.txt"),
            p("x"),
            p("c.txt"),
        ];
        let mut found = Vec::new();
        check_grouped(&rows, &groups, &AtomicBool::new(false), &mut |e| {
            if let Event::Found(f) = e {
                found.push(f);
            }
            true
        });
        found.sort();
        assert_eq!(found, [p("a b.txt"), p("c.txt")]);
        // b.txt also named by a line not taken whole: it stays.
        let mut groups = groups;
        groups.push((vec![p("nope")], vec![p("b.txt")]));
        let mut found = Vec::new();
        check_grouped(&rows, &groups, &AtomicBool::new(false), &mut |e| {
            if let Event::Found(f) = e {
                found.push(f);
            }
            true
        });
        assert!(found.contains(&p("b.txt")), "{found:?}");
        let _ = fs::remove_dir_all(&d);
    }

    /// A line taken whole on its first whole does not decide its second for another
    /// line that names the same thing whole: that one is still looked at.
    #[test]
    fn a_whole_left_unlooked_by_one_line_is_still_found_for_another() {
        let d = std::env::temp_dir().join(format!("xdevcmd-find-whole2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("x y"), "x").unwrap();
        fs::write(d.join("x"), "x").unwrap();
        // Line 1 as shown "x y" (there) then trimmed... a second whole, "x"; line 2 is "x".
        let groups: Vec<Group> = vec![
            (vec![d.join("x y"), d.join("x")], vec![]),
            (vec![d.join("x")], vec![]),
        ];
        let rows = vec![d.join("x y"), d.join("x")];
        let mut found = Vec::new();
        let mut gone = Vec::new();
        check_grouped(&rows, &groups, &AtomicBool::new(false), &mut |e| {
            match e {
                Event::Found(f) => found.push(f),
                Event::Gone(g) => gone.push(g),
                _ => {}
            }
            true
        });
        found.sort();
        assert_eq!(found, [d.join("x"), d.join("x y")], "gone: {gone:?}");
        assert!(gone.is_empty(), "{gone:?}");
        let _ = fs::remove_dir_all(&d);
    }
}
