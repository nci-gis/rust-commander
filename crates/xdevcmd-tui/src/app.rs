use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::backend::{Backend, ClearType};
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

/// The real terminal: crossterm on stdout, behind the backend that skips idle frames.
pub type Term =
    ratatui::Terminal<crate::quiet::Quiet<ratatui::backend::CrosstermBackend<std::io::Stdout>>>;

use xdevcmd_core::format::{elide_middle, human_size};
use xdevcmd_core::ops::{self, Request};
use xdevcmd_core::{Document, SortKey};

use crate::bookmarks::{self, expand_tilde, tidy};
use crate::config::{Mode, Outcome, Settings};
use crate::find_dialog::{self, FindDialog};
use crate::help::Help;
use crate::keys::Action;
use crate::layout::{Axis, Divider, Node, PaneId};
use crate::line_edit::LineEdit;
use crate::menu::{Cmd, Item, Menu, Outcome as MenuOutcome};
use crate::modal::{self, Confirm, Input, InputKind, Job, Modal};
use crate::opener;
use crate::pane::tabs::Click as TabClick;
use crate::pane::terms::Click as TermClick;
use crate::pane::{FilePane, FileTabs, Pane, TermPanel};
use crate::rename_dialog::{self, RenameDialog};
use crate::search::{self, Search};
use crate::session::{self, Saved, SavedTab, SavedTerminal, Session};
use crate::settings_dialog::{self, SettingsDialog};
use crate::theme;
use crate::viewer::{self, Viewer};

/// What a press-and-drag turned out to be. It starts `Pending` and decides as it moves:
/// staying inside its own pane makes it a range selection, crossing into another file
/// pane makes it a drag-and-drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragMode {
    Pending,
    Range,
    Files,
}

pub(crate) struct Drag {
    source: PaneId,
    anchor: usize,
    /// Grabbing a marked row carries the whole selection; grabbing an unmarked one
    /// carries just that file, or draws a range if it never leaves the pane.
    anchor_was_marked: bool,
    marks_before: HashSet<PathBuf>,
    pub(crate) mode: DragMode,
    pub(crate) hover: Option<PaneId>,
}

/// What a key or a click on an open dialog asks for. Both input paths produce one of
/// these and `modal_act` carries it out, so a button can never do something its key
/// does not.
enum ModalAct {
    None,
    Close,
    Start,
    Apply,
    Finish,
    Run(Cmd),
    StartFind,
    /// F4 in the rename dialog: these names, one per file, into the editor.
    BatchEditor(String),
    /// Enter in File → Settings….
    SaveSettings,
    Background,
    /// Left/Right in a menu-bar menu.
    Bar(isize),
}

const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// A deep compare running for Alt+Y: the two roots, how far it has got, and the
/// diffs when they come.
struct Comparing {
    left: PathBuf,
    right: PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    seen: std::sync::Arc<std::sync::atomic::AtomicU64>,
    shown: u64,
    rx: Receiver<Option<Vec<xdevcmd_core::compare::Diff>>>,
}

/// What undo says while a delete to the trash is still running.
const TRASH_RUNNING: &str = "a delete to the trash that is still running";
const TICK: Duration = Duration::from_millis(40);
/// With nothing known to have changed, the screen is still drawn this often, for what
/// changes without telling (the usage view's clock, a listing's age). Drawing every
/// tick cost about 5 % of a core while idle (2026-11-07); a frame is about 2 ms.
const HEARTBEAT: Duration = Duration::from_secs(1);

/// A picture quick view has decoded: the file, the cell size it was shrunk for, the
/// file's modification time then (a changed file is decoded again), and the picture or
/// why there is none.
type Shrunk = (
    PathBuf,
    (u16, u16),
    Option<std::time::SystemTime>,
    Result<xdevcmd_core::image::Picture, String>,
);

/// A picture being decoded on a thread: what it is for, and where the result comes.
type Decoding = (
    PathBuf,
    (u16, u16),
    Option<std::time::SystemTime>,
    Receiver<Result<xdevcmd_core::image::Picture, String>>,
);

/// Where a confirmation's count goes until the count has been made.
const COUNTING: &str = "counting\u{2026}";

