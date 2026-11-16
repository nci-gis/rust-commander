//! What is never deleted, moved or renamed, however its path is written (#2, plan A10).
//!
//! A row of a list can come from any text a shell printed: `src/..`, `.`, `~/..`.
//! Removing `x/..` removes `x`'s parent, a folder nobody named. So:
//!
//! - a target whose path is not plain (a `.` or `..` in it, or no name at its end) is
//!   refused outright: no dialog shows such a path for what it is;
//! - the root of a drive, the home folder and every folder above it are refused,
//!   compared both as written (made plain) and as the system resolves them (links, a
//!   junction, `HOME` itself a link), case-insensitively on Windows and macOS.
//!
//! The UI asks before it offers a dialog; the engine asks again before a Delete or a
//! Move starts ([`crate::ops::start`]), for every way in (a drag, a list, an agent).

use std::path::{Component, Path, PathBuf};

/// Why a target is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarded {
    /// The root of a file system or a drive.
    Root,
    /// The home folder, or a folder above it.
    Home,
    /// A path with `.` or `..` in it, or none at its end: not what it seems.
    NotPlain,
}

impl Guarded {
    /// Stable, for the UI's refusal code and an agent.
    pub fn code(self) -> &'static str {
        match self {
            Guarded::Root => "root",
            Guarded::Home => "home",
            Guarded::NotPlain => "not_plain",
        }
    }
}

/// Whether `path` may be deleted, moved or renamed: `None` when it may. `homes` are the
/// home folders to keep (see [`homes`]).
pub fn protected(path: &Path, homes: &[PathBuf]) -> Option<Guarded> {
    if path.parent().is_none() || is_drive_root(path) {
        return Some(Guarded::Root);
    }
    if !plain(path) {
        return Some(Guarded::NotPlain);
    }
    let as_written = guarded(path, homes);
    if as_written.is_some() {
        return as_written;
    }
    // Resolved by the system only when it could matter (a link on the way can make any
    // path the home): when its name is a root's or a home's or one of their parents'.
    // A plain name elsewhere costs nothing.
    if named_like_a_guard(path, homes) {
        if let Ok(real) = crate::paths::canonical(path) {
            return guarded(&real, homes);
        }
    }
    None
}

/// The home folders: `HOME` and, on Windows, `USERPROFILE`, each when absolute, both as
/// set and as the system resolves them.
pub fn homes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for var in ["HOME", "USERPROFILE"] {
        let Some(v) = std::env::var_os(var) else {
            continue;
        };
        let p = PathBuf::from(v);
        if !p.is_absolute() {
            continue;
        }
        let p = made_plain(&p);
        if let Ok(real) = crate::paths::canonical(&p) {
            if !out.contains(&real) {
                out.push(real);
            }
        }
        if !out.contains(&p) {
            out.push(p);
        }
    }
    // HOME unset or relative: the system's own idea of it (red-team of #2).
    if out.is_empty() {
        // Deprecated before Rust 1.86 fixed it on Windows; MSRV 1.90 may still warn.
        #[allow(deprecated)]
        let home = std::env::home_dir();
        if let Some(h) = home.filter(|h| h.is_absolute()) {
            if let Ok(real) = crate::paths::canonical(&h) {
                out.push(real);
            }
            if !out.contains(&h) {
                out.push(h);
            }
        }
    }
    out
}

/// No `.` or `..` in it, a name at its end, and no separator after that name: a
/// trailing slash makes the system follow a link (`link/` is its target), which every
/// check here, on the link's own name, would miss (red-team of #2).
fn plain(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    !text.ends_with('/')
        && !text.ends_with(std::path::MAIN_SEPARATOR)
        && path.file_name().is_some()
        && !path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
}

