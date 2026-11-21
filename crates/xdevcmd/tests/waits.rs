//! A test that waits for text must wait for text that appears for that reason only
//! (review lesson L11). Twice a wait for one bare word matched something else on the
//! screen first: `wait_for("copied")` the Interrupted dialog, `wait_for("deeper ")` a
//! list row before Enter was taken (past-misses W01, W02). A class that recurs at a
//! rule moves to a check (the escalation rule): this one.
//!
//! A wait for a bare lowercase word (`"ready"`, `"deeper "`) needs a `// wait:` comment
//! on its line or the line above, saying why nothing else on the screen can show that
//! word first. Prefer text that only the awaited state shows: a status line, a pane
//! title, a dialog's own words. The waits written before this check are counted in
//! `KNOWN`; the list may only shrink.

use std::collections::BTreeMap;

/// Bare-word waits written before this check, by text: the count may go down, never up.
const KNOWN: &[(&str, usize)] = &[
    ("a ", 5),
    ("a-new", 1),
    ("aa-first", 1),
    ("arrived ", 1),
    ("at-left", 1),
    ("b-existing", 1),
    ("background", 2),
    ("closed", 2),
    ("d-older ", 2),
    ("deeper", 1),
    ("f ", 1),
    ("first ", 1),
    ("forgotten", 2),
    ("hello", 1),
    ("here", 1),
    ("history", 1),
    ("inside", 3),
    ("inside-other", 1),
    ("inside-sub", 3),
    ("later ", 1),
    ("mkdir", 1),
    ("notes", 1),
    ("other ", 1),
    ("ready", 3),
    ("selected", 1),
    ("slowdir", 2),
    ("start-up ", 1),
    ("sub", 1),
    ("view ", 1),
    ("waiting", 1),
];

/// A bare lowercase word, at most 12 characters, with one space after it or none.
fn bare(text: &str) -> bool {
    let word = text.strip_suffix(' ').unwrap_or(text);
    let mut c = word.chars();
    !word.is_empty()
        && word.len() <= 12
        && c.next().is_some_and(|f| f.is_ascii_lowercase())
        && c.all(|ch| ch.is_ascii_lowercase() || ch == '-')
}

/// Each `wait_for("…")` literal in `src` that is a bare word with no `// wait:` on its
/// line or the line before: (line number, text).
fn unexplained(src: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let explained = line.contains("// wait:")
            || (i > 0 && lines[i - 1].trim_start().starts_with("// wait:"));
        let mut rest = *line;
        while let Some(at) = rest.find("wait_for(\"") {
            rest = &rest[at + "wait_for(\"".len()..];
            let Some(end) = rest.find("\")") else { break };
            let text = &rest[..end];
            if bare(text) && !explained {
                out.push((i + 1, text.to_string()));
            }
            rest = &rest[end..];
        }
    }
    out
}

#[test]
fn the_check_fires_on_the_waits_that_escaped() {
    // L15: shown to fire on the two that got past, and quiet on what they became.
    let escaped = "x.wait_for(\"copied\");\nx.wait_for(\"deeper \");\n";
    assert_eq!(unexplained(escaped).len(), 2);
    let fixed = "x.wait_for(\"1 copied\");\nx.wait_for(\"/deeper\");\n";
    assert!(unexplained(fixed).is_empty());
    let said = "// wait: only the job's summary says it\nx.wait_for(\"copied\");\n";
    assert!(unexplained(said).is_empty());
    assert!(unexplained("x.wait_for(\"ok\"); // wait: the prompt line only\n").is_empty());
}

#[test]
fn no_new_wait_for_a_bare_word() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") || path.ends_with("waits.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        for (line, text) in unexplained(&src) {
            let at = format!("{}:{line}", path.file_name().unwrap().to_string_lossy());
            found.entry(text).or_default().push(at);
        }
    }
    let known: BTreeMap<&str, usize> = KNOWN.iter().copied().collect();
    let mut new = Vec::new();
    for (text, at) in &found {
        let allowed = known.get(text.as_str()).copied().unwrap_or(0);
        if at.len() > allowed {
            new.push(format!("{text:?} at {}", at.join(", ")));
        }
    }
    assert!(
        new.is_empty(),
        "a wait for a bare word can match something else first (review lesson L11): wait \
         for text only the awaited state shows, or say why with `// wait:` on the line \
         above:\n  {}",
        new.join("\n  ")
    );
}
