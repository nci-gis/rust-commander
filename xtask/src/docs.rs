//! `xtask docs`: are the user docs still true? (Round 038, the owner 2026-11-25.)
//!
//! - `xtask docs status`: for each page in scope, whether it was reviewed, and whether
//!   the page or the code it speaks of changed since (the ledger,
//!   `.agents/ledgers/docs-review.tsv`, written by the `docs-review` skill).
//! - `xtask docs check`: what a machine can check: links and their `#anchors`, the
//!   `xdevcmd --flags` and `xtask` words the docs name, `XDEVCMD_*` variables, and the
//!   `[section] key`s, against `xdevcmd --help`, the source and `--print-config`.
//!
//! The sentences about behaviour are the skill's: an agent reads them against the code.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LEDGER: &str = ".agents/ledgers/docs-review.tsv";

/// The pages in scope, and the source each speaks of: a change there since a page's
/// review marks it "code changed". Coarse on purpose: it orders the review, nothing more.
/// Out: `.agents/` (history), `docs/keys.md` (generated, tested), `CHANGELOG.md`.
pub const DOC_PATHS: &[(&str, &[&str])] = &[
    (
        "README.md",
        &[
            "crates/xdevcmd/src/main.rs",
            "Cargo.toml",
            ".github/workflows/release.yml",
        ],
    ),
    (
        "SECURITY.md",
        &[
            "crates/xdevcmd/src/safety.rs",
            "crates/xdevcmd-core/src/protect.rs",
        ],
    ),
    (
        "crates/xdevcmd-core/README.md",
        &[
            "crates/xdevcmd-core/src/lib.rs",
            "crates/xdevcmd-core/Cargo.toml",
        ],
    ),
    ("docs/README.md", &[]),
    (
        "docs/benchmark.md",
        &[
            "xtask/src/main.rs",
            "crates/xdevcmd-core/src/fastcopy.rs",
            ".github/workflows/bench.yml",
        ],
    ),
    (
        "docs/compatibility.md",
        &[
            "crates/xdevcmd/src/graphics.rs",
            "crates/xdevcmd/src/keyfix.rs",
            "crates/xdevcmd/src/pane/term.rs",
            "crates/xdevcmd-core/src/os.rs",
        ],
    ),
    (
        "docs/config.md",
        &[
            "crates/xdevcmd/src/config.rs",
            "crates/xdevcmd/src/settings_dialog.rs",
            "crates/xdevcmd/src/keys.rs",
            "crates/xdevcmd/src/theme.rs",
            "crates/xdevcmd/src/usermenu.rs",
            "crates/xdevcmd/src/opener.rs",
            "crates/xdevcmd/src/bookmarks.rs",
        ],
    ),
    (
        "docs/development.md",
        &["xtask/src", ".github/workflows", "scripts", "release.toml"],
    ),
    (
        "docs/guide.md",
        &[
            "crates/xdevcmd/src/app",
            "crates/xdevcmd/src/modal.rs",
            "crates/xdevcmd/src/viewer.rs",
            "crates/xdevcmd/src/keys.rs",
            "crates/xdevcmd/src/pane/files.rs",
            "crates/xdevcmd-core/src/plan.rs",
            "crates/xdevcmd-core/src/rename.rs",
            "crates/xdevcmd-core/src/ops",
        ],
    ),
    (
        "docs/how-to-test.md",
        &[
            "xtask/src/tryout.rs",
            "xtask/src/manual.rs",
            "crates/xdevcmd/tests/manual",
            "crates/xdevcmd-core/src/faults.rs",
        ],
    ),
    ("docs/known-issues.md", &["crates"]),
    (
        "docs/language.md",
        &["crates/xdevcmd/src/lang.rs", "xtask/src/lang.rs", "lang"],
    ),
    (
        "docs/security.md",
        &[
            "crates/xdevcmd/src/safety.rs",
            "crates/xdevcmd/src/config.rs",
            "crates/xdevcmd/src/opener.rs",
            "crates/xdevcmd/src/usermenu.rs",
            "crates/xdevcmd/src/git.rs",
            "crates/xdevcmd-core/src/protect.rs",
        ],
    ),
    (
        "docs/shell.md",
        &[
            "crates/xdevcmd/src/pane/term.rs",
            "crates/xdevcmd/src/pane/terms.rs",
            "crates/xdevcmd/src/pane/shell_report.rs",
            "crates/xdevcmd/src/app/shell.rs",
            "crates/xdevcmd/src/links.rs",
            "crates/xdevcmd/src/safety.rs",
        ],
    ),
    (
        "docs/troubleshooting.md",
        &[
            "crates/xdevcmd/src/main.rs",
            "crates/xdevcmd/src/crash.rs",
            "crates/xdevcmd/src/graphics.rs",
            "crates/xdevcmd/src/config.rs",
        ],
    ),
];

