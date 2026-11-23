//! A file opened for viewing, without reading it all into memory.
//!
//! The whole point of a viewer in a file manager is that it opens a 2 GB log instantly.
//! So the file stays on disk: a stream through it records where each line starts (eight
//! bytes per line), and everything after that is a seek and a short read of only what is
//! on screen.
//!
//! The stream is progressive (A03, Round 033): opening indexes the first
//! [`INLINE_INDEX`] bytes, enough for the first screens, and a thread of its own indexes
//! the rest while the viewer is already drawn. [`Document::poll_index`] takes in what it
//! found; dropping the document stops it.
//!
//! A document can also hold bytes in memory instead ([`Document::from_bytes`]): a
//! shell's history, which can carry tokens and customer data, is viewed without ever
//! touching the disk. The line index, search and hex work the same on either.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// Bytes indexed before [`Document::open`] returns: the first screens of any file,
/// and the whole of most (a few milliseconds). The rest is indexed in the background.
pub const INLINE_INDEX: u64 = 4 << 20;
/// Bytes read per step of the index, and per message from its thread.
const INDEX_CHUNK: usize = 256 * 1024;
/// Messages the index's thread may have waiting: it waits rather than holding more
/// than this many chunks' offsets in memory when the viewer does not take them (B08).
const INDEX_QUEUE: usize = 16;

/// Stop indexing here. A file with more lines than this is still viewable; the viewer
/// says it is showing the first part rather than pretending otherwise.
pub const MAX_LINES: usize = 4_000_000;
/// A single "line" longer than this is truncated for display. Minified JavaScript is one
/// line of several megabytes, and nothing good comes of trying to lay that out.
pub(crate) const MAX_LINE: u64 = 64 * 1024;
/// Bytes sniffed to decide text or binary.
const SNIFF: usize = 8192;
/// Give up a search after this much reading rather than freezing the UI.
const SEARCH_BUDGET: u64 = 2 * 1024 * 1024 * 1024;
/// Chunk size for the sequential search scan.
const SCAN_CHUNK: usize = 1 << 20;

pub const HEX_COLS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Text,
    Hex,
}

/// Where a document's bytes are: a file left on disk, or bytes held in memory.
enum Source {
    File(File),
    Memory(Vec<u8>),
}

impl Source {
    /// Up to `buf.len()` bytes from `offset`; fewer at the end, which is not an error.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Source::File(f) => {
                f.seek(SeekFrom::Start(offset))?;
                read_up_to(f, buf)
            }
            Source::Memory(bytes) => {
                let start = (offset as usize).min(bytes.len());
                let n = buf.len().min(bytes.len() - start);
                buf[..n].copy_from_slice(&bytes[start..start + n]);
                Ok(n)
            }
        }
    }
}

pub struct Document {
    /// The file's path, or for a document held in memory the name it is shown under.
    pub path: PathBuf,
    /// Held in memory rather than read from `path`: nothing of it is on disk, and
    /// saving it means writing these bytes, not copying a file.
    pub in_memory: bool,
    source: Source,
    pub len: u64,
    /// Byte offset where each line begins.
    lines: Vec<u64>,
    pub binary: bool,
    /// False when the file has more lines than we were willing to index.
    pub fully_indexed: bool,
    /// How much of the file the index covers. Equal to `len` unless indexing stopped
    /// early, in which case everything past here is simply not shown — gluing the
    /// remainder onto the last line would render megabytes of newlines as one row.
    indexed_len: u64,
    /// The rest of the file being indexed in the background, while it is.
    indexer: Option<Indexer>,
    /// Bytes indexed so far: `len` once the index is complete.
    scanned: u64,
}

/// What the index's thread sends.
enum Found {
    /// Line starts after the last ones sent, and the bytes scanned through.
    Lines(Vec<u64>, u64),
    /// The end: of the file (`None`), or the cap on lines, at the start of the first
    /// line left out (`Some`).
    End(Option<u64>),
}

/// The background half of the index: its messages, and the flag that stops it.
struct Indexer {
    rx: Receiver<Found>,
    stop: Arc<AtomicBool>,
}

