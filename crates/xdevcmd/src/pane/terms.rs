//! The terminal panel: tabs of shells, and each tab its own split layout.
//!
//! A tab reuses the same [`Node`] tree the file panes are laid out with, so splitting
//! side by side or top and bottom, closing, and dragging a boundary to resize all work
//! inside a tab without a second implementation of any of it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use std::sync::Arc;

use xdevcmd_core::termlog::{self, Log};

use crate::layout::{Axis, Divider, Node, PaneId};
use crate::pane::TermPane;
use crate::theme;

struct Tab {
    /// Stays with the tab while others open and close around it, unlike its position:
    /// what a tab's menu holds, so a tab that ends while the menu is open cannot turn
    /// the menu onto its neighbour.
    id: TabId,
    tree: Node,
    terms: BTreeMap<PaneId, TermPane>,
    focus: PaneId,
}

impl Tab {
    fn label(&self) -> String {
        let l = self
            .terms
            .get(&self.focus)
            .map_or_else(|| "shell".to_string(), TermPane::tab_label);
        // A deep directory name must not push the + button off the bar.
        xdevcmd_core::format::elide_middle(&l, 16)
    }

    fn leaves(&self) -> Vec<PaneId> {
        let mut v = Vec::new();
        self.tree.leaves(&mut v);
        v
    }

    /// What the tab bar shows for this tab's log, if it has one.
    fn log_mark(&self) -> Option<LogMark> {
        self.logged().map(|(_, l)| match l.status() {
            termlog::Status::Failed(_) => LogMark::Failed,
            _ => LogMark::Running,
        })
    }

    /// The shell in this tab whose output is being logged, and its log. One at most.
    fn logged(&self) -> Option<(PaneId, Arc<Log>)> {
        self.terms
            .iter()
            .find_map(|(id, t)| t.log().map(|l| (*id, l)))
    }
}

/// The mark on a logged tab: ● running, ! failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogMark {
    Running,
    Failed,
}

/// How long ending a log waits for its writer: a hung disk must not hang a close or
/// quitting.
const LOG_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// What a click in the panel asked for. Opening a tab needs the file pane's directory,
/// which only the app knows, so that one is handed back rather than done here.
pub enum Click {
    Nothing,
    NewTab,
    CloseTab(usize),
    Selected,
}

/// A shell's text for the viewer (Shift+F12).
pub struct History {
    pub title: String,
    pub text: String,
    /// The shell's directory, where the system says.
    pub cwd: Option<std::path::PathBuf>,
    /// A full-screen program has the other screen up, so `text` is that screen only.
    pub full_screen: bool,
    /// The commands the shell reported: the line of `text` each one's output starts
    /// on, and the directory it ran in. Empty for a shell that sends no prompt marks.
    pub commands: Vec<(usize, Option<std::path::PathBuf>)>,
    /// The shell's directory now, however it is known: the base for the paths in its
    /// output where no command says better.
    pub dir_now: Option<std::path::PathBuf>,
    /// A program on another machine running in that shell (ssh), if any.
    pub remote: Option<String>,
    /// "sh · tab 1": which shell, for a list's source.
    pub label: String,
}

/// A terminal tab's identity, never reused within a panel.
pub type TabId = u64;

/// One shell: its tab and its place in the tab. Kept by a question about typing into
/// it, so the answer goes to that shell or to none, never to whichever has focus then.
pub type ShellKey = (TabId, PaneId);

/// Something an action types into a shell, through `App::send_to_shell`.
#[derive(Clone)]
pub struct ShellSend {
    pub target: ShellKey,
    /// Typed into `target`.
    pub here: String,
    /// Typed into a new shell once its prompt is up; `None` when starting one in
    /// `dir` is the whole job (Alt+C).
    pub new_shell: Option<String>,
    /// Where a new shell starts: the file pane's directory.
    pub dir: std::path::PathBuf,
    /// Words, not a command line: wrapped in bracketed paste where the shell asked.
    pub paste: bool,
    /// What it does, for messages: "cd /tmp", "2 names".
    pub what: String,
}

