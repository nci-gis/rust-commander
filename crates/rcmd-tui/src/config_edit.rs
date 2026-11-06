//! Changing a few values in someone's `config.toml` without rewriting it.
//!
//! The file is the user's: comments, order and blank lines are theirs and stay. Only the
//! line holding a changed key is touched — its value replaced, a trailing comment kept —
//! and a key that is not there yet is added at the end of its `[section]`, or a new
//! section at the end of the file. No TOML library keeps comments without a new
//! dependency, and these files are flat `key = value` lines; the caller checks the
//! result by parsing it, and refuses to save if it does not say what was meant.

/// A value as TOML writes it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl Value {
    fn toml(&self) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Str(s) => {
                let mut out = String::from("\"");
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
                        c => out.push(c),
                    }
                }
                out.push('"');
                out
            }
        }
    }
}

/// The section a header line opens: `[general]` → `general`. `None` for anything else,
/// an array of tables (`[[open]]`) included.
fn header(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.starts_with("[[") {
        return Some("");
    }
    t.strip_prefix('[')?.split(']').next().map(str::trim)
}

/// The key a `key = value` line sets, if it is one.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    let (k, _) = t.split_once('=')?;
    let k = k.trim();
    (!k.is_empty()).then_some(k)
}

/// Where a trailing `# comment` starts on a value line, outside any string.
fn comment_at(rest: &str) -> Option<usize> {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        match c {
            '\\' if in_str && !escaped => {
                escaped = true;
                continue;
            }
            '"' if !escaped => in_str = !in_str,
            '#' if !in_str => return Some(i),
            _ => {}
        }
        escaped = false;
    }
    None
}

/// `text` with `[section] key = value` set.
pub fn set(text: &str, section: &str, key: &str, value: &Value) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut current: Option<&str> = Some(""); // top level, before any header
    let mut last_in_section: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(h) = header(line) {
            current = Some(h);
            if h == section {
                last_in_section = Some(i);
            }
            continue;
        }
        if current != Some(section) {
            continue;
        }
        // After the section's last key, not after a comment that introduces the next.
        if key_of(line).is_some() {
            last_in_section = Some(i);
        }
        if key_of(line) == Some(key) {
            let eq = line.find('=').expect("a key line has an =");
            let rest = &line[eq + 1..];
            let comment = comment_at(rest)
                .map(|c| format!("   {}", rest[c..].trim_end()))
                .unwrap_or_default();
            let new = format!("{} {}{comment}", &line[..=eq], value.toml());
            let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            out[i] = new;
            return join(out, text);
        }
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let entry = format!("{key} = {}", value.toml());
    match last_in_section {
        Some(i) => out.insert(i + 1, entry),
        None => {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{section}]"));
            out.push(entry);
        }
    }
    join(out, text)
}

/// Lines back into a file, with the line ending the file used and a final newline.
fn join(lines: Vec<String>, original: &str) -> String {
    let nl = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut s = lines.join(nl);
    s.push_str(nl);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# my rcmd\n\n[general]\nmenu_bar = true   # the top row\nautosave_secs = 5\n\n# files, my way\n[files]\nicons = false\n\n[[menu]]\nkey = \"t\"\nlabel = \"top\"\ncommand = \"top\"\n";

    #[test]
    fn a_changed_value_keeps_its_line_its_comment_and_everything_else() {
        let out = set(FILE, "general", "menu_bar", &Value::Bool(false));
        assert!(out.contains("menu_bar = false   # the top row"), "{out}");
        assert!(out.starts_with("# my rcmd\n\n[general]\n"), "{out}");
        assert!(
            out.contains("# files, my way\n[files]\nicons = false"),
            "{out}"
        );
        assert_eq!(out.lines().count(), FILE.lines().count());
    }

    #[test]
    fn a_new_key_goes_at_the_end_of_its_section() {
        let out = set(FILE, "general", "delete_to_trash", &Value::Bool(false));
        assert!(
            out.contains("autosave_secs = 5\ndelete_to_trash = false\n\n# files"),
            "{out}"
        );
        let out = set(FILE, "files", "enter", &Value::Str("view".into()));
        assert!(
            out.contains("icons = false\nenter = \"view\"\n\n[[menu]]"),
            "{out}"
        );
    }

    #[test]
    fn a_new_section_goes_at_the_end_and_an_empty_file_gets_one() {
        let out = set(FILE, "theme", "name", &Value::Str("light".into()));
        assert!(
            out.ends_with("command = \"top\"\n\n[theme]\nname = \"light\"\n"),
            "{out}"
        );
        assert_eq!(
            set("", "theme", "name", &Value::Int(3)),
            "[theme]\nname = 3\n"
        );
    }

    #[test]
    fn a_key_of_the_same_name_in_another_section_is_left_alone() {
        let text = "[viewer]\nwrap = true\n[general]\nmenu_bar = true\n";
        let out = set(text, "general", "wrap", &Value::Bool(false));
        assert!(out.starts_with("[viewer]\nwrap = true\n"), "{out}");
        assert!(out.ends_with("menu_bar = true\nwrap = false\n"), "{out}");
    }

    #[test]
    fn strings_are_quoted_and_a_hash_inside_one_is_not_a_comment() {
        let text = "[theme]\naccent = \"#ff8800\"  # orange\n";
        let out = set(text, "theme", "accent", &Value::Str("#00ff00".into()));
        assert_eq!(out, "[theme]\naccent = \"#00ff00\"   # orange\n");
        assert_eq!(Value::Str("a\"b\\c".into()).toml(), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn windows_line_ends_stay_windows_line_ends() {
        let text = "[general]\r\nmenu_bar = true\r\n";
        assert_eq!(
            set(text, "general", "menu_bar", &Value::Bool(false)),
            "[general]\r\nmenu_bar = false\r\n"
        );
    }
}