pub fn run(args: &[String]) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    match args.first().map(String::as_str) {
        Some("status") => {
            let rows = status(&root)?;
            let width = rows.iter().map(|(p, _)| p.len()).max().unwrap_or(0);
            for (page, state) in &rows {
                println!("{page:width$}  {state}");
            }
            let todo = rows.iter().filter(|(_, s)| !s.starts_with("ok")).count();
            println!("\n{todo} of {} to review ({LEDGER})", rows.len());
            Ok(())
        }
        Some("check") => {
            let problems = check(&root)?;
            for p in &problems {
                println!("{p}");
            }
            if problems.is_empty() {
                println!("ok: links, anchors, flags, variables and config keys");
                Ok(())
            } else {
                Err(format!("{} problems in the docs", problems.len()))
            }
        }
        _ => Err("usage: xtask docs status | check".into()),
    }
}

/// One ledger row: the last review of a page.
#[derive(Debug, PartialEq)]
struct Review {
    hash: String,
    commit: String,
}

fn ledger(root: &Path) -> Result<BTreeMap<String, Review>, String> {
    let text = std::fs::read_to_string(root.join(LEDGER)).unwrap_or_default();
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let [path, hash, commit, ..] = f[..] else {
            return Err(format!(
                "{LEDGER}: a row needs path, hash, commit: {line:?}"
            ));
        };
        out.insert(
            path.to_string(),
            Review {
                hash: hash.into(),
                commit: commit.into(),
            },
        );
    }
    Ok(out)
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Each page and what to do about it, in `DOC_PATHS`' order.
fn status(root: &Path) -> Result<Vec<(String, String)>, String> {
    let reviews = ledger(root)?;
    let mut out = Vec::new();
    for (page, paths) in DOC_PATHS {
        let state = match reviews.get(*page) {
            None => "never reviewed".to_string(),
            Some(r) => {
                let now = git(root, &["hash-object", page])?;
                if now != r.hash {
                    "page changed since its review".to_string()
                } else if paths.is_empty() {
                    format!("ok (reviewed at {})", short(&r.commit))
                } else {
                    let mut args = vec!["rev-list", "--count"];
                    let range = format!("{}..HEAD", r.commit);
                    args.push(&range);
                    args.push("--");
                    args.extend(paths.iter().copied());
                    let n: usize = git(root, &args)?.parse().unwrap_or(0);
                    if n > 0 {
                        format!("code changed: {n} commits since {}", short(&r.commit))
                    } else {
                        format!("ok (reviewed at {})", short(&r.commit))
                    }
                }
            }
        };
        out.push((page.to_string(), state));
    }
    Ok(out)
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(9)]
}

// ---- check ----------------------------------------------------------------------

