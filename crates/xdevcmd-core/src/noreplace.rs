//! A rename that never replaces what is at the new name.
//!
//! Checking that a name is free and then renaming leaves a moment in which another
//! program can create that name, and `rename` then overwrites it. Here the system
//! refuses in the same step: `renameat2(RENAME_NOREPLACE)` on Linux,
//! `renameatx_np(RENAME_EXCL)` on macOS. Elsewhere, and on a file system that does not
//! support those (some network and FUSE ones), a file is linked under its new name,
//! which also fails when the name is taken, and the old name removed. Only a directory
//! or a link on such a system falls back to check-then-rename; renaming a directory
//! there can at worst replace an empty directory, never a file.

use std::fs;
use std::io;
use std::path::Path;

/// Rename `from` to `to`, failing with [`io::ErrorKind::AlreadyExists`] if `to` exists.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        use rustix::io::Errno;
        match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
            Ok(()) => return Ok(()),
            // Not supported by this kernel or file system: the fallback below.
            Err(e) if e == Errno::INVAL || e == Errno::NOSYS || e == Errno::NOTSUP => {}
            Err(e) => return Err(e.into()),
        }
    }
    fallback(from, to)
}

fn fallback(from: &Path, to: &Path) -> io::Result<()> {
    let taken = || io::Error::new(io::ErrorKind::AlreadyExists, "the new name is taken");
    if fs::symlink_metadata(to).is_ok() {
        return Err(taken());
    }
    if fs::symlink_metadata(from)?.is_file() {
        match fs::hard_link(from, to) {
            Ok(()) => {
                if let Err(e) = fs::remove_file(from) {
                    // Both names now; put it back as it was.
                    let _ = fs::remove_file(to);
                    return Err(e);
                }
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(taken()),
            // No hard links here (FAT, exFAT, some network shares), or not allowed.
            Err(_) => {}
        }
    }
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("xdevcmd-noreplace-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_rename_onto_a_name_in_use_fails_and_changes_nothing() {
        let d = dir("taken");
        fs::write(d.join("a"), "a").unwrap();
        fs::write(d.join("b"), "b").unwrap();
        let e = rename_noreplace(&d.join("a"), &d.join("b")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(d.join("a")).unwrap(), "a");
        assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "b");
        // A directory, and an empty one in the way, are refused the same.
        fs::create_dir(d.join("da")).unwrap();
        fs::create_dir(d.join("db")).unwrap();
        let e = rename_noreplace(&d.join("da"), &d.join("db")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert!(d.join("da").is_dir());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_rename_to_a_free_name_moves_files_directories_and_links() {
        let d = dir("free");
        fs::write(d.join("a"), "a").unwrap();
        rename_noreplace(&d.join("a"), &d.join("b")).unwrap();
        assert!(!d.join("a").exists());
        assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "a");
        fs::create_dir(d.join("da")).unwrap();
        rename_noreplace(&d.join("da"), &d.join("db")).unwrap();
        assert!(d.join("db").is_dir() && !d.join("da").exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("b", d.join("la")).unwrap();
            rename_noreplace(&d.join("la"), &d.join("lb")).unwrap();
            assert!(fs::symlink_metadata(d.join("lb"))
                .unwrap()
                .file_type()
                .is_symlink());
        }
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_fallback_refuses_a_name_in_use_too() {
        let d = dir("fallback");
        fs::write(d.join("a"), "a").unwrap();
        fs::write(d.join("b"), "b").unwrap();
        let e = fallback(&d.join("a"), &d.join("b")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "b");
        fallback(&d.join("a"), &d.join("c")).unwrap();
        assert_eq!(fs::read_to_string(d.join("c")).unwrap(), "a");
        assert!(!d.join("a").exists());
        fs::remove_dir_all(&d).unwrap();
    }
}
