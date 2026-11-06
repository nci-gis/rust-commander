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

/// Fit a string into `width` display columns, putting an ellipsis in the middle so both
/// the start of the name and its extension stay readable.
pub fn elide_middle(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        return s.to_string();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let keep = width - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
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
