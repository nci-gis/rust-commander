//! The walker: a small agent that walks across the screen on the row of the file panes'
//! bottom edge, a step a second, when `[general] animation` is on (off by default). When a message comes it
//! goes to the status line and stands just before it for `[general]
//! animation_hold_secs` (5 to 15, 5 by default), then goes back to walking the pane's
//! edge from where it stopped, the message still showing.
//!
//! ASCII only: a sprite of ambiguous or emoji width would be drawn a column off on some
//! terminals (Round 035, W2).

use std::time::{Duration, Instant};

/// Facing right: the dust is behind it, on the left.
const RIGHT: &str = ".[o_o]";
/// Facing left.
const LEFT: &str = "[o_o].";
/// Every fifth step it blinks.
const RIGHT_BLINK: &str = ".[-_-]";
const LEFT_BLINK: &str = "[-_-].";
/// Columns the sprite takes; all four are this wide.
pub const WIDTH: usize = 6;

const STEP: Duration = Duration::from_secs(1);

/// How long it stands before a new message, by default (the owner, 2026-11-24).
pub const HOLD_SECS: u64 = 5;
/// The range `[general] animation_hold_secs` may take.
pub const HOLD_RANGE: std::ops::RangeInclusive<u64> = 5..=15;

#[derive(Debug, Default)]
pub struct Walker {
    /// Columns from the start of its lane.
    pos: usize,
    left: bool,
    steps: u64,
    last: Option<Instant>,
    /// The message it went to stand before, and when; a new message starts it again.
    held: Option<(String, Instant)>,
    /// What the last `tick` said: standing before a message, not walking.
    standing: bool,
}

impl Walker {
    /// Where it is: `true` while it stands before `message` (the first `hold` of each
    /// new message), `false` while it walks. It walks one step if a second has passed
    /// since the last, within a lane `lane` columns wide, turning at either end; the
    /// second starts again when it leaves a message, so it does not jump.
    pub fn tick(&mut self, now: Instant, lane: usize, message: &str, hold: Duration) -> bool {
        if message.is_empty() {
            self.held = None;
        } else if self.held.as_ref().is_none_or(|(m, _)| m != message) {
            self.held = Some((message.to_string(), now));
        }
        let standing = self
            .held
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) < hold);
        self.walk(now, lane, standing);
        self.standing = standing;
        standing
    }

    /// Whether, at the last `tick`, it stood before a message.
    pub fn standing(&self) -> bool {
        self.standing
    }

    fn walk(&mut self, now: Instant, lane: usize, held: bool) {
        let room = lane.saturating_sub(WIDTH);
        self.pos = self.pos.min(room);
        if held {
            self.last = Some(now);
            return;
        }
        let Some(last) = self.last else {
            self.last = Some(now);
            return;
        };
        if now.saturating_duration_since(last) < STEP {
            return;
        }
        self.last = Some(now);
        self.steps += 1;
        if room == 0 {
            return;
        }
        if self.left && self.pos == 0 {
            self.left = false;
        } else if !self.left && self.pos == room {
            self.left = true;
        }
        if self.left {
            self.pos -= 1;
        } else {
            self.pos += 1;
        }
    }

    /// Where it is in its lane.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// What it looks like now: facing the way it walks, blinking every fifth step.
    pub fn sprite(&self) -> &'static str {
        let blink = self.steps % 5 == 4;
        match (self.left, blink) {
            (false, false) => RIGHT,
            (false, true) => RIGHT_BLINK,
            (true, false) => LEFT,
            (true, true) => LEFT_BLINK,
        }
    }
}

