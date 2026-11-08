//! Safe operations' journal: what a job is in the middle of, written before it does it,
//! so that a run ended without warning (`kill -9`, a power cut) can be tidied up and
//! offered again at the next start.
//!
//! One small file per job in the state directory, `<pid>-<n>.journal`, one line per
//! fact: the job (its kind, sources, destination), each directory it makes temporary
//! files in (`dir`, once per directory: their names carry the pid, so a later start finds
//! them there without a line per file), and rename many's temporary names (`ren`, the
//! user's own files under another name) until they are renamed on (`unren`). A job that ends, however it ends while xdevcmd runs,
//! removes its file. A file whose process is gone is what an interrupted run left.
//!
//! Lines are written and flushed, not synced: losing the last lines in a power cut
//! leaves a part file unknown to the journal, never a wrong action on the user's files.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Where journals go: a directory of their own in the state directory. Without it,
/// there is no journal (nothing is recorded, nothing recovered).
pub fn init(dir: PathBuf) {
    let _ = DIR.set(dir);
}

/// A job's journal. Removed when dropped: a job that ends while xdevcmd runs, finished,
/// cancelled or failed, leaves nothing; only a killed process leaves its file.
pub struct Journal {
    path: PathBuf,
    file: File,
    touched: Instant,
    /// Directories already written down.
    dirs: std::collections::HashSet<PathBuf>,
}

/// How often a running job shows it is alive, for systems where a process cannot be
/// asked (Windows): a journal older than [`STALE`] belongs to a run that is gone.
const TOUCH: Duration = Duration::from_secs(20);
#[cfg(not(unix))]
const STALE: Duration = Duration::from_secs(120);

impl Journal {
    /// Start a journal for a job of `kind` ("copy", "move", "delete", "extract", "pack",
    /// "sync", "rename"). `None` when journals are off or the file cannot be made.
    pub fn begin(
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        to_trash: bool,
    ) -> Option<Self> {
        Self::begin_in(DIR.get()?, kind, sources, dest, to_trash)
    }

    fn begin_in(
        dir: &Path,
        kind: &str,
        sources: &[PathBuf],
        dest: Option<&Path>,
        to_trash: bool,
    ) -> Option<Self> {
        static N: AtomicU64 = AtomicU64::new(0);
        fs::create_dir_all(dir).ok()?;
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("{}-{n}.journal", std::process::id()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .ok()?;
        let mut j = Self {
            path,
            file,
            touched: Instant::now(),
            dirs: std::collections::HashSet::new(),
        };
        j.line(&format!("kind {kind}"));
        for s in sources {
            j.line(&format!("src {}", enc(s)));
        }
        if let Some(d) = dest {
            j.line(&format!("dest {}", enc(d)));
        }
        if to_trash {
            j.line("trash");
        }
        Some(j)
    }

    fn line(&mut self, s: &str) {
        let _ = writeln!(self.file, "{s}");
        let _ = self.file.flush();
        self.touched = Instant::now();
    }

    /// About to make `p`, a temporary name: its directory is written down, once.
    pub fn part(&mut self, p: &Path) {
        if let Some(dir) = p.parent() {
            if self.dirs.insert(dir.to_path_buf()) {
                self.line(&format!("dir {}", enc(dir)));
                return;
            }
        }
        self.touch();
    }

    /// `p` is no longer there: renamed into place, or removed. Nothing to write: a later
    /// start only removes what is still there.
    pub fn gone(&mut self, _p: &Path) {
        self.touch();
    }

    /// About to rename `orig` to the temporary name `tmp` (rename many).
    pub fn renamed(&mut self, tmp: &Path, orig: &Path) {
        self.line(&format!("ren {} {}", enc(tmp), enc(orig)));
    }

    /// `tmp` has been renamed on: nothing to put back.
    pub fn unrenamed(&mut self, tmp: &Path) {
        self.line(&format!("unren {}", enc(tmp)));
    }

    /// Show the job is alive, at most every [`TOUCH`]: for a long file with no line to
    /// write.
    pub fn touch(&mut self) {
        if self.touched.elapsed() >= TOUCH {
            let _ = self.file.set_modified(SystemTime::now());
            self.touched = Instant::now();
        }
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// What an interrupted run was doing, found at start-up, after its leftovers were
/// cleaned up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupted {
    pub kind: String,
    pub sources: Vec<PathBuf>,
    pub dest: Option<PathBuf>,
    pub to_trash: bool,
    /// Temporary files of copies removed.
    pub cleaned: usize,
    /// Rename many's files put back under their names.
    pub restored: usize,
    /// Rename many's files that could not be put back (their name is taken): each
    /// (temporary name, name it had).
    pub stranded: Vec<(PathBuf, PathBuf)>,
}

/// Tidy up after runs that ended without warning, and say what they were doing.
pub fn recover() -> Vec<Interrupted> {
    DIR.get().map(|d| recover_in(d)).unwrap_or_default()
}

fn recover_in(dir: &Path) -> Vec<Interrupted> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    let me = std::process::id();
    for e in entries.flatten() {
        let path = e.path();
        let Some(pid) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".journal"))
            .and_then(|n| n.split('-').next())
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me || alive(pid, &path) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        out.push(tidy(&text, pid));
        let _ = fs::remove_file(&path);
    }
    out
}

