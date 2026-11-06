//! What was open at the last quit, so the next start picks up where it left off: the
//! file panes and how they are split, every tab and its directory, the focus, and the
//! terminal — shown or not, how tall, and where each of its tabs was.
//!
//! Stored in `session.toml` beside other per-user state, never in the config: the
//! config is what you chose, this is what you happened to leave open.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    /// Which file pane had focus, as its position in left-to-right, top-to-bottom order.
    pub focus: usize,
    pub terminal_focused: bool,
    pub panes: Saved,
    pub terminal: SavedTerminal,
}

/// The file panes' layout. The terminal is not in here: it always sits along the
/// bottom, so its height is all there is to remember about where it goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Saved {
    Pane {
        active: usize,
        tabs: Vec<SavedTab>,
    },
    Split {
        /// "cols" (side by side) or "rows" (one above the other).
        axis: String,
        pct: u16,
        a: Box<Saved>,
        b: Box<Saved>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub path: String,
    pub sort: String,
    pub reverse: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SavedTerminal {
    pub shown: bool,
    pub height_pct: u16,
    /// Each tab's directory where the system says (Linux); empty where it does not.
    pub tabs: Vec<String>,
}

impl Saved {
    pub fn pane_count(&self) -> usize {
        match self {
            Saved::Pane { .. } => 1,
            Saved::Split { a, b, .. } => a.pane_count() + b.pane_count(),
        }
    }
}

/// `$RCMD_STATE`, then `$XDG_STATE_HOME/rcmd`, `%LOCALAPPDATA%\rcmd` on Windows, then
/// `~/.local/state/rcmd`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RCMD_STATE").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(x) = std::env::var_os("XDG_STATE_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(x).join("rcmd").join("session.toml"));
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("LOCALAPPDATA") {
            return Some(PathBuf::from(a).join("rcmd").join("session.toml"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/rcmd/session.toml"))
}

/// Read the last session. Anything wrong — missing, unreadable, a version from some
/// other rcmd — is `None`, and the start is a fresh one. A session is a convenience;
/// it must never be the reason rcmd does not open.
pub fn load(path: &Path) -> Option<Session> {
    let text = std::fs::read_to_string(path).ok()?;
    let s: Session = toml::from_str(&text).ok()?;
    (s.version == VERSION && s.panes.pane_count() > 0).then_some(s)
}

/// Write the session so that a crash or a full disk halfway through leaves the old
/// one intact: write beside it, then rename over it. Owner-only, since it lists the
/// directories you work in.
pub fn save(path: &Path, s: &Session) -> std::io::Result<()> {
    let text = toml::to_string_pretty(s).map_err(std::io::Error::other)?;
    write_private(path, text.as_bytes())
}

/// Write `bytes` to `path` so that a crash halfway leaves the old file intact, and so
/// that nobody else can read it at any moment — the file is created owner-only rather
/// than created and then narrowed, which would leave a window where it was not.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    // One temporary name per process: two rcmd windows saving at once must not write
    // into the same file.
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    let tmp = path.with_file_name(name);
    // A leftover from a crash keeps whatever mode it had if merely truncated; start
    // from nothing so the mode below is the one it gets.
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path)
}

/// Where rcmd keeps state other than the session: beside it.
pub fn state_file(name: &str) -> Option<PathBuf> {
    default_path().map(|p| p.with_file_name(name))
}

/// The directory to open for a saved path: the path itself, or the nearest ancestor
/// that still exists, or `fallback`. A deleted or unmounted directory should not
/// cost the whole session.
pub fn resolve_dir(saved: &str, fallback: &Path) -> PathBuf {
    let mut p = PathBuf::from(saved);
    loop {
        if p.is_dir() {
            return p;
        }
        if !p.pop() {
            return fallback.to_path_buf();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Session {
        let tab = |p: &str| SavedTab {
            path: p.into(),
            sort: "name".into(),
            reverse: false,
            hidden: false,
        };
        Session {
            version: VERSION,
            focus: 1,
            terminal_focused: false,
            panes: Saved::Split {
                axis: "cols".into(),
                pct: 40,
                a: Box::new(Saved::Pane {
                    active: 1,
                    tabs: vec![tab("/tmp"), tab("/usr")],
                }),
                b: Box::new(Saved::Pane {
                    active: 0,
                    tabs: vec![tab("/var")],
                }),
            },
            terminal: SavedTerminal {
                shown: true,
                height_pct: 35,
                tabs: vec!["/tmp".into(), String::new()],
            },
        }
    }

    #[test]
    fn a_session_survives_the_round_trip_through_disk() {
        let dir = std::env::temp_dir().join(format!("rcmd-session-{}", std::process::id()));
        let path = dir.join("nested/session.toml");
        save(&path, &sample()).unwrap();
        assert_eq!(load(&path), Some(sample()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "it lists your directories; owner only");
        }
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp file left behind: {leftovers:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_or_foreign_session_is_ignored_not_fatal() {
        let dir = std::env::temp_dir().join(format!("rcmd-session-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("session.toml");
        std::fs::write(&p, "this is not toml [").unwrap();
        assert_eq!(load(&p), None);
        let mut s = sample();
        s.version = 99;
        std::fs::write(&p, toml::to_string(&s).unwrap()).unwrap();
        assert_eq!(
            load(&p),
            None,
            "a session from another version is not guessed at"
        );
        assert_eq!(load(&dir.join("missing.toml")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vanished_directory_falls_back_to_its_nearest_parent() {
        let root = std::env::temp_dir().join(format!("rcmd-resolve-{}", std::process::id()));
        std::fs::create_dir_all(root.join("kept")).unwrap();
        let gone = root.join("kept/deleted/deeper");
        assert_eq!(
            resolve_dir(&gone.to_string_lossy(), Path::new("/")),
            root.join("kept")
        );
        assert_eq!(
            resolve_dir("", Path::new("/fallback")),
            PathBuf::from("/fallback")
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
