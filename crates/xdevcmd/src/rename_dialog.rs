//! Shift+F2: rename many files with a pattern, and see every new name before Enter.
//!
//! The masks live in `xdevcmd_core::name_mask`; the plan, and the checks on it, are the
//! same `xdevcmd_core::batch_rename::plan` the editor route uses. F4 hands the names, as
//! the pattern has them so far, to the editor instead.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use xdevcmd_core::name_mask::{Case, Mask};

use crate::find_dialog::field_line;
use crate::line_edit::LineEdit;
use crate::modal::{buttons, frame_box, put};
use crate::theme;

const FIELDS: usize = 5;
const CASE: usize = 4;

pub struct RenameDialog {
    pub files: Vec<PathBuf>,
    edits: [LineEdit; 4],
    case: Case,
    pub field: usize,
    /// The new name of every file, or why there are none.
    names: Result<Vec<String>, String>,
    /// What would change, or why nothing can.
    pub plan: Result<Vec<(PathBuf, PathBuf)>, String>,
    /// First preview row on screen.
    top: usize,
    page: usize,
    pub buttons: Vec<Rect>,
}

pub enum Outcome {
    Stay,
    Close,
    Rename(Vec<(PathBuf, PathBuf)>),
    /// Open the editor with these names, one per file.
    Editor(String),
}

impl RenameDialog {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let m = Mask::default();
        let mut d = Self {
            files,
            edits: [
                LineEdit::new(&m.name),
                LineEdit::new(&m.ext),
                LineEdit::new(""),
                LineEdit::new(""),
            ],
            case: m.case,
            field: 0,
            names: Ok(Vec::new()),
            plan: Ok(Vec::new()),
            top: 0,
            page: 1,
            buttons: Vec::new(),
        };
        d.update();
        d
    }

    fn mask(&self) -> Mask {
        Mask {
            name: self.edits[0].text(),
            ext: self.edits[1].text(),
            find: self.edits[2].text(),
            replace: self.edits[3].text(),
            case: self.case,
        }
    }

    fn update(&mut self) {
        self.names = self.mask().apply(&self.files);
        self.plan = match &self.names {
            Ok(n) => xdevcmd_core::batch_rename::plan(&self.files, &(n.join("\n") + "\n")),
            Err(e) => Err(e.clone()),
        };
    }

    /// Enter: rename, when there is something valid to rename.
    pub fn confirm(&self) -> Outcome {
        match &self.plan {
            Ok(m) if !m.is_empty() => Outcome::Rename(m.clone()),
            _ => Outcome::Stay,
        }
    }

    /// F4: the names so far, or the old ones if the masks do not work yet.
    pub fn to_editor(&self) -> Outcome {
        let names = match &self.names {
            Ok(n) => n.join("\n") + "\n",
            Err(_) => xdevcmd_core::batch_rename::names_text(&self.files),
        };
        Outcome::Editor(names)
    }

    pub fn next_field(&mut self, back: bool) {
        self.field = if back {
            (self.field + FIELDS - 1) % FIELDS
        } else {
            (self.field + 1) % FIELDS
        };
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Outcome {
        match k.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Enter => return self.confirm(),
            KeyCode::F(4) => return self.to_editor(),
            KeyCode::Tab | KeyCode::Down => self.next_field(false),
            KeyCode::BackTab | KeyCode::Up => self.next_field(true),
            KeyCode::PageDown => self.top += self.page,
            KeyCode::PageUp => self.top = self.top.saturating_sub(self.page),
            _ if self.field == CASE => {
                match k.code {
                    KeyCode::Left => self.case = self.case.prev(),
                    KeyCode::Right | KeyCode::Char(' ') => self.case = self.case.next(),
                    _ => return Outcome::Stay,
                }
                self.update();
            }
            _ => {
                if self.edits[self.field].on_key(k) {
                    self.update();
                }
            }
        }
        Outcome::Stay
    }
}

const LABELS: [&str; FIELDS] = ["Name", "Extension", "Find", "Replace with", "Case"];
const LABEL_W: u16 = 14;