pub struct TermPanel {
    tabs: Vec<Tab>,
    next_tab: TabId,
    active: usize,
    next_id: PaneId,
    shell: String,
    /// Lines of history a shell started from now on keeps (`[terminal] scrollback`).
    scrollback: usize,
    // Where things were drawn, for clicks.
    tab_hits: Vec<(usize, Rect)>,
    close_hits: Vec<(usize, Rect)>,
    new_hit: Rect,
    rects: Vec<(PaneId, Rect)>,
    body: Rect,
    /// What ending logs had to say (a shell exited, a tab closed), for the status line.
    notes: Vec<String>,
}

impl TermPanel {
    pub fn new(cwd: &Path, rows: u16, cols: u16, shell: &str, scrollback: usize) -> Result<Self> {
        let mut p = Self {
            tabs: Vec::new(),
            next_tab: 0,
            active: 0,
            next_id: 0,
            shell: shell.to_string(),
            scrollback,
            tab_hits: Vec::new(),
            close_hits: Vec::new(),
            new_hit: Rect::ZERO,
            rects: Vec::new(),
            body: Rect::new(0, 0, cols + 2, rows + 2),
            notes: Vec::new(),
        };
        p.add_tab(cwd)?;
        Ok(p)
    }

    fn spawn(&mut self, cwd: &Path) -> Result<(PaneId, TermPane)> {
        // Start at the size the body last had; the first draw corrects it anyway.
        let rows = self.body.height.saturating_sub(2).max(1);
        let cols = self.body.width.saturating_sub(2).max(1);
        let t = TermPane::spawn(cwd, rows, cols, &self.shell, self.scrollback)?;
        let id = self.next_id;
        self.next_id += 1;
        Ok((id, t))
    }