impl Drop for Indexer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Line starts in `buf`, read at `offset` of a file of `len` bytes, added to `out`,
/// with `count` the lines known before. `Some(start)`: the cap was reached, and
/// `start` is the first line left out (not in `out`).
fn starts_in(buf: &[u8], offset: u64, len: u64, count: usize, out: &mut Vec<u64>) -> Option<u64> {
    let mut count = count;
    for (i, b) in buf.iter().enumerate() {
        if *b == b'\n' {
            let start = offset + i as u64 + 1;
            // A trailing newline ends the last line; it does not begin an empty one.
            // Without this every text file shows a phantom final row.
            if start < len {
                if count >= MAX_LINES {
                    // Every line kept has a known end, and the document stops here.
                    return Some(start);
                }
                out.push(start);
                count += 1;
            }
        }
    }
    None
}

impl Document {
    pub fn open(path: &Path) -> io::Result<Self> {
        // stat first, following symlinks: a link to /dev/zero is still /dev/zero.
        let meta = std::fs::metadata(path)?;
        if let Some(kind) = crate::entry::special_kind(&meta.file_type()) {
            return Err(io::Error::other(format!("{kind} \u{2014} not opened")));
        }
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();

        let mut head = vec![0u8; SNIFF.min(len as usize)];
        let got = read_up_to(&mut file, &mut head)?;
        head.truncate(got);
        // A NUL byte in the first few kilobytes is the same heuristic `grep` and `file`
        // use, and it is right far more often than any cleverer guess.
        let binary = head.contains(&0);

        let mut doc = Self {
            path: path.to_path_buf(),
            in_memory: false,
            source: Source::File(file),
            len,
            lines: Vec::new(),
            binary,
            fully_indexed: true,
            indexed_len: len,
            indexer: None,
            scanned: 0,
        };
        if !binary {
            doc.index_lines()?;
        }
        Ok(doc)
    }

    /// A document over bytes held in memory, shown as `title`. Nothing is written
    /// anywhere; text or binary is decided as for a file.
    pub fn from_bytes(title: impl Into<PathBuf>, bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        let binary = bytes[..SNIFF.min(bytes.len())].contains(&0);
        let mut doc = Self {
            path: title.into(),
            in_memory: true,
            source: Source::Memory(bytes),
            len,
            lines: Vec::new(),
            binary,
            fully_indexed: true,
            indexed_len: len,
            indexer: None,
            scanned: 0,
        };
        if !binary {
            // Reading memory cannot fail, and it is all indexed here: it is already in
            // memory, and a history is small.
            let _ = doc.index_lines();
        }
        doc
    }

    /// The bytes of a document held in memory, for saving them; `None` for a file,
    /// which is saved by copying it.
    pub fn memory_bytes(&self) -> Option<&[u8]> {
        match &self.source {
            Source::Memory(b) => Some(b),
            Source::File(_) => None,
        }
    }

    /// Index the lines of a file that opened as binary, the first time text is asked
    /// for. Binary files skip it on open, so the hex view of a big one costs nothing;
    /// without this, switching such a file to text showed "0 lines" and an empty
    /// screen. Some files that look binary are text — UTF-16 is full of NUL bytes.
    pub fn ensure_lines(&mut self) -> io::Result<()> {
        if self.lines.is_empty() && self.len > 0 && self.indexer.is_none() {
            self.index_lines()?;
        }
        Ok(())
    }

    /// The first [`INLINE_INDEX`] bytes here (all of a document in memory), the rest
    /// on a thread of its own with its own handle on the file: a slow disk or a share
    /// never holds up a key.
    fn index_lines(&mut self) -> io::Result<()> {
        self.lines.clear();
        self.indexer = None;
        self.fully_indexed = true;
        self.indexed_len = self.len;
        if self.len > 0 {
            self.lines.push(0);
        }
        let inline = match self.source {
            Source::Memory(_) => self.len,
            Source::File(_) => INLINE_INDEX.min(self.len),
        };
        let mut buf = vec![0u8; INDEX_CHUNK];
        let mut offset: u64 = 0;
        while offset < inline {
            let want = ((self.len - offset) as usize).min(buf.len());
            let n = self.source.read_at(offset, &mut buf[..want])?;
            if n == 0 {
                break;
            }
            let count = self.lines.len();
            let mut found = Vec::new();
            let cap = starts_in(&buf[..n], offset, self.len, count, &mut found);
            self.lines.extend(found);
            offset += n as u64;
            self.scanned = offset;
            if let Some(start) = cap {
                self.capped(start);
                return Ok(());
            }
        }
        if offset >= self.len {
            self.scanned = self.len;
            return Ok(());
        }
        match File::open(&self.path) {
            Ok(f) => self.indexer = Some(spawn_indexer(f, offset, self.len, self.lines.len())),
            // Not there to open again (removed since): what is indexed is what is shown.
            Err(_) => {
                let last = self.lines.pop().unwrap_or(0);
                self.capped(last);
            }
        }
        Ok(())
    }

