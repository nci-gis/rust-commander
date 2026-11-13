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
use crate::commands;
use crate::config::{Mode, Outcome, Settings};
use crate::find_dialog::{self, FindDialog};
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
mod input;
mod jobs;
mod lists;
mod modals;
mod mouse;
mod open;
mod palette;
mod panes;
mod persist;
mod pictures;
mod run;
mod settings;
mod shell;

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
    /// Enter on a row of F1: run it on what had focus.
    Palette(usize),
    /// Enter, F3 or F4 on a path in a shell's history.
    Link(viewer::LinkAct, crate::links::Link),
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
    rx: Receiver<Option<std::io::Result<Vec<xdevcmd_core::compare::Diff>>>>,
}

/// What undo says while a delete to the trash is still running.
const TRASH_RUNNING: &str = "a delete to the trash that is still running";
/// What undo says while a copy is still running.
const COPY_RUNNING: &str = "a copy that is still running";
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

/// How many status messages Messages (Alt+M) keeps.
const MESSAGES: usize = 50;

pub struct App {
    tree: Node,
    panes: BTreeMap<PaneId, Pane>,
    focus: PaneId,
    next_id: PaneId,
    rects: Vec<(PaneId, Rect)>,
    term_id: Option<PaneId>,
    term_shown: bool,
    status: String,
    /// The status line's past (item 46): each message as it was first shown, with its
    /// time, the newest last; at most [`MESSAGES`].
    messages: std::collections::VecDeque<(std::time::SystemTime, String)>,
    /// The status as last recorded in `messages`, so a message is kept once.
    noted: String,
    quit: bool,
    last_click: Option<(Instant, u16, u16)>,
    modal: Option<Modal>,
    /// Set by F4. Handled in `run`, which is the only place that can hand the real
    /// terminal over to another program and take it back.
    pending_edit: Option<PathBuf>,
    /// The line (and column) F4 on a path in a shell's history asks the editor for.
    edit_at: Option<(u32, Option<u32>)>,
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
    /// F1's recently run commands, most recent first, kept across restarts.
    recent_commands: Vec<String>,
    /// The theme in use, by name (View → Theme… marks it).
    theme_name: String,
    /// The last operation that wrote to the disk, for Ctrl+Z.
    last_undo: Option<xdevcmd_core::undo::Undo>,
    /// The sources of a delete to the trash still running, so its result can become
    /// the undo once it has said where everything went.
    trash_pending: Option<Vec<PathBuf>>,
    /// A copy still running, as undo will name it ("copy of a.txt"): once it ends, what
    /// it made becomes the undo (Round 031).
    copy_pending: Option<String>,
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
    /// A shell's history, while F3 on a path in it views that file: it comes back,
    /// where it was, when that viewer closes.
    return_to: Option<Box<Viewer>>,
    /// `P` in a shell's history: the paths waiting for the menu's answer.
    pending_list: Option<crate::viewer::ListRequest>,
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
    /// How long start-up took, step by step ([`crate::startup`]); `timings`: said on
    /// the status line too (`--timings`).
    pub startup: crate::startup::Clock,
    pub timings: bool,
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
        let notice = outcome.message().or_else(|| settings.theme_note.clone());
        let settings_height = settings.config.terminal.height_percent;
        let theme_name = settings.theme_name.clone();
        let recent_commands = if settings.config.general.restore_session {
            commands::load_recent(&settings.config.menu)
        } else {
            Vec::new()
        };
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
            messages: std::collections::VecDeque::new(),
            noted: String::new(),
            quit: false,
            last_click: None,
            modal: None,
            pending_edit: None,
            edit_at: None,
            settings,
            config_outcome: outcome,
            drag: None,
            mru: Vec::new(),
            zoom: None,
            resizing: None,
            scrolling: None,
            group: None,
            option_hinted: false,
            recent_commands,
            theme_name,
            last_undo: None,
            trash_pending: None,
            copy_pending: None,
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
            return_to: None,
            pending_list: None,
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
            startup: Default::default(),
            timings: false,
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