pub fn render(frame: &mut Frame, area: Rect, d: &mut RenameDialog) {
    let th = theme::t();
    let w = area.width.saturating_sub(4).min(100);
    let h = area.height.saturating_sub(2).min(10 + d.files.len() as u16);
    let r = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    let n = d.files.len();
    let title = format!("Rename {n} file{}", if n == 1 { "" } else { "s" });
    let inner = frame_box(frame, r, &title, th.accent);
    if inner.height < 3 || inner.width < LABEL_W + 4 {
        return;
    }
    let field_w = (inner.width - LABEL_W) as usize;
    for (i, label) in LABELS.iter().enumerate() {
        let y = i as u16;
        if y + 2 >= inner.height {
            break;
        }
        let on = d.field == i;
        let style = if on {
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(th.fg_dim)
        };
        put(frame, inner, y, label, style);
        let line = if i == CASE {
            let s = if on {
                Style::new().fg(th.bg).bg(th.accent)
            } else {
                Style::new().fg(th.fg)
            };
            Line::from(vec![
                Span::styled(format!("\u{2039} {} \u{203a}", d.case.label()), s),
                Span::styled("  \u{2190} \u{2192} to change", Style::new().fg(th.fg_dim)),
            ])
        } else {
            field_line(&d.edits[i], on, field_w)
        };
        frame
            .buffer_mut()
            .set_line(inner.x + LABEL_W, inner.y + y, &line, field_w as u16);
    }
    let mut y = FIELDS as u16;
    let help = "[N] name  [N2-5] letters 2-5  [E] extension  [P] folder  [C] counter  [C10+5:3]";
    if y + 2 < inner.height {
        put(frame, inner, y, help, Style::new().fg(th.fg_dim));
        y += 1;
    }
    // What is wrong, if anything, and what Enter would do.
    let (said, style) = match &d.plan {
        Err(e) => (format!("\u{2716} {e}"), Style::new().fg(th.err)),
        Ok(m) if m.is_empty() => ("no name changes yet".into(), Style::new().fg(th.fg_dim)),
        Ok(m) => (
            format!("Enter renames {} of {n}", m.len()),
            Style::new().fg(th.accent),
        ),
    };
    if y + 2 < inner.height {
        put(frame, inner, y, &said, style);
        y += 1;
    }

    // The preview: every file, old name and new.
    let rows = inner.height.saturating_sub(y + 1);
    d.page = (rows as usize).max(1);
    d.top = d.top.min(n.saturating_sub(d.page));
    let longest = d
        .files
        .iter()
        .map(|f| {
            f.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .count()
        })
        .max()
        .unwrap_or(0);
    let old_w = longest.min((inner.width as usize).saturating_sub(4) / 2);
    let body = Rect::new(inner.x, inner.y + y, inner.width, rows);
    for (row, i) in (d.top..n).take(rows as usize).enumerate() {
        let old = d.files[i]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let new = d.names.as_ref().ok().and_then(|v| v.get(i)).cloned();
        let changed = new.as_ref().is_some_and(|n| *n != old);
        let fit = |s: &str, w: usize| -> String {
            let count = s.chars().count();
            if count <= w {
                format!("{s:<w$}")
            } else {
                let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
                t.push('\u{2026}');
                t
            }
        };
        let dim = Style::new().fg(th.fg_dim);
        let line = Line::from(vec![
            Span::styled(
                fit(&old, old_w),
                if changed { Style::new().fg(th.fg) } else { dim },
            ),
            Span::styled(" \u{2192} ", dim),
            Span::styled(
                new.unwrap_or_default(),
                if changed {
                    Style::new().fg(th.accent)
                } else {
                    dim
                },
            ),
        ]);
        frame
            .buffer_mut()
            .set_line(body.x, body.y + row as u16, &line, body.width);
    }
    crate::scrollbar::draw(
        frame.buffer_mut(),
        crate::scrollbar::track(r, body),
        n,
        d.top,
        Style::new().fg(th.accent),
    );

    let by = inner.y + inner.height - 1;
    d.buttons = buttons(
        frame,
        inner.x,
        by,
        inner.width,
        &[
            ("Enter", "rename", th.accent),
            ("F4", "edit in editor", th.accent),
            ("Tab", "next field", th.accent),
            ("Esc", "cancel", th.accent),
        ],
    );
}