    /// The index stopped at `start`, the first line left out.
    fn capped(&mut self, start: u64) {
        self.fully_indexed = false;
        self.indexed_len = start;
        self.scanned = self.len;
    }

    /// Take in what the background index found since the last call. True when
    /// anything changed (more lines, or the end).
    pub fn poll_index(&mut self) -> bool {
        let Some(ix) = self.indexer.take() else {
            return false;
        };
        let mut moved = false;
        loop {
            match ix.rx.try_recv() {
                Ok(Found::Lines(starts, upto)) => {
                    self.lines.extend(starts);
                    self.scanned = upto;
                    moved = true;
                }
                Ok(Found::End(None)) => {
                    self.scanned = self.len;
                    return true;
                }
                Ok(Found::End(Some(start))) => {
                    self.capped(start);
                    return true;
                }
                Err(TryRecvError::Empty) => {
                    self.indexer = Some(ix);
                    return moved;
                }
                // The thread failed (a read error): the line being read has no known
                // end, so the document stops before it.
                Err(TryRecvError::Disconnected) => {
                    let last = self.lines.pop().unwrap_or(0);
                    self.capped(last);
                    return true;
                }
            }
        }
    }

    /// Whether the rest of the file is still being indexed.
    pub fn indexing(&self) -> bool {
        self.indexer.is_some()
    }

    /// How far the index has got, 0 to 100.
    pub fn indexed_percent(&self) -> u8 {
        (self.scanned.min(self.len) * 100)
            .checked_div(self.len)
            .map_or(100, |p| p as u8)
    }