/// Hooks for extracting one entry to view: no questions, stoppable.
struct ViewHooks(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl xdevcmd_core::archive::Hooks for ViewHooks {
    fn stopped(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn begin(&mut self, _: &Path) {}
    fn conflict(&mut self, _: &Path, _: u64) -> Option<bool> {
        Some(true)
    }
    fn wrote(&mut self, _: u64) {}
    fn done(&mut self) {}
    fn skipped(&mut self, _: &Path, _: &str) {}
    fn failed(&mut self, _: &Path, _: &std::io::Error) {}
}

/// An archive entry being extracted for F3: the result, its directory, its name, and
/// the flag that stops it.
type PendingView = (
    Receiver<std::io::Result<()>>,
    PathBuf,
    String,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
);

/// The menu bar, left to right.
pub const BAR: &[&str] = &["File", "View", "Help"];

/// What quick view has open: the file, and a viewer on it.
type Preview = (PathBuf, Box<Viewer>);

pub struct App {
    tree: Node,
    panes: BTreeMap<PaneId, Pane>,
    focus: PaneId,
    next_id: PaneId,
    rects: Vec<(PaneId, Rect)>,
    term_id: Option<PaneId>,
    term_shown: bool,
    status: String,
    quit: bool,
    last_click: Option<(Instant, u16, u16)>,
    modal: Option<Modal>,
    /// Set by F4. Handled in `run`, which is the only place that can hand the real
    /// terminal over to another program and take it back.
    pending_edit: Option<PathBuf>,
    settings: Settings,
    config_outcome: Outcome,
    drag: Option<Drag>,
    /// Panes in the order they last had focus, most recent first. "The other pane"
    /// stops meaning anything once there are three, so it means "the one you were
    /// just in" instead.
    mru: Vec<PaneId>,
    /// The pane filling the screen, if one is zoomed. Cleared as soon as focus moves
    /// elsewhere, as tmux does: a hidden pane must never have the focus.
    zoom: Option<PaneId>,
    /// The boundary being dragged; `true` when it is inside the terminal panel.
    resizing: Option<(Divider, bool)>,
    /// A file pane's scrollbar being dragged, and where on the thumb it was held.
    scrolling: Option<(PaneId, u16)>,
    /// The group of two-key bindings open, by its first key: Alt+R, then arrows to
    /// resize, as many as wanted (see `on_key`). Open until a key outside it, Enter,
    /// Esc or the mouse; never by time, so the status line always tells the truth.
    group: Option<crate::keys::Chord>,
    /// The Option-key hint has been shown this session.
    option_hinted: bool,
    /// The theme in use, by name (View → Theme… marks it).
    theme_name: String,
    /// The last operation that wrote to the disk, for Ctrl+Z.
    last_undo: Option<xdevcmd_core::undo::Undo>,
    /// The sources of a delete to the trash still running, so its result can become
    /// the undo once it has said where everything went.
    trash_pending: Option<Vec<PathBuf>>,
    /// The key bar's buttons as last drawn.
    footer_hits: Vec<(Action, Rect)>,
    /// The panes' share of the frame as last drawn; dividers are measured against it.
    main: Rect,
    /// False under --screenshot, where writing escape sequences to stdout would end up
    /// in the captured frame.
    interactive: bool,
    /// The terminal panel's share of the height, in percent. Starts from the config,
    /// then follows whatever it was last resized to, through hiding and the session.
    term_height: u16,
    /// From a restored session, consumed by `boot`: how the terminal was left.
    restore_terminal: Option<(SavedTerminal, bool)>,
    /// Where the next shell the panel creates should start, when not the focused pane.
    next_shell_cwd: Option<PathBuf>,
    /// The whole frame from the last draw. A new shell pane is sized from this.
    screen: Rect,
    /// The quick search or filter line, while one is being typed.
    search: Option<Search>,
    /// `--cwd-file`: where to write the directory on quit, for a shell to cd to.
    pub cwd_file: Option<PathBuf>,
    /// Whether this quit should write it: `q` does, `Q` does not.
    quit_cd: bool,
    /// Bookmarks added from inside xdevcmd; the config's own are in `settings`.
    bookmarks: Vec<PathBuf>,
    /// Messages from background work (a program that failed to open a file), shown
    /// in the status line when they arrive.
    notes: (Sender<String>, Receiver<String>),
    /// Shift+F2 in progress: the files, and the list of names handed to the editor.
    pending_batch: Option<(Vec<PathBuf>, PathBuf)>,
    /// Alt+Q: the pane showing a preview instead of its files, and what it shows.
    quick_view: Option<(PaneId, Option<Preview>)>,
    /// Alt+H: a checksum being worked out on its own thread.
    hashing: Option<(PathBuf, Receiver<std::io::Result<Option<String>>>)>,
    /// Alt+Y's deep compare, on a thread of its own: Esc stops it.
    comparing: Option<Comparing>,
    /// The system's directory watcher (interactive runs only; tests poll).
    watcher: Option<crate::watch::Watcher>,
    /// Each shown directory's canonical path, as the watcher reports it.
    canonical: HashMap<PathBuf, PathBuf>,
    /// Panes whose directory changed, to re-read when no dialog or drag is up.
    to_reload: HashSet<PaneId>,
    /// Panes whose directory has just begun to be watched, to look at once.
    to_check: HashSet<PaneId>,
    /// A file extracted from an archive so F3 can show it; removed when the viewer
    /// closes.
    view_temp: Option<PathBuf>,
    /// Copy, move, delete, extract and pack jobs sent to the background (B in the
    /// progress box). One that asks a question comes back to the front.
    jobs: Vec<Job>,
    /// Quick view's last picture: the file, the size it was shrunk for, the result.
    quick_image: Option<Shrunk>,
    /// A big photo takes a moment to decode: never on the drawing thread.
    decoding: Option<Decoding>,
    /// The menu bar's titles as last drawn, for clicks.
    bar_hits: Vec<(usize, Rect)>,
    pub usage: crate::usage::Usage,
    /// F3 inside a big archive, extracting on a thread.
    pending_view: Option<PendingView>,
    /// The session as last written, and when the last check for changes ran.
    saved: Option<Session>,
    last_autosave: Instant,
}

/// xdevcmd's icon for About: 14 columns by 7 rows, about as tall as About's text, so the
/// picture sits beside it rather than taking the dialog over (the owner's call,
/// 2026-11-07). Drawn by hand by `scripts/logo_pixels.py`, one colour per cell: the PNG
/// is 14 x 14 with every row twice, so each cell's two halves are equal and
/// [`modal::paint_picture`] paints it as background colour only. Half blocks came out
/// striped on macOS Terminal.app, which draws `▀` from the font (0.2.0).
const LOGO: &[u8] = include_bytes!("../../../assets/about-logo.png");

/// What xdevcmd made for itself goes with it. A file viewed from inside an archive is
/// extracted to a temporary directory, removed when the viewer closes — and here, when
/// xdevcmd ends with the viewer still open, or with an extraction still running.
impl Drop for App {
    fn drop(&mut self) {
        if let Some(t) = self.view_temp.take() {
            let _ = std::fs::remove_dir_all(&t);
        }
        if let Some((_, tmp, _, stop)) = self.pending_view.take() {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = std::fs::remove_dir_all(&tmp);
        }
        // And everything else this run put in the temporary directory.
        crate::tempfiles::remove();
    }
}

impl App {
    pub fn new(left: &Path, right: &Path, settings: Settings, outcome: Outcome) -> Self {
        let notice = outcome.message();
        let settings_height = settings.config.terminal.height_percent;
        let theme_name = settings.config.theme.name.clone();
        let mut panes = BTreeMap::new();
        panes.insert(
            0,
            Pane::files(FilePane::new(
                left,
                settings.sort,
                settings.config.files.show_hidden,
            )),
        );
        panes.insert(
            1,
            Pane::files(FilePane::new(
                right,
                settings.sort,
                settings.config.files.show_hidden,
            )),
        );
        Self {
            tree: Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1)),
            panes,
            focus: 0,
            next_id: 2,
            rects: Vec::new(),
            term_id: None,
            term_shown: false,
            status: notice.unwrap_or_default(),
            quit: false,
            last_click: None,
            modal: None,
            pending_edit: None,
            settings,
            config_outcome: outcome,
            drag: None,
            mru: Vec::new(),
            zoom: None,
            resizing: None,
            scrolling: None,
            group: None,
            option_hinted: false,
            theme_name,
            last_undo: None,
            trash_pending: None,
            footer_hits: Vec::new(),
            main: Rect::ZERO,
            interactive: false,
            term_height: settings_height,
            restore_terminal: None,
            next_shell_cwd: None,
            screen: Rect::new(0, 0, 80, 24),
            search: None,
            cwd_file: None,
            quit_cd: false,
            bookmarks: bookmarks::load(),
            notes: mpsc::channel(),
            pending_batch: None,
            quick_view: None,
            hashing: None,
            comparing: None,
            watcher: None,
            canonical: HashMap::new(),
            to_reload: HashSet::new(),
            to_check: HashSet::new(),
            view_temp: None,
            jobs: Vec::new(),
            quick_image: None,
            decoding: None,
            bar_hits: Vec::new(),
            usage: crate::usage::Usage::default(),
            pending_view: None,
            saved: None,
            last_autosave: Instant::now(),
        }
    }

    /// Bring the layout up for an area of this size: creates the shell pane that is
    /// visible by default. Separate from `run` so a screenshot can do it too.
    pub fn boot(&mut self, area: Rect) {
        self.screen = area;
        // No startup hint in the status line: the key bar already shows F1, F10 and F12,
        // and a hint that vanished at the first message and never came back looked
        // random. The right of the status line is for messages only.
        let restore = self.restore_terminal.take();
        if self.settings.mode == Mode::Restricted {
            return;
        }
        match restore {
            Some((term, focused)) => {
                if term.shown {
                    let fallback = self.focused_cwd();
                    let mut dirs = term
                        .tabs
                        .iter()
                        .map(|d| session::resolve_dir(d, &fallback))
                        .take(self.settings.config.terminal.max_terminals);
                    self.next_shell_cwd = dirs.next();
                    self.toggle_term(area);
                    let rest: Vec<PathBuf> = dirs.collect();
                    if let Some(p) = self.panel() {
                        for d in rest {
                            let _ = p.add_tab(&d);
                        }
                        p.cycle_tab(1); // back to the first tab
                    }
                    if focused {
                        if let Some(id) = self.term_id {
                            self.focus = id;
                        }
                    }
                }
            }
            None => {
                if self.settings.config.terminal.show {
                    self.toggle_term(area);
                }
            }
        }
    }

    /// Build the app from a saved session. `None` when the session does not fit the
    /// current config — more panes than `max_panes` allows — so the caller starts fresh.
    pub fn from_session(s: &Session, settings: Settings, outcome: Outcome) -> Option<Self> {
        if s.panes.pane_count() > settings.config.files.max_panes {
            return None;
        }
        let home = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut app = Self::new(&home, &home, settings, outcome);
        app.panes.clear();
        app.next_id = 0;
        app.tree = app.build_saved(&s.panes, &home);
        let files = app.file_panes();
        app.focus = files.get(s.focus).or(files.first()).copied().unwrap_or(0);
        app.term_height = s.terminal.height_pct.clamp(10, 90);
        app.restore_terminal = Some((s.terminal.clone(), s.terminal_focused));
        Some(app)
    }

    fn build_saved(&mut self, s: &Saved, fallback: &Path) -> Node {
        match s {
            Saved::Pane { active, tabs } => {
                let mut panes: Vec<FilePane> = tabs
                    .iter()
                    .map(|t| {
                        let key = match t.sort.as_str() {
                            "ext" => SortKey::Ext,
                            "size" => SortKey::Size,
                            "modified" => SortKey::Modified,
                            _ => SortKey::Name,
                        };
                        let sort = xdevcmd_core::Sort {
                            key,
                            reverse: t.reverse,
                            dirs_first: self.settings.sort.dirs_first,
                        };
                        let dir = session::resolve_dir(&t.path, fallback);
                        FilePane::new(&xdevcmd_core::policy::get().clamp(&dir), sort, t.hidden)
                    })
                    .collect();
                if panes.is_empty() {
                    let hidden = self.settings.config.files.show_hidden;
                    panes.push(FilePane::new(fallback, self.settings.sort, hidden));
                }
                let mut it = panes.into_iter();
                let mut ft = FileTabs::new(it.next().expect("at least one tab"));
                for p in it {
                    ft.push(p);
                }
                ft.select(*active);
                let id = self.alloc_id();
                self.panes.insert(id, Pane::Files(Box::new(ft)));
                Node::Leaf(id)
            }
            Saved::Split { axis, pct, a, b } => {
                let axis = if axis == "rows" {
                    Axis::Rows
                } else {
                    Axis::Cols
                };
                let a = self.build_saved(a, fallback);
                let b = self.build_saved(b, fallback);
                Node::split(axis, (*pct).clamp(10, 90), a, b)
            }
        }
    }

    /// What is open now, as a session to save.
    pub fn snapshot(&mut self) -> Session {
        let (files_tree, height) = match (&self.tree, self.term_shown) {
            (Node::Split { pct, a, .. }, true) => ((**a).clone(), 100 - *pct),
            _ => (self.tree.clone(), self.term_height),
        };
        let panes = self.save_node(&files_tree);
        let mut leaves = Vec::new();
        files_tree.leaves(&mut leaves);
        let focus_is_term = self.focus_is_term();
        let last_files = self
            .mru
            .iter()
            .copied()
            .find(|id| leaves.contains(id))
            .unwrap_or(self.focus);
        let focus_leaf = if focus_is_term {
            last_files
        } else {
            self.focus
        };
        let shown = self.term_shown;
        let tabs = self.panel().map(|p| p.tab_dirs()).unwrap_or_default();
        Session {
            version: session::VERSION,
            focus: leaves.iter().position(|id| *id == focus_leaf).unwrap_or(0),
            terminal_focused: focus_is_term,
            panes,
            terminal: SavedTerminal {
                shown,
                height_pct: height,
                tabs,
            },
        }
    }

    fn save_node(&self, n: &Node) -> Saved {
        match n {
            Node::Leaf(id) => {
                let Some(t) = self.panes.get(id).and_then(Pane::as_tabs) else {
                    return Saved::Pane {
                        active: 0,
                        tabs: Vec::new(),
                    };
                };
                let tabs = t
                    .tabs()
                    .iter()
                    .filter_map(|f| {
                        Some(SavedTab {
                            // A path that is not UTF-8 cannot go in TOML; that tab
                            // reopens at its pane's fallback instead.
                            path: f.cwd.to_str()?.to_string(),
                            sort: match f.sort.key {
                                SortKey::Name => "name",
                                SortKey::Ext => "ext",
                                SortKey::Size => "size",
                                SortKey::Modified => "modified",
                            }
                            .to_string(),
                            reverse: f.sort.reverse,
                            hidden: f.show_hidden,
                        })
                    })
                    .collect();
                Saved::Pane {
                    active: t.active_index(),
                    tabs,
                }
            }
            Node::Split { axis, pct, a, b } => Saved::Split {
                axis: if *axis == Axis::Rows { "rows" } else { "cols" }.to_string(),
                pct: *pct,
                a: Box::new(self.save_node(a)),
                b: Box::new(self.save_node(b)),
            },
        }
    }

    /// Write the session on the way out. A failure is not worth stopping a quit for,
    /// and there is no screen left to report it on.
    fn save_session(&mut self) {
        // Read-only means xdevcmd writes nothing, its own state included.
        if !self.settings.config.general.restore_session || self.settings.mode.read_only() {
            return;
        }
        if let Some(path) = session::default_path() {
            let s = self.snapshot();
            if session::save(&path, &s).is_ok() {
                self.saved = Some(s);
            }
        }
    }

    /// Save the session when it has changed, every `[general] autosave_secs`. Saving only on
    /// quit lost everything when the window was closed instead — the X of Windows
    /// Terminal, closing an SSH session — because then xdevcmd is killed, not asked.
    fn autosave(&mut self) {
        let every = self.settings.config.general.autosave_secs;
        if every == 0
            || !self.interactive
            || self.last_autosave.elapsed() < Duration::from_secs(every)
        {
            return;
        }
        self.last_autosave = Instant::now();
        let now = self.snapshot();
        if self.saved.as_ref() != Some(&now) {
            self.save_session();
        }
    }

    pub fn run(mut self, terminal: &mut Term) -> Result<()> {
        self.interactive = true;
        self.watcher = crate::watch::Watcher::new();
        let size = terminal.size()?;
        self.boot(Rect::new(0, 0, size.width, size.height));

        // Drawn only when something changed: input, a shell's output, background work
        // reporting in, a job's progress; or after HEARTBEAT, whatever happened.
        let mut dirty = true;
        let mut drawn = Instant::now();
        while !self.quit {
            self.pump_job();
            dirty |= self.reap_terminals();
            dirty |= self.background();
            dirty |= crate::pane::term::take_output();
            // A job's progress moves on its own, with no message to say so.
            dirty |= matches!(self.modal, Some(Modal::Job(_))) || !self.jobs.is_empty();
            if dirty || drawn.elapsed() >= HEARTBEAT {
                let t0 = Instant::now();
                terminal.draw(|f| self.draw(f))?;
                self.usage.frame(t0.elapsed());
                drawn = Instant::now();
                dirty = false;
            }
            if event::poll(TICK)? {
                // Any event, a resize or a focus change included, may change the screen.
                dirty = true;
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => self.on_key(k),
                    Event::Mouse(m) => self.on_mouse(m),
                    _ => {}
                }
            }
            if let Some(path) = self.pending_edit.take() {
                // Every way to the editor comes here — F4, Shift+F4 on a name that
                // exists, Open with… — so the root is checked here once. The list of
                // names for renaming many lives in xdevcmd's own temporary directory.
                let own = crate::tempfiles::dir().is_some_and(|d| path.starts_with(&d));
                if !own && !xdevcmd_core::policy::get().allows(&path) {
                    self.status = "that file is outside the root xdevcmd is confined to".into();
                    self.pending_batch = None;
                    continue;
                }
                self.edit_externally(terminal, &path)?;
                if let Some((files, list)) = self.pending_batch.take() {
                    self.finish_batch(files, &list);
                }
            }
        }
        self.save_session();
        if self.quit_cd {
            self.write_cwd_file();
        }
        Ok(())
    }

    /// The directory you were in, for the shell that started xdevcmd to cd to. The file
    /// is one the caller named, so it is written even in read-only mode — it is the
    /// caller's output, not xdevcmd's state. Raw bytes on Unix: a name that is not UTF-8
    /// is still a directory you can cd to.
    fn write_cwd_file(&self) {
        let Some(out) = &self.cwd_file else { return };
        let dir = self.last_files_cwd();
        #[cfg(unix)]
        let bytes = {
            use std::os::unix::ffi::OsStrExt;
            dir.as_os_str().as_bytes().to_vec()
        };
        #[cfg(not(unix))]
        let bytes = dir.to_string_lossy().into_owned().into_bytes();
        let _ = std::fs::write(out, bytes);
    }

    /// The directory of the file pane used last, even when the terminal has focus.
    fn last_files_cwd(&self) -> PathBuf {
        let files = self.file_panes();
        let id = std::iter::once(self.focus)
            .chain(self.mru.iter().copied())
            .find(|id| files.contains(id));
        id.and_then(|id| self.panes.get(&id))
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
            .unwrap_or_else(|| self.focused_cwd())
    }

    /// Work that happens between keys: directory sizes arriving, directories that
    /// changed on disk, and messages from programs started in the background. True when
    /// any of it changed what is on screen.
    fn background(&mut self) -> bool {
        let mut changed = false;
        // Not under an open dialog or a drag: a listing that shifted while you were
        // answering "delete these?" or dragging a row is the wrong moment to move it.
        let quiet = self.modal.is_none() && self.drag.is_none();
        let every = Duration::from_millis(self.settings.config.files.auto_refresh_ms);
        let shown: Vec<PaneId> = self.rects.iter().map(|(id, _)| *id).collect();
        self.watch_panes(&shown, every);
        for (id, p) in self.panes.iter_mut() {
            if let Some(t) = p.as_tabs_mut() {
                for f in t.tabs_mut() {
                    changed |= f.poll_sizes();
                    changed |= f.poll_git();
                    changed |= f.poll_opening();
                    let was = f.results.as_ref().is_some_and(|r| r.running());
                    changed |= f.poll_results();
                    if let Some(r) = f.results.as_ref().filter(|r| was && !r.running()) {
                        let n = f.entries.iter().filter(|e| !e.is_parent()).count();
                        let skipped = match r.unreadable {
                            0 => String::new(),
                            u => format!(", {u} unreadable"),
                        };
                        self.status = format!(
                            "{n} found \u{b7} {} looked at{skipped} \u{b7} Enter goes to a file, F3 views it",
                            r.scanned
                        );
                    }
                }
                if quiet && shown.contains(id) {
                    let f = t.active_mut();
                    if self.to_reload.remove(id) {
                        f.reload();
                        changed = true;
                    } else if self.to_check.remove(id) {
                        // Just watched: a change from before the watch began.
                        changed |= f.check_now();
                    } else {
                        // Watched: the system says when it changes, and polling is only
                        // the safety net for what sends no events (network drives).
                        let watched = self
                            .watcher
                            .as_ref()
                            .zip(self.canonical.get(&f.cwd))
                            .is_some_and(|(w, c)| w.watching(c));
                        changed |= f.poll_changes(if watched { every * 10 } else { every });
                    }
                }
            }
        }
        while let Ok(n) = self.notes.1.try_recv() {
            self.status = n;
            changed = true;
        }
        let modal_before = self.modal.is_some();
        self.pump_background_jobs();
        changed |= modal_before != self.modal.is_some();
        self.autosave();
        let viewed = self
            .pending_view
            .as_ref()
            .and_then(|(rx, ..)| rx.try_recv().ok());
        if let (Some(out), Some((_, tmp, name, _))) = (viewed, self.pending_view.take()) {
            self.show_extracted(out, tmp, name);
            changed = true;
        }
        // A count for the open confirmation, if it has come in.
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            let got = c.counting.as_ref().and_then(|(n, _)| n.rx.try_recv().ok());
            if let (Some((files, bytes)), Some((_, line))) = (got, c.counting.take()) {
                if let Some(l) = c.body.get_mut(line) {
                    *l = Self::count_label(files, bytes);
                }
                changed = true;
            }
        }
        changed |= self.poll_comparing();
        if let Some((p, sz, m, rx)) = &self.decoding {
            if let Ok(pic) = rx.try_recv() {
                self.quick_image = Some((p.clone(), *sz, *m, pic));
                self.decoding = None;
                changed = true;
            }
        }
        let viewing = matches!(self.modal, Some(Modal::Usage));
        self.usage.tick(viewing);
        let hashed = self
            .hashing
            .as_ref()
            .and_then(|(p, rx)| rx.try_recv().ok().map(|r| (p.clone(), r)));
        if let Some((path, result)) = hashed {
            self.hashing = None;
            self.show_checksum(&path, result);
            changed = true;
        }
        changed
    }

    // ── identity helpers ───────────────────────────────────────────────────────────

    fn alloc_id(&mut self) -> PaneId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn file_panes(&self) -> Vec<PaneId> {
        let mut leaves = Vec::new();
        self.tree.leaves(&mut leaves);
        leaves
            .into_iter()
            .filter(|id| self.panes.get(id).is_some_and(|p| !p.is_term()))
            .collect()
    }

    fn focused_tabs(&mut self) -> Option<&mut FileTabs> {
        self.panes.get_mut(&self.focus)?.as_tabs_mut()
    }

    fn focused_files(&mut self) -> Option<&mut FilePane> {
        self.panes.get_mut(&self.focus)?.as_files_mut()
    }

    fn focused_cwd(&self) -> PathBuf {
        self.panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
            .or_else(|| {
                self.file_panes()
                    .first()
                    .and_then(|id| self.panes.get(id))
                    .and_then(Pane::as_files)
                    .map(|f| f.cwd.clone())
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    fn focus_is_term(&self) -> bool {
        self.panes.get(&self.focus).is_some_and(Pane::is_term)
    }

    // ── layout actions ─────────────────────────────────────────────────────────────

    fn focus_next_file(&mut self) {
        let files = self.file_panes();
        if files.is_empty() {
            return;
        }
        let i = files.iter().position(|id| *id == self.focus);
        self.focus = match i {
            Some(i) => files[(i + 1) % files.len()],
            None => files[0],
        };
    }

    fn split_focused(&mut self, axis: Axis) {
        if self.focus_is_term() {
            if !self.terminal_room() {
                return;
            }
            let cwd = self.focused_cwd();
            if let Some(Pane::Term(p)) = self.panes.get_mut(&self.focus) {
                if let Err(e) = p.split(axis, &cwd) {
                    self.status = e.to_string();
                }
            }
            return;
        }
        let max = self.settings.config.files.max_panes;
        if self.file_panes().len() >= max {
            self.status =
                format!("at most {max} file panes \u{2014} [files] max_panes in config.toml");
            return;
        }
        let cwd = self.focused_cwd();
        let id = self.alloc_id();
        if self.tree.split_leaf(self.focus, axis, id) {
            let (sort, hidden) = (self.settings.sort, self.settings.config.files.show_hidden);
            self.panes
                .insert(id, Pane::files(FilePane::new(&cwd, sort, hidden)));
            self.focus = id;
        }
    }

    fn close_focused(&mut self) {
        if self.focus_is_term() {
            // Closing a terminal ends its shell. Hiding the panel is Alt+T.
            let empty = match self.panes.get_mut(&self.focus) {
                Some(Pane::Term(p)) => p.close_focused(),
                _ => false,
            };
            if empty {
                self.remove_terminal_panel();
            }
            return;
        }
        if self.file_panes().len() <= 1 {
            self.status = "that is the last file pane".into();
            return;
        }
        let gone = self.focus;
        if self.tree.remove_leaf(gone) {
            self.panes.remove(&gone);
            // Back to the pane used last, not whichever happens to be first in the tree.
            let files = self.file_panes();
            self.focus = self
                .mru
                .iter()
                .copied()
                .find(|id| files.contains(id))
                .or_else(|| files.first().copied())
                .unwrap_or(0);
        }
    }

    fn toggle_term(&mut self, area: Rect) {
        match (self.term_id, self.term_shown) {
            (Some(id), true) => {
                // Remember the height it was dragged to, so showing it again restores it.
                if let Node::Split { pct, .. } = &self.tree {
                    self.term_height = 100 - *pct;
                }
                self.tree.remove_leaf(id);
                self.term_shown = false;
                if self.focus == id {
                    self.focus = self.file_panes().first().copied().unwrap_or(0);
                }
            }
            (Some(id), false) => {
                let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                self.tree = Node::split(Axis::Rows, 100 - self.term_height, old, Node::Leaf(id));
                self.term_shown = true;
                self.focus = id;
            }
            (None, _) => {
                let cwd = self
                    .next_shell_cwd
                    .take()
                    .unwrap_or_else(|| self.focused_cwd());
                let pct = self.term_height;
                let rows = (area.height * pct / 100).saturating_sub(2).max(1);
                let cols = area.width.saturating_sub(2).max(1);
                let shell = self.settings.config.terminal.shell.clone();
                match TermPanel::new(&cwd, rows, cols, &shell) {
                    Ok(t) => {
                        let id = self.alloc_id();
                        self.panes.insert(id, Pane::Term(Box::new(t)));
                        let old = std::mem::replace(&mut self.tree, Node::Leaf(id));
                        self.tree =
                            Node::split(Axis::Rows, 100 - self.term_height, old, Node::Leaf(id));
                        self.term_id = Some(id);
                        self.term_shown = true;
                    }
                    Err(e) => self.status = format!("no shell: {e}"),
                }
            }
        }
    }

    /// The panel has no shells left: take it out of the layout entirely, so the next
    /// F12 starts a fresh one rather than showing an empty frame.
    fn remove_terminal_panel(&mut self) {
        if let Some(id) = self.term_id.take() {
            if self.term_shown {
                self.tree.remove_leaf(id);
            }
            self.panes.remove(&id);
            self.term_shown = false;
            if self.focus == id {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
            }
        }
    }

    fn panel(&mut self) -> Option<&mut TermPanel> {
        match self.panes.get_mut(&self.term_id?) {
            Some(Pane::Term(p)) => Some(p),
            _ => None,
        }
    }

    /// A new terminal tab, showing the panel if it was hidden and creating it if there
    /// was none — in which case the panel's first tab is the new terminal.
    fn new_terminal(&mut self) {
        let full = self.screen;
        match (self.term_id, self.term_shown) {
            (None, _) => self.toggle_term(full),
            (Some(_), shown) => {
                if !shown {
                    self.toggle_term(full);
                }
                if !self.terminal_room() {
                    // Still show and focus the panel: the request was "a terminal",
                    // and the ones already running are right there.
                    if let Some(id) = self.term_id {
                        self.focus = id;
                    }
                    return;
                }
                let cwd = self.focused_cwd();
                if let Some(Err(e)) = self.panel().map(|p| p.add_tab(&cwd)) {
                    self.status = format!("no shell: {e}");
                }
            }
        }
        if let Some(id) = self.term_id {
            self.focus = id;
        }
    }

    /// Whether one more shell fits under `[terminal] max_terminals`; says why if not.
    fn terminal_room(&mut self) -> bool {
        let max = self.settings.config.terminal.max_terminals;
        let running = self.panel().map_or(0, |p| p.shell_count());
        if running >= max {
            self.status =
                format!("at most {max} terminals \u{2014} [terminal] max_terminals in config.toml");
            return false;
        }
        true
    }

    fn cycle_terminal(&mut self, delta: isize) {
        let Some(id) = self.term_id else {
            self.status = "no terminal yet \u{2014} F12".into();
            return;
        };
        if !self.term_shown {
            let full = self.screen;
            self.toggle_term(full);
        }
        if let Some(p) = self.panel() {
            p.cycle_tab(delta);
        }
        self.focus = id;
    }

    fn next_split(&mut self) {
        if self.focus_is_term() {
            if let Some(p) = self.panel() {
                p.next_split();
            }
        } else {
            self.focus_next_file();
        }
    }

    /// Shells that have exited close themselves, as `exit` does in an editor.
    /// True when a shell went, so the screen changed.
    fn reap_terminals(&mut self) -> bool {
        let Some(p) = self.panel() else { return false };
        let (gone, empty) = p.reap();
        if gone > 0 {
            self.status = if gone == 1 {
                "shell exited".into()
            } else {
                format!("{gone} shells exited")
            };
        }
        if empty {
            self.remove_terminal_panel();
        }
        gone > 0 || empty
    }

    /// Jump between the file panes and the shell, the way Ctrl+` works in an editor.
    fn toggle_term_focus(&mut self, area: Rect) {
        match self.term_id {
            Some(id) if self.term_shown && self.focus != id => self.focus = id,
            Some(id) if self.term_shown => {
                self.focus = self.file_panes().first().copied().unwrap_or(0);
                let _ = id;
            }
            _ => {
                // toggle_term leaves focus alone because startup uses it too, but F12
                // means "take me to the shell": a new one that left you typing into
                // the file panes would be a trap.
                self.toggle_term(area);
                if let Some(id) = self.term_id {
                    self.focus = id;
                }
            }
        }
    }

    /// Send the active pane's directory to the shell. The one place the two halves of
    /// the program actually talk to each other, and the reason having both is worth it.
    fn cd_terminal_here(&mut self) {
        let cwd = self.focused_cwd();
        let Some(id) = self.term_id else {
            self.status = "no terminal yet — F12".into();
            return;
        };
        if let Some(Pane::Term(t)) = self.panes.get(&id) {
            let text = cwd.display().to_string();
            // Typed text goes to whatever owns the terminal. Into an editor it edits;
            // into a sudo prompt it becomes the password. Only a shell at its prompt
            // should receive a cd.
            if let Some(program) = t.busy_with() {
                self.status =
                    format!("{program} is running in the terminal \u{2014} not typed into it");
                return;
            }
            match crate::safety::cd_command(&t.shell_name(), &text) {
                Ok(line) => t.send(line.into_bytes()),
                Err(why) => {
                    self.status = why;
                    return;
                }
            }
            self.status = format!("shell: cd {}", cwd.display());
        }
    }

    // ── input ──────────────────────────────────────────────────────────────────────

    /// Feed a key as if it had been typed. Used by `--keys` so a dialog can be put on
    /// screen and asserted on without a terminal or a human.
    pub fn feed_key(&mut self, k: KeyEvent) {
        self.on_key(k);
    }

    /// Feed a mouse event, as `--keys=click:X:Y` does.
    pub fn feed_mouse(&mut self, m: MouseEvent) {
        self.on_mouse(m);
    }

    /// One turn of the event loop without input: lets a running job report in.
    pub fn tick(&mut self) {
        self.pump_job();
        self.reap_terminals();
        self.background();
    }

    /// On macOS, a character Option typed in a file pane, where it means nothing, says
    /// once how to make Option work as Alt.
    fn option_hint(&mut self, k: KeyEvent) {
        if !cfg!(target_os = "macos") || self.option_hinted || self.focus_is_term() {
            return;
        }
        if let Some(c) = crate::keyfix::typed_by_option(k)
            .filter(|_| self.settings.keymap.action_for(k).is_none())
        {
            self.option_hinted = true;
            self.status = crate::keyfix::option_hint(c);
        }
    }

    /// The status line while a group is open: its first key and the keys that follow.
    fn group_hint(&self, first: crate::keys::Chord) -> String {
        let keys: Vec<String> = self
            .settings
            .keymap
            .group(first)
            .iter()
            .map(|((c, m), _)| crate::keys::key_name(*c, *m))
            .collect();
        // A group whose keys all repeat is a mode, left with Enter or Esc; any other
        // is a key to pick, after which it closes by itself.
        if self.group_repeats(first) {
            format!("{} \u{b7} Enter or Esc: done", keys.join(" "))
        } else {
            format!("then: {} \u{b7} Esc: cancel", keys.join(" "))
        }
    }

    /// Whether every key in the group `first` opens repeats.
    fn group_repeats(&self, first: crate::keys::Chord) -> bool {
        let g = self.settings.keymap.group(first);
        !g.is_empty() && g.iter().all(|(_, a)| a.repeats())
    }

    /// The badge at the left of the status line while a group is open: `RESIZE` for a
    /// mode with a name, else the key that opened it, `ALT+R …`.
    fn group_badge(&self) -> Option<String> {
        let first = self.group?;
        let label = match self.settings.keymap.group_name(first) {
            Some(n) if self.group_repeats(first) => n.to_uppercase(),
            _ => format!(
                "{} \u{2026}",
                crate::keys::key_name(first.0, first.1).to_uppercase()
            ),
        };
        Some(format!(" {label} "))
    }

    /// A key of the open group: run it and keep the group open. `false` when the key is
    /// not one of the group's, which closes it.
    fn in_group(&mut self, k: KeyEvent) -> bool {
        let Some(first) = self.group.take() else {
            return false;
        };
        if matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
            self.status.clear();
            return true;
        }
        let Some(a) = self.settings.keymap.in_group(first, k) else {
            self.status.clear();
            return false;
        };
        if a.repeats() {
            self.group = Some(first);
            self.status = self.group_hint(first);
        } else {
            self.status.clear();
        }
        self.run_bound(a, k);
        // An action may have put up its own message; the group's comes back next key.
        true
    }

    /// Run `a`, which `k` was bound to.
    fn run_bound(&mut self, a: Action, k: KeyEvent) {
        if a == Action::GoTab {
            return self.go_tab(k);
        }
        self.perform(a);
    }

    /// Alt+1…9: the tab whose number is the key's digit, in the file pane or the
    /// terminal, whichever has focus.
    fn go_tab(&mut self, k: KeyEvent) {
        let Some(n) = (match k.code {
            KeyCode::Char(c) => c.to_digit(10).filter(|n| *n > 0),
            _ => None,
        }) else {
            self.status = "go_tab needs a key ending in a digit, 1 to 9".into();
            return;
        };
        let i = n as usize - 1;
        // The tabs of whatever has focus. A file pane with one tab shows no tab bar,
        // while the terminal's numbered tabs are on screen: say which were counted.
        let (count, whose) = if self.focus_is_term() {
            let c = self.panel().map_or(0, |p| p.tab_count());
            if let Some(p) = self.panel().filter(|_| i < c) {
                p.select_tab(i);
                return;
            }
            (c, "the terminal")
        } else {
            let c = self.focused_tabs().map_or(0, |t| t.len());
            if let Some(t) = self.focused_tabs().filter(|_| i < c) {
                t.select(i);
                return;
            }
            (c, "this file pane")
        };
        let has = if count == 1 {
            "one tab".to_string()
        } else {
            format!("{count} tabs")
        };
        self.status = if whose == "this file pane" && self.term_shown {
            format!("no tab {n}: {whose} has {has} \u{2014} F12 to the terminal, then Alt+{n} picks its tabs")
        } else {
            format!("no tab {n}: {whose} has {has}")
        };
    }

    fn on_key(&mut self, k: KeyEvent) {
        if self.modal.is_some() {
            return self.on_modal_key(k);
        }
        if k.code == KeyCode::Esc {
            if let Some(c) = self.comparing.take() {
                c.stop.store(true, std::sync::atomic::Ordering::Relaxed);
                self.status = "comparison stopped".into();
                return;
            }
        }
        if self.search.is_some() && self.on_search_key(k) {
            return;
        }
        if self.in_group(k) {
            return;
        }
        self.option_hint(k);
        // The first key of a group. In the shell only when everything in the group works
        // there (resizing does); otherwise the key is the shell's.
        if self.settings.keymap.starts_group(k) {
            let first = crate::keys::Keymap::chord(k);
            let all_global = self
                .settings
                .keymap
                .group(first)
                .iter()
                .all(|(_, a)| a.is_global());
            if !self.focus_is_term() || all_global {
                self.group = Some(first);
                self.status = self.group_hint(first);
                return;
            }
        }
        let action = self.settings.keymap.action_for(k);
        // Esc in a filtered pane shows everything again: a filter left on is files you
        // cannot see, so taking it off should be the most obvious key there is.
        if action.is_none() && k.code == KeyCode::Esc && !self.focus_is_term() {
            if self.focused_files().is_some_and(|f| f.stop_search()) {
                self.status = "search stopped".into();
                return;
            }
            if let Some((_, _, name, stop)) = self.pending_view.take() {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                self.status = format!("stopped extracting {name}");
                return;
            }
            if self.focused_files().is_some_and(|f| f.stop_opening()) {
                self.status = "stopped opening the archive".into();
                return;
            }
            if let Some(f) = self.focused_files().filter(|f| f.filter.is_some()) {
                f.set_filter(None);
                self.status = "filter off".into();
                return;
            }
        }

        // Layout and terminal actions work from anywhere, the shell pane included.
        // Everything else typed into the shell belongs to the shell.
        if let Some(a) = action.filter(|a| a.is_global()) {
            return self.run_bound(a, k);
        }
        if self.focus_is_term() {
            let ended = matches!(self.panes.get(&self.focus), Some(Pane::Term(t)) if t.focused_program_ended());
            if ended && matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
                let empty = self.panel().is_some_and(|p| p.close_focused());
                if empty {
                    self.remove_terminal_panel();
                }
                // Back to the files the program was started from.
                let files = self.file_panes();
                if let Some(id) = self
                    .mru
                    .iter()
                    .copied()
                    .find(|id| files.contains(id))
                    .or_else(|| files.first().copied())
                {
                    self.focus = id;
                }
                return;
            }
            if let Some(Pane::Term(t)) = self.panes.get(&self.focus) {
                t.send_key(k);
            }
            return;
        }
        if let Some(a) = action {
            self.run_bound(a, k);
        }
    }

    /// Why `a` is turned off in the mode in effect, or `None` when it is allowed. The
    /// one place that decides; the key bar, the context menu and every input path ask
    /// here, and the engine in xdevcmd-core refuses writes again behind it.
    fn refused(&self, a: Action) -> Option<String> {
        let mode = self.settings.mode;
        let what = match a {
            Action::Copy | Action::Move => "copying and moving",
            Action::Delete | Action::DeletePermanent => "deleting",
            Action::Rename | Action::BulkRename => "renaming",
            Action::NewFile | Action::CopyAs | Action::Symlink | Action::Pack | Action::Unpack => {
                "creating files"
            }
            Action::SyncDirs => "synchronising",
            Action::Permissions => "changing permissions",
            Action::MkDir => "creating directories",
            Action::Edit => "editing",
            Action::OpenWith => "opening files in other programs",
            Action::Undo => "undoing",
            _ if mode == Mode::Restricted
                && matches!(
                    a,
                    Action::FocusTerminal
                        | Action::ToggleTerminal
                        | Action::CdTerminalHere
                        | Action::NewTerminal
                        | Action::UserMenu
                ) =>
            {
                "the terminal"
            }
            _ => return None,
        };
        mode.read_only()
            .then(|| format!("{} mode \u{2014} {what} is turned off", mode.label()))
    }

    fn perform(&mut self, a: Action) {
        if let Some(why) = self.refused(a) {
            self.status = why;
            return;
        }
        // Inside an archive only reading and extracting make sense. Writing into one
        // would mean rewriting the whole file, which xdevcmd does not do.
        if self.focused_archive().is_some()
            && matches!(
                a,
                Action::Delete
                    | Action::DeletePermanent
                    | Action::Move
                    | Action::Rename
                    | Action::BulkRename
                    | Action::MkDir
                    | Action::Edit
                    | Action::NewFile
                    | Action::CopyAs
                    | Action::Symlink
                    | Action::Permissions
                    | Action::OpenWith
                    | Action::Checksum
                    | Action::Pack
                    | Action::Unpack
                    | Action::FindFiles
            )
        {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status =
                format!("inside an archive you can view and extract ({k}) \u{2014} not that");
            return;
        }
        let full = self.screen;
        match a {
            Action::Quit => {
                if self.ask_before_quitting() {
                    return;
                }
                self.quit = true;
                self.quit_cd = true;
            }
            Action::QuitNoCd => {
                if self.ask_before_quitting() {
                    return;
                }
                self.quit = true;
                self.quit_cd = false;
            }
            Action::QuickSearch => self.begin_search(false),
            Action::Filter => self.begin_search(true),
            Action::GoToPath => self.begin_goto(),
            Action::Back => {
                if !self.focused_files().is_some_and(|f| f.go_back()) {
                    self.status = "nothing to go back to".into();
                }
            }
            Action::Forward => {
                if !self.focused_files().is_some_and(|f| f.go_forward()) {
                    self.status = "nothing to go forward to".into();
                }
            }
            Action::History => self.open_history(),
            Action::Hotlist => self.open_hotlist(),
            Action::OpenWith => self.open_with(),
            Action::SwapPanes => self.swap_panes(),
            Action::MirrorPane => self.mirror_pane(),
            Action::MarkPattern => self.begin_pattern(true),
            Action::UnmarkPattern => self.begin_pattern(false),
            Action::BulkRename => {
                let files = self.selection_in_view();
                if files.is_empty() {
                    self.status = "nothing selected".into();
                } else {
                    self.modal = Some(Modal::Rename(Box::new(RenameDialog::new(files))));
                }
            }
            Action::NewFile => {
                let dir = self.focused_cwd();
                self.modal = Some(Modal::Input(Input {
                    title: "New file".into(),
                    label: format!("in {} \u{b7} opens in the editor", dir.display()),
                    edit: LineEdit::new(""),
                    kind: InputKind::NewFile,
                    dir,
                    buttons: Vec::new(),
                    error: None,
                }));
            }
            Action::CopyAs => self.begin_copy_as(),
            Action::Symlink => self.make_symlink(),
            Action::Permissions => self.begin_permissions(),
            Action::Checksum => self.begin_checksum(),
            Action::UserMenu => self.open_user_menu(),
            Action::Pack => self.begin_pack(),
            Action::MenuBar => self.open_bar(0),
            Action::Usage => self.modal = Some(Modal::Usage),
            Action::CompareDirs => self.compare_panes(),
            Action::SyncDirs => self.begin_sync(),
            Action::Unpack => self.begin_unpack(),
            Action::QuickView => {
                if self.quick_view.take().is_some() {
                    self.status = "quick view off".into();
                } else if self.focus_is_term() {
                } else if let Some(other) = self.other_file_pane() {
                    self.quick_view = Some((other, None));
                    self.status = "quick view \u{b7} the other pane follows the cursor".into();
                } else {
                    self.status = "quick view needs a second pane".into();
                }
            }
            Action::FindFiles => {
                if let Some(f) = self.focused_files() {
                    let d = FindDialog::new(f.cwd.clone(), f.show_hidden);
                    self.modal = Some(Modal::Find(Box::new(d)));
                }
            }
            Action::CalcSizes => {
                if let Some(f) = self.focused_files() {
                    let n = f.request_all_sizes();
                    self.status = match n {
                        0 => "no directories here".into(),
                        1 => "working out the size of 1 directory".into(),
                        n => format!("working out the size of {n} directories"),
                    };
                }
            }
            Action::Help => {
                let help = Help::new(&self.settings, &self.config_outcome);
                self.modal = Some(Modal::Help(Box::new(help)));
            }
            Action::Refresh => self.with_files(|f| f.reload()),
            Action::ToggleHidden => {
                if let Some(f) = self.focused_files() {
                    f.toggle_hidden();
                    let on = f.show_hidden;
                    self.status = format!("hidden files {}", if on { "shown" } else { "hidden" });
                }
            }
            Action::FocusNextPane => self.focus_next_file(),
            Action::FocusTerminal => self.toggle_term_focus(full),
            Action::ToggleTerminal => self.toggle_term(full),
            Action::CdTerminalHere => self.cd_terminal_here(),
            Action::SplitRight => self.split_focused(Axis::Cols),
            Action::SplitDown => self.split_focused(Axis::Rows),
            Action::ClosePane => self.close_focused(),
            Action::ResizeLeft | Action::ResizeRight | Action::ResizeUp | Action::ResizeDown => {
                let (axis, d) = match a {
                    Action::ResizeLeft => (Axis::Cols, -5),
                    Action::ResizeRight => (Axis::Cols, 5),
                    Action::ResizeUp => (Axis::Rows, -5),
                    _ => (Axis::Rows, 5),
                };
                // A border between shells in the terminal tab first, then the layout's.
                let inside =
                    self.focus_is_term() && self.panel().is_some_and(|p| p.move_boundary(axis, d));
                if !inside && !self.tree.move_boundary(self.focus, axis, d) {
                    self.status = "no border that way".into();
                }
            }
            Action::Zoom => self.toggle_zoom(),
            Action::NewTerminal => self.new_terminal(),
            Action::NewTab => {
                if let Some(t) = self.focused_tabs() {
                    t.duplicate();
                }
            }
            Action::CloseTab => {
                let closed = self.focused_tabs().map(|t| {
                    let i = t.active_index();
                    t.close(i)
                });
                if closed == Some(false) {
                    self.status =
                        "that is the pane's only tab \u{2014} Alt+W closes the pane".into();
                }
            }
            // One pair of keys for tabs, acting on whatever has focus.
            Action::NextTab | Action::PrevTab => {
                let d = if a == Action::NextTab { 1 } else { -1 };
                if self.focus_is_term() {
                    self.cycle_terminal(d);
                } else if let Some(t) = self.focused_tabs() {
                    t.cycle(d);
                }
            }
            Action::NextSplit => self.next_split(),
            // Only from a key, which carries the tab's number: see `run_bound`.
            Action::GoTab => {}
            Action::CursorUp => self.with_files(|f| f.move_cursor(-1)),
            Action::CursorDown => self.with_files(|f| f.move_cursor(1)),
            Action::PageUp => self.with_files(|f| f.page(-1)),
            Action::PageDown => self.with_files(|f| f.page(1)),
            Action::Top => self.with_files(|f| f.go_to(0)),
            Action::Bottom => self.with_files(|f| f.go_end()),
            Action::Parent => self.with_files(|f| f.go_up()),
            Action::GoHome => {
                let home = crate::bookmarks::expand_tilde("~");
                if home.is_absolute() {
                    self.with_files(|f| f.set_cwd(&home));
                } else {
                    self.status = "no home directory: neither HOME nor USERPROFILE is set".into();
                }
            }
            Action::GoRoot => self.with_files(|f| {
                if let Some(root) = f.cwd.ancestors().last().map(Path::to_path_buf) {
                    f.set_cwd(&root);
                }
            }),
            Action::EnterDir => {
                // Only ever into a directory: Enter already views files, and a key that
                // sometimes navigates and sometimes opens a viewer is one you stop
                // trusting.
                let is_dir = self
                    .panes
                    .get(&self.focus)
                    .and_then(Pane::as_files)
                    .and_then(|f| f.current())
                    .is_some_and(|e| e.is_dir() && !e.is_parent());
                if is_dir {
                    self.with_files(|f| {
                        f.activate();
                    });
                } else {
                    self.status = "not a directory".into();
                }
            }
            Action::Open => {
                // Entering a directory navigates; on a file, Enter opens it (or views
                // it, with `[files] enter = "view"`).
                if self.focused_files().and_then(|f| f.activate()).is_some() {
                    self.enter_file();
                }
            }
            Action::OpenWithMenu => self.open_with_menu(),
            Action::Undo => self.ask_undo(),
            // Space on a directory also works out its size, as in Total Commander: the
            // moment you select a directory is the moment you want to know.
            Action::Mark => self.with_files(|f| {
                f.toggle_mark();
                if let Some(e) = f.current().filter(|e| e.is_dir() && !e.is_parent()) {
                    if f.marked.contains(&e.path) {
                        let p = e.path.clone();
                        f.request_size(&p);
                    }
                }
            }),
            Action::MarkAndNext => self.with_files(|f| {
                f.toggle_mark();
                f.move_cursor(1);
            }),
            Action::InvertMarks => self.with_files(|f| f.invert_marks()),
            Action::MarkAll => self.with_files(|f| f.mark_all(true)),
            Action::UnmarkAll => self.with_files(|f| f.mark_all(false)),
            Action::View => self.begin_view(),
            Action::Edit => self.begin_edit(),
            Action::Copy => self.begin_transfer(ops::Kind::Copy),
            Action::Move => self.begin_transfer(ops::Kind::Move),
            Action::MkDir => self.begin_mkdir(),
            Action::Rename => self.begin_rename(),
            Action::Delete => {
                let permanent = !self.settings.config.general.delete_to_trash;
                self.begin_delete(permanent);
            }
            Action::DeletePermanent => self.begin_delete(true),
            Action::SortName => self.with_files(|f| f.set_sort(SortKey::Name)),
            Action::SortExt => self.with_files(|f| f.set_sort(SortKey::Ext)),
            Action::SortSize => self.with_files(|f| f.set_sort(SortKey::Size)),
            Action::SortModified => self.with_files(|f| f.set_sort(SortKey::Modified)),
        }
    }

    fn with_files(&mut self, f: impl FnOnce(&mut FilePane)) {
        if let Some(p) = self.focused_files() {
            f(p);
        }
    }

    fn pane_at(&self, x: u16, y: u16) -> Option<PaneId> {
        self.rects
            .iter()
            .find(|(_, r)| r.contains(Position::new(x, y)))
            .map(|(id, _)| *id)
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        // The mouse closes an open key group: arrows typed after a click are arrows.
        if self.group.take().is_some() {
            self.status.clear();
        }
        if self.modal.is_some() {
            return self.on_modal_mouse(m);
        }
        // A click anywhere ends a search, keeping whatever it found.
        if matches!(m.kind, MouseEventKind::Down(_)) {
            self.search = None;
        }
        let (x, y) = (m.column, m.row);
        let p = Position::new(x, y);

        match m.kind {
            MouseEventKind::Drag(MouseButton::Left) => return self.on_drag(x, y),
            MouseEventKind::Up(MouseButton::Left) => return self.on_release(x, y),
            _ => {}
        }

        if let MouseEventKind::Down(MouseButton::Left) = m.kind {
            if let Some(i) = self
                .bar_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(i, _)| *i)
            {
                return self.open_bar(i);
            }
            // The key bar is a row of buttons, as in every orthodox manager.
            if let Some(a) = self
                .footer_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(a, _)| *a)
            {
                return self.perform(a);
            }
            // A scrollbar sits on a pane's right border, which is also half of the
            // boundary beside it; where there is a scrollbar, it wins. The boundary can
            // still be dragged by the other pane's border, or above and below the bar.
            if let Some(id) = self.pane_at(x, y) {
                let grab = self
                    .panes
                    .get(&id)
                    .and_then(Pane::as_files)
                    .and_then(|f| f.bar_at(x, y));
                if let Some(grab) = grab {
                    self.focus = id;
                    if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                        f.drag_bar(y, grab);
                    }
                    self.scrolling = Some((id, grab));
                    return;
                }
            }
            // A boundary is grabbed before either pane it separates: it sits on their
            // borders, which are otherwise just part of each pane. The exception is a
            // button on the terminal's tab bar, which shares a row with that strip.
            let on_tab_button = self.panes.values().any(|p| match p {
                Pane::Term(t) => self.term_shown && t.bar_button_at(x, y),
                Pane::Files(f) => f.bar_button_at(x, y),
            });
            let mut dividers = Vec::new();
            // Zoomed, the layout's borders are not on screen, so none can be grabbed.
            if !on_tab_button && self.zoom.is_none() {
                self.tree.dividers(self.main, &mut dividers);
            }
            // A double-click on a boundary shares it evenly between the two panes on
            // either side, and only those two; a single press starts a drag.
            if let Some(d) = dividers.into_iter().find(|d| d.hit.contains(p)) {
                if self.register_click(x, y) {
                    self.tree.even(d.id);
                    self.status = "shared evenly between the two panes".into();
                } else {
                    self.resizing = Some((d, false));
                }
                return;
            }
            let inner = self
                .term_shown
                .then(|| self.panel().map(|p| p.dividers()))
                .flatten()
                .unwrap_or_default();
            if let Some(d) = inner.into_iter().find(|d| d.hit.contains(p)) {
                if self.register_click(x, y) {
                    if let Some(panel) = self.panel() {
                        panel.even(d.id);
                    }
                    self.status = "shared evenly between the two shells".into();
                } else {
                    self.resizing = Some((d, true));
                }
                return;
            }
        }

        let Some(id) = self.pane_at(x, y) else { return };
        let ctrl = m.modifiers.contains(KeyModifiers::CONTROL);
        let shift = m.modifiers.contains(KeyModifiers::SHIFT);

        match m.kind {
            MouseEventKind::Down(MouseButton::Left) if Some(id) == self.term_id => {
                self.focus = id;
                let click = self.panel().map(|p| p.on_click(x, y));
                match click {
                    Some(TermClick::NewTab) => self.new_terminal(),
                    Some(TermClick::CloseTab(i)) => {
                        // Closing has a side effect, so it stays out of a match guard.
                        let nothing_left = self.panel().is_some_and(|p| p.close_tab(i));
                        if nothing_left {
                            self.remove_terminal_panel();
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if Some(id) == self.term_id => {
                let d = if m.kind == MouseEventKind::ScrollUp {
                    3
                } else {
                    -3
                };
                if let Some(p) = self.panel() {
                    p.scroll_at(x, y, d);
                }
            }
            MouseEventKind::Down(MouseButton::Left)
                if self
                    .panes
                    .get(&id)
                    .and_then(Pane::as_tabs)
                    .is_some_and(|t| t.bar_button_at(x, y)) =>
            {
                self.focus = id;
                let Some(tabs) = self.focused_tabs() else {
                    return;
                };
                match tabs.on_click(x, y) {
                    TabClick::New => tabs.duplicate(),
                    TabClick::Close(i) => {
                        tabs.close(i);
                    }
                    TabClick::Selected | TabClick::Nothing => {}
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = id;
                let double = self.register_click(x, y);
                let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) else {
                    return;
                };
                // A directory named in the title's path: go there, as Alt+G would.
                if let Some(dir) = f.crumb_at(x, y) {
                    f.set_cwd(&dir);
                    return;
                }
                if let Some(key) = f.header_at(x, y) {
                    f.set_sort(key);
                    return;
                }
                let Some(i) = f.row_at(x, y) else { return };
                if ctrl {
                    f.go_to(i);
                    f.toggle_mark();
                    return;
                }
                if shift {
                    // Most terminals keep Shift+click for their own text selection, so
                    // this often never arrives. When it does, it does the obvious thing.
                    let from = f.cursor;
                    f.mark_range(from, i);
                    f.go_to(i);
                    return;
                }
                f.go_to(i);
                if double {
                    // What Enter does: into a directory, or open the file.
                    if f.activate().is_some() {
                        self.enter_file();
                    }
                    return;
                }
                let anchor_was_marked =
                    f.entries.get(i).is_some_and(|e| f.marked.contains(&e.path));
                let marks_before = f.marked.clone();
                self.drag = Some(Drag {
                    source: id,
                    anchor: i,
                    anchor_was_marked,
                    marks_before,
                    mode: DragMode::Pending,
                    hover: None,
                });
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.focus = id;
                let row = self
                    .panes
                    .get_mut(&id)
                    .and_then(Pane::as_files_mut)
                    .map(|f| {
                        let r = f.row_at(x, y);
                        if let Some(i) = r {
                            f.go_to(i);
                        }
                        r
                    });
                if let Some(row) = row {
                    self.open_context_menu(row, x, y);
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                    f.scroll(-3);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                    f.scroll(3);
                }
            }
            _ => {}
        }
    }

    fn on_drag(&mut self, x: u16, y: u16) {
        if let Some((id, grab)) = self.scrolling {
            if let Some(f) = self.panes.get_mut(&id).and_then(Pane::as_files_mut) {
                f.drag_bar(y, grab);
            }
            return;
        }
        if let Some((d, in_panel)) = self.resizing {
            let pct = d.pct_at(x, y);
            if in_panel {
                if let Some(p) = self.panel() {
                    p.set_pct(d.id, pct);
                }
            } else {
                self.tree.set_pct(d.id, pct);
            }
            // Both sides, because one number is ambiguous: dragging the terminal's top
            // edge, "45%" read as the terminal's share when it was the panes'.
            self.status = match d.axis {
                Axis::Cols => format!("left {pct}% \u{b7} right {}%", 100 - pct),
                Axis::Rows => format!("top {pct}% \u{b7} bottom {}%", 100 - pct),
            };
            return;
        }
        let Some(mut d) = self.drag.take() else {
            return;
        };
        let over = self.pane_at(x, y);
        let into_other =
            over.filter(|t| *t != d.source && self.panes.get(t).is_some_and(|p| !p.is_term()));

        if let Some(target) = into_other {
            // Crossing into another pane makes it a drag-and-drop. If a range had been
            // drawn from an unmarked row, that was really the start of carrying one
            // file: put the marks back how they were.
            if d.mode == DragMode::Range && !d.anchor_was_marked {
                if let Some(f) = self.panes.get_mut(&d.source).and_then(Pane::as_files_mut) {
                    f.marked = d.marks_before.clone();
                }
            }
            d.mode = DragMode::Files;
            d.hover = Some(target);
            let n = self.drag_payload(&d).len();
            self.status = format!(
                "\u{21e2} {n} item{} \u{2014} release to copy or move",
                if n == 1 { "" } else { "s" }
            );
        } else if over == Some(d.source) && d.mode != DragMode::Files && !d.anchor_was_marked {
            if let Some(f) = self.panes.get_mut(&d.source).and_then(Pane::as_files_mut) {
                let r = f.rows_rect;
                let last = f.entries.len().saturating_sub(1);
                // Dragging past the top or bottom edge scrolls, so a range can be longer
                // than the pane is tall.
                let row = if y < r.y {
                    f.scroll(-1);
                    f.offset
                } else if y >= r.y + r.height {
                    f.scroll(1);
                    (f.offset + r.height as usize).saturating_sub(1)
                } else {
                    f.offset + (y - r.y) as usize
                }
                .min(last);
                f.marked = d.marks_before.clone();
                f.mark_range(d.anchor, row);
                f.go_to(row);
                d.mode = DragMode::Range;
            }
        } else {
            d.hover = None;
        }
        self.drag = Some(d);
    }

    /// What a drag-and-drop carries: the selection if a marked row was grabbed,
    /// otherwise the one file that was.
    fn drag_payload(&self, d: &Drag) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&d.source).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if d.anchor_was_marked {
            let mut v: Vec<PathBuf> = f
                .entries
                .iter()
                .filter(|e| f.marked.contains(&e.path))
                .map(|e| e.path.clone())
                .collect();
            v.sort();
            v
        } else {
            f.entries
                .get(d.anchor)
                .filter(|e| !e.is_parent())
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        }
    }

    fn on_release(&mut self, x: u16, y: u16) {
        if self.scrolling.take().is_some() {
            return;
        }
        if self.resizing.take().is_some() {
            return;
        }
        let Some(d) = self.drag.take() else { return };
        if d.mode != DragMode::Files {
            return;
        }
        let Some(target) = d.hover else {
            self.status = "drop cancelled".into();
            return;
        };
        let sources = self.drag_payload(&d);
        let Some(dest) = self
            .panes
            .get(&target)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
        else {
            return;
        };
        if sources.is_empty() {
            self.status = "nothing to drop".into();
            return;
        }
        if let Some(why) = self.refused(Action::Copy) {
            self.status = why;
            return;
        }
        let from_archive = self
            .panes
            .get(&d.source)
            .and_then(Pane::as_files)
            .is_some_and(|f| f.archive.is_some());
        let into_virtual = self
            .panes
            .get(&target)
            .and_then(Pane::as_files)
            .is_some_and(|f| f.archive.is_some() || f.results.is_some());
        if from_archive || into_virtual {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status =
                format!("archives and results do not take drops \u{2014} {k} extracts or copies");
            return;
        }
        let src_dir = self
            .panes
            .get(&d.source)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone());
        if src_dir.as_ref() == Some(&dest) {
            self.status = "that pane shows the same directory \u{2014} nothing to copy".into();
            return;
        }
        let what = Self::describe(&sources);
        let name = dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| dest.display().to_string());
        // Copy or move is asked, never guessed from a modifier the terminal may not
        // even pass through. The answer is the confirmation; overwrites still ask.
        let items = vec![
            Item::cmd(
                format!("Copy {what} here"),
                "",
                Cmd::Transfer {
                    kind: ops::Kind::Copy,
                    sources: sources.clone(),
                    dest: dest.clone(),
                },
                true,
            ),
            Item::cmd(
                format!("Move {what} here"),
                "",
                Cmd::Transfer {
                    kind: ops::Kind::Move,
                    sources,
                    dest,
                },
                true,
            ),
            Item::sep(),
            Item::cancel("Cancel", "Esc"),
        ];
        self.status = format!("drop into {name}?");
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Drop into {name}")),
            items,
            (x, y),
        )));
    }

    fn open_context_menu(&mut self, row: Option<usize>, x: u16, y: u16) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let entry = row
            .and_then(|i| f.entries.get(i))
            .filter(|e| !e.is_parent())
            .cloned();
        let any_marks = !f.marked.is_empty();
        let hidden = f.show_hidden;
        let entry_marked = entry.as_ref().is_some_and(|e| f.marked.contains(&e.path));
        let title = match &entry {
            Some(e) => e.name.clone(),
            None => f
                .cwd
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| f.cwd.display().to_string()),
        };
        let sel = self.selection();
        let has_sel = !sel.is_empty();
        let other = self.other_pane_cwd().is_some();
        let n = sel.len();
        // Hints come from the keymap, so the menu teaches the keys you actually have.
        let k = |a| self.footer_key(a).unwrap_or_default();
        let many = if n > 1 {
            format!(" {n} items")
        } else {
            String::new()
        };

        let mut items = Vec::new();
        if let Some(e) = &entry {
            if e.is_dir() {
                items.push(Item::act("Open", k(Action::Open), Action::Open, true));
            } else {
                items.push(Item::act("View", k(Action::View), Action::View, true));
                items.push(Item::act("Edit", k(Action::Edit), Action::Edit, true));
                items.push(Item::act(
                    "Open with system app",
                    k(Action::OpenWith),
                    Action::OpenWith,
                    true,
                ));
                items.push(Item::act(
                    "Open with\u{2026}",
                    k(Action::OpenWithMenu),
                    Action::OpenWithMenu,
                    true,
                ));
                items.push(Item::act(
                    "Checksum (SHA-256)",
                    k(Action::Checksum),
                    Action::Checksum,
                    true,
                ));
            }
            items.push(Item::act(
                "Copy as\u{2026}",
                k(Action::CopyAs),
                Action::CopyAs,
                true,
            ));
            items.push(Item::act(
                "Link in other pane",
                k(Action::Symlink),
                Action::Symlink,
                other,
            ));
            items.push(Item::act(
                "Permissions\u{2026}",
                k(Action::Permissions),
                Action::Permissions,
                true,
            ));
            items.push(Item::act("Rename", k(Action::Rename), Action::Rename, true));
            items.push(Item::sep());
        }
        items.push(Item::act(
            format!("Copy{many} to other pane"),
            k(Action::Copy),
            Action::Copy,
            has_sel && other,
        ));
        items.push(Item::act(
            format!("Move{many} to other pane"),
            k(Action::Move),
            Action::Move,
            has_sel && other,
        ));
        items.push(Item::act(
            format!("Delete{many}"),
            k(Action::Delete),
            Action::Delete,
            has_sel,
        ));
        items.push(Item::act(
            "Delete permanently",
            k(Action::DeletePermanent),
            Action::DeletePermanent,
            has_sel,
        ));
        items.push(Item::sep());
        if entry.is_some() {
            let label = if entry_marked { "Unmark" } else { "Mark" };
            items.push(Item::act(label, k(Action::Mark), Action::Mark, true));
        }
        items.push(Item::act(
            "Mark all",
            k(Action::MarkAll),
            Action::MarkAll,
            true,
        ));
        items.push(Item::act(
            "Clear marks",
            k(Action::UnmarkAll),
            Action::UnmarkAll,
            any_marks,
        ));
        items.push(Item::sep());
        items.push(Item::act(
            "New directory",
            k(Action::MkDir),
            Action::MkDir,
            true,
        ));
        items.push(Item::act(
            "New tab",
            k(Action::NewTab),
            Action::NewTab,
            true,
        ));
        items.push(Item::cmd("Copy path", "", Cmd::CopyPath, true));
        items.push(Item::act(
            "Terminal here",
            k(Action::CdTerminalHere),
            Action::CdTerminalHere,
            self.term_id.is_some(),
        ));
        items.push(Item::act(
            "Refresh",
            k(Action::Refresh),
            Action::Refresh,
            true,
        ));
        let hide = if hidden {
            "Hide hidden files"
        } else {
            "Show hidden files"
        };
        items.push(Item::act(
            hide,
            k(Action::ToggleHidden),
            Action::ToggleHidden,
            true,
        ));

        let restricted = self.settings.mode == Mode::Restricted;
        for it in &mut items {
            match &it.cmd {
                Some(Cmd::Act(a)) if self.refused(*a).is_some() => it.enabled = false,
                Some(Cmd::CopyPath) if restricted => it.enabled = false,
                _ => {}
            }
        }
        self.modal = Some(Modal::Menu(Menu::new(Some(title), items, (x, y))));
    }

    fn perform_cmd(&mut self, c: Cmd) {
        match c {
            Cmd::Act(a) => self.perform(a),
            Cmd::CopyPath if self.settings.mode == Mode::Restricted => {
                self.status = "restricted mode \u{2014} the clipboard is turned off".into();
            }
            Cmd::Transfer { .. } if self.settings.mode.read_only() => {
                self.status = format!(
                    "{} mode \u{2014} copying and moving is turned off",
                    self.settings.mode.label()
                );
            }
            Cmd::AddBookmark(_)
            | Cmd::RemoveBookmark(_)
            | Cmd::OpenFile(_)
            | Cmd::BatchRename(_)
            | Cmd::Sync(_)
                if self.settings.mode.read_only() =>
            {
                self.status = format!(
                    "{} mode \u{2014} that is turned off",
                    self.settings.mode.label()
                );
            }
            Cmd::CopyPath => self.copy_path(),
            Cmd::GoTo(p) => {
                if p.is_dir() {
                    self.with_files(|f| f.set_cwd(&p));
                } else {
                    self.status = format!("{} is not there any more", tidy(&p));
                }
            }
            Cmd::AddBookmark(p) => {
                if !self.bookmarks.contains(&p) {
                    self.bookmarks.push(p.clone());
                }
                self.save_bookmarks(format!("bookmarked {}", tidy(&p)));
            }
            Cmd::RemoveBookmark(p) => {
                self.bookmarks.retain(|b| *b != p);
                self.save_bookmarks(format!("removed the bookmark for {}", tidy(&p)));
            }
            // Checked again here: a link can be pointed elsewhere between the menu that
            // offered this and the choice.
            Cmd::OpenFile(p) if !xdevcmd_core::policy::get().allows(&p) => {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Cmd::OpenFile(p) => self.launch(&p),
            Cmd::RunInTerminal { argv, .. }
                if argv
                    .first()
                    .is_some_and(|p| !xdevcmd_core::policy::get().allows(Path::new(p))) =>
            {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Cmd::RunInTerminal { argv, cwd } => self.run_in_terminal(argv, cwd),
            Cmd::ThemeMenu => self.theme_menu(),
            Cmd::Settings => self.open_settings(),
            Cmd::Undo => self.undo(),
            Cmd::SetTheme(name) => self.set_theme(&name),
            Cmd::ApplyRule(i, p) => self.apply_rule(i, p),
            Cmd::UserCommand(i) => self.run_user_command(i),
            Cmd::About => {
                self.modal = Some(Modal::Ask(modal::Ask {
                    title: format!("xdevcmd {}", env!("CARGO_PKG_VERSION")),
                    // All from [workspace.package] in Cargo.toml: one place to change.
                    body: vec![
                        env!("CARGO_PKG_DESCRIPTION").into(),
                        String::new(),
                        format!("{} licence", env!("CARGO_PKG_LICENSE")),
                        env!("CARGO_PKG_REPOSITORY").into(),
                        String::new(),
                        "No network, no telemetry.".into(),
                    ],
                    danger: false,
                    yes: None,
                    buttons: Vec::new(),
                    footnote: Some(format!(
                        "Developed by {}",
                        env!("CARGO_PKG_AUTHORS").replace(':', ", ")
                    )),
                    pictures: {
                        let bg = match theme::t().bg_panel {
                            ratatui::style::Color::Rgb(r, g, b) => [r, g, b],
                            _ => [0, 0, 0],
                        };
                        xdevcmd_core::image::png_bytes(LOGO, 14, 14, bg)
                            .into_iter()
                            .collect()
                    },
                }));
            }
            Cmd::QuitAnyway => {
                // Cancelled, and waited for: a worker stops between writes and removes
                // its half-written file, which is what makes quitting now safe.
                for j in &self.jobs {
                    j.handle.cancel();
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while !self.jobs.is_empty() && Instant::now() < deadline {
                    for j in &mut self.jobs {
                        j.pump();
                        if let Some(q) = j.question.take() {
                            let _ = q;
                            j.answer(xdevcmd_core::Answer::Abort);
                        }
                    }
                    self.jobs.retain(|j| j.done.is_none());
                    std::thread::sleep(Duration::from_millis(10));
                }
                self.quit = true;
                self.quit_cd = true;
            }
            Cmd::Sync(pairs) => {
                let req = Request::sync(pairs);
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot("a synchronisation".into()));
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            Cmd::CopyText(t) => self.copy_text(t),
            Cmd::BatchRename(moves) => {
                let out = xdevcmd_core::batch_rename::apply(&moves);
                self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(moves.clone()));
                self.reload_all();
                self.status = match (out.renamed, out.errors.first()) {
                    (n, None) => format!("renamed {n}"),
                    (n, Some(e)) => {
                        format!("renamed {n}, {} failed \u{2014} {e}", out.errors.len())
                    }
                };
            }
            Cmd::Transfer {
                kind,
                sources,
                dest,
            } => {
                let request = match kind {
                    ops::Kind::Move => Request::mv(sources, dest),
                    _ => Request::copy(sources, dest),
                };
                self.remember_job(&request);
                self.modal = Some(Modal::Job(Job::new(kind, ops::start(request))));
            }
        }
    }

    /// Put text on the clipboard with OSC 52, unless the mode forbids it or the text
    /// could act when pasted.
    fn copy_text(&mut self, t: String) {
        if self.settings.mode == Mode::Restricted {
            // Short: it also has to fit beside the viewer's key hints.
            self.status = "restricted mode: no clipboard".into();
            return;
        }
        if let Some(why) = crate::safety::unsafe_to_paste(&t) {
            self.status = why;
            return;
        }
        if self.interactive {
            let seq = format!("\x1b]52;c;{}\x07", base64(t.as_bytes()));
            let mut out = stdout();
            let _ = out.write_all(seq.as_bytes());
            let _ = out.flush();
        }
        let lines = t.lines().count().max(1);
        self.status = match lines {
            1 => "copied 1 line to the clipboard".into(),
            n => format!("copied {n} lines to the clipboard"),
        };
    }

    /// Put the selection's paths on the clipboard with OSC 52.
    ///
    /// The terminal does the copying, so it lands on the clipboard of the machine you
    /// are sitting at — including across SSH, which nothing running on the server could
    /// do any other way. Whether it works is the terminal's call: Windows Terminal,
    /// iTerm2, kitty and WezTerm allow it; tmux needs `set-clipboard on`.
    fn copy_path(&mut self) {
        let mut paths = self.selection();
        if paths.is_empty() {
            paths.push(self.focused_cwd());
        }
        let text = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        // Pasted into a terminal, a control character in a path runs commands just as
        // typing it would. Refuse rather than strip: a stripped path is a wrong path.
        if let Some(why) = paths
            .iter()
            .find_map(|p| crate::safety::unsafe_to_type(&p.display().to_string()))
        {
            self.status = why;
            return;
        }
        if self.interactive {
            let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
            let mut out = stdout();
            let _ = out.write_all(seq.as_bytes());
            let _ = out.flush();
        }
        self.status = match paths.as_slice() {
            [one] => format!("sent {} to the clipboard", one.display()),
            many => format!("sent {} paths to the clipboard", many.len()),
        };
    }

    /// What Settings' answer to a key or a click means here: a theme to preview, the
    /// theme put back on cancel, or the settings to save.
    fn settings_outcome(&self, o: settings_dialog::Outcome) -> ModalAct {
        let (spec, depth) = (&self.settings.config.theme, self.settings.depth);
        let show = |name: &str| {
            if let Ok(t) = crate::config::theme_for(name, spec, depth) {
                theme::set(t);
            }
        };
        match o {
            settings_dialog::Outcome::Stay => ModalAct::None,
            settings_dialog::Outcome::Preview(name) => {
                show(&name);
                ModalAct::None
            }
            settings_dialog::Outcome::Cancel => {
                // Put back the theme the preview replaced.
                show(&self.theme_name);
                ModalAct::Close
            }
            settings_dialog::Outcome::Save => ModalAct::SaveSettings,
        }
    }

    fn on_modal_mouse(&mut self, m: MouseEvent) {
        let p = Position::new(m.column, m.row);
        let left = matches!(m.kind, MouseEventKind::Down(MouseButton::Left));
        let hit = |rects: &[Rect], i: usize| rects.get(i).is_some_and(|r| r.contains(p));
        let mut act = ModalAct::None;
        // A click on a menu-bar title switches menus, whatever menu is open.
        if left && matches!(self.modal, Some(Modal::Menu(_)) | Some(Modal::Usage)) {
            if let Some(i) = self
                .bar_hits
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(i, _)| *i)
            {
                return self.open_bar(i);
            }
        }
        if left && matches!(self.modal, Some(Modal::Usage)) {
            return self.modal_act(ModalAct::Close);
        }

        match self.modal.as_mut() {
            Some(Modal::Menu(menu)) => match menu.on_mouse(m) {
                MenuOutcome::Stay => {}
                MenuOutcome::Close => act = ModalAct::Close,
                MenuOutcome::Run(c) => act = ModalAct::Run(c),
                MenuOutcome::Side(d) => act = ModalAct::Bar(d),
            },
            Some(Modal::Confirm(c)) if left => {
                if hit(&c.buttons, 0) {
                    act = ModalAct::Start;
                } else if hit(&c.buttons, 1) {
                    act = ModalAct::Close;
                } else if hit(&c.buttons, 2) {
                    c.next_destination();
                }
            }
            Some(Modal::Input(i)) if left => {
                if hit(&i.buttons, 0) {
                    act = ModalAct::Apply;
                } else if hit(&i.buttons, 1) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Job(j)) if left => {
                if j.question.is_some() {
                    let answer = j
                        .answer_buttons
                        .iter()
                        .find(|(_, r)| r.contains(p))
                        .map(|(a, _)| *a);
                    if let Some(a) = answer {
                        j.answer(a);
                    }
                } else if hit(&j.buttons, 0) {
                    if j.done.is_some() {
                        act = ModalAct::Finish;
                    } else {
                        j.handle.cancel();
                    }
                } else if hit(&j.buttons, 1) && j.done.is_none() {
                    act = ModalAct::Background;
                }
            }
            Some(Modal::View(v)) => v.on_mouse(m),
            Some(Modal::Rename(d)) if left => {
                act = if hit(&d.buttons, 0) {
                    match d.confirm() {
                        rename_dialog::Outcome::Rename(m) => ModalAct::Run(Cmd::BatchRename(m)),
                        _ => ModalAct::None,
                    }
                } else if hit(&d.buttons, 1) {
                    match d.to_editor() {
                        rename_dialog::Outcome::Editor(t) => ModalAct::BatchEditor(t),
                        _ => ModalAct::None,
                    }
                } else if hit(&d.buttons, 2) {
                    d.next_field(false);
                    ModalAct::None
                } else if hit(&d.buttons, 3) {
                    ModalAct::Close
                } else {
                    ModalAct::None
                };
            }
            Some(Modal::Find(d)) if left => {
                if hit(&d.buttons, 0) {
                    act = ModalAct::StartFind;
                } else if hit(&d.buttons, 1) {
                    d.field = 1 - d.field;
                } else if hit(&d.buttons, 2) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Ask(a)) if left => {
                if hit(&a.buttons, 0) {
                    act = match a.yes.take() {
                        Some((_, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    };
                } else if hit(&a.buttons, 1) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Help(h)) => {
                if matches!(h.on_mouse(m), viewer::Action::Close) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Settings(d)) => {
                let o = d.on_mouse(m);
                act = self.settings_outcome(o);
            }
            _ => {}
        }
        self.modal_act(act);
    }

    fn register_click(&mut self, x: u16, y: u16) -> bool {
        let now = Instant::now();
        let double = matches!(
            self.last_click,
            Some((t, px, py)) if px == x && py == y && now.duration_since(t) < DOUBLE_CLICK
        );
        self.last_click = if double { None } else { Some((now, x, y)) };
        double
    }

    // ── file operations ────────────────────────────────────────────────────────────

    /// What the keys act on: everything marked, or the row under the cursor.
    /// The selection in the order the pane shows it, which is what a counter counts.
    fn selection_in_view(&self) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if f.marked.is_empty() {
            return self.selection();
        }
        f.entries
            .iter()
            .filter(|e| f.marked.contains(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }

    fn selection(&self) -> Vec<PathBuf> {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return Vec::new();
        };
        if f.marked.is_empty() {
            return f
                .current()
                .filter(|e| !e.is_parent())
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default();
        }
        let mut v: Vec<PathBuf> = f
            .entries
            .iter()
            .filter(|e| f.marked.contains(&e.path))
            .map(|e| e.path.clone())
            .collect();
        v.sort();
        v
    }

    fn toggle_zoom(&mut self) {
        if self.zoom.take().is_some() {
            self.status.clear();
        } else {
            self.zoom = Some(self.focus);
            let key = self.footer_key(Action::Zoom).unwrap_or_default();
            self.status = format!("zoomed \u{b7} {key} to put it back");
        }
    }

    fn note_focus(&mut self) {
        if self.mru.first() != Some(&self.focus) {
            let f = self.focus;
            self.mru.retain(|id| *id != f);
            self.mru.insert(0, f);
            self.mru.truncate(64);
        }
    }

    /// Where a copy or move from the focused pane could go, best first: the other file
    /// panes by how recently they had focus, then any never focused, one entry per
    /// directory, and a directory equal to the source's own last. With two panes this
    /// is just "the other pane", as it always was.
    fn transfer_targets(&self) -> Vec<PathBuf> {
        let src = self.focused_cwd();
        let mut panes: Vec<PaneId> = self
            .file_panes()
            .into_iter()
            .filter(|id| *id != self.focus)
            .collect();
        let rank = |id: &PaneId| self.mru.iter().position(|m| m == id).unwrap_or(usize::MAX);
        panes.sort_by_key(rank);
        let mut dirs: Vec<PathBuf> = Vec::new();
        for id in panes {
            // A pane browsing an archive or showing search results is not a place
            // files can be copied into.
            let into = self
                .panes
                .get(&id)
                .and_then(Pane::as_files)
                .filter(|f| f.archive.is_none() && f.results.is_none());
            if let Some(f) = into {
                if !dirs.contains(&f.cwd) {
                    dirs.push(f.cwd.clone());
                }
            }
        }
        // Stable sort: recency order is kept within each group.
        dirs.sort_by_key(|d| *d == src);
        dirs
    }

    fn other_pane_cwd(&self) -> Option<PathBuf> {
        self.transfer_targets().into_iter().next()
    }

    /// "1 file", "3 files" — the singular matters because these strings sit in a
    /// confirmation for something irreversible, and sloppy copy reads as untrustworthy.
    fn count_label(files: u64, bytes: u64) -> String {
        let noun = if files == 1 { "file" } else { "files" };
        if bytes < 1024 {
            format!("{files} {noun} \u{2022} {bytes} bytes")
        } else {
            format!("{files} {noun} \u{2022} {}", human_size(bytes))
        }
    }

    fn describe(sources: &[PathBuf]) -> String {
        match sources {
            [one] => one
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| one.display().to_string()),
            many => format!("{} items", many.len()),
        }
    }

    fn begin_transfer(&mut self, kind: ops::Kind) {
        let sources = self.selection();
        if sources.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let choices = self.transfer_targets();
        if let Some((file, prefix)) = self
            .focused_archive()
            .map(|a| (a.file.clone(), a.prefix.clone()))
        {
            let inner = self
                .focused_archive()
                .map(|a| a.inner(&sources))
                .unwrap_or_default();
            let Some(dest) = choices.first().cloned() else {
                self.status = "extracting goes into another pane \u{2014} Alt+S splits one".into();
                return;
            };
            self.confirm_extract(file, inner, prefix, dest, choices);
            return;
        }
        let Some(dest) = choices.first().cloned() else {
            self.status = "no second pane to copy to — Alt+S splits one".into();
            return;
        };
        let request = match kind {
            ops::Kind::Copy => Request::copy(sources.clone(), dest.clone()),
            _ => Request::mv(sources.clone(), dest.clone()),
        };
        // Overwrites still raise a question mid-job; this only skips the "are you sure"
        // before starting, which is all the setting claims to do.
        if !self.settings.config.general.confirm_transfer {
            self.remember_job(&request);
            self.modal = Some(Modal::Job(Job::new(kind, ops::start(request))));
            return;
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: kind.verb().to_string(),
            body: vec![
                Self::describe(&sources),
                String::new(),
                format!("to    {}", dest.display()),
                COUNTING.into(),
            ],
            danger: false,
            request,
            buttons: Vec::new(),
            choices,
            choice: 0,
            source_dir: Some(self.focused_cwd()),
            counting: Some((ops::count(sources), 3)),
            permanent: false,
            // set_destination writes the line, after the destination's own lines.
            existing_line: Some(0),
        }));
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            c.set_destination(0);
        }
    }

    fn begin_delete(&mut self, permanent: bool) {
        let targets = self.selection();
        if targets.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let mut body = vec![Self::describe(&targets), String::new(), COUNTING.into()];
        let counting = Some((ops::count(targets.clone()), 2));
        if permanent {
            // What goes, by name: the first few, so a wrong selection shows.
            if targets.len() > 1 {
                body.push(String::new());
                for t in targets.iter().take(3) {
                    let name = t.file_name().map(|n| n.to_string_lossy().into_owned());
                    body.push(format!(
                        "  {}",
                        name.unwrap_or_else(|| t.display().to_string())
                    ));
                }
                if targets.len() > 3 {
                    body.push(format!("  \u{2026} and {} more", targets.len() - 3));
                }
            }
            body.push(String::new());
            body.push("Deleted permanently. This cannot be undone.".into());
        } else {
            body.push("Moved to the trash.".into());
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: if permanent {
                "Delete permanently".into()
            } else {
                "Delete".to_string()
            },
            body,
            danger: true,
            request: Request::delete(targets, !permanent),
            buttons: Vec::new(),
            choices: Vec::new(),
            choice: 0,
            source_dir: None,
            counting,
            permanent,
            existing_line: None,
        }));
    }

    fn begin_mkdir(&mut self) {
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: "Create directory".into(),
            label: format!("in {}", dir.display()),
            edit: LineEdit::new(""),
            kind: InputKind::MkDir,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    fn begin_rename(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()) else {
            self.status = "nothing to rename here".into();
            return;
        };
        self.modal = Some(Modal::Input(Input {
            title: "Rename".into(),
            label: format!("in {}", f.cwd.display()),
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::Rename {
                from: e.path.clone(),
            },
            dir: f.cwd.clone(),
            buttons: Vec::new(),
            error: None,
        }));
    }

    fn apply_input(&mut self, i: Input) {
        let text = i.edit.text();
        match &i.kind {
            InputKind::MkDir => {
                let name = text.trim();
                if name.is_empty() {
                    self.status = "no name given".into();
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                let first = xdevcmd_core::undo::first_missing(&target);
                // Under --root: no `..` (create_dir_all would follow it out), and the
                // directory it is made in must be inside, links resolved.
                let climbs = Path::new(name)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir));
                let inside = first
                    .parent()
                    .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
                if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
                    self.status = "outside the root xdevcmd is confined to".into();
                    return;
                }
                let existed = target.exists();
                match std::fs::create_dir_all(&target) {
                    Ok(()) => {
                        if !existed {
                            self.last_undo = Some(xdevcmd_core::undo::Undo::MadeDir(first));
                        }
                        self.status = format!("created {name}");
                        self.reload_all();
                    }
                    Err(e) => self.status = format!("mkdir failed: {e}"),
                }
            }
            InputKind::GoTo => {
                let typed = text.trim();
                if typed.is_empty() {
                    return;
                }
                let mut p = expand_tilde(typed);
                if p.is_relative() {
                    p = i.dir.join(p);
                }
                match std::fs::metadata(&p) {
                    Ok(m) if m.is_dir() => self.with_files(|f| f.set_cwd(&p)),
                    Ok(_) => {
                        let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
                            return;
                        };
                        let name = name.to_os_string();
                        self.with_files(|f| {
                            f.set_cwd(parent);
                            let target = f.cwd.join(&name);
                            f.select_path(&target);
                        });
                    }
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e.to_string()),
                            ..i
                        }));
                    }
                }
            }
            InputKind::NewFile => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::validate_name(name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                // An existing file is opened, not truncated: Shift+F4 on a name that
                // is there already means "edit it".
                let made = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target);
                match made {
                    Ok(_) => {
                        self.last_undo = Some(xdevcmd_core::undo::Undo::MadeFile(target.clone()));
                        self.status = format!("created {name}");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if target.is_dir() {
                            self.status = format!("{name} is a directory");
                            return;
                        }
                    }
                    Err(e) => {
                        self.status = format!("cannot create {name}: {e}");
                        return;
                    }
                }
                self.reload_all();
                self.with_files(|f| f.select_path(&target));
                if xdevcmd_core::policy::get().allows(&target) {
                    self.pending_edit = Some(target);
                } else {
                    // A name that exists as a link out of the root.
                    self.status = "that file is outside the root xdevcmd is confined to".into();
                }
            }
            InputKind::CopyAs { from } => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::validate_name(name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let mut req = Request::copy(vec![from.clone()], i.dir.clone());
                req.rename_to = Some(name.into());
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            InputKind::Permissions { targets } => {
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot(
                    "a change of permissions".into(),
                ));
                let setting = match xdevcmd_core::attrs::parse(&text) {
                    Ok(s) => s,
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e),
                            ..i
                        }));
                        return;
                    }
                };
                let mut done = 0;
                let mut first_err = None;
                for t in targets {
                    match xdevcmd_core::attrs::apply(t, setting) {
                        Ok(()) => done += 1,
                        Err(e) => {
                            first_err.get_or_insert_with(|| {
                                format!(
                                    "{}: {e}",
                                    t.file_name().unwrap_or_default().to_string_lossy()
                                )
                            });
                        }
                    }
                }
                self.reload_all();
                self.status = match first_err {
                    None => format!("permissions set on {done}"),
                    Some(e) => format!("set on {done}, not on the rest \u{2014} {e}"),
                };
            }
            InputKind::Pack { sources } => {
                let typed = text.trim();
                if typed.is_empty() {
                    return;
                }
                let mut dest = expand_tilde(typed);
                if dest.is_relative() {
                    dest = i.dir.join(dest);
                }
                // Under --root, as F7: no `..`, and the directory it goes in must be
                // inside, links resolved.
                let climbs = Path::new(typed)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir));
                let inside = dest
                    .parent()
                    .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
                if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
                    self.status = "outside the root xdevcmd is confined to".into();
                    return;
                }
                if xdevcmd_core::archive::format_of(&dest).is_none() {
                    self.modal = Some(Modal::Input(Input {
                        error: Some("the name must end in .zip, .tar, .tar.gz or .tgz".into()),
                        ..i
                    }));
                    return;
                }
                let req = Request::pack(sources.clone(), dest);
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Pack, ops::start(req))));
            }
            InputKind::Pattern { mark } => {
                let mark = *mark;
                let pattern = xdevcmd_core::glob::Pattern::new(&text);
                if pattern.is_empty() {
                    self.status = "no pattern given".into();
                    return;
                }
                let n = self
                    .focused_files()
                    .map_or(0, |f| f.mark_matching(&pattern, mark));
                let verb = if mark { "marked" } else { "unmarked" };
                self.status = match n {
                    0 => format!("nothing matches {}", text.trim()),
                    n => format!("{verb} {n} matching {}", text.trim()),
                };
            }
            InputKind::Rename { from } => match xdevcmd_core::rename::rename(from, &text) {
                Ok(to) => {
                    if to != *from {
                        self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(vec![(
                            from.clone(),
                            to.clone(),
                        )]));
                    }
                    let old = from.file_name().map(|s| s.to_string_lossy().into_owned());
                    self.status = format!("renamed {} \u{2192} {text}", old.unwrap_or_default());
                    self.reload_all();
                    for p in self.panes.values_mut() {
                        if let Some(f) = p.as_files_mut() {
                            f.select_path(&to);
                        }
                    }
                }
                Err(e) => {
                    // Back on screen with the reason, and the text as typed.
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                }
            },
        }
    }

    /// The file the cursor is on, or nothing when it is a directory or `..`.
    fn current_file(&self) -> Option<PathBuf> {
        let f = self.panes.get(&self.focus).and_then(Pane::as_files)?;
        let e = f.current()?;
        (!e.is_dir()).then(|| e.path.clone())
    }

    fn begin_view(&mut self) {
        if self.focused_archive().is_some() {
            return self.view_in_archive();
        }
        let Some(path) = self.current_file() else {
            self.status = "F3 views a file, not a directory".into();
            return;
        };
        // A link inside the root can point anywhere; what it points at is what F3 reads.
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        match Document::open(&path) {
            Ok(doc) => {
                let v = &self.settings.config.viewer;
                let viewer = Viewer::open(doc, v.wrap, v.tab_width);
                self.modal = Some(Modal::View(Box::new(viewer)));
            }
            Err(e) => self.status = format!("cannot view {}: {e}", path.display()),
        }
    }

    fn begin_edit(&mut self) {
        match self.current_file() {
            Some(p) if !xdevcmd_core::policy::get().allows(&p) => {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Some(p) => self.pending_edit = Some(p),
            None => self.status = "F4 edits a file, not a directory".into(),
        }
    }

    /// Hand the real terminal to `$EDITOR` and take it back afterwards.
    ///
    /// Running the editor inside the shell pane was the other option, and it is worse:
    /// the pane is a third of the screen, and an editor that small is not an editor.
    /// Suspending is what every orthodox manager does, and the editor gets the terminal
    /// it expects — full size, its own alternate screen, its own mouse handling.
    fn edit_externally(&mut self, terminal: &mut Term, path: &Path) -> Result<()> {
        let configured = self.settings.config.commands.editor.trim().to_string();
        let spec = Some(configured)
            .filter(|s| !s.is_empty())
            .ok_or(())
            .or_else(|_| std::env::var("VISUAL"))
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| {
                // Something that certainly exists, rather than a failure to launch.
                if cfg!(windows) { "notepad" } else { "vi" }.to_string()
            });
        // EDITOR may carry arguments, as in `code -w` or `emacsclient -nw`.
        let mut parts = spec.split_whitespace();
        let Some(program) = parts.next() else {
            self.status = "EDITOR is set to an empty string".into();
            return Ok(());
        };
        let args: Vec<&str> = parts.collect();

        // Step out of the alternate screen by hand and step back into it, rather than
        // calling ratatui::restore() and ratatui::init() again. Re-initialising asks
        // the terminal for its cursor position and dies when the answer is slow —
        // "The cursor position could not be read within a normal duration" — which is
        // exactly what happens over a laggy SSH link or inside a multiplexer. Keeping
        // the same Terminal avoids the question entirely.
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();

        let outcome = std::process::Command::new(program)
            .args(&args)
            .arg(path)
            .status();

        let _ = enable_raw_mode();
        let _ = execute!(stdout(), EnterAlternateScreen, EnableMouseCapture);
        // Not `terminal.clear()`: in ratatui 0.30 that snapshots the cursor position
        // first, and the query is what times out on a slow link — the very failure we
        // just moved away from. Clear the screen through the backend instead, then
        // reset BOTH diff buffers so the next draw repaints every cell. One swap only
        // resets one of them, and the diff against the stale frame comes out empty.
        let _ = terminal.backend_mut().clear_region(ClearType::All);
        // The editor left the cursor wherever it liked.
        terminal.backend_mut().forget();
        terminal.swap_buffers();
        terminal.swap_buffers();

        self.status = match outcome {
            Ok(s) if s.success() => format!("{program} closed"),
            Ok(s) => format!("{program} exited with {s}"),
            Err(e) => format!("cannot run {program}: {e}"),
        };
        self.reload_all();
        Ok(())
    }

    fn reload_all(&mut self) {
        // Every tab, not only those on show: a tab you switch to after a move must not
        // still list the files that left it.
        for p in self.panes.values_mut() {
            if let Some(t) = p.as_tabs_mut() {
                t.reload_all();
            }
        }
    }

    /// Drain the worker's events each tick, and close the box by itself on a clean run.
    fn pump_job(&mut self) {
        let mut finished = false;
        if let Some(Modal::Job(j)) = self.modal.as_mut() {
            j.pump();
            if let Some(s) = &j.done {
                finished = s.errors.is_empty() && !s.cancelled;
            }
        }
        if finished {
            self.finish_job();
        }
    }

    fn finish_job(&mut self) {
        if let Some(Modal::Job(j)) = self.modal.take() {
            if let Some(s) = &j.done {
                self.status = s.describe(j.kind);
                self.note_trashed(s);
            }
        }
        // Marks are cleared once an operation has run, the way every orthodox manager
        // does it. Leaving them set means the next F8 silently includes the files from
        // the last F5 — which is how you delete something you never selected.
        for p in self.panes.values_mut() {
            if let Some(f) = p.as_files_mut() {
                f.marked.clear();
            }
        }
        self.reload_all();
    }

    fn on_modal_key(&mut self, k: KeyEvent) {
        let mut act = ModalAct::None;

        match self.modal.as_mut() {
            Some(Modal::Menu(menu)) => match menu.on_key(k) {
                MenuOutcome::Stay => {}
                MenuOutcome::Close => act = ModalAct::Close,
                MenuOutcome::Run(c) => act = ModalAct::Run(c),
                MenuOutcome::Side(d) => act = ModalAct::Bar(d),
            },
            Some(Modal::Usage) => {
                if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Confirm(c)) => match k.code {
                KeyCode::Tab => c.next_destination(),
                // What cannot be undone is never one Enter away: Enter is what a hand
                // presses out of habit. Y, or a click on the button, says it.
                KeyCode::Enter if c.permanent => {
                    self.status = "press Y to delete permanently, Esc to cancel".into();
                }
                KeyCode::Char('u' | 'U') if c.existing_line.is_some() => c.next_existing(),
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => act = ModalAct::Start,
                KeyCode::Esc | KeyCode::Char('n' | 'N') => act = ModalAct::Close,
                _ => {}
            },
            Some(Modal::Input(i)) => match k.code {
                KeyCode::Esc => act = ModalAct::Close,
                KeyCode::Enter => act = ModalAct::Apply,
                KeyCode::Tab if i.kind == InputKind::GoTo => complete_path(i),
                _ => {
                    if i.edit.on_key(k) {
                        i.error = None;
                    }
                }
            },
            Some(Modal::View(v)) => match v.on_key(k) {
                viewer::Action::Close => act = ModalAct::Close,
                viewer::Action::Copy(text) => {
                    // Not Run(…): that closes the viewer first, and copying should not.
                    self.copy_text(text);
                    let said = self.status.clone();
                    if let Some(Modal::View(v)) = self.modal.as_mut() {
                        v.note(&said);
                    }
                    return;
                }
                viewer::Action::Stay => {}
            },
            Some(Modal::Help(h)) => {
                if matches!(h.on_key(k), viewer::Action::Close) {
                    act = ModalAct::Close;
                }
            }
            Some(Modal::Settings(d)) => {
                let o = d.on_key(k);
                act = self.settings_outcome(o);
            }
            Some(Modal::Rename(d)) => match d.on_key(k) {
                rename_dialog::Outcome::Stay => {}
                rename_dialog::Outcome::Close => act = ModalAct::Close,
                rename_dialog::Outcome::Rename(m) => act = ModalAct::Run(Cmd::BatchRename(m)),
                rename_dialog::Outcome::Editor(t) => act = ModalAct::BatchEditor(t),
            },
            Some(Modal::Find(d)) => match d.on_key(k) {
                find_dialog::Outcome::Stay => {}
                find_dialog::Outcome::Close => act = ModalAct::Close,
                find_dialog::Outcome::Start => act = ModalAct::StartFind,
            },
            Some(Modal::Ask(a)) => match k.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    act = match a.yes.take() {
                        Some((_, c)) => ModalAct::Run(c),
                        None => ModalAct::Close,
                    }
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => act = ModalAct::Close,
                _ => {}
            },
            Some(Modal::Job(j)) => {
                if let Some(q) = j.question.take() {
                    match modal::answer_for(&q, k.code) {
                        Some(a) => j.answer(a),
                        None => j.question = Some(q),
                    }
                } else if j.done.is_some() {
                    if matches!(k.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ')) {
                        act = ModalAct::Finish;
                    }
                } else if k.code == KeyCode::Esc {
                    j.handle.cancel();
                } else if matches!(k.code, KeyCode::Char('b' | 'B')) {
                    act = ModalAct::Background;
                }
            }
            None => {}
        }

        self.modal_act(act);
    }

    fn modal_act(&mut self, act: ModalAct) {
        match act {
            ModalAct::Run(c) => {
                self.modal = None;
                self.perform_cmd(c);
            }
            ModalAct::Close => {
                self.modal = None;
                if let Some(t) = self.view_temp.take() {
                    let _ = std::fs::remove_dir_all(&t);
                }
            }
            ModalAct::Start => {
                if let Some(Modal::Confirm(c)) = self.modal.take() {
                    let kind = c.request.kind;
                    self.remember_job(&c.request);
                    self.modal = Some(Modal::Job(Job::new(kind, ops::start(c.request))));
                }
            }
            ModalAct::Apply => {
                if let Some(Modal::Input(i)) = self.modal.take() {
                    self.apply_input(i);
                }
            }
            ModalAct::Finish => self.finish_job(),
            ModalAct::Bar(d) => {
                let at = match &self.modal {
                    Some(Modal::Menu(m)) => m.bar.unwrap_or(0),
                    _ => 0,
                };
                let n = crate::app::BAR.len() as isize;
                self.open_bar((at as isize + d).rem_euclid(n) as usize);
            }
            ModalAct::Background => {
                if let Some(Modal::Job(j)) = self.modal.take() {
                    self.jobs.push(j);
                    // The job has its list of files; marks now would only be carried
                    // into the next operation by mistake, as finish_job knows.
                    for p in self.panes.values_mut() {
                        if let Some(f) = p.as_files_mut() {
                            f.marked.clear();
                        }
                    }
                    self.status =
                        "running in the background \u{2014} it comes back if it has a question"
                            .into();
                }
            }
            ModalAct::SaveSettings => self.save_settings(),
            ModalAct::BatchEditor(text) => {
                if let Some(Modal::Rename(d)) = self.modal.take() {
                    self.begin_batch(d.files, &text);
                }
            }
            ModalAct::StartFind => {
                if let Some(Modal::Find(d)) = self.modal.take() {
                    let sort = self.settings.sort;
                    let tab = FilePane::results(d.query(), d.what(), sort);
                    if let Some(t) = self.focused_tabs() {
                        t.push(tab);
                        let last = t.len() - 1;
                        t.select(last);
                    }
                    self.status = "searching \u{2014} Esc stops it, Ctrl+W closes the tab".into();
                }
            }
            ModalAct::None => {}
        }
    }

    // ── search, navigation, bookmarks ──────────────────────────────────────────────

    fn begin_search(&mut self, filter: bool) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let before = f.filter.clone();
        self.search = Some(Search::new(self.focus, filter, before));
        self.status = if filter {
            "type to filter \u{b7} Enter keeps it \u{b7} Esc puts it back".into()
        } else {
            "type to jump \u{b7} \u{2191}\u{2193} next match \u{b7} Enter or Esc to stop".into()
        };
    }

    /// A key while the search line is open. False means the line did not want it: the
    /// search has ended and the key goes on to do what it normally does.
    fn on_search_key(&mut self, k: KeyEvent) -> bool {
        let Some(s) = self.search.as_mut() else {
            return false;
        };
        let id = s.pane;
        let f = self.panes.get_mut(&id).and_then(Pane::as_files_mut);
        let Some(f) = f else {
            self.search = None;
            return false;
        };
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc => {
                if s.filter {
                    f.set_filter(s.before.take());
                }
                self.search = None;
                self.status.clear();
                return true;
            }
            KeyCode::Enter => {
                self.search = None;
                self.status.clear();
                return true;
            }
            KeyCode::Up | KeyCode::Down => {
                let forward = k.code == KeyCode::Down;
                if s.filter {
                    f.move_cursor(if forward { 1 } else { -1 });
                } else if let Some(i) = search::step(&f.entries, &s.text(), f.cursor, forward) {
                    f.go_to(i);
                }
                return true;
            }
            // Ctrl+A and Ctrl+U are the line's own (select all, delete to the start);
            // any other chord is a command.
            _ if alt || (ctrl && !matches!(k.code, KeyCode::Char('a' | 'u'))) => {}
            _ => {
                if s.edit.on_key(k) {
                    let text = s.text();
                    if s.filter {
                        f.set_filter(Some(text.clone()));
                        s.found = text.is_empty() || f.entries.iter().any(|e| !e.is_parent());
                    } else {
                        match search::first(&f.entries, &text) {
                            Some(i) => {
                                f.go_to(i);
                                s.found = true;
                            }
                            None => s.found = text.is_empty(),
                        }
                    }
                    return true;
                }
            }
        }
        self.search = None;
        self.status.clear();
        false
    }

    fn begin_goto(&mut self) {
        let dir = self.focused_cwd();
        let mut start = dir.display().to_string();
        if !start.ends_with(std::path::MAIN_SEPARATOR) {
            start.push(std::path::MAIN_SEPARATOR);
        }
        self.modal = Some(Modal::Input(Input {
            title: "Go to".into(),
            label: "a directory, or a file to land on \u{b7} ~ is home \u{b7} Tab completes".into(),
            edit: LineEdit::new(&start),
            kind: InputKind::GoTo,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    fn begin_pattern(&mut self, mark: bool) {
        if self.focused_files().is_none() {
            return;
        }
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: if mark { "Mark" } else { "Unmark" }.into(),
            label: "names matching \u{b7} *.jpg *.png \u{b7} a trailing / for directories".into(),
            // Selected, so typing replaces it and Enter alone means everything.
            edit: LineEdit::for_rename("*", true),
            kind: InputKind::Pattern { mark },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Where a menu that belongs to the focused pane opens: just inside its corner.
    fn pane_anchor(&self) -> (u16, u16) {
        self.rects
            .iter()
            .find(|(id, _)| *id == self.focus)
            .map(|(_, r)| (r.x + 2, r.y + 1))
            .unwrap_or((2, 1))
    }

    fn open_history(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let list = f.history();
        if list.is_empty() {
            self.status = "no history in this tab yet".into();
            return;
        }
        let items = list
            .iter()
            .take(20)
            .enumerate()
            .map(|(i, p)| Item::cmd(tidy(p), digit(i), Cmd::GoTo(p.clone()), true))
            .collect();
        self.modal = Some(Modal::Menu(Menu::new(
            Some("History".into()),
            items,
            self.pane_anchor(),
        )));
    }

    fn open_hotlist(&mut self) {
        if self.focused_files().is_none() {
            return;
        }
        let cwd = self.focused_cwd();
        let mut items = Vec::new();
        let mut n = 0;
        for (name, path) in &self.settings.config.bookmarks {
            let p = expand_tilde(path);
            items.push(Item::cmd(
                format!("{name}  \u{b7}  {}", tidy(&p)),
                digit(n),
                Cmd::GoTo(p),
                true,
            ));
            n += 1;
        }
        if !items.is_empty() && !self.bookmarks.is_empty() {
            items.push(Item::sep());
        }
        for p in &self.bookmarks {
            items.push(Item::cmd(tidy(p), digit(n), Cmd::GoTo(p.clone()), true));
            n += 1;
        }
        if !items.is_empty() {
            items.push(Item::sep());
        }
        let writable = !self.settings.mode.read_only();
        if self.bookmarks.contains(&cwd) {
            items.push(Item::cmd(
                format!("Remove {}", tidy(&cwd)),
                "",
                Cmd::RemoveBookmark(cwd),
                writable,
            ));
        } else {
            items.push(Item::cmd(
                format!("Bookmark {}", tidy(&cwd)),
                "",
                Cmd::AddBookmark(cwd),
                writable,
            ));
        }
        items.push(Item::cancel("Cancel", "Esc"));
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Hotlist".into()),
            items,
            self.pane_anchor(),
        )));
    }

    fn save_bookmarks(&mut self, done: String) {
        self.status = match bookmarks::save(&self.bookmarks) {
            Ok(()) => done,
            Err(e) => format!("bookmarks not saved: {e}"),
        };
    }

    /// `o`: the system's own application for the file under the cursor. Something
    /// that would run — a program, a script, a launcher — is asked about first, with
    /// Cancel the choice Enter makes.
    fn open_with(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "o opens a file \u{2014} Enter goes into a directory".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        if opener::runs_something(&path) {
            return self.ask_to_run(path);
        }
        self.launch(&path);
    }

    /// "Run it?" for a program, a script or a launcher: Cancel first, then, for a
    /// console program, a terminal tab (its output stays on screen), then the system.
    fn ask_to_run(&mut self, path: PathBuf) {
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let name = Self::describe(std::slice::from_ref(&path));
        let mut items = vec![Item::cancel("Cancel", "Esc")];
        if opener::console_program(&path) {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            items.push(Item::cmd(
                format!("Run {name} in a terminal tab"),
                "",
                Cmd::RunInTerminal {
                    argv: vec![path.clone().into_os_string()],
                    cwd,
                },
                true,
            ));
        }
        items.push(Item::cmd(
            format!("Run {name} with the system"),
            "",
            Cmd::OpenFile(path),
            true,
        ));
        self.status = format!("{name} is a program \u{2014} opening it runs it");
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Run it?".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Enter on a file, as in Total Commander: an `[[open]]` rule for its extension,
    /// else "Run it?" for a program, else the system's program; and where nothing can
    /// be opened (over SSH, no display, read-only), the viewer, saying why.
    fn enter_file(&mut self) {
        if self.settings.config.files.enter == "view" || self.focused_archive().is_some() {
            return self.begin_view();
        }
        let Some(path) = self.current_file() else {
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            return self.begin_view();
        }
        let screen = opener::no_screen();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let rule = self.settings.config.open.iter().position(|r| {
            r.matches(&ext, crate::config::this_os()) && !(r.needs_display && screen.is_some())
        });
        if let Some(i) = rule {
            return self.apply_rule(i, path);
        }
        if self.refused(Action::OpenWith).is_some() {
            let why = format!("{} mode", self.settings.mode.label());
            return self.view_instead(&why);
        }
        if opener::runs_something(&path) {
            return self.ask_to_run(path);
        }
        let configured = !self.settings.config.commands.opener.trim().is_empty();
        if let (Some(why), false) = (screen, configured) {
            return self.view_instead(&why);
        }
        // A type it does not know as a document: what the system (or the configured
        // opener) does with it is its choice, and some types run code. Asked, not done.
        if !opener::document(&path) {
            return self.ask_to_open(path);
        }
        self.launch(&path);
    }

    /// "Open it?" for a file whose type is not a known document: Cancel first, then the
    /// system's program, then the viewer.
    fn ask_to_open(&mut self, path: PathBuf) {
        let name = Self::describe(std::slice::from_ref(&path));
        let items = vec![
            Item::cancel("Cancel", "Esc"),
            Item::cmd(
                format!("Open {name} with the system's program"),
                "",
                Cmd::OpenFile(path),
                true,
            ),
            Item::act(
                "View it",
                self.footer_key(Action::View).unwrap_or_default(),
                Action::View,
                true,
            ),
        ];
        self.status = format!("{name}: not a type xdevcmd knows as a document \u{2014} open it?");
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Open it?".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Enter could not open the file: view it, and say why where it will be seen — in
    /// the viewer, which covers the status line.
    fn view_instead(&mut self, why: &str) {
        let note = format!("{why}: viewing it instead");
        self.begin_view();
        if let Some(Modal::View(v)) = &mut self.modal {
            v.note(&note);
        }
        self.status = note;
    }

    /// Rule `i` of `[[open]]` on `path`.
    fn apply_rule(&mut self, i: usize, path: PathBuf) {
        // Checked here as well as on Enter: Open with… and the mouse reach this too.
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let Some(rule) = self.settings.config.open.get(i).cloned() else {
            return;
        };
        match rule.action.as_str() {
            "view" => return self.begin_view(),
            "edit" => return self.perform(Action::Edit),
            _ => {}
        }
        // A command runs a program: not where programs may not run.
        if self.refused(Action::OpenWith).is_some() {
            let why = format!("{} mode", self.settings.mode.label());
            return self.view_instead(&why);
        }
        let argv = rule.argv(&path);
        if rule.terminal {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            return self.run_in_terminal(argv, cwd);
        }
        let name = Self::describe(std::slice::from_ref(&path));
        if !self.interactive {
            self.status = format!("would open {name} with {}", rule.command[0]);
            return;
        }
        let mut cmd = std::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        self.status = match opener::spawn(cmd, name.clone(), self.notes.0.clone()) {
            Ok(()) => format!("opened {name} with {}", rule.command[0]),
            Err(e) => e,
        };
    }

    /// A program in a terminal tab of its own, started by argv (no shell), kept after
    /// it ends so its output can be read.
    fn run_in_terminal(&mut self, argv: Vec<std::ffi::OsString>, cwd: PathBuf) {
        let name = argv
            .first()
            .map(|p| Self::describe(&[PathBuf::from(p)]))
            .unwrap_or_default();
        if self.refused(Action::NewTerminal).is_some() || self.settings.mode.read_only() {
            self.status = format!(
                "{} mode \u{2014} running programs is turned off",
                self.settings.mode.label()
            );
            return;
        }
        if !self.interactive {
            self.status = format!("would run {name} in a terminal tab");
            return;
        }
        let full = self.screen;
        if self.term_id.is_none() || !self.term_shown {
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        match self.panel().map(|p| p.add_program_tab(&cwd, &argv)) {
            Some(Ok(())) => self.status = format!("running {name}"),
            Some(Err(e)) => self.status = format!("cannot run {name}: {e}"),
            None => self.status = "no terminal panel".into(),
        }
        if let Some(id) = self.term_id {
            self.focus = id;
        }
    }

    /// Shift+O: every way to open the file under the cursor — the `[[open]]` rules that
    /// match it, the system's program, the viewer, the editor.
    fn open_with_menu(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "Open with\u{2026} is for a file".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let screen = opener::no_screen();
        let mut items = Vec::new();
        for (i, r) in self.settings.config.open.iter().enumerate() {
            if r.matches(&ext, crate::config::this_os()) {
                let usable = !(r.needs_display && screen.is_some());
                items.push(Item::cmd(
                    r.label(),
                    if r.terminal { "in a tab" } else { "" },
                    Cmd::ApplyRule(i, path.clone()),
                    usable,
                ));
            }
        }
        items.push(Item::act(
            "System default",
            self.footer_key(Action::OpenWith).unwrap_or_default(),
            Action::OpenWith,
            self.refused(Action::OpenWith).is_none() && screen.is_none(),
        ));
        items.push(Item::act(
            "View",
            self.footer_key(Action::View).unwrap_or_default(),
            Action::View,
            true,
        ));
        items.push(Item::act(
            "Edit",
            self.footer_key(Action::Edit).unwrap_or_default(),
            Action::Edit,
            self.refused(Action::Edit).is_none(),
        ));
        if opener::console_program(&path) {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            items.push(Item::cmd(
                "Run in a terminal tab",
                "",
                Cmd::RunInTerminal {
                    argv: vec![path.clone().into_os_string()],
                    cwd,
                },
                !self.settings.mode.read_only(),
            ));
        }
        items.push(Item::cancel("Cancel", "Esc"));
        let name = Self::describe(std::slice::from_ref(&path));
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Open {name} with")),
            items,
            self.pane_anchor(),
        )));
    }

    fn launch(&mut self, path: &Path) {
        let name = Self::describe(&[path.to_path_buf()]);
        let cmd = match opener::command(path, &self.settings.config.commands.opener) {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        // Under --screenshot nothing is started: say what would have been.
        if !self.interactive {
            self.status = format!(
                "would open {name} with {}",
                cmd.get_program().to_string_lossy()
            );
            return;
        }
        self.status = match opener::spawn(cmd, name.clone(), self.notes.0.clone()) {
            Ok(()) => format!("opened {name}"),
            Err(e) => e,
        };
    }

    /// The other file pane: the one used last, else any.
    fn other_file_pane(&self) -> Option<PaneId> {
        let files = self.file_panes();
        self.mru
            .iter()
            .copied()
            .find(|id| *id != self.focus && files.contains(id))
            .or_else(|| files.iter().copied().find(|id| *id != self.focus))
    }

    fn swap_panes(&mut self) {
        if self.focus_is_term() {
            return;
        }
        let Some(other) = self.other_file_pane() else {
            self.status = "there is no other pane".into();
            return;
        };
        let (Some(a), Some(b)) = (self.panes.remove(&self.focus), self.panes.remove(&other)) else {
            return;
        };
        self.panes.insert(self.focus, b);
        self.panes.insert(other, a);
        self.status = "panes swapped".into();
    }

    fn mirror_pane(&mut self) {
        if self.focus_is_term() {
            return;
        }
        let Some(other) = self.other_file_pane() else {
            self.status = "there is no other pane".into();
            return;
        };
        let cwd = self.focused_cwd();
        let at = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current())
            .map(|e| e.path.clone());
        if let Some(f) = self.panes.get_mut(&other).and_then(Pane::as_files_mut) {
            f.set_cwd(&cwd);
            if let Some(p) = at {
                f.select_path(&p);
            }
        }
        self.status = format!("both panes on {}", tidy(&cwd));
    }

    // ── bulk rename, quick view ────────────────────────────────────────────────────

    /// F4 in the rename dialog: `text`, one name per file, into a file for the editor.
    fn begin_batch(&mut self, files: Vec<PathBuf>, text: &str) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(list) = crate::tempfiles::file(&format!("rename-{n}.txt")) else {
            self.status = "cannot make a temporary directory for the list of names".into();
            return;
        };
        if let Err(e) = session::write_private(&list, text.as_bytes()) {
            self.status = format!("cannot write the list of names: {e}");
            return;
        }
        self.pending_batch = Some((files, list.clone()));
        self.pending_edit = Some(list);
    }

    /// The editor has closed: read the names back and ask before renaming anything.
    fn finish_batch(&mut self, files: Vec<PathBuf>, list: &Path) {
        let edited = std::fs::read_to_string(list);
        let _ = std::fs::remove_file(list);
        let edited = match edited {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("cannot read the names back: {e}");
                return;
            }
        };
        let moves = match xdevcmd_core::batch_rename::plan(&files, &edited) {
            Ok(m) => m,
            Err(e) => {
                self.status = format!("nothing renamed \u{2014} {e}");
                return;
            }
        };
        if moves.is_empty() {
            self.status = "no names changed".into();
            return;
        }
        let name = |p: &PathBuf| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let body = moves
            .iter()
            .map(|(f, t)| format!("{}  \u{2192}  {}", name(f), name(t)))
            .collect();
        let n = moves.len();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: format!("Rename {n} file{}", if n == 1 { "" } else { "s" }),
            body,
            danger: false,
            yes: Some(("rename".into(), Cmd::BatchRename(moves))),
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Draw the quick view into `area`: the file under the cursor of the focused pane.
    fn draw_quick_view(&mut self, frame: &mut Frame, area: Rect) {
        const LIMIT: u64 = 64 << 20;
        let cur = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned());
        let Some((_, shown)) = self.quick_view.as_mut() else {
            return;
        };
        let note = |frame: &mut Frame, lines: Vec<String>| {
            let inner = modal::frame_box(frame, area, "quick view", theme::t().accent);
            for (i, l) in lines.iter().enumerate() {
                modal::put(
                    frame,
                    inner,
                    i as u16,
                    l,
                    Style::new().fg(theme::t().fg_dim),
                );
            }
        };
        let Some(e) = cur.filter(|e| !e.is_parent()) else {
            *shown = None;
            return note(frame, vec!["nothing under the cursor".into()]);
        };
        if e.is_dir() {
            *shown = None;
            return note(
                frame,
                vec![
                    e.name.clone(),
                    "a directory \u{2014} Space works out its size".into(),
                ],
            );
        }
        if e.size > LIMIT {
            *shown = None;
            return note(
                frame,
                vec![
                    e.name.clone(),
                    format!(
                        "{} \u{2014} too big to preview, F3 views it",
                        human_size(e.size)
                    ),
                ],
            );
        }
        if !xdevcmd_core::policy::get().allows(&e.path) {
            *shown = None;
            return note(
                frame,
                vec!["outside the root xdevcmd is confined to".into()],
            );
        }
        if xdevcmd_core::image::is_image(&e.path) {
            *shown = None;
            return self.draw_picture(frame, area, &e.path);
        }
        if shown.as_ref().is_none_or(|(p, _)| *p != e.path) {
            *shown = match Document::open(&e.path) {
                Ok(doc) => {
                    let v = &self.settings.config.viewer;
                    Some((
                        e.path.clone(),
                        Box::new(Viewer::open(doc, v.wrap, v.tab_width)),
                    ))
                }
                Err(err) => {
                    return note(frame, vec![e.name.clone(), err.to_string()]);
                }
            };
        }
        if let Some((_, v)) = shown.as_mut() {
            v.render(frame, area);
        }
    }

    // ── small file operations, checksum, user menu ─────────────────────────────────

    fn begin_copy_as(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()) else {
            self.status = "nothing to copy here".into();
            return;
        };
        let dir = e
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| f.cwd.clone());
        self.modal = Some(Modal::Input(Input {
            title: "Copy as".into(),
            label: format!("a second copy of {}, beside it", e.name),
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::CopyAs {
                from: e.path.clone(),
            },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    fn make_symlink(&mut self) {
        self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot("making a link".into()));
        let Some(e) = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned())
            .filter(|e| !e.is_parent())
        else {
            self.status = "nothing to link to here".into();
            return;
        };
        let Some(dest) = self.other_pane_cwd() else {
            self.status = "a link goes into the other pane \u{2014} there is none".into();
            return;
        };
        let link = dest.join(&e.name);
        match xdevcmd_core::attrs::symlink(&e.path, &link) {
            Ok(()) => {
                self.reload_all();
                self.status = format!("{} \u{2192} {}", tidy(&link), tidy(&e.path));
            }
            Err(err) => self.status = format!("no link made: {err}"),
        }
    }

    fn begin_permissions(&mut self) {
        let targets = self.selection();
        let Some(first) = targets.first() else {
            self.status = "nothing selected".into();
            return;
        };
        let current = xdevcmd_core::attrs::current(first).unwrap_or_default();
        let many = if targets.len() > 1 {
            format!(" \u{b7} applies to all {}", targets.len())
        } else {
            String::new()
        };
        let label = format!("{}{many}", xdevcmd_core::attrs::describe(first));
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: "Permissions".into(),
            label,
            edit: LineEdit::for_rename(&current, true),
            kind: InputKind::Permissions { targets },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    fn begin_checksum(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "a checksum is of a file".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        // Not a FIFO or a device: reading one would never end, or never start.
        let special = std::fs::metadata(&path)
            .ok()
            .and_then(|m| xdevcmd_core::special_kind(&m.file_type()).map(str::to_string));
        if let Some(kind) = special {
            self.status = format!("that is a {kind}, not a file to checksum");
            return;
        }
        let (tx, rx) = mpsc::channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let stop = std::sync::atomic::AtomicBool::new(false);
            let _ = tx.send(xdevcmd_core::sha256::file(&p, &stop));
        });
        self.status = format!(
            "working out the SHA-256 of {}\u{2026}",
            Self::describe(std::slice::from_ref(&path))
        );
        self.hashing = Some((path, rx));
    }

    fn show_checksum(&mut self, path: &Path, result: std::io::Result<Option<String>>) {
        let name = Self::describe(&[path.to_path_buf()]);
        let hex = match result {
            Ok(Some(h)) => h,
            Ok(None) => return,
            Err(e) => {
                self.status = format!("cannot read {name}: {e}");
                return;
            }
        };
        let mut body = vec![name.clone(), String::new(), format!("SHA-256  {hex}")];
        // A published checksum beside the file, in the usual places.
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut sidecars = vec![dir.join(format!("{name}.sha256"))];
        sidecars.extend(["SHA256SUMS", "sha256sums.txt", "SHA256SUMS.txt"].map(|f| dir.join(f)));
        for side in sidecars {
            let Ok(text) = std::fs::read_to_string(&side) else {
                continue;
            };
            let listed = text.lines().find(|l| {
                l.split_whitespace()
                    .nth(1)
                    .is_none_or(|n| n.trim_start_matches('*') == name)
            });
            let Some(line) = listed else { continue };
            let want = line
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let side = side
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            body.push(String::new());
            body.push(if want == hex {
                format!("\u{2714} matches {side}")
            } else {
                format!("\u{2718} DOES NOT MATCH {side}: {want}")
            });
            break;
        }
        let bad = body.iter().any(|l| l.contains("DOES NOT MATCH"));
        self.status.clear();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Checksum".into(),
            body,
            danger: bad,
            yes: Some(("copy to clipboard".into(), Cmd::CopyText(hex))),
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    fn open_user_menu(&mut self) {
        let menu = &self.settings.config.menu;
        if menu.is_empty() {
            self.status =
                "no commands yet \u{2014} add [[menu]] entries to config.toml (F1, Settings)"
                    .into();
            return;
        }
        let mut items: Vec<Item> = menu
            .iter()
            .enumerate()
            .map(|(i, m)| Item::cmd(m.label.clone(), m.key.clone(), Cmd::UserCommand(i), true))
            .collect();
        items.push(Item::sep());
        items.push(Item::cancel("Cancel", "Esc"));
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Commands".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Type entry `i` of `[[menu]]` into the terminal, placeholders filled and quoted.
    fn run_user_command(&mut self, i: usize) {
        let Some(item) = self.settings.config.menu.get(i).cloned() else {
            return;
        };
        let full = self.screen;
        if self.term_id.is_none() || !self.term_shown {
            self.toggle_term(full);
        }
        let Some(id) = self.term_id else {
            return;
        };
        let cx = crate::usermenu::Context {
            shell: match self.panes.get(&id) {
                Some(Pane::Term(t)) => t.shell_name(),
                _ => String::new(),
            },
            current: self
                .panes
                .get(&self.focus)
                .and_then(Pane::as_files)
                .and_then(|f| f.current())
                .filter(|e| !e.is_parent())
                .map(|e| e.path.clone()),
            dir: self.focused_cwd(),
            selection: self.selection(),
            other: self.other_pane_cwd(),
        };
        let line = match crate::usermenu::expand(&item.command, &cx) {
            Ok(l) => l,
            Err(e) => {
                self.status = format!("{}: {e}", item.label);
                return;
            }
        };
        if let Some(Pane::Term(t)) = self.panes.get(&id) {
            if let Some(program) = t.busy_with() {
                self.status =
                    format!("{program} is running in the terminal \u{2014} not typed into it");
                return;
            }
            t.send(format!("{line}\r").into_bytes());
        }
        self.focus = id;
        self.status = format!("ran {}", item.label);
    }

    // ── background jobs ────────────────────────────────────────────────────────────

    fn pump_background_jobs(&mut self) {
        let mut finished = Vec::new();
        for (i, j) in self.jobs.iter_mut().enumerate() {
            j.pump();
            if j.done.is_some() {
                finished.push(i);
            }
        }
        for i in finished.into_iter().rev() {
            let j = self.jobs.remove(i);
            if let Some(s) = &j.done {
                self.status = format!("background: {}", s.describe(j.kind));
                let s = s.clone();
                self.note_trashed(&s);
                // A job that ended in errors comes forward so they can be read.
                if (!s.errors.is_empty() || s.failed > 0) && self.modal.is_none() {
                    self.modal = Some(Modal::Job(j));
                }
            }
            self.reload_all();
        }
        // A job waiting on a question is blocked until someone answers: bring it to
        // the front as soon as nothing else is.
        if self.modal.is_none() {
            if let Some(i) = self.jobs.iter().position(|j| j.question.is_some()) {
                let j = self.jobs.remove(i);
                self.modal = Some(Modal::Job(j));
            }
        }
    }

    /// Jobs still running: ask first. True when the question is now on screen.
    fn ask_before_quitting(&mut self) -> bool {
        if self.jobs.is_empty() {
            return false;
        }
        let n = self.jobs.len();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Quit?".into(),
            body: vec![
                format!(
                    "{n} job{} still running in the background.",
                    if n == 1 { " is" } else { "s are" }
                ),
                "Quitting cancels them; a file being written is removed, not left half done."
                    .into(),
            ],
            danger: true,
            yes: Some(("cancel them and quit".into(), Cmd::QuitAnyway)),
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
        true
    }

    // ── compare and synchronise ────────────────────────────────────────────────────

    /// The two directories to compare: this pane's and the other's, both ordinary
    /// listings (not an archive, not search results).
    fn compare_pair(&mut self) -> Option<(PaneId, PathBuf, PaneId, PathBuf, bool)> {
        let plain = |f: &FilePane| f.archive.is_none() && f.results.is_none();
        let me = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .filter(|f| plain(f));
        let Some(me) = me else {
            self.status = "compare works between two directories".into();
            return None;
        };
        let (left, hidden) = (me.cwd.clone(), me.show_hidden);
        let other = self.other_file_pane();
        let right = other
            .and_then(|o| self.panes.get(&o))
            .and_then(Pane::as_files)
            .filter(|f| plain(f))
            .map(|f| f.cwd.clone());
        match (other, right) {
            (Some(o), Some(r)) if r != left => Some((self.focus, left, o, r, hidden)),
            (Some(_), Some(_)) => {
                self.status = "both panes show the same directory".into();
                None
            }
            _ => {
                self.status = "compare needs a second pane on a directory".into();
                None
            }
        }
    }

    /// Alt+K: mark, in each pane, what is newer there or missing on the other side —
    /// so F5 from either pane copies exactly what the other lacks.
    fn compare_panes(&mut self) {
        use xdevcmd_core::compare::{compare, State};
        let Some((me, left, other, right, hidden)) = self.compare_pair() else {
            return;
        };
        let diffs = compare(&left, &right, false, hidden);
        let count = |s: &[State]| diffs.iter().filter(|d| s.contains(&d.state)).count();
        let mark = |f: &mut FilePane, root: &Path, states: &[State]| {
            f.marked.clear();
            for d in diffs.iter().filter(|d| states.contains(&d.state)) {
                let p = root.join(&d.rel);
                if f.entries.iter().any(|e| e.path == p) {
                    f.marked.insert(p);
                }
            }
        };
        if let Some(f) = self.panes.get_mut(&me).and_then(Pane::as_files_mut) {
            mark(f, &left, &[State::LeftOnly, State::LeftNewer]);
        }
        if let Some(f) = self.panes.get_mut(&other).and_then(Pane::as_files_mut) {
            mark(f, &right, &[State::RightOnly, State::RightNewer]);
        }
        let mine = count(&[State::LeftOnly, State::LeftNewer]);
        let theirs = count(&[State::RightOnly, State::RightNewer]);
        let clash = count(&[State::Conflict]);
        let same = count(&[State::Same]);
        let clash = if clash > 0 {
            format!(" \u{b7} {clash} differ at the same time \u{2014} left alone")
        } else {
            String::new()
        };
        self.status = format!(
            "marked: {mine} here, {theirs} there, newer or missing on the other side{clash} \u{b7} {same} the same"
        );
    }

    /// Alt+Y: both trees, compared all the way down on a thread (a big tree or a network
    /// drive can take a while, and Esc stops it), then a plan to make them agree.
    fn begin_sync(&mut self) {
        use std::sync::atomic::{AtomicBool, AtomicU64};
        use std::sync::Arc;
        let Some((_, left, _, right, hidden)) = self.compare_pair() else {
            return;
        };
        if let Some(old) = self.comparing.take() {
            old.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // A screenshot (the tests) has no later frame to bring the result to: compare
        // there and then.
        if !self.interactive {
            let diffs = xdevcmd_core::compare::compare(&left, &right, true, hidden);
            return self.show_sync_plan(&left, &right, &diffs);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let (l, r, stop, seen) = (left.clone(), right.clone(), stop.clone(), seen.clone());
            std::thread::spawn(move || {
                let _ = tx.send(xdevcmd_core::compare::compare_until(
                    &l, &r, true, hidden, &stop, &seen,
                ));
            });
        }
        self.status = "comparing the two trees\u{2026} Esc stops".into();
        self.comparing = Some(Comparing {
            left,
            right,
            stop,
            seen,
            shown: 0,
            rx,
        });
    }

    /// Alt+Y's compare has come in, or moved on: show its plan, or how far it is.
    fn poll_comparing(&mut self) -> bool {
        let Some(c) = self.comparing.as_mut() else {
            return false;
        };
        match c.rx.try_recv() {
            Ok(Some(diffs)) => {
                let (left, right) = (c.left.clone(), c.right.clone());
                self.comparing = None;
                self.show_sync_plan(&left, &right, &diffs);
                true
            }
            Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.comparing = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                let n = c.seen.load(std::sync::atomic::Ordering::Relaxed);
                if n != c.shown {
                    c.shown = n;
                    self.status =
                        format!("comparing the two trees\u{2026} {n} looked at \u{b7} Esc stops");
                    return true;
                }
                false
            }
        }
    }

    fn show_sync_plan(&mut self, left: &Path, right: &Path, diffs: &[xdevcmd_core::compare::Diff]) {
        use xdevcmd_core::compare::{plan, State};
        let pairs = plan(left, right, diffs);
        let mut body = vec![
            format!("\u{2192} this pane  {}", tidy(left)),
            format!("\u{2190} the other  {}", tidy(right)),
            String::new(),
        ];
        let mut overwrites = 0;
        for d in diffs {
            let slash = if d.is_dir { "/" } else { "" };
            let line = match d.state {
                State::LeftOnly => format!("\u{2192} {}{slash}", d.rel),
                State::RightOnly => format!("\u{2190} {}{slash}", d.rel),
                State::LeftNewer => {
                    overwrites += 1;
                    format!("\u{2192} {}  replaces an older copy", d.rel)
                }
                State::RightNewer => {
                    overwrites += 1;
                    format!("\u{2190} {}  replaces an older copy", d.rel)
                }
                State::Conflict => format!(
                    "\u{2260} {}  same time, different size \u{2014} left alone",
                    d.rel
                ),
                State::Same => continue,
            };
            body.push(line);
        }
        if pairs.is_empty() {
            self.status = if body.len() > 3 {
                "nothing to copy \u{2014} only differences that need a person".into()
            } else {
                "the two trees already agree".into()
            };
            if body.len() <= 3 {
                return;
            }
        }
        let n = pairs.len();
        let title = match overwrites {
            0 => format!("Synchronise \u{b7} {n} to copy"),
            o => format!("Synchronise \u{b7} {n} to copy, {o} replacing older files"),
        };
        self.modal = Some(Modal::Ask(modal::Ask {
            title,
            body,
            danger: overwrites > 0,
            yes: (n > 0).then(|| (format!("copy {n}"), Cmd::Sync(pairs))),
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    // ── archives ───────────────────────────────────────────────────────────────────

    fn focused_archive(&self) -> Option<&crate::pane::files::ArchiveView> {
        self.panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.archive.as_ref())
    }

    fn confirm_extract(
        &mut self,
        file: PathBuf,
        inner: Vec<String>,
        prefix: String,
        dest: PathBuf,
        choices: Vec<PathBuf>,
    ) {
        if inner.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        // Browsing it, the listing is in memory already; from outside (Alt+F9) it is
        // counted on a thread, as F5's count is, so a big .tar.gz does not freeze.
        let open = self.focused_archive().filter(|a| a.file == file).map(|a| {
            xdevcmd_core::archive::expand(&a.listing().items, &inner)
                .iter()
                .filter(|i| i.kind == xdevcmd_core::archive::ItemKind::File)
                .fold((0u64, 0u64), |(n, b), i| (n + 1, b + i.size))
        });
        let (count_line, counting) = match open {
            Some((n, bytes)) => (Self::count_label(n, bytes), None),
            None => {
                let (f, chosen) = (file.clone(), inner.clone());
                let work = move |stop: &std::sync::atomic::AtomicBool| {
                    xdevcmd_core::archive::count_until(&f, &chosen, stop)
                };
                (COUNTING.to_string(), Some((ops::Count::spawn(work), 3)))
            }
        };
        let what = match inner.as_slice() {
            [all] if all.is_empty() => "everything".to_string(),
            [one] => one.rsplit('/').next().unwrap_or(one).to_string(),
            many => format!("{} items", many.len()),
        };
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let request = Request::extract(file, inner, prefix, dest.clone());
        self.modal = Some(Modal::Confirm(Confirm {
            title: "Extract".into(),
            body: vec![
                format!("{what}  from {name}"),
                String::new(),
                format!("to    {}", dest.display()),
                count_line,
            ],
            danger: false,
            request,
            buttons: Vec::new(),
            choices,
            choice: 0,
            source_dir: None,
            counting,
            permanent: false,
            existing_line: None,
        }));
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            c.set_destination(0);
        }
    }

    /// Alt+F9 on an archive: everything in it, into the other pane.
    fn begin_unpack(&mut self) {
        let Some(file) = self
            .current_file()
            .filter(|p| xdevcmd_core::archive::format_of(p).is_some())
        else {
            self.status =
                "Alt+F9 unpacks a .zip, .tar or .tar.gz \u{2014} the cursor is not on one".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&file) {
            self.status = "that archive is outside the root xdevcmd is confined to".into();
            return;
        }
        // The whole archive, named by the empty path: listing it to find its top level
        // would be the freeze this avoids.
        let top = vec![String::new()];
        let choices = self.transfer_targets();
        let Some(dest) = choices.first().cloned() else {
            self.status = "unpacking goes into another pane \u{2014} Alt+S splits one".into();
            return;
        };
        self.confirm_extract(file, top, String::new(), dest, choices);
    }

    /// Alt+F5: the selection into a new archive, in the other pane by default.
    fn begin_pack(&mut self) {
        let sources = self.selection();
        if sources.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let dir = self.other_pane_cwd().unwrap_or_else(|| self.focused_cwd());
        let stem = match sources.as_slice() {
            [one] => one
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            _ => self
                .focused_cwd()
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "archive".into()),
        };
        // The name selected, so typing replaces it; the extension is the format.
        let edit = LineEdit::for_rename(&format!("{stem}.zip"), false);
        self.modal = Some(Modal::Input(Input {
            title: "Pack".into(),
            label: format!(
                "{} into {} \u{b7} .zip .tar .tar.gz",
                Self::describe(&sources),
                dir.display()
            ),
            edit,
            kind: InputKind::Pack { sources },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// F3 inside an archive: the one entry extracted to a private temporary directory,
    /// viewed, and removed when the viewer closes.
    fn view_in_archive(&mut self) {
        const LIMIT: u64 = 256 << 20;
        let Some(a) = self.focused_archive() else {
            return;
        };
        let (file, prefix) = (a.file.clone(), a.prefix.clone());
        let Some(e) = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned())
            .filter(|e| !e.is_dir())
        else {
            self.status = "F3 views a file, not a directory".into();
            return;
        };
        if e.size > LIMIT {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status = format!(
                "{} is too big to view from inside \u{2014} {k} extracts it",
                e.name
            );
            return;
        }
        let inner = self
            .focused_archive()
            .map(|a| a.inner(std::slice::from_ref(&e.path)))
            .unwrap_or_default();
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(tmp) = crate::tempfiles::file(&format!("view-{n}")) else {
            self.status = "cannot make a temporary directory".into();
            return;
        };
        let made = {
            // Mutable only where there is a mode to set.
            #[cfg_attr(not(unix), allow(unused_mut))]
            let mut b = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                b.mode(0o700);
            }
            b.create(&tmp)
        };
        if let Err(err) = made {
            self.status = format!("cannot make a temporary directory: {err}");
            return;
        }
        // The guard on writes is the engine's; viewing is reading, so it is allowed
        // in read-only mode — into a directory xdevcmd made for it and removes after.
        // A small archive is read at once; a big one on a thread, since reaching one
        // entry of a .tar.gz means decompressing everything before it.
        let small = std::fs::metadata(&file).map_or(true, |m| m.len() < (8 << 20));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let name = e.name.clone();
        if small {
            let out =
                xdevcmd_core::archive::extract(&file, &inner, &prefix, &tmp, &mut ViewHooks(stop));
            return self.show_extracted(out, tmp, name);
        }
        let (tx, rx) = mpsc::channel();
        let (dir, flag) = (tmp.clone(), stop.clone());
        std::thread::spawn(move || {
            let out = xdevcmd_core::archive::extract(
                &file,
                &inner,
                &prefix,
                &dir,
                &mut ViewHooks(flag.clone()),
            );
            // Given up on: nobody will view it, so nothing is left behind either.
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = std::fs::remove_dir_all(&dir);
            }
            let _ = tx.send(out);
        });
        self.status = format!("extracting {name} to view it\u{2026} Esc stops it");
        self.pending_view = Some((rx, tmp, name, stop));
    }

    /// Open the viewer on an entry extracted for it, or say why not.
    fn show_extracted(&mut self, out: std::io::Result<()>, tmp: PathBuf, name: String) {
        let path = tmp.join(&name);
        match out.and_then(|_| Document::open(&path)) {
            Ok(doc) => {
                let v = &self.settings.config.viewer;
                let viewer = Viewer::open(doc, v.wrap, v.tab_width);
                self.modal = Some(Modal::View(Box::new(viewer)));
                self.view_temp = Some(tmp);
                self.status.clear();
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&tmp);
                self.status = format!("cannot view {name}: {err}");
            }
        }
    }

    /// A picture in half blocks: each cell is two pixels, the upper as the
    /// foreground of `▀` and the lower as its background.
    fn draw_picture(&mut self, frame: &mut Frame, area: Rect, path: &Path) {
        let th = theme::t();
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        let bg = match th.bg_panel {
            ratatui::style::Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        let size = (inner.width, inner.height);
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let fresh = |p: &Path, s: (u16, u16), m: Option<std::time::SystemTime>| {
            p == path && s == size && m == mtime
        };
        let stale = self
            .quick_image
            .as_ref()
            .is_none_or(|(p, s, m, _)| !fresh(p, *s, *m));
        if stale {
            let (w, h) = (inner.width as usize, inner.height as usize * 2);
            if !self.interactive {
                // A screenshot has no later frame for the picture to arrive in.
                let pic = xdevcmd_core::image::load(path, w, h, bg).map_err(|e| e.to_string());
                self.quick_image = Some((path.to_path_buf(), size, mtime, pic));
            } else if !self
                .decoding
                .as_ref()
                .is_some_and(|(p, s, m, _)| fresh(p, *s, *m))
            {
                let (tx, rx) = std::sync::mpsc::channel();
                let file = path.to_path_buf();
                std::thread::spawn(move || {
                    let _ = tx.send(
                        xdevcmd_core::image::load(&file, w, h, bg).map_err(|e| e.to_string()),
                    );
                });
                self.decoding = Some((path.to_path_buf(), size, mtime, rx));
            }
        }
        let current = self
            .quick_image
            .as_ref()
            .filter(|(p, s, m, _)| fresh(p, *s, *m));
        let Some((_, _, _, pic)) = current else {
            let inner = modal::frame_box(frame, area, "image", th.accent);
            modal::put(
                frame,
                inner,
                0,
                "decoding\u{2026}",
                Style::new().fg(th.fg_dim),
            );
            return;
        };
        let title = match pic {
            Ok(p) => format!("image {}\u{d7}{}", p.source.0, p.source.1),
            Err(_) => "image".into(),
        };
        let inner = modal::frame_box(frame, area, &title, th.accent);
        let pic = match pic {
            Ok(p) => p,
            Err(e) => {
                modal::put(frame, inner, 0, e, Style::new().fg(th.err));
                return;
            }
        };
        let rows = pic.height.div_ceil(2);
        let x0 = inner.x + (inner.width.saturating_sub(pic.width as u16)) / 2;
        let y0 = inner.y + (inner.height.saturating_sub(rows as u16)) / 2;
        modal::paint_picture(frame, x0, y0, pic, bg);
    }

    // ── menu bar, usage ────────────────────────────────────────────────────────────

    /// What the drop-down for menu `i` holds, built from the keymap and the mode in
    /// effect, so its hints are the keys you have and what is off is greyed out.
    fn bar_items(&self, i: usize) -> Vec<Item> {
        use Action as A;
        let file: &[Option<(&str, A)>] = &[
            Some(("View", A::View)),
            Some(("Edit", A::Edit)),
            Some(("Open with system app", A::OpenWith)),
            Some(("Open with\u{2026}", A::OpenWithMenu)),
            None,
            Some(("Copy", A::Copy)),
            Some(("Move", A::Move)),
            Some(("Copy as\u{2026}", A::CopyAs)),
            Some(("Rename", A::Rename)),
            Some(("Rename many\u{2026}", A::BulkRename)),
            Some(("Link in other pane", A::Symlink)),
            Some(("Permissions\u{2026}", A::Permissions)),
            Some(("Checksum", A::Checksum)),
            None,
            Some(("New file\u{2026}", A::NewFile)),
            Some(("New directory\u{2026}", A::MkDir)),
            Some(("Pack\u{2026}", A::Pack)),
            Some(("Unpack", A::Unpack)),
            None,
            Some(("Delete", A::Delete)),
            Some(("Delete permanently", A::DeletePermanent)),
            None,
            Some(("Find files\u{2026}", A::FindFiles)),
            Some(("Your commands\u{2026}", A::UserMenu)),
            None,
            Some(("Quit", A::Quit)),
        ];
        // Settings… is put in just above Quit, below.
        let view: &[Option<(&str, A)>] = &[
            Some(("Quick search", A::QuickSearch)),
            Some(("Filter\u{2026}", A::Filter)),
            Some(("Go to path\u{2026}", A::GoToPath)),
            Some(("Back", A::Back)),
            Some(("Forward", A::Forward)),
            Some(("History", A::History)),
            Some(("Hotlist", A::Hotlist)),
            None,
            Some(("Hidden files", A::ToggleHidden)),
            Some(("Sort by name", A::SortName)),
            Some(("Sort by extension", A::SortExt)),
            Some(("Sort by size", A::SortSize)),
            Some(("Sort by date", A::SortModified)),
            Some(("Directory sizes", A::CalcSizes)),
            Some(("Refresh", A::Refresh)),
            None,
            Some(("Quick view", A::QuickView)),
            Some(("Zoom", A::Zoom)),
            Some(("Split right", A::SplitRight)),
            Some(("Split down", A::SplitDown)),
            Some(("Close pane", A::ClosePane)),
            Some(("Swap panes", A::SwapPanes)),
            Some(("Same directory there", A::MirrorPane)),
            Some(("Compare with other pane", A::CompareDirs)),
            Some(("Synchronise\u{2026}", A::SyncDirs)),
            None,
            Some(("Terminal", A::FocusTerminal)),
            Some(("New terminal", A::NewTerminal)),
            Some(("Show or hide terminal", A::ToggleTerminal)),
        ];
        let help: &[Option<(&str, A)>] = &[
            Some(("Keys and settings", A::Help)),
            Some(("Usage", A::Usage)),
        ];
        let list = match i {
            0 => file,
            1 => view,
            _ => help,
        };
        let mut items: Vec<Item> = list
            .iter()
            .map(|e| match e {
                None => Item::sep(),
                Some((label, a)) => Item::act(
                    *label,
                    self.footer_key(*a).unwrap_or_default(),
                    *a,
                    self.refused(*a).is_none(),
                ),
            })
            .collect();
        if i == 0 {
            // Above Quit, which is last.
            let at = items.len().saturating_sub(1);
            items.insert(at, Item::cmd("Settings\u{2026}", "", Cmd::Settings, true));
        }
        if i == 1 {
            items.push(Item::sep());
            items.push(Item::cmd("Theme\u{2026}", "", Cmd::ThemeMenu, true));
        }
        if i == 2 {
            items.push(Item::sep());
            items.push(Item::cmd("About xdevcmd", "", Cmd::About, true));
        }
        items
    }

    /// What a job about to start means for Ctrl+Z: a move can be undone; the rest say
    /// that they cannot, so an older rename is never undone behind them.
    fn remember_job(&mut self, r: &Request) {
        use xdevcmd_core::undo::Undo;
        self.last_undo = Some(match r.kind {
            ops::Kind::Move => Undo::Move {
                sources: r.sources.clone(),
                dest: r.dest.clone().unwrap_or_default(),
                rename_to: r.rename_to.clone(),
            },
            ops::Kind::Copy => Undo::Cannot("a copy".into()),
            ops::Kind::Delete if r.to_trash => {
                // Undoable once it has finished and said where each item went.
                self.trash_pending = Some(r.sources.clone());
                Undo::Cannot(TRASH_RUNNING.into())
            }
            ops::Kind::Delete => Undo::Cannot("a permanent delete".into()),
            ops::Kind::Extract => Undo::Cannot("an extraction".into()),
            ops::Kind::Pack => Undo::Cannot("packing".into()),
        });
    }

    /// Keep the system watching the directories on screen, and note the panes whose
    /// directory it says changed; they re-read on the next quiet moment.
    fn watch_panes(&mut self, shown: &[PaneId], every: Duration) {
        let Some(w) = self.watcher.as_mut() else {
            return;
        };
        let mut dirs = HashSet::new();
        let mut owners: Vec<(PaneId, PathBuf)> = Vec::new();
        if !every.is_zero() {
            for id in shown {
                let Some(f) = self.panes.get(id).and_then(Pane::as_files) else {
                    continue;
                };
                if f.archive.is_some() || f.results.is_some() {
                    continue;
                }
                // The system reports canonical paths (macOS's /tmp is /private/tmp).
                let canon = self
                    .canonical
                    .entry(f.cwd.clone())
                    .or_insert_with(|| f.cwd.canonicalize().unwrap_or_else(|_| f.cwd.clone()))
                    .clone();
                dirs.insert(canon.clone());
                owners.push((*id, canon));
            }
        }
        let new = w.sync(&dirs);
        let changed = w.changed();
        for (id, canon) in owners {
            if changed.contains(&canon) {
                self.to_reload.insert(id);
            } else if new.contains(&canon) {
                self.to_check.insert(id);
            }
        }
        // Forget what is no longer shown, so the map does not grow without end.
        self.canonical.retain(|_, c| dirs.contains(c));
    }

    /// A delete to the trash has finished: if it is still the last operation, undo
    /// can now put back what it moved.
    fn note_trashed(&mut self, s: &ops::Summary) {
        let Some(sources) = &self.trash_pending else {
            return;
        };
        let ours = !s.trashed.is_empty() && s.trashed.iter().all(|t| sources.contains(&t.original));
        let still_last = matches!(&self.last_undo, Some(xdevcmd_core::undo::Undo::Cannot(w)) if w == TRASH_RUNNING);
        if ours && still_last {
            self.last_undo = Some(xdevcmd_core::undo::Undo::Trashed(s.trashed.clone()));
            self.trash_pending = None;
        }
    }

    /// Ctrl+Z: say what undoing would do, against the disk as it is now, and ask.
    fn ask_undo(&mut self) {
        let Some(u) = &self.last_undo else {
            self.status = "nothing to undo".into();
            return;
        };
        let plan = match u.plan() {
            Ok(p) => p,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        use xdevcmd_core::undo::Plan;
        let what = match &plan {
            Plan::Rename(m) => m
                .iter()
                .take(8)
                .map(|(f, t)| {
                    format!(
                        "{}  \u{2192}  {}",
                        Self::describe(std::slice::from_ref(f)),
                        Self::describe(std::slice::from_ref(t))
                    )
                })
                .collect::<Vec<_>>(),
            Plan::Move { back, into } => vec![format!(
                "move {} back to {}",
                Self::describe(back),
                tidy(into)
            )],
            Plan::Remove(p) => vec![format!("remove {}, still empty", tidy(p))],
            Plan::Restore(items) => {
                let mut lines: Vec<String> = items
                    .iter()
                    .take(8)
                    .map(|i| format!("put back {}", tidy(&i.original)))
                    .collect();
                if items.len() > 8 {
                    lines.push(format!("\u{2026} and {} more", items.len() - 8));
                }
                lines
            }
        };
        let mut body = vec![format!("Undo the {}?", u.describe()), String::new()];
        body.extend(what);
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Undo".into(),
            body,
            danger: false,
            yes: Some(("undo".into(), Cmd::Undo)),
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Carry out the undo, checking the disk once more.
    fn undo(&mut self) {
        use xdevcmd_core::undo::Plan;
        let Some(u) = self.last_undo.clone() else {
            return;
        };
        let plan = match u.plan() {
            Ok(p) => p,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        match plan {
            Plan::Rename(moves) => {
                let out = xdevcmd_core::batch_rename::apply(&moves);
                self.status = match out.errors.first() {
                    None => format!("undone: {}", u.describe()),
                    Some(e) => format!(
                        "undo: {} renamed back, {} failed \u{2014} {e}",
                        out.renamed,
                        out.errors.len()
                    ),
                };
            }
            Plan::Move { back, into } => {
                let request = Request::mv(back, into);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Move, ops::start(request))));
                self.status = format!("undoing the {}", u.describe());
            }
            Plan::Remove(p) => {
                self.status = match xdevcmd_core::undo::remove(&p) {
                    Ok(()) => format!("undone: {}", u.describe()),
                    Err(e) => format!("cannot undo: {e}"),
                };
            }
            Plan::Restore(items) => {
                let mut failed = Vec::new();
                for t in &items {
                    if let Err(e) = xdevcmd_core::trashed::restore(t) {
                        failed.push(e.to_string());
                    }
                }
                self.status = match failed.first() {
                    None => format!("undone: {}", u.describe()),
                    Some(e) => format!(
                        "undo: {} put back, {} not \u{2014} {e}",
                        items.len() - failed.len(),
                        failed.len()
                    ),
                };
            }
        }
        // One step: an undone operation is not undone again.
        self.last_undo = None;
        self.reload_all();
    }

    /// File → Settings….
    fn open_settings(&mut self) {
        let cannot_save = if self.settings.mode.read_only() {
            Some(format!(
                "{} mode \u{2014} xdevcmd writes nothing, its settings included",
                self.settings.mode.label()
            ))
        } else if let Outcome::Rejected { .. } = self.config_outcome {
            Some(
                "config.toml has an error (F1 \u{203a} Settings says which): fix it by hand first"
                    .into(),
            )
        } else {
            None
        };
        self.modal = Some(Modal::Settings(Box::new(SettingsDialog::new(
            &self.settings.config,
            crate::config::available_themes(),
            cannot_save,
        ))));
    }

    /// Enter in Settings: write what changed into the config file, keeping the rest of
    /// it as it was, then use the new settings. The edited file is read back first, and
    /// nothing is written unless it says exactly what the dialog showed.
    fn save_settings(&mut self) {
        let Some(Modal::Settings(d)) = self.modal.take() else {
            return;
        };
        let changes = d.changed();
        if changes.is_empty() {
            self.status = "settings: nothing changed".into();
            return;
        }
        // The dialog offers no Save in these modes; refused here too, not on its word.
        if self.settings.mode.read_only() {
            self.status = format!(
                "{} mode \u{2014} xdevcmd writes nothing, settings included",
                self.settings.mode.label()
            );
            return;
        }
        let Some(path) = crate::config::path_in_use() else {
            self.status = "nowhere to save: no config location (HOME is not set)".into();
            return;
        };
        // A config.toml that is a link (dotfiles managed with stow, chezmoi) is
        // written where it points, so the link stays a link.
        let path = std::fs::canonicalize(&path)
            .or_else(|_| {
                // A link whose target does not exist yet (stow before the first save):
                // write the target, so the link stays.
                std::fs::read_link(&path).map(|t| match path.parent() {
                    Some(dir) if t.is_relative() => dir.join(t),
                    _ => t,
                })
            })
            .unwrap_or(path);
        let before = match std::fs::read(&path) {
            Ok(b) => match String::from_utf8(b) {
                Ok(t) => t,
                Err(_) => {
                    self.status = format!("{} is not UTF-8: change it by hand", path.display());
                    return;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                self.status = format!("cannot read {}: {e}", path.display());
                return;
            }
        };
        let mut text = before;
        for (f, v) in &changes {
            text = crate::config_edit::set(&text, f.section, f.key, &settings_dialog::value(f, v));
        }
        let parsed = match crate::config::parse(&text) {
            Ok(s) if s.config == d.now => s,
            Ok(_) | Err(_) => {
                self.status = format!(
                    "{} is written in a way Settings cannot change safely: change it by hand",
                    path.display()
                );
                return;
            }
        };
        if let Err(e) = session::write_private(&path, text.as_bytes()) {
            self.status = format!("cannot save {}: {e}", path.display());
            return;
        }
        // The new settings, keeping what start-up decided: the mode (a flag may have
        // made it stricter), the root, the colour depth.
        let mut s = parsed;
        s.mode = self.settings.mode;
        s.root = self.settings.root.clone();
        s.depth = self.settings.depth;
        self.settings = s;
        self.config_outcome = Outcome::Loaded(path.clone());
        let name = self.settings.config.theme.name.clone();
        self.set_theme(&name);
        let later = d.need_restart();
        self.status = if later.is_empty() {
            format!("settings saved to {}", path.display())
        } else {
            format!(
                "settings saved; at the next start: {}",
                later.join(", ").to_lowercase()
            )
        };
    }

    /// View → Theme…: every theme, built in and from `themes/`, the current one marked.
    fn theme_menu(&mut self) {
        let current = self.theme_name.clone();
        let items: Vec<Item> = crate::config::available_themes()
            .into_iter()
            .map(|name| {
                let mark = if name == current { "\u{2022}" } else { "" };
                Item::cmd(name.clone(), mark, Cmd::SetTheme(name), true)
            })
            .chain(std::iter::once(Item::cancel("Cancel", "Esc")))
            .collect();
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Theme".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Switch themes now. The colours set in `[theme]` stay on top, as at start-up.
    fn set_theme(&mut self, name: &str) {
        let s = &self.settings;
        match crate::config::theme_for(name, &s.config.theme, s.depth) {
            Ok(t) => {
                theme::set(t);
                self.theme_name = name.to_string();
                self.status = format!(
                    "theme: {name} \u{2014} [theme] name = \"{name}\" in config.toml keeps it"
                );
            }
            Err(e) => self.status = e,
        }
    }

    fn open_bar(&mut self, i: usize) {
        let x = self
            .bar_hits
            .iter()
            .find(|(j, _)| *j == i)
            .map(|(_, r)| r.x)
            .unwrap_or(0);
        // Below the bar when it is on screen; at the top otherwise (F9 with it off).
        let y = u16::from(!self.bar_hits.is_empty());
        self.modal = Some(Modal::Menu(Menu::dropdown(i, self.bar_items(i), (x, y))));
    }

    fn draw_bar(&mut self, frame: &mut Frame, area: Rect) {
        self.bar_hits.clear();
        if area.height == 0 {
            return;
        }
        let th = theme::t();
        let open = match &self.modal {
            Some(Modal::Menu(m)) => m.bar,
            _ => None,
        };
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(th.bg_bar));
        let mut x = area.x + 1;
        for (i, title) in BAR.iter().enumerate() {
            let text = format!(" {title} ");
            let w = text.chars().count() as u16;
            if x + w > area.x + area.width {
                break;
            }
            let style = if open == Some(i) {
                Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(th.fg)
            };
            buf.set_line(x, area.y, &Line::from(Span::styled(text, style)), w);
            self.bar_hits.push((i, Rect::new(x, area.y, w, 1)));
            x += w + 1;
        }
    }

    fn holding(&self) -> crate::usage::Holding {
        let mut h = crate::usage::Holding {
            jobs: self.jobs.len(),
            ..Default::default()
        };
        for p in self.panes.values() {
            match p {
                Pane::Files(t) => {
                    h.panes += 1;
                    h.tabs += t.len();
                    h.listed += t.tabs().iter().map(|f| f.entries.len()).sum::<usize>();
                }
                Pane::Term(t) => h.shells += t.shell_count(),
            }
        }
        h
    }

    // ── drawing ────────────────────────────────────────────────────────────────────

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.screen = area;
        self.note_focus();
        for p in self.panes.values_mut() {
            if let Some(n) = p.as_files_mut().and_then(|f| f.notice.take()) {
                self.status = n;
            }
        }
        frame
            .buffer_mut()
            .set_style(area, Style::new().bg(theme::t().bg).fg(theme::t().fg));

        let bar_h = u16::from(self.settings.config.general.menu_bar && area.height >= 8);
        let [bar, main, status, footer] = Layout::vertical([
            Constraint::Length(bar_h),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        self.draw_bar(frame, bar);

        self.main = main;
        if self
            .zoom
            .is_some_and(|z| z != self.focus || !self.tree.contains(z))
        {
            self.zoom = None;
            // Do not leave "zoomed, press Alt+Z" on screen once it is not.
            if self.status.starts_with("zoomed") {
                self.status.clear();
            }
        }
        let mut rects = Vec::new();
        match self.zoom {
            Some(z) => rects.push((z, main)),
            None => self.tree.assign(main, &mut rects),
        }
        self.rects = rects.clone();

        // Quick view ends when its pane takes the focus or goes away.
        if self
            .quick_view
            .as_ref()
            .is_some_and(|(q, _)| *q == self.focus || !self.tree.contains(*q))
        {
            self.quick_view = None;
        }
        let previewing = self.quick_view.as_ref().map(|(q, _)| *q);
        for (id, r) in rects {
            let active = id == self.focus;
            if previewing == Some(id) {
                self.draw_quick_view(frame, r);
                continue;
            }
            match self.panes.get_mut(&id) {
                Some(Pane::Files(f)) => {
                    let target = self
                        .drag
                        .as_ref()
                        .is_some_and(|d| d.mode == DragMode::Files && d.hover == Some(id));
                    f.render(frame, r, active, target)
                }
                Some(Pane::Term(p)) => p.render(frame, r, active),
                None => {}
            }
        }

        if let Some(s) = &self.search {
            let at = self
                .rects
                .iter()
                .find(|(id, _)| *id == s.pane)
                .map(|(_, r)| *r);
            let f = self.panes.get(&s.pane).and_then(Pane::as_files);
            if let (Some(r), Some(f)) = (at, f) {
                let count = if s.filter {
                    f.entries.iter().filter(|e| !e.is_parent()).count()
                } else {
                    search::matches(&f.entries, &s.text()).len()
                };
                search::render(frame, r, s, count);
            }
        }

        self.draw_status(frame, status);
        self.draw_footer(frame, footer);

        if let Some(m) = self.modal.as_mut() {
            modal::render(frame, area, m);
        }
        if matches!(self.modal, Some(Modal::Usage)) {
            let holding = self.holding();
            crate::usage::render(frame, area, &self.usage, &holding);
        }
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let left = match self.panes.get(&self.focus) {
            Some(Pane::Files(tabs)) => {
                let f = tabs.active();
                let tab = if tabs.len() > 1 {
                    format!("  \u{b7}  tab {}/{}", tabs.active_index() + 1, tabs.len())
                } else {
                    String::new()
                };
                let sort = match f.sort.key {
                    SortKey::Name => "name",
                    SortKey::Ext => "ext",
                    SortKey::Size => "size",
                    SortKey::Modified => "date",
                };
                format!(
                    " {}  ·  {} items  ·  sort {}{}{}{}",
                    f.summary(),
                    f.entries.len(),
                    sort,
                    if f.sort.reverse { " ▲" } else { " ▼" },
                    if f.show_hidden { "  ·  hidden" } else { "" },
                    tab
                )
            }
            Some(Pane::Term(p)) => format!(
                " terminal {}/{}{}  \u{b7}  {} back to files",
                p.active_tab() + 1,
                p.tab_count(),
                if p.split_count() > 1 {
                    format!("  \u{b7}  split {}/{}", p.focused_split(), p.split_count())
                } else {
                    String::new()
                },
                self.footer_key(Action::FocusTerminal).unwrap_or_default()
            ),
            None => String::new(),
        };
        let left = if self.zoom.is_some() {
            format!("{left}  \u{b7}  zoomed")
        } else {
            left
        };
        let left = if self.jobs.is_empty() {
            left
        } else {
            let (done, total) = self.jobs.iter().fold((0u64, 0u64), |(d, t), j| {
                (d + j.bytes_done, t + j.total_bytes)
            });
            let pct = (done * 100).checked_div(total).unwrap_or(0);
            format!(
                "{left}  \u{b7}  \u{27f3} {} job{} {pct}%",
                self.jobs.len(),
                if self.jobs.len() == 1 { "" } else { "s" }
            )
        };
        // The mode is never something you have to remember: it sits at the left of
        // the status line in the error colour for as long as it is on.
        let badge = match self.settings.mode {
            Mode::Normal => "",
            Mode::ReadOnly => " READ-ONLY ",
            Mode::Restricted => " RESTRICTED ",
        };
        // An open two-key group is shown the same way, in the accent colour, so a key
        // waiting for its second is never a surprise.
        let group = self.group_badge();
        let badges = badge.chars().count() + group.as_ref().map_or(0, |g| g.chars().count() + 1);
        let left = format!("{}{left}", " ".repeat(badges));
        let right = format!("{} ", self.status);

        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        let avail = area.width as usize;
        // The right half (messages) wins space, but a gap of two is always kept: with
        // both halves long they otherwise ran together, "…back to filestoo small".
        let rw = right.chars().count().min(avail.saturating_sub(3));
        let lw = avail.saturating_sub(rw + 2);
        buf.set_line(
            area.x,
            area.y,
            &Line::from(Span::styled(
                elide_middle(&left, lw),
                Style::new().fg(theme::t().fg),
            )),
            lw as u16,
        );
        buf.set_line(
            area.x + lw as u16,
            area.y,
            &Line::from(Span::styled(
                elide_middle(&right, rw),
                Style::new().fg(theme::t().fg_dim),
            )),
            rw as u16,
        );
        if !badge.is_empty() {
            let th = theme::t();
            frame.buffer_mut().set_line(
                area.x,
                area.y,
                &Line::from(Span::styled(
                    badge,
                    Style::new()
                        .fg(th.bg)
                        .bg(th.err)
                        .add_modifier(Modifier::BOLD),
                )),
                area.width,
            );
        }
        if let Some(g) = group {
            let th = theme::t();
            let x = area.x + badge.chars().count() as u16;
            frame.buffer_mut().set_line(
                x,
                area.y,
                &Line::from(Span::styled(
                    g,
                    Style::new()
                        .fg(th.bg)
                        .bg(th.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                area.width.saturating_sub(x - area.x),
            );
        }
    }

    /// The key to advertise for an action: an F-key if one is bound, since that is
    /// what the bar's layout expects, otherwise whatever is bound first.
    fn footer_key(&self, action: Action) -> Option<String> {
        let keys = self.settings.keymap.keys_for(action);
        keys.iter()
            .find(|k| k.strip_prefix('F').is_some_and(|d| d.parse::<u8>().is_ok()))
            .or(keys.first())
            .cloned()
    }

    fn draw_footer(&mut self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        // Read from the keymap, not written down: after `copy = ["F2"]` a bar that still
        // says "5 Copy" is lying, and this is the line people look at most.
        // In F-key order, as in every orthodox manager. On a narrow screen the ones that
        // do not fit are left off whole, from the end, rather than cut mid-word.
        const KEYS: &[(Action, &str)] = &[
            (Action::Help, "Help"),
            (Action::Rename, "Ren"),
            (Action::View, "View"),
            (Action::Edit, "Edit"),
            (Action::Copy, "Copy"),
            (Action::Move, "Move"),
            (Action::MkDir, "MkDir"),
            (Action::Delete, "Del"),
            (Action::MenuBar, "Menu"),
            (Action::Quit, "Quit"),
        ];
        let mut spans = Vec::new();
        let mut hits = Vec::new();
        let mut cx = area.x;
        for (action, label) in KEYS {
            if self.refused(*action).is_some() {
                continue;
            }
            let Some(key) = self.footer_key(*action) else {
                continue;
            };
            let k = key
                .strip_prefix('F')
                .filter(|d| d.parse::<u8>().is_ok())
                .unwrap_or(&key);
            let w = (k.chars().count() + label.chars().count() + 3) as u16;
            if cx + w > area.x + area.width {
                break;
            }
            hits.push((*action, Rect::new(cx, area.y, w, 1)));
            cx += w;
            spans.push(Span::styled(
                format!(" {k}"),
                Style::new()
                    .fg(theme::t().accent)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(theme::t().fg_dim),
            ));
        }
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().bg(theme::t().bg_bar));
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);
        self.footer_hits = hits;
    }
}

/// `1`-`9` for the first nine entries of a list menu, so they can be picked by typing.
fn digit(i: usize) -> String {
    if i < 9 {
        (i + 1).to_string()
    } else {
        String::new()
    }
}

/// Tab in the Go to box: complete the last part of the path to a directory. One
/// candidate is filled in with a separator after it; several are filled in as far as
/// they agree, and listed under the line.
fn complete_path(i: &mut Input) {
    let text = i.edit.text();
    let cut = text
        .rfind(['/', std::path::MAIN_SEPARATOR])
        .map_or(0, |n| n + 1);
    let (head, part) = text.split_at(cut);
    let base = if head.is_empty() {
        i.dir.clone()
    } else {
        let p = expand_tilde(head);
        if p.is_relative() {
            i.dir.join(p)
        } else {
            p
        }
    };
    let Ok(it) = std::fs::read_dir(&base) else {
        i.error = Some(format!("cannot read {}", base.display()));
        return;
    };
    let mut names: Vec<String> = it
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(part))
        .collect();
    names.sort();
    let Some(first) = names.first() else {
        i.error = Some("no directory starts with that".into());
        return;
    };
    let common: String = if names.len() == 1 {
        format!("{first}{}", std::path::MAIN_SEPARATOR)
    } else {
        let mut c: Vec<char> = first.chars().collect();
        for n in &names[1..] {
            let same = c
                .iter()
                .zip(n.chars())
                .take_while(|(a, b)| **a == *b)
                .count();
            c.truncate(same);
        }
        c.into_iter().collect()
    };
    i.edit = LineEdit::new(&format!("{head}{common}"));
    i.error = (names.len() > 1).then(|| {
        let shown: Vec<&str> = names.iter().take(6).map(String::as_str).collect();
        let more = if names.len() > 6 { " \u{2026}" } else { "" };
        format!("{}{more}", shown.join("  "))
    });
}

/// Standard base64, for OSC 52. Thirty lines rather than a dependency.
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;
    use super::*;

    #[test]
    fn the_logo_is_one_colour_per_cell_of_the_size_about_expects() {
        // Laid over a colour that is none of the logo's, every pixel is either that
        // colour or one of the logo's five: a smoothed edge would blend the two.
        let bg = [255, 0, 255];
        let colours = [
            bg,
            [0x1E, 0x25, 0x30],
            [0xF2, 0x8C, 0x28],
            [0xE6, 0xED, 0xF3],
            [0x2D, 0xB5, 0xA3],
        ];
        let p = logo_picture(LOGO, bg);
        assert_eq!((p.width, p.height), (14, 14));
        let odd = p.rgb.iter().find(|c| !colours.contains(c));
        assert_eq!(odd, None, "the logo has a blended pixel");
        // Each cell's two halves are equal, so About draws no half block anywhere.
        for r in 0..7 {
            let (top, bottom) = (&p.rgb[2 * r * 14..][..14], &p.rgb[(2 * r + 1) * 14..][..14]);
            assert_eq!(top, bottom, "cell row {r} has two colours");
        }
    }

    fn logo_picture(bytes: &[u8], bg: [u8; 3]) -> xdevcmd_core::image::Picture {
        xdevcmd_core::image::png_bytes(bytes, 64, 64, bg).unwrap()
    }

    #[test]
    fn a_session_restores_to_exactly_what_was_saved() {
        let root = std::env::temp_dir().join(format!("xdevcmd-snap-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(a.join("sub")).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        let mut app = App::new(&a, &b, Settings::default(), Outcome::NoFile);
        // Two tabs in the left pane, the second inside sub, sorted by size, reversed.
        app.focused_tabs().unwrap().duplicate();
        app.focused_files().unwrap().set_cwd(&a.join("sub"));
        app.focused_files().unwrap().set_sort(SortKey::Size);
        app.focused_files().unwrap().set_sort(SortKey::Size);
        // A third pane, split off the right one, and the border moved.
        app.focus_next_file();
        app.split_focused(Axis::Rows);
        app.tree.move_boundary(app.focus, Axis::Cols, -10);

        let saved = app.snapshot();
        assert_eq!(saved.panes.pane_count(), 3);
        let mut back = App::from_session(&saved, Settings::default(), Outcome::NoFile).unwrap();
        assert_eq!(back.snapshot(), saved);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn wait_until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(app) {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: {}",
                app.status
            );
            app.tick();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_background_job_finishes_on_its_own_and_one_with_a_question_comes_back() {
        let root = std::env::temp_dir().join(format!("xdevcmd-bg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("new.txt"), "n").unwrap();
        std::fs::write(a.join("clash.txt"), "mine").unwrap();
        std::fs::write(b.join("clash.txt"), "theirs").unwrap();
        let mut app = App::new(&a, &b, Settings::default(), Outcome::NoFile);

        let req = Request::copy(vec![a.join("new.txt")], b.clone());
        app.jobs.push(Job::new(ops::Kind::Copy, ops::start(req)));
        wait_until(&mut app, "the job to finish", |a| a.jobs.is_empty());
        assert_eq!(app.status, "background: 1 copied");
        assert!(app.modal.is_none(), "a clean finish stays out of the way");
        assert!(b.join("new.txt").exists());

        let req = Request::copy(vec![a.join("clash.txt")], b.clone());
        app.jobs.push(Job::new(ops::Kind::Copy, ops::start(req)));
        wait_until(
            &mut app,
            "the question to come forward",
            |a| matches!(&a.modal, Some(Modal::Job(j)) if j.question.is_some()),
        );
        assert!(
            app.jobs.is_empty(),
            "it moved from the background to the front"
        );
        assert_eq!(
            std::fs::read_to_string(b.join("clash.txt")).unwrap(),
            "theirs"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn quitting_with_jobs_running_asks_first() {
        let root = std::env::temp_dir().join(format!("xdevcmd-bgq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut app = App::new(&root, &root, Settings::default(), Outcome::NoFile);
        // A job blocked on a question never finishes by itself.
        let (tx, rx) = std::sync::mpsc::channel();
        let _keep = tx;
        let handle = ops::Handle::for_test(rx);
        app.jobs.push(Job::new(ops::Kind::Copy, handle));
        app.perform(Action::Quit);
        assert!(!app.quit, "not straight out");
        assert!(matches!(&app.modal, Some(Modal::Ask(a)) if a.title == "Quit?"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), want, "{input:?}");
        }
    }
}
