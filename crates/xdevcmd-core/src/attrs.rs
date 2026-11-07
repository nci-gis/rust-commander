//! Permissions (Alt+P) and symlinks (Ctrl+Shift+F5) — the two places where Unix and
//! Windows are different enough to need saying.
//!
//! Unix: a mode in octal, `644` or `0755`, applied to each file chosen; nothing is
//! recursive. Windows has ACLs, which a text box cannot honestly edit, so the only
//! thing offered there is the read-only attribute: `ro` or `rw`.

use std::fs;
use std::io;
use std::path::Path;

/// What the permissions box starts with for `path`.
pub fn current(path: &Path) -> io::Result<String> {
    let m = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(format!("{:o}", m.permissions().mode() & 0o7777))
    }
    #[cfg(not(unix))]
    {
        Ok(if m.permissions().readonly() {
            "ro"
        } else {
            "rw"
        }
        .into())
    }
}

/// A line describing `path` for a person: the mode spelled out, and whose it is.
pub fn describe(path: &Path) -> String {
    let Ok(m) = fs::symlink_metadata(path) else {
        return String::new();
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        format!(
            "{}  owner {} group {}",
            rwx(m.permissions().mode()),
            m.uid(),
            m.gid()
        )
    }
    #[cfg(not(unix))]
    {
        format!(
            "read-only attribute {} \u{2014} ro or rw",
            if m.permissions().readonly() {
                "on"
            } else {
                "off"
            }
        )
    }
}

#[cfg(unix)]
fn rwx(mode: u32) -> String {
    let mut s = String::new();
    for (shift, special, mark) in [(6, 0o4000, 's'), (3, 0o2000, 's'), (0, 0o1000, 't')] {
        let bits = (mode >> shift) & 7;
        s.push(if bits & 4 != 0 { 'r' } else { '-' });
        s.push(if bits & 2 != 0 { 'w' } else { '-' });
        s.push(match (bits & 1 != 0, mode & special != 0) {
            (true, true) => mark,
            (false, true) => mark.to_ascii_uppercase(),
            (true, false) => 'x',
            (false, false) => '-',
        });
    }
    s
}

/// Check what was typed, before anything is changed.
pub fn parse(text: &str) -> Result<Setting, String> {
    let t = text.trim();
    #[cfg(unix)]
    {
        if t.is_empty() || t.len() > 4 || !t.chars().all(|c| ('0'..='7').contains(&c)) {
            return Err(format!("{t:?} is not an octal mode like 644 or 0755"));
        }
        u32::from_str_radix(t, 8)
            .map(Setting::Mode)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        match t.to_ascii_lowercase().as_str() {
            "ro" | "readonly" | "read-only" => Ok(Setting::ReadOnly(true)),
            "rw" | "writable" => Ok(Setting::ReadOnly(false)),
            _ => Err(format!("{t:?}: ro or rw")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    #[cfg(unix)]
    Mode(u32),
    #[cfg(not(unix))]
    ReadOnly(bool),
}

pub fn apply(path: &Path, s: Setting) -> io::Result<()> {
    crate::policy::guard_write()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Setting::Mode(m) = s;
        // Never through a symlink: chmod on a link changes what it points at, which
        // may be anywhere.
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(io::Error::other("a symlink has no mode of its own"));
        }
        fs::set_permissions(path, fs::Permissions::from_mode(m))
    }
    #[cfg(not(unix))]
    {
        let Setting::ReadOnly(ro) = s;
        let mut p = fs::metadata(path)?.permissions();
        p.set_readonly(ro);
        fs::set_permissions(path, p)
    }
}

/// Make `link` point at `target`. Never replaces anything at `link`.
pub fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    crate::policy::guard_write()?;
    if fs::symlink_metadata(link).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", link.display()),
        ));
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        // Needs Developer Mode or an elevated process; the error says so if not.
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = target;
        Err(io::Error::other("symlinks are not supported here"))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn modes_round_trip_and_read_like_ls() {
        let d = std::env::temp_dir().join(format!("xdevcmd-attrs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let f = d.join("f");
        fs::write(&f, "x").unwrap();
        apply(&f, parse("640").unwrap()).unwrap();
        assert_eq!(current(&f).unwrap(), "640");
        assert!(describe(&f).starts_with("rw-r-----"), "{}", describe(&f));
        assert_eq!(rwx(0o4755), "rwsr-xr-x");
        assert_eq!(rwx(0o1777), "rwxrwxrwt");
        assert!(parse("9").is_err() && parse("75555").is_err() && parse("").is_err());
        let _ = fs::set_permissions(&f, fs::Permissions::from_mode(0o644));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_symlink_is_made_but_never_over_something() {
        let d = std::env::temp_dir().join(format!("xdevcmd-attrs-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("t"), "T").unwrap();
        symlink(&d.join("t"), &d.join("l")).unwrap();
        assert_eq!(fs::read_to_string(d.join("l")).unwrap(), "T");
        assert!(
            symlink(&d.join("t"), &d.join("t")).is_err(),
            "never replaces"
        );
        assert!(
            apply(&d.join("l"), Setting::Mode(0o777)).is_err(),
            "never chmod through a link"
        );
        let _ = fs::remove_dir_all(&d);
    }
}
