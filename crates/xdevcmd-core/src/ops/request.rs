use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Move,
    Delete,
    /// Out of an archive: `archive`, the entries `inner`, below `base`, into `dest`.
    Extract,
    /// `sources` into a new archive at `dest`, its format from its name.
    Pack,
}

impl Kind {
    pub fn verb(self) -> &'static str {
        match self {
            Kind::Copy => "Copy",
            Kind::Move => "Move",
            Kind::Delete => "Delete",
            Kind::Extract => "Extract",
            Kind::Pack => "Pack",
        }
    }
}

/// What a copy or move does with a file already at the destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Existing {
    /// Ask about each one (the default).
    #[default]
    Ask,
    /// Replace it only when the source is newer; skip it when the two are the same or
    /// the destination is newer. robocopy's and rsync's `--update`.
    Update,
    /// Leave every existing file as it is.
    Skip,
    /// Replace every existing file.
    Replace,
    /// Continuing an interrupted job: skip a file that is already there and identical
    /// (same size, same date within a FAT drive's two seconds), ask about any other.
    /// Not [`Existing::Update`]: a file the user chose to keep in the first run would
    /// be replaced if its source is newer, a decision nobody made.
    Resume,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub kind: Kind,
    pub sources: Vec<PathBuf>,
    /// Destination directory. Required for copy and move, ignored for delete.
    pub dest: Option<PathBuf>,
    /// Delete to the XDG trash instead of unlinking.
    pub to_trash: bool,
    /// Override the trash location. Exists so tests do not touch the real trash.
    pub trash_root: Option<PathBuf>,
    /// Copy or move a single source under this name instead of its own: Shift+F5
    /// copies a file beside itself, which is this with `dest` its own directory.
    pub rename_to: Option<std::ffi::OsString>,
    /// Extract: the archive, the entries chosen in it, and the directory being
    /// browsed inside it ("" at the top), which is taken off each name.
    pub archive: Option<PathBuf>,
    pub inner: Vec<String>,
    pub base: String,
    /// Copy each source into its own directory (synchronising): `(source, into)`.
    /// Directories are created as needed. When set, `sources` and `dest` are unused.
    pub pairs: Vec<(PathBuf, PathBuf)>,
    /// Synchronising: what the confirmed plan expected at each destination (the path a
    /// pair's source is copied to). A destination is replaced without a question only
    /// when it is still the file the plan saw ([`Expect::Replace`]); anything else there
    /// (a file where the plan saw none, one changed since) is asked about.
    pub expect: std::collections::HashMap<PathBuf, Expect>,
    /// Copy and move: what to do with files already at the destination.
    pub existing: Existing,
    /// Files and bytes the confirmed plan already counted (Round 030): the job's
    /// progress bar uses them instead of walking the sources a second time.
    pub planned: Option<(u64, u64)>,
    /// Safe operations for this job, taken from the setting when the request is made
    /// and kept to the end: turning the setting over mid-job must not split one job
    /// between two ways of writing, and tests set it here rather than in a global
    /// other tests read.
    pub safe: bool,
    /// The platform's fast copy for this job (item 22, `[general] fast_copy`), taken
    /// as `safe` is.
    pub fast: bool,
    /// The waits between tries of a file that failed with an error that passes (a
    /// network one): [`BACKOFF_SECS`]; then the user is asked. Tests shorten them.
    pub backoff: Vec<std::time::Duration>,
    /// Bytes between a big file's checkpoints ([`CHECKPOINT`]; 0: none). Tests make it
    /// small.
    pub checkpoint: u64,
    /// Resume: big files an interrupted run had copied part of
    /// ([`crate::journal::Interrupted::partial`]), each taken over where it stopped.
    pub partial: Vec<crate::journal::Partial>,
    /// Resume: the interrupted run's journal, which this job's own replaces.
    pub resumed: Option<PathBuf>,
    /// Resume: source folders the interrupted run finished
    /// ([`crate::journal::Interrupted::done`]): neither counted nor walked again while
    /// their copy is still there.
    pub done_dirs: std::collections::HashSet<PathBuf>,
    /// Resume, checked (B2, owner 2026-11-15): trust nothing the interrupted run made.
    /// Finished folders are walked again, and a file at the destination that looks
    /// identical (size and date) is read with its source and copied again if the two
    /// differ: after a power cut, a new file can have the right size and no data.
    /// Slow (it reads both sides of everything copied), so off unless asked for.
    pub verify: bool,
}

