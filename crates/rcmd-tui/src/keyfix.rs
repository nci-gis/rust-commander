//! A Mac's Option key, which types characters unless told otherwise.
//!
//! macOS terminals make Option type characters (Option+C is `ç`) unless "Use Option as
//! Meta key" is on, so none of rcmd's Alt keys arrive. When such a character reaches a
//! file pane, where it means nothing, [`option_hint`] says what to turn on, once.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Whether `k` looks like a character a Mac's Option key typed: not ASCII, no Ctrl or
/// Alt with it. A file pane binds no such key, so it can only have been meant as one.
pub fn typed_by_option(k: KeyEvent) -> Option<char> {
    match k.code {
        KeyCode::Char(c) if !c.is_ascii() && (k.modifiers - KeyModifiers::SHIFT).is_empty() => {
            Some(c)
        }
        _ => None,
    }
}

/// What to tell someone whose Option key typed `c`.
pub fn option_hint(c: char) -> String {
    format!(
        "Option typed \u{201c}{c}\u{201d}: for Alt keys, turn on \u{201c}Use Option as Meta \
         key\u{201d} (Terminal \u{2192} Settings \u{2192} Profiles \u{2192} Keyboard)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(c, m)
    }

    #[test]
    fn only_a_bare_non_ascii_character_looks_like_option() {
        assert_eq!(
            typed_by_option(key(KeyCode::Char('ç'), KeyModifiers::NONE)),
            Some('ç')
        );
        assert_eq!(
            typed_by_option(key(KeyCode::Char('c'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            typed_by_option(key(KeyCode::Char('ç'), KeyModifiers::CONTROL)),
            None
        );
        assert_eq!(
            typed_by_option(key(KeyCode::F(5), KeyModifiers::NONE)),
            None
        );
    }
}
