//! Dialogs: confirm, text input, and the progress box a running job draws.
//!
//! Every destructive key goes through a confirm here first, and a running job renders a
//! question box the moment the worker asks one. The worker is blocked while that box is
//! up, which is the point: nothing is overwritten or deleted until a key is pressed.

use std::path::PathBuf;

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge};
use ratatui::Frame;

use rcmd_core::format::{elide_middle, human_size};
use rcmd_core::ops::{self, Answer, Question, Request, Summary};

use crate::find_dialog::{self, FindDialog};
use crate::help::Help;
use crate::line_edit::LineEdit;
use crate::menu::{Cmd, Menu};
use crate::rename_dialog::{self, RenameDialog};
use crate::theme;
use crate::viewer::Viewer;

pub enum Modal {
    Confirm(Confirm),
    Input(Input),
    Job(Job),
    /// Takes the whole area rather than a centred box: a viewer that showed sixty
    /// columns of a log would be worse than no viewer.
    View(Box<Viewer>),
    Help(Box<Help>),
    Menu(Menu),
    Find(Box<FindDialog>),
    /// Shift+F2: rename many with a pattern.
    Rename(Box<RenameDialog>),
    Ask(Ask),
    /// Help → Usage. Drawn by the app, which holds the numbers.
    Usage,
}

/// A question that is not about a file job: a plan to confirm (bulk rename), or just
/// something to read (a checksum). `yes` is what Enter does; `None` makes it a notice
/// with only a way out.
pub struct Ask {
    pub title: String,
    pub body: Vec<String>,
    pub danger: bool,
    pub yes: Option<(String, Cmd)>,
    pub buttons: Vec<Rect>,
}

pub struct Confirm {
    pub title: String,
    pub body: Vec<String>,
    pub danger: bool,
    pub request: Request,
    /// Where [Enter] and [Esc] were drawn, so a click can press them.
    pub buttons: Vec<Rect>,
    /// For copy and move: the directories it could go to, best guess first. Tab
    /// steps through them. Empty for anything else.
    pub choices: Vec<PathBuf>,
    pub choice: usize,
    pub source_dir: Option<PathBuf>,
    /// Files and bytes still being counted on another thread, and the body line that
    /// says "counting…" until they arrive. The dialog does not wait for the count:
    /// Enter starts the job, Esc closes it, and either stops the count.
    pub counting: Option<(ops::Count, usize)>,
}

impl Confirm {
    /// The line that names the destination, and a warning when it is the source's own
    /// directory — said before Enter, not discovered afterwards as "1 skipped".
    pub fn set_destination(&mut self, i: usize) {
        let Some(dest) = self.choices.get(i).cloned() else {
            return;
        };
        self.choice = i;
        self.request.dest = Some(dest.clone());
        self.body.truncate(4);
        if let Some(line) = self.body.get_mut(2) {
            *line = format!("to    {}", dest.display());
        }
        if self.source_dir.as_ref() == Some(&dest) {
            self.body.push(String::new());
            self.body
                .push("Same directory as the source \u{2014} nothing would change.".into());
        }
    }

