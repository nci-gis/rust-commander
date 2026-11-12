//! A terminal's output written to a file while it runs (Round 027).
//!
//! The terminal's reader thread hands each chunk it reads to [`Log::tap`], before the
//! screen parser sees it, so what scrolls past the scrollback is still logged. The
//! reader never waits on the disk: chunks go through a bounded queue to a writer
//! thread of the log's own, which filters (`text`) and writes. When the queue stays
//! full or a write fails, the log stops and its [`Status`] says why; it never goes on
//! with a hole in it.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// What goes into the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Escapes stripped, each line as the terminal left it, written once.
    Text,
    /// The bytes as the program wrote them.
    Raw,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Text => "text",
            Mode::Raw => "raw",
        }
    }
}

/// Past this many characters without a line feed, a line is written and a new one
/// begun: output with no line breaks (`cat` of a binary) must not grow without bound.
pub const MAX_LINE: usize = 16 * 1024;

/// Chunks the queue holds; a chunk is at most one read of the terminal (8 KiB), so
/// about 2 MiB waits for the disk at most.
pub const QUEUE: usize = 256;

/// How long the reader waits for room in a full queue before it gives the log up.
pub const PATIENCE: Duration = Duration::from_millis(500);

/// The line a text log gets where a full-screen program ran.
const FULL_SCREEN: &str =
    "[xdevcmd: a full-screen program ran here; its screen is not in a text log]";

/// The text log's own name for a file: `xdevcmd-<shell>-<date>-<time>.log`, the shell's
/// name reduced to letters, digits, `-` and `_`.
pub fn file_name(shell: &str) -> String {
    let mut name: String = shell
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(24)
        .collect();
    if name.is_empty() {
        name.push_str("shell");
    }
    format!(
        "xdevcmd-{name}-{}.log",
        chrono::Local::now().format("%Y-%m-%d-%H%M%S")
    )
}

/// Open `path` to log into: appended to when it exists, created readable by its owner
/// only (`0600`, Unix) when it does not. Refused in read-only mode, outside the root,
/// and for anything but a regular file (a link someone left there, a FIFO that would
/// block, a device). True when the file existed.
pub fn open(path: &Path) -> io::Result<(File, bool)> {
    crate::policy::guard_write()?;
    let policy = crate::policy::get();
    if policy.root.is_some() && !path.parent().is_some_and(|p| policy.allows(p)) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "outside the root xdevcmd is confined to",
        ));
    }
    let (f, existed) = open_append(path)?;
    #[cfg(unix)]
    if existed {
        // Someone else's file, writable by us (a shared log directory): what this
        // shell prints would land in a file they own and control. Refused.
        use std::os::unix::fs::MetadataExt as _;
        if f.metadata()?.uid() != rustix::process::geteuid().as_raw() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "that file belongs to another user \u{2014} choose another name",
            ));
        }
    }
    Ok((f, existed))
}

fn not_regular() -> io::Error {
    io::Error::other("not a regular file \u{2014} choose another name")
}

/// The file itself, with nothing between a look and the open for something to be
/// swapped into: a new file is made with `create_new` (`O_EXCL`, which never follows a
/// link nor opens what is there); an existing one is opened for appending with
/// `O_NOFOLLOW` (a link is refused, not followed) and `O_NONBLOCK` (a FIFO with nobody
/// reading fails at once instead of blocking the UI thread; one with a reader opens and
/// is refused by the check below). `O_NONBLOCK` stays set: it changes nothing for a
/// regular file, whose writes never return "would block". Then `fstat`: anything but a
/// regular file is refused. Windows: an existing name is opened with
/// `FILE_FLAG_OPEN_REPARSE_POINT`, so a link is opened as itself and then refused as
/// not a regular file (not tested on Windows).
fn open_append(path: &Path) -> io::Result<(File, bool)> {
    let opts = |create: bool| {
        let mut o = OpenOptions::new();
        if create {
            o.write(true).create_new(true);
        } else {
            o.append(true);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            o.mode(0o600);
            o.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            o.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        o
    };
    let (f, existed) = match opts(true).open(path) {
        Ok(f) => (f, false),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match opts(false).open(path) {
            Ok(f) => (f, true),
            #[cfg(unix)]
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
                return Err(io::Error::other(
                    "a link \u{2014} choose the file itself or another name",
                ))
            }
            #[cfg(unix)]
            Err(e) if e.raw_os_error() == Some(libc::ENXIO) => return Err(not_regular()),
            Err(e) => return Err(e),
        },
        Err(e) => return Err(e),
    };
    if !f.metadata()?.is_file() {
        return Err(not_regular());
    }
    Ok((f, existed))
}

