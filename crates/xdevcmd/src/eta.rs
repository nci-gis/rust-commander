//! When a job will end, from what it has really done (Round 031): no probe, no guess
//! from the disk's kind. The work is counted in bytes plus a fixed cost per file, so a
//! run of small files and one large file are measured on one scale; its rate is a moving
//! average over a few seconds. Shown only once it has settled, and never while nothing
//! moves: a number that swings, or one held up while a share is gone, is worse than none.

use std::time::{Duration, Instant};

/// What one file costs beyond its bytes: opening, making, closing, its metadata.
const FILE_COST: f64 = 64.0 * 1024.0;
/// The moving average's time constant.
const TAU: f64 = 5.0;
/// Nothing is said before the job has run this long…
const FIRST: Duration = Duration::from_secs(3);
/// …nor unless every estimate of this last stretch is within [`SPREAD`] of the latest.
const SETTLE: Duration = Duration::from_secs(2);
const SPREAD: f64 = 0.25;
/// Nothing is said once nothing has moved for this long.
const STALL: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub struct Rate {
    start: Option<Instant>,
    /// The last observation: when, and the work done by then.
    last: Option<(Instant, f64)>,
    /// Work per second, averaged.
    rate: Option<f64>,
    /// Recent estimates of the time left, for "has it settled".
    recent: Vec<(Instant, f64)>,
}

fn work(bytes: u64, files: u64) -> f64 {
    bytes as f64 + files as f64 * FILE_COST
}

impl Rate {
    /// The job had done `bytes` and `files` at `at`, of `total` (bytes, files).
    pub fn observe(&mut self, at: Instant, bytes: u64, files: u64, total: (u64, u64)) {
        let w = work(bytes, files);
        self.start.get_or_insert(at);
        if let Some((t0, w0)) = self.last {
            let dt = at.saturating_duration_since(t0).as_secs_f64();
            if dt <= 0.0 {
                return;
            }
            let now = (w - w0).max(0.0) / dt;
            let alpha = 1.0 - (-dt / TAU).exp();
            self.rate = Some(match self.rate {
                Some(r) => r + alpha * (now - r),
                None => now,
            });
        }
        self.last = Some((at, w));
        if let Some(left) = self.left_secs(total) {
            self.recent.push((at, left));
            self.recent
                .retain(|(t, _)| at.saturating_duration_since(*t) <= SETTLE);
        }
    }

    fn left_secs(&self, (bytes, files): (u64, u64)) -> Option<f64> {
        let rate = self.rate.filter(|r| *r > 0.0)?;
        let (_, done) = self.last?;
        Some((work(bytes, files) - done).max(0.0) / rate)
    }

    /// The time left at `at`, if it can honestly be said.
    pub fn left(&self, at: Instant, total: (u64, u64)) -> Option<Duration> {
        let start = self.start?;
        let (last, _) = self.last?;
        if at.saturating_duration_since(start) < FIRST || at.saturating_duration_since(last) > STALL
        {
            return None;
        }
        let now = self.left_secs(total)?;
        let first = self.recent.first()?.0;
        let settled = last.saturating_duration_since(first) >= SETTLE.mul_f64(0.9)
            && self
                .recent
                .iter()
                .all(|(_, l)| (l - now).abs() <= SPREAD * now.max(1.0));
        if !settled {
            return None;
        }
        // Counted from the last observation.
        let gone = at.saturating_duration_since(last).as_secs_f64();
        Some(Duration::from_secs_f64((now - gone).max(0.0)))
    }
}

/// "about 40 s", "about 4 min", "about 1 h 20 min": rounded, never seconds past a
/// minute.
pub fn say(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("about {} s", (s.div_ceil(5) * 5).max(5))
    } else if s < 3600 {
        format!("about {} min", (s + 30) / 60)
    } else {
        let m = (s + 30) / 60;
        format!("about {} h {} min", m / 60, m % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;

    /// Feed `steps` observations, one every 250 ms from `t0`, from `step(i)` (bytes,
    /// files done by then).
    fn feed(
        r: &mut Rate,
        t0: Instant,
        n: u32,
        total: (u64, u64),
        step: impl Fn(u32) -> (u64, u64),
    ) {
        for i in 0..=n {
            let (b, f) = step(i);
            r.observe(t0 + Duration::from_millis(250) * i, b, f, total);
        }
    }

    #[test]
    fn a_steady_copy_says_the_time_left_once_settled() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        // 10 MB/s of one 600 MB file: 60 s in all.
        let total = (600 * MB, 1);
        feed(&mut r, t0, 4, total, |i| (u64::from(i) * 10 * MB / 4, 0));
        assert_eq!(
            r.left(t0 + Duration::from_secs(1), total),
            None,
            "too early"
        );
        feed(&mut r, t0, 40, total, |i| (u64::from(i) * 10 * MB / 4, 0));
        let at = t0 + Duration::from_secs(10);
        let left = r.left(at, total).expect("settled").as_secs_f64();
        assert!((48.0..=52.0).contains(&left), "{left}");
        assert_eq!(say(Duration::from_secs_f64(left)), "about 50 s");
    }

    #[test]
    fn a_slowdown_lengthens_the_estimate_and_hides_it_while_it_moves() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        let total = (1000 * MB, 1);
        feed(&mut r, t0, 40, total, |i| (u64::from(i) * 10 * MB / 4, 0));
        let before = r.left(t0 + Duration::from_secs(10), total).unwrap();
        // From 10 s on, a tenth of the speed.
        let base = 100 * MB;
        for i in 1..=8 {
            r.observe(
                t0 + Duration::from_secs(10) + Duration::from_millis(250) * i,
                base + u64::from(i) * MB / 4,
                0,
                total,
            );
        }
        assert_eq!(
            r.left(t0 + Duration::from_secs(12), total),
            None,
            "not settled while the rate falls"
        );
        for i in 9..=120 {
            r.observe(
                t0 + Duration::from_secs(10) + Duration::from_millis(250) * i,
                base + u64::from(i) * MB / 4,
                0,
                total,
            );
        }
        let after = r.left(t0 + Duration::from_secs(40), total).unwrap();
        assert!(after > before * 5, "{before:?} then {after:?}");
    }

    /// Ten thousand small files: the bytes alone hardly move, the files do.
    #[test]
    fn a_burst_of_small_files_is_counted_by_files_too() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        let total = (10_000 * 1024, 10_000);
        // 500 files a second, 1 KiB each: 20 s in all.
        feed(&mut r, t0, 40, total, |i| {
            let f = u64::from(i) * 125;
            (f * 1024, f)
        });
        let left = r
            .left(t0 + Duration::from_secs(10), total)
            .unwrap()
            .as_secs_f64();
        assert!((8.0..=12.0).contains(&left), "{left}");
    }

    #[test]
    fn nothing_is_said_while_nothing_moves() {
        let t0 = Instant::now();
        let mut r = Rate::default();
        let total = (600 * MB, 1);
        feed(&mut r, t0, 40, total, |i| (u64::from(i) * 10 * MB / 4, 0));
        assert!(r.left(t0 + Duration::from_secs(10), total).is_some());
        assert_eq!(r.left(t0 + Duration::from_secs(20), total), None);
    }

    #[test]
    fn the_time_left_is_rounded_as_a_person_would() {
        assert_eq!(say(Duration::from_secs(3)), "about 5 s");
        assert_eq!(say(Duration::from_secs(41)), "about 45 s");
        assert_eq!(say(Duration::from_secs(250)), "about 4 min");
        assert_eq!(say(Duration::from_secs(4800)), "about 1 h 20 min");
    }
}
