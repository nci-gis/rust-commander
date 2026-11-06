//! Help → Usage (Shift+F1): what rcmd itself is using, and whether memory keeps
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
use ratatui::widgets::{Block, Clear, Sparkline};
use ratatui::Frame;

use rcmd_core::format::human_size;

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

/// What rcmd holds, counted by the app when the view is drawn.
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

    /// Bytes per minute, once there are a minute's worth of samples.
    pub fn trend(&self) -> Option<f64> {
        slope(self.samples.iter().copied())
    }
}

/// Least-squares slope of resident memory over time, in bytes per minute.
pub fn slope(points: impl Iterator<Item = (f64, u64)>) -> Option<f64> {
    let pts: Vec<(f64, f64)> = points.map(|(t, b)| (t, b as f64)).collect();
    let n = pts.len() as f64;
    let span = pts.last()?.0 - pts.first()?.0;
    if pts.len() < 10 || span < 60.0 {
        return None;
    }
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    (sxx > 0.0).then(|| sxy / sxx * 60.0)
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
        // Clock ticks; 100 a second on every Linux rcmd is likely to meet.
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
    frame.render_widget(Clear, r);
    let block = Block::bordered()
        .border_style(Style::new().fg(th.accent))
        .style(Style::new().bg(th.bg_panel).fg(th.fg))
        .title(Line::from(Span::styled(
            " Usage ",
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(r);
    frame.render_widget(block, r);

    let mut lines: Vec<(String, Style)> = Vec::new();
    let dim = Style::new().fg(th.fg_dim);
    let plain = Style::new().fg(th.fg);
    lines.push((
        format!(
            "rcmd {} \u{b7} pid {} \u{b7} up {}",
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
        None => ("trend      needs a minute of samples".to_string(), dim),
        // Under 64 KiB a minute is allocator noise: a viewer opened, a listing read.
        Some(s) if s.abs() < 64.0 * 1024.0 => {
            ("trend      flat".to_string(), Style::new().fg(th.exec))
        }
        Some(s) if s > 0.0 => (
            format!(
                "trend      growing {}/min over {} \u{2014} worth a look if it stays",
                human_size(s as u64),
                uptime(Duration::from_secs_f64(
                    u.samples.back().map_or(0.0, |b| b.0) - u.samples.front().map_or(0.0, |f| f.0)
                ))
            ),
            Style::new().fg(th.err).add_modifier(Modifier::BOLD),
        ),
        Some(s) => (
            format!("trend      falling {}/min", human_size((-s) as u64)),
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
    lines.push((
        format!(
            "resident memory, last {} (one bar per 2 s)",
            uptime(Duration::from_secs(2 * u.samples.len() as u64))
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
        let width = inner.width.saturating_sub(2) as usize;
        // Relative to the lowest value shown, so a few hundred KiB of change is visible
        // against tens of MiB of baseline.
        let shown: Vec<u64> = u
            .samples
            .iter()
            .rev()
            .take(width)
            .rev()
            .map(|s| s.1)
            .collect();
        let floor = shown.iter().min().copied().unwrap_or(0);
        let data: Vec<u64> = shown.iter().map(|b| b - floor + 1).collect();
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

    #[test]
    fn the_slope_needs_a_minute_and_reads_in_bytes_per_minute() {
        let flat: Vec<(f64, u64)> = (0..40).map(|i| (i as f64 * 2.0, 50_000_000)).collect();
        assert_eq!(slope(flat.into_iter()), Some(0.0));
        let short: Vec<(f64, u64)> = (0..20).map(|i| (i as f64, 1)).collect();
        assert_eq!(
            slope(short.into_iter()),
            None,
            "twenty seconds is not a trend"
        );
        // 1 KiB a second is 60 KiB a minute.
        let rising: Vec<(f64, u64)> = (0..60)
            .map(|i| (i as f64 * 2.0, 1_000_000 + i * 2048))
            .collect();
        let s = slope(rising.into_iter()).unwrap();
        assert!((s - 61_440.0).abs() < 1.0, "{s}");
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
