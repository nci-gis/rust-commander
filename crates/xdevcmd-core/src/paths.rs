//! Canonical paths a person can read and a shell can use.
//!
//! On Windows `std::fs::canonicalize` returns the verbatim form, `\\?\D:\data` — fine
//! for the API, wrong everywhere else: it showed in pane titles, and `cmd.exe` cannot
//! `cd` into it, so Alt+C, copy path, cd-on-exit, bookmarks and the session all carried
//! it. [`canonical`] resolves the same way and then drops the prefix whenever the plain
//! form means the same file, which is the rule the `dunce` crate follows.
//!
//! Every canonicalisation that reaches a pane, the policy or the user goes through here,
//! so two paths compared with `starts_with` (the root confinement) are in one form.

use std::io;
use std::path::{Path, PathBuf};

/// `std::fs::canonicalize`, without the verbatim prefix on Windows when it can go.
pub fn canonical<P: AsRef<Path>>(p: P) -> io::Result<PathBuf> {
    let c = std::fs::canonicalize(p)?;
    #[cfg(windows)]
    {
        if let Some(plain) = c.to_str().and_then(strip_verbatim) {
            return Ok(PathBuf::from(plain));
        }
    }
    Ok(c)
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether `a` and `b` name one directory entry on disk, whatever the text of each: a
/// link to a parent, `..`, or another case on a case-insensitive drive. Neither path's
/// last part is followed if it is a link (a link and its target are two entries).
/// False when either is missing.
pub fn same_entry(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // std has no stable file index here. The final path names the entry with the
        // case on disk, so two files in a case-sensitive folder stay two. A link is
        // its own entry: its parent's final path and its name, as written.
        let entry = |p: &Path| {
            if std::fs::symlink_metadata(p).ok()?.file_type().is_symlink() {
                let parent = p.parent().filter(|q| !q.as_os_str().is_empty());
                Some(
                    canonical(parent.unwrap_or(Path::new(".")))
                        .ok()?
                        .join(p.file_name()?),
                )
            } else {
                canonical(p).ok()
            }
        };
        match (entry(a), entry(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }
}

/// Whether the directory `dir` is `src_dir` or below it on disk, following links: what
/// a copy or move of `src_dir` into `dir` must refuse, as it would recurse into its own
/// copy. The text of the paths does not matter (a link into `src_dir`, `..`, another
/// case). False when `dir` cannot be resolved.
pub fn within(dir: &Path, src_dir: &Path) -> bool {
    let Ok(mut at) = canonical(dir) else {
        return false;
    };
    #[cfg(unix)]
    let target = {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(src_dir) {
            Ok(m) => (m.dev(), m.ino()),
            Err(_) => return false,
        }
    };
    #[cfg(not(unix))]
    let Ok(target) = canonical(src_dir) else {
        return false;
    };
    loop {
        #[cfg(unix)]
        let hit = {
            use std::os::unix::fs::MetadataExt;
            std::fs::metadata(&at).is_ok_and(|m| (m.dev(), m.ino()) == target)
        };
        #[cfg(not(unix))]
        let hit = at == target;
        if hit {
            return true;
        }
        if !at.pop() {
            return false;
        }
    }
}

/// `p` without a `\\?\` prefix when the plain form means the same file; otherwise
/// `p` as it is. No disk access: for paths xdevcmd kept from before, such as a session
/// or bookmarks saved by a version that did not strip it.
pub fn plain(p: &Path) -> PathBuf {
    p.to_str()
        .and_then(strip_verbatim)
        .map(PathBuf::from)
        .unwrap_or_else(|| p.to_path_buf())
}

/// The plain form of a verbatim Windows path, or `None` when it has none that means the
/// same file: too long for the 260-character limit, a reserved device name, or a part
/// ending in a dot or space (Windows trims those from a plain path, not a verbatim one).
pub fn strip_verbatim(s: &str) -> Option<String> {
    let plain = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        let rest = s.strip_prefix(r"\\?\")?;
        let b = rest.as_bytes();
        if b.len() < 2 || !b[0].is_ascii_alphabetic() || b[1] != b':' {
            return None;
        }
        if b.len() > 2 && b[2] != b'\\' {
            return None;
        }
        rest.to_string()
    };
    if plain.len() >= 260 || plain.contains('/') {
        return None;
    }
    // After "D:" or the leading "\\" of a share.
    for part in plain[2..].split('\\').filter(|p| !p.is_empty()) {
        if part.ends_with('.') || part.ends_with(' ') {
            return None;
        }
        let stem = part.split('.').next().unwrap_or(part);
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return None;
        }
    }
    Some(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_path_loses_the_prefix() {
        assert_eq!(strip_verbatim(r"\\?\D:\"), Some(r"D:\".into()));
        assert_eq!(
            strip_verbatim(r"\\?\C:\Users\me\src"),
            Some(r"C:\Users\me\src".into())
        );
        assert_eq!(strip_verbatim(r"\\?\c:"), Some("c:".into()));
    }

    #[test]
    fn plain_strips_kept_paths_and_leaves_others_alone() {
        assert_eq!(plain(Path::new(r"\\?\D:\var")), PathBuf::from(r"D:\var"));
        assert_eq!(plain(Path::new("/home/x")), PathBuf::from("/home/x"));
        let con = Path::new(r"\\?\C:\CON");
        assert_eq!(
            plain(con),
            con.to_path_buf(),
            "a device name stays verbatim"
        );
    }

    #[test]
    fn a_share_becomes_a_unc_path() {
        assert_eq!(
            strip_verbatim(r"\\?\UNC\server\share\dir"),
            Some(r"\\server\share\dir".into())
        );
    }

    #[test]
    fn it_stays_verbatim_when_the_plain_form_means_something_else() {
        let long = format!(r"\\?\C:\{}", "a".repeat(300));
        assert_eq!(strip_verbatim(&long), None, "past MAX_PATH");
        assert_eq!(strip_verbatim(r"\\?\C:\dir\NUL"), None, "a device");
        assert_eq!(
            strip_verbatim(r"\\?\C:\dir\con.txt"),
            None,
            "a device, extension or not"
        );
        assert_eq!(strip_verbatim(r"\\?\C:\trailing."), None);
        assert_eq!(strip_verbatim(r"\\?\C:\trailing "), None);
        assert_eq!(strip_verbatim(r"\\?\Volume{1234}\x"), None, "not a drive");
        assert_eq!(
            strip_verbatim(r"C:\already\plain"),
            None,
            "nothing to strip"
        );
        assert_eq!(strip_verbatim("/home/me"), None);
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-paths-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("d").join("inner")).unwrap();
        canonical(&d).unwrap()
    }

    #[test]
    fn within_sees_a_folder_below_itself_whatever_the_text() {
        let root = scratch("within");
        let d = root.join("d");
        assert!(within(&d, &d), "itself");
        assert!(within(&d.join("inner"), &d));
        assert!(within(&d.join("inner").join("..").join("inner"), &d));
        assert!(!within(&root, &d), "its parent is not inside it");
        assert!(
            !within(&root.join("missing"), &d),
            "a missing folder is not"
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(d.join("inner"), root.join("link")).unwrap();
            assert!(within(&root.join("link"), &d), "through a link");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn same_entry_sees_one_entry_by_two_texts() {
        let root = scratch("same");
        let d = root.join("d");
        assert!(same_entry(&d, &d.join("inner").join("..")));
        assert!(!same_entry(&d, &d.join("inner")));
        assert!(!same_entry(&d, &root.join("missing")));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("d").join("up")).unwrap();
            assert!(same_entry(&d, &d.join("up").join("d")), "through a link");
            assert!(
                !same_entry(&root, &d.join("up")),
                "a link is not its target"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Another case names the same folder where the drive ignores case: Windows always,
    /// macOS when the volume is case-insensitive (the default), checked here first.
    #[test]
    fn another_case_is_the_same_folder_where_case_is_ignored() {
        let root = scratch("case");
        let d = root.join("d");
        if !root.join("D").exists() {
            let _ = std::fs::remove_dir_all(&root);
            return; // a case-sensitive file system: two names, two folders
        }
        assert!(within(&root.join("D").join("INNER"), &d));
        assert!(same_entry(&root.join("D"), &d));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn on_unix_it_is_canonicalize() {
        let d = std::env::temp_dir();
        assert_eq!(canonical(&d).unwrap(), std::fs::canonicalize(&d).unwrap());
    }
}
