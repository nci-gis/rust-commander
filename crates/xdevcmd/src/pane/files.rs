use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;
use ratatui::Frame;

use xdevcmd_core::archive::{self, ItemKind, Listing};
use xdevcmd_core::find::{self, Query};
use xdevcmd_core::format::{
    elide_middle, elided, fit_end, fit_start, format_time, human_size, text_cells,
};
use xdevcmd_core::glob::{has_wildcards, Pattern};
use xdevcmd_core::size::{Sizer, Total};
use xdevcmd_core::{entry_at, read_dir, sort_entries, Entry, Kind, Sort, SortKey};

use crate::theme;

pub struct FilePane {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub cursor: usize,
    pub offset: usize,
    pub marked: HashSet<PathBuf>,
    pub sort: Sort,
    pub show_hidden: bool,
    /// Set when the directory could not be read at all. The pane keeps its previous
    /// contents on screen rather than going blank, so you can still navigate out.
    pub error: Option<String>,

    // Geometry recorded during render, used to turn a click into a row.
    pub rows_rect: Rect,
    /// The scrollbar as last drawn; `None` when everything fits.
    pub bar: Option<crate::scrollbar::Bar>,
    pub header_cells: Vec<(SortKey, Rect)>,
    /// Each directory named in the title's path, where it was drawn: a click there
    /// goes to it.
    pub crumbs: Vec<(Rect, PathBuf)>,
    /// Something the pane refused to do and the user should hear about, collected by
    /// the app into the status line on the next draw.
    pub notice: Option<String>,

    /// Everything the directory holds; `entries` is this seen through `filter`.
    all: Vec<Entry>,
    /// The quick filter in force, as typed. Cleared on leaving the directory.
    pub filter: Option<String>,
    /// Directories this tab was in, for back and forward. Most recent last.
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    /// Directory sizes worked out so far, and the walks still running.
    sizes: HashMap<PathBuf, Total>,
    sizing: HashSet<PathBuf>,
    sizer: Sizer,
    /// When the directory was last looked at for changes, and its mtime then.
    checked: (Instant, Option<SystemTime>),
    /// Set when this tab shows search results rather than a directory.
    pub results: Option<Results>,
    /// Set while browsing inside an archive. `cwd` stays the directory holding it.
    pub archive: Option<ArchiveView>,
    /// A big archive being listed on another thread: Esc gives up on it.
    opening: Option<Opening>,
    /// `[files] git_status`: the letters for this directory, and the run in flight.
    git: Option<crate::git::Marks>,
    git_rx: Option<std::sync::mpsc::Receiver<crate::git::Marks>>,
}

