//! Safe operations' journal: what a job is in the middle of, written before it does it,
//! so that a run ended without warning (`kill -9`, a power cut) can be tidied up and
//! offered again at the next start.
//!
//! One small file per job in the state directory, `<run>-<n>.journal`, one line per
//! fact: the host, the job (its kind, sources, destination), each directory it makes
//! temporary files in (`dir`, once per directory: their names carry the run, so a later
//! start finds them there without a line per file), and rename many's temporary names
//! (`ren`, the user's own files under another name) until they are renamed on (`unren`).
//! A job that ends, however it ends while xdevcmd runs, removes its file.
//!
//! **Telling a dead run from a live one.** A job holds a lock on its journal for as long
//! as it runs, and the system lets go of it when the process ends, however it ends: a
//! journal another start can lock is a dead run's. Unix: `flock`. Windows: the journal
//! is open without sharing write access, so another start's attempt to open it for
//! writing, sharing nothing, fails while the job runs. A date or a bare pid would not do:
//! a job blocked on a slow drive stops touching its file, and a pid is reused. Where the
//! file system has no locks (some network ones), the pid is asked, and only for a
//! journal written on this machine. A journal from another host (a home directory shared
//! over NFS) is always left alone: its job may be running there.
//!
//! **The run** is `<pid>x<start time, hex>`: it names the journal, the part files and
//! rename many's temporary names, so a later process given the same pid never matches a
//! dead run's files, nor a dead run's journal a live process's files.
//!
//! Lines are written and flushed, not synced: losing the last lines in a power cut
//! leaves a part file unknown to the journal, never a wrong action on the user's files.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Where journals go: a directory of their own in the state directory. Without it,
/// there is no journal (nothing is recorded, nothing recovered).
pub fn init(dir: PathBuf) {
    let _ = DIR.set(dir);
}

/// This process's run: `<pid>x<start time in nanoseconds, hex>`. In every temporary
/// name xdevcmd makes, so that only this run's files are ever taken for this run's.
pub fn run_tag() -> &'static str {
    static RUN: OnceLock<String> = OnceLock::new();
    RUN.get_or_init(|| {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{}x{t:x}", std::process::id())
    })
}

/// This machine's name, to tell its journals from another's in a shared home directory.
/// Empty when the system will not say.
pub fn hostname() -> String {
    #[cfg(unix)]
    {
        rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(unix))]
    {
        std::env::var("COMPUTERNAME").unwrap_or_default()
    }
}

/// A job's journal. Removed when dropped: a job that ends while xdevcmd runs, finished,
/// cancelled or failed, leaves nothing; only a killed process leaves its file.
pub struct Journal {
    path: PathBuf,
    /// Open, and locked, for the life of the job: the lock is what says it is alive.
    file: File,
    /// Directories already written down.
    dirs: std::collections::HashSet<PathBuf>,
}

/// What trying a journal's lock found.
#[derive(Debug, PartialEq, Eq)]
enum Lock {
    /// Ours now: the run that wrote it is gone.
    Held,
    /// Someone holds it: the run is alive.
    Busy,
    /// The file system has no locks. Unix only: on Windows the open is the lock.
    #[cfg_attr(not(unix), allow(dead_code))]
    Unsupported,
}

/// Try to take `file`'s lock without waiting.
#[cfg(unix)]
fn try_lock(file: &File) -> Lock {
    use rustix::fs::{flock, FlockOperation};
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Lock::Held,
        Err(e) if e == rustix::io::Errno::WOULDBLOCK => Lock::Busy,
        // ENOLCK, EOPNOTSUPP: a file system without locks. Anything else is no better
        // an answer, and the fallback is the cautious one.
        Err(_) => Lock::Unsupported,
    }
}

/// Open a journal so that, while it is open, nobody else can open it for writing.
fn open_held(path: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_DELETE: readable, and the job can rename and
        // remove its own journal while it holds it; never writable by another.
        o.share_mode(0x1 | 0x4);
    }
    o.open(path)
}

/// Open someone's journal to see whether its run is gone; on Windows, the open itself
/// is the test. `None`: it cannot be opened, or (Windows) its run holds it.
fn open_probe(path: &Path) -> Option<File> {
    let mut o = OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Writing, sharing nothing: refused (a sharing violation) while the job that
        // wrote it has it open, granted once that process is gone.
        o.write(true).share_mode(0);
    }
    o.open(path).ok()
}

