//! Control characters, in text that names a file or is typed, pasted or shown: one
//! definition (Round 035, W3). A control character (a newline, ESC, DEL, a C1 code) is
//! legal in a Unix name and acts when the name is listed, typed into a shell or pasted:
//! readline ran Ctrl+U and Enter from a directory's name (safety.rs). Each place says
//! which ones it lets through, a tab in a line of text, a newline in a copied block.

/// Whether `c` is a control character: Unicode's Cc (C0, DEL, C1).
pub fn is(c: char) -> bool {
    c.is_control()
}

/// The first control character in `s` that `allowed` does not let through.
pub fn first(s: &str, allowed: &[char]) -> Option<char> {
    s.chars().find(|&c| is(c) && !allowed.contains(&c))
}

/// How a message names it, the same everywhere: `a control character (U+001B)`.
pub fn named(c: char) -> String {
    format!("a control character (U+{:04X})", c as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c0_del_and_c1_are_control_and_letters_are_not() {
        for c in ['\0', '\n', '\t', '\x1b', '\x7f', '\u{85}', '\u{9b}'] {
            assert!(is(c), "{c:?}");
        }
        for c in ['a', ' ', 'é', '設', '\u{200b}', '\u{2028}'] {
            assert!(!is(c), "{c:?}");
        }
    }

    #[test]
    fn first_skips_what_is_allowed() {
        assert_eq!(first("a\tb\x1bc", &[]), Some('\t'));
        assert_eq!(first("a\tb\x1bc", &['\t']), Some('\x1b'));
        assert_eq!(first("a\nb\tc", &['\n', '\t']), None);
        assert_eq!(first("plain", &[]), None);
    }

    #[test]
    fn named_spells_the_code_point() {
        assert_eq!(named('\x1b'), "a control character (U+001B)");
        assert_eq!(named('\u{9b}'), "a control character (U+009B)");
    }
}