    /// Wait for the whole index (tests, and callers that need every line).
    pub fn wait_indexed(&mut self) {
        while self.indexing() {
            if !self.poll_index() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    /// Where the text shown so far ends: the start of the line still being indexed,
    /// whose end is not known yet; once indexed, [`Document::indexed_len`]'s value.
    fn text_end(&self) -> u64 {
        match (&self.indexer, self.lines.last()) {
            (Some(_), Some(&last)) => last,
            _ => self.indexed_len,
        }
    }

    /// Where line `i` starts in the file; for `i == line_count()`, where the text shown
    /// ends. `None` past that.
    pub fn line_start(&self, i: usize) -> Option<u64> {
        let count = self.line_count();
        match i.cmp(&count) {
            std::cmp::Ordering::Less => self.lines.get(i).copied(),
            std::cmp::Ordering::Equal => Some(self.text_end()),
            std::cmp::Ordering::Greater => None,
        }
    }

    /// Lines known whole. While the index runs, the line it is in the middle of is not
    /// one of them yet.
    pub fn line_count(&self) -> usize {
        if self.indexer.is_some() {
            self.lines.len().saturating_sub(1)
        } else {
            self.lines.len()
        }
    }

    pub fn hex_rows(&self) -> usize {
        self.len.div_ceil(HEX_COLS) as usize
    }

    /// One line, with its terminator removed and over-long content cut.
    pub fn line(&mut self, i: usize) -> io::Result<String> {
        if i >= self.line_count() {
            return Ok(String::new());
        }
        let (Some(start), Some(end)) = (self.line_start(i), self.line_start(i + 1)) else {
            return Ok(String::new());
        };
        let want = end.saturating_sub(start).min(MAX_LINE) as usize;
        let mut buf = vec![0u8; want];
        let got = self.source.read_at(start, &mut buf)?;
        buf.truncate(got);
        Ok(line_text(buf))
    }

    pub fn lines_from(&mut self, from: usize, count: usize) -> Vec<String> {
        (from..(from + count).min(self.line_count()))
            .map(|i| self.line(i).unwrap_or_default())
            .collect()
    }

    /// Raw bytes for the hex view.
    pub fn bytes_at(&mut self, offset: u64, want: usize) -> Vec<u8> {
        if offset >= self.len {
            return Vec::new();
        }
        let want = want.min((self.len - offset) as usize);
        let mut buf = vec![0u8; want];
        match self.source.read_at(offset, &mut buf) {
            Ok(n) => {
                buf.truncate(n);
                buf
            }
            Err(_) => Vec::new(),
        }
    }

    /// Case-insensitive search over the file's bytes, reported as a line index.
    ///
    /// Scans sequentially in chunks rather than reading line by line: on a 337 MB log
    /// the per-line version did four million seeks and took 1.2 s with the UI frozen.
    /// Folding is ASCII-only, which is what `grep -i` gives you without a locale.
    pub fn find(&mut self, needle: &str, from: usize, backwards: bool) -> Option<usize> {
        let count = self.line_count();
        if needle.is_empty() || count == 0 {
            return None;
        }
        let pat = needle.as_bytes().to_ascii_lowercase();
        if backwards {
            let end = *self.lines.get(from.min(count - 1))?;
            self.scan(0, end, &pat, true)
        } else {
            if from + 1 >= count {
                return None;
            }
            let start = self.lines.get(from + 1).copied()?;
            let end = self.text_end();
            self.scan(start, end, &pat, false)
        }
    }

    /// Byte offset -> line index.
    fn line_of(&self, offset: u64) -> usize {
        self.lines
            .partition_point(|&s| s <= offset)
            .saturating_sub(1)
    }

    fn scan(&mut self, from: u64, to: u64, pat: &[u8], want_last: bool) -> Option<usize> {
        if from >= to || pat.is_empty() {
            return None;
        }
        let overlap = pat.len() - 1;
        let mut buf = vec![0u8; SCAN_CHUNK + overlap];
        let mut pos = from;
        let mut budget = SEARCH_BUDGET;
        let mut last_hit: Option<u64> = None;

        while pos < to && budget > 0 {
            let want = ((to - pos) as usize).min(buf.len());
            let n = self.source.read_at(pos, &mut buf[..want]).ok()?;
            if n == 0 {
                break;
            }
            budget = budget.saturating_sub(n as u64);
            let hay = &mut buf[..n];
            hay.make_ascii_lowercase();

            let mut i = 0;
            while let Some(rel) = find_sub(&hay[i..], pat) {
                let at = pos + (i + rel) as u64;
                if !want_last {
                    return Some(self.line_of(at));
                }
                last_hit = Some(at);
                i += rel + 1;
            }

            if n < want || n <= overlap {
                break;
            }
            // Step back by the pattern length so a match straddling the boundary is
            // not missed.
            pos += (n - overlap) as u64;
        }
        last_hit.map(|at| self.line_of(at))
    }
}

/// The rest of a file's index, from `offset` (`count` lines known) to `len`, on a thread
/// of its own reading `f`.
fn spawn_indexer(mut f: File, offset: u64, len: u64, count: usize) -> Indexer {
    let (tx, rx) = std::sync::mpsc::sync_channel(INDEX_QUEUE);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let work = move || {
        if f.seek(SeekFrom::Start(offset)).is_err() {
            return;
        }
        let mut buf = vec![0u8; INDEX_CHUNK];
        let (mut offset, mut count) = (offset, count);
        while offset < len {
            if flag.load(Ordering::Relaxed) {
                return;
            }
            let want = ((len - offset) as usize).min(buf.len());
            let n = match read_up_to(&mut f, &mut buf[..want]) {
                Ok(0) => break,
                Ok(n) => n,
                // Dropped without an end: the document stops where it got to.
                Err(_) => return,
            };
            let mut found = Vec::new();
            let cap = starts_in(&buf[..n], offset, len, count, &mut found);
            count += found.len();
            offset += n as u64;
            // The viewer gone (the receiver dropped): stop.
            if tx.send(Found::Lines(found, offset)).is_err() {
                return;
            }
            if let Some(start) = cap {
                let _ = tx.send(Found::End(Some(start)));
                return;
            }
        }
        let _ = tx.send(Found::End(None));
    };
    // No thread to be had: the work and its sender are dropped, the receiver sees a
    // disconnect, and the document stops at what was indexed inline.
    let _ = std::thread::Builder::new().name("index".into()).spawn(work);
    Indexer { rx, stop }
}

/// A line as the viewer shows it, from its first bytes (at most [`MAX_LINE`], the
/// terminator counted): the terminator removed, invalid UTF-8 replaced. The one rule,
/// shared with the highlighter's scan ([`crate::syntax::Scan`]), so a line coloured from
/// the scan's checkpoints is the line drawn.
pub(crate) fn line_text(mut buf: Vec<u8>) -> String {
    while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
        buf.pop();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// The name Save as offers for a shell's history: `xdevcmd-history-2026-11-10-142233.txt`,
/// local time, so two saves a second apart never offer the same name.
pub fn history_file_name() -> String {
    chrono::Local::now()
        .format("xdevcmd-history-%Y-%m-%d-%H%M%S.txt")
        .to_string()
}

/// Write a document held in memory to `path` (Save as…). Behind the UI's own refusal,
/// the engine's wall: nothing is written in read-only mode.
///
/// Without `replace`, an existing `path` is never touched: the file is made with
/// `create_new`, so a name taken between the question and the write fails with
/// `AlreadyExists` instead of being overwritten. With `replace`, the bytes go to a
/// hidden temporary name beside `path` and are renamed over it only once complete, so
/// the old file stays whole if anything fails on the way. `private` makes the file
/// readable by its owner only (`0600`) on Unix, for a shell's output, which can hold
/// tokens and customer data; Windows gives a new file its directory's permissions.
pub fn save_bytes(path: &Path, bytes: &[u8], replace: bool, private: bool) -> io::Result<()> {
    crate::policy::guard_write()?;
    if !replace {
        return write_new(path, bytes, private);
    }
    let part = crate::ops::part_name(path);
    let f = match create(&part, bytes, private) {
        Ok(f) => f,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    // Synced while open, then renamed over: the one way a file is replaced.
    crate::ops::commit_replace(f, &part, path, || Ok(true)).map(|_| ())
}

/// A new file at `path` holding `bytes`; removed again if the write fails, so a full
/// disk does not leave half of it under a name that looks finished.
fn write_new(path: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    // `create` removes what it made if the write fails, and never touches a file that
    // was already there (`AlreadyExists`).
    let f = create(path, bytes, private)?;
    let synced = crate::faults::sync_data(&f, path);
    if synced.is_err() {
        drop(f);
        let _ = std::fs::remove_file(path);
    }
    synced
}

/// `path`, made new, holding `bytes`, still open; not synced.
fn create(path: &Path, bytes: &[u8], private: bool) -> io::Result<std::fs::File> {
    use std::io::Write as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(if private { 0o600 } else { 0o666 });
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut f = opts.open(path)?;
    if let Err(e) = f.write_all(bytes) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(f)
}

/// First occurrence of `pat` in `hay`. Short needles over megabyte chunks, so the
/// obvious scan is fine and avoids a dependency.
fn find_sub(hay: &[u8], pat: &[u8]) -> Option<usize> {
    if pat.len() > hay.len() {
        return None;
    }
    let first = pat[0];
    let mut i = 0;
    while let Some(rel) = hay[i..hay.len() - pat.len() + 1]
        .iter()
        .position(|b| *b == first)
    {
        let at = i + rel;
        if &hay[at..at + pat.len()] == pat {
            return Some(at);
        }
        i = at + 1;
        if i + pat.len() > hay.len() {
            break;
        }
    }
    None
}

/// `read` until the buffer is full or the file ends. `read_exact` is wrong here: a short
/// read at the end of a file is normal, not an error.
fn read_up_to(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        match f.read(&mut buf[done..]) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Save as over a file: synced before it is renamed over the old one.
    #[test]
    fn saving_over_a_file_syncs_before_renaming() {
        let d = std::env::temp_dir().join(format!("xdevcmd-doc-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("s.txt"), b"old").unwrap();
        let frag = d.to_string_lossy().into_owned();
        crate::faults::trace(&frag);
        save_bytes(&d.join("s.txt"), b"new", true, false).unwrap();
        let seen: Vec<&str> = crate::faults::traced(&frag)
            .into_iter()
            .map(|(p, _)| p.name())
            .collect();
        assert_eq!(seen, ["sync-data", "rename"]);
        assert_eq!(std::fs::read(d.join("s.txt")).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    /// One parent per run, `xdevcmd-doc-<pid>`; the first call removes the parents of runs
    /// more than an hour old. These tests write a few megabytes each and never cleaned
    /// up, which left gigabytes in a developer's temp directory over time.
    fn write(name: &str, body: &[u8]) -> PathBuf {
        static SWEPT: std::sync::Once = std::sync::Once::new();
        SWEPT.call_once(|| {
            let own = format!("xdevcmd-doc-{}", std::process::id());
            let hour = std::time::Duration::from_secs(3600);
            for e in fs::read_dir(std::env::temp_dir())
                .into_iter()
                .flatten()
                .flatten()
            {
                let name = e.file_name().to_string_lossy().into_owned();
                let old = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > hour);
                if name.starts_with("xdevcmd-doc-") && name != own && old {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
        });
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir()
            .join(format!("xdevcmd-doc-{}", std::process::id()))
            .join(n.to_string());
        fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn counts_lines_without_inventing_a_trailing_one() {
        let p = write("a.txt", b"one\ntwo\nthree\n");
        let mut d = Document::open(&p).unwrap();
        assert_eq!(
            d.line_count(),
            3,
            "a trailing newline ends a line, it does not add one"
        );
        assert_eq!(d.line(0).unwrap(), "one");
        assert_eq!(d.line(2).unwrap(), "three");
    }

    #[test]
    fn handles_a_file_with_no_final_newline() {
        let p = write("b.txt", b"one\ntwo");
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.line_count(), 2);
        assert_eq!(d.line(1).unwrap(), "two");
    }

    #[test]
    fn strips_crlf() {
        let p = write("c.txt", b"one\r\ntwo\r\n");
        let mut d = Document::open(&p).unwrap();
        assert_eq!(
            d.line(0).unwrap(),
            "one",
            "a stray \\r would draw as a glyph"
        );
        assert_eq!(d.line(1).unwrap(), "two");
    }

    #[test]
    fn an_empty_file_has_no_lines() {
        let p = write("empty.txt", b"");
        let d = Document::open(&p).unwrap();
        assert_eq!(d.line_count(), 0);
        assert_eq!(d.len, 0);
    }

    #[test]
    fn detects_binary_by_a_nul_byte() {
        let text = write("t.txt", b"plain text here\n");
        assert!(!Document::open(&text).unwrap().binary);

        let bin = write("t.bin", b"\x7fELF\x02\x01\x01\x00\x00\x00");
        assert!(Document::open(&bin).unwrap().binary);
    }

    #[test]
    fn a_pathological_single_line_is_truncated_not_hung() {
        let huge = vec![b'x'; 3 * 1024 * 1024];
        let p = write("min.js", &huge);
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.line_count(), 1);
        assert_eq!(d.line(0).unwrap().len(), MAX_LINE as usize);
    }

    #[test]
    fn reads_a_window_from_the_middle_of_a_big_file() {
        let body: String = (0..50_000).map(|i| format!("line {i}\n")).collect();
        let p = write("big.txt", body.as_bytes());
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.line_count(), 50_000);
        assert_eq!(d.lines_from(49_998, 10), vec!["line 49998", "line 49999"]);
        assert_eq!(d.line(12_345).unwrap(), "line 12345");
    }

    /// A03: past [`INLINE_INDEX`], open returns with the first part indexed and the
    /// rest comes in behind; every line shown is whole, and a search does not pretend
    /// it looked at what is not indexed yet.
    #[test]
    fn a_big_file_opens_at_once_and_its_index_comes_in_behind() {
        let body: String = (0..1_000_000).map(|i| format!("line {i}\n")).collect();
        assert!(body.len() as u64 > 2 * INLINE_INDEX);
        let p = write("progressive.txt", body.as_bytes());
        let mut d = Document::open(&p).unwrap();
        // Nothing taken in yet: only the inline part, and the line it stopped in the
        // middle of is not counted.
        assert!(d.indexing());
        let first = d.line_count();
        assert!(first > 0 && first < 1_000_000, "{first}");
        assert_eq!(d.line(first - 1).unwrap(), format!("line {}", first - 1));
        assert_eq!(d.line(first).unwrap(), "", "past what is known: nothing");
        assert_eq!(d.find("line 999999", 0, false), None);
        assert!(d.indexed_percent() < 100);

        d.wait_indexed();
        assert!(!d.indexing() && d.fully_indexed);
        assert_eq!(d.line_count(), 1_000_000);
        assert_eq!(d.indexed_percent(), 100);
        assert_eq!(d.line(999_999).unwrap(), "line 999999");
        assert_eq!(d.line(first).unwrap(), format!("line {first}"));
        assert_eq!(d.find("line 999999", 0, false), Some(999_999));
    }

    /// The cap on lines, reached by the background index: the document stops at a
    /// line's end, as when it is reached inline.
    #[test]
    fn the_cap_reached_in_the_background_stops_at_a_whole_line() {
        let mut body = Vec::new();
        while (body.len() as u64) < INLINE_INDEX + 1000 {
            body.extend_from_slice(&[b'a'; 999]);
            body.push(b'\n');
        }
        let before = body.len() / 1000;
        body.extend(std::iter::repeat_n(b'\n', MAX_LINES));
        body.extend_from_slice(b"tail\n");
        let p = write("capped.txt", &body);
        let mut d = Document::open(&p).unwrap();
        assert!(d.indexing() && d.fully_indexed);
        d.wait_indexed();
        assert!(!d.fully_indexed);
        assert_eq!(d.line_count(), MAX_LINES);
        assert_eq!(d.line(0).unwrap().len(), 999);
        assert_eq!(d.line(before).unwrap(), "");
        let last = d.line_count() - 1;
        assert!(!d.line(last).unwrap().contains('\n'));
    }

    /// Dropped while indexing (the viewer closed with Esc): the thread is told to stop,
    /// and nothing waits for it.
    #[test]
    fn a_document_dropped_while_indexing_does_not_wait_for_its_index() {
        let body: String = (0..1_000_000).map(|i| format!("row {i}\n")).collect();
        let p = write("dropped.txt", body.as_bytes());
        let t = std::time::Instant::now();
        let d = Document::open(&p).unwrap();
        assert!(d.indexing());
        drop(d);
        assert!(
            t.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn hex_reads_bytes_at_an_offset() {
        let p = write("h.bin", b"ABCDEFGHIJ");
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.bytes_at(4, 3), b"EFG");
        assert_eq!(
            d.bytes_at(8, 16),
            b"IJ",
            "a short read at the end is not an error"
        );
        assert_eq!(d.bytes_at(99, 4), b"");
        assert_eq!(d.hex_rows(), 1);
    }

    #[test]
    fn a_truncated_index_does_not_glue_the_rest_of_the_file_into_one_line() {
        // Cheap stand-in for the real cap: the invariant is that the last line the
        // viewer shows ends at a newline, never at EOF.
        let body: String = (0..1000).map(|i| format!("row {i}\n")).collect();
        let p = write("trunc.txt", body.as_bytes());
        let mut d = Document::open(&p).unwrap();
        let last = d.line_count() - 1;
        let text = d.line(last).unwrap();
        assert!(
            !text.contains('\n'),
            "a line must never contain a newline; got {} bytes",
            text.len()
        );
        assert_eq!(text, "row 999");
    }

    #[test]
    fn finds_a_match_far_into_a_large_file() {
        let body: String = (0..200_000).map(|i| format!("record {i}\n")).collect();
        let p = write("scan.txt", body.as_bytes());
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.find("record 199999", 0, false), Some(199_999));
        assert_eq!(d.find("RECORD 123456", 0, false), Some(123_456));
    }

    #[test]
    fn a_match_spanning_a_chunk_boundary_is_still_found() {
        // Pad so the needle straddles the 1 MiB read boundary.
        let mut body = vec![b'.'; SCAN_CHUNK - 4];
        body.extend_from_slice(b"\nneedle-here\n");
        let p = write("boundary.txt", &body);
        let mut d = Document::open(&p).unwrap();
        assert_eq!(
            d.find("needle-here", 0, false),
            Some(1),
            "a chunked scan must overlap its reads"
        );
    }

    #[test]
    fn backwards_search_returns_the_nearest_earlier_match() {
        let p = write("back.txt", b"hit\nmiss\nhit\nmiss\nstart\n");
        let mut d = Document::open(&p).unwrap();
        assert_eq!(d.find("hit", 4, true), Some(2), "nearest, not the first");
        assert_eq!(d.find("hit", 2, true), Some(0));
        assert_eq!(d.find("hit", 0, true), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_is_refused_instead_of_blocking_forever() {
        let p = write("placeholder", b"");
        let fifo = p.with_file_name("pipe");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        // Before: File::open on a FIFO with no writer never returned.
        let e = Document::open(&fifo).err().expect("must refuse, not block");
        assert!(e.to_string().contains("FIFO"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_device_is_refused() {
        let p = write("placeholder", b"");
        let link = p.with_file_name("zero");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        let e = Document::open(&link)
            .err()
            .expect("/dev/zero has no end to index");
        assert!(e.to_string().contains("device"), "{e}");
    }

    #[test]
    fn a_document_in_memory_reads_as_a_file_with_the_same_bytes() {
        let body: Vec<u8> = (0..3000)
            .flat_map(|i| format!("row {i} \x1b-free text\n").into_bytes())
            .collect();
        let p = write("same.txt", &body);
        let mut file = Document::open(&p).unwrap();
        let mut mem = Document::from_bytes("history \u{b7} sh \u{b7} tab 1", body.clone());
        assert!(mem.in_memory && !file.in_memory);
        assert_eq!(mem.path, PathBuf::from("history \u{b7} sh \u{b7} tab 1"));
        assert_eq!(mem.len, file.len);
        assert_eq!(mem.line_count(), file.line_count());
        assert_eq!(mem.lines_from(2990, 20), file.lines_from(2990, 20));
        assert_eq!(mem.bytes_at(100, 64), file.bytes_at(100, 64));
        assert_eq!(mem.hex_rows(), file.hex_rows());
        assert_eq!(mem.find("ROW 2500", 0, false), Some(2500));
        assert_eq!(
            mem.find("row 10 ", 2500, true),
            file.find("row 10 ", 2500, true)
        );
        assert_eq!(mem.memory_bytes(), Some(&body[..]));
        assert_eq!(file.memory_bytes(), None);
    }

    #[test]
    fn a_document_in_memory_sniffs_binary_and_handles_empty() {
        let bin = Document::from_bytes("b", b"ab\0cd".to_vec());
        assert!(bin.binary);
        assert_eq!(
            bin.line_count(),
            0,
            "binary skips the index, as a file does"
        );
        let mut empty = Document::from_bytes("e", Vec::new());
        assert_eq!(empty.line_count(), 0);
        assert_eq!(empty.bytes_at(0, 16), b"");
        assert_eq!(empty.find("x", 0, false), None);
        let mut one = Document::from_bytes("o", b"no newline".to_vec());
        assert_eq!(one.line_count(), 1);
        assert_eq!(one.line(0).unwrap(), "no newline");
    }

    #[test]
    fn save_bytes_never_replaces_without_being_told() {
        let p = write("saved.txt", b"first");
        let e = save_bytes(&p, b"second", false, true).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&p).unwrap(), b"first", "the old file is untouched");
        save_bytes(&p, b"second", true, true).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"second");
        // No temporary name left beside it.
        let dir = p.parent().unwrap();
        let names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("saved.txt")]);
    }

    #[test]
    fn a_failed_replacement_leaves_the_old_file_and_no_part() {
        // A directory where the file should be: the rename over it fails.
        let p = write("keep.txt", b"keep");
        let dir = p.with_file_name("a-dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("inside"), b"x").unwrap();
        assert!(save_bytes(&dir, b"new", true, true).is_err());
        assert!(dir.is_dir(), "the directory is still there");
        let names: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !names.iter().any(|n| n.contains("xdevcmd-part")),
            "{names:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn saved_history_is_the_owners_alone() {
        use std::os::unix::fs::PermissionsExt;
        let p = write("placeholder", b"");
        let new = p.with_file_name("history.txt");
        save_bytes(&new, b"token=abc\n", false, true).unwrap();
        let mode = fs::metadata(&new).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // A replacement too, whatever the old file's mode was.
        fs::set_permissions(&new, fs::Permissions::from_mode(0o644)).unwrap();
        save_bytes(&new, b"again\n", true, true).unwrap();
        let mode = fs::metadata(&new).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn the_history_name_carries_date_and_time() {
        let n = history_file_name();
        assert!(
            n.starts_with("xdevcmd-history-20") && n.ends_with(".txt"),
            "{n}"
        );
        assert_eq!(n.len(), "xdevcmd-history-2026-11-10-142233.txt".len());
    }

    #[test]
    fn search_moves_forwards_and_backwards_and_ignores_case() {
        let p = write("s.txt", b"alpha\nBETA\ngamma\nbeta again\n");
        let mut d = Document::open(&p).unwrap();

        assert_eq!(d.find("beta", 0, false), Some(1), "case must not matter");
        assert_eq!(
            d.find("beta", 1, false),
            Some(3),
            "must move past the current hit"
        );
        assert_eq!(d.find("beta", 3, false), None, "no wrap-around");
        assert_eq!(d.find("beta", 3, true), Some(1));
        assert_eq!(d.find("nowhere", 0, false), None);
        assert_eq!(d.find("", 0, false), None);
    }
}
