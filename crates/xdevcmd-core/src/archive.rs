//! Archives: browsed as directories, extracted, and packed. zip, tar and tar.gz.
//!
//! An archive is a file somebody else made, and extracting one writes wherever its
//! names say unless told otherwise. The rules here, each of them tested:
//!
//! - **A name must stay inside.** Absolute paths, `..`, drive prefixes and anything a
//!   platform would read as one are refused per entry ("zip-slip"). The destination's
//!   own directories are checked too: an existing symlink inside it that points out is
//!   not followed.
//! - **No links are created.** Symlinks and hard links in an archive are listed but
//!   skipped on extraction, and said so. A link is how an archive writes outside the
//!   destination after its names have all been checked.
//! - **No set-id bits.** Modes are kept to `rwx` for user, group and other; SUID,
//!   SGID and sticky are dropped.
//! - **An entry is no bigger than it says.** Reading stops one byte past the declared
//!   size and the entry fails, so a lying header cannot fill the disk.
//!
//! The extraction itself never uses the zip crate's `extract()`: the rules above are
//! easier to keep when every byte goes through code that can be read here.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    Tar,
    TarGz,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Zip => "zip",
            Format::Tar => "tar",
            Format::TarGz => "tar.gz",
        }
    }
}

/// What an archive is, from its name.
pub fn format_of(path: &Path) -> Option<Format> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(Format::TarGz)
    } else if name.ends_with(".tar") {
        Some(Format::Tar)
    } else if [
        ".zip", ".jar", ".war", ".apk", ".epub", ".docx", ".xlsx", ".pptx", ".odt",
    ]
    .iter()
    .any(|e| name.ends_with(e))
    {
        Some(Format::Zip)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Dir,
    /// A symlink or hard link: listed, never extracted.
    Link,
    /// A device, FIFO or anything else tar can hold: listed, never extracted.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Inside the archive, `/`-separated, no leading or trailing slash.
    pub path: String,
    pub kind: ItemKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mode: Option<u32>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Listing {
    pub items: Vec<Item>,
    /// Entries whose names would land outside, left out of the listing.
    pub unsafe_names: usize,
}

/// A name as it may be used: `/`-separated, every component ordinary. `None` for a
/// name that is absolute or climbs out.
pub fn clean(name: &str) -> Option<String> {
    let unified = name.replace('\\', "/");
    if unified.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for c in Path::new(&unified).components() {
        match c {
            Component::Normal(s) => {
                let s = s.to_str()?;
                // A drive or stream, `C:` or `file:stream`, on any platform: the archive
                // may well be opened on Windows later.
                if s.contains(':') {
                    return None;
                }
                parts.push(s.to_string());
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn zip_time(t: Option<zip::DateTime>) -> Option<SystemTime> {
    let t = t?;
    let date = chrono::NaiveDate::from_ymd_opt(t.year() as i32, t.month() as u32, t.day() as u32)?;
    let dt = date.and_hms_opt(t.hour() as u32, t.minute() as u32, t.second() as u32)?;
    let secs = dt.and_utc().timestamp();
    (secs >= 0).then(|| SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Everything in the archive, directories implied by deeper names included.
pub fn list(archive: &Path) -> io::Result<Listing> {
    list_until(archive, &std::sync::atomic::AtomicBool::new(false))
}

/// As [`list`], giving up when `stop` is set. A `.tar.gz` has no index: listing one
/// reads all of it, about a second for 300 MB, so it runs off the UI thread and can
/// be called off.
pub fn list_until(archive: &Path, stop: &std::sync::atomic::AtomicBool) -> io::Result<Listing> {
    use std::sync::atomic::Ordering;
    let stopped = || io::Error::new(io::ErrorKind::Interrupted, "stopped");
    let mut out = Listing::default();
    let mut push = |raw: &str, kind: ItemKind, size: u64, modified, mode| match clean(raw) {
        Some(path) => out.items.push(Item {
            path,
            kind,
            size,
            modified,
            mode,
        }),
        None => out.unsafe_names += 1,
    };
    match format_of(archive).ok_or_else(|| io::Error::other("not an archive xdevcmd reads"))? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(archive)?).map_err(io::Error::other)?;
            for i in 0..z.len() {
                if stop.load(Ordering::Relaxed) {
                    return Err(stopped());
                }
                let f = z.by_index_raw(i).map_err(io::Error::other)?;
                let name = f.name().map_err(io::Error::other)?.into_owned();
                let kind = if f.is_dir() {
                    ItemKind::Dir
                } else if f.is_symlink() {
                    ItemKind::Link
                } else {
                    ItemKind::File
                };
                push(
                    &name,
                    kind,
                    f.size(),
                    zip_time(f.last_modified()),
                    f.unix_mode(),
                );
            }
        }
        fmt @ (Format::Tar | Format::TarGz) => {
            let file = File::open(archive)?;
            let reader: Box<dyn Read> = if fmt == Format::TarGz {
                Box::new(GzDecoder::new(file))
            } else {
                Box::new(file)
            };
            let mut t = tar::Archive::new(reader);
            for e in t.entries()? {
                if stop.load(Ordering::Relaxed) {
                    return Err(stopped());
                }
                let e = e?;
                let h = e.header();
                let name = String::from_utf8_lossy(&e.path_bytes()).into_owned();
                let kind = match h.entry_type() {
                    tar::EntryType::Regular | tar::EntryType::Continuous => ItemKind::File,
                    tar::EntryType::Directory => ItemKind::Dir,
                    tar::EntryType::Symlink | tar::EntryType::Link => ItemKind::Link,
                    _ => ItemKind::Other,
                };
                let modified = h
                    .mtime()
                    .ok()
                    .map(|s| SystemTime::UNIX_EPOCH + Duration::from_secs(s));
                push(&name, kind, h.size().unwrap_or(0), modified, h.mode().ok());
            }
        }
    }
    // Directories that only exist as part of a deeper name.
    let mut known: std::collections::HashSet<String> =
        out.items.iter().map(|i| i.path.clone()).collect();
    let mut implied = Vec::new();
    for it in &out.items {
        let mut p = it.path.as_str();
        while let Some((parent, _)) = p.rsplit_once('/') {
            if known.insert(parent.to_string()) {
                implied.push(Item {
                    path: parent.to_string(),
                    kind: ItemKind::Dir,
                    size: 0,
                    modified: None,
                    mode: None,
                });
            }
            p = parent;
        }
    }
    out.items.extend(implied);
    Ok(out)
}

/// The entries directly inside `prefix` ("" for the top), as (name, item).
pub fn children<'a>(items: &'a [Item], prefix: &str) -> Vec<(String, &'a Item)> {
    let mut v: Vec<(String, &Item)> = items
        .iter()
        .filter_map(|it| {
            let rest = if prefix.is_empty() {
                it.path.as_str()
            } else {
                it.path.strip_prefix(prefix)?.strip_prefix('/')?
            };
            (!rest.is_empty() && !rest.contains('/')).then(|| (rest.to_string(), it))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.dedup_by(|a, b| a.0 == b.0);
    v
}

/// Everything at or below each of `chosen` — what extracting them means. An empty
/// name stands for the whole archive.
pub fn expand<'a>(items: &'a [Item], chosen: &[String]) -> Vec<&'a Item> {
    items
        .iter()
        .filter(|it| {
            chosen.iter().any(|c| {
                c.is_empty()
                    || it.path == *c
                    || it
                        .path
                        .strip_prefix(c.as_str())
                        .is_some_and(|r| r.starts_with('/'))
            })
        })
        .collect()
}

/// Files and bytes that extracting `chosen` would write, counted off the UI thread.
pub fn count_until(
    archive: &Path,
    chosen: &[String],
    stop: &std::sync::atomic::AtomicBool,
) -> Option<(u64, u64)> {
    let l = list_until(archive, stop).ok()?;
    Some(
        expand(&l.items, chosen)
            .iter()
            .filter(|i| i.kind == ItemKind::File)
            .fold((0, 0), |(n, b), i| (n + 1, b + i.size)),
    )
}

/// What the extractor needs from whoever runs it: the copy engine supplies these, so
/// extraction asks the same questions, shows the same progress and stops the same way.
pub trait Hooks {
    fn stopped(&self) -> bool;
    fn begin(&mut self, path: &Path);
    /// An existing destination: `Some(true)` replace, `Some(false)` skip, `None` stop.
    fn conflict(&mut self, dst: &Path, incoming: u64) -> Option<bool>;
    fn wrote(&mut self, bytes: u64);
    fn done(&mut self);
    fn skipped(&mut self, path: &Path, why: &str);
    fn failed(&mut self, path: &Path, e: &io::Error);
    /// A temporary file is about to be made (`made`), or is gone: for safe
    /// operations' journal.
    fn part(&mut self, _p: &Path, _made: bool) {}
    /// About to replace `dst`, which cannot be undone: `false` stops here (the journal
    /// cannot be written and the user said stop).
    fn durable(&mut self, _dst: &Path) -> bool {
        true
    }
    /// Right before replacing `dst`: an error if it is no longer the file the user
    /// agreed to replace (it is then kept, and the entry reported).
    fn recheck(&mut self, _dst: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// Where `rel` goes under `dest`, refusing a path whose existing directories include a
/// symlink: a link already inside the destination must not carry a write out of it.
fn target(dest: &Path, rel: &str) -> io::Result<PathBuf> {
    let mut p = dest.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        p.push(part);
        let last = i + 1 == parts.len();
        if let Ok(m) = fs::symlink_metadata(&p) {
            if m.file_type().is_symlink() && !last {
                return Err(io::Error::other(format!(
                    "{} is a symlink in the destination \u{2014} not followed",
                    p.display()
                )));
            }
        }
    }
    Ok(p)
}

/// Extract the entries at or below `chosen` into `dest`, with `base` (the directory
/// being browsed, "" for the top) taken off the front of each name.
pub fn extract(
    archive: &Path,
    chosen: &[String],
    base: &str,
    dest: &Path,
    hooks: &mut dyn Hooks,
) -> io::Result<()> {
    let listing = list(archive)?;
    let wanted: std::collections::HashSet<String> = expand(&listing.items, chosen)
        .into_iter()
        .map(|i| i.path.clone())
        .collect();
    check_room(archive, dest, &listing.items, &wanted)?;
    let rel_of = |path: &str| -> Option<String> {
        let rel = if base.is_empty() {
            path
        } else {
            path.strip_prefix(base)?.strip_prefix('/')?
        };
        (!rel.is_empty()).then(|| rel.to_string())
    };
    // Directories first, so every file has somewhere to go.
    for it in listing.items.iter().filter(|i| i.kind == ItemKind::Dir) {
        if !wanted.contains(&it.path) {
            continue;
        }
        let Some(rel) = rel_of(&it.path) else {
            continue;
        };
        match target(dest, &rel).and_then(|p| fs::create_dir_all(&p).map(|_| p)) {
            Ok(_) => {}
            Err(e) => hooks.failed(&dest.join(&rel), &e),
        }
    }
    let each = |name: &str,
                kind: ItemKind,
                size: u64,
                mode: Option<u32>,
                data: &mut dyn Read,
                hooks: &mut dyn Hooks|
     -> bool {
        if hooks.stopped() {
            return false;
        }
        let Some(path) = clean(name) else { return true };
        if !wanted.contains(&path) || kind == ItemKind::Dir {
            return true;
        }
        let Some(rel) = rel_of(&path) else {
            return true;
        };
        let shown = dest.join(&rel);
        match kind {
            ItemKind::Link => {
                hooks.skipped(&shown, "links in archives are not extracted");
                return true;
            }
            ItemKind::Other => {
                hooks.skipped(&shown, "not a regular file");
                return true;
            }
            _ => {}
        }
        hooks.begin(&shown);
        let dst = match target(dest, &rel) {
            Ok(d) => d,
            Err(e) => {
                hooks.failed(&shown, &e);
                return true;
            }
        };
        if let Some(parent) = dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let free = dst.symlink_metadata().is_err();
        match hooks.conflict(&dst, size) {
            None => return false,
            Some(false) => return true,
            Some(true) => {}
        }
        // write_entry removes what it wrote when it does not finish, and leaves an
        // existing file as it was until the new one is complete.
        match write_entry(&dst, free, size, mode, data, hooks) {
            Ok(true) => hooks.done(),
            Ok(false) => return false,
            Err(e) => hooks.failed(&shown, &e),
        }
        true
    };
    match format_of(archive).ok_or_else(|| io::Error::other("not an archive"))? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(archive)?).map_err(io::Error::other)?;
            for i in 0..z.len() {
                let mut f = z.by_index(i).map_err(io::Error::other)?;
                let name = f.name().map_err(io::Error::other)?.into_owned();
                let kind = if f.is_dir() {
                    ItemKind::Dir
                } else if f.is_symlink() {
                    ItemKind::Link
                } else {
                    ItemKind::File
                };
                let (size, mode) = (f.size(), f.unix_mode());
                if !each(&name, kind, size, mode, &mut f, hooks) {
                    break;
                }
            }
        }
        fmt @ (Format::Tar | Format::TarGz) => {
            let file = File::open(archive)?;
            let reader: Box<dyn Read> = if fmt == Format::TarGz {
                Box::new(GzDecoder::new(file))
            } else {
                Box::new(file)
            };
            let mut t = tar::Archive::new(reader);
            for e in t.entries()? {
                let mut e = e?;
                let name = String::from_utf8_lossy(&e.path_bytes()).into_owned();
                let h = e.header();
                let kind = match h.entry_type() {
                    tar::EntryType::Regular | tar::EntryType::Continuous => ItemKind::File,
                    tar::EntryType::Directory => ItemKind::Dir,
                    tar::EntryType::Symlink | tar::EntryType::Link => ItemKind::Link,
                    _ => ItemKind::Other,
                };
                let (size, mode) = (h.size().unwrap_or(0), h.mode().ok());
                if !each(&name, kind, size, mode, &mut e, hooks) {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Copy one entry's bytes to `dst`. `Ok(false)` when stopped part-way.
///
/// Written under a temporary name beside `dst` and renamed into place once complete:
/// a stop or an error leaves an existing file as it was, and a symlink at `dst` is
/// replaced, never written through (it could point outside the destination). `free`:
/// `dst` was not there when checked; one that appears meanwhile is kept, not replaced.
fn write_entry(
    dst: &Path,
    free: bool,
    size: u64,
    mode: Option<u32>,
    data: &mut dyn Read,
    hooks: &mut dyn Hooks,
) -> io::Result<bool> {
    let part = crate::ops::part_name(dst);
    hooks.part(&part, true);
    let result = write_part(&part, size, mode, data, hooks).and_then(|complete| {
        let Some(file) = complete else {
            return Ok(false);
        };
        if !free {
            // Synced, then renamed over the old file (ops::commit_replace).
            return crate::ops::commit_replace(file, &part, dst, || {
                hooks.recheck(dst)?;
                Ok(hooks.durable(dst))
            });
        }
        drop(file);
        {
            crate::noreplace::rename_noreplace(&part, dst).map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    io::Error::other("a file of that name appeared meanwhile \u{2014} kept")
                } else {
                    e
                }
            })?;
        }
        Ok(true)
    });
    if !matches!(result, Ok(true)) {
        let _ = fs::remove_file(&part);
    }
    hooks.part(&part, false);
    result
}

/// The entry's bytes into the new file `part`: the file, still open, once complete;
/// `None` when stopped part-way.
fn write_part(
    part: &Path,
    size: u64,
    mode: Option<u32>,
    data: &mut dyn Read,
    hooks: &mut dyn Hooks,
) -> io::Result<Option<File>> {
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part)?;
    // One byte more than declared is allowed in, so that a lie is seen rather than
    // silently cut short.
    let mut limited = data.take(size.saturating_add(1));
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        if hooks.stopped() {
            return Ok(None);
        }
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > size {
            return Err(io::Error::other(
                "larger than the archive says it is \u{2014} stopped",
            ));
        }
        out.write_all(&buf[..n])?;
        hooks.wrote(n as u64);
    }
    #[cfg(unix)]
    if let Some(m) = mode {
        use std::os::unix::fs::PermissionsExt;
        // rwx for user, group and other only: never setuid, setgid or sticky.
        let _ = fs::set_permissions(part, fs::Permissions::from_mode(m & 0o777));
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(Some(out))
}

/// Above this many entries, or this ratio of unpacked size to archive size beyond a
/// gigabyte, an extraction is refused as a likely decompression bomb: a 10 MB zip can
/// claim hundreds of terabytes (Fifield, USENIX WOOT 2019). Each entry is still held to
/// its declared size as it is written.
const MAX_ENTRIES: usize = 1_000_000;
const MAX_RATIO: u64 = 1000;
const RATIO_FROM: u64 = 1 << 30;

/// Refuse an extraction that would not fit, or looks like a bomb, before writing
/// anything.
fn check_room(
    archive: &Path,
    dest: &Path,
    items: &[Item],
    wanted: &std::collections::HashSet<String>,
) -> io::Result<()> {
    let files: Vec<&Item> = items
        .iter()
        .filter(|i| i.kind == ItemKind::File && wanted.contains(&i.path))
        .collect();
    if files.len() > MAX_ENTRIES {
        return Err(io::Error::other(format!(
            "{} files \u{2014} more than {MAX_ENTRIES}; refused",
            files.len()
        )));
    }
    let total: u64 = files.iter().map(|i| i.size).fold(0, u64::saturating_add);
    let packed = fs::metadata(archive).map(|m| m.len()).unwrap_or(0).max(1);
    if total > RATIO_FROM && total / packed > MAX_RATIO {
        return Err(io::Error::other(format!(
            "it would unpack to {}, {}\u{d7} its own size \u{2014} refused as a likely \
             decompression bomb",
            crate::format::human_size(total),
            total / packed
        )));
    }
    #[cfg(unix)]
    if let Ok(st) = rustix::fs::statvfs(dest) {
        let free = st.f_bavail.saturating_mul(st.f_frsize);
        if total > free {
            return Err(io::Error::other(format!(
                "it needs {}, and only {} is free there",
                crate::format::human_size(total),
                crate::format::human_size(free)
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = dest;
    Ok(())
}

/// Pack `sources` into a new archive at `dest`, its format from its name. Each source
/// goes in under its own name. Symlinks are skipped, never followed, and the archive
/// being written is never put inside itself.
pub fn pack(sources: &[PathBuf], dest: &Path, hooks: &mut dyn Hooks) -> io::Result<()> {
    let fmt = format_of(dest)
        .ok_or_else(|| io::Error::other("the name must end in .zip, .tar, .tar.gz or .tgz"))?;
    let mut files: Vec<(PathBuf, String, bool)> = Vec::new();
    for s in sources {
        let base = s.parent().unwrap_or(Path::new(""));
        let mut stack = vec![s.clone()];
        while let Some(p) = stack.pop() {
            let Ok(m) = fs::symlink_metadata(&p) else {
                continue;
            };
            let rel = p
                .strip_prefix(base)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            if m.file_type().is_symlink() {
                hooks.skipped(&p, "symlinks are not packed");
            } else if m.is_dir() {
                files.push((p.clone(), rel, true));
                if let Ok(it) = fs::read_dir(&p) {
                    let mut kids: Vec<PathBuf> = it.flatten().map(|e| e.path()).collect();
                    kids.sort();
                    stack.extend(kids.into_iter().rev());
                }
            } else if m.is_file() {
                if p == dest {
                    continue;
                }
                files.push((p.clone(), rel, false));
            } else {
                hooks.skipped(&p, "not a regular file");
            }
        }
    }
    let part = crate::ops::part_name(dest);
    hooks.part(&part, true);
    let result = write_archive(fmt, &part, &files, hooks);
    let out = match result {
        // The archive's writers own the file and close it; opened again to sync it
        // (the sync is of the file, whichever handle asks), then renamed over.
        Ok(true) => fs::OpenOptions::new()
            .write(true)
            .open(&part)
            .and_then(|f| {
                let existing = dest.symlink_metadata().is_ok();
                crate::ops::commit_replace(f, &part, dest, || {
                    if existing {
                        hooks.recheck(dest)?;
                    }
                    Ok(!existing || hooks.durable(dest))
                })
            })
            .map(|_| ())
            .inspect_err(|_| {
                let _ = fs::remove_file(&part);
            }),
        Ok(false) => {
            let _ = fs::remove_file(&part);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    };
    hooks.part(&part, false);
    out
}

fn write_archive(
    fmt: Format,
    part: &Path,
    files: &[(PathBuf, String, bool)],
    hooks: &mut dyn Hooks,
) -> io::Result<bool> {
    let out = File::create(part)?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut copy = |from: &Path, to: &mut dyn Write, hooks: &mut dyn Hooks| -> io::Result<bool> {
        let mut f = File::open(from)?;
        loop {
            if hooks.stopped() {
                return Ok(false);
            }
            let n = f.read(&mut buf)?;
            if n == 0 {
                return Ok(true);
            }
            to.write_all(&buf[..n])?;
            hooks.wrote(n as u64);
        }
    };
    match fmt {
        Format::Zip => {
            use zip::write::SimpleFileOptions;
            let mut z = zip::ZipWriter::new(out);
            for (path, rel, is_dir) in files {
                let meta = fs::metadata(path)?;
                let mut opts = SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .large_file(meta.len() >= u32::MAX as u64);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    opts = opts.unix_permissions(meta.permissions().mode() & 0o777);
                }
                if let Some(t) = meta.modified().ok().and_then(to_zip_time) {
                    opts = opts.last_modified_time(t);
                }
                if *is_dir {
                    z.add_directory(format!("{rel}/"), opts)
                        .map_err(io::Error::other)?;
                    continue;
                }
                hooks.begin(path);
                z.start_file(rel.as_str(), opts).map_err(io::Error::other)?;
                if !copy(path, &mut z, hooks)? {
                    return Ok(false);
                }
                hooks.done();
            }
            z.finish().map_err(io::Error::other)?;
        }
        Format::Tar | Format::TarGz => {
            let sink: Box<dyn Write> = if fmt == Format::TarGz {
                Box::new(GzEncoder::new(out, flate2::Compression::default()))
            } else {
                Box::new(out)
            };
            let mut t = tar::Builder::new(sink);
            t.follow_symlinks(false);
            for (path, rel, is_dir) in files {
                if hooks.stopped() {
                    return Ok(false);
                }
                if *is_dir {
                    t.append_dir(rel, path)?;
                    continue;
                }
                hooks.begin(path);
                let mut f = File::open(path)?;
                t.append_file(rel, &mut f)?;
                hooks.wrote(f.metadata().map(|m| m.len()).unwrap_or(0));
                hooks.done();
            }
            let mut sink = t.into_inner()?;
            sink.flush()?;
        }
    }
    Ok(true)
}

fn to_zip_time(t: SystemTime) -> Option<zip::DateTime> {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64;
    let dt = chrono::DateTime::from_timestamp(secs, 0)?.naive_utc();
    use chrono::{Datelike, Timelike};
    zip::DateTime::from_date_and_time(
        u16::try_from(dt.year()).ok()?,
        dt.month() as u8,
        dt.day() as u8,
        dt.hour() as u8,
        dt.minute() as u8,
        dt.second() as u8,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    #[derive(Default)]
    struct Log {
        done: usize,
        skipped: Vec<String>,
        failed: Vec<String>,
        answer: Option<bool>,
    }
    impl Hooks for Log {
        fn stopped(&self) -> bool {
            false
        }
        fn begin(&mut self, _: &Path) {}
        fn conflict(&mut self, dst: &Path, _: u64) -> Option<bool> {
            if fs::symlink_metadata(dst).is_ok() {
                self.answer
            } else {
                Some(true)
            }
        }
        fn wrote(&mut self, _: u64) {}
        fn done(&mut self) {
            self.done += 1;
        }
        fn skipped(&mut self, p: &Path, why: &str) {
            self.skipped.push(format!("{}: {why}", p.display()));
        }
        fn failed(&mut self, p: &Path, e: &io::Error) {
            self.failed.push(format!("{}: {e}", p.display()));
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-arch-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A zip built by hand, with whatever names an attacker likes.
    fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, body) in entries {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(body).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn names_that_climb_out_are_refused() {
        for bad in [
            "/etc/passwd",
            "../x",
            "a/../../x",
            "C:/x",
            "c:x",
            "..\\x",
            "a\\..\\..\\x",
            "",
        ] {
            assert_eq!(clean(bad), None, "{bad:?}");
        }
        assert_eq!(clean("./a//b/"), Some("a/b".into()));
        assert_eq!(
            clean("a\\b"),
            Some("a/b".into()),
            "a Windows separator is a separator"
        );
    }

    #[test]
    fn a_zip_slip_archive_writes_nothing_outside() {
        let d = scratch("slip");
        let z = d.join("evil.zip");
        zip_with(
            &z,
            &[
                ("../escaped.txt", b"x"),
                ("/tmp/abs.txt", b"x"),
                ("ok/fine.txt", b"fine"),
            ],
        );
        let l = list(&z).unwrap();
        assert_eq!(l.unsafe_names, 2);
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        let all: Vec<String> = l.items.iter().map(|i| i.path.clone()).collect();
        let mut log = Log::default();
        extract(&z, &all, "", &out, &mut log).unwrap();
        assert!(!d.join("escaped.txt").exists());
        assert_eq!(fs::read_to_string(out.join("ok/fine.txt")).unwrap(), "fine");
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_already_in_the_destination_is_not_followed() {
        let d = scratch("dest-link");
        let out = d.join("out");
        let elsewhere = d.join("elsewhere");
        fs::create_dir_all(&out).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, out.join("sub")).unwrap();
        let z = d.join("a.zip");
        zip_with(&z, &[("sub/planted.txt", b"x")]);
        let mut log = Log::default();
        extract(&z, &["sub".into()], "", &out, &mut log).unwrap();
        assert!(!elsewhere.join("planted.txt").exists(), "{:?}", log.failed);
        assert_eq!(log.failed.len(), 1, "{:?}", log.failed);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn links_in_a_tar_are_skipped_and_setuid_is_dropped() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("tar");
        let t = d.join("a.tar");
        {
            let mut b = tar::Builder::new(File::create(&t).unwrap());
            let mut h = tar::Header::new_gnu();
            h.set_size(2);
            h.set_mode(0o4755);
            h.set_entry_type(tar::EntryType::Regular);
            h.set_cksum();
            b.append_data(&mut h, "prog", &b"hi"[..]).unwrap();
            let mut l = tar::Header::new_gnu();
            l.set_entry_type(tar::EntryType::Symlink);
            l.set_size(0);
            b.append_link(&mut l, "passwd", "/etc/passwd").unwrap();
            b.finish().unwrap();
        }
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        let mut log = Log::default();
        extract(&t, &["prog".into(), "passwd".into()], "", &out, &mut log).unwrap();
        assert!(
            fs::symlink_metadata(out.join("passwd")).is_err(),
            "no link made"
        );
        assert_eq!(log.skipped.len(), 1);
        let mode = fs::metadata(out.join("prog")).unwrap().permissions().mode();
        assert_eq!(mode & 0o7777, 0o755, "setuid dropped, rwx kept");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn browsing_sees_implied_directories_and_a_level_at_a_time() {
        let d = scratch("browse");
        let z = d.join("a.zip");
        zip_with(
            &z,
            &[
                ("top.txt", b"1"),
                ("src/deep/x.rs", b"22"),
                ("src/y.rs", b"333"),
            ],
        );
        let l = list(&z).unwrap();
        let top: Vec<String> = children(&l.items, "").into_iter().map(|c| c.0).collect();
        assert_eq!(top, vec!["src", "top.txt"]);
        let src: Vec<String> = children(&l.items, "src").into_iter().map(|c| c.0).collect();
        assert_eq!(src, vec!["deep", "y.rs"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn extracting_from_inside_takes_the_browsed_directory_off_the_front() {
        let d = scratch("base");
        let z = d.join("a.zip");
        zip_with(&z, &[("src/deep/x.rs", b"22"), ("src/y.rs", b"333")]);
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        let mut log = Log::default();
        extract(&z, &["src/deep".into()], "src", &out, &mut log).unwrap();
        assert_eq!(fs::read_to_string(out.join("deep/x.rs")).unwrap(), "22");
        assert!(!out.join("y.rs").exists() && !out.join("src").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_existing_file_is_asked_about_and_skip_leaves_it() {
        let d = scratch("conflict");
        let z = d.join("a.zip");
        zip_with(&z, &[("f.txt", b"new")]);
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("f.txt"), "old").unwrap();
        let mut log = Log {
            answer: Some(false),
            ..Log::default()
        };
        extract(&z, &["f.txt".into()], "", &out, &mut log).unwrap();
        assert_eq!(fs::read_to_string(out.join("f.txt")).unwrap(), "old");
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn overwriting_a_link_in_the_destination_replaces_the_link_not_its_target() {
        let d = scratch("last-link");
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        let victim = d.join("victim.txt");
        fs::write(&victim, "precious").unwrap();
        // out/f.txt is a link to a file outside; the archive has an f.txt too.
        std::os::unix::fs::symlink(&victim, out.join("f.txt")).unwrap();
        let z = d.join("a.zip");
        zip_with(&z, &[("f.txt", b"from the archive")]);
        let mut log = Log {
            answer: Some(true),
            ..Log::default()
        };
        extract(&z, &["f.txt".into()], "", &out, &mut log).unwrap();
        assert_eq!(
            fs::read_to_string(&victim).unwrap(),
            "precious",
            "{:?}",
            log.failed
        );
        let m = fs::symlink_metadata(out.join("f.txt")).unwrap();
        assert!(m.is_file(), "the link itself was replaced by the file");
        assert_eq!(
            fs::read_to_string(out.join("f.txt")).unwrap(),
            "from the archive"
        );
        let left: Vec<_> = fs::read_dir(&out).unwrap().flatten().collect();
        assert_eq!(left.len(), 1, "no temporary file left");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_archive_that_would_unpack_to_far_more_than_its_size_is_refused() {
        let d = scratch("bomb");
        let z = d.join("small.zip");
        fs::write(&z, vec![0u8; 1000]).unwrap();
        let item = |path: &str, size: u64| Item {
            path: path.into(),
            kind: ItemKind::File,
            size,
            modified: None,
            mode: None,
        };
        let items = vec![item("a", 3 << 30), item("b", 10)];
        let all: std::collections::HashSet<String> = ["a".into(), "b".into()].into();
        let e = check_room(&z, &d, &items, &all).unwrap_err();
        assert!(e.to_string().contains("decompression bomb"), "{e}");
        // Only b chosen: small, fine.
        let b: std::collections::HashSet<String> = ["b".into()].into();
        assert!(check_room(&z, &d, &items, &b).is_ok());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn pack_then_list_then_extract_round_trips_for_every_format() {
        for ext in ["zip", "tar", "tar.gz"] {
            let d = scratch(&format!("pack-{}", ext.replace('.', "-")));
            fs::create_dir_all(d.join("proj/src")).unwrap();
            fs::write(d.join("proj/src/main.rs"), "fn main() {}").unwrap();
            fs::write(d.join("proj/README"), "hi").unwrap();
            let archive = d.join(format!("proj.{ext}"));
            let mut log = Log::default();
            pack(&[d.join("proj")], &archive, &mut log).unwrap();
            assert!(!d.join(format!("proj.{ext}.part")).exists());
            let l = list(&archive).unwrap();
            let mut names: Vec<_> = l.items.iter().map(|i| i.path.as_str()).collect();
            names.sort();
            assert_eq!(
                names,
                vec!["proj", "proj/README", "proj/src", "proj/src/main.rs"],
                "{ext}"
            );
            let out = d.join("out");
            fs::create_dir_all(&out).unwrap();
            extract(&archive, &["proj".into()], "", &out, &mut log).unwrap();
            assert_eq!(
                fs::read_to_string(out.join("proj/src/main.rs")).unwrap(),
                "fn main() {}",
                "{ext}"
            );
            let _ = fs::remove_dir_all(&d);
        }
    }

    /// Extracting over a file and packing over an archive go through the one replace:
    /// synced, then renamed.
    #[test]
    fn extract_and_pack_sync_a_replacement_before_renaming_it() {
        let d = scratch("sync");
        let z = d.join("a.zip");
        zip_with(&z, &[("f.txt", b"new")]);
        let out = d.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("f.txt"), b"old").unwrap();
        let frag = d.to_string_lossy().into_owned();
        crate::faults::trace(&frag);
        let mut log = Log {
            answer: Some(true),
            ..Log::default()
        };
        extract(&z, &["f.txt".into()], "", &out, &mut log).unwrap();
        assert_eq!(fs::read(out.join("f.txt")).unwrap(), b"new");
        let packed = d.join("p.zip");
        fs::write(&packed, b"old archive").unwrap();
        pack(&[out.join("f.txt")], &packed, &mut log).unwrap();
        let seen: Vec<&str> = crate::faults::traced(&frag)
            .into_iter()
            .map(|(p, _)| p.name())
            .collect();
        assert_eq!(
            seen,
            ["sync-data", "rename", "sync-data", "rename"],
            "{seen:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }
}
