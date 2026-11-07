//! A file opened for viewing, without reading it all into memory.
//!
//! The whole point of a viewer in a file manager is that it opens a 2 GB log instantly.
//! So the file stays on disk: opening streams through it once to record where each line
//! starts (eight bytes per line), and everything after that is a seek and a short read of
//! only what is on screen.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Stop indexing here. A file with more lines than this is still viewable; the viewer
/// says it is showing the first part rather than pretending otherwise.
const MAX_LINES: usize = 4_000_000;
/// A single "line" longer than this is truncated for display. Minified JavaScript is one
/// line of several megabytes, and nothing good comes of trying to lay that out.
const MAX_LINE: u64 = 64 * 1024;
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

pub struct Document {
    pub path: PathBuf,
    file: File,
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
            file,
            len,
            lines: Vec::new(),
            binary,
            fully_indexed: true,
            indexed_len: len,
        };
        if !binary {
            doc.index_lines()?;
        }
        Ok(doc)
    }

    /// Index the lines of a file that opened as binary, the first time text is asked
    /// for. Binary files skip it on open, so the hex view of a big one costs nothing;
    /// without this, switching such a file to text showed "0 lines" and an empty
    /// screen. Some files that look binary are text — UTF-16 is full of NUL bytes.
    pub fn ensure_lines(&mut self) -> io::Result<()> {
        if self.lines.is_empty() && self.len > 0 {
            self.index_lines()?;
        }
        Ok(())
    }

    fn index_lines(&mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut reader = BufReader::with_capacity(256 * 1024, &self.file);
        let mut buf = [0u8; 256 * 1024];
        let mut offset: u64 = 0;
        self.lines.clear();
        if self.len > 0 {
            self.lines.push(0);
        }
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            for (i, b) in buf[..n].iter().enumerate() {
                if *b == b'\n' {
                    let start = offset + i as u64 + 1;
                    // A trailing newline ends the last line; it does not begin an empty
                    // one. Without this every text file shows a phantom final row.
                    if start < self.len {
                        self.lines.push(start);
                        if self.lines.len() >= MAX_LINES {
                            // Drop the line we just started so every remaining line has
                            // a known end, and stop the document here.
                            self.fully_indexed = false;
                            self.indexed_len = self.lines.pop().unwrap_or(self.len);
                            return Ok(());
                        }
                    }
                }
            }
            offset += n as u64;
        }
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn hex_rows(&self) -> usize {
        self.len.div_ceil(HEX_COLS) as usize
    }

    /// One line, with its terminator removed and over-long content cut.
    pub fn line(&mut self, i: usize) -> io::Result<String> {
        let Some(&start) = self.lines.get(i) else {
            return Ok(String::new());
        };
        let end = self.lines.get(i + 1).copied().unwrap_or(self.indexed_len);
        let want = end.saturating_sub(start).min(MAX_LINE) as usize;
        let mut buf = vec![0u8; want];
        self.file.seek(SeekFrom::Start(start))?;
        let got = read_up_to(&mut self.file, &mut buf)?;
        buf.truncate(got);
        while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
            buf.pop();
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    pub fn lines_from(&mut self, from: usize, count: usize) -> Vec<String> {
        (from..(from + count).min(self.lines.len()))
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
        if self.file.seek(SeekFrom::Start(offset)).is_err() {
            return Vec::new();
        }
        match read_up_to(&mut self.file, &mut buf) {
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
        if needle.is_empty() || self.lines.is_empty() {
            return None;
        }
        let pat = needle.as_bytes().to_ascii_lowercase();
        if backwards {
            let end = *self.lines.get(from.min(self.lines.len() - 1))?;
            self.scan(0, end, &pat, true)
        } else {
            let start = self.lines.get(from + 1).copied()?;
            self.scan(start, self.indexed_len, &pat, false)
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
            self.file.seek(SeekFrom::Start(pos)).ok()?;
            let n = read_up_to(&mut self.file, &mut buf[..want]).ok()?;
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
