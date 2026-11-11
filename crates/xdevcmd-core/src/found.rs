//! Paths in a line of a shell's output: what might name a file, and where.
//!
//! [`paths_in`] only finds candidates in the text; it never touches the disk. The caller
//! resolves each against a directory and keeps the ones that exist, which is what keeps
//! a word in prose that happens to look like a name from being offered as a file.
//!
//! The forms (each a row of the table test below):
//! - `path:line[:col]` and `path:text`: grep, rustc's `--> src/main.rs:3:5`, gcc, eslint,
//!   Node's `at f (/x.js:3:7)`;
//! - `File "x.py", line 12`: Python;
//! - `x.cs(12,5)` and `x.cs(12)`: MSBuild and the C# compiler;
//! - `C:\…`, `.\x`, `..\x`: Windows;
//! - `~/…`, absolute and relative paths, bare names as `ls` prints them, and `'a b'`
//!   (GNU `ls` quoting a name with a space).
//!
//! Refused: URLs (`scheme://`), version numbers (`1.2.3`), times (`12:30:45`), anything
//! without a letter in it, and anything with a control character.

use std::ops::Range;

/// A candidate path in a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Where it is in the line, in bytes: the path, and its `:line:col` when attached.
    pub span: Range<usize>,
    /// The path as written.
    pub path: String,
    /// The line it points at, counted from 1.
    pub line: Option<u32>,
    pub col: Option<u32>,
}

/// Characters that end a path in running text. Quotes and brackets are how output
/// wraps a path; `=` is how a flag joins one (`--manifest-path=Cargo.toml`).
fn delimits(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\''
                | '`'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '|'
                | ','
                | ';'
                | '='
        )
}

/// Every candidate path in `line`, left to right, none overlapping.
pub fn paths_in(line: &str) -> Vec<Found> {
    let mut out = Vec::new();
    if line.chars().any(|c| c.is_control() && c != '\t') {
        return out;
    }
    // Quoted names first: `'a b'` and `"x y.py"` may hold spaces.
    let mut taken: Vec<Range<usize>> = Vec::new();
    for (open, close) in [('\'', '\''), ('"', '"')] {
        let mut from = 0;
        while let Some(rel) = line[from..].find(open) {
            let start = from + rel + open.len_utf8();
            let Some(len) = line[start..].find(close) else {
                break;
            };
            let end = start + len;
            let inner = &line[start..end];
            if inner.contains(' ') && !taken.iter().any(|r| r.start < end && start < r.end) {
                if let Some(mut f) = candidate(inner, start) {
                    locate_after(line, end + close.len_utf8(), &mut f);
                    taken.push(start..end);
                    out.push(f);
                }
            }
            from = end + close.len_utf8();
        }
    }
    // Then words.
    let mut i = 0;
    while i < line.len() {
        let c = line[i..].chars().next().unwrap_or(' ');
        if delimits(c) {
            i += c.len_utf8();
            continue;
        }
        let end = line[i..]
            .char_indices()
            .find(|&(_, c)| delimits(c))
            .map_or(line.len(), |(n, _)| i + n);
        if !taken.iter().any(|r| r.start < end && i < r.end) {
            if let Some(mut f) = candidate(&line[i..end], i) {
                locate_after(line, end, &mut f);
                out.push(f);
            }
        }
        i = end;
    }
    out.sort_by_key(|f| f.span.start);
    out
}

/// A line number that follows the path rather than being joined to it: Python's
/// `", line 12` and MSBuild's `(12,5)`.
fn locate_after(line: &str, at: usize, f: &mut Found) {
    if f.line.is_some() {
        return;
    }
    let rest = &line[at.min(line.len())..];
    // Python's name is quoted: the line follows the closing quote.
    let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
    if let Some(r) = rest.strip_prefix(", line ") {
        if let Some(n) = digits(r) {
            f.line = Some(n);
        }
    } else if let Some(r) = rest.strip_prefix('(') {
        let Some(close) = r.find(')') else { return };
        let inside = &r[..close];
        let mut parts = inside.split(',');
        let line_no = parts.next().and_then(whole);
        let col = parts.next().and_then(whole);
        if let (Some(n), true) = (line_no, parts.next().is_none()) {
            f.line = Some(n);
            f.col = col;
            f.span.end = at + 1 + close + 1;
        }
    }
}

/// The number at the start of `s`.
fn digits(s: &str) -> Option<u32> {
    let n: String = s.chars().take_while(char::is_ascii_digit).collect();
    n.parse().ok().filter(|&n| n > 0)
}

/// `s` if it is all a number.
fn whole(s: &str) -> Option<u32> {
    let s = s.trim();
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
        .filter(|&n| n > 0)
}

