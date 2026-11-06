//! Name patterns, as typed into "mark by pattern" and the quick filter.
//!
//! `*` is any run of characters, `?` is one character, and nothing else is special —
//! no brackets, no braces, no escapes. Several patterns may be given at once,
//! separated by spaces, commas or semicolons: `*.rs *.toml` matches either. A pattern
//! ending in `/` matches directories only. Matching ignores case on every platform:
//! nobody typing `*.jpg` means to skip `HOLIDAY.JPG`.

#[derive(Debug, Clone, PartialEq, Eq)]
struct One {
    chars: Vec<char>,
    dirs_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    alts: Vec<One>,
}

impl Pattern {
    /// Parse what was typed. Empty input gives a pattern that matches nothing.
    pub fn new(text: &str) -> Self {
        let alts = text
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (body, dirs_only) = match s.strip_suffix('/') {
                    Some(b) => (if b.is_empty() { "*" } else { b }, true),
                    None => (s, false),
                };
                One {
                    chars: fold(body),
                    dirs_only,
                }
            })
            .collect();
        Self { alts }
    }

    pub fn is_empty(&self) -> bool {
        self.alts.is_empty()
    }

    pub fn matches(&self, name: &str, is_dir: bool) -> bool {
        let name = fold(name);
        self.alts
            .iter()
            .any(|a| (!a.dirs_only || is_dir) && wildcard(&a.chars, &name))
    }
}

/// Whether `text` looks like a pattern rather than plain text to search for.
pub fn has_wildcards(text: &str) -> bool {
    text.contains(['*', '?'])
}

fn fold(s: &str) -> Vec<char> {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// Classic two-pointer wildcard match: linear in practice, and no recursion for a
/// pattern like `*a*a*a*a*b` to blow up on.
fn wildcard(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, si));
            pi += 1;
        } else if let Some((sp, ss)) = star {
            pi = sp + 1;
            si = ss + 1;
            star = Some((sp, ss + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, name: &str) -> bool {
        Pattern::new(p).matches(name, false)
    }

    #[test]
    fn star_and_question_mark() {
        assert!(m("*.rs", "main.rs"));
        assert!(!m("*.rs", "main.rsx"));
        assert!(m("a?c", "abc"));
        assert!(!m("a?c", "ac"));
        assert!(m("*", ""));
        assert!(m("*", "anything"));
        assert!(m("**x", "x"));
        assert!(!m("", "x"), "an empty pattern marks nothing");
    }

    #[test]
    fn several_patterns_at_once() {
        for sep in [" ", ",", ";", " , "] {
            let p = format!("*.rs{sep}*.toml");
            assert!(m(&p, "Cargo.toml"), "{p:?}");
            assert!(m(&p, "lib.rs"), "{p:?}");
            assert!(!m(&p, "README.md"), "{p:?}");
        }
    }

    #[test]
    fn case_is_ignored_including_beyond_ascii() {
        assert!(m("*.jpg", "HOLIDAY.JPG"));
        assert!(m("báo*", "BÁO CÁO.docx"));
    }

    #[test]
    fn a_trailing_slash_means_directories_only() {
        let p = Pattern::new("src/");
        assert!(p.matches("src", true));
        assert!(!p.matches("src", false));
        let any_dir = Pattern::new("/");
        assert!(any_dir.matches("whatever", true));
        assert!(!any_dir.matches("whatever", false));
    }

    #[test]
    fn a_pathological_pattern_is_still_fast() {
        let name = "a".repeat(5000);
        let t = std::time::Instant::now();
        assert!(!m("*a*a*a*a*a*a*a*a*b", &name));
        assert!(t.elapsed().as_millis() < 500, "{:?}", t.elapsed());
    }
}