/// Whether the run behind a probe opened with [`open_probe`] is gone.
fn probe_lock(file: &File) -> Lock {
    #[cfg(unix)]
    {
        try_lock(file)
    }
    #[cfg(not(unix))]
    {
        // Opened at all, with nothing shared: nobody else has it.
        let _ = file;
        Lock::Held
    }
}

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
        let name = format!("{}-{n}.journal", run_tag());
        // Made under a hidden name and locked before it takes its real one, so no start
        // ever sees a journal by its real name that is not yet locked.
        let new = dir.join(format!(".{name}.new"));
        let file = open_held(&new).ok()?;
        #[cfg(unix)]
        let _ = try_lock(&file);
        let path = dir.join(name);
        if fs::rename(&new, &path).is_err() {
            let _ = fs::remove_file(&new);
            return None;
        }
        let mut j = Self {
            path,
            file,
            dirs: std::collections::HashSet::new(),
        };
        j.line(&format!("host {}", enc(Path::new(&hostname()))));
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
    }

    /// About to make `p`, a temporary name: its directory is written down, once.
    pub fn part(&mut self, p: &Path) {
        if let Some(dir) = p.parent() {
            if self.dirs.insert(dir.to_path_buf()) {
                self.line(&format!("dir {}", enc(dir)));
            }
        }
    }

    /// About to rename `orig` to the temporary name `tmp` (rename many).
    pub fn renamed(&mut self, tmp: &Path, orig: &Path) {
        self.line(&format!("ren {} {}", enc(tmp), enc(orig)));
    }

    /// `tmp` has been renamed on: nothing to put back.
    pub fn unrenamed(&mut self, tmp: &Path) {
        self.line(&format!("unren {}", enc(tmp)));
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        // Removed while still locked (the file closes after this): a start that finds
        // it must never get the lock of a job that ended normally while the name is
        // still there. One that opened it just before is told by its link count.
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
    let host = hostname();
    for e in entries.flatten() {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // A journal still being made: `.<run>-<n>.journal.new`. Empty; removed once
        // its run is gone, left while it lives.
        if let Some(run) = name
            .strip_prefix('.')
            .and_then(|n| n.strip_suffix(".journal.new"))
            .and_then(|n| n.rsplit_once('-'))
            .map(|(run, _)| run)
        {
            if run != run_tag() && gone(&path, run, None) {
                let _ = fs::remove_file(&path);
            }
            continue;
        }
        let Some(run) = name
            .strip_suffix(".journal")
            .filter(|_| !name.starts_with('.'))
            .and_then(|n| n.rsplit_once('-'))
            .map(|(run, _)| run.to_string())
        else {
            continue;
        };
        if run == run_tag() {
            continue;
        }
        let mut text = String::new();
        if !gone(&path, &run, Some((&mut text, &host))) {
            continue;
        }
        out.push(tidy(&text, &run));
        let _ = fs::remove_file(&path);
    }
    out
}

/// Whether the run that wrote the journal at `path` is gone, by its lock. `read`: read
/// the journal into the string, and count it gone only if it was written on this host
/// (named in the second); a journal with no host line is an older one of this machine.
fn gone(path: &Path, run: &str, read: Option<(&mut String, &str)>) -> bool {
    let Some(mut f) = open_probe(path) else {
        return false;
    };
    let lock = probe_lock(&f);
    if lock == Lock::Busy {
        return false;
    }
    // Locked just as its job ended and removed it: not a dead run's.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if f.metadata().map(|m| m.nlink() == 0).unwrap_or(true) {
            return false;
        }
    }
    if let Some((text, host)) = read {
        if f.read_to_string(text).is_err() {
            return false;
        }
        let written_on = text
            .lines()
            .find_map(|l| l.strip_prefix("host "))
            .and_then(dec)
            .map(|p| p.to_string_lossy().into_owned());
        if written_on.is_some_and(|h| h != host) {
            return false;
        }
    }
    match lock {
        Lock::Held => true,
        Lock::Busy => false,
        Lock::Unsupported => !pid_alive(run),
    }
}

