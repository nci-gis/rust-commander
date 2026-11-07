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

/// A scrollbar as drawn: enough to turn the pointer back into a scroll position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    pub track: Rect,
    pub total: usize,
    pub offset: usize,
}

impl Bar {
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.track.contains(Position::new(x, y))
    }

    fn thumb(&self) -> (u16, u16) {
        let h = self.track.height;
        thumb(h, self.total, h as usize, self.offset).unwrap_or((0, h))
    }

    /// Where on the thumb the pointer took hold. Pressed on the thumb, that spot; on
    /// the track, its middle, so the thumb jumps to centre on the pointer.
    pub fn grab(&self, y: u16) -> u16 {
        let (start, len) = self.thumb();
        let row = y.saturating_sub(self.track.y);
        if row >= start && row < start + len {
            row - start
        } else {
            len / 2
        }
    }

    /// The offset that puts the grabbed cell of the thumb under row `y`.
    pub fn offset_at(&self, y: u16, grab: u16) -> usize {
        let h = self.track.height as usize;
        let (_, len) = self.thumb();
        let room = h.saturating_sub(len as usize);
        let max_offset = self.total.saturating_sub(h);
        if room == 0 {
            return 0;
        }
        let row = (y as isize - self.track.y as isize - grab as isize).clamp(0, room as isize);
        // Rounded up, so the thumb drawn for this offset starts on exactly this row.
        ((row as usize * max_offset).div_ceil(room)).min(max_offset)
    }
}

/// Draw the thumb into `track`, over whatever border is there. `offset` is the first
/// line on screen, counted from the top of everything. Returns the bar when there is
/// one, for the mouse.
pub fn draw(
    buf: &mut Buffer,
    track: Rect,
    total: usize,
    offset: usize,
    style: Style,
) -> Option<Bar> {
    let visible = track.height as usize;
    let (start, len) = thumb(track.height, total, visible, offset)?;
    for row in start..start + len {
        if let Some(cell) = buf.cell_mut(Position::new(track.x, track.y + row)) {
            // A full block: a line-thin thumb on a line-thin border was easy to miss.
            cell.set_symbol("\u{2588}").set_style(style);
        }
    }
    Some(Bar {
        track,
        total,
        offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_the_thumb_to_a_row_draws_it_on_that_row() {
        for total in [11, 20, 37, 100, 5000] {
            let bar = |offset| Bar {
                track: Rect::new(0, 3, 1, 10),
                total,
                offset,
            };
            let (_, len) = bar(0).thumb();
            for row in 0..=(10 - len) {
                let o = bar(0).offset_at(3 + row, 0);
                assert_eq!(bar(o).thumb().0, row, "total {total}, row {row}");
            }
            assert_eq!(bar(0).offset_at(0, 0), 0, "above the track is the top");
            assert_eq!(bar(0).offset_at(99, 0), total - 10, "below it, the end");
        }
    }

    #[test]
    fn a_click_on_the_track_centres_the_thumb_and_on_the_thumb_holds_it() {
        let bar = Bar {
            track: Rect::new(0, 0, 1, 10),
            total: 20,
            offset: 0,
        };
        // Thumb: rows 0-4.
        assert_eq!(bar.grab(3), 3);
        assert_eq!(bar.grab(8), 2);
    }

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
