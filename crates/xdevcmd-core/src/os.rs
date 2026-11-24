//! What xdevcmd asks of the operating system, in one place: the file system under a
//! path, free space, locks, processes, the machine's name, an error's meaning. One
//! function per question, with a body per platform inside it where the platforms
//! differ (Linux, macOS, Windows; one platform per build, so no trait). Callers stay
//! free of `cfg`.

use std::fs;
use std::io;
use std::path::Path;

/// What kind of file system a destination is, as far as names and sizes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fs {
    /// FAT12/16/32: files under 4 GiB, Windows' names, case ignored.
    Fat,
    Exfat,
    Ntfs,
    /// Reached over a network (NFS, SMB, sshfs, rclone, a UNC path): errors that pass
    /// are retried, and new files synced before their rename (Round 031).
    Network,
    Other,
    /// It could not be told (Windows, for beta.1: owner, 2026-11-13).
    Unknown,
}

#[cfg(target_os = "linux")]
pub(crate) fn fs_kind(dest: &Path) -> Fs {
    let Ok(st) = rustix::fs::statfs(dest) else {
        return Fs::Unknown;
    };
    #[allow(clippy::unnecessary_cast)]
    match st.f_type as i64 {
        0x4d44 => Fs::Fat,
        0x2011_bab0 => Fs::Exfat,
        0x5346_544e | 0x7366_746e => Fs::Ntfs,
        // NFS, SMB2/3, CIFS, SMB1, Ceph, AFS.
        0x6969 | 0xfe53_4d42 | 0xff53_4d42 | 0x517b | 0x00c3_6400 | 0x5346_414f => Fs::Network,
        // Every FUSE mount has this one number, sshfs and a local ntfs-3g alike: its
        // type in the mount table tells them apart.
        0x6573_5546 => match linux_mount_type(dest).as_deref().map(network_fuse) {
            Some(true) => Fs::Network,
            // ntfs-3g and exfat-fuse are both "fuseblk": not told apart.
            _ => Fs::Unknown,
        },
        _ => Fs::Other,
    }
}

/// A FUSE type from the mount table that reaches over a network.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn network_fuse(t: &str) -> bool {
    let name = t.strip_prefix("fuse.").unwrap_or(t);
    matches!(
        name,
        "sshfs"
            | "rclone"
            | "s3fs"
            | "gcsfuse"
            | "goofys"
            | "davfs"
            | "davfs2"
            | "curlftpfs"
            | "smbnetfs"
            | "glusterfs"
            | "cephfs"
            | "ceph-fuse"
            | "afpfs"
            | "mountpoint-s3"
    )
}

/// The type of the mount holding `dest` ("fuse.sshfs"), from `/proc/self/mountinfo`:
/// the mount point that is the longest prefix of `dest`, the last such in the table.
#[cfg(target_os = "linux")]
pub(crate) fn linux_mount_type(dest: &Path) -> Option<String> {
    let dest = std::fs::canonicalize(dest).ok()?;
    mount_type_in(
        &std::fs::read_to_string("/proc/self/mountinfo").ok()?,
        &dest,
    )
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn mount_type_in(table: &str, dest: &Path) -> Option<String> {
    // Spaces and the like are written as octal escapes (`\040`).
    let unescape = |s: &str| {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(u8::is_ascii_digit) {
                let n = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
                out.push(n);
                i += 4;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    let mut best: Option<(usize, String)> = None;
    for line in table.lines() {
        let (left, right) = line.split_once(" - ")?;
        let point = unescape(left.split(' ').nth(4)?);
        let kind = right.split(' ').next()?.to_string();
        if dest.starts_with(&point) && best.as_ref().is_none_or(|(n, _)| point.len() >= *n) {
            best = Some((point.len(), kind));
        }
    }
    best.map(|(_, k)| k)
}

/// The file system's name macOS gives: "apfs", "msdos", "smbfs" …
#[cfg(target_os = "macos")]
pub(crate) fn mac_fs_name(dest: &Path) -> Option<Vec<u8>> {
    let st = rustix::fs::statfs(dest).ok()?;
    Some(
        st.f_fstypename
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect(),
    )
}

#[cfg(target_os = "macos")]
pub(crate) fn fs_kind(dest: &Path) -> Fs {
    let Some(name) = mac_fs_name(dest) else {
        return Fs::Unknown;
    };
    match name.as_slice() {
        b"msdos" => Fs::Fat,
        b"exfat" => Fs::Exfat,
        b"ntfs" | b"ufsd_NTFS" => Fs::Ntfs,
        b"smbfs" | b"nfs" | b"afpfs" | b"webdav" | b"cifs" | b"ftp" => Fs::Network,
        n if n.starts_with(b"macfuse") || n.starts_with(b"osxfuse") => Fs::Unknown,
        _ => Fs::Other,
    }
}

/// Windows: a UNC path is a network share; a mapped drive letter cannot be told
/// without the console API, and stays unknown.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn fs_kind(dest: &Path) -> Fs {
    if unc(dest) {
        Fs::Network
    } else {
        Fs::Unknown
    }
}

/// `\\server\share` or `\\?\UNC\server\share`.
#[cfg(any(windows, test))]
pub(crate) fn unc(p: &Path) -> bool {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("UNC\\"),
        None => s.starts_with(r"\\") || s.starts_with("//"),
    }
}

/// Whether the directory `dir` is reached over a network ([`Fs::Network`]).
pub fn is_network(dir: &Path) -> bool {
    fs_kind(dir) == Fs::Network
}

/// The file system at `dir`, as a word for the debug log ([`crate::debuglog`]): the
/// system's own name where it gives one ("apfs", "ext4"), else the kind.
pub fn fs_word(dir: &Path) -> String {
    #[cfg(target_os = "linux")]
    if let Ok(st) = rustix::fs::statfs(dir) {
        #[allow(clippy::unnecessary_cast)]
        let name = match st.f_type as i64 {
            0xef53 => Some("ext4"),
            0x9123_683e => Some("btrfs"),
            0x5846_5342 => Some("xfs"),
            0x0102_1994 => Some("tmpfs"),
            0x2fc1_2fc1 => Some("zfs"),
            0xcafe_4a11 => Some("bcachefs"),
            0x794c_7630 => Some("overlay"),
            0xf2f5_2010 => Some("f2fs"),
            _ => None,
        };
        if let Some(n) = name {
            return n.to_string();
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(n) = mac_fs_name(dir) {
        return String::from_utf8_lossy(&n).into_owned();
    }
    match fs_kind(dir) {
        Fs::Fat => "fat",
        Fs::Exfat => "exfat",
        Fs::Ntfs => "ntfs",
        Fs::Network => "network",
        Fs::Other => "other",
        Fs::Unknown => "unknown",
    }
    .to_string()
}

/// Bytes free at `dest` for this user; `None` where it cannot be asked (Windows).
pub fn free_space(dest: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let st = rustix::fs::statvfs(dest).ok()?;
        Some(st.f_bavail.saturating_mul(st.f_frsize))
    }
    #[cfg(not(unix))]
    {
        let _ = dest;
        None
    }
}

pub(crate) fn writable(dest: &Path) -> Option<bool> {
    #[cfg(unix)]
    {
        use rustix::io::Errno;
        match rustix::fs::access(dest, rustix::fs::Access::WRITE_OK) {
            Ok(()) => Some(true),
            Err(e) if e == Errno::ACCESS || e == Errno::ROFS || e == Errno::PERM => Some(false),
            Err(_) => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dest;
        None
    }
}

/// Whether `src` and the directory `dest` are on one device. `None`: one of them
/// cannot be looked at, or (Windows) a path without a drive or share to compare.
pub fn same_device(src: &Path, dest: &Path) -> Option<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let a = fs::symlink_metadata(src).ok()?.dev();
        let b = fs::metadata(dest).ok()?.dev();
        Some(a == b)
    }
    #[cfg(not(unix))]
    {
        let volume = |p: &Path| -> Option<std::ffi::OsString> {
            let p = crate::paths::canonical(p).ok()?;
            match p.components().next()? {
                std::path::Component::Prefix(x) => Some(x.as_os_str().to_ascii_uppercase()),
                _ => None,
            }
        };
        Some(volume(src)? == volume(dest)?)
    }
}

/// This machine's name, to tell its journals from another's in a shared home directory.
/// Empty when the system will not say.
pub fn hostname() -> String {
    #[cfg(unix)]
    {
        rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(unix))]
    {
        std::env::var("COMPUTERNAME").unwrap_or_default()
    }
}

