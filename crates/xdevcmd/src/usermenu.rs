//! The user menu (F9): commands from `[[menu]]` in the config, typed into the
//! terminal panel so their output is where you can read it.
//!
//! A command is yours — it comes from your config, which xdevcmd refuses to load when
//! others can write it. What fills its placeholders is not: file names are chosen by
//! whoever made the files. Every substitution is quoted for the shell it is going to,
//! and one that cannot be quoted safely (a control character, or `%` for cmd.exe) stops
//! the command rather than sending something else.
//!
//! | | |
//! |---|---|
//! | `%f` / `%F` | the file under the cursor: name / full path |
//! | `%s` / `%S` | the selection (marked, else the cursor): names / full paths |
//! | `%d` | the pane's directory |
//! | `%D` | the other pane's directory |
//! | `%%` | a percent sign |

use std::path::{Path, PathBuf};

use crate::safety::word;

pub struct Context {
    pub shell: String,
    pub current: Option<PathBuf>,
    pub dir: PathBuf,
    pub selection: Vec<PathBuf>,
    pub other: Option<PathBuf>,
}

/// A lossy name would be another file, or none: refused, as typing names refuses it.
fn text(p: &Path) -> Result<String, String> {
    p.to_str()
        .map(str::to_string)
        .ok_or_else(|| "a name is not UTF-8 \u{2014} not typed".to_string())
}

fn name(p: &Path) -> Result<String, String> {
    text(p.file_name().map(Path::new).unwrap_or(p))
}

pub fn expand(template: &str, cx: &Context) -> Result<String, String> {
    let q = |s: &str| word(&cx.shell, s);
    let mut out = String::new();
    let mut it = template.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(code) = it.next() else {
            return Err("the command ends in a lone %".into());
        };
        let words: Vec<String> = match code {
            '%' => {
                out.push('%');
                continue;
            }
            'f' | 'F' => {
                let Some(p) = &cx.current else {
                    return Err(format!("%{code} needs a file under the cursor"));
                };
                vec![if code == 'f' { name(p)? } else { text(p)? }]
            }
            's' | 'S' => {
                if cx.selection.is_empty() {
                    return Err(format!("%{code} needs something selected"));
                }
                cx.selection
                    .iter()
                    .map(|p| if code == 's' { name(p) } else { text(p) })
                    .collect::<Result<_, _>>()?
            }
            'd' => vec![text(&cx.dir)?],
            'D' => match &cx.other {
                Some(d) => vec![text(d)?],
                None => return Err("%D needs a second pane".into()),
            },
            other => {
                return Err(format!(
                    "%{other} is not a placeholder (%f %F %s %S %d %D %%)"
                ))
            }
        };
        let quoted: Result<Vec<String>, String> = words.iter().map(|w| q(w)).collect();
        out.push_str(&quoted?.join(" "));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx(shell: &str) -> Context {
        Context {
            shell: shell.into(),
            current: Some(PathBuf::from("/w/it's.txt")),
            dir: PathBuf::from("/w"),
            selection: vec![PathBuf::from("/w/a b"), PathBuf::from("/w/c")],
            other: Some(PathBuf::from("/x")),
        }
    }

    #[test]
    fn a_name_like_an_option_is_a_file_and_a_lossy_name_is_refused() {
        let mut c = cx("bash");
        c.current = Some(PathBuf::from("/w/-rf"));
        c.selection = vec![PathBuf::from("/w/-n"), PathBuf::from("/w/c")];
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(expand("rm %f", &c).unwrap(), format!("rm '.{sep}-rf'"));
        assert_eq!(expand("ls %s", &c).unwrap(), format!("ls '.{sep}-n' 'c'"));
        // As typed names do: the same rule, the same place.
        assert_eq!(
            expand("rm %f", &c).unwrap(),
            format!(
                "rm {}",
                crate::safety::words("bash", &["-rf"]).unwrap().trim_end()
            )
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let bad = std::ffi::OsStr::from_bytes(b"/w/caf\xe9");
            c.current = Some(PathBuf::from(bad));
            assert!(expand("cat %f", &c).unwrap_err().contains("not UTF-8"));
            assert!(expand("cat %F", &c).unwrap_err().contains("not UTF-8"));
            c.selection = vec![PathBuf::from(bad)];
            assert!(expand("cat %s", &c).unwrap_err().contains("not UTF-8"));
        }
    }

    #[test]
    fn placeholders_are_quoted_for_the_shell() {
        assert_eq!(
            expand("cp %S %D", &cx("bash")).unwrap(),
            "cp '/w/a b' '/w/c' '/x'"
        );
        assert_eq!(
            expand("echo %f 100%%", &cx("bash")).unwrap(),
            "echo 'it'\\''s.txt' 100%"
        );
        assert_eq!(expand("echo %f", &cx("pwsh")).unwrap(), "echo 'it''s.txt'");
    }

    #[test]
    fn a_name_that_cannot_be_typed_safely_stops_the_command() {
        let mut c = cx("bash");
        c.current = Some(PathBuf::from("/w/evil\u{15}rm -rf ~\r"));
        assert!(expand("cat %f", &c)
            .unwrap_err()
            .contains("control character"));
        let mut c = cx("cmd");
        c.current = Some(PathBuf::from("C:/w/100%.txt"));
        assert!(expand("type %f", &c).is_err(), "cmd.exe expands %");
    }

    #[test]
    fn mistakes_in_the_template_are_named() {
        assert!(expand("x %q", &cx("bash")).unwrap_err().contains("%q"));
        assert!(expand("x %", &cx("bash")).is_err());
        let mut c = cx("bash");
        c.other = None;
        assert!(expand("ls %D", &c).unwrap_err().contains("second pane"));
    }
}
