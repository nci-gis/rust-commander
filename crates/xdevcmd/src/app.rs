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
use crate::pane::term::{Decision, ShellState};
use crate::pane::terms::Click as TermClick;
use crate::pane::terms::{ShellKey, ShellSend};
use crate::pane::{FilePane, FileTabs, Pane, TermPanel};
use crate::rename_dialog::{self, RenameDialog};
use crate::search::{self, Search};
use crate::session::{self, Saved, SavedTab, SavedTerminal, Session};
use crate::settings_dialog::{self, SettingsDialog};
use crate::theme;
use crate::viewer::{self, Viewer};

// impl App, by subject: one file for each part of the app.
mod archives;
mod compare;
mod draw;
mod file_ops;
mod jobs;
mod modals;
mod mouse;
mod open;
mod persist;
mod pictures;
mod run;
mod settings;

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
/// A picture for the terminal to draw itself: where, and which decode it is.
#[derive(Debug, Clone, PartialEq)]
struct Graphic {
    rect: Rect,
    key: PicKey,
}

/// What a decoded picture was made for: the file as it was, the pixels it was shrunk
/// to, the box it fits and the protocol its bytes are in. Any change, another decode.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PicKey {
    path: PathBuf,
    px: (u16, u16),
    mtime: Option<std::time::SystemTime>,
    inner: (u16, u16),
    protocol: crate::graphics::Protocol,
}

/// A picture ready to show, made off the drawing thread: decoded, and for the
/// terminal's own protocols encoded for its box, so a frame only writes bytes.
struct Prepared {
    pic: Result<xdevcmd_core::image::Picture, String>,
    /// The cells the encoded picture covers.
    cells: (u16, u16),
    /// The picture in the terminal's protocol (Kitty: the transmission); empty for half
    /// blocks.
    bytes: Vec<u8>,
}

/// Decode the picture `key` names at `w` x `h` pixels and, for the terminal's own
/// protocols, encode it for its box: on a thread, never the drawing one. `cell`: a
/// cell's pixels at the density decoded for.
fn prepare(key: &PicKey, w: usize, h: usize, bg: [u8; 3], cell: (u16, u16)) -> Prepared {
    let pic = xdevcmd_core::image::load(&key.path, w, h, bg).map_err(|e| e.to_string());
    let (bw, bh) = key.inner;
    let (cells, bytes) = match &pic {
        Ok(p) if key.protocol != crate::graphics::Protocol::Blocks && bw > 0 && bh > 0 => {
            let cols = (p.width as u16).div_ceil(cell.0.max(1)).clamp(1, bw);
            let rows = (p.height as u16).div_ceil(cell.1.max(1)).clamp(1, bh);
            let bytes = crate::graphics::encode(key.protocol, p, cols, rows, KITTY_ID);
            ((cols, rows), bytes)
        }
        _ => ((0, 0), Vec::new()),
    };
    Prepared { pic, cells, bytes }
}

/// The terminal's input, read on a thread of its own. crossterm (0.29) must not be read
/// from the main loop: on a hung-up terminal (its window closed, SSH lost) its reader
/// loops forever, at full speed, on the read that fails, and nothing after it runs:
/// not the SIGHUP flag, not the clean-up. Here the main loop waits on a channel, so it
/// goes on drawing (a write to a lost terminal fails, and the loop ends tidily) and looks
/// at the terminal itself meanwhile ([`terminal_alive`]); the stuck thread spins until
/// the process ends. The thread reads only when asked, so it
/// never takes keys meant for an editor run in the meantime.
struct KeyReader {
    ask: Sender<Duration>,
    got: Receiver<std::io::Result<Option<Event>>>,
    /// A read asked for and not answered yet.
    pending: bool,
    /// An event that arrived while waiting for quiet ([`KeyReader::quiet`]).
    held: Option<Event>,
    /// Stdin was the terminal at the start: watched for a hang-up ([`terminal_alive`]).
    watch: bool,
}

impl KeyReader {
    fn new() -> Self {
        let (ask, asked) = mpsc::channel::<Duration>();
        let (tell, got) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("input".into())
            .spawn(move || {
                for wait in asked {
                    let r = event::poll(wait).and_then(|any| {
                        if any {
                            event::read().map(Some)
                        } else {
                            Ok(None)
                        }
                    });
                    if tell.send(r).is_err() {
                        break;
                    }
                }
            });
        Self {
            ask,
            got,
            pending: false,
            held: None,
            watch: std::io::IsTerminal::is_terminal(&std::io::stdin()),
        }
    }