/// What trying a journal's lock found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Lock {
    /// Ours now: the run that wrote it is gone.
    Held,
    /// Someone holds it: the run is alive. Unix only: on Windows a held journal does
    /// not open at all.
    #[cfg_attr(not(unix), allow(dead_code))]
    Busy,
    /// The file system has no locks. Unix only: on Windows the open is the lock.
    #[cfg_attr(not(unix), allow(dead_code))]
    Unsupported,
}

/// Try to take `file`'s lock without waiting.
#[cfg(unix)]
pub(crate) fn try_lock(file: &fs::File) -> Lock {
    use rustix::fs::{flock, FlockOperation};
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Lock::Held,
        Err(e) if e == rustix::io::Errno::WOULDBLOCK => Lock::Busy,
        // ENOLCK, EOPNOTSUPP: a file system without locks. Anything else is no better
        // an answer, and the fallback is the cautious one.
        Err(_) => Lock::Unsupported,
    }
}

pub(crate) fn is_cross_device(e: &io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(18) // EXDEV
    }
    #[cfg(windows)]
    {
        e.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = e;
        false
    }
}

/// Whether the process `pid` exists, which a reused pid can make look alive (cautious)
/// but never dead. Unix only; elsewhere taken as alive.
pub(crate) fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        rustix::process::Pid::from_raw(pid as i32)
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Whether `m` has an execute bit for anyone. Unix only: Windows has no such bit, and
/// what runs there is a matter of the name's extension, which each caller judges for
/// its own purpose.
pub fn has_exec_bit(m: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 031: a network target from the mount table (the longest mount point
    /// holding the path, spaces escaped), FUSE's network types, and UNC paths.
    #[test]
    fn network_targets_are_told_from_the_mount_table_and_unc_paths() {
        let table = "\
22 1 8:1 / / rw - ext4 /dev/sda1 rw
40 22 0:50 / /mnt/box rw - fuse.sshfs me@box: rw
41 40 0:51 / /mnt/box/my\\040disk rw - fuse.ntfs-3g /dev/sdb1 rw
";
        let t = |p: &str| mount_type_in(table, Path::new(p));
        assert_eq!(t("/home/me").as_deref(), Some("ext4"));
        assert_eq!(t("/mnt/box/x").as_deref(), Some("fuse.sshfs"));
        assert_eq!(t("/mnt/box/my disk/x").as_deref(), Some("fuse.ntfs-3g"));
        assert!(network_fuse("fuse.sshfs") && network_fuse("fuse.rclone"));
        assert!(!network_fuse("fuse.ntfs-3g") && !network_fuse("fuseblk"));
        assert!(unc(Path::new(r"\\server\share\x")));
        assert!(unc(Path::new(r"\\?\UNC\server\share")));
        assert!(!unc(Path::new(r"\\?\C:\x")));
        assert!(!unc(Path::new(r"C:\x")));
    }
}