    /// A tab running one program by its argv, kept after it ends; it becomes the
    /// active tab.
    pub fn add_program_tab(&mut self, cwd: &Path, argv: &[std::ffi::OsString]) -> Result<()> {
        let rows = self.body.height.saturating_sub(2).max(1);
        let cols = self.body.width.saturating_sub(2).max(1);
        let t = TermPane::spawn_argv(cwd, rows, cols, argv, self.scrollback)?;
        let id = self.next_id;
        self.next_id += 1;
        let mut terms = BTreeMap::new();
        terms.insert(id, t);
        let tab_id = self.take_tab_id();
        self.tabs.push(Tab {
            id: tab_id,
            tree: Node::Leaf(id),
            terms,
            focus: id,
        });
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    fn take_tab_id(&mut self) -> TabId {
        self.next_tab += 1;
        self.next_tab
    }

    /// The identity of the tab at position `i`.
    pub fn tab_id(&self, i: usize) -> Option<TabId> {
        self.tabs.get(i).map(|t| t.id)
    }

    /// Where tab `id` is now; `None` once it has gone.
    pub fn tab_index(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// Settings changed `[terminal] scrollback`: shells started from now on keep this
    /// many lines; those running keep theirs (vt100 cannot resize a history).
    pub fn set_scrollback(&mut self, lines: usize) {
        self.scrollback = lines;
    }

    /// The focused pane is a program run in a tab that has ended: Enter or Esc closes it.
    pub fn focused_program_ended(&self) -> bool {
        self.focused().is_some_and(|t| t.keeps() && t.has_exited())
    }

    /// A new tab with a shell in `cwd`, made the active tab; the new shell's key.
    pub fn add_tab(&mut self, cwd: &Path) -> Result<ShellKey> {
        let (id, t) = self.spawn(cwd)?;
        let mut terms = BTreeMap::new();
        terms.insert(id, t);
        let tab_id = self.take_tab_id();
        self.tabs.push(Tab {
            id: tab_id,
            tree: Node::Leaf(id),
            terms,
            focus: id,
        });
        self.active = self.tabs.len() - 1;
        Ok((tab_id, id))
    }

    /// The focused shell's key.
    pub fn focused_key(&self) -> Option<ShellKey> {
        let tab = self.tabs.get(self.active)?;
        Some((tab.id, tab.focus))
    }

    /// The shell `key` names, while it lives.
    pub fn shell(&self, key: ShellKey) -> Option<&TermPane> {
        self.tabs
            .iter()
            .find(|t| t.id == key.0)
            .and_then(|t| t.terms.get(&key.1))
    }

    pub fn split(&mut self, axis: Axis, cwd: &Path) -> Result<()> {
        // Each half needs a border plus a few usable rows or columns. A shell squeezed
        // below that is useless, so say so instead of making one.
        let tab = &self.tabs[self.active];
        if let Some((_, r)) = self.rects.iter().find(|(id, _)| *id == tab.focus) {
            let (have, need, what) = match axis {
                Axis::Rows => (r.height, 2 * (2 + 3), "taller"),
                Axis::Cols => (r.width, 2 * (2 + 12), "wider"),
            };
            if have < need {
                anyhow::bail!("too small to split \u{2014} make the terminal {what} first");
            }
        }
        let (id, t) = self.spawn(cwd)?;
        let tab = &mut self.tabs[self.active];
        if tab.tree.split_leaf(tab.focus, axis, id) {
            tab.terms.insert(id, t);
            tab.focus = id;
        }
        Ok(())
    }

    /// Remove one shell. Returns true when the panel has nothing left.
    fn remove(&mut self, tab: usize, id: PaneId) -> bool {
        let t = &mut self.tabs[tab];
        // Dropping the TermPane drops the pty master, which hangs up the shell.
        if let Some(gone) = t.terms.remove(&id) {
            Self::end_log(&gone, &mut self.notes);
        }
        if t.terms.is_empty() {
            self.tabs.remove(tab);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            } else if self.active > tab {
                self.active -= 1;
            }
            return self.tabs.is_empty();
        }
        t.tree.remove_leaf(id);
        if t.focus == id {
            t.focus = t.leaves()[0];
        }
        false
    }

    pub fn close_focused(&mut self) -> bool {
        let id = self.tabs[self.active].focus;
        self.remove(self.active, id)
    }

    pub fn close_tab(&mut self, i: usize) -> bool {
        if i >= self.tabs.len() {
            return false;
        }
        let tab = self.tabs.remove(i);
        for t in tab.terms.values() {
            Self::end_log(t, &mut self.notes);
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        } else if self.active > i {
            self.active -= 1;
        }
        self.tabs.is_empty()
    }

    /// Close every shell whose process has ended — typing `exit` closes its terminal,
    /// as in an editor. Returns how many went, and whether the panel is now empty.
    pub fn reap(&mut self) -> (usize, bool) {
        let mut gone = 0;
        for tab in (0..self.tabs.len()).rev() {
            let dead: Vec<PaneId> = self.tabs[tab]
                .terms
                .iter()
                // A program's own tab stays for its output; Enter or Esc closes it.
                .filter(|(_, t)| t.has_exited() && !t.keeps())
                .map(|(id, _)| *id)
                .collect();
            for id in dead {
                gone += 1;
                if self.remove(tab, id) {
                    return (gone, true);
                }
                if tab >= self.tabs.len() {
                    break;
                }
            }
        }
        (gone, self.tabs.is_empty())
    }

    /// A shell's log, ended: queued output written, flushed, said in `notes`.
    fn end_log(t: &TermPane, notes: &mut Vec<String>) {
        if let Some(l) = t.take_log() {
            notes.push(l.close(LOG_WAIT));
        }
    }

    /// Tab `i`'s log, while it has one.
    pub fn tab_log(&self, i: usize) -> Option<Arc<Log>> {
        self.tabs.get(i)?.logged().map(|(_, l)| l)
    }

    /// The name of tab `i`'s focused shell: what a log's file is named after.
    pub fn tab_shell_name(&self, i: usize) -> Option<String> {
        let t = self.tabs.get(i)?;
        t.terms.get(&t.focus).map(|p| p.shell_name().to_string())
    }

    /// Log tab `i`'s focused shell to `path` (appended; created `0600`). The words for
    /// the status line, or why not.
    pub fn start_log(
        &mut self,
        i: usize,
        path: &Path,
        mode: termlog::Mode,
    ) -> std::result::Result<String, String> {
        let Some(tab) = self.tabs.get(i) else {
            return Err("that terminal tab is gone".into());
        };
        if let Some((_, l)) = tab.logged() {
            return Err(format!(
                "this tab is already logged to {}",
                l.path().display()
            ));
        }
        let Some(shell) = tab.terms.get(&tab.focus) else {
            return Err("that terminal tab is gone".into());
        };
        if shell.has_exited() {
            return Err("that shell has ended \u{2014} nothing more to log".into());
        }
        let (file, existed) =
            termlog::open(path).map_err(|e| format!("cannot log to {}: {e}", path.display()))?;
        let shared = existed && termlog::readable_by_others(&file);
        let about = termlog::About {
            label: format!("{} \u{b7} tab {}", shell.shell_name(), i + 1),
        };
        shell.set_log(Log::start(path.to_path_buf(), mode, Box::new(file), about));
        let verb = if existed {
            "appending to"
        } else {
            "logging to"
        };
        let mut said = format!("{verb} {} ({})", path.display(), mode.word());
        if shared {
            said.push_str(" \u{2014} note: others can read that file");
        }
        Ok(said)
    }

    /// End tab `i`'s log; what to say, or `None` when it had none.
    pub fn stop_log(&mut self, i: usize) -> Option<String> {
        let (id, _) = self.tabs.get(i)?.logged()?;
        let t = self.tabs[i].terms.get(&id)?;
        let mut notes = Vec::new();
        Self::end_log(t, &mut notes);
        notes.pop()
    }

    /// What the logs have to say since the last call: a log that ended with its shell
    /// (closed and let go), a failure (said once; the log stays, marked, until Stop
    /// logging or the tab closes), and what closing shells and tabs said.
    pub fn take_notes(&mut self) -> Vec<String> {
        let mut out = std::mem::take(&mut self.notes);
        for tab in &self.tabs {
            for t in tab.terms.values() {
                let Some(l) = t.log() else { continue };
                match l.status() {
                    termlog::Status::Running => {}
                    termlog::Status::Ended => Self::end_log(t, &mut out),
                    termlog::Status::Failed(why) => {
                        if !t.log_failure_said() {
                            out.push(format!(
                                "log stopped: {why} \u{2014} {}",
                                l.path().display()
                            ));
                        }
                    }
                }
            }
        }
        out
    }

    /// Quitting: every log ended, each given [`LOG_WAIT`] at most.
    pub fn close_logs(&mut self) -> Vec<String> {
        let mut out = std::mem::take(&mut self.notes);
        for tab in &self.tabs {
            for t in tab.terms.values() {
                Self::end_log(t, &mut out);
            }
        }
        out
    }

    pub fn cycle_tab(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        if n > 0 {
            self.active = (self.active as isize + delta).rem_euclid(n) as usize;
        }
    }

    pub fn next_split(&mut self) {
        let tab = &mut self.tabs[self.active];
        let leaves = tab.leaves();
        if let Some(i) = leaves.iter().position(|id| *id == tab.focus) {
            tab.focus = leaves[(i + 1) % leaves.len()];
        }
    }

    /// Each tab's directory (its focused shell's), or an empty string where unknown.
    pub fn tab_dirs(&self) -> Vec<String> {
        self.tabs
            .iter()
            // A tab started for one program is not reopened next time as a shell.
            .filter(|t| t.terms.get(&t.focus).is_none_or(|p| !p.keeps()))
            .map(|t| {
                t.terms
                    .get(&t.focus)
                    .and_then(TermPane::cwd)
                    .and_then(|p| p.to_str().map(str::to_string))
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Every running shell, across all tabs and splits — what the limit counts.
    pub fn shell_count(&self) -> usize {
        self.tabs.iter().map(|t| t.terms.len()).sum()
    }

    pub fn select_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_tab(&self) -> usize {
        self.active
    }

    pub fn split_count(&self) -> usize {
        self.tabs.get(self.active).map_or(0, |t| t.terms.len())
    }

    /// 1-based position of the focused shell among the tab's splits.
    pub fn focused_split(&self) -> usize {
        self.tabs.get(self.active).map_or(0, |tab| {
            tab.leaves()
                .iter()
                .position(|id| *id == tab.focus)
                .map_or(0, |i| i + 1)
        })
    }

    fn focused(&self) -> Option<&TermPane> {
        let tab = self.tabs.get(self.active)?;
        tab.terms.get(&tab.focus)
    }

    /// The text of tab `tab`'s focused shell (the active tab's when `None`), all its
    /// history and screen, with the title the viewer shows it under,
    /// "history · sh · tab 2", and the shell's directory where the system says.
    /// `None` when there is no such tab.
    pub fn history(&self, tab: Option<usize>) -> Option<History> {
        let i = tab.unwrap_or(self.active);
        let t = self.tabs.get(i)?;
        let shell = t.terms.get(&t.focus)?;
        let label = format!("{} \u{b7} tab {}", shell.shell_name(), i + 1);
        let title = format!("history \u{b7} {label}");
        let remote = match shell.state() {
            crate::pane::term::ShellState::Remote(p) => Some(p),
            _ => None,
        };
        let (text, commands, full_screen) = shell.history_commands();
        let dir = shell.shell_dir();
        Some(History {
            title,
            text,
            // Only a directory the system gave: a Save as… default is written to.
            cwd: dir.clone().filter(|d| d.trusted).map(|d| d.path),
            full_screen,
            commands,
            dir_now: dir.map(|d| d.path),
            remote,
            label,
        })
    }

    pub fn send_key(&self, k: KeyEvent) -> bool {
        self.focused().is_some_and(|t| t.send_key(k))
    }

    pub fn shell_name(&self) -> String {
        self.focused()
            .map(|t| t.shell_name().to_string())
            .unwrap_or_default()
    }

    /// The focused shell's state; see [`TermPane::state`]. `Unknown` with no shell.
    pub fn shell_state(&self) -> crate::pane::term::ShellState {
        self.focused()
            .map_or(crate::pane::term::ShellState::Unknown, TermPane::state)
    }

    /// The focused shell's directory; see [`TermPane::shell_dir`].
    pub fn shell_dir(&self) -> Option<crate::pane::term::ShellDir> {
        self.focused().and_then(TermPane::shell_dir)
    }

    /// Move a border between this tab's shells. False when the focused shell has no
    /// border that way inside the tab, so the caller moves the panel's own edge.
    pub fn move_boundary(&mut self, axis: Axis, delta: i16) -> bool {
        let tab = &mut self.tabs[self.active];
        tab.terms.len() > 1 && tab.tree.move_boundary(tab.focus, axis, delta)
    }

    pub fn dividers(&self) -> Vec<Divider> {
        let mut out = Vec::new();
        if let Some(tab) = self.tabs.get(self.active) {
            tab.tree.dividers(self.body, &mut out);
        }
        out
    }

    /// Share a boundary between this tab's shells evenly; see [`Node::even`].
    pub fn even(&mut self, id: usize) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.tree.even(id);
        }
    }

    pub fn set_pct(&mut self, id: usize, pct: u16) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.tree.set_pct(id, pct);
        }
    }

