//! Keys a terminal does not send, and how to get them anyway.
//!
//! - **Esc as a prefix**, as Midnight Commander has it: Esc (pressed and released),
//!   then a key, is Alt+key; Esc then 1…9, 0 is F1…F10 ([`after_esc`]). It works in
//!   every terminal with no setting — for a Mac whose Option key types characters, and
//!   for a keyboard whose F-keys are media keys.
//! - **Saying so on a Mac.** macOS terminals make Option type characters (Option+C is
//!   `ç`) unless "Use Option as Meta key" is on. When such a character arrives where it
//!   means nothing, [`option_hint`] says what to do, once.

use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// How long after Esc the next key still counts as Alt+key.
pub const ESC_WINDOW: Duration = Duration::from_secs(2);

/// The key meant by `k` pressed just after Esc: Alt+`k`, or F1…F10 for 1…9, 0.
/// `None` for a second Esc, which cancels.
pub fn after_esc(k: KeyEvent) -> Option<KeyEvent> {
    if k.code == KeyCode::Esc {
        return None;
    }
    let plain = (k.modifiers - KeyModifiers::SHIFT).is_empty();
    if plain {
        if let KeyCode::Char(c @ '0'..='9') = k.code {
            let n = if c == '0' { 10 } else { c as u8 - b'0' };
            return Some(KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE));
        }
    }
    Some(KeyEvent::new(k.code, k.modifiers | KeyModifiers::ALT))
}

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
        "Option typed \u{201c}{c}\u{201d}: press Esc then the key for Alt+key, or turn on \
         \u{201c}Use Option as Meta key\u{201d} (Terminal \u{2192} Settings \u{2192} Profiles \
         \u{2192} Keyboard)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(c, m)
    }
    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn esc_then_a_key_is_alt_and_esc_then_a_digit_is_an_f_key() {
        assert_eq!(
            after_esc(ch('c')),
            Some(key(KeyCode::Char('c'), KeyModifiers::ALT))
        );
        assert_eq!(
            after_esc(key(KeyCode::Left, KeyModifiers::NONE)),
            Some(key(KeyCode::Left, KeyModifiers::ALT))
        );
        assert_eq!(
            after_esc(key(KeyCode::F(7), KeyModifiers::NONE)),
            Some(key(KeyCode::F(7), KeyModifiers::ALT))
        );
        assert_eq!(
            after_esc(ch('1')),
            Some(key(KeyCode::F(1), KeyModifiers::NONE))
        );
        assert_eq!(
            after_esc(ch('5')),
            Some(key(KeyCode::F(5), KeyModifiers::NONE))
        );
        assert_eq!(
            after_esc(ch('0')),
            Some(key(KeyCode::F(10), KeyModifiers::NONE))
        );
        assert_eq!(after_esc(key(KeyCode::Esc, KeyModifiers::NONE)), None);
    }

    #[test]
    fn only_a_bare_non_ascii_character_looks_like_option() {
        assert_eq!(typed_by_option(ch('ç')), Some('ç'));
        assert_eq!(typed_by_option(ch('c')), None);
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
