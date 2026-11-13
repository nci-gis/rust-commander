//! Word wrap for dialog text and F1: at spaces, by display width, keeping each
//! character's style. A continuation line starts `hang` columns in, under the column the
//! text it continues began at. A word wider than the room is cut where the room ends
//! (a path with no spaces is better shortened by the caller, which knows it is one).

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

type Cells = Vec<(char, Style)>;

/// `line` as lines at most `width` columns wide. A line that fits comes back as it was.
pub fn line(line: &Line<'static>, width: usize, hang: usize) -> Vec<Line<'static>> {
    if line.width() <= width || width == 0 {
        return vec![line.clone()];
    }
    let cells: Cells = line
        .spans
        .iter()
        .flat_map(|s| {
            let style = line.style.patch(s.style);
            s.content.chars().map(move |c| (c, style))
        })
        .collect();
    split(&cells, width, hang)
        .into_iter()
        .map(|row| {
            let mut l = Line::from(spans(&row));
            l.style = line.style;
            l
        })
        .collect()
}

/// `text` as lines at most `width` columns wide, continuation lines `hang` columns in.
pub fn plain(text: &str, width: usize, hang: usize) -> Vec<String> {
    if text.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>() <= width || width == 0 {
        return vec![text.to_string()];
    }
    let cells: Cells = text.chars().map(|c| (c, Style::new())).collect();
    split(&cells, width, hang)
        .into_iter()
        .map(|row| row.into_iter().map(|(c, _)| c).collect())
        .collect()
}

/// The fewest columns a continuation line keeps for its text.
const MIN_ROOM: usize = 20;

fn w(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// The greedy fill: words (runs of non-spaces) are placed whole while they fit; the
/// spaces between two words stay on the line before a break and are dropped after it.
fn split(cells: &[(char, Style)], width: usize, hang: usize) -> Vec<Cells> {
    // A hang that leaves too little room for the text is no help: start at the left.
    let hang = if width.saturating_sub(hang) < MIN_ROOM {
        0
    } else {
        hang
    };
    let mut rows: Vec<Cells> = Vec::new();
    let mut row: Cells = Vec::new();
    let mut used = 0;
    let mut i = 0;
    while i < cells.len() {
        let space = cells[i].0 == ' ';
        let mut j = i;
        while j < cells.len() && (cells[j].0 == ' ') == space {
            j += 1;
        }
        let token = &cells[i..j];
        let tw: usize = token.iter().map(|(c, _)| w(*c)).sum();
        let fresh = !rows.is_empty() && used == hang;
        if space {
            // Leading spaces of the first line are the line's own indent: kept.
            if !fresh {
                row.extend_from_slice(token);
                used += tw;
            }
        } else if used + tw <= width {
            row.extend_from_slice(token);
            used += tw;
        } else {
            // Does not fit: start a new line, unless this one has nothing yet, or the
            // word would not fit a fresh line either (then it is cut from here on).
            let fits_fresh = tw <= width - hang;
            if fits_fresh && used > if rows.is_empty() { 0 } else { hang } {
                trim_end(&mut row);
                rows.push(std::mem::take(&mut row));
                row.extend(std::iter::repeat_n((' ', Style::new()), hang));
                used = hang;
            }
            // A word wider than a whole line: cut it where the room ends.
            for &(c, s) in token {
                if used + w(c) > width && used > hang {
                    rows.push(std::mem::take(&mut row));
                    row.extend(std::iter::repeat_n((' ', Style::new()), hang));
                    used = hang;
                }
                row.push((c, s));
                used += w(c);
            }
        }
        i = j;
    }
    trim_end(&mut row);
    rows.push(row);
    rows
}

fn trim_end(row: &mut Cells) {
    while row.last().is_some_and(|(c, _)| *c == ' ') {
        row.pop();
    }
}

/// Runs of one style as spans.
fn spans(row: &[(char, Style)]) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    for &(c, s) in row {
        match out.last_mut() {
            Some(last) if last.style == s => last.content.to_mut().push(c),
            _ => out.push(Span::styled(c.to_string(), s)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line<'static>]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_line_that_fits_is_left_alone() {
        assert_eq!(plain("short line", 20, 2), ["short line"]);
        let l = Line::from("  exactly twenty col");
        assert_eq!(text(&line(&l, 20, 2)), ["  exactly twenty col"]);
    }

    #[test]
    fn words_wrap_at_spaces_under_the_hang() {
        let s = "xDev Commander: a cross-platform, two-pane terminal file manager that extends your shell.";
        let got = plain(s, 40, 0);
        assert_eq!(
            got,
            [
                "xDev Commander: a cross-platform,",
                "two-pane terminal file manager that",
                "extends your shell."
            ]
        );
        assert!(got.iter().all(|l| l.chars().count() <= 40));
        let got = plain("  one two three four five six seven eight nine", 24, 2);
        assert_eq!(
            got,
            ["  one two three four", "  five six seven eight", "  nine"]
        );
    }

    #[test]
    fn styles_follow_their_characters_across_a_break() {
        let bold = Style::new().add_modifier(ratatui::style::Modifier::BOLD);
        let l = Line::from(vec![
            Span::styled("KEY      ", bold),
            Span::raw("a description that is far too long"),
        ]);
        let got = line(&l, 30, 9);
        assert_eq!(
            text(&got),
            ["KEY      a description that is", "         far too long"]
        );
        assert_eq!(got[0].spans[0].content, "KEY      ");
        assert_eq!(got[0].spans[0].style, bold);
        assert!(got[1].spans.iter().all(|s| s.style != bold));
    }

    #[test]
    fn a_word_wider_than_the_line_is_cut_and_wide_characters_count_twice() {
        // Too long for any line: cut from where it starts, no line left half empty.
        assert_eq!(
            plain("go /a/very/long/path/name now", 10, 0),
            ["go /a/very", "/long/path", "/name now"]
        );
        // Each of these takes two columns: four fit in eight.
        assert_eq!(plain("日本語の 文字", 8, 0), ["日本語の", "文字"]);
    }

    #[test]
    fn a_hang_too_deep_for_the_room_is_dropped() {
        assert_eq!(plain("aaaa bbbb cccc", 9, 6), ["aaaa bbbb", "cccc"]);
        // F1 at 80 columns: the description starts at 51, which still leaves 27.
        let row = format!(
            "{:51}{}",
            "    Q", "Quit, leaving the shell where it was (with --cwd-file)"
        );
        let got = plain(&row, 78, 51);
        assert!(
            got.len() > 1 && got[1..].iter().all(|l| l.starts_with(&" ".repeat(51))),
            "{got:?}"
        );
    }

    #[test]
    fn a_word_too_long_for_any_line_starts_where_it_is() {
        let url = crate::docs::docs_url(Some("troubleshooting.md"));
        let got = plain(&format!("    More      {url}"), 50, 14);
        assert!(got[0].starts_with("    More      https://"), "{got:?}");
        assert_eq!(
            got.concat().replace(' ', "").len(),
            format!("More{url}").len()
        );
    }
}
