//! Colouring a line anywhere in a big file without lexing the file from its start on the
//! UI thread.
//!
//! A line's colours depend on the state at its start, which depends on every line before
//! it. So a background [`Scan`] goes through the file once, lexing for the state only,
//! and keeps the state at every [`EVERY`]th line (more often where lines are long) in
//! [`Checkpoints`]. Drawing line `i` then lexes from the checkpoint before it (or from a
//! line drawn recently): at most `EVERY - 1` lines and half of [`SYNC_BYTES`]. A jump
//! past what the scan has reached lexes at most [`SYNC_LINES`] and [`SYNC_BYTES`] on
//! the spot, else the line is drawn plain until the scan gets there.
//!
//! `Scan` is fed chunks of bytes, so whatever reads the file can feed it: today its own
//! thread; when the viewer's line index is built in the background (pillar 2, A03), the
//! same pass that finds line starts can feed it, and the file is read once.
//!
//! The scan cuts lines exactly as [`Document::line`] does (the same function), so a line
//! reached from a checkpoint gets the colours it gets when scrolled to.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{lex_line, Lang, Span, State};
use crate::document::{line_text, Document, MAX_LINE};

/// A checkpoint every this many lines.
pub const EVERY: usize = 128;
/// A jump past the scan lexes at most this many lines where it lands, else waits.
pub const SYNC_LINES: usize = 2048;
/// … and at most this many bytes of them: 2,048 lines of 32 KiB are 64 MiB, most of a
/// second on the UI thread (Round 028's review).
pub const SYNC_BYTES: u64 = 1 << 20;
/// Lines longer than this (in bytes) are drawn plain: lexed only for the state after them.
pub const LONG_LINE: usize = 4096;
/// States of lines drawn recently, kept to scroll on from; cleared past this many.
const CACHE: usize = 16_384;

/// The state at the start of some lines, in order: every [`EVERY`]th line, and more
/// often where lines are long (at least every half of [`SYNC_BYTES`]), so a line is
/// never more than that many bytes of lexing from one.
#[derive(Debug)]
pub struct Checkpoints {
    states: Vec<(usize, State)>,
    done: bool,
    /// The file changed while it was scanned: the checkpoints were dropped.
    dropped: bool,
}

impl Default for Checkpoints {
    fn default() -> Self {
        Self {
            states: vec![(0, State::Normal)],
            done: false,
            dropped: false,
        }
    }
}

impl Checkpoints {
    /// The last checkpoint at or before `line`: its line and state.
    pub fn before(&self, line: usize) -> (usize, State) {
        let k = self.states.partition_point(|(l, _)| *l <= line);
        self.states
            .get(k.saturating_sub(1))
            .copied()
            .unwrap_or((0, State::Normal))
    }

    /// The state at the start of `line`, if there is a checkpoint there.
    pub fn at(&self, line: usize) -> Option<State> {
        let (l, st) = self.before(line);
        (l == line).then_some(st)
    }

    /// How many checkpoints there are (line 0's always).
    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// The file changed while it was scanned, so its checkpoints were dropped.
    pub fn was_dropped(&self) -> bool {
        self.dropped
    }

    /// The scan has ended: at the file's end, cancelled, or on an error.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

/// One pass over a file's bytes, filling a [`Checkpoints`].
pub struct Scan {
    lang: Lang,
    state: State,
    line: usize,
    lines: usize,
    buf: Vec<u8>,
    found: Vec<(usize, State)>,
    /// Bytes lexed since the last checkpoint.
    since: usize,
    store: Arc<Mutex<Checkpoints>>,
}

impl Scan {
    /// A scan of the first `lines` lines (the document's count: a file indexed only in
    /// part is scanned as far as it is shown).
    pub fn new(lang: Lang, lines: usize, store: Arc<Mutex<Checkpoints>>) -> Self {
        Self {
            lang,
            state: State::Normal,
            line: 0,
            lines,
            buf: Vec::new(),
            found: Vec::new(),
            since: 0,
            store,
        }
    }

