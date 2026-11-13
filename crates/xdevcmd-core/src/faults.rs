//! The fault seam: failures, halts and swaps put in front of the file system calls that
//! data safety rests on, for tests to reach the moments no real disk can be made to fail
//! at on demand.
//!
//! Each guarded call goes through [`at`] (or one of the wrappers below, which call it
//! first) with the [`Point`] it is about to pass and the path it concerns. With no rule
//! set, nothing happens.
//!
//! **Not in the shipped program.** The seam exists only in the engine's own tests
//! (`cfg(test)`) and with the `faults` feature, which `xdevcmd` turns on for its tests
//! alone (a dev-dependency): in any other build [`at`] is an inline `Ok(())` and the
//! rules, the trace and `XDEVCMD_FAULTS` are not compiled at all. A test in
//! `tests/budgets.rs` checks the release binary holds no such string.
//!
//! Rules are process-wide, since the worker runs on its own thread; each matches a
//! fragment of the path, so tests running at once, each in its own temporary directory,
//! never meet each other's.

use std::fs::File;
use std::io;
use std::path::Path;

/// A place a fault can be put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Point {
    /// One chunk written to a file being copied.
    Write,
    /// A file's data synced to the disk.
    SyncData,
    /// A directory synced, so the names in it survive a power cut.
    SyncDir,
    /// A rename onto the destination.
    Rename,
    /// A move across devices removing its source.
    RemoveSource,
    /// A file about to be replaced put in the trash (Round 031).
    Trash,
    /// A job's journal begun.
    JournalBegin,
    /// One line written to a journal.
    JournalLine,
    /// A journal's lines synced.
    JournalSync,
    /// A directory read (compare).
    ReadDir,
    /// An entry's metadata read (compare).
    Stat,
    /// A name looked up on the other side of a compare: found there means the volume
    /// ignores case. [`Action::Found`] makes a case-sensitive one answer as if it did.
    Lookup,
}

impl Point {
    /// The name used in `XDEVCMD_FAULTS`.
    pub fn name(self) -> &'static str {
        match self {
            Point::Write => "write",
            Point::SyncData => "sync-data",
            Point::SyncDir => "sync-dir",
            Point::Rename => "rename",
            Point::RemoveSource => "remove-source",
            Point::Trash => "trash",
            Point::JournalBegin => "journal-begin",
            Point::JournalLine => "journal-line",
            Point::JournalSync => "journal-sync",
            Point::ReadDir => "read-dir",
            Point::Stat => "stat",
            Point::Lookup => "lookup",
        }
    }
}

#[cfg(any(test, feature = "faults"))]
pub use imp::*;

/// No seam in this build: every point passes.
#[cfg(not(any(test, feature = "faults")))]
#[inline(always)]
pub fn at(_point: Point, _path: &Path) -> io::Result<()> {
    Ok(())
}

/// No seam in this build: every sync happens.
#[cfg(not(any(test, feature = "faults")))]
#[inline(always)]
pub(crate) fn skipped(_point: Point, _path: &Path) -> bool {
    false
}

/// No seam in this build: a lookup finds what is there.
#[cfg(not(any(test, feature = "faults")))]
#[inline(always)]
pub(crate) fn found(_point: Point, _path: &Path) -> bool {
    false
}

/// `f`'s data to the disk, `f` still open: on a network file system this is where a
/// write that failed is told. `path` is what `f` is, for the seam.
pub fn sync_data(f: &File, path: &Path) -> io::Result<()> {
    at(Point::SyncData, path)?;
    if skipped(Point::SyncData, path) {
        return Ok(());
    }
    f.sync_data()
}

