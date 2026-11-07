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

    #[cfg(unix)]
    #[test]
    fn on_unix_it_is_canonicalize() {
        let d = std::env::temp_dir();
        assert_eq!(canonical(&d).unwrap(), std::fs::canonicalize(&d).unwrap());
    }
}