    /// The next bytes of the file. False once every line has been lexed.
    pub fn feed(&mut self, mut chunk: &[u8]) -> bool {
        while !chunk.is_empty() && self.line < self.lines {
            match chunk.iter().position(|&c| c == b'\n') {
                Some(p) => {
                    self.take(&chunk[..=p]);
                    self.end_line();
                    chunk = &chunk[p + 1..];
                }
                None => {
                    self.take(chunk);
                    chunk = &[];
                }
            }
        }
        self.publish();
        self.line < self.lines
    }

    /// The file has ended (or the scan stops): the last line, if it had no newline.
    pub fn finish(mut self) {
        if !self.buf.is_empty() && self.line < self.lines {
            self.end_line();
        }
        self.publish();
        if let Ok(mut s) = self.store.lock() {
            s.done = true;
        }
    }

    /// A line's first bytes, as many as `Document::line` reads of it.
    fn take(&mut self, bytes: &[u8]) {
        let room = (MAX_LINE as usize).saturating_sub(self.buf.len());
        self.buf.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    fn end_line(&mut self) {
        self.since += self.buf.len();
        let text = line_text(std::mem::take(&mut self.buf));
        self.state = lex_line(self.lang, self.state, &text, None);
        self.line += 1;
        if self.line.is_multiple_of(EVERY) || self.since as u64 >= SYNC_BYTES / 2 {
            self.found.push((self.line, self.state));
            self.since = 0;
        }
    }

    fn publish(&mut self) {
        if self.found.is_empty() {
            return;
        }
        if let Ok(mut s) = self.store.lock() {
            s.states.append(&mut self.found);
        }
    }
}

/// Marks the store done if the scanning thread ends any other way (a panic).
struct DoneOnDrop(Arc<Mutex<Checkpoints>>);

impl Drop for DoneOnDrop {
    fn drop(&mut self) {
        if let Ok(mut s) = self.0.lock() {
            s.done = true;
        }
    }
}

/// A document's colours, line by line.
pub struct Highlighter {
    lang: Lang,
    store: Arc<Mutex<Checkpoints>>,
    cancel: Arc<AtomicBool>,
    /// The state at the start of lines lexed recently.
    cache: BTreeMap<usize, State>,
    /// The scan's checkpoints were dropped, and the cache with them.
    dropped_seen: bool,
    /// Lines lexed, for tests that count them.
    #[cfg(test)]
    lexed: usize,
}

impl Highlighter {
    /// Colours for `lang`, with no scan: lines within [`SYNC_LINES`] of the top only.
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            store: Arc::new(Mutex::new(Checkpoints {
                states: vec![(0, State::Normal)],
                done: true,
                dropped: false,
            })),
            cancel: Arc::new(AtomicBool::new(false)),
            cache: BTreeMap::new(),
            dropped_seen: false,
            #[cfg(test)]
            lexed: 0,
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// The checkpoints, shared with the scan.
    pub fn checkpoints(&self) -> Arc<Mutex<Checkpoints>> {
        Arc::clone(&self.store)
    }

    /// Scan the file at `path` in the background: its own handle (the document seeks
    /// its own), 1 MiB at a time, the first `lines` lines. Stops when this highlighter
    /// is dropped.
    pub fn scan_file(&mut self, path: &Path, lines: usize) {
        // Not there any more: nothing to scan, and lines past the top stay plain.
        if let Ok(f) = std::fs::File::open(path) {
            let path = path.to_path_buf();
            let before = stamp(&path);
            self.scan_checked(f, lines, Box::new(move || stamp(&path) == before));
        }
    }

    /// Scan what `reader` gives, in the background.
    pub fn scan_from(&mut self, reader: impl Read + Send + 'static, lines: usize) {
        self.scan_checked(reader, lines, Box::new(|| true));
    }

