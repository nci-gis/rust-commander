//! Shell completions and a man page, from one list of the command-line options, for the
//! packages (Homebrew, AUR…) to install: `--print-completions SHELL`, `--print-man`.

/// What an option takes after it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    None,
    Dir,
    File,
    Theme,
    Shell,
}

/// Every option `--help` lists (`--screenshot`'s test helpers aside), with its help.
pub const OPTIONS: &[(&str, Arg, &str)] = &[
    (
        "--no-restore",
        Arg::None,
        "start fresh instead of reopening the last session",
    ),
    (
        "--read-only",
        Arg::None,
        "write nothing: no copy, move, delete, rename, edit",
    ),
    (
        "--restricted",
        Arg::None,
        "read-only, and no shell, no editor, no clipboard",
    ),
    ("--root", Arg::Dir, "never navigate above this directory"),
    (
        "--cwd-file",
        Arg::File,
        "on quit, write the directory you were in to this file",
    ),
    (
        "--config",
        Arg::File,
        "use this config file instead of the default one",
    ),
    (
        "--print-config",
        Arg::None,
        "print every setting at its default",
    ),
    (
        "--print-theme",
        Arg::Theme,
        "print a theme (built in, or a file in themes/) as a file",
    ),
    ("--print-keys", Arg::None, "print every key as Markdown"),
    (
        "--print-completions",
        Arg::Shell,
        "print completions for bash, zsh, fish or powershell",
    ),
    ("--print-man", Arg::None, "print this manual page (roff)"),
    (
        "--check-config",
        Arg::None,
        "validate the config file; exit 1 if it is wrong",
    ),
    (
        "--show-keys",
        Arg::None,
        "print each key and click as xdevcmd receives it",
    ),
    ("--version", Arg::None, "print the version and exit"),
    ("--help", Arg::None, "print the help and exit"),
];

const THEMES: &str = "xdevtheme dark light ansi blue high-contrast";
const SHELLS: &str = "bash zsh fish powershell";

/// Completions for `shell`, or why not.
pub fn completions(shell: &str) -> Result<String, String> {
    match shell {
        "bash" => Ok(bash()),
        "zsh" => Ok(zsh()),
        "fish" => Ok(fish()),
        "powershell" | "pwsh" => Ok(powershell()),
        _ => Err(format!(
            "--print-completions {shell}: expected one of {SHELLS}"
        )),
    }
}

fn bash() -> String {
    let all: Vec<&str> = OPTIONS.iter().map(|(o, ..)| *o).collect();
    let mut s = String::from(
        "# bash completion for xdevcmd: source it, or put it in bash-completion's directory\n",
    );
    s.push_str("_xdevcmd() {\n    local cur prev\n    cur=\"${COMP_WORDS[COMP_CWORD]}\"\n    prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n    case \"$prev\" in\n");
    for (o, a, _) in OPTIONS {
        let words = match a {
            Arg::Dir => "        COMPREPLY=($(compgen -d -- \"$cur\")); return ;;".to_string(),
            Arg::File => "        COMPREPLY=($(compgen -f -- \"$cur\")); return ;;".to_string(),
            Arg::Theme => {
                format!("        COMPREPLY=($(compgen -W \"{THEMES}\" -- \"$cur\")); return ;;")
            }
            Arg::Shell => {
                format!("        COMPREPLY=($(compgen -W \"{SHELLS}\" -- \"$cur\")); return ;;")
            }
            Arg::None => continue,
        };
        s.push_str(&format!("        {o})\n{words}\n"));
    }
    s.push_str("    esac\n    if [[ \"$cur\" == -* ]]; then\n");
    s.push_str(&format!(
        "        COMPREPLY=($(compgen -W \"{}\" -- \"$cur\"))\n",
        all.join(" ")
    ));
    s.push_str("    else\n        COMPREPLY=($(compgen -d -- \"$cur\"))\n    fi\n}\ncomplete -F _xdevcmd xdevcmd\n");
    s
}

fn zsh() -> String {
    let mut s = String::from("#compdef xdevcmd\n# zsh completion for xdevcmd: put it in a directory on $fpath as _xdevcmd\n_arguments \\\n");
    for (o, a, help) in OPTIONS {
        let help = help.replace('\'', "'\\''").replace(':', "\\:");
        let tail = match a {
            Arg::None => String::new(),
            Arg::Dir => ":directory:_files -/".into(),
            Arg::File => ":file:_files".into(),
            Arg::Theme => format!(":theme:({THEMES})"),
            Arg::Shell => format!(":shell:({SHELLS})"),
        };
        s.push_str(&format!("  '{o}[{help}]{tail}' \\\n"));
    }
    s.push_str("  '1:left directory:_files -/' \\\n  '2:right directory:_files -/'\n");
    s
}

