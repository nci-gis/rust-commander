//! Compare the panes, and synchronise them from the plan.

use super::*;
use xdevcmd_core::task::Poll;

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
            self.status = tr!(
                "compare.compare_works_between_two_directories",
                "compare works between two directories"
            )
            .into();
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
                self.status = tr!(
                    "compare.both_panes_show_the_same_directory",
                    "both panes show the same directory"
                )
                .into();
                None
            }
            _ => {
                self.status = tr!(
                    "compare.compare_needs_a_second_pane_on",
                    "compare needs a second pane on a directory"
                )
                .into();
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
                self.status = tr!("compare.cannot_compare", "cannot compare: {e}", e = e);
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
            tr!(
                "compare.u_differ_at_the_same_time",
                " \u{b7} {clash} differ at the same time \u{2014} left alone",
                clash = clash
            )
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
            1 => tr!(
                "compare.u_u_not_read_in_full",
                " \u{b7} \u{26a0} {v} not read in full",
                v = unknown[0]
            ),
            n => tr!(
                "compare.u_u_not_read_in_full_2",
                " \u{b7} \u{26a0} {n} not read in full ({v}, \u{2026})",
                n = n,
                v = unknown[0]
            ),
        };
        self.status = tr!("compare.marked_here_there_newer_or_missing", "marked: {mine} here, {theirs} there, newer or missing on the other side{clash}{unknown} \u{b7} {same} the same", mine = mine, theirs = theirs, clash = clash, unknown = unknown, same = same);
    }

    /// Alt+Y: both trees, compared all the way down on a thread (a big tree or a network
    /// drive can take a while, and Esc stops it), then a plan to make them agree.
    pub(super) fn begin_sync(&mut self) {
        use std::sync::atomic::AtomicU64;
        use std::sync::Arc;
        let Some((_, left, _, right, hidden)) = self.compare_pair() else {
            return;
        };
        if let Some(old) = self.comparing.take() {
            old.task.stop();
        }
        // A screenshot (the tests) has no later frame to bring the result to: compare
        // there and then.
        if !self.interactive {
            return match xdevcmd_core::compare::compare(&left, &right, true, hidden) {
                Ok(diffs) => self.show_sync_plan(&left, &right, &diffs),
                Err(e) => self.status = tr!("compare.cannot_compare", "cannot compare: {e}", e = e),
            };
        }
        let seen = Arc::new(AtomicU64::new(0));
        let task = {
            let (l, r, seen) = (left.clone(), right.clone(), seen.clone());
            xdevcmd_core::task::Task::spawn(move |stop| {
                xdevcmd_core::compare::compare_until(&l, &r, true, hidden, stop, &seen)
            })
        };
        self.status = tr!(
            "compare.comparing_the_two_trees_u_esc",
            "comparing the two trees\u{2026} Esc stops"
        )
        .into();
        self.comparing = Some(Comparing {
            left,
            right,
            seen,
            shown: 0,
            task,
        });
    }

    /// Alt+Y's compare has come in, or moved on: show its plan, or how far it is.
    pub(super) fn poll_comparing(&mut self) -> bool {
        let Some(c) = self.comparing.as_mut() else {
            return false;
        };
        match c.task.poll() {
            Poll::Done(Some(Ok(diffs))) => {
                let (left, right) = (c.left.clone(), c.right.clone());
                self.comparing = None;
                self.show_sync_plan(&left, &right, &diffs);
                true
            }
            Poll::Done(Some(Err(e))) => {
                self.comparing = None;
                self.status = tr!("compare.cannot_compare", "cannot compare: {e}", e = e);
                true
            }
            // Stopped: Esc said so already.
            Poll::Done(None) => {
                self.comparing = None;
                true
            }
            // Its status line said "comparing…" on, before (W8).
            Poll::Died => {
                self.comparing = None;
                self.status = tr!(
                    "compare.comparing_the_two_trees",
                    "comparing the two trees {DIED}",
                    DIED = crate::app::died()
                );
                true
            }
            Poll::Pending => {
                let n = c.seen.load(std::sync::atomic::Ordering::Relaxed);
                if n != c.shown {
                    c.shown = n;
                    self.status = tr!(
                        "compare.comparing_the_two_trees_u_looked",
                        "comparing the two trees\u{2026} {n} looked at \u{b7} Esc stops",
                        n = n
                    );
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
            tr!(
                "compare.u_this_pane",
                "\u{2192} this pane  {v}",
                v = tidy(left)
            ),
            tr!(
                "compare.u_the_other",
                "\u{2190} the other  {v}",
                v = tidy(right)
            ),
            String::new(),
        ];
        let mut overwrites = 0;
        let mut unknown = 0;
        let which = |s: Side| match s {
            Side::Left => tr!("app_compare.this_pane", "this pane"),
            Side::Right => tr!("app_compare.the_other", "the other"),
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
                    tr!("compare.u_replaces_an_older_copy", "\u{2192} {rel}  replaces an older copy", rel = rel)
                }
                State::RightNewer => {
                    overwrites += 1;
                    tr!("compare.u_replaces_an_older_copy_2", "\u{2190} {rel}  replaces an older copy", rel = rel)
                }
                State::Conflict => {
                    tr!("compare.u_same_time_different_size_u", "\u{2260} {rel}  same time, different size \u{2014} left alone", rel = rel)
                }
                State::CaseClash => tr!("compare.u_same_name_different_case_on", "\u{2260} {rel}{slash}  same name, different case on the other side \u{2014} left alone", rel = rel, slash = slash),
                State::Same => continue,
                State::Unreadable(s) => {
                    unknown += 1;
                    tr!("compare.u_cannot_be_read_in_u", "\u{26a0} {rel}/  cannot be read in {v} ({why}) \u{2014} nothing under it is copied", rel = rel, v = which(s), why = why)
                }
                State::Missing(s) => {
                    unknown += 1;
                    tr!("compare.u_went_away_in_while_compared", "\u{26a0} {rel}/  went away in {v} while compared \u{2014} left alone", rel = rel, v = which(s))
                }
                State::Incomplete(s) if d.error.is_some() => {
                    unknown += 1;
                    tr!("compare.u_not_all_of_it_could", "\u{26a0} {rel}/  not all of it could be read in {v} ({why})", rel = rel, v = which(s), why = why)
                }
                State::Incomplete(s) => tr!("compare.not_copied_could_not_be_read", "? {rel}{slash}  not copied: {v} could not be read in full here", rel = rel, slash = slash, v = which(s)),
            };
            body.push(line);
        }
        if pairs.is_empty() {
            self.status = if body.len() > 3 {
                tr!(
                    "compare.nothing_to_copy_u_only_differences",
                    "nothing to copy \u{2014} only differences that need a person"
                )
                .into()
            } else {
                tr!(
                    "compare.the_two_trees_already_agree",
                    "the two trees already agree"
                )
                .into()
            };
            if body.len() <= 3 {
                return;
            }
        }
        let n = pairs.len();
        let mut title = match overwrites {
            0 => tr!(
                "compare.synchronise_u_to_copy",
                "Synchronise \u{b7} {n} to copy",
                n = n
            ),
            o => tr!(
                "compare.synchronise_u_to_copy_replacing_older",
                "Synchronise \u{b7} {n} to copy, {o} replacing older files",
                n = n,
                o = o
            ),
        };
        if unknown > 0 {
            title.push_str(&tr!(
                "compare.u_u_not_read",
                " \u{b7} \u{26a0} {unknown} not read",
                unknown = unknown
            ));
        }
        self.modal = Some(Modal::Ask(modal::Ask {
            title,
            body,
            danger: overwrites > 0,
            yes: (n > 0).then(|| (format!("copy {n}"), Cmd::Sync(pairs))),
            buttons: Vec::new(),
            also: None,
            more: None,
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