fn check(root: &Path) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let pages: Vec<&str> = DOC_PATHS
        .iter()
        .map(|(p, _)| *p)
        .chain(["docs/keys.md"])
        .collect();
    let texts: BTreeMap<&str, String> = pages
        .iter()
        .map(|p| {
            (
                *p,
                std::fs::read_to_string(root.join(p)).unwrap_or_default(),
            )
        })
        .collect();
    // Links and anchors.
    for (page, text) in &texts {
        problems.extend(links(root, page, text, &texts));
    }
    // What the app itself names: `docs/x.md` in its source.
    for file in rust_files(&root.join("crates")) {
        let src = std::fs::read_to_string(&file).unwrap_or_default();
        // Tests name made-up pages (`docs/b.md` as a path in a list).
        let src = src.split("#[cfg(test)]").next().unwrap_or_default();
        for name in doc_names_in(src) {
            if !root.join(&name).is_file() {
                let rel = file
                    .strip_prefix(root)
                    .unwrap_or(&file)
                    .display()
                    .to_string();
                problems.push(format!("{rel}: names {name}, which is not there"));
            }
        }
    }
    // Flags, variables and config keys, against the program and its source.
    let help = xdevcmd(root, &["--help"])?;
    // xtask's help is spread over its modules (`lang pseudo --style` is lang.rs's).
    let xtask_help: String = rust_files(&root.join("xtask"))
        .into_iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();
    let xtask_help = xtask_help.as_str();
    let source: String = rust_files(&root.join("crates"))
        .into_iter()
        .chain(rust_files(&root.join("xtask")))
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();
    let config = xdevcmd(root, &["--print-config"])?;
    let keys = config_keys(&config)?;
    for (page, text) in &texts {
        for (line, flag, of) in flags(text) {
            let known = match of {
                "xdevcmd" => has_flag(&help, flag),
                _ => has_flag(xtask_help, flag),
            };
            if !known {
                problems.push(format!(
                    "{page}:{line}: `{of} {flag}` is not in {of}'s help"
                ));
            }
        }
        for (line, var) in variables(text) {
            if !source.contains(var) {
                problems.push(format!(
                    "{page}:{line}: {var} is read nowhere in the source"
                ));
            }
        }
        for (line, section, key) in named_keys(text) {
            if !keys.contains(&(section.to_string(), key.to_string())) {
                problems.push(format!(
                    "{page}:{line}: [{section}] {key} is not a setting (xdevcmd --print-config)"
                ));
            }
        }
    }
    Ok(problems)
}

fn xdevcmd(root: &Path, args: &[&str]) -> Result<String, String> {
    // A config folder of its own: the developer's config does not change the defaults.
    let empty = std::env::temp_dir().join("xtask-docs-empty-config");
    let out = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["run", "-q", "-p", "xdevcmd", "--"])
        .args(args)
        .env("XDG_CONFIG_HOME", &empty)
        .env("APPDATA", &empty)
        .output()
        .map_err(|e| format!("cargo run xdevcmd: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "xdevcmd {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

/// Lines outside fenced code, with their numbers (from 1).
fn prose(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut fenced = false;
    text.lines().enumerate().filter_map(move |(i, l)| {
        if l.trim_start().starts_with("```") {
            fenced = !fenced;
            return None;
        }
        (!fenced).then_some((i + 1, l))
    })
}

/// GitHub's anchor for a heading: lower case, letters, digits, `-` and `_` kept, spaces
/// to `-`, the rest dropped.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// The anchors a page has: one per heading, `-1`, `-2` … for a repeated one.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = BTreeSet::new();
    for (_, l) in prose(text) {
        let Some(h) = l.strip_prefix('#') else {
            continue;
        };
        let h = h.trim_start_matches('#');
        if !h.starts_with(' ') {
            continue;
        }
        let s = slug(&h.replace('`', ""));
        let n = seen.entry(s.clone()).or_insert(0);
        out.insert(if *n == 0 {
            s.clone()
        } else {
            format!("{s}-{n}")
        });
        *n += 1;
    }
    out
}

/// `[text](target)` links in prose: (line, target).
fn link_targets(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, l) in prose(text) {
        let mut rest = l;
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(end) = after.find(')') else { break };
            out.push((n, after[..end].to_string()));
            rest = &after[end..];
        }
    }
    out
}

