//! The debug log (Round 033): what jobs did and how long each part took, for a user to
//! send with a report. Off unless `XDEVCMD_DEBUG` asks for it, in every build.
//!
//! **No names.** Nothing written here names a file, a directory, the user or the host:
//! a line holds numbers and fixed words only, and a file is a number ([`file_id`]), the
//! same for the same path within one run and unrelated across runs (a hash keyed by a
//! random key that is never written). A log can be attached to a report as it is.
//!
//! One JSON object a line (`debug-<time>-<pid>.jsonl`), written straight to the file so
//! that a run that ends without warning (killed, aborted) leaves every line before it.
//! It stops at [`LIMIT`]; the newest [`KEEP`] logs are kept. Nothing is sent anywhere.
//!
//! Off, each call site costs one atomic load ([`on`]): nothing is built or written.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::fs::File;
use std::hash::BuildHasher;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// What a log can hold. `XDEVCMD_DEBUG=jobs,errors`, `all` or `1` for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    /// Each job: its shape, the time per phase, the copy engines, group commit's
    /// batches and probe, where a resume starts.
    Jobs = 1,
    /// Each error, retry and question in a job: the system's code, the phase, the
    /// attempt, the answer.
    Errors = 2,
}

/// The log stops here, with a last line saying so.
pub const LIMIT: u64 = 16 << 20;
/// How many logs are kept: the newest few, so they never pile up.
pub const KEEP: usize = 5;
/// Lines kept in memory for a crash log ([`tail`]).
pub const TAIL: usize = 200;

static ON: AtomicU8 = AtomicU8::new(0);
static LOG: Mutex<Option<Log>> = Mutex::new(None);
static JOBS: AtomicU64 = AtomicU64::new(0);

struct Log {
    file: Option<File>,
    path: PathBuf,
    written: u64,
    started: std::time::Instant,
    tail: VecDeque<String>,
}

/// Whether `t` is being logged. One load: call sites build nothing when it is off.
#[inline]
pub fn on(t: Topic) -> bool {
    ON.load(Ordering::Relaxed) & t as u8 != 0
}

/// The topics `value` asks for, and the words it holds that are none.
pub fn parse(value: &str) -> (u8, Vec<String>) {
    let mut bits = 0;
    let mut unknown = Vec::new();
    for w in value
        .split([',', ' ', ';'])
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        match w.to_ascii_lowercase().as_str() {
            "1" | "all" | "on" | "true" | "yes" => bits |= Topic::Jobs as u8 | Topic::Errors as u8,
            "jobs" | "job" => bits |= Topic::Jobs as u8,
            "errors" | "error" => bits |= Topic::Errors as u8,
            "0" | "off" | "false" | "no" => {}
            _ => unknown.push(w.to_string()),
        }
    }
    (bits, unknown)
}

/// The topics' names, for the status line.
pub fn topic_names(bits: u8) -> &'static str {
    match bits & 3 {
        3 => "jobs, errors",
        1 => "jobs",
        2 => "errors",
        _ => "",
    }
}

/// Start logging `bits` to a new file in `dir`. The file's path; an error if it could
/// not be made (the log stays off).
pub fn start(bits: u8, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("debug-{now}-{}.jsonl", std::process::id()));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    // The others first, the new one never among them: one fewer than KEEP stay.
    prune(dir, KEEP - 1);
    let file = o.open(&path)?;
    if let Ok(mut g) = LOG.lock() {
        *g = Some(Log {
            file: Some(file),
            path: path.clone(),
            written: 0,
            started: std::time::Instant::now(),
            tail: VecDeque::new(),
        });
    }
    ON.store(bits & 3, Ordering::Relaxed);
    Event::new("start")
        .word("version", env!("CARGO_PKG_VERSION"))
        .word("os", std::env::consts::OS)
        .word("arch", std::env::consts::ARCH)
        .word(
            "topics",
            if bits & 3 == 3 {
                "all"
            } else {
                topic_names(bits)
            },
        )
        .num("unix_time", now)
        .num(
            "cpus",
            std::thread::available_parallelism().map_or(0, |n| n.get() as u64),
        )
        .write();
    Ok(path)
}