    /// The wheel at (`x`, `y`): to the shell under it, which hands it on to its program
    /// or scrolls its history ([`TermPane::wheel`]).
    pub fn wheel_at(&mut self, x: u16, y: u16, up: bool) {
        let p = Position::new(x, y);
        let Some((id, r)) = self.rects.iter().find(|(_, r)| r.contains(p)).copied() else {
            return;
        };
        // Counted from 1 inside the shell's border, as a terminal reports it.
        let col = x.saturating_sub(r.x).max(1);
        let row = y.saturating_sub(r.y).max(1);
        if let Some(t) = self.tabs[self.active].terms.get(&id) {
            t.wheel(col, row, up);
        }
    }

    /// A mouse button inside a shell of the active tab (not on its border): to the
    /// program there when it asked for the mouse; true when it went. See
    /// [`TermPane::mouse`].
    ///
    /// When it went, that shell and its rectangle: the rest of the gesture goes there
    /// ([`TermPanel::mouse_to`]), wherever the pointer is by then.
    pub fn mouse_at(
        &self,
        x: u16,
        y: u16,
        ev: crate::pane::term::MouseEv,
    ) -> Option<(ShellKey, Rect)> {
        let p = Position::new(x, y);
        let (id, r) = self.rects.iter().find(|(_, r)| r.contains(p)).copied()?;
        let inside = x > r.x
            && y > r.y
            && x + 1 < r.x.saturating_add(r.width)
            && y + 1 < r.y.saturating_add(r.height);
        if !inside {
            return None;
        }
        let tab = self.tabs.get(self.active)?;
        let key = (tab.id, id);
        self.mouse_to(key, r, x, y, ev).then_some((key, r))
    }