/// Whether process `pid` still runs. Unix asks the system; elsewhere a journal touched
/// within [`STALE`] counts as alive.
fn alive(pid: u32, journal: &Path) -> bool {
    #[cfg(unix)]
    {
        let _ = journal;
        rustix::process::Pid::from_raw(pid as i32)
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        fs::metadata(journal)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < STALE)
    }
}

/// Undo what one journal, left by process `pid`, says was left half done.
fn tidy(text: &str, pid: u32) -> Interrupted {
    let mut it = Interrupted {
        kind: String::new(),
        sources: Vec::new(),
        dest: None,
        to_trash: false,
        cleaned: 0,
        restored: 0,
        stranded: Vec::new(),
    };
    let mut parts: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut renames: Vec<(PathBuf, PathBuf)> = Vec::new();
    for line in text.lines() {
        let mut w = line.split(' ');
        let tag = w.next().unwrap_or_default();
        let a = w.next().and_then(dec);
        let b = w.next().and_then(dec);
        match (tag, a, b) {
            ("kind", _, _) => it.kind = line[5..].to_string(),
            ("src", Some(p), _) => it.sources.push(p),
            ("dest", Some(p), _) => it.dest = Some(p),
            ("trash", _, _) => it.to_trash = true,
            ("dir", Some(d), _) => dirs.push(d),
            ("part", Some(p), _) => parts.push(p),
            ("gone", Some(p), _) => parts.retain(|x| *x != p),
            ("ren", Some(t), Some(o)) => renames.push((t, o)),
            ("unren", Some(t), _) => renames.retain(|(x, _)| *x != t),
            _ => {}
        }
    }
    // The temporary files that process made: `.name.xdevcmd-part-<pid>-<n>`.
    let mark = format!(".xdevcmd-part-{pid}-");
    for d in dirs {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().contains(&mark) {
                parts.push(e.path());
            }
        }
    }
    for p in parts {
        // Only xdevcmd's own temporary names, whatever a damaged journal says.
        let ours = p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains(".xdevcmd-part-"));
        if ours
            && fs::symlink_metadata(&p).is_ok_and(|m| m.is_file())
            && fs::remove_file(&p).is_ok()
        {
            it.cleaned += 1;
        }
    }
    for (tmp, orig) in renames {
        if fs::symlink_metadata(&tmp).is_err() {
            continue;
        }
        match crate::noreplace::rename_noreplace(&tmp, &orig) {
            Ok(()) => it.restored += 1,
            Err(_) => it.stranded.push((tmp, orig)),
        }
    }
    it
}