/// Stop logging (tests; xdevcmd logs until it ends).
pub fn stop() {
    ON.store(0, Ordering::Relaxed);
    if let Ok(mut g) = LOG.lock() {
        *g = None;
    }
}

/// The log's path and its last lines, for a crash log. Never waits: a panic while the
/// log was being written finds it locked and gets nothing.
pub fn tail() -> Option<(PathBuf, Vec<String>)> {
    let g = LOG.try_lock().ok()?;
    let log = g.as_ref()?;
    Some((log.path.clone(), log.tail.iter().cloned().collect()))
}

/// A new job's number, for its lines.
pub fn next_job() -> u64 {
    JOBS.fetch_add(1, Ordering::Relaxed) + 1
}

/// A file as a number: the same path gives the same number within this run, and no
/// run can tell another's (the key is random and never written). 48 bits, so a JSON
/// reader keeps it whole.
pub fn file_id(p: &Path) -> u64 {
    static KEY: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
    KEY.get_or_init(Default::default).hash_one(p) & ((1 << 48) - 1)
}

/// One line being built. Values are numbers and fixed words only: a word is kept to
/// letters, digits and `_ . -`, and 40 characters, so even a mistake cannot write a
/// path with its separators.
pub struct Event {
    line: String,
}

impl Event {
    pub fn new(ev: &str) -> Self {
        let mut e = Self {
            line: String::with_capacity(128),
        };
        e.line.push('{');
        let ms = LOG
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|l| l.started.elapsed().as_millis() as u64))
            .unwrap_or(0);
        let _ = write!(e.line, "\"t_ms\":{ms}");
        e.word("ev", ev)
    }

    pub fn num(mut self, k: &str, v: u64) -> Self {
        let _ = write!(self.line, ",\"{k}\":{v}");
        self
    }

    /// Milliseconds, to a hundredth.
    pub fn ms(mut self, k: &str, d: std::time::Duration) -> Self {
        let _ = write!(self.line, ",\"{k}\":{:.2}", d.as_secs_f64() * 1000.0);
        self
    }

    pub fn float(mut self, k: &str, v: f64) -> Self {
        if v.is_finite() {
            let _ = write!(self.line, ",\"{k}\":{v:.3}");
        }
        self
    }

    pub fn flag(mut self, k: &str, v: bool) -> Self {
        let _ = write!(self.line, ",\"{k}\":{v}");
        self
    }

    pub fn word(mut self, k: &str, v: &str) -> Self {
        let clean: String = v
            .chars()
            .map(|c| if c == ' ' || c == ',' { '_' } else { c })
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
            .take(40)
            .collect();
        let _ = write!(self.line, ",\"{k}\":\"{clean}\"");
        self
    }

    /// An error: its kind and the system's code, never its message (which can name a
    /// file).
    pub fn error(self, e: &std::io::Error) -> Self {
        let s = self.word("kind", &format!("{:?}", e.kind()));
        match e.raw_os_error() {
            Some(c) => s.num("os", c as u64),
            None => s,
        }
    }

    pub fn file(self, p: &Path) -> Self {
        self.num("file", file_id(p))
    }

    pub fn write(mut self) {
        self.line.push('}');
        let Ok(mut g) = LOG.lock() else { return };
        let Some(log) = g.as_mut() else { return };
        if log.tail.len() == TAIL {
            log.tail.pop_front();
        }
        log.tail.push_back(self.line.clone());
        self.line.push('\n');
        let Some(f) = log.file.as_mut() else { return };
        let n = self.line.len() as u64;
        if log.written + n > LIMIT {
            let _ = f.write_all(
                b"{\"ev\":\"limit\",\"note\":\"the log is full; nothing more is written\"}\n",
            );
            log.file = None;
            return;
        }
        // A file that cannot be written any more (disk full): the log ends there, the
        // job goes on.
        if f.write_all(self.line.as_bytes()).is_err() {
            log.file = None;
            return;
        }
        log.written += n;
    }
}

