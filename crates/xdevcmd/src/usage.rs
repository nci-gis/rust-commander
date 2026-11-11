//! Help → Usage (Shift+F1): what xdevcmd itself is using, and whether memory keeps
//! climbing.
//!
//! Memory is sampled every two seconds for the last twenty minutes. A file manager
//! left open all day should sit flat once warm; the trend line is a least-squares
//! slope over the window, and it is only called growth when the window is long enough
//! to mean something and the slope outlives the noise.
//!
//! Where the numbers come from: Linux reads `/proc/self` (no process started). macOS
//! asks `ps`, and only while this view is open. Windows is not measured yet — reading
//! a process's memory there needs an OS call this crate's `forbid(unsafe_code)` rules
//! out without a dependency, and none is worth adding for this.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Sparkline;
use ratatui::Frame;

use xdevcmd_core::format::human_size;

use crate::theme;

const EVERY: Duration = Duration::from_secs(2);
const KEEP: usize = 600; // twenty minutes at one sample per two seconds

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Proc {
    pub rss: u64,
    pub peak: Option<u64>,
    pub threads: Option<u64>,
    pub fds: Option<u64>,
    pub cpu_secs: Option<f64>,
}

/// What xdevcmd holds, counted by the app when the view is drawn.
#[derive(Debug, Clone, Default)]
pub struct Holding {
    pub panes: usize,
    pub tabs: usize,
    pub shells: usize,
    pub jobs: usize,
    pub listed: usize,
}

pub struct Usage {
    started: Instant,
    last: Option<Instant>,
    /// (seconds since start, bytes resident)
    samples: VecDeque<(f64, u64)>,
    latest: Option<Proc>,
    draws: VecDeque<Duration>,
    frames: u64,
    /// The terminal it runs in and how it draws pictures, for a bug report.
    pub terminal: String,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            last: None,
            samples: VecDeque::new(),
            latest: None,
            draws: VecDeque::new(),
            frames: 0,
            terminal: String::new(),
        }
    }
}

impl Usage {
    /// Take a sample if one is due. `viewing` lets platforms where sampling costs a
    /// process (macOS) do it only while someone is looking.
    pub fn tick(&mut self, viewing: bool) {
        if self.last.is_some_and(|t| t.elapsed() < EVERY) {
            return;
        }
        if !cfg!(target_os = "linux") && !viewing {
            return;
        }
        self.last = Some(Instant::now());
        if let Some(p) = read() {
            let t = self.started.elapsed().as_secs_f64();
            self.samples.push_back((t, p.rss));
            if self.samples.len() > KEEP {
                self.samples.pop_front();
            }
            self.latest = Some(p);
        }
    }

    /// How long a frame took to draw.
    pub fn frame(&mut self, d: Duration) {
        self.frames += 1;
        self.draws.push_back(d);
        if self.draws.len() > 240 {
            self.draws.pop_front();
        }
    }

    pub fn trend(&self) -> Trend {
        let v: Vec<(f64, u64)> = self.samples.iter().copied().collect();
        verdict(&v)
    }
}

/// Seconds after start when memory is still finding its level: listings read, tabs
/// restored, the shell's scrollback filling towards its cap. Growth here is expected.
pub const WARM_UP: f64 = 120.0;
/// How much settled history a verdict needs.
pub const WINDOW: f64 = 300.0;
const KIB: f64 = 1024.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trend {
    /// Not enough settled history yet; about this many seconds to go.
    WarmingUp(f64),
    Flat,
    /// `bytes` more over `secs`, as medians of the first and last thirds.
    Growing {
        bytes: u64,
        secs: f64,
    },
    Falling {
        bytes: u64,
        secs: f64,
    },
}

fn median(v: &[(f64, u64)]) -> u64 {
    let mut b: Vec<u64> = v.iter().map(|s| s.1).collect();
    b.sort_unstable();
    b[b.len() / 2]
}

/// Whether memory is climbing, from `(seconds since start, bytes)` samples.
///
/// The first version fitted a line through everything from the first minute on, and
/// called warm-up — a few steps up as xdevcmd settled — a leak. Now warm-up is left out,
/// five minutes of what follows are needed, and the medians of the first and last
/// thirds are compared, so one step does not tilt the answer. Growth counts only when
/// it is both steady (128 KiB a minute) and real (1 MiB, and 2% of the total).
pub fn verdict(samples: &[(f64, u64)]) -> Trend {
    let now = samples.last().map_or(0.0, |s| s.0);
    let settled: Vec<(f64, u64)> = samples.iter().copied().filter(|s| s.0 >= WARM_UP).collect();
    let span = match (settled.first(), settled.last()) {
        (Some(a), Some(b)) => b.0 - a.0,
        _ => 0.0,
    };
    if settled.len() < 30 || span < WINDOW {
        let from = settled.first().map_or(WARM_UP, |s| s.0);
        return Trend::WarmingUp((from + WINDOW - now).max(1.0));
    }
    let third = settled.len() / 3;
    let (a, b) = (&settled[..third], &settled[settled.len() - third..]);
    let centre = |v: &[(f64, u64)]| (v[0].0 + v[v.len() - 1].0) / 2.0;
    let secs = centre(b) - centre(a);
    let (m1, m2) = (median(a) as f64, median(b) as f64);
    let per_min = (m2 - m1) / secs * 60.0;
    let delta = m2 - m1;
    let real = |d: f64| d >= 1024.0 * KIB && d >= m1 * 0.02;
    if per_min >= 128.0 * KIB && real(delta) {
        Trend::Growing {
            bytes: delta as u64,
            secs,
        }
    } else if per_min <= -128.0 * KIB && real(-delta) {
        Trend::Falling {
            bytes: (-delta) as u64,
            secs,
        }
    } else {
        Trend::Flat
    }
}