    /// [`Highlighter::scan_from`], and at the end `unchanged` says whether what was
    /// read is still what is there; if not, the checkpoints are dropped (a file
    /// rewritten while scanned is not coloured from states of its old contents).
    fn scan_checked(
        &mut self,
        mut reader: impl Read + Send + 'static,
        lines: usize,
        unchanged: Box<dyn Fn() -> bool + Send>,
    ) {
        let store = Arc::new(Mutex::new(Checkpoints::default()));
        self.store = Arc::clone(&store);
        self.cache.clear();
        let cancel = Arc::clone(&self.cancel);
        let lang = self.lang;
        let spawned = std::thread::Builder::new()
            .name("syntax".into())
            .spawn(move || {
                let _done = DoneOnDrop(Arc::clone(&store));
                let mut scan = Scan::new(lang, lines, Arc::clone(&store));
                let mut buf = vec![0u8; 1 << 20];
                while !cancel.load(Ordering::Relaxed) {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if !scan.feed(&buf[..n]) {
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
                scan.finish();
                if !unchanged() {
                    if let Ok(mut s) = store.lock() {
                        s.states.truncate(1);
                        s.dropped = true;
                    }
                }
            });
        if spawned.is_err() {
            if let Ok(mut s) = self.store.lock() {
                s.done = true;
            }
        }
    }

    /// How far the scan is: checkpoints so far, and whether it has ended.
    pub fn progress(&self) -> (usize, bool) {
        self.store
            .lock()
            .map(|s| (s.len(), s.done))
            .unwrap_or((1, true))
    }

    /// The coloured pieces of line `i` of `doc`, or `None` when it cannot be known
    /// cheaply yet (the scan has not reached it): drawn plain until it has. A line over
    /// [`LONG_LINE`] bytes is `Some` and empty.
    pub fn spans(&mut self, doc: &mut Document, i: usize) -> Option<Vec<Span>> {
        let text = doc.line(i).ok()?;
        self.spans_for(doc, i, &text)
    }

    /// [`Highlighter::spans`] when the caller has read line `i` already (`text`, as
    /// `Document::line` gave it).
    pub fn spans_for(&mut self, doc: &mut Document, i: usize, text: &str) -> Option<Vec<Span>> {
        // A long line is drawn plain: once the state after it is known, there is
        // nothing to lex it for again (each frame did, Round 028's review).
        if text.len() > LONG_LINE && self.cache.contains_key(&(i + 1)) {
            return Some(Vec::new());
        }
        let (mut at, mut state) = {
            let cached = self.cache.range(..=i).next_back().map(|(&l, &s)| (l, s));
            let checkpoint = self.store.lock().ok().map(|s| {
                if s.dropped && !self.dropped_seen {
                    // States lexed from the dropped checkpoints go with them.
                    self.dropped_seen = true;
                    self.cache.clear();
                }
                s.before(i)
            });
            match (cached, checkpoint) {
                (Some(c), Some(k)) => {
                    if c.0 >= k.0 {
                        c
                    } else {
                        k
                    }
                }
                (Some(c), None) => c,
                (None, Some(k)) => k,
                (None, None) => (0, State::Normal),
            }
        };
        if i - at > SYNC_LINES {
            return None;
        }
        let bytes = doc
            .line_start(i)
            .zip(doc.line_start(at))
            .map_or(0, |(to, from)| to.saturating_sub(from));
        if bytes > SYNC_BYTES {
            return None;
        }
        while at < i {
            let line = doc.line(at).ok()?;
            state = self.lex(state, &line, None);
            at += 1;
            self.remember(at, state);
        }
        let mut spans = Vec::new();
        let next = if text.len() > LONG_LINE {
            self.lex(state, text, None)
        } else {
            self.lex(state, text, Some(&mut spans))
        };
        self.remember(i + 1, next);
        Some(spans)
    }

    fn lex(&mut self, state: State, text: &str, spans: Option<&mut Vec<Span>>) -> State {
        #[cfg(test)]
        {
            self.lexed += 1;
        }
        lex_line(self.lang, state, text, spans)
    }

    fn remember(&mut self, line: usize, state: State) {
        if self.cache.len() >= CACHE {
            self.cache.clear();
        }
        self.cache.insert(line, state);
    }
}

/// What tells a file has changed: its length and when it was last written.
fn stamp(path: &Path) -> Option<(u64, Option<std::time::SystemTime>)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()))
}

impl Drop for Highlighter {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xdevcmd-syntax-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn wait(h: &Highlighter) {
        let t = Instant::now();
        while !h.progress().1 {
            assert!(
                t.elapsed() < Duration::from_secs(20),
                "the scan never ended"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Rust with everything that carries a state over lines, across checkpoints: block
    /// comments, strings, raw strings; and lines that test the cut: CRLF, invalid UTF-8,
    /// a line past 64 KiB with `/*` after the cut (seen by neither side), a long line
    /// with `/*` before it (plain, but the next line is in a comment).
    fn awkward_rust() -> Vec<u8> {
        let mut f = Vec::new();
        for i in 0..5000usize {
            match i % 397 {
                120 => f.extend_from_slice(b"let a = 1; /* opened\n"),
                135 => f.extend_from_slice(b"closed */ fn after() {}\n"),
                200 => f.extend_from_slice(b"let s = \"spans\r\n"),
                260 => f.extend_from_slice(b"lines\"; // end\r\n"),
                300 => f.extend_from_slice(b"let r = r#\"raw\n"),
                330 => f.extend_from_slice(b"\"# ;\n"),
                340 => {
                    f.extend_from_slice(b"let x = 1; ");
                    f.extend(std::iter::repeat_n(b'x', 70 * 1024));
                    f.extend_from_slice(b"/* never seen\n");
                }
                350 => {
                    f.extend_from_slice(b"/* long ");
                    f.extend(std::iter::repeat_n(b'y', 5000));
                    f.push(b'\n');
                }
                351 => f.extend_from_slice(b"*/ fn g() {}\n"),
                360 => f.extend_from_slice(b"let bad = \"\xff\xfe\xc3\"; /* \xe2\x82\n"),
                361 => f.extend_from_slice(b"*/\n"),
                _ => {
                    f.extend_from_slice(format!("fn f{i}(x: u8) -> u8 {{ x + {i} }}\n").as_bytes())
                }
            }
        }
        f
    }

    #[test]
    fn the_same_colours_by_scrolling_and_by_jumping() {
        let path = temp("jump.rs", &awkward_rust());
        let mut doc = Document::open(&path).unwrap();
        let total = doc.line_count();
        assert_eq!(total, 5000);

        // Scrolled: every line in order, no scan, each from the one before.
        let mut scrolled = Highlighter::new(Lang::Rust);
        let mut by_scroll = Vec::new();
        for i in 0..total {
            by_scroll.push(
                scrolled
                    .spans(&mut doc, i)
                    .expect("one line on from the last"),
            );
        }

        // Jumped: the scan's checkpoints, lines visited out of order.
        let mut jumped = Highlighter::new(Lang::Rust);
        jumped.scan_file(&path, total);
        wait(&jumped);
        assert!(jumped.progress().0 >= total.div_ceil(EVERY));
        let mut order: Vec<usize> = (0..total).rev().step_by(7).collect();
        order.extend([
            121,
            128,
            129,
            136,
            300 + 397,
            341 + 397 * 3,
            351,
            352,
            361,
            362,
            4999,
        ]);
        for i in order {
            let got = jumped.spans(&mut doc, i).expect("a checkpoint is near");
            assert_eq!(got, by_scroll[i], "line {}", i + 1);
        }
        // Spot checks that the states did carry: inside the comment, after the long line.
        let kinds = |i: usize| by_scroll[i].iter().map(|s| s.kind).collect::<Vec<_>>();
        assert_eq!(kinds(127), vec![super::super::Kind::Comment]);
        assert_eq!(
            kinds(351)[0],
            super::super::Kind::Comment,
            "the long line opened it"
        );
        assert!(by_scroll[350].is_empty(), "a long line is plain");
    }

    #[test]
    fn a_checkpoint_carries_a_comment_across_it() {
        let mut f = String::new();
        for i in 0..300 {
            f.push_str(match i {
                126 => "/* starts before the checkpoint\n",
                130 => "ends after it */\n",
                _ => "int x;\n",
            });
        }
        let store = Arc::new(Mutex::new(Checkpoints::default()));
        let mut scan = Scan::new(Lang::C, 300, Arc::clone(&store));
        // Fed in awkward pieces: a line split across chunks counts once.
        for piece in f.as_bytes().chunks(5) {
            scan.feed(piece);
        }
        scan.finish();
        let s = store.lock().unwrap();
        assert!(s.is_done());
        assert_eq!(s.len(), 3, "lines 0, 128, 256");
        assert_eq!(s.at(128), Some(State::Comment(1)));
        assert_eq!(s.at(256), Some(State::Normal));
        assert_eq!(s.before(200), (128, State::Comment(1)));
    }

    #[test]
    fn a_far_jump_waits_for_the_scan_and_a_near_one_does_not() {
        let mut f = String::new();
        for _ in 0..10_000 {
            f.push_str("x = 1 # c\n");
        }
        let path = temp("far.py", f.as_bytes());
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Python);
        assert!(
            h.spans(&mut doc, 9_000).is_none(),
            "far past line 0, no scan"
        );
        assert!(
            h.spans(&mut doc, SYNC_LINES).is_some(),
            "near enough to lex"
        );
        // Lexed once, it is a place to go on from.
        assert!(h.spans(&mut doc, SYNC_LINES * 2).is_some());
        h.scan_file(&path, doc.line_count());
        wait(&h);
        assert!(h.spans(&mut doc, 9_000).is_some());
    }

    /// Few lines but many bytes: a jump does not lex them on the spot (plain), and is
    /// coloured once the scan has been through.
    #[test]
    fn a_far_jump_in_long_lines_waits_for_the_scan() {
        let unit = "let a = \"str\"; /* c */ f(1, 2.5); ";
        let long = unit.repeat(32 * 1024 / unit.len());
        let mut f = String::new();
        for _ in 0..200 {
            f.push_str(&long);
            f.push('\n');
        }
        f.push_str("fn tail() {}\n");
        let path = temp("long-jump.rs", f.as_bytes());
        let mut doc = Document::open(&path).unwrap();
        // 6 MiB: past what open indexes itself (A03).
        doc.wait_indexed();
        let mut h = Highlighter::new(Lang::Rust);
        let t = Instant::now();
        assert_eq!(
            h.spans(&mut doc, 200),
            None,
            "6 MiB before it: plain for now"
        );
        assert!(
            t.elapsed() < Duration::from_millis(100),
            "{:?}",
            t.elapsed()
        );
        h.scan_file(&path, doc.line_count());
        wait(&h);
        let got = h.spans(&mut doc, 200).expect("from a checkpoint");
        assert_eq!(got.len(), 2, "fn and tail: {got:?}");
    }

    /// A screen of long lines, drawn again: nothing is lexed the second time.
    #[test]
    fn long_lines_are_not_lexed_again_on_the_next_frame() {
        let path = long_lines("frames.rs", 50, 8 * 1024);
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Rust);
        for i in 0..50 {
            assert_eq!(h.spans(&mut doc, i), Some(Vec::new()), "plain");
        }
        assert_eq!(h.lexed, 50);
        for i in (0..50).rev() {
            h.spans(&mut doc, i);
        }
        assert_eq!(h.lexed, 50, "the second frame lexed nothing");
    }