/// Whether others than the owner may read the file (Unix); false elsewhere.
pub fn readable_by_others(f: &File) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        f.metadata()
            .map(|m| m.permissions().mode() & 0o044 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = f;
        false
    }
}

// ── text mode ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Esc,
    /// ESC, then intermediates (0x20–0x2F), waiting for the final byte.
    EscInter,
    Csi,
    /// OSC (ended by BEL or ST), or DCS, SOS, PM, APC (ended by ST).
    Str,
    /// ESC inside a string: `\` ends it.
    StrEsc,
}

/// A terminal's output as text: escapes removed, each line written once, as the
/// terminal left it. Its state survives between [`TextFilter::feed`] calls, so a
/// sequence or a character split between two reads is taken whole.
#[derive(Debug)]
pub struct TextFilter {
    state: State,
    /// A CSI's parameter and intermediate bytes, bounded.
    csi: Vec<u8>,
    /// A UTF-8 character begun and not finished.
    utf8: Vec<u8>,
    line: Vec<char>,
    col: usize,
    /// A full-screen program has the alternate screen.
    alt: bool,
}

impl Default for TextFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextFilter {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            csi: Vec::new(),
            utf8: Vec::new(),
            line: Vec::new(),
            col: 0,
            alt: false,
        }
    }

    /// Filter `input`, appending finished lines to `out`.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        for &b in input {
            self.byte(b, out);
        }
    }

    /// The end of the output: a line not ended by a line feed is written too.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if !self.utf8.is_empty() {
            self.utf8.clear();
            self.put('\u{fffd}', out);
        }
        if !self.line.is_empty() {
            self.emit(out);
        }
    }

    fn byte(&mut self, b: u8, out: &mut Vec<u8>) {
        match self.state {
            State::Ground => self.ground(b, out),
            State::Esc => self.esc(b),
            State::EscInter => match b {
                0x20..=0x2f => {}
                0x1b => self.state = State::Esc,
                _ => self.state = State::Ground,
            },
            State::Csi => match b {
                0x40..=0x7e => {
                    self.state = State::Ground;
                    self.csi_final(b, out);
                }
                0x20..=0x3f => {
                    if self.csi.len() < 64 {
                        self.csi.push(b);
                    }
                }
                0x1b => self.state = State::Esc,
                0x18 | 0x1a => self.state = State::Ground,
                _ => {}
            },
            State::Str => match b {
                0x07 => self.state = State::Ground,
                0x1b => self.state = State::StrEsc,
                0x18 | 0x1a => self.state = State::Ground,
                _ => {}
            },
            State::StrEsc => {
                if b == b'\\' {
                    self.state = State::Ground;
                } else {
                    self.state = State::Esc;
                    self.esc(b);
                }
            }
        }
    }

    fn esc(&mut self, b: u8) {
        self.state = match b {
            b'[' => {
                self.csi.clear();
                State::Csi
            }
            b']' | b'P' | b'X' | b'^' | b'_' => State::Str,
            0x20..=0x2f => State::EscInter,
            0x1b => State::Esc,
            _ => State::Ground,
        };
    }

    fn ground(&mut self, b: u8, out: &mut Vec<u8>) {
        if !self.utf8.is_empty() {
            if (0x80..=0xbf).contains(&b) {
                self.utf8.push(b);
                let want = match self.utf8[0] {
                    0xc0..=0xdf => 2,
                    0xe0..=0xef => 3,
                    _ => 4,
                };
                if self.utf8.len() == want {
                    let bytes = std::mem::take(&mut self.utf8);
                    let c = std::str::from_utf8(&bytes)
                        .ok()
                        .and_then(|s| s.chars().next())
                        .unwrap_or('\u{fffd}');
                    self.put(c, out);
                }
                return;
            }
            // Cut short: what there was is not a character.
            self.utf8.clear();
            self.put('\u{fffd}', out);
        }
        match b {
            0x1b => self.state = State::Esc,
            b'\n' => {
                if !self.alt {
                    self.emit(out);
                }
            }
            b'\r' => self.col = 0,
            0x08 => self.col = self.col.saturating_sub(1),
            b'\t' => self.put('\t', out),
            0x00..=0x1f | 0x7f => {}
            0x20..=0x7e => self.put(b as char, out),
            0xc2..=0xf4 => self.utf8.push(b),
            _ => self.put('\u{fffd}', out),
        }
    }

    /// A character at the cursor: over what is there, or past the end.
    fn put(&mut self, c: char, out: &mut Vec<u8>) {
        if self.alt || ('\u{80}'..='\u{9f}').contains(&c) {
            return;
        }
        if self.col < self.line.len() {
            self.line[self.col] = c;
        } else {
            self.line.resize(self.col, ' ');
            self.line.push(c);
        }
        self.col = self.col.saturating_add(1);
        if self.line.len() >= MAX_LINE {
            self.emit(out);
        }
    }

    fn emit(&mut self, out: &mut Vec<u8>) {
        let end = self
            .line
            .iter()
            .rposition(|c| *c != ' ')
            .map_or(0, |i| i + 1);
        let mut b = [0u8; 4];
        for c in &self.line[..end] {
            out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
        }
        out.push(b'\n');
        self.line.clear();
        self.col = 0;
    }

    fn csi_final(&mut self, f: u8, out: &mut Vec<u8>) {
        let params = std::mem::take(&mut self.csi);
        let private = params.first() == Some(&b'?');
        let nums: Vec<usize> = params
            .split(|b| *b == b';')
            .map(|p| {
                std::str::from_utf8(p)
                    .ok()
                    .map(|s| s.trim_start_matches('?'))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0)
            })
            .collect();
        let n = nums.first().copied().unwrap_or(0);
        let count = n.max(1);
        match f {
            b'h' | b'l' if private && nums.iter().any(|m| matches!(m, 47 | 1047 | 1049)) => {
                let on = f == b'h';
                if on && !self.alt {
                    if !self.line.is_empty() {
                        self.emit(out);
                    }
                    out.extend_from_slice(FULL_SCREEN.as_bytes());
                    out.push(b'\n');
                }
                if !on {
                    self.line.clear();
                    self.col = 0;
                }
                self.alt = on;
            }
            _ if self.alt || private => {}
            b'K' => match n {
                0 => self.line.truncate(self.col),
                1 => {
                    let upto = self.col.saturating_add(1).min(self.line.len());
                    for c in &mut self.line[..upto] {
                        *c = ' ';
                    }
                }
                _ => self.line.clear(),
            },
            b'G' => self.col = count.saturating_sub(1).min(MAX_LINE - 1),
            b'C' => self.col = self.col.saturating_add(count).min(MAX_LINE - 1),
            b'D' => self.col = self.col.saturating_sub(count),
            _ => {}
        }
    }
}