/// The owner's decision 2, Round 031: five tries more, about 30 s in all, then the
/// question.
pub const BACKOFF_SECS: [u64; 5] = [1, 2, 4, 8, 15];

pub(super) fn backoff() -> Vec<std::time::Duration> {
    BACKOFF_SECS
        .iter()
        .map(|s| std::time::Duration::from_secs(*s))
        .collect()
}

/// Whether `e` is an error that may pass if tried again: the network's (a timeout, a
/// lost connection, a stale handle). An I/O error only on a network target: on a local
/// disk it is the disk failing, and trying again is wrong.
pub fn transient(e: &io::Error, network: bool) -> bool {
    let Some(code) = e.raw_os_error() else {
        return false;
    };
    #[cfg(unix)]
    {
        use rustix::io::Errno;
        let n = Errno::from_raw_os_error(code);
        [
            Errno::TIMEDOUT,
            Errno::NOTCONN,
            Errno::HOSTDOWN,
            Errno::HOSTUNREACH,
            Errno::CONNRESET,
            Errno::CONNABORTED,
            Errno::NETDOWN,
            Errno::NETUNREACH,
            Errno::NETRESET,
            Errno::STALE,
        ]
        .contains(&n)
            || network && n == Errno::IO
    }
    #[cfg(windows)]
    {
        // BAD_NETPATH, UNEXP_NET_ERR, NETNAME_DELETED, BAD_NET_NAME, SEM_TIMEOUT,
        // NETWORK_UNREACHABLE, CONNECTION_ABORTED, CONNECTION_INVALID.
        let _ = network;
        matches!(code, 53 | 59 | 64 | 67 | 121 | 1231 | 1236 | 1229)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (code, network);
        false
    }
}

impl Request {
    pub fn extract(archive: PathBuf, inner: Vec<String>, base: String, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Extract,
            sources: vec![archive.clone()],
            dest: Some(dest),
            archive: Some(archive),
            inner,
            base,
            ..Self::delete(Vec::new(), false)
        }
    }
    /// Copy each `(source, into, expected)`, replacing what the confirmed plan said it
    /// would, and only while it is what the plan saw.
    pub fn sync(pairs: Vec<(PathBuf, PathBuf, Expect)>) -> Self {
        let expect = pairs
            .iter()
            .filter_map(|(src, into, e)| Some((into.join(src.file_name()?), *e)))
            .collect();
        Self {
            kind: Kind::Copy,
            pairs: pairs.into_iter().map(|(s, i, _)| (s, i)).collect(),
            expect,
            ..Self::delete(Vec::new(), false)
        }
    }
    /// `dest` is the archive file to create.
    pub fn pack(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Pack,
            sources,
            dest: Some(dest),
            ..Self::delete(Vec::new(), false)
        }
    }

    pub fn copy(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Copy,
            sources,
            dest: Some(dest),
            to_trash: false,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
            fast: crate::fastcopy::enabled(),
            backoff: backoff(),
            checkpoint: CHECKPOINT,
            partial: Vec::new(),
            resumed: None,
            done_dirs: Default::default(),
            verify: false,
        }
    }
    pub fn mv(sources: Vec<PathBuf>, dest: PathBuf) -> Self {
        Self {
            kind: Kind::Move,
            sources,
            dest: Some(dest),
            to_trash: false,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
            fast: crate::fastcopy::enabled(),
            backoff: backoff(),
            checkpoint: CHECKPOINT,
            partial: Vec::new(),
            resumed: None,
            done_dirs: Default::default(),
            verify: false,
        }
    }
    pub fn delete(targets: Vec<PathBuf>, to_trash: bool) -> Self {
        Self {
            kind: Kind::Delete,
            sources: targets,
            dest: None,
            to_trash,
            trash_root: None,
            rename_to: None,
            archive: None,
            inner: Vec::new(),
            base: String::new(),
            pairs: Vec::new(),
            expect: Default::default(),
            existing: Existing::Ask,
            planned: None,
            safe: safe(),
            fast: crate::fastcopy::enabled(),
            backoff: backoff(),
            checkpoint: CHECKPOINT,
            partial: Vec::new(),
            resumed: None,
            done_dirs: Default::default(),
            verify: false,
        }
    }
}

/// Which file a decision was about: enough to tell, right before the step it allowed,
/// that the file there is still that one. Kind, size and modification time; on Unix the
/// device and inode too, on Windows the creation time (std has no stable file index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub(super) kind: u8,
    pub(super) len: u64,
    pub(super) modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    pub(super) node: (u64, u64),
    #[cfg(not(unix))]
    pub(super) created: Option<std::time::SystemTime>,
}