    /// The rest of a gesture to the shell that took its press: `r` is that shell's
    /// rectangle at the press, and the point is kept inside it (a drag out of the shell
    /// is reported at its edge, as a terminal does), so the release always reaches the
    /// program that saw the press. False when that shell is gone.
    pub fn mouse_to(
        &self,
        key: ShellKey,
        r: Rect,
        x: u16,
        y: u16,
        ev: crate::pane::term::MouseEv,
    ) -> bool {
        if r.width < 3 || r.height < 3 {
            return false;
        }
        let x = x.clamp(r.x + 1, r.x + r.width - 2);
        let y = y.clamp(r.y + 1, r.y + r.height - 2);
        self.shell(key)
            .is_some_and(|t| t.mouse(x - r.x, y - r.y, ev))
    }

    /// True when the point is on a tab, a close mark or the + button. The tab bar is
    /// the panel's first row, which is also half of the strip that resizes the panel,
    /// so a button has to win over the resize or no tab could ever be clicked.
    pub fn bar_button_at(&self, x: u16, y: u16) -> bool {
        let p = Position::new(x, y);
        self.new_hit.contains(p)
            || self.tab_hits.iter().any(|(_, r)| r.contains(p))
            || self.close_hits.iter().any(|(_, r)| r.contains(p))
    }

