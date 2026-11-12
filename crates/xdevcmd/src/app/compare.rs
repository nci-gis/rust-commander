//! Compare the panes, and synchronise them from the plan.

use super::*;

impl App {
    /// The two directories to compare: this pane's and the other's, both ordinary
    /// listings (not an archive, not search results).
    pub(super) fn compare_pair(&mut self) -> Option<(PaneId, PathBuf, PaneId, PathBuf, bool)> {
        let plain = |f: &FilePane| f.archive.is_none() && f.results.is_none();
        let me = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .filter(|f| plain(f));
        let Some(me) = me else {
            self.status = "compare works between two directories".into();
            return None;
        };
        let (left, hidden) = (me.cwd.clone(), me.show_hidden);
        let other = self.other_file_pane();
        let right = other
            .and_then(|o| self.panes.get(&o))
            .and_then(Pane::as_files)
            .filter(|f| plain(f))
            .map(|f| f.cwd.clone());
        match (other, right) {
            (Some(o), Some(r)) if r != left => Some((self.focus, left, o, r, hidden)),
            (Some(_), Some(_)) => {
                self.status = "both panes show the same directory".into();
                None
            }
            _ => {
                self.status = "compare needs a second pane on a directory".into();
                None
            }
        }
    }

    /// Alt+K: mark, in each pane, what is newer there or missing on the other side —
    /// so F5 from either pane copies exactly what the other lacks.
    pub(super) fn compare_panes(&mut self) {
        use xdevcmd_core::compare::{compare, State};
        let Some((me, left, other, right, hidden)) = self.compare_pair() else {
            return;
        };
        let diffs = match compare(&left, &right, false, hidden) {
            Ok(d) => d,
            Err(e) => {
                self.status = format!("cannot compare: {e}");
                return;
            }
        };
        let count = |s: &[State]| diffs.iter().filter(|d| s.contains(&d.state)).count();
        let mark = |f: &mut FilePane, root: &Path, states: &[State]| {
            f.marked.clear();
            for d in diffs.iter().filter(|d| states.contains(&d.state)) {
                let p = root.join(&d.rel);
                if f.entries.iter().any(|e| e.path == p) {
                    f.marked.insert(p);
                }
            }
        };
        if let Some(f) = self.panes.get_mut(&me).and_then(Pane::as_files_mut) {
            mark(f, &left, &[State::LeftOnly, State::LeftNewer]);
        }
        if let Some(f) = self.panes.get_mut(&other).and_then(Pane::as_files_mut) {
            mark(f, &right, &[State::RightOnly, State::RightNewer]);
        }
        let mine = count(&[State::LeftOnly, State::LeftNewer]);
        let theirs = count(&[State::RightOnly, State::RightNewer]);
        let clash = count(&[State::Conflict]);
        let same = count(&[State::Same]);
        let clash = if clash > 0 {
            format!(" \u{b7} {clash} differ at the same time \u{2014} left alone")
        } else {
            String::new()
        };
        // What could not be read is named, never taken for "nothing there".
        let unknown: Vec<String> = diffs
            .iter()
            .filter(|d| d.state.unknown() && d.error.is_some())
            .map(|d| shown_rel(&d.rel))
            .collect();
        let unknown = match unknown.len() {
            0 => String::new(),
            1 => format!(" \u{b7} \u{26a0} {} not read in full", unknown[0]),
            n => format!(
                " \u{b7} \u{26a0} {n} not read in full ({}, \u{2026})",
                unknown[0]
            ),
        };
        self.status = format!(
            "marked: {mine} here, {theirs} there, newer or missing on the other side{clash}{unknown} \u{b7} {same} the same"
        );
    }

    /// Alt+Y: both trees, compared all the way down on a thread (a big tree or a network
    /// drive can take a while, and Esc stops it), then a plan to make them agree.
    pub(super) fn begin_sync(&mut self) {
        use std::sync::atomic::{AtomicBool, AtomicU64};
        use std::sync::Arc;
        let Some((_, left, _, right, hidden)) = self.compare_pair() else {
            return;
        };
        if let Some(old) = self.comparing.take() {
            old.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // A screenshot (the tests) has no later frame to bring the result to: compare
        // there and then.
        if !self.interactive {
            return match xdevcmd_core::compare::compare(&left, &right, true, hidden) {
                Ok(diffs) => self.show_sync_plan(&left, &right, &diffs),
                Err(e) => self.status = format!("cannot compare: {e}"),
            };
        }
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let (l, r, stop, seen) = (left.clone(), right.clone(), stop.clone(), seen.clone());
            std::thread::spawn(move || {
                let _ = tx.send(xdevcmd_core::compare::compare_until(
                    &l, &r, true, hidden, &stop, &seen,
                ));
            });
        }
        self.status = "comparing the two trees\u{2026} Esc stops".into();
        self.comparing = Some(Comparing {
            left,
            right,
            stop,
            seen,
            shown: 0,
            rx,
        });
    }