fn fish() -> String {
    let mut s = String::from(
        "# fish completion for xdevcmd: put it in ~/.config/fish/completions/xdevcmd.fish\n",
    );
    s.push_str("complete -c xdevcmd -f -a '(__fish_complete_directories)'\n");
    for (o, a, help) in OPTIONS {
        let long = o.trim_start_matches("--");
        let help = help.replace('\'', "\\'");
        let tail = match a {
            Arg::None => String::new(),
            Arg::Dir => " -r -a '(__fish_complete_directories)'".into(),
            Arg::File => " -r -F".into(),
            Arg::Theme => format!(" -r -f -a '{THEMES}'"),
            Arg::Shell => format!(" -r -f -a '{SHELLS}'"),
        };
        s.push_str(&format!(
            "complete -c xdevcmd -l {long} -d '{help}'{tail}\n"
        ));
    }
    s
}

fn powershell() -> String {
    let mut s =
        String::from("# PowerShell completion for xdevcmd: dot-source it from your $PROFILE\n");
    s.push_str("Register-ArgumentCompleter -Native -CommandName xdevcmd -ScriptBlock {\n    param($wordToComplete, $commandAst, $cursorPosition)\n    $options = @(\n");
    for (o, _, help) in OPTIONS {
        s.push_str(&format!(
            "        @('{o}', '{}')\n",
            help.replace('\'', "''")
        ));
    }
    s.push_str("    )\n    $options | Where-Object { $_[0] -like \"$wordToComplete*\" } | ForEach-Object {\n        [System.Management.Automation.CompletionResult]::new($_[0], $_[0], 'ParameterName', $_[1])\n    }\n}\n");
    s
}

/// The manual page, in roff.
pub fn man() -> String {
    let esc = |t: &str| t.replace('\\', "\\e").replace('-', "\\-");
    let mut s = format!(
        ".TH XDEVCMD 1 \"\" \"xdevcmd {}\" \"User Commands\"\n.SH NAME\nxdevcmd \\- {}\n",
        env!("CARGO_PKG_VERSION"),
        esc(env!("CARGO_PKG_DESCRIPTION"))
    );
    s.push_str(".SH SYNOPSIS\n.B xdevcmd\n[\\fIOPTIONS\\fR] [\\fILEFT\\fR] [\\fIRIGHT\\fR]\n");
    s.push_str(".SH DESCRIPTION\nA two-pane file manager for the terminal, in the tradition of Total Commander: \
copy, move, rename and compare between two panes, with archives, a terminal panel and \
undo. Press F1 inside it for every command and its key, as configured; Help, Config \
in use shows the settings in effect.\n");
    s.push_str(".SH OPTIONS\n");
    for (o, a, help) in OPTIONS {
        let arg = match a {
            Arg::None => "",
            Arg::Dir => " \\fIDIR\\fR",
            Arg::File => " \\fIPATH\\fR",
            Arg::Theme => " \\fINAME\\fR",
            Arg::Shell => " \\fISHELL\\fR",
        };
        s.push_str(&format!(".TP\n\\fB{}\\fR{arg}\n{}\n", esc(o), esc(help)));
    }
    s.push_str(".SH FILES\n.TP\n\\fI~/.config/xdevcmd/config.toml\\fR\nThe configuration (or \\fB$XDEVCMD_CONFIG\\fR, \\fB$XDG_CONFIG_HOME/xdevcmd/config.toml\\fR).\n");
    s.push_str(".TP\n\\fI~/.config/xdevcmd/themes/\\fR\nYour own themes; see \\fB\\-\\-print\\-theme\\fR.\n");
    s.push_str(".TP\n\\fI~/.local/state/xdevcmd/\\fR\nThe session, safe operations' journal and crash logs (or \\fB$XDEVCMD_STATE\\fR, \\fB$XDG_STATE_HOME/xdevcmd\\fR).\n");
    s.push_str(".SH SEE ALSO\nhttps://github.com/nci\\-gis/rust\\-commander\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_is_in_the_help_and_every_shell_gets_completions() {
        for (o, ..) in OPTIONS {
            assert!(crate::USAGE.contains(o), "{o} is missing from --help");
        }
        for sh in ["bash", "zsh", "fish", "powershell"] {
            let c = completions(sh).unwrap();
            assert!(c.contains("read-only") && c.contains("print-theme"), "{sh}");
        }
        assert!(completions("csh").is_err());
        assert!(bash().contains("complete -F _xdevcmd xdevcmd"));
        assert!(zsh().starts_with("#compdef xdevcmd"));
    }

    #[test]
    fn the_man_page_is_roff_with_every_option() {
        let m = man();
        assert!(m.starts_with(".TH XDEVCMD 1"));
        for (o, ..) in OPTIONS {
            assert!(m.contains(&o.replace('-', "\\-")), "{o}");
        }
    }
}