    /// The tab whose label is drawn at (`x`, `y`), for the right-click menu. Only the
    /// tab bar, which is xdevcmd's: inside a shell the click is the program's.
    pub fn tab_at(&self, x: u16, y: u16) -> Option<usize> {
        let p = Position::new(x, y);
        self.tab_hits
            .iter()
            .chain(self.close_hits.iter())
            .find(|(_, r)| r.contains(p))
            .map(|(i, _)| *i)
    }

    pub fn on_click(&mut self, x: u16, y: u16) -> Click {
        let p = Position::new(x, y);
        if self.new_hit.contains(p) {
            return Click::NewTab;
        }
        if let Some((i, _)) = self.close_hits.iter().find(|(_, r)| r.contains(p)) {
            return Click::CloseTab(*i);
        }
        if let Some((i, _)) = self.tab_hits.iter().find(|(_, r)| r.contains(p)) {
            self.active = *i;
            return Click::Selected;
        }
        if let Some((id, _)) = self.rects.iter().find(|(_, r)| r.contains(p)) {
            self.tabs[self.active].focus = *id;
            return Click::Selected;
        }
        Click::Nothing
    }

    /// Where the shells' screens are, inside their frames, as last drawn: what the
    /// programs in them draw, which the theme's decor leaves alone.
    pub fn screens(&self) -> Rect {
        Rect::new(
            self.body.x + 1,
            self.body.y + 1,
            self.body.width.saturating_sub(2),
            self.body.height.saturating_sub(2),
        )
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, active: bool) {
        let th = theme::t();
        if area.height == 0 || self.tabs.is_empty() {
            return;
        }

        // ── tab bar ────────────────────────────────────────────────────────────────
        let bar = Rect::new(area.x, area.y, area.width, 1);
        frame
            .buffer_mut()
            .set_style(bar, Style::new().bg(th.bg_bar));
        self.tab_hits.clear();
        self.close_hits.clear();
        let right = area.x + area.width;
        let mut cx = area.x;
        let put = |frame: &mut Frame, x: u16, text: &str, style: Style| -> u16 {
            let w = text.chars().count() as u16;
            if x < right {
                frame.buffer_mut().set_line(
                    x,
                    bar.y,
                    &Line::from(Span::styled(text.to_string(), style)),
                    right - x,
                );
            }
            w
        };
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active;
            let style = match (is_active, active) {
                (true, true) => Style::new()
                    .fg(th.bg)
                    .bg(th.accent)
                    .add_modifier(Modifier::BOLD),
                (true, false) => Style::new().fg(th.fg).bg(th.row_cursor),
                (false, _) => Style::new().fg(th.fg_dim),
            };
            // ● while the tab is logged; ! in the error colour once its log failed.
            let mark = tab.log_mark().map(|m| m == LogMark::Failed);
            let label = match mark {
                Some(_) => format!(" \u{2302} {} {}", i + 1, tab.label()),
                None => format!(" \u{2302} {} {} ", i + 1, tab.label()),
            };
            let mut w = put(frame, cx, &label, style);
            if let Some(failed) = mark {
                let (text, st) = if failed {
                    (" ! ", style.fg(th.err).add_modifier(Modifier::BOLD))
                } else {
                    (" \u{25cf} ", style)
                };
                w += put(frame, cx + w, text, st);
            }
            // The selected tab of the focused pane as a slanted segment, in that look.
            let slanted =
                i == self.active && active && crate::decor::get().keys == crate::decor::Keys::Slant;
            if slanted {
                crate::decor::slant(frame.buffer_mut(), cx, cx + w - 1, bar.y);
            }
            let style = if slanted {
                Style::new().bg(th.bg_bar)
            } else {
                style
            };
            self.tab_hits.push((i, Rect::new(cx, bar.y, w, 1)));
            cx += w;
            let w = put(frame, cx, "\u{00d7}", style.fg(th.fg_dim));
            self.close_hits.push((i, Rect::new(cx, bar.y, w, 1)));
            cx += w + 1;
        }
        let w = put(
            frame,
            cx,
            " + ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        );
        self.new_hit = Rect::new(cx, bar.y, w, 1);

        // ── the active tab's shells ────────────────────────────────────────────────
        self.body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        if self.body.height == 0 {
            return;
        }
        let tab = &mut self.tabs[self.active];
        let mut rects = Vec::new();
        tab.tree.assign(self.body, &mut rects);
        for (id, r) in &rects {
            if let Some(t) = tab.terms.get_mut(id) {
                let (rows, cols) = TermPane::inner_size(*r);
                t.resize(rows, cols);
                t.render(frame, *r, active && *id == tab.focus);
            }
        }
        self.rects = rects;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// A sink whose write panics, as a bug in the writer's path would.
    struct Panics;
    impl std::io::Write for Panics {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            panic!("a bug in the writer");
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A log whose writer panics: said once on the status line, and the tab's ● turns
    /// into the !, which stays until Stop logging.
    #[test]
    fn a_log_whose_writer_dies_is_said_once_and_marked() {
        let dir = std::env::temp_dir();
        let mut p = TermPanel::new(&dir, 10, 40, "sh", 100).expect("a shell");
        let log = Log::start(
            "x.log".into(),
            termlog::Mode::Raw,
            Box::new(Panics),
            termlog::About { label: "sh".into() },
        );
        {
            let tab = &p.tabs[0];
            tab.terms[&tab.focus].set_log(log.clone());
            assert_eq!(tab.log_mark(), Some(LogMark::Running));
        }
        log.tap(&vec![b'x'; 128 * 1024]);
        assert!(log.wait(Duration::from_secs(5)));
        let deadline = Instant::now() + Duration::from_secs(5);
        let notes = loop {
            let n = p.take_notes();
            if !n.is_empty() || Instant::now() > deadline {
                break n;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            notes,
            ["log stopped: the log's writer stopped \u{2014} x.log"]
        );
        assert!(p.take_notes().is_empty(), "said once");
        assert_eq!(p.tabs[0].log_mark(), Some(LogMark::Failed));
        let said = p.stop_log(0).expect("a log to stop");
        assert!(said.starts_with("log stopped"), "{said}");
        assert_eq!(p.tabs[0].log_mark(), None);
    }
}