fn links(root: &Path, page: &str, text: &str, texts: &BTreeMap<&str, String>) -> Vec<String> {
    let dir = Path::new(page).parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();
    for (line, target) in link_targets(text) {
        if target.contains("://") || target.starts_with("mailto:") || target.is_empty() {
            continue;
        }
        let (file, anchor) = match target.split_once('#') {
            Some((f, a)) => (f, Some(a)),
            None => (target.as_str(), None),
        };
        let path = if file.is_empty() {
            PathBuf::from(page)
        } else {
            normal(&dir.join(file))
        };
        if !root.join(&path).exists() {
            out.push(format!(
                "{page}:{line}: link to {target}: {} is not there",
                path.display()
            ));
            continue;
        }
        if let Some(a) = anchor {
            let key = path.to_string_lossy().replace('\\', "/");
            let body = texts
                .get(key.as_str())
                .cloned()
                .unwrap_or_else(|| std::fs::read_to_string(root.join(&path)).unwrap_or_default());
            if path.extension().is_some_and(|x| x == "md") && !anchors(&body).contains(a) {
                out.push(format!("{page}:{line}: link to {target}: no heading #{a}"));
            }
        }
    }
    out
}

/// `a/../b` as `b`, so a link from `docs/` up to `README.md` reads as the repo's.
fn normal(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// `docs/<name>.md` named in source text.
fn doc_names_in(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = src;
    while let Some(i) = rest.find("docs/") {
        let tail = &rest[i..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.')))
            .unwrap_or(tail.len());
        let name = tail[..end].trim_end_matches('.');
        if name.ends_with(".md") && !name.contains("..") {
            out.insert(name.to_string());
        }
        rest = &rest[i + 5..];
    }
    out
}

/// `xdevcmd --flag` and `xtask … --flag` in prose and code: (line, flag, program). The
/// words after the program, up to the end of a code span or a `|`, `;`, `#`.
fn flags(text: &str) -> Vec<(usize, &str, &'static str)> {
    let mut out = Vec::new();
    for (n, l) in text.lines().enumerate() {
        for (prog, name) in [("xdevcmd ", "xdevcmd"), ("xtask ", "xtask")] {
            let mut rest = l;
            while let Some(i) = rest.find(prog) {
                // The program run, not a word about it: first in a code span or a line
                // (`$ ` allowed), or `cargo xtask`. `cargo install xdevcmd --locked`
                // passes cargo's flag, not xdevcmd's.
                let head = rest[..i].trim_end_matches(' ');
                let after = &rest[i + prog.len()..];
                let end = after.find(['`', '|', ';', '#', ')']).unwrap_or(after.len());
                let run = head.is_empty()
                    || head.ends_with('`')
                    || head.ends_with('$')
                    || (name == "xtask" && head.ends_with("cargo"));
                if run {
                    for w in after[..end].split_whitespace() {
                        let w = w.trim_end_matches([',', '.', ':']);
                        if w.starts_with("--") && w.len() > 2 {
                            let w = w.split('=').next().unwrap_or(w);
                            out.push((n + 1, w, name));
                        }
                    }
                }
                rest = &after[end..];
            }
        }
    }
    out
}

fn has_flag(help: &str, flag: &str) -> bool {
    help.match_indices(flag).any(|(i, _)| {
        let next = help[i + flag.len()..].chars().next();
        next.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-'))
    })
}

/// `XDEVCMD_*` names: (line, name).
fn variables(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (n, l) in text.lines().enumerate() {
        let mut rest = l;
        while let Some(i) = rest.find("XDEVCMD_") {
            let tail = &rest[i..];
            let end = tail
                .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
                .unwrap_or(tail.len());
            let name = tail[..end].trim_end_matches('_');
            // `XDEVCMD_*`, a family, is not one name.
            if name.len() > "XDEVCMD_".len() {
                out.push((n + 1, name));
            }
            rest = &tail[end..];
        }
    }
    out
}

/// `` `[section] key` `` in prose: (line, section, key).
fn named_keys(text: &str) -> Vec<(usize, &str, &str)> {
    let mut out = Vec::new();
    for (n, l) in prose(text) {
        let mut rest = l;
        while let Some(i) = rest.find("`[") {
            let tail = &rest[i + 2..];
            let Some(close) = tail.find("] ") else { break };
            let section = &tail[..close];
            let after = &tail[close + 2..];
            let end = after
                .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
                .unwrap_or(after.len());
            let key = &after[..end];
            if !key.is_empty()
                && section.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && !section.is_empty()
            {
                out.push((n, section, key));
            }
            rest = &tail[close..];
        }
    }
    out
}

/// Every (section, key) `--print-config` writes; `[keys]` holds every action's name.
fn config_keys(config: &str) -> Result<BTreeSet<(String, String)>, String> {
    let table: toml::Table = config.parse().map_err(|e| format!("--print-config: {e}"))?;
    let mut out = BTreeSet::new();
    for (section, v) in &table {
        if let Some(t) = v.as_table() {
            for key in t.keys() {
                out.insert((section.clone(), key.clone()));
            }
        }
    }
    Ok(out)
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
    fn every_page_and_path_in_doc_paths_is_there() {
        for (page, paths) in DOC_PATHS {
            assert!(root().join(page).is_file(), "{page}");
            for p in *paths {
                assert!(root().join(p).exists(), "{page}: {p}");
            }
        }
        // And every page under docs/ is in scope, or named as out of it.
        for e in std::fs::read_dir(root().join("docs")).unwrap().flatten() {
            let name = format!("docs/{}", e.file_name().to_string_lossy());
            if name.ends_with(".md") && name != "docs/keys.md" {
                assert!(
                    DOC_PATHS.iter().any(|(p, _)| *p == name),
                    "{name} is not in DOC_PATHS"
                );
            }
        }
    }

    #[test]
    fn slugs_are_githubs() {
        assert_eq!(slug("Where the file is"), "where-the-file-is");
        assert_eq!(slug("Not yet (will change)"), "not-yet-will-change");
        assert_eq!(slug("Copy and move (F5, F6)"), "copy-and-move-f5-f6");
        assert_eq!(slug("`[keys]` and F1"), "keys-and-f1");
        let a = anchors("# A\n## B\n```\n# not a heading\n```\n## B\n");
        assert_eq!(a, ["a", "b", "b-1"].into_iter().map(String::from).collect());
    }

    #[test]
    fn what_the_check_reads_from_a_page() {
        let page = "Run `xdevcmd --print-keys`, then `xtask lang pseudo --style leet`.\n\
                    `xdevcmd --nope` | grep x\n\
                    Set `[general] language`, `[files] nothing`. XDEVCMD_STATE, XDEVCMD_*.\n\
                    `cargo install xdevcmd --locked` and git --oneline are not ours.\n\
                    cargo xtask docs check --all\n";
        let f: Vec<_> = flags(page).into_iter().map(|(_, f, p)| (f, p)).collect();
        assert_eq!(
            f,
            [
                ("--print-keys", "xdevcmd"),
                ("--style", "xtask"),
                ("--nope", "xdevcmd"),
                ("--all", "xtask")
            ]
        );
        let v: Vec<_> = variables(page).into_iter().map(|(_, v)| v).collect();
        assert_eq!(v, ["XDEVCMD_STATE"]);
        let k: Vec<_> = named_keys(page)
            .into_iter()
            .map(|(_, s, k)| (s, k))
            .collect();
        assert_eq!(k, [("general", "language"), ("files", "nothing")]);
        assert!(has_flag("  --keys=F8  --keyset", "--keys"));
        assert!(!has_flag("  --keyset", "--keys"));
    }

    #[test]
    fn every_link_and_anchor_in_the_docs_is_there() {
        let root = root();
        let texts: BTreeMap<&str, String> = DOC_PATHS
            .iter()
            .map(|(p, _)| (*p, std::fs::read_to_string(root.join(p)).unwrap()))
            .collect();
        let bad: Vec<String> = texts
            .iter()
            .flat_map(|(page, text)| links(&root, page, text, &texts))
            .collect();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn the_ledger_reads_its_rows() {
        let r = ledger(&root()).unwrap();
        for page in r.keys() {
            assert!(
                DOC_PATHS.iter().any(|(p, _)| p == page),
                "{LEDGER}: {page} is not in scope"
            );
        }
    }
}
