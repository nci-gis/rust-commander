//! A scrollbar drawn over a pane's right border: no column taken from the content, and
//! nothing drawn when everything fits.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

/// Where the thumb goes in a track of `height` cells: (first cell, length). `None` when
/// the content fits and there is nothing to scroll.
pub fn thumb(height: u16, total: usize, visible: usize, offset: usize) -> Option<(u16, u16)> {
    if height == 0 || total <= visible || visible == 0 {
        return None;
    }
    let h = height as usize;
    let len = (h * visible / total).clamp(1, h);
    let max_offset = total - visible;
    let start = (h - len) * offset.min(max_offset) / max_offset;
    Some((start as u16, len as u16))
}

/// The track: the right border of `area`, beside the rows `body` covers.
pub fn track(area: Rect, body: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(1),
        body.y,
        1,
        body.height,
    )
}

/// Draw the thumb into `track`, over whatever border is there. `offset` is the first
/// line on screen, counted from the top of everything.
pub fn draw(buf: &mut Buffer, track: Rect, total: usize, offset: usize, style: Style) {
    let visible = track.height as usize;
    let Some((start, len)) = thumb(track.height, total, visible, offset) else {
        return;
    };
    for row in start..start + len {
        if let Some(cell) = buf.cell_mut(Position::new(track.x, track.y + row)) {
            cell.set_symbol("\u{2503}").set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_shows_how_much_is_on_screen_and_where() {
        assert_eq!(thumb(10, 5, 10, 0), None, "it all fits");
        assert_eq!(thumb(10, 100, 10, 0), Some((0, 1)), "top");
        assert_eq!(thumb(10, 100, 10, 90), Some((9, 1)), "bottom");
        assert_eq!(thumb(10, 20, 10, 5), Some((2, 5)), "half shown, half way");
        assert_eq!(thumb(10, 100, 10, 500), Some((9, 1)), "past the end clamps");
        assert_eq!(thumb(0, 100, 10, 0), None);
    }
}
