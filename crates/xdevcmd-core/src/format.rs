use std::time::SystemTime;

use chrono::{DateTime, Local};

/// Render a byte count the way a file list wants it: narrow, and never surprising.
///
/// Under 1 KiB the exact byte count is shown, because in a listing of config files the
/// difference between 0 and 12 bytes matters and "0.0 K" hides it.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["", "K", "M", "G", "T", "P"];
    if bytes < 1024 {
        return bytes.to_string();
    }
    let mut v = bytes as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 {
        format!("{v:.1} {}", UNITS[i])
    } else {
        format!("{v:.0} {}", UNITS[i])
    }
}

/// `2026-11-05 21:08` in the machine's local timezone, or `-` when the filesystem did not
/// give us a timestamp (which happens on some network mounts and for the `..` row).
pub fn format_time(t: Option<SystemTime>) -> String {
    match t {
        Some(t) => DateTime::<Local>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        None => "-".to_string(),
    }
}

/// `21:08:05` in the machine's local timezone: the time of a status message.
pub fn format_clock(t: SystemTime) -> String {
    DateTime::<Local>::from(t).format("%H:%M:%S").to_string()
}

/// Fit a string into `width` display columns, putting an ellipsis in the middle so both
/// the start of the name and its extension stay readable. Columns as the terminal draws
/// them: a CJK character takes two, a combining accent none.
pub fn elide_middle(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    match elided(s, width) {
        None => s.to_string(),
        Some((head, tail)) => {
            let n = s.chars().count();
            let mut out: String = s.chars().take(head).collect();
            out.push('\u{2026}');
            out.extend(s.chars().skip(n - tail));
            out
        }
    }
}

/// How [`elide_middle`] cuts `s` to `width` columns: `None` when it fits as it is, or how
/// many characters it keeps from the start and from the end, around the ellipsis. The
/// start gets the odd column; columns a wide character could not use go to the end.
pub fn elided(s: &str, width: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = s.chars().collect();
    if chars.iter().map(|&c| cells(c)).sum::<usize>() <= width {
        return None;
    }
    let keep = width.saturating_sub(1);
    let mut head = 0;
    let mut used = 0;
    while head < chars.len() && used + cells(chars[head]) <= keep.div_ceil(2) {
        used += cells(chars[head]);
        head += 1;
    }
    let room = keep - used;
    let mut tail = 0;
    let mut tail_used = 0;
    while tail < chars.len() - head && tail_used + cells(chars[chars.len() - 1 - tail]) <= room {
        tail_used += cells(chars[chars.len() - 1 - tail]);
        tail += 1;
    }
    // An accent whose letter did not fit is not shown on its own.
    while tail > 0 && cells(chars[chars.len() - tail]) == 0 {
        tail -= 1;
    }
    Some((head, tail))
}

/// `s` in exactly `width` columns, from the left: elided in the middle if longer, padded
/// with spaces if shorter. `format!("{:<w$}")` counts characters, which a CJK name
/// outgrows.
pub fn fit_start(s: &str, width: usize) -> String {
    let mut out = elide_middle(s, width);
    let w = text_cells(&out);
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(w)));
    out
}

/// As [`fit_start`], to the right: the padding goes first.
pub fn fit_end(s: &str, width: usize) -> String {
    let text = elide_middle(s, width);
    let w = text_cells(&text);
    let mut out: String = std::iter::repeat_n(' ', width.saturating_sub(w)).collect();
    out.push_str(&text);
    out
}

/// Columns a string takes on the terminal.
pub fn text_cells(s: &str) -> usize {
    s.chars().map(cells).sum()
}

fn cells(c: char) -> usize {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Make one line of a file safe to draw: tabs become spaces on a fixed grid, and other
/// control characters become a visible dot instead of moving the cursor around.
pub fn display_line(s: &str, tab: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    for c in s.chars() {
        match c {
            '\t' => {
                let n = tab - (col % tab);
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push('\u{b7}');
                col += 1;
            }
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sizes_stay_exact() {
        assert_eq!(human_size(0), "0");
        assert_eq!(human_size(12), "12");
        assert_eq!(human_size(1023), "1023");
    }

    #[test]
    fn larger_sizes_get_a_unit() {
        assert_eq!(human_size(1024), "1.0 K");
        assert_eq!(human_size(1536), "1.5 K");
        assert_eq!(human_size(10 * 1024), "10 K");
        assert_eq!(human_size(1024 * 1024), "1.0 M");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 G");
    }

    #[test]
    fn elide_keeps_both_ends() {
        assert_eq!(elide_middle("short", 10), "short");
        assert_eq!(elide_middle("abcdefghij", 10), "abcdefghij");
        // Balanced split, head takes the odd column: "ab" + ellipsis + "jk".
        assert_eq!(elide_middle("abcdefghijk", 5), "ab…jk");
        assert_eq!(elide_middle("report-2026-final.pdf", 12), "report…l.pdf");
        assert_eq!(elide_middle("abcdefghijk", 1), "…");
        assert_eq!(elide_middle("abcdefghijk", 0), "");
    }

    #[test]
    fn wide_characters_take_two_columns() {
        // Each of these takes two columns: ten characters are twenty columns.
        let name = "活用品質向上段階提案";
        assert_eq!(text_cells(name), 20);
        assert_eq!(elide_middle(name, 20), name);
        let cut = elide_middle(name, 11);
        // The start gets five columns, room for two; the one left over goes to the end.
        assert_eq!(cut, "活用…階提案");
        assert!(text_cells(&cut) <= 11);
        // An odd room a wide character cannot fill is left over, never overrun.
        for w in 0..24 {
            assert!(text_cells(&elide_middle(name, w)) <= w, "{w}");
            assert_eq!(text_cells(&fit_start(name, w)), w, "{w}");
            assert_eq!(text_cells(&fit_end(name, w)), w, "{w}");
        }
        assert_eq!(fit_start("【 LIFE】.html", 8), "【 L…tml");
        assert_eq!(fit_end("12 K", 6), "  12 K");
        // A decomposed accent (macOS names) takes no column and stays with its letter.
        assert_eq!(text_cells("cafe\u{301}"), 4);
        assert_eq!(elide_middle("cafe\u{301}-menu.txt", 9), "cafe\u{301}….txt");
    }

    #[test]
    fn tabs_land_on_the_grid_and_control_characters_stay_put() {
        assert_eq!(display_line("a\tb", 4), "a   b");
        assert_eq!(display_line("ab\tc", 4), "ab  c");
        assert_eq!(display_line("abcd\te", 4), "abcd    e");
        assert_eq!(
            display_line("x\u{7}y", 4),
            "x\u{b7}y",
            "a bell must not ring the terminal"
        );
        assert_eq!(display_line("plain", 4), "plain");
    }

    #[test]
    fn missing_timestamp_is_a_dash_not_a_panic() {
        assert_eq!(format_time(None), "-");
        assert_eq!(format_time(Some(SystemTime::UNIX_EPOCH)).len(), 16);
    }
}