// ── the log ───────────────────────────────────────────────────────────────────────

/// Where a log is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Running,
    /// Ended as asked (stopped, the shell ended, the tab closed): everything is in.
    Ended,
    /// Stopped by itself: the disk did not keep up, or a write failed. Why.
    Failed(String),
}

struct Shared {
    status: Mutex<Status>,
    /// The writer has finished, whatever the status.
    done: Mutex<bool>,
    done_cv: Condvar,
    /// Bytes written into the file so far.
    written: AtomicU64,
    /// The reader gave up waiting for room.
    overflow: AtomicBool,
}

impl Shared {
    fn fail(&self, why: String) {
        if let Ok(mut s) = self.status.lock() {
            if *s == Status::Running {
                *s = Status::Failed(why);
            }
        }
    }
}

/// Set when the writer thread ends, by returning or by panicking.
struct Ended<'a>(&'a Shared);

impl Drop for Ended<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.fail("the log's writer stopped".into());
        }
        if let Ok(mut d) = self.0.done.lock() {
            *d = true;
        }
        self.0.done_cv.notify_all();
    }
}

/// One terminal's log: the queue in, the writer thread, and how it is going.
pub struct Log {
    path: PathBuf,
    mode: Mode,
    tx: Mutex<Option<SyncSender<Vec<u8>>>>,
    shared: Arc<Shared>,
}

/// What a text log says about whose output it is, in its header and footer.
pub struct About {
    /// "sh · tab 2".
    pub label: String,
}

