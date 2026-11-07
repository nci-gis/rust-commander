//! What a delete to the trash did, so that it can be undone.
//!
//! Each system keeps its own trash, and none says afterwards which item in it was the
//! one just deleted. So the place is written down at the moment of the delete, as
//! GNOME Files, Dolphin and Explorer do:
//!
//! - Linux: xdevcmd writes the freedesktop trash itself, and knows the file and the
//!   `.trashinfo` it made.
//! - macOS: `NSFileManager.trashItem` hands back the item's new location.
//! - Windows: the Recycle Bin is asked, through the `trash` crate, for the newest
//!   item from that original path deleted no earlier than the delete began.
//!
//! Restoring checks first that the item is still in the trash and its old name still
//! free, and never replaces anything.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trashed {
    /// Where it was.
    pub original: PathBuf,
    /// When the delete began.
    pub when: SystemTime,
    pub place: Place,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A freedesktop trash written by xdevcmd: the item under `files/`, its record
    /// under `info/`.
    Freedesktop { file: PathBuf, info: PathBuf },
    /// Somewhere the system said: macOS's Trash.
    At(PathBuf),
    /// The Windows Recycle Bin, found again by original path and time.
    RecycleBin,
}

fn there(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// Why `t` cannot be put back now, if it cannot; checked before asking.
pub fn check(t: &Trashed) -> Result<(), String> {
    let name = t.original.file_name().map_or_else(
        || t.original.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    if there(&t.original) {
        return Err(format!("{name} exists again where it was"));
    }
    if t.original.parent().is_some_and(|p| !p.is_dir()) {
        return Err(format!("the directory {name} was in is gone"));
    }
    match &t.place {
        Place::Freedesktop { file, .. } | Place::At(file) => {
            if !there(file) {
                return Err(format!("{name} is not in the trash any more"));
            }
        }
        Place::RecycleBin => {}
    }
    Ok(())
}

/// Put `t` back where it was. Refused if its old name is taken; never replaces.
pub fn restore(t: &Trashed) -> io::Result<()> {
    crate::policy::guard_write()?;
    if !crate::policy::get().allows(&t.original) {
        return Err(io::Error::other("outside the root xdevcmd is confined to"));
    }
    check(t).map_err(io::Error::other)?;
    match &t.place {
        Place::Freedesktop { file, info } => {
            crate::noreplace::rename_noreplace(file, &t.original)?;
            let _ = std::fs::remove_file(info);
            Ok(())
        }
        Place::At(file) => crate::noreplace::rename_noreplace(file, &t.original).map_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                io::Error::other(
                    "macOS did not allow it (the Trash is protected): use Put Back in Finder",
                )
            } else {
                e
            }
        }),
        Place::RecycleBin => recycle_bin_restore(t),
    }
}

#[cfg(windows)]
fn recycle_bin_restore(t: &Trashed) -> io::Result<()> {
    use trash::os_limited;
    let since = t
        .when
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        // The Recycle Bin keeps whole seconds; allow for the one the delete began in.
        - 2;
    let items = os_limited::list().map_err(|e| io::Error::other(e.to_string()))?;
    let found = items
        .into_iter()
        .filter(|i| i.original_path() == t.original && i.time_deleted >= since)
        .max_by_key(|i| i.time_deleted)
        .ok_or_else(|| io::Error::other("it is not in the Recycle Bin any more"))?;
    os_limited::restore_all([found]).map_err(|e| io::Error::other(e.to_string()))
}

#[cfg(not(windows))]
fn recycle_bin_restore(_: &Trashed) -> io::Result<()> {
    Err(io::Error::other("there is no Recycle Bin here"))
}

/// macOS: move `path` to the Trash and say where it went.
#[cfg(target_os = "macos")]
pub fn mac_trash(path: &Path) -> io::Result<PathBuf> {
    use objc2_foundation::{NSFileManager, NSURL};
    let url = NSURL::from_file_path(path)
        .ok_or_else(|| io::Error::other("this path cannot be given to the Trash"))?;
    let mut out = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, Some(&mut out))
        .map_err(|e| io::Error::other(e.localizedDescription().to_string()))?;
    out.and_then(|u| u.to_file_path())
        .ok_or_else(|| io::Error::other("the Trash did not say where the item went"))
}