struct Opening {
    file: PathBuf,
    rx: std::sync::mpsc::Receiver<std::io::Result<Listing>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Opening {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Below this an archive is listed at once: a few milliseconds, and the next key lands
/// inside it. Above it, listing runs on a thread — a 300 MB `.tar.gz` took 1.1 s,
/// all of it with the UI frozen, before this.
const LIST_AT_ONCE: u64 = 8 << 20;

pub struct ArchiveView {
    pub file: PathBuf,
    /// The directory inside the archive, `/`-separated, "" at the top.
    pub prefix: String,
    listing: Listing,
}

impl ArchiveView {
    /// The names inside the archive that `paths` (rows of this pane) stand for.
    /// Everything in the archive, as listed when it was opened.
    pub fn listing(&self) -> &Listing {
        &self.listing
    }

    pub fn inner(&self, paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .filter_map(|p| p.strip_prefix(&self.file).ok())
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .filter(|r| !r.is_empty())
            .collect()
    }
}

/// A tab of Find results (Alt+F7): every operation works on them as on a listing,
/// since each row carries its full path.
pub struct Results {
    pub what: String,
    search: Option<find::Search>,
    pub scanned: u64,
    pub unreadable: u64,
}

impl Results {
    pub fn running(&self) -> bool {
        self.search.is_some()
    }
}

const HISTORY: usize = 64;
/// Up to this many entries a change check re-reads the whole listing, which catches a
/// file growing as well as one appearing. Beyond it only the directory's own mtime is
/// compared, so a huge directory costs one stat per check.
const FULL_CHECK_LIMIT: usize = 2000;

fn dir_mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// `[files] filter`: "fuzzy" (the default) or "plain". Set at start and when Settings
/// saves; read by every pane's filter.
static FUZZY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_fuzzy(on: bool) {
    FUZZY.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn fuzzy() -> bool {
    FUZZY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether, and how well, `name` passes the quick filter `text`: a pattern when it has
/// wildcards; otherwise its letters in order (fuzzy, scored for ranking), or anywhere in
/// the name as typed (plain). Case is ignored, except in fuzzy with a capital in `text`.
fn filter_score(text: &str, name: &str, is_dir: bool) -> Option<i32> {
    if has_wildcards(text) {
        Pattern::new(text).matches(name, is_dir).then_some(0)
    } else if fuzzy() {
        xdevcmd_core::fuzzy::score(text, name)
    } else {
        name.to_lowercase()
            .contains(&text.to_lowercase())
            .then_some(0)
    }
}

impl FilePane {
    pub fn new(cwd: &Path, sort: Sort, show_hidden: bool) -> Self {
        let mut p = Self {
            // A session saved by an older build may hold `\\?\D:\…`.
            cwd: xdevcmd_core::paths::plain(cwd),
            entries: Vec::new(),
            cursor: 0,
            offset: 0,
            marked: HashSet::new(),
            sort,
            show_hidden,
            error: None,
            rows_rect: Rect::ZERO,
            bar: None,
            header_cells: Vec::new(),
            crumbs: Vec::new(),
            notice: None,
            all: Vec::new(),
            filter: None,
            back: Vec::new(),
            forward: Vec::new(),
            sizes: HashMap::new(),
            sizing: HashSet::new(),
            sizer: Sizer::default(),
            checked: (Instant::now(), None),
            results: None,
            archive: None,
            opening: None,
            git: None,
            git_rx: None,
        };
        p.reload();
        p
    }

    /// A tab that fills with what `q` finds, as it finds it.
    pub fn results(q: Query, what: String, sort: Sort) -> Self {
        let mut p = Self::new(&q.root, sort, q.hidden);
        p.all.retain(|e| e.is_parent());
        p.results = Some(Results {
            what,
            search: Some(find::start(q)),
            scanned: 0,
            unreadable: 0,
        });
        p.refilter(None);
        p
    }

    /// Take in what the search has found since the last call. True when anything
    /// changed.
    pub fn poll_results(&mut self) -> bool {
        let Some(r) = self.results.as_mut() else {
            return false;
        };
        let Some(search) = r.search.as_ref() else {
            return false;
        };
        let mut found = Vec::new();
        let mut finished = false;
        for ev in search.rx.try_iter() {
            match ev {
                find::Event::Found(p) => found.push(p),
                find::Event::Progress { scanned } => r.scanned = scanned,
                find::Event::Done {
                    scanned,
                    unreadable,
                } => {
                    r.scanned = scanned;
                    r.unreadable = unreadable;
                    finished = true;
                }
            }
        }
        if finished {
            r.search = None;
        }
        if found.is_empty() {
            return finished;
        }
        for p in found {
            let name = p
                .strip_prefix(&self.cwd)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
            self.all.push(entry_at(&p, name));
        }
        let keep = self.current().map(|e| e.path.clone());
        sort_entries(&mut self.all, self.sort);
        self.refilter(keep);
        true
    }

    /// Stop a search still running. True if there was one.
    pub fn stop_search(&mut self) -> bool {
        match self.results.as_mut().and_then(|r| r.search.take()) {
            Some(s) => {
                s.cancel();
                true
            }
            None => false,
        }
    }

    /// Browse `file` as a directory. False, with a notice, if it cannot be read.
    /// Browse `file` as a directory: at once if it is small, else from a thread, with a
    /// notice meanwhile.
    pub fn enter_archive(&mut self, file: &Path) {
        // A link inside the root to an archive outside it: its contents are outside.
        if !xdevcmd_core::policy::get().allows(file) {
            self.notice = Some("that archive is outside the root xdevcmd is confined to".into());
            return;
        }
        let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
        if size < LIST_AT_ONCE {
            let listed = archive::list(file);
            self.show_archive(file, listed);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (path, flag) = (file.to_path_buf(), stop.clone());
        std::thread::spawn(move || {
            let _ = tx.send(archive::list_until(&path, &flag));
        });
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        self.notice = Some(format!("opening {name}\u{2026} Esc stops it"));
        self.opening = Some(Opening {
            file: file.to_path_buf(),
            rx,
            stop,
        });
    }

    /// Take in a listing that has arrived. True when one did.
    pub fn poll_opening(&mut self) -> bool {
        let Some(o) = &self.opening else { return false };
        let Ok(listed) = o.rx.try_recv() else {
            return false;
        };
        let file = o.file.clone();
        self.opening = None;
        self.show_archive(&file, listed);
        true
    }

    /// Give up on an archive still being listed. True if there was one.
    pub fn stop_opening(&mut self) -> bool {
        self.opening.take().is_some()
    }

    fn show_archive(&mut self, file: &Path, listed: std::io::Result<Listing>) {
        match listed {
            Ok(listing) => {
                if listing.unsafe_names > 0 {
                    self.notice = Some(format!(
                        "{} entries with names outside the archive are hidden and will not be extracted",
                        listing.unsafe_names
                    ));
                }
                self.archive = Some(ArchiveView {
                    file: file.to_path_buf(),
                    prefix: String::new(),
                    listing,
                });
                self.cursor = 0;
                self.offset = 0;
                self.marked.clear();
                self.filter = None;
                self.reload();
            }
            Err(e) => {
                self.notice = Some(format!("cannot open the archive: {e}"));
            }
        }
    }

    fn leave_archive(&mut self) {
        if let Some(a) = self.archive.take() {
            self.marked.clear();
            self.reload();
            self.select_path(&a.file);
        }
    }

    fn read(&self) -> std::io::Result<Vec<Entry>> {
        if let Some(a) = &self.archive {
            let here = if a.prefix.is_empty() {
                a.file.clone()
            } else {
                a.file.join(&a.prefix)
            };
            let up = if a.prefix.is_empty() {
                self.cwd.clone()
            } else {
                match a.prefix.rsplit_once('/') {
                    Some((p, _)) => a.file.join(p),
                    None => a.file.clone(),
                }
            };
            let mut v = vec![Entry {
                name: "..".into(),
                path: up,
                kind: Kind::Parent,
                size: 0,
                modified: None,
                hidden: false,
                symlink: false,
                error: None,
            }];
            for (name, it) in archive::children(&a.listing.items, &a.prefix) {
                v.push(Entry {
                    path: here.join(&name),
                    name,
                    kind: if it.kind == ItemKind::Dir {
                        Kind::Dir
                    } else {
                        Kind::File
                    },
                    size: it.size,
                    modified: it.modified,
                    hidden: false,
                    symlink: it.kind == ItemKind::Link,
                    error: None,
                });
            }
            return Ok(v);
        }
        if self.results.is_some() {
            // Results are re-checked, not re-searched: what was deleted or moved away
            // goes, what is still there gets its size and time again.
            let mut v = vec![Entry {
                name: "..".into(),
                path: self.cwd.clone(),
                kind: Kind::Parent,
                size: 0,
                modified: None,
                hidden: false,
                symlink: false,
                error: None,
            }];
            v.extend(
                self.all
                    .iter()
                    .filter(|e| !e.is_parent())
                    .filter(|e| std::fs::symlink_metadata(&e.path).is_ok())
                    .map(|e| entry_at(&e.path, e.name.clone())),
            );
            return Ok(v);
        }
        let mut v = read_dir(&self.cwd, self.show_hidden)?;
        // At the root there is no way up, so no `..` to offer one.
        if xdevcmd_core::policy::get().is_root(&self.cwd) {
            v.retain(|e| !e.is_parent());
        }
        // A size already worked out for a directory stays, so sorting by size and
        // the selection total both see it.
        for e in v.iter_mut().filter(|e| e.is_dir()) {
            if let Some(t) = self.sizes.get(&e.path) {
                e.size = t.bytes;
            }
        }
        Ok(v)
    }

    /// Read the directory again. The cursor stays on the same file wherever it now
    /// sorts — a refresh that left it on whatever slid into its row would make the
    /// next key act on the wrong file.
    pub fn reload(&mut self) {
        let keep = self.current().map(|e| e.path.clone());
        match self.read() {
            Ok(mut v) => {
                sort_entries(&mut v, self.sort);
                self.all = v;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{}: {e}", self.cwd.display())),
        }
        self.checked = (Instant::now(), dir_mtime(&self.cwd));
        self.refilter(keep);
        if crate::look::get().git && self.archive.is_none() && self.results.is_none() {
            self.git_rx = Some(crate::git::start(&self.cwd));
        } else {
            self.git = None;
        }
    }

    /// Take in git's answer if it has come. True when it did.
    pub fn poll_git(&mut self) -> bool {
        let Some(rx) = &self.git_rx else { return false };
        match rx.try_recv() {
            Ok(m) => {
                self.git = Some(m);
                self.git_rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.git_rx = None;
                false
            }
            Err(_) => false,
        }
    }

    /// Rebuild `entries` from `all` through the filter, keeping the cursor on `keep`.
    fn refilter(&mut self, keep: Option<PathBuf>) {
        let filter = self.filter.clone().filter(|f| !f.is_empty());
        let mut scored: Vec<(i32, &Entry)> = self
            .all
            .iter()
            .filter_map(|e| {
                if e.is_parent() {
                    return Some((i32::MAX, e));
                }
                match filter.as_deref() {
                    None => Some((0, e)),
                    Some(t) => filter_score(t, &e.name, e.is_dir()).map(|s| (s, e)),
                }
            })
            .collect();
        // A fuzzy filter puts the best matches first (directories still before files);
        // equal scores keep the listing's order.
        if filter.is_some() && fuzzy() {
            scored.sort_by_key(|(s, e)| (!e.is_parent(), !e.is_dir(), std::cmp::Reverse(*s)));
        }
        self.entries = scored.into_iter().map(|(_, e)| e.clone()).collect();
        // A mark on a row the filter hides would be acted on by F8 without being on
        // screen. Hidden means unmarked.
        self.marked
            .retain(|p| self.entries.iter().any(|e| &e.path == p));
        match keep.and_then(|k| self.entries.iter().position(|e| e.path == k)) {
            Some(i) => self.cursor = i,
            None => self.cursor = self.cursor.min(self.entries.len().saturating_sub(1)),
        }
    }

    pub fn resort(&mut self) {
        let keep = self.current().map(|e| e.path.clone());
        sort_entries(&mut self.all, self.sort);
        self.refilter(keep);
    }

    /// Show only what matches `text`; `None` shows everything again.
    pub fn set_filter(&mut self, text: Option<String>) {
        let keep = self.current().map(|e| e.path.clone());
        self.filter = text.filter(|t| !t.is_empty());
        self.refilter(keep);
    }

    /// Re-read the directory if it changed since it was last looked at, at most once
    /// per `every`. Returns whether it did.
    pub fn poll_changes(&mut self, every: Duration) -> bool {
        if every.is_zero() || self.checked.0.elapsed() < every {
            return false;
        }
        self.check_now()
    }

    /// Look now whether the directory changed, and re-read it if so. True when it did.
    pub fn check_now(&mut self) -> bool {
        if self.results.is_some() || self.archive.is_some() {
            return false;
        }
        let mtime = dir_mtime(&self.cwd);
        let changed = mtime != self.checked.1
            || (self.all.len() <= FULL_CHECK_LIMIT && self.listing_changed());
        if changed {
            self.reload();
        } else {
            self.checked = (Instant::now(), mtime);
        }
        changed
    }

    fn listing_changed(&self) -> bool {
        let Ok(mut now) = self.read() else {
            return self.error.is_none();
        };
        if now.len() != self.all.len() {
            return true;
        }
        let key = |e: &Entry| (e.path.clone(), e.size, e.modified, e.kind);
        now.sort_by(|a, b| a.path.cmp(&b.path));
        let mut was: Vec<_> = self.all.iter().map(key).collect();
        was.sort_by(|a, b| a.0.cmp(&b.0));
        now.iter().map(key).ne(was)
    }

    // ── directory sizes ────────────────────────────────────────────────────────────

    /// Start working out the size of the directory at `path`, unless it is known or
    /// already being worked out.
    pub fn request_size(&mut self, path: &Path) {
        if self.sizes.contains_key(path) || !self.sizing.insert(path.to_path_buf()) {
            return;
        }
        self.sizer.request(path.to_path_buf());
    }

    /// Every directory in view, `..` aside.
    pub fn request_all_sizes(&mut self) -> usize {
        let dirs: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|e| e.is_dir() && !e.is_parent())
            .map(|e| e.path.clone())
            .collect();
        for d in &dirs {
            self.request_size(d);
        }
        dirs.len()
    }

    /// Take in sizes that have arrived. True when any did.
    pub fn poll_sizes(&mut self) -> bool {
        let got = self.sizer.poll();
        if got.is_empty() {
            return false;
        }
        for (p, t) in got {
            self.sizing.remove(&p);
            for e in self.all.iter_mut().filter(|e| e.path == p) {
                e.size = t.bytes;
            }
            self.sizes.insert(p, t);
        }
        let keep = self.current().map(|e| e.path.clone());
        if self.sort.key == SortKey::Size {
            sort_entries(&mut self.all, self.sort);
        }
        self.refilter(keep);
        true
    }

    pub fn set_sort(&mut self, key: SortKey) {
        if self.sort.key == key {
            self.sort.reverse = !self.sort.reverse;
        } else {
            self.sort.key = key;
            self.sort.reverse = false;
        }
        self.resort();
    }

    pub fn current(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, last) as usize;
    }

    pub fn go_to(&mut self, i: usize) {
        if !self.entries.is_empty() {
            self.cursor = i.min(self.entries.len() - 1);
        }
    }

    pub fn go_end(&mut self) {
        self.cursor = self.entries.len().saturating_sub(1);
    }

    /// Enter the directory under the cursor, or report the file to act on.
    pub fn activate(&mut self) -> Option<PathBuf> {
        let e = self.current()?.clone();
        if self.archive.is_some() {
            if e.is_parent() {
                self.go_up();
            } else if e.is_dir() {
                if let Some(a) = self.archive.as_mut() {
                    a.prefix = if a.prefix.is_empty() {
                        e.name.clone()
                    } else {
                        format!("{}/{}", a.prefix, e.name)
                    };
                }
                self.cursor = 0;
                self.offset = 0;
                self.marked.clear();
                self.reload();
            } else {
                return Some(e.path.clone());
            }
            return None;
        }
        // Enter on an archive goes into it, as into a directory.
        if !e.is_dir() && archive::format_of(&e.path).is_some() && self.results.is_none() {
            let file = e.path.clone();
            self.enter_archive(&file);
            return None;
        }
        // In search results Enter goes to where the file is, cursor on it — the thing
        // you search for a file to do. F3 still views it.
        if self.results.is_some() && !e.is_dir() {
            let file = e.path.clone();
            if let (Some(dir), Some(name)) = (file.parent(), file.file_name()) {
                self.set_cwd(dir);
                // By name in the directory as it now reads: set_cwd resolves links, so
                // the path the search found (/var/… on macOS) may not be the path the
                // listing has (/private/var/…).
                let here = self.cwd.join(name);
                self.select_path(&here);
            }
            return None;
        }
        if e.is_dir() {
            let target = e.path.clone();
            self.set_cwd(&target);
            None
        } else {
            Some(e.path.clone())
        }
    }

    /// The directory named in the title under (x, y), if any.
    pub fn crumb_at(&self, x: u16, y: u16) -> Option<PathBuf> {
        let p = ratatui::layout::Position::new(x, y);
        self.crumbs
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|(_, d)| d.clone())
    }

    pub fn set_cwd(&mut self, path: &Path) {
        self.navigate(path, true);
    }

    /// Go to `path`. `record` puts the directory being left on the back list, which
    /// every move does except stepping through that list itself.
    fn navigate(&mut self, path: &Path, record: bool) -> bool {
        let canonical = xdevcmd_core::paths::canonical(path).unwrap_or_else(|_| path.to_path_buf());
        let policy = xdevcmd_core::policy::get();
        if !policy.allows(&canonical) {
            if let Some(root) = &policy.root {
                self.notice = Some(format!(
                    "outside {} \u{2014} xdevcmd is confined there",
                    root.display()
                ));
            }
            return false;
        }
        self.archive = None;
        self.opening = None;
        // Leaving a results tab for a directory, the search is over.
        if let Some(mut r) = self.results.take() {
            if let Some(s) = r.search.take() {
                s.cancel();
            }
        }
        let old = std::mem::replace(&mut self.cwd, canonical);
        if record && old != self.cwd {
            self.back.retain(|p| *p != old);
            self.back.push(old.clone());
            if self.back.len() > HISTORY {
                self.back.remove(0);
            }
            self.forward.clear();
        }
        self.cursor = 0;
        self.offset = 0;
        self.marked.clear();
        self.filter = None;
        self.sizer.cancel();
        self.sizing.clear();
        self.sizes.clear();
        self.reload();
        // Coming up out of a directory lands on the directory you came from, however
        // you came up: `..`, Backspace, Back, or a bookmark.
        if old.parent() == Some(self.cwd.as_path()) {
            self.select_path(&old);
        }
        true
    }

    /// Back to the directory before this one. False when there is none.
    pub fn go_back(&mut self) -> bool {
        while let Some(p) = self.back.pop() {
            let here = self.cwd.clone();
            if p.is_dir() && self.navigate(&p, false) {
                self.forward.push(here);
                return true;
            }
        }
        false
    }

    pub fn go_forward(&mut self) -> bool {
        while let Some(p) = self.forward.pop() {
            let here = self.cwd.clone();
            if p.is_dir() && self.navigate(&p, false) {
                self.back.push(here);
                return true;
            }
        }
        false
    }

    /// Directories visited, most recent first, without repeats or the current one.
    pub fn history(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for p in self.back.iter().rev().chain(self.forward.iter().rev()) {
            if *p != self.cwd && !out.contains(p) {
                out.push(p.clone());
            }
        }
        out
    }

    pub fn go_up(&mut self) {
        if let Some(a) = self.archive.as_mut() {
            if a.prefix.is_empty() {
                self.leave_archive();
            } else {
                let leaving = a.file.join(&a.prefix);
                a.prefix = a
                    .prefix
                    .rsplit_once('/')
                    .map(|(p, _)| p.to_string())
                    .unwrap_or_default();
                self.marked.clear();
                self.reload();
                self.select_path(&leaving);
            }
            return;
        }
        if self.results.is_some() {
            let root = self.cwd.clone();
            self.set_cwd(&root);
            return;
        }
        if xdevcmd_core::policy::get().is_root(&self.cwd) {
            self.notice = Some("this is the top \u{2014} xdevcmd is confined here".into());
            return;
        }
        if let Some(parent) = self.cwd.parent().map(Path::to_path_buf) {
            self.set_cwd(&parent);
        }
    }

    pub fn toggle_mark(&mut self) {
        let Some(e) = self.entries.get(self.cursor) else {
            return;
        };
        if e.is_parent() {
            return;
        }
        if !self.marked.remove(&e.path) {
            self.marked.insert(e.path.clone());
        }
    }

    pub fn mark_all(&mut self, on: bool) {
        self.marked.clear();
        if on {
            for e in self.entries.iter().filter(|e| !e.is_parent()) {
                self.marked.insert(e.path.clone());
            }
        }
    }

    /// Mark every entry between two rows, inclusive, in either order. `..` is skipped.
    /// Move the cursor to `path` if it is listed. Used after a rename, so the file you
    /// just renamed is still the one under the cursor rather than whatever sorted into
    /// its old position.
    pub fn select_path(&mut self, path: &Path) {
        if let Some(i) = self.entries.iter().position(|e| e.path == path) {
            self.cursor = i;
        }
    }

    pub fn mark_range(&mut self, a: usize, b: usize) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        for e in self.entries.iter().skip(lo).take(hi + 1 - lo) {
            if !e.is_parent() {
                self.marked.insert(e.path.clone());
            }
        }
    }

    /// Mark (or unmark) every row whose name matches `pattern`. Returns how many rows
    /// matched.
    pub fn mark_matching(&mut self, pattern: &Pattern, on: bool) -> usize {
        let mut n = 0;
        for e in self.entries.iter().filter(|e| !e.is_parent()) {
            if pattern.matches(&e.name, e.is_dir()) {
                n += 1;
                if on {
                    self.marked.insert(e.path.clone());
                } else {
                    self.marked.remove(&e.path);
                }
            }
        }
        n
    }

    pub fn invert_marks(&mut self) {
        for e in self.entries.iter().filter(|e| !e.is_parent()) {
            if !self.marked.remove(&e.path) {
                self.marked.insert(e.path.clone());
            }
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.reload();
    }

    /// `n selected, size` for the status bar, or the entry under the cursor.
    pub fn summary(&self) -> String {
        if !self.marked.is_empty() {
            let total: u64 = self
                .entries
                .iter()
                .filter(|e| self.marked.contains(&e.path))
                .map(|e| e.size)
                .sum();
            // Directories report 0 bytes, so showing "3 selected  0" would look broken
            // rather than informative.
            if total == 0 {
                format!("{} selected", self.marked.len())
            } else {
                format!("{} selected  {}", self.marked.len(), human_size(total))
            }
        } else {
            match self.current() {
                Some(e) if e.is_parent() => "..".to_string(),
                // A directory's size is unknown until worked out (Space, Ctrl+Space);
                // "0" would be a wrong answer rather than no answer.
                Some(e) if e.is_dir() && !self.sizes.contains_key(&e.path) => e.name.clone(),
                Some(e) => format!("{}  {}", e.name, human_size(e.size)),
                None => "empty".to_string(),
            }
        }
    }

    fn scroll_into_view(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + height {
            self.offset = self.cursor + 1 - height;
        }
        let max_offset = self.entries.len().saturating_sub(height);
        self.offset = self.offset.min(max_offset);
    }

    pub fn page(&mut self, pages: isize) {
        let h = self.rows_rect.height.max(1) as isize;
        self.move_cursor(pages * h);
    }

    pub fn scroll(&mut self, lines: isize) {
        self.set_offset((self.offset as isize + lines).max(0) as usize);
    }

    /// Show the list from `offset`. The cursor comes along to stay on screen, as the
    /// wheel does in other managers; left behind, the next frame would scroll back to it.
    fn set_offset(&mut self, offset: usize) {
        let h = (self.rows_rect.height as usize).max(1);
        let max_offset = self.entries.len().saturating_sub(h);
        self.offset = offset.min(max_offset);
        self.cursor = self.cursor.clamp(self.offset, self.offset + h - 1);
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    /// The scrollbar under the pointer, and where on its thumb it took hold.
    pub fn bar_at(&self, x: u16, y: u16) -> Option<u16> {
        self.bar.filter(|b| b.contains(x, y)).map(|b| b.grab(y))
    }

    /// The thumb, held at `grab`, dragged to row `y`.
    pub fn drag_bar(&mut self, y: u16, grab: u16) {
        if let Some(b) = self.bar {
            self.set_offset(b.offset_at(y, grab));
        }
    }

    /// Turn a click position into a row index, if it landed on one.
    pub fn row_at(&self, x: u16, y: u16) -> Option<usize> {
        let r = self.rows_rect;
        if x < r.x || x >= r.x + r.width || y < r.y || y >= r.y + r.height {
            return None;
        }
        let i = self.offset + (y - r.y) as usize;
        (i < self.entries.len()).then_some(i)
    }

    pub fn header_at(&self, x: u16, y: u16) -> Option<SortKey> {
        self.header_cells
            .iter()
            .find_map(|(k, r)| (x >= r.x && x < r.x + r.width && y == r.y).then_some(*k))
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, active: bool, drop_target: bool) {
        // While files are dragged over this pane it changes colour, so a release is
        // never a guess about where they will land.
        let border = if drop_target {
            Style::new()
                .fg(theme::t().mark)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::border_style(active)
        };
        // A filter hides files, so it is never something you have to remember is on.
        let mut filter = self
            .filter
            .as_ref()
            .map(|f| format!(" filter: {f} "))
            .unwrap_or_default();
        if let Some(a) = &self.archive {
            let name = a.file.file_name().unwrap_or_default().to_string_lossy();
            filter = format!(" \u{25b8} {name}/{} {filter}", a.prefix);
        }
        if let Some(r) = &self.results {
            let n = self.all.iter().filter(|e| !e.is_parent()).count();
            let state = if r.running() {
                format!("searching\u{2026} {} looked at", r.scanned)
            } else {
                format!("{n} found")
            };
            filter = format!(" find {} \u{b7} {state} {filter}", r.what);
        }
        let fw = filter
            .chars()
            .count()
            .min(area.width.saturating_sub(12) as usize / 2);
        let full = self.cwd.display().to_string();
        let room = (area.width as usize).saturating_sub(6 + fw);
        // The title starts one cell in from the corner, after a space.
        self.crumbs = crumbs(&self.cwd, &full, room, area.x + 2, area.y);
        let mut title = vec![
            Span::styled(" ", border),
            Span::styled(elide_middle(&full, room), theme::title_style(active)),
            Span::styled(" ", border),
        ];
        if fw > 0 {
            title.push(Span::styled(
                elide_middle(&filter, fw),
                Style::new()
                    .fg(theme::t().bg)
                    .bg(theme::t().mark)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        let title = Line::from(title);
        let block = Block::bordered()
            .border_style(border)
            .style(Style::new().bg(theme::t().bg_panel))
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 8 || inner.height < 2 {
            return;
        }

        // ── column geometry ────────────────────────────────────────────────────────
        let w = inner.width;
        // Thresholds are the point at which dropping the column leaves Name usable:
        // with all four present the fixed columns cost 34 of the width.
        let w_date: u16 = if w >= 52 { 16 } else { 0 };
        let w_size: u16 = if w >= 34 { 9 } else { 0 };
        let w_ext: u16 = if w >= 26 { 5 } else { 0 };
        let gutter: u16 = 1;
        let used = gutter
            + w_ext
            + w_size
            + w_date
            + [w_ext, w_size, w_date].iter().filter(|c| **c > 0).count() as u16;
        let w_name = w.saturating_sub(used).max(4);

        let x_name = inner.x + gutter;
        let x_ext = x_name + w_name + 1;
        let x_size = if w_ext > 0 { x_ext + w_ext + 1 } else { x_ext };
        let x_date = if w_size > 0 {
            x_size + w_size + 1
        } else {
            x_size
        };

        // ── header ─────────────────────────────────────────────────────────────────
        let hy = inner.y;
        self.header_cells.clear();
        let mut header: Vec<Span> = vec![Span::raw(" ")];
        let push_header = |cells: &mut Vec<(SortKey, Rect)>,
                           spans: &mut Vec<Span<'static>>,
                           key: SortKey,
                           label: &str,
                           x: u16,
                           width: u16,
                           right: bool| {
            if width == 0 {
                return;
            }
            let marker = if self.sort.key == key {
                if self.sort.reverse {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            let text = format!("{label}{marker}");
            let text = elide_middle(&text, width as usize);
            let padded = if right {
                format!("{text:>width$}", width = width as usize)
            } else {
                format!("{text:<width$}", width = width as usize)
            };
            let style = if self.sort.key == key {
                Style::new()
                    .fg(theme::t().accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme::t().fg_dim)
            };
            spans.push(Span::styled(padded, style));
            spans.push(Span::raw(" "));
            cells.push((key, Rect::new(x, hy, width, 1)));
        };
        let mut cells = Vec::new();
        push_header(
            &mut cells,
            &mut header,
            SortKey::Name,
            "Name",
            x_name,
            w_name,
            false,
        );
        push_header(
            &mut cells,
            &mut header,
            SortKey::Ext,
            "Ext",
            x_ext,
            w_ext,
            false,
        );
        push_header(
            &mut cells,
            &mut header,
            SortKey::Size,
            "Size",
            x_size,
            w_size,
            true,
        );
        push_header(
            &mut cells,
            &mut header,
            SortKey::Modified,
            "Modified",
            x_date,
            w_date,
            false,
        );
        self.header_cells = cells;

        let buf = frame.buffer_mut();
        buf.set_style(
            Rect::new(inner.x, hy, inner.width, 1),
            Style::new().bg(theme::t().bg_bar),
        );
        buf.set_line(inner.x, hy, &Line::from(header), inner.width);

        // ── rows ───────────────────────────────────────────────────────────────────
        let rows = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        self.rows_rect = rows;
        self.scroll_into_view(rows.height as usize);

        if let Some(err) = &self.error {
            let line = Line::from(Span::styled(
                elide_middle(err, rows.width as usize),
                Style::new().fg(theme::t().err),
            ));
            frame
                .buffer_mut()
                .set_line(rows.x, rows.y, &line, rows.width);
            return;
        }

        for row in 0..rows.height {
            let idx = self.offset + row as usize;
            let Some(e) = self.entries.get(idx) else {
                break;
            };
            let y = rows.y + row;
            let is_cursor = idx == self.cursor;
            let is_marked = self.marked.contains(&e.path);

            let look = crate::look::get();
            let base = look
                .ls
                .as_ref()
                .and_then(|l| l.style(e, matches!(e.kind, Kind::File) && theme::is_executable(e)))
                .unwrap_or_else(|| theme::entry_style(e));
            let base = if is_marked {
                Style::new()
                    .fg(theme::t().mark)
                    .add_modifier(Modifier::BOLD)
            } else {
                base
            };

            let name_text = if e.is_dir() {
                e.name.clone()
            } else {
                e.stem_ext().0.to_string()
            };
            let name_text = if look.icons {
                format!("{} {name_text}", crate::look::icon(e))
            } else {
                name_text
            };
            let ext_text = if w_ext > 0 {
                e.stem_ext().1.to_string()
            } else {
                String::new()
            };
            let size_text = if let Some(t) = e.is_dir().then(|| self.sizes.get(&e.path)).flatten() {
                // A walk that met unreadable directories found at least this much.
                let plus = if t.unreadable > 0 { "+" } else { "" };
                format!("{}{plus}", human_size(t.bytes))
            } else if e.is_dir() && self.sizing.contains(&e.path) {
                "\u{2026}".to_string()
            } else if e.is_dir() {
                "<DIR>".to_string()
            } else {
                human_size(e.size)
            };

            // The gutter: the mark, or else git's letter for the row.
            let git = self.git.as_ref().and_then(|m| m.get(&e.name)).copied();
            let mut spans = vec![match (is_marked, git) {
                (true, _) => Span::styled("\u{2022}", Style::new().fg(theme::t().mark)),
                (false, Some(g)) => Span::styled(
                    g.to_string(),
                    Style::new().fg(match g {
                        'U' | 'D' => theme::t().err,
                        '?' | 'A' => theme::t().exec,
                        _ => theme::t().archive,
                    }),
                ),
                (false, None) => Span::raw(" "),
            }];
            // Fitted in columns, not characters: a CJK name is wider than it is long.
            spans.push(Span::styled(fit_start(&name_text, w_name as usize), base));
            if w_ext > 0 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(fit_start(&ext_text, w_ext as usize), base));
            }
            if w_size > 0 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    fit_end(&size_text, w_size as usize),
                    if e.is_dir() {
                        Style::new().fg(theme::t().fg_dim)
                    } else {
                        base
                    },
                ));
            }
            if w_date > 0 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("{:<w$}", format_time(e.modified), w = w_date as usize),
                    Style::new().fg(theme::t().fg_dim),
                ));
            }

            // The cursor's row shows its whole name when the column cuts it: the name
            // takes the row, over Ext, Size and Modified, which the status line still
            // gives for that file. Every other row keeps the columns.
            if is_cursor && text_cells(&name_text) > w_name as usize {
                let whole = if look.icons {
                    format!("{} {}", crate::look::icon(e), e.name)
                } else {
                    e.name.clone()
                };
                spans.truncate(1);
                spans.push(Span::styled(
                    fit_start(&whole, rows.width.saturating_sub(gutter) as usize),
                    base,
                ));
            }

            let buf = frame.buffer_mut();
            if is_cursor {
                let bg = if active {
                    theme::t().row_cursor_active
                } else {
                    theme::t().row_cursor
                };
                buf.set_style(Rect::new(rows.x, y, rows.width, 1), Style::new().bg(bg));
            }
            buf.set_line(rows.x, y, &Line::from(spans), rows.width);
        }
        // Over the right border, beside the rows: how long the list is, and where.
        self.bar = crate::scrollbar::draw(
            frame.buffer_mut(),
            crate::scrollbar::track(area, rows),
            self.entries.len(),
            self.offset,
            border,
        );
    }
}

/// Where each directory of `cwd` lands in its title, drawn from `x0` as `full` elided
/// to `room` characters: one rectangle per name, for the directories above `cwd`. A
/// name hidden in the elided middle gets none; one cut by it is clickable on what
/// shows. Measured in cells, as the terminal draws: a CJK character takes two, a
/// combining accent (as in macOS's decomposed file names) none.
fn crumbs(cwd: &Path, full: &str, room: usize, x0: u16, y: u16) -> Vec<(Rect, PathBuf)> {
    use unicode_width::UnicodeWidthChar;
    let n = full.chars().count();
    // Which character of the drawn text character `i` of `full` became, as
    // elide_middle lays it out.
    let cut = elided(full, room);
    let at = |i: usize| -> Option<usize> {
        let Some((head, tail)) = cut else {
            return Some(i);
        };
        if i < head {
            Some(i)
        } else if i >= n - tail {
            Some(head + 1 + i - (n - tail))
        } else {
            None
        }
    };
    // The cell each drawn character starts at, and how wide it is.
    let mut cells = Vec::new();
    let mut x = 0usize;
    for c in elide_middle(full, room).chars() {
        let w = c.width().unwrap_or(0);
        cells.push((x, w));
        x += w;
    }
    let cell = |k: usize| cells.get(k).copied();
    let mut out = Vec::new();
    let mut prev: Option<usize> = None;
    let mut dirs: Vec<&Path> = cwd.ancestors().collect();
    dirs.reverse();
    for d in dirs {
        let s = d.display().to_string();
        if !full.starts_with(&s) {
            break;
        }
        let end = s.chars().count();
        let start = match prev {
            None => 0,
            // After the separator, unless the parent already ends in one ("/", "C:\").
            Some(p)
                if full[..]
                    .chars()
                    .nth(p.saturating_sub(1))
                    .is_some_and(std::path::is_separator) =>
            {
                p
            }
            Some(p) => p + 1,
        };
        prev = Some(end);
        if d == cwd || start >= end {
            continue;
        }
        let shown: Vec<(usize, usize)> = (start..end).filter_map(at).filter_map(cell).collect();
        if let (Some(a), Some(b)) = (shown.first(), shown.last()) {
            let w = (b.0 + b.1).saturating_sub(a.0).max(1) as u16;
            out.push((Rect::new(x0 + a.0 as u16, y, w, 1), d.to_path_buf()));
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn each_directory_in_the_title_is_where_it_was_drawn() {
        let cwd = Path::new("/home/me/src");
        let full = "/home/me/src";
        let c = crumbs(cwd, full, 40, 10, 0);
        let got: Vec<(u16, u16, &str)> = c
            .iter()
            .map(|(r, d)| (r.x, r.width, d.to_str().unwrap()))
            .collect();
        // "/" at 10, "home" at 11..15, "me" at 16..18; "src" is where it already is.
        assert_eq!(got, [(10, 1, "/"), (11, 4, "/home"), (16, 2, "/home/me")]);
        // Elided to 9 cells, "/hom…/src": "home" is cut but still there, "me" is gone.
        let c = crumbs(cwd, full, 9, 0, 0);
        let got: Vec<(u16, u16, &str)> = c
            .iter()
            .map(|(r, d)| (r.x, r.width, d.to_str().unwrap()))
            .collect();
        assert_eq!(got, [(0, 1, "/"), (1, 3, "/home")]);
        // Wide characters take two cells each: "日本" is four cells, and "src" after it
        // starts at cell 6.
        let c = crumbs(Path::new("/日本/src/x"), "/日本/src/x", 40, 0, 0);
        let got: Vec<(u16, u16)> = c.iter().map(|(r, _)| (r.x, r.width)).collect();
        assert_eq!(got, [(0, 1), (1, 4), (6, 3)]);
    }
    use std::io::Read as _;

    fn big_tar(dir: &Path) -> PathBuf {
        let t = dir.join("big.tar");
        let mut b = tar::Builder::new(std::fs::File::create(&t).unwrap());
        let mut h = tar::Header::new_gnu();
        h.set_size(LIST_AT_ONCE + 1024);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(
            &mut h,
            "inside.bin",
            std::io::repeat(7).take(LIST_AT_ONCE + 1024),
        )
        .unwrap();
        b.finish().unwrap();
        t
    }

    #[test]
    fn a_big_archive_opens_from_a_thread_and_esc_can_give_up_on_it() {
        let dir =
            std::env::temp_dir().join(format!("xdevcmd-files-bigarch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tar = big_tar(&dir);
        let mut p = FilePane::new(&dir, Sort::default(), false);

        p.enter_archive(&tar);
        assert!(
            p.archive.is_none() && p.opening.is_some(),
            "not on the UI thread"
        );
        assert!(p
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("opening big.tar"));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !p.poll_opening() {
            assert!(Instant::now() < deadline, "the listing never came");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(p.archive.is_some());
        assert!(p.entries.iter().any(|e| e.name == "inside.bin"));

        p.go_up();
        p.enter_archive(&tar);
        assert!(p.stop_opening(), "Esc gives up");
        assert!(!p.poll_opening() && p.archive.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
