use super::*;

/// The one way xdevcmd replaces a file (Round 029). `tmp`, complete and still open as
/// `f`, is synced to the disk (a close error, or an NFS or SMB write that failed, is told
/// here, not lost when the file is dropped), then `ready` runs (the journal's line
/// synced, the target looked at once more) and may stop it with `Ok(false)`, and only
/// then is `tmp` renamed over `dst`. Without the sync, a power cut after the rename can
/// leave `dst` empty or short where ext4's defaults do not happen to save it.
///
/// Whatever does not end in the rename removes `tmp`, and `dst` is as it was. A sync
/// that failed is never tried again on the same file: Linux may have dropped the data
/// and cleared the error ("fsyncgate", PostgreSQL 2018), so the caller writes the file
/// again from its source.
pub(crate) fn commit_replace(
    f: File,
    tmp: &Path,
    dst: &Path,
    ready: impl FnOnce() -> io::Result<bool>,
) -> io::Result<bool> {
    let r = (|| {
        crate::faults::sync_data(&f, tmp)?;
        drop(f);
        if !ready()? {
            return Ok(false);
        }
        crate::faults::at(crate::faults::Point::Rename, dst)?;
        fs::rename(tmp, dst)?;
        Ok(true)
    })();
    if !matches!(r, Ok(true)) {
        let _ = fs::remove_file(tmp);
    }
    r
}

/// A temporary name beside `dst` to write a replacement under: hidden, and unique to
/// this run (pid and start time, [`crate::journal::run_tag`]) and call, so two jobs
/// never share one and a later process given the same pid never takes it for its own.
pub(crate) fn part_name(dst: &Path) -> PathBuf {
    let n = PART_SEQ.fetch_add(1, Ordering::Relaxed);
    let tail = format!(".xdevcmd-part-{}-{n}", crate::journal::run_tag());
    let name = dst
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    // The run and counter make the name unique; the original name is only there for a
    // person who finds a leftover. Cut it so the whole name fits the 255-byte limit most
    // file systems have (a lossy non-UTF-8 name can grow threefold). Bytes are never
    // fewer than UTF-16 units, so this also keeps Windows' 255 units.
    let room = NAME_MAX.saturating_sub(1 + tail.len());
    let mut end = name.len().min(room);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    let dir = dst.parent().unwrap_or(Path::new("."));
    dir.join(format!(".{}{tail}", &name[..end]))
}

/// The counter in [`part_name`]: unique per call within a run.
pub(super) static PART_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The longest file name, in bytes, on the common file systems (ext4, APFS, Btrfs, XFS;
/// NTFS counts 255 UTF-16 units).
pub(super) const NAME_MAX: usize = 255;

/// The folders a job told its journal it finished, for tests.
#[cfg(test)]
pub(super) static TOLD: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Tests put a path here to have a file appear at it between the check that found the
/// name free and the write: the moment no test could otherwise reach.
#[cfg(test)]
pub(super) static APPEAR: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
pub(super) fn appear_in_test(dst: &Path) {
    let mut paths = APPEAR.lock().unwrap();
    if let Some(i) = paths.iter().position(|p| p == dst) {
        paths.remove(i);
        fs::write(dst, "theirs").unwrap();
    }
    drop(paths);
    // A folder in its place while the bytes are written (red-team of #5).
    let mut dirs = APPEAR_DIR.lock().unwrap();
    if let Some(i) = dirs.iter().position(|p| p == dst) {
        dirs.remove(i);
        let _ = fs::remove_file(dst);
        fs::create_dir_all(dst.join("inner")).unwrap();
        fs::write(dst.join("inner/keep.txt"), "kept").unwrap();
    }
}

#[cfg(test)]
pub(super) static APPEAR_DIR: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

#[cfg(not(test))]
pub(super) fn appear_in_test(_: &Path) {}

/// Every name a copy opened to write, for tests to see which way a job wrote.
#[cfg(test)]
pub(super) static WRITING: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
pub(super) fn writing_in_test(target: &Path) {
    WRITING.lock().unwrap().push(target.to_path_buf());
}

#[cfg(not(test))]
pub(super) fn writing_in_test(_: &Path) {}