#[cfg(target_os = "linux")]
pub fn read() -> Option<Proc> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = |key: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
    };
    let ticks = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            // Fields after the command name, which is in parentheses and may hold spaces.
            let rest = s.rsplit_once(')')?.1;
            let f: Vec<&str> = rest.split_whitespace().collect();
            Some(f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?)
        });
    Some(Proc {
        rss: kb("VmRSS:")? * 1024,
        peak: kb("VmHWM:").map(|k| k * 1024),
        threads: kb("Threads:"),
        fds: std::fs::read_dir("/proc/self/fd")
            .ok()
            .map(|d| d.count() as u64),
        // Clock ticks; 100 a second on every Linux xdevcmd is likely to meet.
        cpu_secs: ticks.map(|t| t as f64 / 100.0),
    })
}

#[cfg(target_os = "macos")]
pub fn read() -> Option<Proc> {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=,time=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut it = text.split_whitespace();
    let rss = it.next()?.parse::<u64>().ok()? * 1024;
    let cpu_secs = it.next().and_then(|t| {
        let mut secs = 0.0;
        for part in t.split(':') {
            secs = secs * 60.0 + part.parse::<f64>().ok()?;
        }
        Some(secs)
    });
    Some(Proc {
        rss,
        cpu_secs,
        ..Proc::default()
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn read() -> Option<Proc> {
    None
}

fn uptime(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, s / 60 % 60),
    }
}

pub fn render(frame: &mut Frame, area: Rect, u: &Usage, h: &Holding) {
    let th = theme::t();
    let w = area.width.min(84);
    let ht = area.height.min(22);
    let r = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - ht) / 2,
        w,
        ht,
    );
    crate::modal::clear_overlay(frame, r);
    let block = crate::decor::bordered()
        .border_style(Style::new().fg(th.accent))
        .style(Style::new().bg(th.bg_panel).fg(th.fg))
        .title(Line::from(Span::styled(
            " Usage ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(r);
    frame.render_widget(block, r);
    crate::decor::notch(frame.buffer_mut(), r);

    let mut lines: Vec<(String, Style)> = Vec::new();
    let dim = Style::new().fg(th.fg_dim);
    let plain = Style::new().fg(th.fg);
    lines.push((
        format!(
            "xdevcmd {} \u{b7} pid {} \u{b7} up {}",
            env!("CARGO_PKG_VERSION"),
            std::process::id(),
            uptime(u.started.elapsed())
        ),
        dim,
    ));
    lines.push((String::new(), plain));
    match &u.latest {
        Some(p) => {
            let peak = p
                .peak
                .map(|b| format!(" \u{b7} peak {}", human_size(b)))
                .unwrap_or_default();
            lines.push((format!("memory     {}{peak}", human_size(p.rss)), plain));
            let mut rest = Vec::new();
            if let Some(c) = p.cpu_secs {
                rest.push(format!("cpu {c:.1}s"));
            }
            if let Some(t) = p.threads {
                rest.push(format!("{t} threads"));
            }
            if let Some(f) = p.fds {
                rest.push(format!("{f} open files"));
            }
            if !rest.is_empty() {
                lines.push((format!("process    {}", rest.join(" \u{b7} ")), plain));
            }
        }
        None if cfg!(windows) => lines.push(("memory     not measured on Windows yet".into(), dim)),
        None => lines.push(("memory     measuring\u{2026}".into(), dim)),
    }
    let (trend, style) = match u.trend() {
        Trend::WarmingUp(left) => (
            format!(
                "trend      settling in \u{2014} a verdict in about {}",
                uptime(Duration::from_secs_f64(left))
            ),
            dim,
        ),
        Trend::Flat => ("trend      flat".to_string(), Style::new().fg(th.exec)),
        Trend::Growing { bytes, secs } => (
            format!(
                "trend      up {} in {} \u{2014} a leak if it keeps going",
                human_size(bytes),
                uptime(Duration::from_secs_f64(secs))
            ),
            Style::new().fg(th.err).add_modifier(Modifier::BOLD),
        ),
        Trend::Falling { bytes, secs } => (
            format!(
                "trend      down {} in {}",
                human_size(bytes),
                uptime(Duration::from_secs_f64(secs))
            ),
            Style::new().fg(th.exec),
        ),
    };
    lines.push((trend, style));
    let avg = if u.draws.is_empty() {
        Duration::ZERO
    } else {
        u.draws.iter().sum::<Duration>() / u.draws.len() as u32
    };
    let worst = u.draws.iter().max().copied().unwrap_or_default();
    lines.push((
        format!(
            "drawing    {:.1} ms a frame, worst {:.1} ms of the last {} \u{b7} {} frames",
            avg.as_secs_f64() * 1000.0,
            worst.as_secs_f64() * 1000.0,
            u.draws.len(),
            u.frames
        ),
        plain,
    ));
    if !u.terminal.is_empty() {
        lines.push((format!("terminal   {}", u.terminal), plain));
    }
    let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    lines.push((
        format!(
            "holding    {} \u{b7} {} \u{b7} {} listed \u{b7} {} \u{b7} {}",
            n(h.panes, "pane", "panes"),
            n(h.tabs, "tab", "tabs"),
            n(h.listed, "row", "rows"),
            n(h.shells, "shell", "shells"),
            n(h.jobs, "job", "jobs"),
        ),
        plain,
    ));
    lines.push((
        "           each shell keeps at most 5000 lines of scrollback".into(),
        dim,
    ));
    lines.push((String::new(), plain));
    // What the graph shows: the newest samples, one per column.
    let width = inner.width.saturating_sub(2) as usize;
    let shown: Vec<u64> = u
        .samples
        .iter()
        .rev()
        .take(width)
        .rev()
        .map(|s| s.1)
        .collect();
    let (lo, hi) = (
        shown.iter().min().copied().unwrap_or(0),
        shown.iter().max().copied().unwrap_or(0),
    );
    lines.push((
        format!(
            "resident memory, last {}: {} to {} (a bar every 2 s)",
            uptime(Duration::from_secs(2 * shown.len() as u64)),
            human_size(lo),
            human_size(hi)
        ),
        dim,
    ));
    for (i, (text, style)) in lines.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        frame.buffer_mut().set_line(
            inner.x + 1,
            inner.y + i as u16,
            &Line::from(Span::styled(text.clone(), *style)),
            inner.width.saturating_sub(2),
        );
    }
    let top = inner.y + lines.len() as u16;
    let spark_h = inner.height.saturating_sub(lines.len() as u16 + 1);
    if spark_h > 0 {
        // The floor is three quarters of the lowest value, not the lowest value itself:
        // measured from the minimum, a 0.9 MB step on 5 MB drew as a cliff from nothing
        // and read as an alarm. This way a change shows at its real proportion.
        let floor = lo / 4 * 3;
        let data: Vec<u64> = shown.iter().map(|b| b - floor).collect();
        frame.render_widget(
            Sparkline::default()
                .data(&data)
                .style(Style::new().fg(th.accent)),
            Rect::new(inner.x + 1, top, width as u16, spark_h),
        );
    }
    frame.buffer_mut().set_line(
        inner.x + 1,
        inner.y + inner.height.saturating_sub(1),
        &Line::from(Span::styled("Esc close", dim)),
        inner.width,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples every 2 s for `mins` minutes, memory given by `f(seconds)`.
    fn run(mins: u64, f: impl Fn(f64) -> u64) -> Vec<(f64, u64)> {
        (0..mins * 30)
            .map(|i| (i as f64 * 2.0, f(i as f64 * 2.0)))
            .collect()
    }

    #[test]
    fn steps_while_settling_in_are_not_a_leak() {
        // What the first report showed: 5.2 MB stepping up to 6.1 MB in two minutes,
        // then staying there. It was called "growing" after 1m 54s.
        let warm = |t: f64| match t {
            t if t < 30.0 => 5_200_000,
            t if t < 90.0 => 5_700_000,
            _ => 6_100_000,
        };
        assert!(
            matches!(verdict(&run(2, warm)), Trend::WarmingUp(_)),
            "too early to say"
        );
        assert_eq!(verdict(&run(10, warm)), Trend::Flat);
        // Jitter around a level is flat too.
        let noisy = |t: f64| 20_000_000 + ((t as u64 * 7919) % 300_000);
        assert_eq!(verdict(&run(15, noisy)), Trend::Flat);
    }

    #[test]
    fn a_steady_climb_is_called_growth() {
        // 200 KiB a minute on 20 MB: a slow leak, but a leak.
        let leak = |t: f64| 20_000_000 + (t / 60.0 * 200.0 * 1024.0) as u64;
        match verdict(&run(15, leak)) {
            Trend::Growing { bytes, secs } => {
                assert!(bytes > 1 << 20, "{bytes}");
                assert!(secs > 300.0, "{secs}");
            }
            other => panic!("{other:?}"),
        }
        // The same climb with under five minutes past warm-up is not judged yet.
        assert!(matches!(verdict(&run(6, leak)), Trend::WarmingUp(_)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn this_process_can_be_measured() {
        let p = read().unwrap();
        assert!(p.rss > 1 << 20, "{p:?}");
        assert!(p.peak.unwrap() >= p.rss);
        assert!(p.threads.unwrap() >= 1);
    }
}