/// A word (or a quoted name) at byte `at` of its line, as a candidate, or `None`.
fn candidate(word: &str, at: usize) -> Option<Found> {
    if word.contains("://") {
        return None;
    }
    let b = word.as_bytes();
    // `C:\…` or `C:/…`: that colon is the drive's.
    let drive = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    let from = if drive { 2 } else { 0 };
    let (mut path, mut line, mut col, mut used) = (word, None, None, word.len());
    if let Some(rel) = word[from..].find(':') {
        let colon = from + rel;
        path = &word[..colon];
        used = colon;
        let rest = &word[colon + 1..];
        let n = rest.chars().take_while(char::is_ascii_digit).count();
        if n > 0 {
            line = rest[..n].parse().ok().filter(|&v: &u32| v > 0);
            used = colon + 1 + n;
            if let Some(a) = rest[n..].strip_prefix(':') {
                let m = a.chars().take_while(char::is_ascii_digit).count();
                if m > 0 {
                    col = a[..m].parse().ok().filter(|&v: &u32| v > 0);
                    used += 1 + m;
                }
            }
        }
    }
    // A sentence's full stop or a list's comma is not part of the name.
    if line.is_none() {
        let t = path.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if !t.is_empty() {
            path = t;
            used = path.len();
        }
    }
    if path.is_empty() || path == "." || path == ".." || !path.chars().any(char::is_alphabetic) {
        return None;
    }
    // A version (`1.2.3`, `v0.4.0`) is not a file.
    let core = path.strip_prefix('v').unwrap_or(path);
    if core.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some(Found {
        span: at..at + used,
        path: path.to_string(),
        line,
        col,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path found: its text, line and column.
    type Got = (String, Option<u32>, Option<u32>);

    /// The paths found.
    fn found(line: &str) -> Vec<Got> {
        paths_in(line)
            .into_iter()
            .map(|f| (f.path, f.line, f.col))
            .collect()
    }

    #[test]
    fn each_form_gives_its_path_and_line() {
        let p = |s: &str| s.to_string();
        let rows: &[(&str, Got)] = &[
            ("src/main.rs:42:9", (p("src/main.rs"), Some(42), Some(9))),
            (
                "  --> src/net/peer.rs:42:9",
                (p("src/net/peer.rs"), Some(42), Some(9)),
            ),
            ("README.md:12:## Usage", (p("README.md"), Some(12), None)),
            (
                "main.c:7: error: expected ';'",
                (p("main.c"), Some(7), None),
            ),
            ("src/lib.rs:fn main() {", (p("src/lib.rs"), None, None)),
            (
                "    at run (/srv/app/x.js:3:7)",
                (p("/srv/app/x.js"), Some(3), Some(7)),
            ),
            (
                "  File \"/tmp/x y.py\", line 12, in f",
                (p("/tmp/x y.py"), Some(12), None),
            ),
            (
                "Program.cs(12,5): error CS1002",
                (p("Program.cs"), Some(12), Some(5)),
            ),
            ("Program.cs(12): warning", (p("Program.cs"), Some(12), None)),
            (
                "C:\\Users\\me\\x.rs:12:3",
                (p("C:\\Users\\me\\x.rs"), Some(12), Some(3)),
            ),
            (".\\build\\out.log", (p(".\\build\\out.log"), None, None)),
            ("..\\x", (p("..\\x"), None, None)),
            ("~/notes/todo.md", (p("~/notes/todo.md"), None, None)),
            ("/etc/hosts", (p("/etc/hosts"), None, None)),
            ("see docs/shell.md.", (p("docs/shell.md"), None, None)),
            ("'my file.txt'", (p("my file.txt"), None, None)),
            ("--manifest-path=Cargo.toml", (p("Cargo.toml"), None, None)),
            ("設計書_v2.md", (p("設計書_v2.md"), None, None)),
        ];
        for (line, want) in rows {
            let got = found(line).into_iter().find(|f| f.0 == want.0);
            assert_eq!(got.as_ref(), Some(want), "{line:?}: {:?}", found(line));
        }
    }

    #[test]
    fn what_is_not_a_path_is_not_offered() {
        for line in [
            "https://example.com/a.html",
            "1.2.3",
            "v0.4.0",
            "12:30:45",
            "42",
            "...",
            "a\u{1b}[31mred",
        ] {
            assert_eq!(found(line), vec![], "{line:?}");
        }
    }

    #[test]
    fn a_listing_gives_each_name() {
        let names: Vec<String> = found("Cargo.toml  README.md  src  'a b.txt'")
            .into_iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(names, ["Cargo.toml", "README.md", "src", "a b.txt"]);
    }

    #[test]
    fn the_span_covers_the_path_and_its_place() {
        let line = "error at src/a.rs:3:4 here";
        let f = paths_in(line)
            .into_iter()
            .find(|f| f.path == "src/a.rs")
            .unwrap();
        assert_eq!(&line[f.span.clone()], "src/a.rs:3:4");
        let line = "x Program.cs(12,5) y";
        let f = paths_in(line)
            .into_iter()
            .find(|f| f.path == "Program.cs")
            .unwrap();
        assert_eq!(&line[f.span], "Program.cs(12,5)");
        let line = "  File \"/tmp/x y.py\", line 12";
        let f = paths_in(line)
            .into_iter()
            .find(|f| f.path == "/tmp/x y.py")
            .unwrap();
        assert_eq!(&line[f.span], "/tmp/x y.py");
    }
}