/// Keep only the newest `keep` logs.
fn prune(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("debug-") && n.ends_with(".jsonl"))
        })
        .map(|e| {
            let t = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (t, e.path())
        })
        .collect();
    logs.sort();
    let n = logs.len();
    for (_, p) in logs.into_iter().take(n.saturating_sub(keep)) {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_topics_are_read_from_the_variable() {
        assert_eq!(parse("jobs"), (1, vec![]));
        assert_eq!(parse("Errors"), (2, vec![]));
        assert_eq!(parse("jobs,errors"), (3, vec![]));
        assert_eq!(parse("1"), (3, vec![]));
        assert_eq!(parse("ALL"), (3, vec![]));
        assert_eq!(parse("0"), (0, vec![]));
        assert_eq!(parse(""), (0, vec![]));
        assert_eq!(parse("jobs, speed"), (1, vec!["speed".to_string()]));
    }

    #[test]
    fn a_word_cannot_carry_a_path() {
        let e = Event {
            line: String::new(),
        }
        .word("w", "/home/someone/Secret Report.pdf")
        .word("x", r"C:\Users\someone\a.txt");
        assert_eq!(
            e.line,
            ",\"w\":\"homesomeoneSecret_Report.pdf\",\"x\":\"CUserssomeonea.txt\""
        );
    }

    #[test]
    fn a_file_is_the_same_number_within_a_run() {
        let a = file_id(Path::new("/a/b"));
        assert_eq!(a, file_id(Path::new("/a/b")));
        assert_ne!(a, file_id(Path::new("/a/c")));
        assert!(a < 1 << 48);
    }

    #[test]
    fn an_error_is_its_kind_and_code_never_its_message() {
        let e = std::io::Error::from_raw_os_error(2);
        let line = Event {
            line: String::new(),
        }
        .error(&e)
        .line;
        assert_eq!(line, ",\"kind\":\"NotFound\",\"os\":2");
        let named = std::io::Error::other("/home/someone/x: gone");
        assert!(!Event {
            line: String::new()
        }
        .error(&named)
        .line
        .contains("someone"));
    }

    /// The one test that turns the log on: others in this crate may run meanwhile and
    /// write to it, which only adds lines.
    #[test]
    fn a_log_is_written_kept_small_and_pruned() {
        let dir = std::env::temp_dir().join(format!("xdevcmd-debuglog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..KEEP + 2 {
            std::fs::write(dir.join(format!("debug-{i}-1.jsonl")), "old").unwrap();
        }
        let path = start(1, &dir).unwrap();
        assert!(on(Topic::Jobs));
        assert!(!on(Topic::Errors));
        Event::new("test").num("n", 7).write();
        let (p, lines) = tail().unwrap();
        assert_eq!(p, path);
        assert!(lines
            .iter()
            .any(|l| l.contains("\"ev\":\"test\"") && l.contains("\"n\":7")));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.lines().next().unwrap().contains("\"ev\":\"start\""));
        assert!(text.lines().any(|l| l.contains("\"ev\":\"test\"")));
        let kept = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(kept, KEEP);

        // A copy, logged: its shape and times, and no name of what it copied.
        ON.store(3, Ordering::Relaxed);
        let src = dir.join("Secret-Customer");
        std::fs::create_dir_all(&src).unwrap();
        for i in 0..3 {
            std::fs::write(src.join(format!("Private-{i}.txt")), b"data").unwrap();
        }
        std::fs::write(src.join("Big-Private.bin"), vec![7u8; 2 << 20]).unwrap();
        let out = dir.join("Out-Secret");
        std::fs::create_dir_all(&out).unwrap();
        let h = crate::ops::start(crate::ops::Request::copy(vec![src.clone()], out.clone()));
        let sum = loop {
            if let crate::ops::Event::Done(s) = h.events.recv().unwrap() {
                break s;
            }
        };
        assert_eq!(sum.failed, 0);
        let text = std::fs::read_to_string(&path).unwrap();
        let end = text
            .lines()
            .find(|l| l.contains("\"ev\":\"job_end\"") && l.contains("\"large_files\":1"))
            .expect(&text);
        for k in [
            "\"small_files\":3",
            "\"large_files\":1",
            "\"write_ms\":",
            "\"sync_ms\":",
        ] {
            assert!(end.contains(k), "{k} in {end}");
        }
        assert!(text.contains("\"ev\":\"job_start\""));
        for name in ["Secret", "Private", "Customer", "Out-", "xdevcmd-debuglog"] {
            assert!(!text.contains(name), "{name} in the log:\n{text}");
        }
        stop();
        assert!(!on(Topic::Jobs));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
