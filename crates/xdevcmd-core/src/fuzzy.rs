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

/// The score of `query` in `name`, higher is better; `None` when it does not match.
pub fn score(query: &str, name: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let exact_case = q.iter().any(|c| c.is_uppercase());
    let fold = |c: char| {
        if exact_case {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let n: Vec<char> = name.chars().collect();
    let q: Vec<char> = q.into_iter().map(fold).collect();
    // Leftmost match, forward.
    let mut qi = 0;
    let mut end = None;
    for (i, &c) in n.iter().enumerate() {
        if fold(c) == q[qi] {
            qi += 1;
            if qi == q.len() {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    // Tighten: backwards from the end, the latest start that still matches.
    let mut qi = q.len();
    let mut start = end;
    for i in (0..=end).rev() {
        if fold(n[i]) == q[qi - 1] {
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
    let mut prev_match: Option<usize> = None;
    for (i, &c) in n.iter().enumerate().take(end + 1).skip(start) {
        if qi < q.len() && fold(c) == q[qi] {
            let mut pts = MATCH;
            let boundary = i == 0
                || matches!(n[i - 1], '_' | '-' | '.' | ' ' | '/' | '\\')
                || (n[i - 1].is_lowercase() && c.is_uppercase())
                || (!n[i - 1].is_ascii_digit() && c.is_ascii_digit());
            if boundary {
                pts += BOUNDARY;
            }
            match prev_match {
                Some(p) if p + 1 == i => pts += CONSECUTIVE,
                Some(p) => s += GAP_START + GAP_MORE * (i - p - 2) as i32,
                None => {}
            }
            if qi == 0 {
                pts += if boundary { FIRST } else { 0 };
            }
            s += pts;
            prev_match = Some(i);
            qi += 1;
        }
    }
    Some(s)
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
}
