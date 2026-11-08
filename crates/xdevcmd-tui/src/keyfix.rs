//! A Mac's keys as macOS terminals send them: the Option key, which types characters
//! unless told otherwise, and Terminal.app's Shift+F keys.
//!
//! macOS terminals make Option type characters (Option+C is `ç`) unless "Use Option as
//! Meta key" is on, so none of xdevcmd's Alt keys arrive. When such a character reaches a
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

/// Terminal.app sends Shift+F5…F12 as F13…F20 and nothing at all for Shift+F1…F4
/// (measured on the owner's Mac, 2026-11-08). An F13…F20 that nothing is bound to is
/// taken as the Shift+F key it stands for: over SSH xdevcmd cannot tell it is Terminal.app,
/// and the keyboards that have real F13 keys (Apple's extended one) bind nothing there.
pub fn shifted_f(k: KeyEvent, bound: impl Fn(KeyEvent) -> bool) -> KeyEvent {
    match k.code {
        KeyCode::F(n @ 13..=20) if k.modifiers.is_empty() && !bound(k) => KeyEvent {
            code: KeyCode::F(n - 8),
            modifiers: KeyModifiers::SHIFT,
            ..k
        },
        _ => k,
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

    #[test]
    fn terminal_apps_f13_to_f20_are_shift_f5_to_f12_unless_bound() {
        let none = |_: KeyEvent| false;
        let f = |n| key(KeyCode::F(n), KeyModifiers::NONE);
        assert_eq!(
            shifted_f(f(13), none),
            key(KeyCode::F(5), KeyModifiers::SHIFT)
        );
        assert_eq!(
            shifted_f(f(18), none),
            key(KeyCode::F(10), KeyModifiers::SHIFT)
        );
        assert_eq!(
            shifted_f(f(20), none),
            key(KeyCode::F(12), KeyModifiers::SHIFT)
        );
        // Not in the range, or with a modifier, or bound by the user: as it came.
        assert_eq!(shifted_f(f(12), none), f(12));
        assert_eq!(shifted_f(f(21), none), f(21));
        let ctrl = key(KeyCode::F(15), KeyModifiers::CONTROL);
        assert_eq!(shifted_f(ctrl, none), ctrl);
        assert_eq!(shifted_f(f(15), |_| true), f(15));
    }
}
