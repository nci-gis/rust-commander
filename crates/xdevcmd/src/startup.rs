//! How long xdevcmd took to start, step by step, from `main` to the first frame on
//! screen: shown in Help → About, on the status line with `--timings` (or
//! `XDEVCMD_TIMINGS=1`), and with `--timings` printed as one JSON line on stderr when
//! xdevcmd ends, for a script to read. What the system does before `main` (loading the
//! program) is not counted: a few milliseconds for one static binary.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The finished clock's JSON, kept for `main` to print once the screen is restored.
static LAST: OnceLock<String> = OnceLock::new();

/// The times of this start, once the first frame is drawn.
pub fn last_json() -> Option<String> {
    LAST.get().cloned()
}

#[derive(Debug, Clone)]
pub struct Clock {
    start: Instant,
    last: Instant,
    /// Each step's name and how long it took, in order.
    steps: Vec<(&'static str, Duration)>,
    /// The first frame is on screen: nothing more is counted.
    done: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self::start()
    }
}

impl Clock {
    pub fn start() -> Self {
        Self::start_at(Instant::now())
    }

    fn start_at(at: Instant) -> Self {
        Self {
            start: at,
            last: at,
            steps: Vec::new(),
            done: false,
        }
    }

    /// The step `name` ended now.
    pub fn mark(&mut self, name: &'static str) {
        self.mark_at(name, Instant::now());
    }

    fn mark_at(&mut self, name: &'static str, at: Instant) {
        if self.done {
            return;
        }
        self.steps
            .push((name, at.saturating_duration_since(self.last)));
        self.last = at;
    }

    /// The first frame is drawn: the last step, and the clock stops. True the first time.
    pub fn first_frame(&mut self) -> bool {
        if self.done {
            return false;
        }
        self.mark("first frame");
        self.done = true;
        let _ = LAST.set(self.json());
        true
    }

    pub fn finished(&self) -> bool {
        self.done
    }

    pub fn total(&self) -> Duration {
        self.last.saturating_duration_since(self.start)
    }

    /// "started in 48 ms · config 2 · panes 3 · terminal check 31 · first frame 4".
    pub fn line(&self) -> String {
        let mut s = format!("started in {}", ms(self.total()));
        for (name, d) in &self.steps {
            s.push_str(&format!(" \u{b7} {name} {}", ms_bare(*d)));
        }
        s
    }

    /// One line of JSON: `{"total_ms":48.1,"config_ms":2.0,…}`, names with `_` for spaces.
    pub fn json(&self) -> String {
        let mut s = format!("{{\"total_ms\":{:.1}", self.total().as_secs_f64() * 1000.0);
        for (name, d) in &self.steps {
            s.push_str(&format!(
                ",\"{}_ms\":{:.1}",
                name.replace(' ', "_"),
                d.as_secs_f64() * 1000.0
            ));
        }
        s.push('}');
        s
    }
}

fn ms_bare(d: Duration) -> String {
    let m = d.as_secs_f64() * 1000.0;
    if m < 10.0 {
        format!("{m:.1}")
    } else {
        format!("{m:.0}")
    }
}

fn ms(d: Duration) -> String {
    format!("{} ms", ms_bare(d))
}

/// Whether the start-up times go on the status line and to stderr: `--timings` or
/// `XDEVCMD_TIMINGS` set to anything but empty or `0`.
pub fn wanted(flag: bool, env: Option<String>) -> bool {
    flag || env.is_some_and(|v| !v.is_empty() && v != "0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_add_up_and_stop_at_the_first_frame() {
        let t0 = Instant::now();
        let mut c = Clock::start_at(t0);
        c.mark_at("config", t0 + Duration::from_micros(2_400));
        c.mark_at("terminal check", t0 + Duration::from_millis(33));
        c.steps.push(("first frame", Duration::from_millis(15)));
        c.last = t0 + Duration::from_millis(48);
        c.done = true;
        assert_eq!(
            c.line(),
            "started in 48 ms \u{b7} config 2.4 \u{b7} terminal check 31 \u{b7} first frame 15"
        );
        assert_eq!(
            c.json(),
            "{\"total_ms\":48.0,\"config_ms\":2.4,\"terminal_check_ms\":30.6,\"first_frame_ms\":15.0}"
        );
        c.mark_at("later", t0 + Duration::from_secs(9));
        assert_eq!(c.total(), Duration::from_millis(48), "stopped");
        assert!(!c.first_frame());
    }

    #[test]
    fn timings_are_asked_for_by_the_flag_or_the_variable() {
        assert!(wanted(true, None));
        assert!(wanted(false, Some("1".into())));
        assert!(!wanted(false, Some("0".into())));
        assert!(!wanted(false, Some(String::new())));
        assert!(!wanted(false, None));
    }
}
