//! `xtask lang`: the texts the UI can show in another language (beta.4, Round 037).
//!
//! - `xtask lang template` prints every key with its English, as a language file to
//!   copy to `lang/<name>.toml` and translate (any name: `vi`, `ja`, a pseudo-language).
//! - `xtask lang check FILE` says which keys of FILE the code no longer has, and which
//!   texts name a `{param}` their English does not give (xdevcmd would show the English).
//!
//! The keys are read from the source: every `tr!("key", "English" …)` and
//! `Text::new("key", "English")` under `crates/*/src`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let keys = keys(&root)?;
    match args.first().map(String::as_str) {
        Some("template") => {
            print!("{}", template(&keys));
            Ok(())
        }
        Some("check") => {
            let file = args.get(1).ok_or("usage: xtask lang check FILE")?;
            let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            let (errors, warnings) = check(&keys, &text)?;
            for w in &warnings {
                println!("note: {w}");
            }
            for e in &errors {
                println!("error: {e}");
            }
            if errors.is_empty() {
                println!("ok: {file}");
                Ok(())
            } else {
                Err(format!(
                    "{}: {} texts would show in English",
                    file,
                    errors.len()
                ))
            }
        }
        _ => Err("usage: xtask lang template | xtask lang check FILE".into()),
    }
}

/// Every key in the source with its English; an error if one key has two Englishes.
pub fn keys(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let dirs = [
        root.join("crates/xdevcmd/src"),
        root.join("crates/xdevcmd-core/src"),
    ];
    let mut out: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (key, en, at) in calls_in(&dirs) {
        if let Some((old, old_at)) = out.get(&key) {
            if *old != en {
                return Err(format!("{key}: {old:?} at {old_at}, {en:?} at {at}"));
            }
            continue;
        }
        out.insert(key, (en, at));
    }
    Ok(out.into_iter().map(|(k, (en, _))| (k, en)).collect())
}

/// A language file with every key and its English.
pub fn template(keys: &BTreeMap<String, String>) -> String {
    let mut s = String::from(
        "# An xdevcmd language file: copy to lang/<name>.toml in xdevcmd's config folder,\n\
         # translate the texts, and set [general] language = \"<name>\". A key left out\n\
         # shows in English. Keep each {param}; {{ and }} are a brace.\n\
         # Optional, at most 32 characters: the code [general] language picks it by\n\
         # (else the file's name) and the name a user reads (else the ISO name, or the code).\n\
         # \"$__code\" = \"vi\"\n\
         # \"$__name\" = \"Tiếng Việt\"\n\n",
    );
    // One section per first part of the key: `[keybar]`, then `copy = "Copy"`.
    let mut section = None;
    for (k, en) in keys {
        let (head, rest) = k.split_once('.').unwrap_or(("", k.as_str()));
        if section != Some(head) {
            if !head.is_empty() {
                s += &format!("\n[{head}]\n");
            }
            section = Some(head);
        }
        s += &format!("{} = {}\n", toml_key(rest), toml_string(en));
    }
    s
}

/// (errors, notes) for a language file: a text naming a `{param}` its English does not
/// have (it would show in English) is an error; a key the code does not use, a note.
pub fn check(
    keys: &BTreeMap<String, String>,
    text: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut flat = Vec::new();
    flatten("", &table, &mut flat);
    let (mut errors, mut notes) = (Vec::new(), Vec::new());
    for (k, v) in &flat {
        // About the file: what `[general] language` picks it by, and its name.
        if k == "$__code" || k == "$__name" {
            match v.as_str() {
                Some(t) if t.trim().chars().count() > 32 => {
                    notes.push(format!("{k}: longer than 32 characters, cut"))
                }
                Some(t) if t.trim().is_empty() => {
                    notes.push(format!("{k}: empty, the file name is used"))
                }
                Some(_) => {}
                None => errors.push(format!("{k}: not a text")),
            }
            continue;
        }
        let Some(t) = v.as_str() else {
            errors.push(format!("{k}: not a text"));
            continue;
        };
        let Some(en) = keys.get(k) else {
            notes.push(format!("{k}: the code has no such key"));
            continue;
        };
        match (params(t), params(en)) {
            (None, _) => errors.push(format!("{k}: a brace is not closed: {t:?}")),
            (Some(mine), Some(theirs)) => {
                for p in mine.iter().filter(|p| !theirs.contains(p)) {
                    errors.push(format!("{k}: {{{p}}} is not one of {theirs:?}"));
                }
            }
            (Some(_), None) => errors.push(format!("{k}: its English is malformed: {en:?}")),
        }
    }
    Ok((errors, notes))
}

/// A table's values under dotted keys, as xdevcmd reads them (`[keybar]` then `copy`
/// is `keybar.copy`).
fn flatten<'a>(prefix: &str, table: &'a toml::Table, out: &mut Vec<(String, &'a toml::Value)>) {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::Table(t) => flatten(&key, t, out),
            v => out.push((key, v)),
        }
    }
}

