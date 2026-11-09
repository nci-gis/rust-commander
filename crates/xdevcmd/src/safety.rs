//! Checks for text that xdevcmd is about to type into a shell or put on the clipboard.
//!
//! A path is data someone else may have chosen. Typed into a terminal, a control
//! character in it is not data any more: readline acts on Ctrl+U (kill the line) and
//! Enter before the shell ever sees the quotes around it. A directory named
//! `evil<Ctrl+U>touch PWNED<Enter>` turned Alt+C into "run touch PWNED" — reproduced,
//! then fixed here. The same text pasted from the clipboard would do the same, so
//! Copy path applies the same rule.

/// The first control character in `s`, if there is one.
pub fn control_char(s: &str) -> Option<char> {
    s.chars().find(|c| c.is_control())
}

/// Why `s` must not be typed into a terminal, or `None` if it is safe to.
pub fn unsafe_to_type(s: &str) -> Option<String> {
    control_char(s).map(|c| {
        format!(
            "the path contains a control character (U+{:04X}) \u{2014} not sent, it could run commands",
            c as u32
        )
    })
}

/// Why `text` must not go on the clipboard, or `None`. Lines and tabs are fine — that
/// is what copied text is — but any other control character would act when pasted
/// into a terminal, as Ctrl+U and Enter did in a file name typed by Alt+C.
pub fn unsafe_to_paste(text: &str) -> Option<String> {
    text.chars()
        .find(|c| c.is_control() && *c != '\n' && *c != '\t')
        .map(|c| {
            format!(
                "not copied: a control character (U+{:04X}) would act when pasted",
                c as u32
            )
        })
}