impl Ident {
    pub fn of(m: &fs::Metadata) -> Self {
        let t = m.file_type();
        Self {
            kind: if t.is_symlink() {
                2
            } else if t.is_dir() {
                1
            } else if t.is_file() {
                0
            } else {
                3
            },
            len: m.len(),
            modified: m.modified().ok(),
            #[cfg(unix)]
            node: {
                use std::os::unix::fs::MetadataExt;
                (m.dev(), m.ino())
            },
            #[cfg(not(unix))]
            created: m.created().ok(),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == 1
    }

    /// What is at `p` now (a link as itself), if anything.
    pub fn at(p: &Path) -> Option<Self> {
        fs::symlink_metadata(p).ok().map(|m| Self::of(&m))
    }

    /// As one word for a journal line (no spaces): read back by [`Ident::decode`] on the
    /// same system. A time before 1970 is written as none, so it never matches again:
    /// the file is then asked about, which is the safe side.
    pub fn encode(&self) -> String {
        let t = |t: Option<std::time::SystemTime>| {
            t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or("-".to_string(), |d| {
                    format!("{}.{}", d.as_secs(), d.subsec_nanos())
                })
        };
        #[cfg(unix)]
        let rest = format!("{}:{}", self.node.0, self.node.1);
        #[cfg(not(unix))]
        let rest = t(self.created);
        format!("i1:{}:{}:{}:{rest}", self.kind, self.len, t(self.modified))
    }

    pub fn decode(s: &str) -> Option<Self> {
        let t = |s: &str| -> Option<Option<std::time::SystemTime>> {
            if s == "-" {
                return Some(None);
            }
            let (a, b) = s.split_once('.')?;
            let d = std::time::Duration::new(a.parse().ok()?, b.parse().ok()?);
            Some(Some(std::time::UNIX_EPOCH + d))
        };
        let mut w = s.split(':');
        if w.next()? != "i1" {
            return None;
        }
        let kind = w.next()?.parse().ok()?;
        let len = w.next()?.parse().ok()?;
        let modified = t(w.next()?)?;
        #[cfg(unix)]
        let id = Self {
            kind,
            len,
            modified,
            node: (w.next()?.parse().ok()?, w.next()?.parse().ok()?),
        };
        #[cfg(not(unix))]
        let id = Self {
            kind,
            len,
            modified,
            created: t(w.next()?)?,
        };
        Some(id)
    }
}

/// What a synchronisation plan saw at a destination when the user confirmed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Nothing: the copy is new there.
    Absent,
    /// This file, which the copy replaces.
    Replace(Ident),
    /// Whatever is there is left as it is (a planned skip: the user's choice for its
    /// group, or a row the plan found cannot be done).
    Skip,
}

/// Something a copy made, for undoing it (Round 031): a file, link or directory that
/// was not there before, as it was right after the copy made it. `old`: the file it
/// replaced, put in the trash on the way; undoing puts it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub path: PathBuf,
    pub ident: Ident,
    pub old: Option<crate::trashed::Trashed>,
}

/// The most a copy remembers for its undo, as the plan's own cap: past it the copy
/// cannot be undone, and says so.
pub const MADE_CAP: usize = crate::plan::CAP;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// The destination already exists. `changed`: it is not the file the user agreed
    /// to replace, or the one the plan saw (replaced, or made, since): asked again.
    Conflict {
        path: PathBuf,
        existing_bytes: u64,
        incoming_bytes: u64,
        changed: bool,
    },
    /// A move would cross a filesystem boundary, so it becomes a copy then a delete.
    CrossDevice { path: PathBuf },
    /// The trash is on another filesystem, so the only way to honour the key is to
    /// delete for real. Never assumed.
    TrashUnavailable { path: PathBuf, reason: String },
    /// Something went wrong and the user decides whether it is fatal.
    Failed { path: PathBuf, error: String },
    /// Safe operations' journal cannot be written (`error`): at the start, or before a
    /// step that cannot be undone. [`Answer::Yes`] goes on without it for the rest of the
    /// job; anything else stops the job there. Asked once per job.
    NoJournal { error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Yes for this one: overwrite, cross the device, delete for real.
    Yes,
    /// Yes, and stop asking.
    YesToAll,
    Skip,
    SkipAll,
    /// From here on: replace a destination only when the source is newer, skip the rest.
    Update,
    Retry,
    Abort,
}