/// `path` with `.` dropped and `..` taking off the name before it, as written.
fn made_plain(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn guarded(path: &Path, homes: &[PathBuf]) -> Option<Guarded> {
    if path.parent().is_none() || is_drive_root(path) {
        return Some(Guarded::Root);
    }
    homes
        .iter()
        .any(|h| same(h, path) || starts_with(h, path))
        .then_some(Guarded::Home)
}

/// `C:\`, `\\server\share\`: a prefix and nothing after it.
fn is_drive_root(path: &Path) -> bool {
    let mut c = path.components();
    matches!(c.next(), Some(Component::Prefix(_)))
        && matches!(c.next(), None | Some(Component::RootDir))
        && c.next().is_none()
}

/// Whether the last name of `path` is the last name of a home or of a folder above it,
/// so that resolving it could find one.
fn named_like_a_guard(path: &Path, homes: &[PathBuf]) -> bool {
    let Some(name) = path.file_name() else {
        return true;
    };
    homes.iter().any(|h| {
        h.ancestors()
            .filter_map(Path::file_name)
            .any(|n| eq_name(n, name))
    })
}

const FOLD: bool = cfg!(any(windows, target_os = "macos"));

fn eq_name(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    if FOLD {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

fn same(a: &Path, b: &Path) -> bool {
    let (mut x, mut y) = (a.components(), b.components());
    loop {
        match (x.next(), y.next()) {
            (None, None) => return true,
            (Some(p), Some(q)) if eq_name(p.as_os_str(), q.as_os_str()) => {}
            _ => return false,
        }
    }
}

/// Whether `inner` is `outer` or inside it, names compared as [`eq_name`].
fn starts_with(inner: &Path, outer: &Path) -> bool {
    let mut x = inner.components();
    for o in outer.components() {
        match x.next() {
            Some(i) if eq_name(i.as_os_str(), o.as_os_str()) => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\x")
        } else {
            PathBuf::from("/home/x")
        }
    }

    /// The table of plan A10: each way of writing a guarded folder, and plain paths
    /// beside them that stay allowed.
    #[test]
    fn a_guarded_folder_is_refused_however_it_is_written() {
        let h = vec![home()];
        let p = |s: &str| protected(Path::new(s), &h);
        let not_plain = Some(Guarded::NotPlain);
        if cfg!(windows) {
            assert_eq!(p(r"C:\"), Some(Guarded::Root));
            assert_eq!(p(r"C:\Users"), Some(Guarded::Home));
            assert_eq!(p(r"C:\USERS"), Some(Guarded::Home), "names fold on Windows");
            assert_eq!(p(r"C:\Users\x"), Some(Guarded::Home));
            assert_eq!(p(r"C:\Users\X"), Some(Guarded::Home));
            assert_eq!(p(r"C:\Users\x\.."), not_plain);
            assert_eq!(p(r"C:\Users\x\src"), None);
            assert_eq!(p(r"C:\Users\y"), None);
        } else {
            assert_eq!(p("/"), Some(Guarded::Root));
            assert_eq!(p("/home"), Some(Guarded::Home));
            assert_eq!(p("/home/x"), Some(Guarded::Home));
            assert_eq!(p("/home/x/src"), None);
            assert_eq!(p("/home/y"), None);
            assert_eq!(p("/home/x/.."), not_plain);
            // Rust's components drop a `.` after the first: this is the home itself.
            assert_eq!(p("/home/x/."), Some(Guarded::Home));
            assert_eq!(p("/home/x/src/.."), not_plain);
            assert_eq!(p("/home/x/src/../../.."), not_plain);
            assert_eq!(p("/srv/data/src/.."), not_plain, "a folder nobody named");
            assert_eq!(
                p("/HOME/X"),
                if cfg!(target_os = "macos") {
                    Some(Guarded::Home)
                } else {
                    None
                },
                "names fold on macOS only"
            );
        }
    }

    /// Through the file system: a link to the home is the home, and a home set through
    /// a link is guarded under both names.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_guarded_folder_is_refused() {
        let top = std::env::temp_dir().join(format!("xdevcmd-protect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let real = top.join("real-home");
        std::fs::create_dir_all(real.join("src")).unwrap();
        let real = crate::paths::canonical(&real).unwrap();
        std::os::unix::fs::symlink(&real, top.join("linked-home")).unwrap();
        // HOME set through the link, as [`homes`] makes it: both names.
        let h = vec![real.clone(), top.join("linked-home")];
        assert_eq!(protected(&top.join("linked-home"), &h), Some(Guarded::Home));
        // Another link to the same home, by a name the home does not have: resolved
        // only when its name is a guard's, so give it one.
        let other = top.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(&real, other.join("real-home")).unwrap();
        assert_eq!(protected(&other.join("real-home"), &h), Some(Guarded::Home));
        assert_eq!(protected(&real.join("src"), &h), None);
        // A trailing slash goes through a link: `link/` is the home, whatever the link
        // is called (red-team of #2: remove_tree emptied the home through it).
        let data = top.join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::os::unix::fs::symlink(&real, data.join("link")).unwrap();
        let through = PathBuf::from(format!("{}/", data.join("link").display()));
        assert!(protected(&through, &h).is_some(), "{through:?}");
        let _ = std::fs::remove_dir_all(&top);
    }
}