/// The command that changes a shell's directory to `path`, quoted the way that shell
/// quotes, ending in Enter. `None` with the reason when the path cannot be quoted for
/// that shell at all — refusing beats sending something that means something else.
///
/// Shells are told apart by program name, so `bash -l`, `pwsh.exe` and a full path
/// all work.
pub fn cd_command(shell: &str, path: &str) -> Result<String, String> {
    let q = quote(shell, path)?;
    let line = match shell_kind(shell) {
        Shell::Pwsh => format!("Set-Location -LiteralPath {q}"),
        Shell::Cmd => format!("cd /d {q}"),
        Shell::Fish | Shell::Posix => format!("cd {q}"),
    };
    // Enter is a carriage return from a keyboard, which every shell and ConPTY accept.
    Ok(format!("{line}\r"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Posix,
    Fish,
    Pwsh,
    Cmd,
}

/// Shells are told apart by program name, so `bash -l`, `pwsh.exe` and a full path
/// all work.
pub fn shell_kind(shell: &str) -> Shell {
    let program = shell
        .split_whitespace()
        .next()
        .unwrap_or("")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match program.strip_suffix(".exe").unwrap_or(&program) {
        "pwsh" | "powershell" => Shell::Pwsh,
        "cmd" => Shell::Cmd,
        "fish" => Shell::Fish,
        _ => Shell::Posix,
    }
}

/// `text` as one literal word for `shell`, or why it cannot be one. Refusing beats
/// sending something that means something else.
pub fn quote(shell: &str, text: &str) -> Result<String, String> {
    if let Some(why) = unsafe_to_type(text) {
        return Err(why);
    }
    Ok(match shell_kind(shell) {
        // PowerShell: single quotes are literal, and a quote inside is doubled. It also
        // takes the typographic quotes ‘ ’ ‚ ‛ as single quotes, so a name like
        // `it’s` would end the string early: each is doubled the same way.
        Shell::Pwsh => {
            let mut q = String::with_capacity(text.len() + 2);
            q.push('\'');
            for c in text.chars() {
                if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
                    q.push(c);
                }
                q.push(c);
            }
            q.push('\'');
            q
        }
        // cmd.exe has no quoting that stops %VAR% expansion, and a path cannot hold ".
        Shell::Cmd => {
            if let Some(c) = text.chars().find(|c| matches!(c, '%' | '"' | '!' | '^')) {
                return Err(format!(
                    "cmd.exe cannot be given text containing {c:?} safely \u{2014} not sent"
                ));
            }
            format!("\"{text}\"")
        }
        // fish: inside single quotes, backslash and quote are escapes.
        Shell::Fish => format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'")),
        // POSIX shells and anything unknown: close the quote, escape one, reopen.
        Shell::Posix => format!("'{}'", text.replace('\'', r"'\''")),
    })
}

/// File names as words for `shell`'s command line: each quoted, joined by spaces,
/// with a space after the last so the next word can be typed straight on, and no
/// Enter. One that cannot be typed refuses the whole list: a list with a name missing
/// would be a different command. A name starting with `-` gets `./` in front, so
/// `rm ` followed by a file called `-rf` is not an option.
pub fn words(shell: &str, names: &[&str]) -> Result<String, String> {
    let mut out = String::new();
    for name in names {
        out.push_str(&word(shell, name)?);
        out.push(' ');
    }
    Ok(out)
}

/// One name as one word for `shell`: quoted, and a name starting with `-` given a
/// `./` in front so the command takes it for a file, not an option (`rm -rf` from a
/// file called `-rf`). The rule for every name xdevcmd types, here and in the user
/// menu.
pub fn word(shell: &str, name: &str) -> Result<String, String> {
    let dashed = name.starts_with('-') && !matches!(shell_kind(shell), Shell::Cmd);
    if dashed {
        quote(shell, &format!(".{}{name}", std::path::MAIN_SEPARATOR))
    } else {
        quote(shell, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shell_gets_its_own_quoting() {
        let p = "/home/me/it's here";
        assert_eq!(
            cd_command("bash", p).unwrap(),
            "cd '/home/me/it'\\''s here'\r"
        );
        assert_eq!(
            cd_command("/bin/zsh -l", p).unwrap(),
            "cd '/home/me/it'\\''s here'\r"
        );
        assert_eq!(
            cd_command("fish", p).unwrap(),
            "cd '/home/me/it\\'s here'\r"
        );
        let w = r"C:\Users\me\it's here";
        assert_eq!(
            cd_command("pwsh.exe", w).unwrap(),
            "Set-Location -LiteralPath 'C:\\Users\\me\\it''s here'\r"
        );
        assert_eq!(
            cd_command(r"C:\Windows\System32\cmd.exe", w).unwrap(),
            "cd /d \"C:\\Users\\me\\it's here\"\r"
        );
    }

    #[test]
    fn cmd_refuses_what_it_cannot_quote() {
        assert!(cd_command("cmd.exe", r"C:\a%PATH%b")
            .unwrap_err()
            .contains("'%'"));
    }

    #[test]
    fn names_are_words_for_each_shell() {
        let names = ["a b", "it's", "$HOME", "bang!", "50%"];
        assert_eq!(
            words("bash", &names).unwrap(),
            "'a b' 'it'\\''s' '$HOME' 'bang!' '50%' "
        );
        assert_eq!(
            words("/usr/bin/fish", &names).unwrap(),
            "'a b' 'it\\'s' '$HOME' 'bang!' '50%' "
        );
        assert_eq!(
            words("pwsh", &names).unwrap(),
            "'a b' 'it''s' '$HOME' 'bang!' '50%' "
        );
        // cmd.exe cannot be given % or ! safely: the whole list is refused.
        assert!(words("cmd.exe", &names).is_err());
        assert_eq!(
            words("cmd.exe", &["a b", "it's", "$HOME"]).unwrap(),
            "\"a b\" \"it's\" \"$HOME\" "
        );
        // One control character refuses every name, in every shell.
        for sh in ["bash", "fish", "pwsh", "cmd.exe"] {
            assert!(words(sh, &["fine", "bad\u{15}x"]).is_err(), "{sh}");
        }
        assert_eq!(words("bash", &[]).unwrap(), "");
    }

    #[test]
    fn a_name_that_looks_like_an_option_is_not_one() {
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(words("bash", &["-rf"]).unwrap(), format!("'.{sep}-rf' "));
        assert_eq!(words("cmd", &["-rf"]).unwrap(), "\"-rf\" ");
    }

    #[test]
    fn powershell_doubles_its_typographic_quotes_too() {
        assert_eq!(
            quote("pwsh", "it\u{2019}s").unwrap(),
            "'it\u{2019}\u{2019}s'"
        );
        assert_eq!(
            quote("powershell.exe", "\u{2018}a\u{201a}b\u{201b}").unwrap(),
            "'\u{2018}\u{2018}a\u{201a}\u{201a}b\u{201b}\u{201b}'"
        );
        // Other shells leave them alone: they are ordinary characters there.
        assert_eq!(quote("bash", "it\u{2019}s").unwrap(), "'it\u{2019}s'");
    }

    #[test]
    fn every_shell_refuses_control_characters() {
        for sh in ["bash", "fish", "pwsh", "cmd.exe"] {
            assert!(cd_command(sh, "/x\u{15}touch PWNED\r").is_err(), "{sh}");
        }
    }

    #[test]
    fn the_attack_that_worked_is_caught() {
        assert!(unsafe_to_type("/tmp/l/evil\u{15}touch PWNED\r").is_some());
    }

    #[test]
    fn every_kind_of_control_character_counts() {
        for c in [
            '\n', '\r', '\t', '\u{3}', '\u{15}', '\u{1b}', '\u{7f}', '\u{9b}',
        ] {
            assert!(
                control_char(&format!("a{c}b")).is_some(),
                "U+{:04X}",
                c as u32
            );
        }
    }

    #[test]
    fn ordinary_names_pass_including_quotes_and_vietnamese() {
        for ok in [
            "/home/me/projects",
            "it's here",
            "báo cáo tuần",
            "a b  c",
            "$(x)",
        ] {
            assert_eq!(unsafe_to_type(ok), None, "{ok}");
        }
    }
}