/// A path as hex of its bytes (UTF-16 units on Windows): any name, no escaping rules.
fn enc(p: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        p.as_os_str()
            .encode_wide()
            .map(|u| format!("{u:04x}"))
            .collect()
    }
    #[cfg(not(any(unix, windows)))]
    {
        p.to_string_lossy()
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

fn dec(s: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bytes: Option<Vec<u8>> = (0..s.len())
            .step_by(2)
            .map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .collect();
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes?)))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let units: Option<Vec<u16>> = (0..s.len())
            .step_by(4)
            .map(|i| {
                s.get(i..i + 4)
                    .and_then(|h| u16::from_str_radix(h, 16).ok())
            })
            .collect();
        Some(PathBuf::from(std::ffi::OsString::from_wide(&units?)))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let bytes: Option<Vec<u8>> = (0..s.len())
            .step_by(2)
            .map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .collect();
        String::from_utf8(bytes?).ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-journal-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A journal as a dead process would have left it: written under another pid.
    fn orphan(jdir: &Path, text: &str) {
        // A pid no live process has: the largest a 32-bit pid_t allows.
        fs::write(jdir.join("2147483646-0.journal"), text).unwrap();
    }

    #[test]
    fn paths_survive_the_journal_whatever_they_hold() {
        for p in ["/a b/c\nd", "/tài liệu/ảnh.png", "C:\\x"] {
            assert_eq!(dec(&enc(Path::new(p))), Some(PathBuf::from(p)));
        }
    }

    #[test]
    fn a_finished_job_leaves_no_journal_and_a_live_one_is_left_alone() {
        let d = dir("live");
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false).unwrap();
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        // Ours, and running: not recovered.
        assert!(recover_in(&d).is_empty());
        drop(j);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_dead_runs_parts_are_removed_and_renames_put_back() {
        let d = dir("dead");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let part = d.join(".big.iso.xdevcmd-part-99-0");
        let done = d.join(".ok.txt.xdevcmd-part-99-1");
        // Found by its directory and the dead pid in its name, with no line of its own;
        // another live run's part file in the same directory is left alone.
        let by_dir = d.join(".c.bin.xdevcmd-part-2147483646-5");
        let other_run = d.join(".c.bin.xdevcmd-part-1-5");
        fs::write(&by_dir, "x").unwrap();
        fs::write(&other_run, "x").unwrap();
        let user = d.join("not-ours.txt");
        fs::write(&part, "half").unwrap();
        fs::write(&user, "keep").unwrap();
        // Rename many had moved photo.jpg to its temporary name when it died.
        let tmp = d.join(".xdevcmd-rename-99-0");
        fs::write(&tmp, "photo").unwrap();
        let text = format!(
            "kind copy\nsrc {}\ndest {}\ndir {}\npart {}\npart {}\ngone {}\npart {}\nren {} {}\n",
            enc(&d.join("src/big.iso")),
            enc(&d),
            enc(&d),
            enc(&part),
            enc(&done),
            enc(&done),
            enc(&user),
            enc(&tmp),
            enc(&d.join("photo.jpg")),
        );
        orphan(&jdir, &text);
        let got = recover_in(&jdir);
        assert_eq!(got.len(), 1);
        let it = &got[0];
        assert_eq!(it.kind, "copy");
        assert_eq!(it.sources, vec![d.join("src/big.iso")]);
        assert_eq!(it.dest.as_deref(), Some(d.as_path()));
        assert_eq!((it.cleaned, it.restored), (2, 1));
        assert!(!part.exists(), "the part file of the dead run is gone");
        assert!(!by_dir.exists(), "found by its directory");
        assert!(other_run.exists(), "another run's part file is not touched");
        assert!(
            user.exists(),
            "a name that is not xdevcmd's own is never removed"
        );
        assert_eq!(fs::read_to_string(d.join("photo.jpg")).unwrap(), "photo");
        assert_eq!(
            fs::read_dir(&jdir).unwrap().count(),
            0,
            "the journal is gone"
        );
        let _ = fs::remove_dir_all(&d);
    }
}