/// No locks on this file system: whether the run's process still exists, which a reused
/// pid can make look alive (cautious) but never dead. Unix only; elsewhere the lock is
/// never unsupported.
fn pid_alive(run: &str) -> bool {
    let Some(pid) = run.split('x').next().and_then(|p| p.parse::<u32>().ok()) else {
        return true;
    };
    #[cfg(unix)]
    {
        rustix::process::Pid::from_raw(pid as i32)
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Undo what one journal, left by run `run`, says was left half done.
fn tidy(text: &str, run: &str) -> Interrupted {
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
            // Not `line[5..]`: a journal cut short after "kind" panicked every start.
            ("kind", _, _) => it.kind = line.strip_prefix("kind ").unwrap_or("").to_string(),
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
    // The temporary files that run made: `.name.xdevcmd-part-<run>-<n>`. The whole
    // run, with the dash after it: a pid alone would match another run given it later.
    let mark = format!(".xdevcmd-part-{run}-");
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
    fn a_journal_cut_short_anywhere_or_full_of_garbage_never_panics() {
        let d = dir("prefix");
        let part = d.join(".a.txt.xdevcmd-part-2147483646-0");
        let text = format!(
            "kind copy\nsrc {}\ndest {}\ndir {}\npart {}\ngone {}\nren {} {}\nunren {}\ntrash\n",
            enc(&d.join("a.txt")),
            enc(&d),
            enc(&d),
            enc(&part),
            enc(&part),
            enc(&d.join(".xdevcmd-rename-2147483646-0")),
            enc(&d.join("b.txt")),
            enc(&d.join(".xdevcmd-rename-2147483646-0")),
        );
        // A power cut can stop the file at any byte; the text is ASCII, so every
        // prefix is a str.
        for end in 0..=text.len() {
            tidy(&text[..end], "2147483646");
        }
        for junk in [
            "kind",
            "kind ",
            "src",
            "src zz",
            "src 4",
            "dir",
            "ren 41",
            "ren 41 zz",
            "unren",
            "part",
            "gone",
            "\u{0}",
            " ",
            "  kind copy",
            "trash extra words",
        ] {
            tidy(junk, "2147483646");
            tidy(&format!("{junk}\n{junk}\n"), "2147483646");
        }
        assert_eq!(tidy("kind", "1").kind, "");
        let _ = fs::remove_dir_all(&d);
    }

    /// A run tag no live process has: the largest pid a 32-bit pid_t allows.
    const DEAD: &str = "2147483646x1";

    #[test]
    fn a_journal_held_by_a_running_job_is_left_alone_whatever_pid_it_names() {
        let d = dir("held");
        let j = Journal::begin_in(&d, "copy", &[d.join("a")], Some(&d), false).unwrap();
        // The same lock under a dead pid's name: a pid check would recover it.
        let other = d.join(format!("{DEAD}-0.journal"));
        fs::rename(&j.path, &other).unwrap();
        assert!(recover_in(&d).is_empty(), "held: alive");
        assert!(other.exists());
        drop(j);
        // Unheld now: a dead run's.
        let got = recover_in(&d);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "copy");
        assert!(!other.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_journal_from_another_host_is_left_alone() {
        let d = dir("host");
        let text = format!(
            "host {}\nkind copy\nsrc {}\n",
            enc(Path::new("elsewhere-не-здесь")),
            enc(&d.join("a"))
        );
        let other = d.join(format!("{DEAD}-0.journal"));
        fs::write(&other, text).unwrap();
        assert!(recover_in(&d).is_empty(), "its job may be running there");
        assert!(other.exists());
        // The same, written here: recovered.
        let text = format!(
            "host {}\nkind copy\nsrc {}\n",
            enc(Path::new(&hostname())),
            enc(&d.join("a"))
        );
        fs::write(&other, text).unwrap();
        assert_eq!(recover_in(&d).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_later_run_given_the_same_pid_keeps_its_part_files() {
        let d = dir("same-pid");
        let jdir = d.join("journal");
        fs::create_dir_all(&jdir).unwrap();
        let dead = d.join(format!(".a.xdevcmd-part-{DEAD}-0"));
        let live = d.join(".b.xdevcmd-part-2147483646x2-0");
        fs::write(&dead, "x").unwrap();
        fs::write(&live, "x").unwrap();
        fs::write(
            jdir.join(format!("{DEAD}-0.journal")),
            format!("kind copy\ndir {}\n", enc(&d)),
        )
        .unwrap();
        let got = recover_in(&jdir);
        assert_eq!(got[0].cleaned, 1);
        assert!(!dead.exists(), "the dead run's part file is removed");
        assert!(live.exists(), "same pid, another run: not touched");
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_journal_still_being_made_is_removed_only_once_its_run_is_gone() {
        let d = dir("new");
        let unheld = d.join(format!(".{DEAD}-0.journal.new"));
        let held = d.join(format!(".{DEAD}-1.journal.new"));
        fs::write(&unheld, "").unwrap();
        let f = open_held(&held).unwrap();
        assert_eq!(try_lock(&f), Lock::Held);
        assert!(recover_in(&d).is_empty());
        assert!(!unheld.exists(), "left by a run that died making it");
        assert!(held.exists(), "its run is making it");
        drop(f);
        let _ = fs::remove_dir_all(&d);
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