/// The names in `dir` to the disk, so a file renamed or made there keeps its name after
/// a power cut. Unix only: Windows cannot open a directory to sync it without `unsafe`
/// code, and NTFS journals its names (see docs/security.md). A file system that cannot
/// sync a directory (it says EINVAL or ENOTSUP) is taken as having nothing to sync.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    at(Point::SyncDir, dir)?;
    if skipped(Point::SyncDir, dir) {
        return Ok(());
    }
    #[cfg(unix)]
    {
        let r = File::open(dir).and_then(|d| d.sync_all());
        match r {
            Err(e)
                if e.kind() == io::ErrorKind::Unsupported
                    || e.kind() == io::ErrorKind::InvalidInput =>
            {
                Ok(())
            }
            r => r,
        }
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

#[cfg(any(test, feature = "faults"))]
mod imp {
    use super::Point;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;

    /// What a rule does when its point is passed on a matching path.
    #[derive(Debug, Clone)]
    pub enum Action {
        /// Fail the `nth` time (1 is the first), with this error, then never again.
        Fail { nth: u32, error: Fault },
        /// Wait, every time.
        Delay(Duration),
        /// Write `marker` and block for ever, for a test to kill the process there.
        Halt { marker: PathBuf },
        /// Write `body` to `target` the first time, as another program would: the file
        /// replaced between a question and the step it was about.
        Swap { target: PathBuf, body: Vec<u8> },
        /// Do not make the real call (a sync), every time: the cost of the sync measured
        /// against its absence.
        Skip,
        /// Say a lookup found something there, every time ([`Point::Lookup`]): a volume
        /// that ignores case, on one that does not.
        Found,
    }

    /// An error to fail with, built when it is needed (`io::Error` is not `Clone`).
    #[derive(Debug, Clone, Copy)]
    pub enum Fault {
        Kind(io::ErrorKind),
        Os(i32),
    }

    impl Fault {
        fn make(self) -> io::Error {
            match self {
                Fault::Kind(k) => io::Error::new(k, "injected fault"),
                Fault::Os(code) => io::Error::from_raw_os_error(code),
            }
        }
    }

    #[derive(Debug, Clone)]
    struct Rule {
        point: Point,
        fragment: String,
        action: Action,
        seen: u32,
        id: u64,
    }

    static RULES: Mutex<Vec<Rule>> = Mutex::new(Vec::new());
    /// What a trace saw: each point passed, and on what.
    pub type Trace = Vec<(Point, PathBuf)>;

    /// Fragments being traced, each with what was seen.
    static TRACES: Mutex<Vec<(String, Trace)>> = Mutex::new(Vec::new());

    /// A rule while it is held; dropped, it is gone.
    #[must_use = "the rule is removed when this is dropped"]
    pub struct Guard(u64);

    impl Drop for Guard {
        fn drop(&mut self) {
            if let Ok(mut r) = RULES.lock() {
                r.retain(|x| x.id != self.0);
            }
        }
    }

    /// Put `action` at `point` for paths containing `fragment`.
    pub fn set(point: Point, fragment: &str, action: Action) -> Guard {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID: AtomicU64 = AtomicU64::new(1);
        let id = ID.fetch_add(1, Ordering::Relaxed);
        lock(&RULES).push(Rule {
            point,
            fragment: fragment.to_string(),
            action,
            seen: 0,
            id,
        });
        Guard(id)
    }

    /// Fail the `nth` pass of `point` on a path containing `fragment`.
    pub fn fail(point: Point, fragment: &str, nth: u32, error: Fault) -> Guard {
        set(point, fragment, Action::Fail { nth, error })
    }

    /// Record every point passed on a path containing `fragment`, until [`traced`].
    pub fn trace(fragment: &str) {
        lock(&TRACES).push((fragment.to_string(), Vec::new()));
    }

    /// What was recorded for `fragment`, and stop recording.
    pub fn traced(fragment: &str) -> Trace {
        let mut t = lock(&TRACES);
        match t.iter().position(|(f, _)| f == fragment) {
            Some(i) => t.remove(i).1,
            None => Vec::new(),
        }
    }

    fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        // A test that panicked while holding it must not take every later one down.
        m.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn matches(fragment: &str, path: &Path) -> bool {
        path.to_string_lossy().contains(fragment)
    }

    /// Pass `point` on `path`: whatever a rule says happens here.
    pub fn at(point: Point, path: &Path) -> io::Result<()> {
        load_env();
        {
            let mut t = lock(&TRACES);
            for (f, seen) in t.iter_mut() {
                if matches(f, path) {
                    seen.push((point, path.to_path_buf()));
                }
            }
        }
        let mut todo = Vec::new();
        {
            let mut rules = lock(&RULES);
            for r in rules.iter_mut() {
                if r.point == point && matches(&r.fragment, path) {
                    r.seen += 1;
                    todo.push((r.action.clone(), r.seen));
                }
            }
        }
        // Acted on with the lock let go: a halt blocks, and a delay must not hold up
        // other tests' points.
        for (action, seen) in todo {
            match action {
                Action::Fail { nth, error } if seen == nth => return Err(error.make()),
                Action::Fail { .. } | Action::Skip | Action::Found => {}
                Action::Delay(by) => std::thread::sleep(by),
                Action::Halt { marker } => {
                    let _ = std::fs::write(&marker, point.name());
                    loop {
                        std::thread::sleep(Duration::from_secs(3600));
                    }
                }
                Action::Swap { target, body } if seen == 1 => {
                    let _ = std::fs::remove_file(&target);
                    std::fs::write(&target, body)?;
                }
                Action::Swap { .. } => {}
            }
        }
        Ok(())
    }

    /// Whether a rule says a lookup at `point` finds something at `path`.
    pub(crate) fn found(point: Point, path: &Path) -> bool {
        lock(&RULES).iter().any(|r| {
            r.point == point && matches(&r.fragment, path) && matches!(r.action, Action::Found)
        })
    }

    /// Whether a rule says to leave out the real call at `point` on `path`.
    pub(crate) fn skipped(point: Point, path: &Path) -> bool {
        lock(&RULES).iter().any(|r| {
            r.point == point && matches(&r.fragment, path) && matches!(r.action, Action::Skip)
        })
    }

    /// `XDEVCMD_FAULTS`, read once: rules for a program run by a test (the pty suite),
    /// `;`-separated, each `kind|point|fragment[|argument]`: `|`, which no Windows path
    /// can hold, where `:` would cut `C:\…` in two:
    ///
    /// - `fail|<point>|<fragment>|<n>[|<os error code>]` (EIO when no code is given)
    /// - `halt|<point>|<fragment>|<marker file>`
    /// - `delay|<point>|<fragment>|<milliseconds>`
    fn load_env() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let Ok(spec) = std::env::var("XDEVCMD_FAULTS") else {
                return;
            };
            for (point, fragment, action) in parse(&spec) {
                std::mem::forget(set(point, &fragment, action));
            }
        });
    }

    /// The rules in an `XDEVCMD_FAULTS` value; what cannot be read is left out.
    pub fn parse(spec: &str) -> Vec<(Point, String, Action)> {
        const POINTS: [Point; 12] = [
            Point::Trash,
            Point::Write,
            Point::SyncData,
            Point::SyncDir,
            Point::Rename,
            Point::RemoveSource,
            Point::JournalBegin,
            Point::JournalLine,
            Point::JournalSync,
            Point::ReadDir,
            Point::Stat,
            Point::Lookup,
        ];
        let mut out = Vec::new();
        for rule in spec.split(';').filter(|r| !r.trim().is_empty()) {
            let mut w = rule.trim().splitn(4, '|');
            let (Some(kind), Some(point), Some(fragment)) = (w.next(), w.next(), w.next()) else {
                continue;
            };
            let arg = w.next().unwrap_or("");
            let Some(point) = POINTS.iter().copied().find(|p| p.name() == point) else {
                continue;
            };
            let action = match kind {
                "fail" => {
                    let mut a = arg.splitn(2, '|');
                    let nth = a.next().and_then(|n| n.parse().ok()).unwrap_or(1);
                    // EIO: 5 on Unix, ERROR_CRC (23, "data error") on Windows.
                    let code = a
                        .next()
                        .and_then(|c| c.parse().ok())
                        .unwrap_or(if cfg!(windows) { 23 } else { 5 });
                    Action::Fail {
                        nth,
                        error: Fault::Os(code),
                    }
                }
                "halt" if !arg.is_empty() => Action::Halt {
                    marker: PathBuf::from(arg),
                },
                "delay" => Action::Delay(Duration::from_millis(arg.parse().unwrap_or(0))),
                _ => continue,
            };
            out.push((point, fragment.to_string(), action));
        }
        out
    }
}

