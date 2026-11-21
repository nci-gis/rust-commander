//! The hand-test cases keep their shape, and their oracle is the user docs (review
//! lesson L16, past-misses C14): a case checks what xdevcmd promises a user, so text
//! it quotes as Expected must be in `docs/`, not only in the code or a pty test.
//!
//! - Every case is `### M-XX-NN Title`, with an ID used once.
//! - Every case has **Platform**, **Steps**, **Expected** and **Fail if**.
//! - Every `"quoted"` text on an **Expected** line (four characters or more) is in
//!   `docs/*.md`; `…` stands for a part that varies. Whitespace and backticks aside.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/manual/test-cases")
}

fn flat(s: &str) -> String {
    s.replace('`', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Each case of `md`: (heading, its whole text).
fn cases(md: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in md.lines() {
        if let Some(h) = line.strip_prefix("### ") {
            out.push((h.to_string(), String::new()));
        } else if let Some((_, text)) = out.last_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    out
}

/// The **Expected** part of a case: from its marker to the next `**…**:` field.
fn expected(text: &str) -> &str {
    let Some(at) = text.find("**Expected**:") else {
        return "";
    };
    let rest = &text[at + "**Expected**:".len()..];
    let end = ["\n**Fail if**", "\n**Steps**", "\n**Platform**"]
        .iter()
        .filter_map(|m| rest.find(m))
        .min()
        .unwrap_or(rest.len());
    &rest[..end]
}

/// What a case breaks: (case, why) for each rule it does not keep.
fn problems(file: &str, md: &str, docs: &str, seen: &mut BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (head, text) in cases(md) {
        let id = head.split_whitespace().next().unwrap_or("");
        let shaped = id.len() == 7
            && id.starts_with("M-")
            && id[2..4].bytes().all(|b| b.is_ascii_uppercase())
            && &id[4..5] == "-"
            && id[5..].bytes().all(|b| b.is_ascii_digit())
            && head.len() > 8;
        if !shaped {
            out.push(format!("{file}: `### {head}` is not `### M-XX-NN Title`"));
            continue;
        }
        if !seen.insert(id.to_string()) {
            out.push(format!("{file}: {id} is used twice"));
        }
        for field in ["**Platform**", "**Steps**", "**Expected**", "**Fail if**"] {
            if !text.contains(field) {
                out.push(format!("{file}: {id} has no {field}"));
            }
        }
        let exp = expected(&text);
        for (i, q) in exp.split('"').enumerate() {
            if i % 2 == 0 || q.chars().count() < 4 {
                continue;
            }
            for part in q.split('…').map(flat).filter(|p| !p.is_empty()) {
                if !docs.contains(&part) {
                    out.push(format!(
                        "{file}: {id} quotes {part:?}, which docs/ does not say"
                    ));
                }
            }
        }
    }
    out
}

fn docs_text() -> String {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut all = String::new();
    for e in std::fs::read_dir(docs).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "md") {
            all.push_str(&std::fs::read_to_string(e.path()).unwrap());
            all.push('\n');
        }
    }
    flat(&all)
}

#[test]
fn the_check_fires() {
    // L15: shown to fire on each rule, quiet on a case that keeps them.
    let docs = flat("the job box says \"about 4 min left\" and ends");
    let good = "### M-RJ-01 Time left\n**Platform**: all\n**Steps**: copy\n**Expected**: \"about … min left\"\n**Fail if**: none\n";
    assert!(problems("f", good, &docs, &mut BTreeSet::new()).is_empty());
    let bad = "### M-RJ-01 Time left\n**Platform**: all\n**Expected**: \"Undo the copy of …?\"\n### RJ-2 x\n### M-RJ-01 again\n**Platform**: a\n**Steps**: b\n**Expected**: c\n**Fail if**: d\n";
    let p = problems("f", bad, &docs, &mut BTreeSet::new());
    assert!(p.iter().any(|x| x.contains("no **Steps**")), "{p:?}");
    assert!(p.iter().any(|x| x.contains("no **Fail if**")), "{p:?}");
    assert!(
        p.iter().any(|x| x.contains("which docs/ does not say")),
        "{p:?}"
    );
    assert!(
        p.iter().any(|x| x.contains("is not `### M-XX-NN Title`")),
        "{p:?}"
    );
    assert!(p.iter().any(|x| x.contains("used twice")), "{p:?}");
}

#[test]
fn every_case_keeps_its_shape_and_quotes_the_docs() {
    let docs = docs_text();
    let mut seen = BTreeSet::new();
    let mut all = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
    {
        let md = std::fs::read_to_string(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        all.extend(problems(&name, &md, &docs, &mut seen));
    }
    assert!(!seen.is_empty(), "no case found in {}", dir().display());
    assert!(
        all.is_empty(),
        "manual cases (README \"A case\"; lesson L16):\n  {}",
        all.join("\n  ")
    );
}