    /// A file rewritten while it was scanned: its checkpoints are dropped at the end,
    /// and lines far from the top are plain rather than coloured from the old text.
    #[test]
    fn a_file_changed_while_scanned_loses_its_checkpoints() {
        let mut f = String::new();
        for _ in 0..3000 {
            f.push_str("int x; /* c */\n");
        }
        let path = temp("changed.c", f.as_bytes());
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::C);
        h.scan_checked(
            std::fs::File::open(&path).unwrap(),
            doc.line_count(),
            Box::new(|| false),
        );
        wait(&h);
        let store = h.checkpoints();
        assert!(store.lock().unwrap().was_dropped());
        assert_eq!(h.progress().0, 1, "line 0's only");
        assert_eq!(h.spans(&mut doc, 2900), None);
        // The real check: a different length or time is a change.
        let before = stamp(&path);
        std::fs::write(&path, "int y;\n").unwrap();
        assert_ne!(stamp(&path), before);
        // Unchanged, they are kept.
        let mut h = Highlighter::new(Lang::C);
        h.scan_file(&path, 1);
        wait(&h);
        assert!(!h.checkpoints().lock().unwrap().was_dropped());
    }

    #[test]
    fn empty_and_tiny_documents() {
        let path = temp("empty.rs", b"");
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Rust);
        h.scan_file(&path, doc.line_count());
        wait(&h);
        assert_eq!(h.spans(&mut doc, 0), Some(Vec::new()));
        let path = temp("one.rs", b"fn");
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Rust);
        h.scan_file(&path, doc.line_count());
        wait(&h);
        assert_eq!(h.spans(&mut doc, 0).unwrap().len(), 1);
        assert_eq!(
            h.spans(&mut doc, 5),
            Some(Vec::new()),
            "past the end: nothing"
        );
    }

    #[test]
    fn dropping_the_highlighter_stops_its_scan() {
        struct Endless;
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                for (i, b) in buf.iter_mut().enumerate() {
                    *b = if i % 8 == 7 { b'\n' } else { b'a' };
                }
                Ok(buf.len())
            }
        }
        let mut h = Highlighter::new(Lang::Rust);
        h.scan_from(Endless, usize::MAX);
        let store = h.checkpoints();
        std::thread::sleep(Duration::from_millis(20));
        drop(h);
        let t = Instant::now();
        while !store.lock().unwrap().is_done() {
            assert!(t.elapsed() < Duration::from_secs(10), "still scanning");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_missing_file_scans_nothing_and_says_it_is_done() {
        let mut h = Highlighter::new(Lang::Rust);
        h.scan_file(Path::new("/no/such/file/here.rs"), 10);
        assert_eq!(h.progress(), (1, true));
    }

    /// A file of `n` lines of about `len` bytes each, Rust-ish.
    fn long_lines(name: &str, n: usize, len: usize) -> std::path::PathBuf {
        let unit = "let a = \"str\"; /* c */ f(1, 2.5); ";
        let line = unit.repeat(len / unit.len() + 1);
        let mut f = String::with_capacity(n * (len + 1));
        for _ in 0..n {
            f.push_str(&line[..len]);
            f.push('\n');
        }
        temp(name, f.as_bytes())
    }

    /// Measurements for Round 028's review (release: `cargo test --release -p
    /// xdevcmd-core measure_ -- --ignored --nocapture`).
    #[test]
    #[ignore = "a measurement; run by hand"]
    fn measure_frames_of_long_lines_and_a_far_jump() {
        let path = long_lines("frame.rs", 50, 32 * 1024);
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Rust);
        for frame in 1..=3 {
            let t = Instant::now();
            for i in 0..50 {
                h.spans(&mut doc, i);
            }
            eprintln!("frame {frame} of 50 lines of 32 KiB: {:?}", t.elapsed());
        }
        let path = long_lines("jump.rs", 2048, 32 * 1024);
        let mut doc = Document::open(&path).unwrap();
        let mut h = Highlighter::new(Lang::Rust);
        let t = Instant::now();
        let got = h.spans(&mut doc, 2047);
        eprintln!(
            "jump to the end of 64 MiB in 32 KiB lines, no scan yet: {:?} ({})",
            t.elapsed(),
            if got.is_some() { "lexed" } else { "plain" }
        );
    }
}