/// Places the tests need that a machine may not have: a second file system to move
/// across, a small one to fill. Each is `None`, with a line saying the test was skipped,
/// where there is none; nothing here mounts anything.
#[cfg(test)]
pub(crate) mod places {
    use std::path::{Path, PathBuf};

    /// A fresh directory `tag` under `base`.
    fn fresh(base: &Path, tag: &str) -> Option<PathBuf> {
        let d = base.join(format!("xdevcmd-xdev-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).ok()?;
        Some(d)
    }

    /// Whether `p` is on another volume than the temporary directory, as far as can be
    /// told without unsafe code: the device number on Unix, the drive on Windows.
    fn elsewhere(p: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let (Ok(a), Ok(b)) = (
                std::fs::metadata(p),
                std::fs::metadata(std::env::temp_dir()),
            ) else {
                return false;
            };
            a.dev() != b.dev()
        }
        #[cfg(not(unix))]
        {
            let drive = |p: &Path| {
                p.components()
                    .next()
                    .map(|c| c.as_os_str().to_ascii_lowercase())
            };
            drive(p) != drive(&std::env::temp_dir())
        }
    }

    /// A directory on another file system than the temporary directory:
    /// `XDEVCMD_TEST_OTHER_DEV` (CI mounts one), else tmpfs at /dev/shm on Linux.
    pub fn other_device(tag: &str) -> Option<PathBuf> {
        let mut bases: Vec<PathBuf> = std::env::var_os("XDEVCMD_TEST_OTHER_DEV")
            .map(PathBuf::from)
            .into_iter()
            .collect();
        if cfg!(target_os = "linux") {
            bases.push(PathBuf::from("/dev/shm"));
        }
        for b in bases {
            if b.is_dir() && elsewhere(&b) {
                if let Some(d) = fresh(&b, tag) {
                    return Some(d);
                }
            }
        }
        eprintln!("{tag}: skipped, no second file system (set XDEVCMD_TEST_OTHER_DEV)");
        None
    }

    /// A directory on a small file system of its own, to fill: `XDEVCMD_TEST_SMALL_FS`.
    pub fn small_fs(tag: &str) -> Option<PathBuf> {
        let got = std::env::var_os("XDEVCMD_TEST_SMALL_FS")
            .map(PathBuf::from)
            .filter(|b| b.is_dir())
            .and_then(|b| fresh(&b, tag));
        if got.is_none() {
            eprintln!("{tag}: skipped, no small file system (set XDEVCMD_TEST_SMALL_FS)");
        }
        got
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-faults-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_seam_fails_the_nth_pass_of_one_path_and_only_that_path() {
        let d = dir("nth");
        let (a, b) = (d.join("a.txt"), d.join("b.txt"));
        let _g = fail(
            Point::Write,
            "faults-nth-",
            2,
            Fault::Kind(io::ErrorKind::StorageFull),
        );
        // A path of another test: never touched.
        let _other = fail(Point::Write, "no-such-fragment-here", 1, Fault::Os(5));
        assert!(at(Point::Write, &a).is_ok(), "first pass");
        let e = at(Point::Write, &b).unwrap_err();
        assert_eq!(
            e.kind(),
            io::ErrorKind::StorageFull,
            "the second, on either path"
        );
        assert!(at(Point::Write, &a).is_ok(), "once only");
        assert!(
            at(Point::SyncData, &a).is_ok(),
            "another point is not this rule's"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_dropped_rule_is_gone_and_a_trace_records_order() {
        let d = dir("trace");
        let p = d.join("x");
        let g = fail(Point::Rename, "faults-trace-", 1, Fault::Os(28));
        drop(g);
        trace("faults-trace-");
        assert!(
            at(Point::Rename, &p).is_ok(),
            "the rule went with its guard"
        );
        let mut f = File::create(&p).unwrap();
        f.write_all(b"x").unwrap();
        sync_data(&f, &p).unwrap();
        sync_dir(&d).unwrap();
        let seen: Vec<Point> = traced("faults-trace-")
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        assert_eq!(seen, vec![Point::Rename, Point::SyncData, Point::SyncDir]);
        assert!(traced("faults-trace-").is_empty(), "taken once");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_swap_replaces_the_target_once() {
        let d = dir("swap");
        let t = d.join("t");
        std::fs::write(&t, "agreed").unwrap();
        let _g = set(
            Point::SyncData,
            "faults-swap-",
            Action::Swap {
                target: t.clone(),
                body: b"theirs".to_vec(),
            },
        );
        at(Point::SyncData, &d.join("tmp")).unwrap();
        assert_eq!(std::fs::read(&t).unwrap(), b"theirs");
        std::fs::write(&t, "again").unwrap();
        at(Point::SyncData, &d.join("tmp")).unwrap();
        assert_eq!(std::fs::read(&t).unwrap(), b"again", "once");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn xdevcmd_faults_is_read_rule_by_rule() {
        let rules = parse(
            "fail|sync-data|b.txt|2|28; halt|remove-source|a.txt|C:\\t\\m;bogus;fail|nowhere|x|1;delay|write|c|5",
        );
        assert_eq!(rules.len(), 3, "{rules:?}");
        assert!(matches!(
            rules[0],
            (Point::SyncData, ref f, Action::Fail { nth: 2, error: Fault::Os(28) }) if f == "b.txt"
        ));
        assert!(matches!(
            &rules[1],
            (Point::RemoveSource, f, Action::Halt { marker }) if f == "a.txt" && marker == Path::new("C:\\t\\m")
        ));
        assert!(matches!(rules[2], (Point::Write, _, Action::Delay(d)) if d.as_millis() == 5));
    }
}
