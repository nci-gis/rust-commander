use super::*;

impl Worker {
    // ── delete ─────────────────────────────────────────────────────────────────────

    pub(super) fn delete(&mut self, target: &Path, to_trash: bool, trash_root: Option<&Path>) {
        self.phase = "delete";
        if self.stopped() {
            return;
        }
        if to_trash {
            match send_to_trash(target, trash_root) {
                Ok(t) => {
                    self.sum.done += 1;
                    self.sum.trashed.push(t);
                    return;
                }
                Err(e) => {
                    let answer = if self.trash_all {
                        Answer::Yes
                    } else {
                        self.ask(Question::TrashUnavailable {
                            path: target.to_path_buf(),
                            reason: e.to_string(),
                        })
                    };
                    match answer {
                        Answer::Yes => {}
                        Answer::YesToAll => self.trash_all = true,
                        Answer::Skip | Answer::SkipAll => {
                            self.sum.skipped += 1;
                            return;
                        }
                        _ => {
                            self.abort = true;
                            return;
                        }
                    }
                }
            }
        }
        match remove_tree(target) {
            Ok(()) => self.sum.done += 1,
            Err(e) => self.note_error(target, &e),
        }
    }
}

pub fn remove_tree(p: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(p)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
}

/// `[general] safe_operations`: every copy writes under a temporary name and renames at
/// the end, and jobs keep a journal (journal.rs) so a run killed outright is tidied up
/// at the next start. Off, a new file is written under its own name, as before; a
/// replacement goes through a temporary name either way. Read once per job, into
/// [`Request::safe`], never by a running worker.
pub(super) static SAFE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_safe(on: bool) {
    SAFE.store(on, Ordering::Relaxed);
}

pub fn safe() -> bool {
    SAFE.load(Ordering::Relaxed)
}

/// Whether a directory with `m`'s permissions refuses new names in it: no write bit
/// for its owner on Unix; on Windows read-only does not stop that for a directory.
pub(super) fn denies_writing(m: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o200 == 0
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        false
    }
}

/// Remove the file `p`. On Windows a read-only file cannot be removed until its
/// read-only attribute is cleared: a moved read-only source, or a read-only copy that
/// failed its sync, would otherwise stay.
pub(super) fn remove_file_anyway(p: &Path) -> io::Result<()> {
    let r = fs::remove_file(p);
    #[cfg(windows)]
    if let Err(e) = &r {
        if e.kind() == io::ErrorKind::PermissionDenied {
            if let Ok(m) = fs::symlink_metadata(p) {
                let mut perm = m.permissions();
                if perm.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    if fs::set_permissions(p, perm).is_ok() {
                        return fs::remove_file(p);
                    }
                }
            }
        }
    }
    r
}

/// Move one entry to wherever this platform keeps deleted files, and say where it went.
pub fn send_to_trash(path: &Path, root: Option<&Path>) -> io::Result<crate::trashed::Trashed> {
    if let Some(base) = root {
        // Tests pass a temp directory here, so a test run never puts anything in the
        // real trash of whoever happens to be running it.
        return trash_into(base, path, None);
    }
    #[cfg(target_os = "linux")]
    {
        let home = default_trash_base()
            .ok_or_else(|| io::Error::other("no HOME, so no trash directory"))?;
        match volume_trash(path, &home)? {
            None => trash_into(&home, path, None),
            Some((base, topdir)) => trash_into(&base, path, Some(&topdir)),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use crate::trashed::{Place, Trashed};
        let when = std::time::SystemTime::now();
        let at = crate::trashed::mac_trash(path)?;
        Ok(Trashed {
            original: path.to_path_buf(),
            when,
            place: Place::At(at),
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        use crate::trashed::{Place, Trashed};
        let when = std::time::SystemTime::now();
        ::trash::delete(path).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Trashed {
            original: path.to_path_buf(),
            when,
            place: Place::RecycleBin,
        })
    }
}

/// The freedesktop trash for a path on another file system than the home trash: the
/// spec's `$topdir/.Trash/$uid` when an administrator made `.Trash` (a real, sticky
/// directory), else `$topdir/.Trash-$uid`, made here, private. `None` for the same file
/// system as `home`, whose trash is used.
#[cfg(target_os = "linux")]
pub(super) fn volume_trash(path: &Path, home: &Path) -> io::Result<Option<(PathBuf, PathBuf)>> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let dir = path.parent().unwrap_or(Path::new("/"));
    let dev = fs::metadata(dir)?.dev();
    // The home trash may not exist yet: its nearest existing ancestor decides.
    let home_dev = home
        .ancestors()
        .find_map(|a| fs::metadata(a).ok())
        .map(|m| m.dev());
    if home_dev == Some(dev) {
        return Ok(None);
    }
    // The top of the mount: the last ancestor still on this device.
    let mut topdir = dir.to_path_buf();
    while let Some(up) = topdir.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => topdir = up.to_path_buf(),
            _ => break,
        }
    }
    let uid = rustix::process::getuid().as_raw();
    let shared = topdir.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&shared) {
        let sticky = m.permissions().mode() & 0o1000 != 0;
        if m.is_dir() && !m.file_type().is_symlink() && sticky {
            let mine = shared.join(uid.to_string());
            if fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&mine)
                .is_ok()
            {
                return Ok(Some((mine, topdir)));
            }
        }
    }
    let own = topdir.join(format!(".Trash-{uid}"));
    if let Ok(m) = fs::symlink_metadata(&own) {
        if !m.is_dir() || m.file_type().is_symlink() || m.uid() != uid {
            return Err(io::Error::other(format!(
                "{} is not a trash directory of yours",
                own.display()
            )));
        }
    } else {
        fs::DirBuilder::new().mode(0o700).create(&own)?;
    }
    Ok(Some((own, topdir)))
}

