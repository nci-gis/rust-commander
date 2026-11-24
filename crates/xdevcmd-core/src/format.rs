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
///
/// Cut between drawn characters (grapheme clusters), never inside one: an accent stays
/// with its letter, an emoji with its VS16 or ZWJ partners, each measured as a whole.
pub fn elided(s: &str, width: usize) -> Option<(usize, usize)> {
    if text_cells(s) <= width {
        return None;
    }
    // Each drawn character: its columns, and how many chars it is made of.
    let g: Vec<(usize, usize)> = graphemes(s)
        .map(|g| (text_cells(g), g.chars().count()))
        .collect();
    let keep = width.saturating_sub(1);
    let (mut head, mut used, mut head_chars) = (0, 0, 0);
    while head < g.len() && used + g[head].0 <= keep.div_ceil(2) {
        used += g[head].0;
        head_chars += g[head].1;
        head += 1;
    }
    let room = keep - used;
    let (mut tail, mut tail_used, mut tail_chars) = (0, 0, 0);
    while tail < g.len() - head && tail_used + g[g.len() - 1 - tail].0 <= room {
        tail_used += g[g.len() - 1 - tail].0;
        tail_chars += g[g.len() - 1 - tail].1;
        tail += 1;
    }
    Some((head_chars, tail_chars))
}

/// The drawn characters of `s` (extended grapheme clusters).
fn graphemes(s: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true)
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

/// Columns a string takes on the terminal. The one measure of text on screen: a CJK
/// character takes two columns, a combining accent none, and an emoji sequence (❤️, a
/// family joined by ZWJ) what the terminal and ratatui draw it as, which is not the sum
/// of its characters. Never `chars().count()` for a width (Round 035, W2).
pub fn text_cells(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// `.cells()`: the columns of a `str` (or a `String`) on the terminal, as
/// [`text_cells`]; read where a width is wanted, as `.chars().count()` was.
pub trait Cells {
    fn cells(&self) -> usize;
}

impl Cells for str {
    fn cells(&self) -> usize {
        text_cells(self)
    }
}

/// Columns one character takes on the terminal: 2 for a wide one, 0 for an accent.
pub fn char_cells(c: char) -> usize {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
}

/// `s` cut to `width` columns, with `…` where it was cut: the end gives way. As
/// [`elide_middle`], which keeps both ends, for text whose start says what it is.
pub fn cut_end(s: &str, width: usize) -> String {
    if text_cells(s) <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for g in graphemes(s) {
        let gw = text_cells(g);
        if used + gw > width - 1 {
            break;
        }
        out.push_str(g);
        used += gw;
    }
    out.push('\u{2026}');
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

    /// An emoji sequence is as wide as the terminal draws it, not the sum of its
    /// characters (beta.3 review: ❤️ measured 1, a ZWJ family 6, both drawn 2).
    #[test]
    fn an_emoji_sequence_is_as_wide_as_it_is_drawn() {
        assert_eq!(text_cells("\u{2764}\u{fe0f}"), 2);
        assert_eq!(text_cells("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"), 2);
        assert_eq!(text_cells("Vi\u{1ec7}t"), 4);
        assert_eq!(text_cells("Vie\u{323}\u{302}t"), 4);
        assert_eq!(text_cells("\u{65e5}\u{672c}"), 4);
    }

    /// Fitting and cutting never come out wider than asked and never split a drawn
    /// character (beta.3 review of the fix above: they still summed per char).
    #[test]
    fn fitting_and_cutting_keep_emoji_and_accents_whole() {
        let heart = "\u{2764}\u{fe0f}";
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        let names = [
            format!("a{heart}"),
            format!("{heart}{heart}b"),
            format!("{family}xyz"),
            format!("{family}{family}{family}"),
            "Vie\u{323}\u{302}t Nam.txt".to_string(),
            "\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{540d}\u{524d}.md".to_string(),
        ];
        for name in &names {
            for w in 1..=text_cells(name) + 1 {
                let fit = fit_start(name, w);
                assert_eq!(text_cells(&fit), w, "fit_start({name:?}, {w}) = {fit:?}");
                let mid = elide_middle(name, w);
                assert!(
                    text_cells(&mid) <= w,
                    "elide_middle({name:?}, {w}) = {mid:?}"
                );
                let end = cut_end(name, w);
                assert!(text_cells(&end) <= w, "cut_end({name:?}, {w}) = {end:?}");
                for out in [&mid, &end] {
                    let kept: String = out.chars().filter(|&c| c != '\u{2026}').collect();
                    for g in graphemes(&kept) {
                        assert!(
                            graphemes(name).any(|whole| whole == g),
                            "{g:?} is part of a drawn character of {name:?}: {out:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cut_end_counts_columns_so_wide_characters_fit() {
        // Six CJK characters are twelve columns: cut to seven, five fit with the `…`.
        let c = cut_end("日本語のコマ", 7);
        assert_eq!(c, "日本語\u{2026}");
        assert!(text_cells(&c) <= 7);
        assert_eq!(cut_end("日本語", 6), "日本語");
        assert_eq!(cut_end("copy", 3), "co\u{2026}");
    }

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