/// The row the walker walks on: the bottom edge of the lowest file pane, so it stays
/// low on the screen whichever pane has the focus and however many there are (the
/// owner, 2026-11-24). `None` without a file pane at least three rows high.
pub fn row(file_panes: impl IntoIterator<Item = ratatui::layout::Rect>) -> Option<u16> {
    file_panes
        .into_iter()
        // A box: a top edge, a row, a bottom edge. Lower than that, the bottom edge
        // is the top one, and the pane's title is there.
        .filter(|r| r.height >= 3)
        .map(|r| r.y + r.height - 1)
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_walks_on_the_lowest_file_panes_edge() {
        use ratatui::layout::Rect;
        // Left whole height, right split in two: the left and lower right share the
        // bottom row, the upper right's edge is higher.
        let left = Rect::new(0, 1, 60, 30);
        let up = Rect::new(60, 1, 60, 16);
        let down = Rect::new(60, 17, 60, 14);
        assert_eq!(row([up, left, down]), Some(30));
        assert_eq!(row([up]), Some(16));
        // Panes stacked: the lowest wins, not the tallest.
        assert_eq!(
            row([Rect::new(0, 0, 80, 20), Rect::new(0, 20, 80, 4)]),
            Some(23)
        );
        assert_eq!(row([Rect::new(0, 5, 80, 0)]), None);
        assert_eq!(
            row([Rect::new(0, 5, 80, 2)]),
            None,
            "no room for a bottom edge"
        );
        assert_eq!(row(Vec::<Rect>::new()), None);
    }
    use xdevcmd_core::format::text_cells;

    #[test]
    fn every_sprite_is_ascii_and_one_width() {
        for s in [RIGHT, LEFT, RIGHT_BLINK, LEFT_BLINK] {
            assert!(s.is_ascii(), "{s}");
            assert_eq!(text_cells(s), WIDTH, "{s}");
        }
    }

    #[test]
    fn it_steps_once_a_second_and_turns_at_each_end() {
        let t0 = Instant::now();
        let mut w = Walker::default();
        let lane = WIDTH + 2;
        w.walk(t0, lane, false);
        assert_eq!(w.pos(), 0, "the first frame only starts the clock");
        w.walk(t0 + Duration::from_millis(500), lane, false);
        assert_eq!(w.pos(), 0, "not a second yet");
        let mut at = Vec::new();
        for s in 1..=6 {
            w.walk(t0 + Duration::from_secs(s), lane, false);
            at.push(w.pos());
        }
        assert_eq!(at, [1, 2, 1, 0, 1, 2], "right to the end, back, and again");
    }

    #[test]
    fn a_message_holds_it_and_it_goes_on_from_there() {
        let t0 = Instant::now();
        let mut w = Walker::default();
        let lane = 20;
        for s in 0..=3 {
            w.walk(t0 + Duration::from_secs(s), lane, false);
        }
        assert_eq!(w.pos(), 3);
        for s in 4..=9 {
            w.walk(t0 + Duration::from_secs(s), lane, true);
        }
        assert_eq!(w.pos(), 3, "held while the message shows");
        w.walk(t0 + Duration::from_millis(9_500), lane, false);
        assert_eq!(w.pos(), 3, "the second starts again when the message goes");
        w.walk(t0 + Duration::from_secs(11), lane, false);
        assert_eq!(w.pos(), 4, "and it goes on from where it stopped");
    }

    /// The owner (2026-11-24): before a message for the hold, then walking again, the
    /// message still showing; a new message brings it back.
    #[test]
    fn it_stands_before_a_message_for_the_hold_then_walks_on() {
        let t0 = Instant::now();
        let hold = Duration::from_secs(HOLD_SECS);
        let mut w = Walker::default();
        for s in 0..=2 {
            assert!(!w.tick(t0 + Duration::from_secs(s), 20, "", hold));
        }
        assert_eq!(w.pos(), 2);
        for s in 3..8 {
            assert!(
                w.tick(t0 + Duration::from_secs(s), 20, "copied", hold),
                "{s}"
            );
        }
        assert_eq!(w.pos(), 2, "it did not walk while it stood");
        assert!(
            !w.tick(t0 + Duration::from_secs(8), 20, "copied", hold),
            "the hold is over"
        );
        assert_eq!(
            w.pos(),
            3,
            "walking again from where it stopped, a step a second"
        );
        assert!(!w.tick(t0 + Duration::from_secs(9), 20, "copied", hold));
        assert_eq!(w.pos(), 4);
        assert!(
            w.tick(t0 + Duration::from_secs(10), 20, "deleted", hold),
            "a new message"
        );
        assert!(
            !w.tick(t0 + Duration::from_secs(11), 20, "", hold),
            "no message"
        );
    }

    #[test]
    fn a_lane_too_narrow_keeps_it_still_and_a_narrower_one_pulls_it_in() {
        let t0 = Instant::now();
        let mut w = Walker::default();
        for s in 0..=10 {
            w.walk(t0 + Duration::from_secs(s), 30, false);
        }
        assert_eq!(w.pos(), 10);
        w.walk(t0 + Duration::from_secs(11), WIDTH + 3, false);
        assert!(w.pos() <= 3, "pulled into the narrower lane: {}", w.pos());
        let mut still = Walker::default();
        for s in 0..=5 {
            still.walk(t0 + Duration::from_secs(s), WIDTH, false);
        }
        assert_eq!(still.pos(), 0);
    }
}
