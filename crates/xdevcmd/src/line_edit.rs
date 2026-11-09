//! One editable line: a cursor, an optional selection, and the keys people expect.
//!
//! Positions are counted in `char`s, never bytes. A name like `báo cáo tuần.docx` has
//! more bytes than characters, and a cursor measured in bytes lands in the middle of a
//! letter and panics on the next insert.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineEdit {
    chars: Vec<char>,
    cursor: usize,
    /// The other end of the selection; the cursor is always one end.
    anchor: Option<usize>,
}

impl LineEdit {
    pub fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let cursor = chars.len();
        Self {
            chars,
            cursor,
            anchor: None,
        }
    }

    /// Ready to rename: the name part selected, the extension left alone, so typing
    /// replaces `report` in `report.docx` and keeps `.docx`. A directory, or a name with
    /// no extension, is selected whole.
    pub fn for_rename(name: &str, is_dir: bool) -> Self {
        let mut e = Self::new(name);
        let stem_len = if is_dir {
            e.chars.len()
        } else {
            xdevcmd_core::split_name(name).0.chars().count()
        };
        e.anchor = Some(0);
        e.cursor = stem_len;
        e
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selected range, start before end, or `None` when nothing is selected.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    fn delete_selection(&mut self) -> bool {
        let Some((s, e)) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.chars.drain(s..e);
        self.cursor = s;
        self.anchor = None;
        true
    }

    fn move_to(&mut self, pos: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = pos.min(self.chars.len());
    }

    /// Returns false for keys it does not handle, so the caller can use them.
    pub fn on_key(&mut self, k: KeyEvent) -> bool {
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Char('a') if ctrl => {
                self.anchor = Some(0);
                self.cursor = self.chars.len();
            }
            KeyCode::Char('u') if ctrl => {
                // Shell habit: delete back to the start.
                self.chars.drain(..self.cursor);
                self.cursor = 0;
                self.anchor = None;
            }
            KeyCode::Char(c) if !ctrl => {
                self.delete_selection();
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if !self.delete_selection() && self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if !self.delete_selection() && self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                }
            }
            KeyCode::Left => match self.selection() {
                // Collapsing a selection lands at its edge, as in every text field.
                Some((s, _)) if !shift => self.move_to(s, false),
                _ => self.move_to(self.cursor.saturating_sub(1), shift),
            },
            KeyCode::Right => match self.selection() {
                Some((_, e)) if !shift => self.move_to(e, false),
                _ => self.move_to(self.cursor + 1, shift),
            },
            KeyCode::Home => self.move_to(0, shift),
            KeyCode::End => self.move_to(self.chars.len(), shift),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }
    fn shift(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::SHIFT)
    }
    fn typed(e: &mut LineEdit, s: &str) {
        for c in s.chars() {
            e.on_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn rename_selects_the_name_and_typing_keeps_the_extension() {
        let mut e = LineEdit::for_rename("report.docx", false);
        assert_eq!(e.selection(), Some((0, 6)));
        typed(&mut e, "summary");
        assert_eq!(e.text(), "summary.docx");
    }

    #[test]
    fn positions_are_characters_not_bytes() {
        // "báo cáo" is 7 characters and 9 bytes.
        let mut e = LineEdit::for_rename("báo cáo.docx", false);
        assert_eq!(e.selection(), Some((0, 7)));
        e.on_key(key(KeyCode::End));
        e.on_key(key(KeyCode::Left));
        e.on_key(key(KeyCode::Backspace));
        assert_eq!(
            e.text(),
            "báo cáo.dox",
            "must not split a multi-byte letter"
        );
        e.on_key(key(KeyCode::Home));
        e.on_key(key(KeyCode::Delete));
        assert_eq!(e.text(), "áo cáo.dox");
    }

    #[test]
    fn directories_and_dotfiles_are_selected_whole() {
        assert_eq!(
            LineEdit::for_rename("src.old", true).selection(),
            Some((0, 7))
        );
        assert_eq!(
            LineEdit::for_rename(".bashrc", false).selection(),
            Some((0, 7))
        );
        assert_eq!(
            LineEdit::for_rename("Makefile", false).selection(),
            Some((0, 8))
        );
    }

    #[test]
    fn arrows_collapse_a_selection_to_its_edge() {
        let mut e = LineEdit::for_rename("report.docx", false);
        e.on_key(key(KeyCode::Right));
        assert_eq!(
            (e.cursor(), e.selection()),
            (6, None),
            "right lands at the dot"
        );
        let mut e = LineEdit::for_rename("report.docx", false);
        e.on_key(key(KeyCode::Left));
        assert_eq!((e.cursor(), e.selection()), (0, None));
    }

    #[test]
    fn shift_arrows_extend_and_backspace_removes_the_selection() {
        let mut e = LineEdit::new("abcdef");
        e.on_key(key(KeyCode::Home));
        e.on_key(shift(KeyCode::Right));
        e.on_key(shift(KeyCode::Right));
        assert_eq!(e.selection(), Some((0, 2)));
        e.on_key(key(KeyCode::Backspace));
        assert_eq!(e.text(), "cdef");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn ctrl_a_selects_all_and_ctrl_u_clears_to_the_start() {
        let mut e = LineEdit::new("hello");
        e.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        typed(&mut e, "x");
        assert_eq!(e.text(), "x");
        let mut e = LineEdit::new("hello world");
        e.on_key(key(KeyCode::Left));
        e.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(e.text(), "d");
    }

    #[test]
    fn editing_at_the_edges_does_not_panic() {
        let mut e = LineEdit::new("");
        e.on_key(key(KeyCode::Backspace));
        e.on_key(key(KeyCode::Delete));
        e.on_key(key(KeyCode::Left));
        e.on_key(key(KeyCode::Right));
        assert_eq!(e.text(), "");
        assert_eq!(e.cursor(), 0);
    }
}
