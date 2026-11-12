//! Fuzzy name matching for the quick filter: the query's characters appear in the name
//! in order, not necessarily side by side ("dcfg" finds "docs/config.md"'s "config"
//! and "dev-config.toml").
//!
//! The score follows fzf's ideas, in a simpler form: points for each character
//! matched, more where a match starts a word (after `_ - . space`, at a capital in
//! camelCase, or at the start), more for runs of consecutive matches, fewer for gaps.
//! The leftmost match is found, then tightened from its end backwards, which finds the
//! shortest window ending there — what fzf's first algorithm does. Case is ignored
//! unless the query has a capital (smart case).

const MATCH: i32 = 16;
const BOUNDARY: i32 = 8;
const CONSECUTIVE: i32 = 4;
const FIRST: i32 = 8;
const GAP_START: i32 = -3;
const GAP_MORE: i32 = -1;

fn fold_with(exact_case: bool, c: char) -> char {
    if exact_case {
        c
    } else if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

/// Whether the character at a match starts a word: the start, after `_ - . space /`,
/// a capital in camelCase, or a digit after a non-digit.
fn starts_word(prev: Option<char>, c: char) -> bool {
    match prev {
        None => true,
        Some(p) => {
            matches!(p, '_' | '-' | '.' | ' ' | '/' | '\\')
                || (p.is_lowercase() && c.is_uppercase())
                || (!p.is_ascii_digit() && c.is_ascii_digit())
        }
    }
}

/// A query prepared once, to be matched against many names: F1 ranks every command on
/// each key typed.
pub struct Query {
    q: Vec<char>,
    exact_case: bool,
}

impl Query {
    pub fn new(query: &str) -> Self {
        let exact_case = query.chars().any(char::is_uppercase);
        let q = query
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| fold_with(exact_case, c))
            .collect();
        Self { q, exact_case }
    }

    /// The match in `name`: its score and the characters matched (indices in
    /// characters, not bytes). The leftmost match is found, tightened from its end
    /// backwards, then matched greedily from its start. Nothing is allocated for a name
    /// that does not match.
    pub fn score_positions(&self, name: &str) -> Option<(i32, Vec<usize>)> {
        let q = &self.q;
        let exact_case = self.exact_case;
        let fold = |c: char| fold_with(exact_case, c);
        if q.is_empty() {
            return Some((0, Vec::new()));
        }
        // Leftmost match, forward.
        let mut qi = 0;
        let mut end = None;
        for (i, c) in name.chars().enumerate() {
            if fold(c) == q[qi] {
                qi += 1;
                if qi == q.len() {
                    end = Some(i);
                    break;
                }
            }
        }
        let end = end?;
        let count = name.chars().count();
        // Tighten: backwards from the end, the latest start that still matches.
        let mut qi = q.len();
        let mut start = end;
        for (back, c) in name.chars().rev().enumerate().skip(count - 1 - end) {
            let i = count - 1 - back;
            if fold(c) == q[qi - 1] {
                qi -= 1;
                if qi == 0 {
                    start = i;
                    break;
                }
            }
        }
        // Score the window, matching greedily from its start.
        let mut s = 0;
        let mut qi = 0;
        let mut hits = Vec::with_capacity(q.len());
        let mut prev: Option<char> = None;
        for (i, c) in name.chars().enumerate().take(end + 1) {
            if i >= start && qi < q.len() && fold(c) == q[qi] {
                let mut pts = MATCH;
                let boundary = starts_word(prev, c);
                if boundary {
                    pts += BOUNDARY;
                }
                match hits.last() {
                    Some(&p) if p + 1 == i => pts += CONSECUTIVE,
                    Some(&p) => s += GAP_START + GAP_MORE * (i - p - 2) as i32,
                    None => {}
                }
                if qi == 0 && boundary {
                    pts += FIRST;
                }
                s += pts;
                hits.push(i);
                qi += 1;
            }
            prev = Some(c);
        }
        Some((s, hits))
    }
}

/// [`Query::score_positions`] for one name.
pub fn score_positions(query: &str, name: &str) -> Option<(i32, Vec<usize>)> {
    Query::new(query).score_positions(name)
}

/// The score of `query` in `name`, higher is better; `None` when it does not match.
pub fn score(query: &str, name: &str) -> Option<i32> {
    score_positions(query, name).map(|(s, _)| s)
}

/// Which characters of `name` `query` matched, as character indices, for drawing them
/// highlighted: the same ones [`score`] scored. `None` when it does not match.
pub fn positions(query: &str, name: &str) -> Option<Vec<usize>> {
    score_positions(query, name).map(|(_, p)| p)
}

/// Whether a match reads as words typed, not letters scattered: each matched character
/// starts a word or follows the one matched before it ("sbn" in "Sort by name", "cop"
/// in "Copy"), and the first starts a word. F1 ranks these above a description's match.
pub fn is_words(name: &str, hits: &[usize]) -> bool {
    let mut k = 0;
    let mut prev = None;
    for (i, c) in name.chars().enumerate() {
        if k == hits.len() {
            break;
        }
        if hits[k] == i {
            let run = k > 0 && hits[k - 1] + 1 == i;
            if !run && !starts_word(prev, c) {
                return false;
            }
            k += 1;
        }
        prev = Some(c);
    }
    k == hits.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_in_order_match_and_out_of_order_do_not() {
        assert!(score("cfg", "config.toml").is_some());
        assert!(score("ctml", "config.toml").is_some());
        assert!(score("gfc", "config.toml").is_none());
        assert!(score("xyz", "config.toml").is_none());
        assert_eq!(score("", "anything"), Some(0));
    }

    #[test]
    fn case_is_ignored_unless_the_query_has_a_capital() {
        assert!(score("readme", "README.md").is_some());
        assert!(score("README", "readme.md").is_none());
        assert!(score("Read", "ReadMe").is_some());
    }

    #[test]
    fn a_word_start_and_a_run_score_above_scattered_letters() {
        let tight = score("cfg", "cfg.rs").unwrap();
        let words = score("cfg", "client_file_graph").unwrap();
        let scattered = score("cfg", "ancient-fig").unwrap();
        assert!(tight > scattered, "{tight} {scattered}");
        assert!(words > scattered, "{words} {scattered}");
        // A substring stays at the top: it is a run.
        assert!(score("main", "main.rs").unwrap() > score("main", "my_animation.rs").unwrap());
    }

    #[test]
    fn spaces_in_the_query_are_ignored_and_unicode_works() {
        assert!(score("my doc", "my_document.txt").is_some());
        assert!(score("tài", "Tài liệu.docx").is_some());
    }

    #[test]
    fn positions_are_the_characters_scored() {
        assert_eq!(positions("cfg", "config"), Some(vec![0, 3, 5]));
        // Counted in characters, not bytes.
        assert_eq!(positions("lieu", "Tài liệu"), None);
        assert_eq!(positions("liệu", "Tài liệu"), Some(vec![4, 5, 6, 7]));
        assert_eq!(positions("main", "my_animation"), Some(vec![6, 7, 9, 11]));
        assert_eq!(positions("", "x"), Some(vec![]));
        assert_eq!(positions("zz", "x"), None);
    }

    #[test]
    fn words_typed_are_told_from_scattered_letters() {
        let w = |q: &str, n: &str| is_words(n, &positions(q, n).unwrap());
        assert!(w("sbn", "Sort by name"));
        assert!(w("cop", "Files: Copy"));
        assert!(w("files", "Files: Copy"));
        assert!(!w("trash", "Terminal: Show or hide terminal"));
        assert!(!w("opy", "Files: Copy"), "starts mid-word");
    }
}