/// Whether a file in the directory `dir` can go to a trash on its own file system:
/// the home trash on the same one, or one [`volume_trash`] would use or may make at the
/// top of its mount. Reads only: nothing is made here (the planner asks it).
#[cfg(target_os = "linux")]
pub(crate) fn volume_trash_usable(dir: &Path) -> Option<bool> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let home = default_trash_base()?;
    let dev = fs::metadata(dir).ok()?.dev();
    let home_dev = home
        .ancestors()
        .find_map(|a| fs::metadata(a).ok())
        .map(|m| m.dev());
    if home_dev == Some(dev) {
        return Some(true);
    }
    let mut topdir = dir.to_path_buf();
    while let Some(up) = topdir.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => topdir = up.to_path_buf(),
            _ => break,
        }
    }
    let uid = rustix::process::getuid().as_raw();
    let usable_dir = |p: &Path| {
        rustix::fs::access(
            p,
            rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
        )
        .is_ok()
    };
    let shared = topdir.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&shared) {
        let sticky = m.permissions().mode() & 0o1000 != 0;
        if m.is_dir() && !m.file_type().is_symlink() && sticky {
            let mine = shared.join(uid.to_string());
            if mine.is_dir() && usable_dir(&mine) || !mine.exists() && usable_dir(&shared) {
                return Some(true);
            }
        }
    }
    let own = topdir.join(format!(".Trash-{uid}"));
    Some(match fs::symlink_metadata(&own) {
        Ok(m) => m.is_dir() && !m.file_type().is_symlink() && m.uid() == uid,
        Err(_) => usable_dir(&topdir),
    })
}

/// Where deleted files go on Linux: `$XDG_DATA_HOME/Trash`, per the freedesktop spec.
pub fn default_trash_base() -> Option<PathBuf> {
    crate::dirs::data_home().map(|d| d.join("Trash"))
}

/// Move one entry into a freedesktop trash directory, with the `.trashinfo` that
/// makes it restorable.
///
/// As the spec says, the `.trashinfo` is made first (with `O_EXCL`, which also reserves
/// the name), then the entry moved: a crash in between leaves a record without a file,
/// which tools ignore, never a file nobody can restore. `topdir`: a per-volume trash,
/// whose records give the path relative to the volume's top.
pub(super) fn trash_into(
    base: &Path,
    path: &Path,
    topdir: Option<&Path>,
) -> io::Result<crate::trashed::Trashed> {
    use crate::trashed::{Place, Trashed};
    let when = std::time::SystemTime::now();
    let files = base.join("files");
    let info = base.join("info");
    fs::create_dir_all(&files)?;
    fs::create_dir_all(&info)?;

    // The path of the entry itself, not of what a link points to: the directory is
    // made canonical, the name kept.
    let name_os = path
        .file_name()
        .ok_or_else(|| io::Error::other("that has no name to put in the trash"))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let original = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .join(name_os);
    let recorded = match topdir.and_then(|t| original.strip_prefix(t).ok()) {
        Some(rel) => rel.to_path_buf(),
        None => original.clone(),
    };
    let stem = name_os.to_string_lossy().into_owned();
    let stamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(&recorded.to_string_lossy()),
        stamp
    );

    // The first free name: its record reserved, then the entry moved in the same step
    // as the name is checked, so nothing in the trash is ever overwritten.
    let mut name = stem.clone();
    let mut n = 1;
    loop {
        let record = info.join(format!("{name}.trashinfo"));
        let reserved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&record);
        match reserved {
            Ok(mut f) => {
                f.write_all(body.as_bytes())?;
                drop(f);
                match crate::noreplace::rename_noreplace(path, &files.join(&name)) {
                    Ok(()) => {
                        return Ok(Trashed {
                            original: path.to_path_buf(),
                            when,
                            place: Place::Freedesktop {
                                file: files.join(&name),
                                info: record,
                            },
                        });
                    }
                    Err(e) => {
                        let _ = fs::remove_file(&record);
                        if e.kind() != io::ErrorKind::AlreadyExists {
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        name = format!("{stem}.{n}");
        n += 1;
    }
}

/// Percent-encoding as the trash spec wants it: path separators stay readable,
/// everything outside the unreserved set is escaped.
pub(super) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