    /// The next event, waiting at most `wait`; an error once the terminal is gone.
    fn next(&mut self, wait: Duration) -> Result<Option<Event>> {
        if let Some(e) = self.held.take() {
            return Ok(Some(e));
        }
        if !self.pending {
            self.ask.send(wait).map_err(|_| input_gone())?;
            self.pending = true;
        }
        // One wait, not slices of it: an idle xdevcmd wakes once a TICK, as it did when
        // it polled crossterm itself. A hung-up terminal is seen within that wait.
        match self.got.recv_timeout(wait) {
            Ok(r) => {
                self.pending = false;
                Ok(r?)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                terminal_alive(self.watch)?;
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(input_gone()),
        }
    }

    /// Wait until the thread is not reading (at most the wait it was given), before
    /// another program takes the terminal; what it read is kept for later.
    fn quiet(&mut self) -> Result<()> {
        while self.pending {
            match self.got.recv_timeout(TICK) {
                Ok(r) => {
                    self.pending = false;
                    if let Some(e) = r? {
                        self.held = Some(e);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => terminal_alive(self.watch)?,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(input_gone()),
            }
        }
        Ok(())
    }
}

/// The input thread ended, which only a panic does: xdevcmd can take no more keys, so it
/// ends (tidily, as for a lost terminal), saying where the panic's details went.
fn input_gone() -> anyhow::Error {
    match crate::crash::take_background().flatten() {
        Some(log) => anyhow::anyhow!(
            "the thread reading the keyboard failed \u{2014} details in {}",
            log.display()
        ),
        None => anyhow::anyhow!("the thread reading the keyboard failed"),
    }
}

/// An error if the terminal (stdin) is hung up. `watch`: stdin was the terminal when
/// the reader started (asked then: a hung-up terminal no longer says it is one). When it
/// was not (crossterm then reads /dev/tty), nothing is known here; SIGHUP and the next
/// write still tell.
#[cfg(unix)]
fn terminal_alive(watch: bool) -> Result<()> {
    if watch && tty_gone(std::io::stdin()) {
        anyhow::bail!("the terminal is gone (hung up)");
    }
    Ok(())
}

/// Whether the terminal at `fd` is hung up. Two signs, either enough:
///
/// - asking for its settings fails with EIO (Linux, the BSDs) or ENXIO;
/// - `poll`, not waiting, answers POLLHUP. macOS needs this one: there `tcgetattr` on
///   a pty whose other side has closed still succeeds (seen on the CI's macOS runner).
///
/// Only POLLHUP counts. macOS's poll does not support some devices (`/dev/tty`) and
/// answers POLLNVAL for them, live or not: taken for a hang-up, it would end xdevcmd
/// under a working terminal. Where poll cannot tell, the next draw's failed write
/// still does. The caller asks only of a terminal (a pipe whose writer closed would
/// say POLLHUP too).
#[cfg(unix)]
fn tty_gone(fd: impl std::os::fd::AsFd) -> bool {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    use rustix::io::Errno;
    let fd = fd.as_fd();
    if matches!(
        rustix::termios::tcgetattr(fd),
        Err(Errno::IO) | Err(Errno::NXIO)
    ) {
        return true;
    }
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    let now = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(poll(&mut fds, Some(&now)), Ok(n) if n > 0)
        && fds[0].revents().contains(PollFlags::HUP)
}

/// Windows: crossterm's console reader reports a closed console as an error itself.
#[cfg(not(unix))]
fn terminal_alive(_watch: bool) -> Result<()> {
    Ok(())
}

/// How long the cursor stays on a picture before it is decoded: going down a directory
/// of photos decodes and sends only the one stopped on.
const SETTLE: Duration = Duration::from_millis(120);

/// The id of quick view's picture in the Kitty protocol, so it can be removed.
const KITTY_ID: u32 = 0x7864;

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
/// Names to type into a new shell at its prompt; see `App::pending_type`.
struct PendingType {
    key: ShellKey,
    text: String,
    paste: bool,
    what: String,
    since: Instant,
}

/// How long a new shell has to show its prompt before names are typed into it.
const PROMPT_WAIT: Duration = Duration::from_secs(4);

const TICK: Duration = Duration::from_millis(40);
/// With nothing known to have changed, the screen is still drawn this often, for what
/// changes without telling (the usage view's clock, a listing's age). Drawing every
/// tick cost about 5 % of a core while idle (2026-11-07); a frame is about 2 ms.
const HEARTBEAT: Duration = Duration::from_secs(1);
/// How long quitting waits for cancelled jobs to clean up before asking again.
const STOP_WAIT: Duration = Duration::from_secs(5);

/// A picture quick view has decoded: the file, the cell size it was shrunk for, the
/// file's modification time then (a changed file is decoded again), and the picture or
/// why there is none.
type Shrunk = (PicKey, Prepared);

/// A picture being decoded on a thread: what it is for, and where the result comes.
type Decoding = (PicKey, Receiver<Prepared>);

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
    /// The viewer while F2's Save as… asks where (and perhaps whether to replace, or a
    /// copy runs): it comes back, where it was, once that dialog closes.
    save_from: Option<Box<Viewer>>,
    /// Save as… was started from a terminal tab's menu, not the viewer: once its
    /// dialogs close, the viewer holding the history is dropped, not shown.
    save_only: bool,
    /// Where the history shown in the viewer came from: its shell's directory where the
    /// system says (Linux), else the file pane's. Save as… offers it.
    history_dir: Option<PathBuf>,
    /// Names waiting to be typed into a new shell once its prompt is up ("Open a new
    /// shell" in the question before typing): that shell, the text, bracketed paste
    /// allowed, what it is, and since when.
    pending_type: Option<PendingType>,
    /// `[terminal] follow`: the shell last looked at, its report count (where reports
    /// are what there is), and the directory it was last seen idle in. The pane moves
    /// only when that directory changes for the same shell: an edge, not a level.
    follow_seen: Option<(ShellKey, u64, Option<PathBuf>)>,
    /// A press inside a shell went to the program there (it asked for the mouse): the
    /// drag and the release that follow go to it too, not to xdevcmd.
    mouse_to_shell: Option<(ShellKey, Rect)>,
    /// Copy, move, delete, extract and pack jobs sent to the background (B in the
    /// progress box). One that asks a question comes back to the front.
    jobs: Vec<Job>,
    /// Quick view's last picture: the file, the size it was shrunk for, the result.
    quick_image: Option<Shrunk>,
    /// A big photo takes a moment to decode: never on the drawing thread.
    decoding: Option<Decoding>,
    /// The picture the cursor is on, and since when, until [`SETTLE`] has passed.
    settle: Option<(PathBuf, Instant)>,
    /// A frame left a picture waiting to settle: the loop keeps drawing at TICK.
    settling: bool,
    /// The Kitty picture the terminal holds (transmitted once, under [`KITTY_ID`]):
    /// placed again after a dialog rather than sent again.
    kitty_held: Option<PicKey>,
    /// How quick view draws pictures: the terminal's own protocol, or half blocks.
    /// [`crate::graphics::effective`] of what was found, `graphics_found`.
    graphics: crate::graphics::Protocol,
    graphics_found: crate::graphics::Protocol,
    /// A terminal cell in pixels, for decoding pictures at the terminal's resolution.
    cell_px: (u16, u16),
    /// Whether `cell_px` is the terminal's word, not a guess: sixel needs it.
    cell_known: bool,
    /// What the terminal answered at start-up about pictures (main.rs asks).
    pub probe: crate::graphics::Probe,
    /// The terminal's input, on its own thread; made by the first turn of the loop.
    input: Option<KeyReader>,
    /// Set by SIGHUP or SIGTERM: the terminal is gone, or someone asked us to end.
    hangup: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The "running in …" note shown at start, cleared by the first key or click.
    boot_note: Option<String>,
    /// Quitting with jobs running: they were cancelled, and until this moment xdevcmd
    /// waits for them to clean up, drawing as usual and taking no keys ([`App::stop_step`]).
    stopping: Option<Instant>,
    /// The picture this frame wants drawn by the terminal, and the one it shows now.
    graphic_want: Option<Graphic>,
    graphic_shown: Option<Graphic>,
    /// What the last frame has in the cells the shown picture covers, to write back
    /// when an iTerm2 or sixel picture (which is cells) is taken away.
    under_graphic: Vec<(u16, u16, ratatui::buffer::Cell)>,
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
const LOGO: &[u8] = include_bytes!("../assets/about-logo.png");

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
            save_from: None,
            save_only: false,
            history_dir: None,
            pending_type: None,
            follow_seen: None,
            mouse_to_shell: None,
            jobs: Vec::new(),
            quick_image: None,
            decoding: None,
            settle: None,
            settling: false,
            kitty_held: None,
            graphics: crate::graphics::Protocol::Blocks,
            graphics_found: crate::graphics::Protocol::Blocks,
            cell_known: false,
            // VT340's cell, which Windows Terminal and xterm also use for sixel.
            cell_px: (10, 20),
            probe: crate::graphics::Probe::default(),
            hangup: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            input: None,
            boot_note: None,
            stopping: None,
            graphic_want: None,
            graphic_shown: None,
            under_graphic: Vec::new(),
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
                let scrollback = self.settings.config.terminal.scrollback;
                match TermPanel::new(&cwd, rows, cols, &shell, scrollback) {
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

    /// Shift+F12: the shell's history and screen in the viewer, held in memory — a
    /// shell's output can hold tokens and customer data, so none of it is written
    /// anywhere unless the user saves it. The focused shell, or from the files the
    /// shell the panel last had in focus; `tab` names another tab's (the tab menu).
    /// The viewer is a dialog over everything, so Esc leaves focus where it was.
    fn view_history(&mut self, tab: Option<usize>) {
        if self.term_id.is_none() {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        }
        let Some(h) = self.panel().and_then(|p| p.history(tab)) else {
            self.status = "no such terminal tab".into();
            return;
        };
        self.history_dir = Some(h.cwd.unwrap_or_else(|| self.focused_cwd()));
        let lines = h.text.lines().count();
        let doc = Document::from_bytes(h.title, h.text.into_bytes());
        let v = &self.settings.config.viewer;
        let mut viewer = Viewer::open(doc, v.wrap, v.tab_width);
        viewer.show_end();
        let note = if h.full_screen {
            // The shell's lines are behind the program's screen, and vt100 keeps no
            // history for that screen: say so rather than count a screenful as history.
            "a full-screen program is running \u{2014} its screen only \u{2014} Esc back".into()
        } else {
            format!(
                "{lines} line{}, in memory only \u{2014} Esc back",
                if lines == 1 { "" } else { "s" }
            )
        };
        viewer.note(&note);
        self.modal = Some(Modal::View(Box::new(viewer)));
    }

    /// Where the tab a menu was opened on is now, or a message that it has gone (its
    /// shells ended while the menu was open).
    fn tab_still_there(&mut self, id: crate::pane::terms::TabId) -> Option<usize> {
        let i = self.panel().and_then(|p| p.tab_index(id));
        if i.is_none() {
            self.status = "that terminal tab is gone".into();
        }
        i
    }

    /// The menu on terminal tab `i` (right-click on it): only what matters (owner,
    /// 2026-11-09), each with the key that does it, so the menu also teaches the keys.
    /// New tab, split and paste are left out (the + button, Alt+N/S/D and the terminal's
    /// own paste cover them), and so is clearing the history, which is destructive and
    /// rarely wanted. Every item acts on the tab clicked, not the focused one.
    fn open_tab_menu(&mut self, i: usize, x: u16, y: u16) {
        let history = self.footer_key(Action::TerminalHistory).unwrap_or_default();
        let save = if history.is_empty() {
            String::new()
        } else {
            format!("{history}, F2")
        };
        let writes = self.refused(Action::CopyAs).is_none();
        // The tab's identity, not its position: shells are reaped every turn while the
        // menu is open, and an earlier tab ending would move this one's position.
        let Some(id) = self.panel().and_then(|p| p.tab_id(i)) else {
            return;
        };
        let items = vec![
            Item::cmd("View history", history, Cmd::TabHistory(id), true),
            Item::cmd(
                "Save history as\u{2026}",
                save,
                Cmd::TabSaveHistory(id),
                writes,
            ),
            Item::sep(),
            Item::cmd("Close tab", "\u{00d7}", Cmd::TabClose(id), true),
        ];
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Terminal {}", i + 1)),
            items,
            (x, y),
        )));
    }