impl Log {
    /// Start logging into `sink` on a writer thread of its own.
    pub fn start(path: PathBuf, mode: Mode, sink: Box<dyn Write + Send>, about: About) -> Arc<Log> {
        Self::start_with(path, mode, sink, about, QUEUE)
    }

    fn start_with(
        path: PathBuf,
        mode: Mode,
        sink: Box<dyn Write + Send>,
        about: About,
        queue: usize,
    ) -> Arc<Log> {
        let (tx, rx) = sync_channel::<Vec<u8>>(queue);
        let shared = Arc::new(Shared {
            status: Mutex::new(Status::Running),
            done: Mutex::new(false),
            done_cv: Condvar::new(),
            written: AtomicU64::new(0),
            overflow: AtomicBool::new(false),
        });
        {
            let shared = shared.clone();
            std::thread::spawn(move || {
                // Whether the loop returns or panics, the log is seen to end: a panic
                // is a failure said as one, and `done` is set, so nothing waits on a
                // writer that is gone.
                let _ended = Ended(&shared);
                write_loop(rx, sink, mode, &shared, &about.label);
            });
        }
        Arc::new(Log {
            path,
            mode,
            tx: Mutex::new(Some(tx)),
            shared,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn status(&self) -> Status {
        self.shared
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| Status::Failed("the log's state was lost".into()))
    }

    /// Bytes in the file from this log so far.
    pub fn written(&self) -> u64 {
        self.shared.written.load(Ordering::Relaxed)
    }

    /// Whether this log still takes output.
    pub fn taking(&self) -> bool {
        self.tx.lock().is_ok_and(|t| t.is_some())
    }

    /// A chunk of the terminal's output, from its reader thread. Never waits on the
    /// disk: a full queue is waited on for at most [`PATIENCE`], then the log is given
    /// up (Failed) rather than left with a hole. False once the log takes no more.
    pub fn tap(&self, chunk: &[u8]) -> bool {
        self.tap_waiting(chunk, PATIENCE)
    }

    fn tap_waiting(&self, chunk: &[u8], patience: Duration) -> bool {
        // A clone, so the lock is not held while waiting for room.
        let Some(tx) = self.tx.lock().ok().and_then(|t| t.clone()) else {
            return false;
        };
        let mut item = chunk.to_vec();
        let deadline = Instant::now() + patience;
        loop {
            match tx.try_send(item) {
                Ok(()) => return true,
                Err(TrySendError::Disconnected(_)) => {
                    // The writer stopped: it said why, unless it could not (then this
                    // says it; a reason already given is kept).
                    self.shared.fail("the log's writer stopped".into());
                    self.drop_sender();
                    return false;
                }
                Err(TrySendError::Full(back)) => {
                    if Instant::now() >= deadline {
                        self.shared.overflow.store(true, Ordering::Relaxed);
                        self.shared.fail(format!(
                            "the disk did not keep up (waited {} ms) \u{2014} later output is not in the log",
                            patience.as_millis()
                        ));
                        self.drop_sender();
                        return false;
                    }
                    item = back;
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    fn drop_sender(&self) {
        if let Ok(mut t) = self.tx.lock() {
            t.take();
        }
    }

    /// No more output: the writer writes what is queued, the footer, and flushes.
    /// Does not wait; see [`Log::wait`].
    pub fn finish(&self) {
        self.drop_sender();
    }

    /// Wait at most `d` for the writer to finish; true when it has.
    pub fn wait(&self, d: Duration) -> bool {
        let deadline = Instant::now() + d;
        let Ok(mut done) = self.shared.done.lock() else {
            return false;
        };
        while !*done {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match self.shared.done_cv.wait_timeout(done, left) {
                Ok((d, _)) => done = d,
                Err(_) => return false,
            }
        }
        true
    }

    /// Finish and wait at most `d`; the status line's words for how it ended.
    pub fn close(&self, d: Duration) -> String {
        self.finish();
        let finished = self.wait(d);
        let size = crate::format::human_size(self.written());
        match self.status() {
            Status::Failed(why) => format!("log stopped: {why} \u{2014} {}", self.path.display()),
            _ if !finished => format!(
                "log may be incomplete: the disk did not answer \u{2014} {}",
                self.path.display()
            ),
            _ => format!("log closed: {} ({size})", self.path.display()),
        }
    }
}

impl Drop for Log {
    fn drop(&mut self) {
        // Whatever drops it last, the writer is told to finish; it does so on its own.
        self.drop_sender();
    }
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// The writer thread: chunks in, the file out, flushed whenever the queue runs dry.
fn write_loop(
    rx: Receiver<Vec<u8>>,
    sink: Box<dyn Write + Send>,
    mode: Mode,
    shared: &Shared,
    label: &str,
) {
    let mut w = Counted {
        inner: BufWriter::with_capacity(64 * 1024, sink),
        shared,
    };
    let mut filter = TextFilter::new();
    let mut out = Vec::with_capacity(16 * 1024);
    let fail = |e: io::Error| shared.fail(format!("cannot write: {e}"));
    if mode == Mode::Text {
        let head = format!("--- xdevcmd: log of {label}, started {} ---\n", stamp());
        if let Err(e) = w.write_all(head.as_bytes()) {
            return fail(e);
        }
    }
    while let Ok(first) = rx.recv() {
        let mut chunk = Some(first);
        while let Some(c) = chunk.take() {
            let r = match mode {
                Mode::Raw => w.write_all(&c),
                Mode::Text => {
                    out.clear();
                    filter.feed(&c, &mut out);
                    w.write_all(&out)
                }
            };
            if let Err(e) = r {
                return fail(e);
            }
            chunk = rx.try_recv().ok();
        }
        if let Err(e) = w.flush() {
            return fail(e);
        }
    }
    if mode == Mode::Text {
        out.clear();
        filter.finish(&mut out);
        if shared.overflow.load(Ordering::Relaxed) {
            out.extend_from_slice(
                b"[xdevcmd: logging stopped here: the disk did not keep up, later output is not in this file]\n",
            );
        }
        out.extend_from_slice(format!("--- xdevcmd: log ended {} ---\n", stamp()).as_bytes());
        if let Err(e) = w.write_all(&out) {
            return fail(e);
        }
    }
    if let Err(e) = w.flush() {
        return fail(e);
    }
    if let Ok(mut s) = shared.status.lock() {
        if *s == Status::Running {
            *s = Status::Ended;
        }
    }
}

/// Counts what reaches the file.
struct Counted<'a> {
    inner: BufWriter<Box<dyn Write + Send>>,
    shared: &'a Shared,
}

impl Write for Counted<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.shared.written.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(chunks: &[&[u8]]) -> String {
        let mut f = TextFilter::new();
        let mut out = Vec::new();
        for c in chunks {
            f.feed(c, &mut out);
        }
        f.finish(&mut out);
        String::from_utf8(out).unwrap()
    }

    /// Every way of cutting `input` in two gives what it gives whole.
    fn same_split_anywhere(input: &[u8]) -> String {
        let whole = text(&[input]);
        for i in 0..=input.len() {
            assert_eq!(text(&[&input[..i], &input[i..]]), whole, "split at {i}");
        }
        // And byte by byte.
        let bytes: Vec<&[u8]> = input.chunks(1).collect();
        assert_eq!(text(&bytes), whole, "one byte at a time");
        whole
    }

    #[test]
    fn colours_titles_and_other_escapes_are_removed() {
        let got = text(&[
            b"\x1b[1;31mred\x1b[0m plain\n",
            b"\x1b]0;a title\x07\x1b]7;file://h/tmp\x1b\\after\n",
            b"\x1bP1$r0m\x1b\\dcs gone\n",
            b"\x1b(Bcharset\x1b=keypad\x07bell\n",
        ]);
        assert_eq!(got, "red plain\nafter\ndcs gone\ncharsetkeypadbell\n");
    }

    #[test]
    fn a_line_a_cr_rewrote_is_kept_once_as_the_terminal_left_it() {
        assert_eq!(text(&[b"10%\r50%\r100%\n"]), "100%\n");
        // Shorter over longer leaves the longer's tail, as on screen.
        assert_eq!(text(&[b"downloading\rdone\n"]), "doneloading\n");
        // ...unless the program erased it, as progress bars do.
        assert_eq!(text(&[b"downloading\r\x1b[Kdone\n"]), "done\n");
        assert_eq!(text(&[b"abc\x1b[2Kx\n"]), "   x\n");
        // CRLF is a plain line end.
        assert_eq!(text(&[b"one\r\ntwo\r\n"]), "one\ntwo\n");
        // Backspace and cursor moves along the line.
        assert_eq!(text(&[b"ab\x08c\n"]), "ac\n");
        assert_eq!(text(&[b"abc\x1b[2Dz\x1b[3Gy\n"]), "azy\n");
        assert_eq!(text(&[b"a\x1b[3Cb\n"]), "a   b\n");
    }

    #[test]
    fn utf8_and_escapes_split_between_reads_are_taken_whole() {
        let s = same_split_anywhere("é日本\u{1f600} ok\n".as_bytes());
        assert_eq!(s, "é日本\u{1f600} ok\n");
        let s = same_split_anywhere(b"\x1b[38;2;255;0;0mred\x1b]0;t\x1b\\\x1b[0m\r\x1b[Kfinal\n");
        assert_eq!(s, "final\n");
    }

    #[test]
    fn invalid_bytes_become_replacement_characters() {
        assert_eq!(text(&[b"a\xffb\n"]), "a\u{fffd}b\n");
        assert_eq!(
            text(&[b"a\xe6\x97b\n"]),
            "a\u{fffd}b\n",
            "a character cut short"
        );
        assert_eq!(text(&[b"end\xe6"]), "end\u{fffd}\n", "cut short at the end");
        // C1 controls written as UTF-8 are controls, not text.
        assert_eq!(text(&["a\u{9b}b\n".as_bytes()]), "ab\n");
    }

    #[test]
    fn a_very_long_line_is_written_in_bounded_pieces() {
        let long = vec![b'y'; MAX_LINE * 2 + 10];
        let got = text(&[&long]);
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.len() <= MAX_LINE));
        assert_eq!(lines.iter().map(|l| l.len()).sum::<usize>(), long.len());
    }

    /// A count no terminal would send must not overflow the column (a panic in a debug
    /// build, a wrap in release): the cursor stops at the line's bound.
    #[test]
    fn a_huge_cursor_count_is_bounded() {
        let max = usize::MAX.to_string();
        let input = format!("ab\x1b[{max}Cz\r\x1b[{max}Dy\x1b[{max}G\x1b[{max}K\n");
        let got = text(&[input.as_bytes()]);
        let lines: Vec<&str> = got.lines().collect();
        // `z` lands on the last column, which fills the line and writes it.
        assert!(
            lines[0].starts_with("ab") && lines[0].ends_with('z'),
            "{}",
            lines[0].len()
        );
        assert_eq!(lines[0].chars().count(), MAX_LINE);
        // Back to 0, `y`, to the last column, the line erased: an empty line.
        assert_eq!(&lines[1..], [""]);
    }

    #[test]
    fn a_full_screen_program_is_one_line() {
        let got = text(&[
            b"before\n\x1b[?1049h\x1b[H\x1b[2Jvim screen\nmore\n",
            b"\x1b[?1049l\x1b[?25hafter\n",
        ]);
        assert_eq!(got, format!("before\n{FULL_SCREEN}\nafter\n"));
    }

    /// Not a check: the text filter's speed on coloured output, against the terminal's
    /// reader (Round 027 measured vt100 at about 3 MB/s). `cargo test --release -p
    /// xdevcmd-core -- --ignored --nocapture filter_throughput`.
    #[test]
    #[ignore]
    fn filter_throughput() {
        let line = b"\x1b[32mok\x1b[0m src/some/path/file.rs:120: a line of build output\r\n";
        let input: Vec<u8> = line.iter().copied().cycle().take(50_000_000).collect();
        let mut f = TextFilter::new();
        let mut out = Vec::with_capacity(16 * 1024);
        let t = Instant::now();
        for chunk in input.chunks(8192) {
            out.clear();
            f.feed(chunk, &mut out);
        }
        let took = t.elapsed();
        eprintln!(
            "filter: 50 MB in {took:?} ({:.0} MB/s)",
            50.0 / took.as_secs_f64()
        );
    }

    #[test]
    fn an_unfinished_line_and_trailing_spaces() {
        assert_eq!(text(&[b"$ "]), "$\n");
        assert_eq!(text(&[b"a   \n"]), "a\n");
        assert_eq!(text(&[b""]), "");
    }

    #[test]
    fn the_file_name_is_safe() {
        let n = file_name("bash -l/../x");
        assert!(n.starts_with("xdevcmd-bash_-l____x-"), "{n}");
        assert!(n.ends_with(".log"));
        assert!(!n.contains('/'));
        assert!(file_name("").starts_with("xdevcmd-shell-"));
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-termlog-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn open_creates_owner_only_and_appends() {
        let d = scratch("open");
        let p = d.join("a.log");
        let (mut f, existed) = open(&p).unwrap();
        assert!(!existed);
        f.write_all(b"one\n").unwrap();
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let (mut f, existed) = open(&p).unwrap();
        assert!(existed);
        assert!(!readable_by_others(&f));
        f.write_all(b"two\n").unwrap();
        drop(f);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "one\ntwo\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Run `f` on another thread and require an answer within two seconds: a FIFO
    /// opened for writing without `O_NONBLOCK` blocks until someone reads it.
    #[cfg(unix)]
    fn within_2s<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(Duration::from_secs(2)).ok()
    }

    /// The open itself refuses what could be swapped in after any look at the name: a
    /// link is not followed and a FIFO does not block (nobody reading it), and neither
    /// is opened as a log.
    #[cfg(unix)]
    #[test]
    fn the_open_refuses_a_fifo_and_a_link_without_hanging() {
        let d = scratch("race");
        let fifo = d.join("fifo");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status();
        assert!(made.is_ok_and(|s| s.success()), "mkfifo");
        let p = fifo.clone();
        let got = within_2s(move || open_append(&p).map(|_| ()));
        if got.is_none() {
            // Unblock the stuck open before failing, so the test run ends.
            let _ = std::fs::File::open(&fifo);
            panic!("opening a FIFO blocked");
        }
        let e = got.unwrap().unwrap_err();
        assert!(e.to_string().contains("not a regular file"), "{e}");

        std::fs::write(d.join("target"), "x").unwrap();
        std::os::unix::fs::symlink(d.join("target"), d.join("link")).unwrap();
        let e = open_append(&d.join("link")).unwrap_err();
        assert!(e.to_string().contains("a link"), "{e}");
        assert_eq!(std::fs::read_to_string(d.join("target")).unwrap(), "x");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Appending to a file another user owns is refused. Making such a file needs root
    /// (this container runs as root; GitHub's runners do not); as anyone else it says so and
    /// checks only that one's own file is accepted.
    #[cfg(unix)]
    #[test]
    fn appending_to_another_users_file_is_refused() {
        let d = scratch("owner");
        let p = d.join("theirs.log");
        std::fs::write(&p, "theirs\n").unwrap();
        if rustix::process::geteuid().is_root() {
            let nobody = rustix::process::Uid::from_raw(65534);
            rustix::fs::chown(&p, Some(nobody), None).unwrap();
            let e = open(&p).unwrap_err();
            assert!(e.to_string().contains("another user"), "{e}");
            assert_eq!(std::fs::read_to_string(&p).unwrap(), "theirs\n");
        } else {
            eprintln!("not root: another user's file cannot be made here");
            assert!(open(&p).is_ok());
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn open_refuses_a_link_and_a_directory_and_warns_of_a_shared_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let d = scratch("refuse");
        std::fs::write(d.join("target"), "x").unwrap();
        std::os::unix::fs::symlink(d.join("target"), d.join("link")).unwrap();
        assert!(open(&d.join("link")).is_err(), "a link is not followed");
        assert_eq!(std::fs::read_to_string(d.join("target")).unwrap(), "x");
        assert!(open(&d).is_err(), "a directory");
        std::fs::set_permissions(d.join("target"), std::fs::Permissions::from_mode(0o644)).unwrap();
        let (f, existed) = open(&d.join("target")).unwrap();
        assert!(existed && readable_by_others(&f));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A sink shared with the test, to read what reached it.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn about() -> About {
        About {
            label: "sh \u{b7} tab 1".into(),
        }
    }

    #[test]
    fn a_text_log_has_its_lines_between_a_header_and_a_footer() {
        let sink = Sink::default();
        let log = Log::start("x.log".into(), Mode::Text, Box::new(sink.clone()), about());
        assert!(log.tap(b"\x1b[32mgreen\x1b[0m\n50%\r"));
        assert!(log.tap(b"100%\nno end"));
        assert_eq!(log.close(Duration::from_secs(5)), {
            let n = sink.0.lock().unwrap().len() as u64;
            format!("log closed: x.log ({})", crate::format::human_size(n))
        });
        assert_eq!(log.status(), Status::Ended);
        let got = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = got.lines().collect();
        assert!(lines[0].starts_with("--- xdevcmd: log of sh \u{b7} tab 1, started "));
        assert_eq!(&lines[1..4], ["green", "100%", "no end"]);
        assert!(lines[4].starts_with("--- xdevcmd: log ended "));
        assert_eq!(log.written(), got.len() as u64);
        assert!(!log.tap(b"late"), "a finished log takes nothing");
    }

    #[test]
    fn a_raw_log_is_the_bytes_and_nothing_else() {
        let sink = Sink::default();
        let log = Log::start("x.log".into(), Mode::Raw, Box::new(sink.clone()), about());
        let bytes = b"\x1b[31mred\r\n\xe6\x97";
        assert!(log.tap(bytes));
        assert!(log.tap(b"\xa5"));
        log.close(Duration::from_secs(5));
        let mut want = bytes.to_vec();
        want.push(0xa5);
        assert_eq!(*sink.0.lock().unwrap(), want);
    }

    /// A disk that fills up after a few bytes.
    struct Full(usize);
    impl Write for Full {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "No space left on device",
                ));
            }
            let n = b.len().min(self.0);
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_failing_write_stops_the_log_and_says_why() {
        let log = Log::start("x.log".into(), Mode::Raw, Box::new(Full(10)), about());
        // Past the buffer, so the write reaches the sink.
        let big = vec![b'x'; 128 * 1024];
        log.tap(&big);
        assert!(log.wait(Duration::from_secs(5)), "the writer ends");
        match log.status() {
            Status::Failed(why) => assert!(why.contains("No space left"), "{why}"),
            s => panic!("{s:?}"),
        }
        assert!(!log.tap(b"more"), "the reader is told");
        assert!(!log.taking());
        assert!(log
            .close(Duration::from_secs(1))
            .starts_with("log stopped: cannot write"));
    }

    /// A sink whose write panics, as a bug in the writer's path would.
    struct Panics;
    impl Write for Panics {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("a bug in the writer");
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A writer thread that panics is a failed log, said, not one left "running" with
    /// nobody writing it: `done` is set, so closing does not wait out its bound.
    #[test]
    fn a_writer_that_panics_is_a_failed_log() {
        let log = Log::start("x.log".into(), Mode::Raw, Box::new(Panics), about());
        log.tap(&vec![b'x'; 128 * 1024]);
        assert!(
            log.wait(Duration::from_secs(5)),
            "the writer is seen to end"
        );
        assert_eq!(
            log.status(),
            Status::Failed("the log's writer stopped".into())
        );
        assert!(!log.tap(b"more"));
        let t = Instant::now();
        let said = log.close(Duration::from_secs(2));
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "no wait for a dead writer"
        );
        assert!(
            said.starts_with("log stopped: the log's writer stopped"),
            "{said}"
        );
    }

    /// A disk that does not answer until let go.
    struct Stuck(Arc<(Mutex<bool>, Condvar)>);
    impl Write for Stuck {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            let (m, cv) = &*self.0;
            let mut go = m.lock().unwrap();
            while !*go {
                go = cv.wait(go).unwrap();
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_disk_that_does_not_keep_up_stops_the_log_without_holding_the_reader() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let log = Log::start_with(
            "x.log".into(),
            Mode::Text,
            Box::new(Stuck(gate.clone())),
            about(),
            2,
        );
        let patience = Duration::from_millis(50);
        let t0 = Instant::now();
        let mut taken = 0;
        while log.tap_waiting(b"line\n", patience) {
            taken += 1;
            assert!(taken < 100, "the queue is bounded");
        }
        let took = t0.elapsed();
        assert!(took < Duration::from_secs(2), "the reader waited {took:?}");
        match log.status() {
            Status::Failed(why) => assert!(why.contains("did not keep up"), "{why}"),
            s => panic!("{s:?}"),
        }
        // Closing does not hang on a disk that does not answer.
        let said = log.close(Duration::from_millis(50));
        assert!(
            said.starts_with("log stopped: the disk did not keep up"),
            "{said}"
        );
        // Once the disk answers, the writer writes the note and ends.
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        assert!(log.wait(Duration::from_secs(5)));
    }
}