    pub fn next_destination(&mut self) {
        if self.choices.len() > 1 {
            self.set_destination((self.choice + 1) % self.choices.len());
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum InputKind {
    MkDir,
    Rename {
        from: PathBuf,
    },
    /// Ctrl+L: a directory, or a file to put the cursor on.
    GoTo,
    /// `+` / `-`: mark or unmark what matches a pattern.
    Pattern {
        mark: bool,
    },
    /// Shift+F4.
    NewFile,
    /// Shift+F5: a second copy of `from` beside it.
    CopyAs {
        from: PathBuf,
    },
    /// Alt+P on these.
    Permissions {
        targets: Vec<PathBuf>,
    },
    /// Alt+F5: these into an archive named by the text.
    Pack {
        sources: Vec<PathBuf>,
    },
}

pub struct Input {
    pub title: String,
    pub label: String,
    pub edit: LineEdit,
    pub kind: InputKind,
    /// Why the last Enter was refused. The dialog stays open so the name can be fixed
    /// rather than retyped.
    pub error: Option<String>,
    pub dir: PathBuf,
    pub buttons: Vec<Rect>,
}

pub struct Job {
    pub kind: ops::Kind,
    pub handle: ops::Handle,
    pub total_files: u64,
    pub total_bytes: u64,
    pub bytes_done: u64,
    pub current: Option<PathBuf>,
    pub question: Option<Question>,
    pub done: Option<Summary>,
    /// Cancel while running, Close once done.
    pub buttons: Vec<Rect>,
    /// The answers on the question box, when one is up.
    pub answer_buttons: Vec<(Answer, Rect)>,
}

impl Job {
    pub fn new(kind: ops::Kind, handle: ops::Handle) -> Self {
        Self {
            kind,
            handle,
            total_files: 0,
            total_bytes: 0,
            bytes_done: 0,
            current: None,
            question: None,
            done: None,
            buttons: Vec::new(),
            answer_buttons: Vec::new(),
        }
    }

    /// Take everything the worker has sent without blocking.
    pub fn pump(&mut self) {
        while let Ok(ev) = self.handle.events.try_recv() {
            match ev {
                ops::Event::Scanned { files, bytes } => {
                    self.total_files = files;
                    self.total_bytes = bytes;
                }
                ops::Event::Begin { path } => self.current = Some(path),
                ops::Event::Progress { bytes_done } => self.bytes_done = bytes_done,
                ops::Event::Ask(q) => self.question = Some(q),
                ops::Event::Done(s) => {
                    self.bytes_done = self.total_bytes;
                    self.done = Some(s);
                }
            }
        }
    }

    pub fn answer(&mut self, a: Answer) {
        self.handle.answer(a);
        self.question = None;
    }

    pub fn ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            return if self.done.is_some() { 1.0 } else { 0.0 };
        }
        (self.bytes_done as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
    }
}

/// Which keys answer a question, and the hint shown under it.
pub fn options(q: &Question) -> Vec<(&'static str, &'static str, Answer)> {
    match q {
        Question::Conflict { .. } => vec![
            ("O", "overwrite", Answer::Yes),
            ("A", "overwrite all", Answer::YesToAll),
            ("S", "skip", Answer::Skip),
            ("L", "skip all", Answer::SkipAll),
            ("Esc", "abort", Answer::Abort),
        ],
        Question::CrossDevice { .. } => vec![
            ("Y", "copy then delete", Answer::Yes),
            ("A", "do that for all", Answer::YesToAll),
            ("S", "skip", Answer::Skip),
            ("Esc", "abort", Answer::Abort),
        ],
        Question::TrashUnavailable { .. } => vec![
            ("D", "delete permanently", Answer::Yes),
            ("A", "delete all permanently", Answer::YesToAll),
            ("S", "skip", Answer::Skip),
            ("Esc", "abort", Answer::Abort),
        ],
        Question::Failed { .. } => vec![
            ("R", "retry", Answer::Retry),
            ("S", "skip", Answer::Skip),
            ("Esc", "abort", Answer::Abort),
        ],
    }
}

pub fn answer_for(q: &Question, code: KeyCode) -> Option<Answer> {
    if code == KeyCode::Esc {
        return Some(Answer::Abort);
    }
    let KeyCode::Char(c) = code else { return None };
    let c = c.to_ascii_uppercase();
    options(q)
        .into_iter()
        .find(|(k, _, _)| k.len() == 1 && k.starts_with(c))
        .map(|(_, _, a)| a)
}

fn question_text(q: &Question) -> (String, Vec<String>) {
    match q {
        Question::Conflict {
            path,
            existing_bytes,
            incoming_bytes,
        } => (
            "Already exists".to_string(),
            vec![
                path.display().to_string(),
                String::new(),
                format!("on disk   {}", human_size(*existing_bytes)),
                format!("incoming  {}", human_size(*incoming_bytes)),
            ],
        ),
        Question::CrossDevice { path } => (
            "Different filesystem".to_string(),
            vec![
                path.display().to_string(),
                String::new(),
                "A move across filesystems is a copy followed by a delete.".to_string(),
                "It can fail halfway, leaving both copies.".to_string(),
            ],
        ),
        Question::TrashUnavailable { path, reason } => (
            "Cannot move to trash".to_string(),
            vec![
                path.display().to_string(),
                String::new(),
                reason.clone(),
                String::new(),
                "Deleting permanently cannot be undone.".to_string(),
            ],
        ),
        Question::Failed { path, error } => (
            "Failed".to_string(),
            vec![path.display().to_string(), String::new(), error.clone()],
        ),
    }
}

// ── rendering ──────────────────────────────────────────────────────────────────────

pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width.saturating_sub(w)) / 2,
        area.y + (area.height.saturating_sub(h)) / 2,
        w,
        h,
    )
}

