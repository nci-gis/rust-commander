//! The platform's own fast copy (item 22), behind a fallback to xdevcmd's read and write.
//!
//! The planner decides what is copied; here the system's fastest primitive moves the
//! bytes when the drive allows it:
//! - **Linux**: a clone (`FICLONE`: Btrfs, XFS, bcachefs; shares the blocks, no data
//!   written), else `copy_file_range` (the kernel copies, no trip through xdevcmd;
//!   NFS 4.2 and SMB servers copy on their side), else xdevcmd's own loop.
//! - **macOS**: a clone (`fclonefileat`, APFS), else xdevcmd's own loop.
//! - **Windows and the rest**: xdevcmd's own loop, unchanged.
//!
//! Nothing here decides anything a user sees: the file is still written under its
//! temporary name, synced and renamed as before. A step the drive refuses falls back
//! without a word; a real error (disk full, I/O) is the copy's error, as before.
//! `[general] fast_copy = false` turns all of it off.

use std::sync::atomic::{AtomicBool, Ordering};

/// Which code moved a file's bytes, counted in [`crate::ops::Summary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// The drive shares the source's blocks: nothing is written until one changes.
    Clone,
    /// The kernel copied (`copy_file_range`).
    Kernel,
    /// xdevcmd read and wrote.
    Own,
}

static FAST: AtomicBool = AtomicBool::new(true);

/// `[general] fast_copy`. Read once per job, by its worker.
pub fn set_enabled(on: bool) {
    FAST.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    FAST.load(Ordering::Relaxed)
}

/// A clone of `src` into the empty, just opened `dst` (Linux). `false`: the drive or
/// kernel cannot (not a cloning file system, across file systems, too old): nothing
/// happened, and the caller copies another way.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn clone_into(src: &std::fs::File, dst: &std::fs::File) -> bool {
    rustix::fs::ioctl_ficlone(dst, src).is_ok()
}

/// A clone of `src` made at `dst`, which must not exist (macOS): `fclonefileat` makes
/// the file itself, so it is tried before the file is created. `false`: not APFS,
/// across volumes, or the name is taken; the caller creates the file as before, and a
/// taken name fails there as it always did.
#[cfg(target_vendor = "apple")]
pub fn clone_at(src: &std::fs::File, dst: &std::path::Path) -> bool {
    use rustix::fs::{fclonefileat, CloneFlags, CWD};
    // NOOWNERCOPY: the clone is the user's, as a written copy is.
    fclonefileat(
        src,
        CWD,
        dst,
        CloneFlags::NOFOLLOW | CloneFlags::NOOWNERCOPY,
    )
    .is_ok()
}

/// What one `copy_file_range` step did.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub enum Step {
    /// This many bytes copied; the files' offsets moved on by as much.
    Copied(usize),
    /// The end of the source.
    End,
    /// The kernel will not copy between these two (across file systems on an old
    /// kernel, a file system without it, a special file that reports no size): the
    /// caller goes on with read and write from the offsets where this stopped.
    Refused,
}

/// Up to `len` bytes from `src`'s offset to `dst`'s, by the kernel.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn kernel_step(
    src: &std::fs::File,
    dst: &std::fs::File,
    len: usize,
    first: bool,
) -> std::io::Result<Step> {
    use rustix::io::Errno;
    match rustix::fs::copy_file_range(src, None, dst, None, len) {
        // Some files (in /proc, /sys) say 0 at once although read gives data.
        Ok(0) if first => Ok(Step::Refused),
        Ok(0) => Ok(Step::End),
        Ok(n) => Ok(Step::Copied(n)),
        Err(e)
            if [
                Errno::XDEV,
                Errno::NOSYS,
                Errno::NOTSUP,
                Errno::OPNOTSUPP,
                Errno::INVAL,
                Errno::BADF,
                Errno::PERM,
                Errno::OVERFLOW,
                Errno::TXTBSY,
            ]
            .contains(&e) =>
        {
            Ok(Step::Refused)
        }
        Err(e) => Err(e.into()),
    }
}
