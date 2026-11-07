//! What xdevcmd is allowed to do to the disk, decided once at startup.
//!
//! The UI hides and refuses what a mode turns off; this is the second wall behind it.
//! The copy/move/delete engine and rename ask here themselves, so a code path in the
//! UI that forgets to check still cannot write. The policy is installed once and can
//! never be loosened while running: `install` only takes effect the first time.
//!
//! This guards against mistakes and against hostile content. It is not a boundary
//! against the person running xdevcmd, who can start it without the flag.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    /// Nothing is created, changed, moved or deleted by xdevcmd.
    pub read_only: bool,
    /// Navigation never leaves this directory. Canonical, so symlinks are resolved.
    pub root: Option<PathBuf>,
}

static POLICY: OnceLock<Policy> = OnceLock::new();

/// Install the policy. Only the first call counts; later ones are ignored, so nothing
/// that runs after startup can relax it.
pub fn install(p: Policy) {
    let _ = POLICY.set(p);
}

pub fn get() -> &'static Policy {
    POLICY.get_or_init(Policy::default)
}

pub fn read_only() -> bool {
    get().read_only
}

/// Err when writing is turned off, worded for the person who tried.
pub fn guard_write() -> std::io::Result<()> {
    if read_only() {
        Err(std::io::Error::other(
            "xdevcmd is in read-only mode \u{2014} nothing is written",
        ))
    } else {
        Ok(())
    }
}

impl Policy {
    /// Whether `path` is inside the root. Resolved through symlinks first, so a link
    /// inside the root that points out of it is outside. A path that cannot be
    /// resolved is outside: when unsure, the answer is no.
    pub fn allows(&self, path: &Path) -> bool {
        let Some(root) = &self.root else {
            return true;
        };
        match crate::paths::canonical(path) {
            Ok(p) => p.starts_with(root),
            Err(_) => false,
        }
    }

    /// `path` if it is allowed, otherwise the root itself.
    pub fn clamp(&self, path: &Path) -> PathBuf {
        match &self.root {
            Some(root) if !self.allows(path) => root.clone(),
            _ => path.to_path_buf(),
        }
    }

    /// True when `dir` is the root: the place with no way up.
    pub fn is_root(&self, dir: &Path) -> bool {
        self.root
            .as_ref()
            .is_some_and(|r| crate::paths::canonical(dir).is_ok_and(|d| &d == r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// One directory per test: they run in parallel, and a shared one was deleted by
    /// one test while another was still asserting inside it.
    fn root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-policy-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("inside/deeper")).unwrap();
        fs::create_dir_all(d.join("outside")).unwrap();
        crate::paths::canonical(&d).unwrap()
    }

    #[test]
    fn no_root_allows_everything() {
        assert!(Policy::default().allows(Path::new("/")));
    }

    #[test]
    fn the_root_and_below_are_allowed_and_nothing_else() {
        let r = root("t1");
        let p = Policy {
            read_only: false,
            root: Some(r.join("inside")),
        };
        assert!(p.allows(&r.join("inside")));
        assert!(p.allows(&r.join("inside/deeper")));
        assert!(!p.allows(&r.join("outside")));
        assert!(!p.allows(&r), "the root's parent is outside");
        assert!(
            !p.allows(&r.join("inside/../outside")),
            ".. is resolved, not trusted"
        );
        assert!(p.is_root(&r.join("inside")));
        assert!(!p.is_root(&r.join("inside/deeper")));
        let _ = fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_root_is_outside() {
        let r = root("t2");
        std::os::unix::fs::symlink(r.join("outside"), r.join("inside/escape")).unwrap();
        let p = Policy {
            read_only: false,
            root: Some(r.join("inside")),
        };
        assert!(
            !p.allows(&r.join("inside/escape")),
            "followed to where it really goes"
        );
        assert_eq!(p.clamp(&r.join("inside/escape")), r.join("inside"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_path_that_cannot_be_resolved_is_refused() {
        let r = root("t3");
        let p = Policy {
            read_only: false,
            root: Some(r.join("inside")),
        };
        assert!(!p.allows(&r.join("inside/missing")));
        let _ = fs::remove_dir_all(&r);
    }
}