pub(crate) fn frame_box(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    accent: ratatui::style::Color,
) -> Rect {
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_style(Style::new().fg(accent))
        .style(Style::new().bg(theme::t().bg_panel).fg(theme::t().fg))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title.to_string(),
                Style::new().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

pub(crate) fn put(frame: &mut Frame, area: Rect, row: u16, text: &str, style: Style) {
    if row >= area.height {
        return;
    }
    let line = Line::from(Span::styled(elide_middle(text, area.width as usize), style));
    frame
        .buffer_mut()
        .set_line(area.x, area.y + row, &line, area.width);
}

/// Draw `[Key] label` buttons in a row and return where each one landed.
///
/// Recording the rectangles while drawing, rather than recomputing them in the click
/// handler, means the click target is always exactly what is on screen.
pub(crate) fn buttons(
    frame: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    items: &[(&str, &str, ratatui::style::Color)],
) -> Vec<Rect> {
    let mut rects = Vec::new();
    let mut spans = Vec::new();
    let mut cx = x;
    for (key, label, colour) in items {
        let k = format!("[{key}]");
        let l = format!(" {label}");
        let w = (k.chars().count() + l.chars().count()) as u16;
        rects.push(Rect::new(cx, y, w.min((x + width).saturating_sub(cx)), 1));
        cx += w + 2;
        spans.push(Span::styled(
            k,
            Style::new().fg(*colour).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(l, Style::new().fg(theme::t().fg_dim)));
        spans.push(Span::raw("  "));
    }
    frame.buffer_mut().set_line(x, y, &Line::from(spans), width);
    rects
}

pub fn render(frame: &mut Frame, area: Rect, modal: &mut Modal) {
    match modal {
        Modal::Confirm(c) => render_confirm(frame, area, c),
        Modal::Input(i) => render_input(frame, area, i),
        Modal::Job(j) => render_job(frame, area, j),
        Modal::View(v) => v.render(frame, area),
        Modal::Help(h) => h.render(frame, area),
        Modal::Menu(m) => m.render(frame, area),
        Modal::Find(d) => find_dialog::render(frame, area, d),
        Modal::Rename(d) => rename_dialog::render(frame, area, d),
        Modal::Ask(a) => render_ask(frame, area, a),
        Modal::Usage => {}
    }
}

fn render_ask(frame: &mut Frame, area: Rect, a: &mut Ask) {
    let th = theme::t();
    let accent = if a.danger { th.err } else { th.accent };
    let shown = a.body.len().min(area.height.saturating_sub(6) as usize);
    let w = a
        .body
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(40, 100) as u16
        + 4;
    let r = centered(area, w, shown as u16 + 4);
    let inner = frame_box(frame, r, &a.title, accent);
    for (i, line) in a.body.iter().take(shown).enumerate() {
        let last = i + 1 == shown && shown < a.body.len();
        let text = if last {
            format!("\u{2026} and {} more", a.body.len() - shown + 1)
        } else {
            line.clone()
        };
        put(frame, inner, i as u16, &text, Style::new().fg(th.fg));
    }
    let y = inner.y + inner.height.saturating_sub(1);
    a.buttons = match &a.yes {
        Some((verb, _)) => buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[
                (("Enter"), verb.as_str(), accent),
                ("Esc", "cancel", th.accent),
            ],
        ),
        None => buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[("Enter", "close", th.accent)],
        ),
    };
}

fn render_confirm(frame: &mut Frame, area: Rect, c: &mut Confirm) {
    let th = theme::t();
    let accent = if c.danger { th.err } else { th.accent };
    let h = (c.body.len() as u16) + 4;
    let w = c
        .body
        .iter()
        .map(|l| l.chars().count())
        .chain(std::iter::once(c.title.chars().count() + 8))
        .max()
        .unwrap_or(40)
        .clamp(40, 100) as u16
        + 4;
    let r = centered(area, w, h);
    let inner = frame_box(frame, r, &c.title, accent);
    for (i, line) in c.body.iter().enumerate() {
        put(frame, inner, i as u16, line, Style::new().fg(th.fg));
    }
    let y = inner.y + inner.height.saturating_sub(1);
    let mut items = vec![("Enter", "go", accent), ("Esc", "cancel", th.accent)];
    if c.choices.len() > 1 {
        items.push(("Tab", "other pane", th.accent));
    }
    c.buttons = buttons(frame, inner.x, y, inner.width, &items);
}