    /// Alt+Y's compare has come in, or moved on: show its plan, or how far it is.
    pub(super) fn poll_comparing(&mut self) -> bool {
        let Some(c) = self.comparing.as_mut() else {
            return false;
        };
        match c.rx.try_recv() {
            Ok(Some(Ok(diffs))) => {
                let (left, right) = (c.left.clone(), c.right.clone());
                self.comparing = None;
                self.show_sync_plan(&left, &right, &diffs);
                true
            }
            Ok(Some(Err(e))) => {
                self.comparing = None;
                self.status = format!("cannot compare: {e}");
                true
            }
            Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.comparing = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                let n = c.seen.load(std::sync::atomic::Ordering::Relaxed);
                if n != c.shown {
                    c.shown = n;
                    self.status =
                        format!("comparing the two trees\u{2026} {n} looked at \u{b7} Esc stops");
                    return true;
                }
                false
            }
        }
    }

    pub(super) fn show_sync_plan(
        &mut self,
        left: &Path,
        right: &Path,
        diffs: &[xdevcmd_core::compare::Diff],
    ) {
        use xdevcmd_core::compare::{plan, Side, State};
        let pairs = plan(left, right, diffs);
        let mut body = vec![
            format!("\u{2192} this pane  {}", tidy(left)),
            format!("\u{2190} the other  {}", tidy(right)),
            String::new(),
        ];
        let mut overwrites = 0;
        let mut unknown = 0;
        let which = |s: Side| match s {
            Side::Left => "this pane",
            Side::Right => "the other",
        };
        for d in diffs {
            let slash = if d.is_dir { "/" } else { "" };
            let rel = shown_rel(&d.rel);
            let why = d.error.as_deref().unwrap_or_default();
            let line = match d.state {
                State::LeftOnly => format!("\u{2192} {rel}{slash}"),
                State::RightOnly => format!("\u{2190} {rel}{slash}"),
                State::LeftNewer => {
                    overwrites += 1;
                    format!("\u{2192} {rel}  replaces an older copy")
                }
                State::RightNewer => {
                    overwrites += 1;
                    format!("\u{2190} {rel}  replaces an older copy")
                }
                State::Conflict => {
                    format!("\u{2260} {rel}  same time, different size \u{2014} left alone")
                }
                State::CaseClash => format!(
                    "\u{2260} {rel}{slash}  same name, different case on the other side \u{2014} left alone"
                ),
                State::Same => continue,
                State::Unreadable(s) => {
                    unknown += 1;
                    format!(
                        "\u{26a0} {rel}/  cannot be read in {} ({why}) \u{2014} nothing under it is copied",
                        which(s)
                    )
                }
                State::Missing(s) => {
                    unknown += 1;
                    format!(
                        "\u{26a0} {rel}/  went away in {} while compared \u{2014} left alone",
                        which(s)
                    )
                }
                State::Incomplete(s) if d.error.is_some() => {
                    unknown += 1;
                    format!(
                        "\u{26a0} {rel}/  not all of it could be read in {} ({why})",
                        which(s)
                    )
                }
                State::Incomplete(s) => format!(
                    "? {rel}{slash}  not copied: {} could not be read in full here",
                    which(s)
                ),
            };
            body.push(line);
        }
        if pairs.is_empty() {
            self.status = if body.len() > 3 {
                "nothing to copy \u{2014} only differences that need a person".into()
            } else {
                "the two trees already agree".into()
            };
            if body.len() <= 3 {
                return;
            }
        }
        let n = pairs.len();
        let mut title = match overwrites {
            0 => format!("Synchronise \u{b7} {n} to copy"),
            o => format!("Synchronise \u{b7} {n} to copy, {o} replacing older files"),
        };
        if unknown > 0 {
            title.push_str(&format!(" \u{b7} \u{26a0} {unknown} not read"));
        }
        self.modal = Some(Modal::Ask(modal::Ask {
            title,
            body,
            danger: overwrites > 0,
            yes: (n > 0).then(|| (format!("copy {n}"), Cmd::Sync(pairs))),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }
}

/// A path below the compared roots as shown: "." for the roots themselves.
fn shown_rel(rel: &Path) -> String {
    if rel.as_os_str().is_empty() {
        ".".into()
    } else {
        rel.display().to_string()
    }
}
