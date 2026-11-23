//! The status line's walker: a small agent that walks along the status line, a step a
//! second, when `[general] animation` is on (off by default). It walks the space
//! between the panes' line and the message; while a message shows it waits just before
//! it, and goes on from where it stopped once the message is gone.
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

#[derive(Debug, Default)]
pub struct Walker {
    /// Columns from the start of its lane.
    pos: usize,
    left: bool,
    steps: u64,
    last: Option<Instant>,
}

impl Walker {
    /// One step if a second has passed since the last, within a lane `lane` columns
    /// wide: it turns at either end. `held` (a message shows) keeps it where it is, and
    /// the second starts again once the message is gone, so it does not jump.
    pub fn tick(&mut self, now: Instant, lane: usize, held: bool) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
        w.tick(t0, lane, false);
        assert_eq!(w.pos(), 0, "the first frame only starts the clock");
        w.tick(t0 + Duration::from_millis(500), lane, false);
        assert_eq!(w.pos(), 0, "not a second yet");
        let mut at = Vec::new();
        for s in 1..=6 {
            w.tick(t0 + Duration::from_secs(s), lane, false);
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
            w.tick(t0 + Duration::from_secs(s), lane, false);
        }
        assert_eq!(w.pos(), 3);
        for s in 4..=9 {
            w.tick(t0 + Duration::from_secs(s), lane, true);
        }
        assert_eq!(w.pos(), 3, "held while the message shows");
        w.tick(t0 + Duration::from_millis(9_500), lane, false);
        assert_eq!(w.pos(), 3, "the second starts again when the message goes");
        w.tick(t0 + Duration::from_secs(11), lane, false);
        assert_eq!(w.pos(), 4, "and it goes on from where it stopped");
    }

    #[test]
    fn a_lane_too_narrow_keeps_it_still_and_a_narrower_one_pulls_it_in() {
        let t0 = Instant::now();
        let mut w = Walker::default();
        for s in 0..=10 {
            w.tick(t0 + Duration::from_secs(s), 30, false);
        }
        assert_eq!(w.pos(), 10);
        w.tick(t0 + Duration::from_secs(11), WIDTH + 3, false);
        assert!(w.pos() <= 3, "pulled into the narrower lane: {}", w.pos());
        let mut still = Walker::default();
        for s in 0..=5 {
            still.tick(t0 + Duration::from_secs(s), WIDTH, false);
        }
        assert_eq!(still.pos(), 0);
    }
}