    /// F2 in the viewer: ask where to save what it shows. History (held in memory) is
    /// offered as `xdevcmd-history-<date>-<time>.txt` in its shell's directory; a file
    /// under its own name in its own directory, to be changed. Refused as any other way
    /// of creating a file is, in read-only and restricted modes.
    fn begin_save_as(&mut self) {
        // Saving is creating a file, which is what Copy as… is refused for.
        if let Some(why) = self.refused(Action::CopyAs) {
            if let Some(Modal::View(v)) = self.modal.as_mut() {
                v.note(&why);
            }
            self.status = why;
            return;
        }
        let Some(Modal::View(v)) = self.modal.take() else {
            return;
        };
        let (dir, name) = if v.doc.in_memory {
            (
                self.history_dir
                    .clone()
                    .unwrap_or_else(|| self.focused_cwd()),
                xdevcmd_core::document::history_file_name(),
            )
        } else {
            let p = &v.doc.path;
            (
                p.parent()
                    .map_or_else(|| self.focused_cwd(), Path::to_path_buf),
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        };
        // The prefill is text: a directory whose name is not UTF-8 would come back as
        // another, with U+FFFD in it. The pane's directory is offered instead.
        let dir = if dir.to_str().is_some() {
            dir
        } else {
            self.focused_cwd()
        };
        let what = if v.doc.in_memory {
            "the history, readable by you only"
        } else {
            "a copy of this file"
        };
        self.save_from = Some(v);
        self.modal = Some(Modal::Input(Input {
            title: "Save as".into(),
            label: format!("{what}, to:"),
            edit: LineEdit::new(&dir.join(name).display().to_string()),
            kind: InputKind::SaveAs,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// Enter in Save as…: a document in memory is written by the core
    /// (`document::save_bytes`, `0600`), asking first if the name is taken; a file is
    /// copied by an ordinary job, which asks its own question about a name taken.
    fn apply_save_as(&mut self, i: Input) {
        let typed = i.edit.text();
        let typed = typed.trim();
        if typed.is_empty() {
            return;
        }
        let mut dest = expand_tilde(typed);
        if dest.is_relative() {
            dest = i.dir.join(dest);
        }
        // Under --root, as F7 and Alt+F5: no `..`, and the directory it goes in must be
        // inside, links resolved.
        let climbs = Path::new(typed)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        let inside = dest
            .parent()
            .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
        let refuse = |app: &mut Self, i: Input, why: String| {
            app.modal = Some(Modal::Input(Input {
                error: Some(why),
                ..i
            }));
        };
        if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
            return refuse(self, i, "outside the root xdevcmd is confined to".into());
        }
        // The prefill comes from a file's name or the shell's directory, which someone
        // else may have chosen: a control character in it is refused, as everywhere a
        // name could later be typed into a shell or pasted.
        if let Some(c) = crate::safety::control_char(typed) {
            return refuse(
                self,
                i,
                format!(
                    "the name contains a control character (U+{:04X}) \u{2014} not saved",
                    c as u32
                ),
            );
        }
        if dest.is_dir() {
            return refuse(
                self,
                i,
                "that is a directory \u{2014} add a file name".into(),
            );
        }
        // Said plainly here, before a job is started that would end in the system's
        // own words for it.
        if !dest.parent().is_some_and(Path::is_dir) {
            return refuse(self, i, "that folder does not exist".into());
        }
        let Some(v) = self.save_from.as_ref() else {
            return;
        };
        if let Some(bytes) = v.doc.memory_bytes() {
            let len = bytes.len() as u64;
            match xdevcmd_core::document::save_bytes(&dest, bytes, false, true) {
                Ok(()) => self.status = format!("saved to {}", dest.display()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // The same question a copy asks, before anything is touched.
                    let on_disk = std::fs::metadata(&dest).map_or(0, |m| m.len());
                    self.modal = Some(Modal::Ask(modal::Ask {
                        title: "Already exists".into(),
                        body: vec![
                            dest.display().to_string(),
                            String::new(),
                            format!("on disk   {}", human_size(on_disk)),
                            format!("incoming  {}", human_size(len)),
                        ],
                        danger: true,
                        yes: Some(("Replace".into(), Cmd::SaveReplacing(dest))),
                        buttons: Vec::new(),
                        also: None,
                        footnote: None,
                        pictures: Vec::new(),
                    }));
                }
                Err(e) => refuse(self, i, format!("cannot save: {e}")),
            }
            return;
        }
        let from = v.doc.path.clone();
        let (Some(dir), Some(name)) = (dest.parent(), dest.file_name()) else {
            return refuse(self, i, "not a file name".into());
        };
        let mut req = Request::copy(vec![from], dir.to_path_buf());
        req.rename_to = Some(name.to_os_string());
        self.remember_job(&req);
        self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
    }

    /// Send the active pane's directory to the shell. The one place the two halves of
    /// the program actually talk to each other, and the reason having both is worth it.
    /// Typed through [`App::send_to_shell`], like everything typed into a shell.
    fn cd_terminal_here(&mut self) {
        let cwd = self.focused_cwd();
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        };
        let Some(target) = panel.focused_key() else {
            return;
        };
        // A lossy name would be another directory, or none.
        let Some(text) = cwd.to_str() else {
            self.status = "the directory's name is not UTF-8 \u{2014} not typed".into();
            return;
        };
        let line = match crate::safety::cd_command(&panel.shell_name(), text) {
            Ok(line) => line,
            Err(why) => {
                self.status = why;
                return;
            }
        };
        self.send_to_shell(ShellSend {
            target,
            here: line,
            new_shell: None,
            what: format!("cd {}", cwd.display()),
            dir: cwd,
            paste: false,
        });
    }

    /// Alt+Enter / Ctrl+Enter: the marked names (else the cursor's) typed into the
    /// shell's command line, each quoted for that shell, a space after the last, no
    /// Enter: the user finishes the command. Bare names where the shell is in their
    /// directory, full paths otherwise. Through [`App::send_to_shell`].
    fn type_names(&mut self) {
        let paths = self.selection();
        if paths.is_empty() {
            self.status = "nothing to type \u{2014} the cursor is on ..".into();
            return;
        }
        let pane_dir = self.focused_cwd();
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        };
        let Some(target) = panel.focused_key() else {
            return;
        };
        let shell = panel.shell_name();
        // Read only when a key asks: on macOS this runs lsof.
        let typed = bare_names_dir(panel.shell_dir());
        let build = |dir: Option<&Path>| name_words(&shell, &paths, dir);
        let (here, new_shell) = match (build(typed.as_deref()), build(Some(&pane_dir))) {
            (Ok(h), Ok(n)) => (h, n),
            (Err(why), _) | (_, Err(why)) => {
                self.status = why;
                return;
            }
        };
        let what = match paths.len() {
            1 => here.trim_end().to_string(),
            n => format!("{n} names"),
        };
        self.send_to_shell(ShellSend {
            target,
            here,
            new_shell: Some(new_shell),
            dir: pane_dir,
            paste: true,
            what,
        });
    }

    /// Everything xdevcmd types into a shell comes through here (owner, 2026-11-09), and
    /// one table decides ([`crate::pane::term::decide`]): a shell surely at its prompt
    /// gets it at once (when `[terminal] send_when_idle`, the default); a program in
    /// the terminal, or ssh, gets nothing, and the question offers a new shell instead;
    /// a shell whose state cannot be known (Windows without prompt marks), or an idle
    /// one with the setting off, asks with Send to this shell as well.
    fn send_to_shell(&mut self, s: ShellSend) {
        let Some(state) = self
            .panel_ref()
            .and_then(|p| p.shell(s.target))
            .map(crate::pane::TermPane::state)
        else {
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        };
        let long = s.here.len() > crate::pane::term::LONG_LINE;
        let decision = crate::pane::term::decide_for(
            &state,
            self.settings.config.terminal.send_when_idle,
            s.here.len(),
        );
        if decision == Decision::Send {
            return self.send_here(s);
        }
        let new_shell = if s.new_shell.is_some() {
            format!("Open a new shell in {} and type it there?", tidy(&s.dir))
        } else {
            format!("Open a new shell in {} instead?", tidy(&s.dir))
        };
        let what = format!(
            "To type: {}",
            xdevcmd_core::format::elide_middle(&s.what, 70)
        );
        let (title, mut body) = match &state {
            ShellState::Busy(p) => (
                "A program is running",
                vec![format!(
                    "{p} is running in this terminal \u{2014} xdevcmd does not type into a program."
                )],
            ),
            ShellState::Remote(p) => (
                "The shell is on another machine",
                vec![format!(
                    "{p} is running in this terminal \u{2014} the shell there is on another machine."
                )],
            ),
            ShellState::Unknown => (
                "Type into the shell?",
                vec![
                    "xdevcmd cannot tell whether a program is running in this shell.".into(),
                    "docs/shell.md shows the prompt snippet that lets it tell.".into(),
                ],
            ),
            ShellState::Idle if !long => (
                "Type into the shell?",
                vec![
                    "The shell is at its prompt.".into(),
                    "Settings \u{2192} Terminal can send without asking.".into(),
                ],
            ),
            ShellState::Idle => ("Type into the shell?", vec!["The shell is at its prompt.".into()]),
        };
        if long {
            body.push(format!(
                "It is {} bytes \u{2014} some shells drop a line over 4 KB without a word.",
                s.here.len()
            ));
        }
        body.push(String::new());
        body.push(what);
        body.push(new_shell);
        let s = Box::new(s);
        let (yes, also) = match decision {
            Decision::AskNewShell => (("open a new shell".to_string(), Cmd::ShellNew(s)), None),
            _ => {
                let here = s.clone();
                (
                    ("open a new shell".to_string(), Cmd::ShellNew(s)),
                    Some((
                        's',
                        "send to this shell".to_string(),
                        Cmd::ShellSendHere(here),
                    )),
                )
            }
        };
        self.modal = Some(Modal::Ask(modal::Ask {
            title: title.into(),
            body,
            danger: false,
            yes: Some(yes),
            also,
            buttons: Vec::new(),
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    /// Type `s` into its shell now, if that shell is still there.
    fn send_here(&mut self, s: ShellSend) {
        let Some(t) = self.panel_ref().and_then(|p| p.shell(s.target)) else {
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        };
        t.type_text(&s.here, s.paste);
        self.status = format!("shell: {}", s.what);
    }

    /// "Open a new shell": a new terminal tab started in the pane's directory. For a
    /// `cd` that is the whole job; names are typed into it once its prompt is up
    /// (`pending_type`, in `background`), or not at all if it does not come up in time.
    fn open_shell_for(&mut self, s: ShellSend) {
        if self.term_id.is_none() {
            self.status = "that shell is gone \u{2014} nothing typed".into();
            return;
        }
        if !self.term_shown {
            let full = self.screen;
            self.toggle_term(full);
        }
        if !self.terminal_room() {
            return;
        }
        let key = match self.panel().map(|p| p.add_tab(&s.dir)) {
            Some(Ok(k)) => k,
            Some(Err(e)) => {
                self.status = format!("no shell: {e}");
                return;
            }
            None => return,
        };
        match s.new_shell {
            None => self.status = format!("a new shell in {}", tidy(&s.dir)),
            Some(text) => {
                // One wait at a time: an earlier one still waiting is dropped, and said.
                let dropped = match self.pending_type.take() {
                    Some(p) => format!(" ({} for the earlier new shell: not typed)", p.what),
                    None => String::new(),
                };
                self.status = format!(
                    "a new shell in {} \u{2014} typing at its prompt{dropped}",
                    tidy(&s.dir)
                );
                self.pending_type = Some(PendingType {
                    key,
                    text,
                    paste: s.paste,
                    what: s.what,
                    since: Instant::now(),
                });
            }
        }
    }

    /// Names waiting for a new shell's prompt: typed once it is up, dropped (and said)
    /// if the shell ends or the prompt does not come in time. True when it said so.
    fn pump_pending_type(&mut self) -> bool {
        let Some(p) = self.pending_type.take() else {
            return false;
        };
        let ready = self
            .panel_ref()
            .and_then(|panel| panel.shell(p.key))
            .map(|t| (t.prompt_ready(), t));
        match ready {
            None => {
                self.status = "the new shell ended before its prompt \u{2014} nothing typed".into();
            }
            Some((true, t)) => {
                t.type_text(&p.text, p.paste);
                self.status = format!("new shell: {}", p.what);
            }
            Some((false, _)) if p.since.elapsed() >= PROMPT_WAIT => {
                self.status = "a new shell is open, but its prompt did not show in time \u{2014} \
                               nothing typed (docs/shell.md)"
                    .into();
            }
            Some((false, _)) => {
                self.pending_type = Some(p);
                return false;
            }
        }
        true
    }

    /// `[terminal] follow`: when the focused shell's directory changes while it is at
    /// its prompt, the file pane used last goes there. Edge-triggered: the pane going
    /// somewhere else (Backspace) changes nothing until the shell's next `cd`. Never
    /// the other way. Waits while a dialog or a quick search is open; while the pane
    /// shows an archive or find results the change is let pass, not kept for later.
    /// True when it moved a pane.
    fn follow_shell(&mut self) -> bool {
        if !self.settings.config.terminal.follow {
            self.follow_seen = None;
            return false;
        }
        if self.modal.is_some() || self.drag.is_some() || self.search.is_some() {
            return false;
        }
        let seen = self.follow_seen.take();
        let Some(panel) = self.panel_ref() else {
            return false;
        };
        let Some(key) = panel.focused_key() else {
            return false;
        };
        let Some(shell) = panel.shell(key) else {
            return false;
        };
        let (mut seq, last) = match seen {
            Some((k, seq, last)) if k == key => (seq, Some(last)),
            // Another shell (a tab switched, a new one): only learn where it is.
            _ => (0, None),
        };
        let now = shell.idle_dir(&mut seq);
        let Some(dir) = now else {
            self.follow_seen = Some((key, seq, last.flatten()));
            return false;
        };
        self.follow_seen = Some((key, seq, Some(dir.clone())));
        match last {
            Some(Some(was)) if was != dir => {}
            _ => return false,
        }
        if !dir.is_dir() {
            return false;
        }
        let canonical = xdevcmd_core::paths::canonical(&dir).unwrap_or_else(|_| dir.clone());
        if !xdevcmd_core::policy::get().allows(&canonical) {
            return false;
        }
        let Some(f) = self
            .last_files_id()
            .and_then(|id| self.panes.get_mut(&id))
            .and_then(Pane::as_files_mut)
        else {
            return false;
        };
        if f.archive.is_some() || f.results.is_some() || f.cwd == canonical {
            return false;
        }
        f.set_cwd(&canonical);
        self.status = format!("following the shell: {}", tidy(&canonical));
        true
    }

    /// A press inside a shell, reported to the program there when it asked for the
    /// mouse (vim, htop, Claude Code); its drag and release then follow it there.
    fn press_into_shell(&mut self, x: u16, y: u16, button: u32, mods: KeyModifiers) {
        use crate::pane::term::MouseEv;
        let ev = MouseEv::Press(button | mouse_mods(mods));
        self.mouse_to_shell = self.panel_ref().and_then(|p| p.mouse_at(x, y, ev));
    }

    /// The terminal panel, to read from.
    fn panel_ref(&self) -> Option<&TermPanel> {
        match self.panes.get(&self.term_id?) {
            Some(Pane::Term(p)) => Some(p),
            _ => None,
        }
    }

    /// The file pane used last: the focused one, else the one focused before, else
    /// the first (focus starting in the files and going straight to the shell leaves
    /// no file pane in `mru`).
    fn last_files_id(&self) -> Option<PaneId> {
        let files = self.file_panes();
        std::iter::once(self.focus)
            .chain(self.mru.iter().copied())
            .find(|id| files.contains(id))
            .or_else(|| files.first().copied())
    }

    /// Alt+G: the file pane goes to the shell's directory, the reverse of Alt+C. It only
    /// reads, nothing is typed; still refused while a program runs in the shell (its
    /// directory is the shell's, not the program's, and may not be what is meant) or
    /// when that program is ssh (the shell that matters is on another machine).
    fn go_shell_dir(&mut self) {
        let Some(panel) = self.panel_ref() else {
            let key = self.footer_key(Action::FocusTerminal).unwrap_or_default();
            self.status = format!("no terminal yet \u{2014} {key}");
            return;
        };
        match panel.shell_state() {
            ShellState::Busy(p) => {
                self.status = format!(
                    "{p} is running in the shell \u{2014} its directory is read at the prompt"
                );
                return;
            }
            ShellState::Remote(p) => {
                self.status = format!(
                    "{p} is running \u{2014} the shell's directory there is on another machine"
                );
                return;
            }
            // Reading types nothing, so a shell whose state is unknown may be read.
            ShellState::Idle | ShellState::Unknown => {}
        }
        // A reported directory (OSC 7, 9;9) may be used here: it only moves a pane.
        let Some(dir) = panel.shell_dir().map(|d| d.path) else {
            self.status = "the shell's directory is not known \u{2014} see docs/shell.md".into();
            return;
        };
        if !dir.is_dir() {
            self.status = format!("the shell's directory {} is not there any more", tidy(&dir));
            return;
        }
        let canonical = xdevcmd_core::paths::canonical(&dir).unwrap_or_else(|_| dir.clone());
        if !xdevcmd_core::policy::get().allows(&canonical) {
            self.status = "the shell's directory is outside the root xdevcmd is confined to".into();
            return;
        }
        let Some(f) = self
            .last_files_id()
            .and_then(|id| self.panes.get_mut(&id))
            .and_then(Pane::as_files_mut)
        else {
            return;
        };
        if f.cwd == canonical && f.archive.is_none() && f.results.is_none() {
            self.status = "already in the shell's directory".into();
            return;
        }
        f.set_cwd(&canonical);
        self.status = format!("the shell's directory: {}", tidy(&canonical));
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
    /// Inside the shell only the keys that work there are offered, and the rest of
    /// what the prefix does is said in words.
    fn group_hint(&self, first: crate::keys::Chord) -> String {
        let in_shell = self.focus_is_term();
        let keys: Vec<String> = self
            .settings
            .keymap
            .group(first)
            .iter()
            .filter(|(_, a)| !in_shell || a.is_global())
            .map(|((c, m), _)| crate::keys::key_name(*c, *m))
            .collect();
        // A group whose keys all repeat is a mode, left with Enter or Esc; any other
        // is a key to pick, after which it closes by itself.
        if self.group_repeats(first) {
            format!("{} \u{b7} Enter or Esc: done", keys.join(" "))
        } else if in_shell {
            format!(
                "then: {} \u{b7} a terminal key for this shell \u{b7} Esc: cancel",
                keys.join(" ")
            )
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
        // Inside the shell only Esc cancels: Enter is the program's like any other key
        // xdevcmd does not use there, so it goes on after the prefix.
        if self.focus_is_term() && k.code != KeyCode::Esc {
            return self.in_shell_prefix(first, k);
        }
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

    /// The key after the prefix (Alt+X) inside the shell. One of the prefix's own keys
    /// that works in the shell (F12, for the history) runs; a key whose action acts on
    /// the shell (Alt+S, Alt+W, Alt+1…) runs on it; anything else was the program's all
    /// along, so the prefix and the key both go to it, as if xdevcmd had not looked.
    /// Always `true`: in the shell no key falls through to a second meaning.
    fn in_shell_prefix(&mut self, first: crate::keys::Chord, k: KeyEvent) -> bool {
        self.status.clear();
        let keymap = &self.settings.keymap;
        let run = keymap
            .in_group(first, k)
            .filter(|a| a.is_global())
            .or_else(|| {
                keymap
                    .action_for(k)
                    .filter(|a| a.is_global() || a.acts_on_shell())
            });
        if let Some(a) = run {
            self.run_bound(a, k);
        } else if let Some(Pane::Term(t)) = self.panes.get(&self.focus) {
            t.send_key(KeyEvent::new(first.0, first.1));
            t.send_key(k);
        }
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
        let prefix = self
            .settings
            .keymap
            .shell_prefix()
            .map(|c| format!("{} ", crate::keys::key_name(c.0, c.1)))
            .unwrap_or_default();
        self.status = if whose == "this file pane" && self.term_shown {
            format!(
                "no tab {n}: {whose} has {has} \u{2014} F12 to the terminal, then {prefix}Alt+{n} picks its tabs"
            )
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
        // The first key of a group. In the shell only the group holding F12's two-key
        // forms opens (Alt+X, for Alt+X F12); it is xdevcmd's prefix there. Any other
        // group's first key is the shell's (Alt+R is readline's revert-line).
        if self.settings.keymap.starts_group(k) {
            let first = crate::keys::Keymap::chord(k);
            let prefix = self
                .settings
                .keymap
                .group(first)
                .iter()
                .any(|(_, a)| a.is_global());
            if !self.focus_is_term() || prefix {
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

        // F12's family works from anywhere, the shell included (keymap A, owner
        // 2026-11-09). Everything else typed into the shell belongs to the shell.
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
                        | Action::TerminalHistory
                        | Action::ToggleTerminal
                        | Action::CdTerminalHere
                        | Action::GoShellDir
                        | Action::TypeNames
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
                    | Action::TypeNames
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
            Action::ContextMenu => self.context_menu_at_cursor(),
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
            Action::TerminalHistory => self.view_history(None),
            Action::ToggleTerminal => self.toggle_term(full),
            Action::CdTerminalHere => self.cd_terminal_here(),
            Action::GoShellDir => self.go_shell_dir(),
            Action::TypeNames => self.type_names(),
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
            Cmd::ShellNew(s) => self.open_shell_for(*s),
            Cmd::ShellSendHere(s) => self.send_here(*s),
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
                        // Empty in screenshots: no terminal was asked.
                        if self.usage.terminal.is_empty() {
                            String::new()
                        } else {
                            format!("Running in {}", self.usage.terminal)
                        },
                        // xdevcmd never looks for updates itself (no network): where to.
                        format!("Releases: {}/releases", env!("CARGO_PKG_REPOSITORY")),
                    ],
                    danger: false,
                    yes: None,
                    buttons: Vec::new(),
                    also: None,
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
                // its half-written file, which is what makes quitting now safe. The wait
                // is the main loop's (`stop_step`), so the screen says what is going on.
                for j in &self.jobs {
                    j.handle.cancel();
                }
                self.stopping = Some(Instant::now() + STOP_WAIT);
                self.stop_step();
            }
            Cmd::QuitNow => {
                self.quit = true;
                self.quit_cd = true;
            }
            Cmd::Resume(it) => self.resume(it),
            Cmd::Sync(pairs) => {
                let req = Request::sync(pairs);
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot("a synchronisation".into()));
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            Cmd::CopyText(t) => self.copy_text(t),
            Cmd::TabHistory(id) => {
                if let Some(i) = self.tab_still_there(id) {
                    self.view_history(Some(i));
                }
            }
            Cmd::TabSaveHistory(id) => {
                // The history goes into a viewer that is never shown: Save as… holds it
                // and lets it go when its dialogs close.
                if let Some(i) = self.tab_still_there(id) {
                    self.view_history(Some(i));
                    if matches!(self.modal, Some(Modal::View(_))) {
                        self.begin_save_as();
                        self.save_only = self.save_from.is_some();
                    }
                }
            }
            Cmd::TabClose(id) => {
                if let Some(i) = self.tab_still_there(id) {
                    let nothing_left = self.panel().is_some_and(|p| p.close_tab(i));
                    if nothing_left {
                        self.remove_terminal_panel();
                    }
                }
            }
            Cmd::SaveReplacing(dest) => {
                let saved = self
                    .save_from
                    .as_ref()
                    .and_then(|v| v.doc.memory_bytes())
                    .map(|b| xdevcmd_core::document::save_bytes(&dest, b, true, true));
                self.status = match saved {
                    Some(Ok(())) => format!("saved to {}, replacing it", dest.display()),
                    Some(Err(e)) => format!("cannot save: {e}"),
                    None => "nothing to save".into(),
                };
            }
            Cmd::BatchRename(moves) => {
                let out = xdevcmd_core::batch_rename::apply(&moves, xdevcmd_core::ops::safe());
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

    // ── file operations ────────────────────────────────────────────────────────────

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

    // ── search, navigation, bookmarks ──────────────────────────────────────────────

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

    // ── small file operations, checksum, user menu ─────────────────────────────────

    // ── background jobs ────────────────────────────────────────────────────────────

    // ── compare and synchronise ────────────────────────────────────────────────────

    // ── archives ───────────────────────────────────────────────────────────────────

    // ── menu bar, usage ────────────────────────────────────────────────────────────

    // ── drawing ────────────────────────────────────────────────────────────────────

    /// The key to advertise for an action: an F-key if one is bound, since that is
    /// what the bar's layout expects, otherwise whatever is bound first.
    fn footer_key(&self, action: Action) -> Option<String> {
        let keys = self.settings.keymap.keys_for(action);
        keys.iter()
            .find(|k| k.strip_prefix('F').is_some_and(|d| d.parse::<u8>().is_ok()))
            .or(keys.first())
            .cloned()
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

/// Standard base64, for OSC 52 and the terminal's own pictures (graphics.rs). Thirty
/// lines rather than a dependency.
pub(crate) fn base64(data: &[u8]) -> String {
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

/// "s" for a count other than one.
/// The directory typed names may be bare in: the shell's, only when the system gave it.
/// One the shell reported about itself could be forged by any program's output, and
/// bare names typed against a forged directory would name other files.
fn bare_names_dir(d: Option<crate::pane::term::ShellDir>) -> Option<PathBuf> {
    let d = d.filter(|d| d.trusted)?.path;
    Some(xdevcmd_core::paths::canonical(&d).unwrap_or(d))
}

/// `paths` as words for `shell`: bare names when all are in `dir`, else full paths.
fn name_words(shell: &str, paths: &[PathBuf], dir: Option<&Path>) -> Result<String, String> {
    let bare = dir.is_some_and(|d| all_in(paths, d));
    let mut names = Vec::with_capacity(paths.len());
    for p in paths {
        let shown = if bare {
            p.file_name().map(Path::new).unwrap_or(p)
        } else {
            p.as_path()
        };
        // A lossy name would be another file, or none.
        names.push(
            shown
                .to_str()
                .ok_or_else(|| "a name is not UTF-8 \u{2014} not typed".to_string())?,
        );
    }
    crate::safety::words(shell, &names)
}

/// Whether every one of `paths` is directly in `dir`, the same directory by another
/// name included. The shell's directory is the system's answer, with every link
/// resolved; a pane's path keeps the links the user went through. On macOS the
/// temporary directory itself is one (`/var` is `/private/var`), so a pane in it and
/// a shell started there name the same directory differently, and comparing the text
/// alone typed full paths at a shell already there.
fn all_in(paths: &[PathBuf], dir: &Path) -> bool {
    let mut parents = paths.iter().map(|p| p.parent());
    let Some(Some(first)) = parents.next() else {
        return false;
    };
    if !parents.all(|q| q == Some(first)) {
        return false;
    }
    first == dir
        || xdevcmd_core::paths::canonical(first)
            .is_ok_and(|f| xdevcmd_core::paths::canonical(dir).is_ok_and(|d| f == d))
}

/// Shift, Alt and Ctrl as a mouse report carries them: 4, 8 and 16 added to the button.
fn mouse_mods(m: KeyModifiers) -> u32 {
    let mut b = 0;
    if m.contains(KeyModifiers::SHIFT) {
        b |= 4;
    }
    if m.contains(KeyModifiers::ALT) {
        b |= 8;
    }
    if m.contains(KeyModifiers::CONTROL) {
        b |= 16;
    }
    b
}

/// A mouse button as a terminal numbers it in a report.
fn button_number(b: MouseButton) -> u32 {
    match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::base64;
    use super::*;

    /// Bare names only against a directory the system gave: a reported one (OSC 7 or
    /// 9;9, forgeable by any output) gets full paths even when it equals theirs.
    #[test]
    fn typed_names_are_bare_only_in_a_directory_the_system_gave() {
        use crate::pane::term::ShellDir;
        let dir = std::env::temp_dir().join("xdevcmd-bare-names");
        let paths = vec![dir.join("a b"), dir.join("c")];
        let words =
            |d: Option<ShellDir>| name_words("bash", &paths, bare_names_dir(d).as_deref()).unwrap();
        // Not created on disk, so canonical() leaves the path as it is.
        let trusted = ShellDir {
            path: dir.clone(),
            trusted: true,
        };
        let reported = ShellDir {
            path: dir.clone(),
            trusted: false,
        };
        assert_eq!(words(Some(trusted)), "'a b' 'c' ");
        let full = words(Some(reported));
        assert!(full.contains("xdevcmd-bare-names"), "{full}");
        assert_eq!(full, words(None));
    }

    /// A pane reached through a link and a shell in the same directory (which the system
    /// names with the link resolved, as macOS does for its temporary directory) get
    /// bare names; another directory still gets full paths.
    #[cfg(unix)]
    #[test]
    fn typed_names_are_bare_in_the_same_directory_by_another_name() {
        let top = std::env::temp_dir().join(format!("xdevcmd-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let real = top.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = top.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let shell_dir = xdevcmd_core::paths::canonical(&real).unwrap();
        let paths = vec![link.join("a b")];
        assert_eq!(
            name_words("bash", &paths, Some(&shell_dir)).unwrap(),
            "'a b' "
        );
        let elsewhere = name_words("bash", &paths, Some(&top)).unwrap();
        assert!(elsewhere.contains("link"), "{elsewhere}");
        let _ = std::fs::remove_dir_all(&top);
    }

    /// A live terminal is not taken for a lost one, and a hung-up one is seen: the check
    /// must never end xdevcmd under a working terminal (as `poll`'s POLLNVAL might on
    /// macOS), and must notice when the other side has closed.
    #[cfg(unix)]
    #[test]
    fn a_terminal_is_gone_only_once_its_other_side_closes() {
        use rustix::fs::{open, Mode, OFlags};
        use rustix::pty::{grantpt, openpt, ptsname, unlockpt, OpenptFlags};
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
        grantpt(&master).unwrap();
        unlockpt(&master).unwrap();
        let name = ptsname(&master, Vec::new()).unwrap();
        let slave = open(
            name.as_c_str(),
            OFlags::RDWR | OFlags::NOCTTY,
            Mode::empty(),
        )
        .unwrap();
        assert!(!tty_gone(&slave), "a live terminal");
        drop(master);
        assert!(tty_gone(&slave), "its other side closed");
        // Not a terminal at all is not "gone": the caller watches only a terminal.
        assert!(!tty_gone(
            std::fs::File::open(std::env::current_exe().unwrap()).unwrap()
        ));
    }

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