fn render_input(frame: &mut Frame, area: Rect, i: &mut Input) {
    let th = theme::t();
    let h = if i.error.is_some() { 7 } else { 6 };
    let r = centered(area, 64, h);
    let inner = frame_box(frame, r, &i.title, th.accent);
    put(frame, inner, 0, &i.label, Style::new().fg(th.fg_dim));

    // The line itself, scrolled so the cursor stays in view on long names.
    let w = inner.width as usize;
    let chars = i.edit.chars();
    let cur = i.edit.cursor();
    let sel = i.edit.selection();
    let start = if cur >= w { cur + 1 - w } else { 0 };
    let mut spans = Vec::new();
    for idx in start..(start + w) {
        let at_cursor = idx == cur && sel.is_none();
        let Some(c) = chars.get(idx).copied().or(at_cursor.then_some(' ')) else {
            break;
        };
        let style = if at_cursor {
            Style::new().fg(th.bg).bg(th.fg)
        } else if sel.is_some_and(|(s, e)| idx >= s && idx < e) {
            Style::new().fg(th.bg).bg(th.accent)
        } else {
            Style::new().fg(th.fg).add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(c.to_string(), style));
    }
    frame
        .buffer_mut()
        .set_line(inner.x, inner.y + 1, &Line::from(spans), inner.width);

    if let Some(e) = &i.error {
        put(frame, inner, 2, e, Style::new().fg(th.err));
    }
    let verb = match i.kind {
        InputKind::MkDir => "create",
        InputKind::Rename { .. } => "rename",
        InputKind::GoTo => "go",
        InputKind::Pattern { mark: true } => "mark",
        InputKind::Pattern { mark: false } => "unmark",
        InputKind::NewFile => "create",
        InputKind::CopyAs { .. } => "copy",
        InputKind::Permissions { .. } => "apply",
        InputKind::Pack { .. } => "pack",
    };
    let y = inner.y + inner.height.saturating_sub(1);
    i.buttons = buttons(
        frame,
        inner.x,
        y,
        inner.width,
        &[("Enter", verb, th.accent), ("Esc", "cancel", th.accent)],
    );
}

fn render_job(frame: &mut Frame, area: Rect, j: &mut Job) {
    let th = theme::t();
    let r = centered(area, 74, 8);
    let inner = frame_box(frame, r, j.kind.verb(), th.accent);

    match &j.done {
        Some(s) => {
            put(
                frame,
                inner,
                0,
                &s.describe(j.kind),
                Style::new()
                    .fg(if s.failed > 0 { th.err } else { th.exec })
                    .add_modifier(Modifier::BOLD),
            );
            if let Some(first) = s.errors.first() {
                put(frame, inner, 1, first, Style::new().fg(th.err));
            } else {
                put(
                    frame,
                    inner,
                    1,
                    &format!("{} files · {}", j.total_files, human_size(s.bytes)),
                    Style::new().fg(th.fg_dim),
                );
            }
        }
        None => {
            let what = j
                .current
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "scanning…".to_string());
            put(frame, inner, 0, &what, Style::new().fg(th.fg));
            put(
                frame,
                inner,
                1,
                &format!(
                    "{} files · {} of {}",
                    j.total_files,
                    human_size(j.bytes_done),
                    human_size(j.total_bytes)
                ),
                Style::new().fg(th.fg_dim),
            );
        }
    }

    let bar = Rect::new(inner.x, inner.y + 3, inner.width, 1);
    frame.render_widget(
        Gauge::default()
            .ratio(j.ratio())
            .gauge_style(Style::new().fg(th.accent).bg(th.row_cursor))
            .label(format!("{:.0}%", j.ratio() * 100.0)),
        bar,
    );

    let y = inner.y + inner.height.saturating_sub(1);
    j.buttons = if j.done.is_some() {
        buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[("Enter", "close", th.accent)],
        )
    } else {
        buttons(
            frame,
            inner.x,
            y,
            inner.width,
            &[("Esc", "cancel", th.accent), ("B", "background", th.accent)],
        )
    };

    j.answer_buttons = match &j.question {
        Some(q) => render_question(frame, area, q),
        None => Vec::new(),
    };
}

fn render_question(frame: &mut Frame, area: Rect, q: &Question) -> Vec<(Answer, Rect)> {
    let th = theme::t();
    let (title, body) = question_text(q);
    let opts = options(q);
    let danger = matches!(
        q,
        Question::TrashUnavailable { .. } | Question::Failed { .. }
    );
    let accent = if danger { th.err } else { th.mark };

    let h = body.len() as u16 + 4;
    let r = centered(area, 86, h);
    let inner = frame_box(frame, r, &title, accent);
    for (i, line) in body.iter().enumerate() {
        put(frame, inner, i as u16, line, Style::new().fg(th.fg));
    }
    let y = inner.y + inner.height.saturating_sub(1);
    let labels: Vec<(&str, &str, ratatui::style::Color)> =
        opts.iter().map(|(k, l, _)| (*k, *l, th.accent)).collect();
    let rects = buttons(frame, inner.x, y, inner.width, &labels);
    opts.into_iter().map(|(_, _, a)| a).zip(rects).collect()
}