/// The `{names}` in a text; `None` if a brace is not closed.
pub fn params(text: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        let tail = &rest[i..];
        if let Some(t) = tail.strip_prefix("{{").or_else(|| tail.strip_prefix("}}")) {
            rest = t;
        } else {
            // A `{` opens a name; a lone `}` is malformed.
            let t = tail.strip_prefix('{')?;
            let end = t.find('}')?;
            out.push(t[..end].to_string());
            rest = &t[end + 1..];
        }
    }
    Some(out)
}

/// A key as TOML writes it: bare when it can be, else quoted.
fn toml_key(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        k.to_string()
    } else {
        toml_string(k)
    }
}

fn toml_string(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// Every `tr!("key", "English"` and `Text::new("key", "English"` under `dirs`:
/// (key, English, file:line). Reads the source as text, one line at a time.
pub fn calls_in(dirs: &[PathBuf]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = dirs.to_vec();
    let mut files = Vec::new();
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for p in rd.flatten().map(|e| e.path()) {
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                files.push(p);
            }
        }
    }
    files.sort();
    for p in files {
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            // Comments show how to call it; they are not texts.
            if line.trim_start().starts_with("//") {
                continue;
            }
            for opener in ["tr!(\"", "Text::new(\""] {
                let mut rest = line;
                while let Some(i) = rest.find(opener) {
                    let after = &rest[i + opener.len()..];
                    let Some(kend) = after.find('"') else { break };
                    let key = &after[..kend];
                    let english = after[kend + 1..]
                        .trim_start()
                        .strip_prefix(',')
                        .map(str::trim_start)
                        .and_then(|s| s.strip_prefix('"'))
                        .and_then(literal);
                    if let Some(english) = english {
                        out.push((
                            key.to_string(),
                            english,
                            format!("{}:{}", p.display(), n + 1),
                        ));
                    }
                    rest = &after[kend..];
                }
            }
        }
    }
    out
}

/// A string literal's text up to its closing quote, escapes read.
fn literal(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn calls_are_found_in_source_text() {
        let d = std::env::temp_dir().join(format!("xtask-lang-src-{}", std::process::id()));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(
            d.join("sub/a.rs"),
            "let x = tr!(\"k.one\", \"One \\\"q\\\"\");\nlet y = tr!(\"k.two\", \"{n} two\", n = 2); const T: Text = Text::new(\"k.three\", \"Three\");\n",
        )
        .unwrap();
        let got: Vec<(String, String)> = calls_in(std::slice::from_ref(&d))
            .into_iter()
            .map(|(k, e, _)| (k, e))
            .collect();
        let want = [
            ("k.one", "One \"q\""),
            ("k.two", "{n} two"),
            ("k.three", "Three"),
        ];
        assert_eq!(got, want.map(|(k, e)| (k.to_string(), e.to_string())));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The source: each key one English, each English's braces closed, and the key bar
    /// and menus found (they are the first texts given keys).
    #[test]
    fn the_sources_keys_are_one_english_each_and_well_formed() {
        let keys = keys(&root()).unwrap();
        for (k, en) in &keys {
            assert!(params(en).is_some(), "{k}: {en:?}");
        }
        for k in ["keybar.copy", "menu.file"] {
            assert!(keys.contains_key(k), "{k} not found");
        }
    }

    /// The language files the repo ships pass the check: none would fall back.
    #[test]
    fn the_shipped_language_files_are_whole() {
        let keys = keys(&root()).unwrap();
        let dir = root().join("lang");
        let mut seen = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                let (errors, notes) = check(&keys, &std::fs::read_to_string(&p).unwrap()).unwrap();
                assert!(errors.is_empty(), "{}: {errors:?}", p.display());
                assert!(notes.is_empty(), "{}: {notes:?}", p.display());
                seen += 1;
            }
        }
        assert!(seen >= 2, "lang/ ships vi and ja");
    }

    #[test]
    fn a_check_names_what_would_show_in_english() {
        let keys: BTreeMap<String, String> = [("a".to_string(), "{n} files".to_string())]
            .into_iter()
            .collect();
        let (errors, notes) = check(&keys, "a = \"{count} tệp\"\nb = \"x\"\n").unwrap();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(notes.len(), 1, "{notes:?}");
        let (errors, _) = check(&keys, "a = \"{n tệp\"\n").unwrap();
        assert_eq!(errors.len(), 1);
        let (errors, notes) = check(
            &keys,
            "\"$__code\" = \"vi\"\n\"$__name\" = \"Tiếng Việt\"\n",
        )
        .unwrap();
        assert!(
            errors.is_empty() && notes.is_empty(),
            "{errors:?} {notes:?}"
        );
        let t = template(&keys);
        assert!(t.contains("a = \"{n} files\""), "{t}");
        let dotted: BTreeMap<String, String> = [("keybar.copy".to_string(), "Copy".to_string())]
            .into_iter()
            .collect();
        let t2 = template(&dotted);
        assert!(t2.contains("[keybar]\ncopy = \"Copy\""), "{t2}");
        assert!(
            check(&dotted, &t2).unwrap().0.is_empty() && check(&dotted, &t2).unwrap().1.is_empty()
        );
        assert!(check(&keys, &t).unwrap().0.is_empty());
    }
}
