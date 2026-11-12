//! Comparing two directories (Alt+K) and planning to make them agree (Alt+Y).
//!
//! Files are compared by size and modification time, with two seconds' slack: FAT
//! and many network shares keep times to two seconds, and a copy onto one would
//! otherwise always look "newer". Content is not read — a compare of two large trees
//! must take seconds, not hours. Same time but a different size is reported as a
//! conflict and never synchronised automatically: there is no telling which is right.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

/// One side of a compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    LeftOnly,
    RightOnly,
    LeftNewer,
    RightNewer,
    /// Same time, different size.
    Conflict,
    Same,
    /// A directory on this side could not be read ([`Diff::error`] says why): the whole
    /// subtree is this one row, and nothing under it is planned on either side, since
    /// "only on the other side" means nothing where one side is unknown.
    Unreadable(Side),
    /// A directory on this side went away while it was compared.
    Missing(Side),
    /// Some entries of a directory on this side could not be read or looked at: a name
    /// only on the other side, there, may have its twin among them, so it is not planned.
    /// The directory's own row carries the first error; each such name is a row too.
    Incomplete(Side),
}

impl State {
    /// Something could not be decided: shown, never planned.
    pub fn unknown(self) -> bool {
        matches!(
            self,
            State::Unreadable(_) | State::Missing(_) | State::Incomplete(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    /// Below both roots, as the file system names it (any bytes, not only UTF-8): the
    /// text shown is made where it is drawn.
    pub rel: PathBuf,
    pub is_dir: bool,
    pub state: State,
    /// Why, for [`State::Unreadable`], [`State::Missing`] and an incomplete directory.
    pub error: Option<String>,
}

const SLACK_SECS: u64 = 2;

/// Whether `a` was modified later than `b`, beyond the two seconds a FAT drive can
/// round a time by: `Greater` when `a` is newer, `Equal` when they count as the same.
pub(crate) fn newer_than(a: &fs::Metadata, b: &fs::Metadata) -> std::cmp::Ordering {
    newer(stamp(a), stamp(b))
}

fn stamp(m: &fs::Metadata) -> Option<SystemTime> {
    m.modified().ok()
}

fn newer(a: Option<SystemTime>, b: Option<SystemTime>) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        (Some(a), Some(b)) => {
            let d = match a.duration_since(b) {
                Ok(d) => (d.as_secs(), Greater),
                Err(e) => (e.duration().as_secs(), Less),
            };
            if d.0 <= SLACK_SECS {
                Equal
            } else {
                d.1
            }
        }
        _ => Equal,
    }
}

/// One directory's entries, sorted by their names' bytes, links not followed.
struct Listing {
    entries: Vec<(OsString, fs::Metadata)>,
    /// The first entry that could not be read or looked at, if any did.
    incomplete: Option<String>,
}

/// List `dir`; an error when it cannot be read at all.
fn entries(dir: &Path, hidden: bool) -> io::Result<Listing> {
    crate::faults::at(crate::faults::Point::ReadDir, dir)?;
    let mut out = Listing {
        entries: Vec::new(),
        incomplete: None,
    };
    for e in fs::read_dir(dir)? {
        let e = match e {
            Ok(e) => e,
            Err(err) => {
                out.incomplete.get_or_insert_with(|| err.to_string());
                continue;
            }
        };
        let name = e.file_name();
        if !hidden && name.as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }
        // Links are compared as what they are, never followed.
        let path = e.path();
        match crate::faults::at(crate::faults::Point::Stat, &path)
            .and_then(|()| fs::symlink_metadata(&path))
        {
            Ok(m) => out.entries.push((name, m)),
            // Gone since it was listed: not there, which is no doubt about it.
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                out.incomplete
                    .get_or_insert_with(|| format!("{}: {err}", path.display()));
            }
        }
    }
    out.entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Compare `left` and `right`. With `deep`, directories present on both sides are
/// compared inside too; a directory on one side only is one entry, not its contents.
/// An error when either root cannot be read: that is no plan at all.
pub fn compare(left: &Path, right: &Path, deep: bool, hidden: bool) -> io::Result<Vec<Diff>> {
    compare_until(
        left,
        right,
        deep,
        hidden,
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap_or_else(|| Ok(Vec::new()))
}

/// [`compare`] that can be stopped, counting the entries looked at into `seen` as it
/// goes: for a deep compare on a thread of its own, which a big tree or a network
/// drive can keep busy for a long time. `None` when stopped.
pub fn compare_until(
    left: &Path,
    right: &Path,
    deep: bool,
    hidden: bool,
    stop: &AtomicBool,
    seen: &AtomicU64,
) -> Option<io::Result<Vec<Diff>>> {
    let w = Walk {
        deep,
        hidden,
        stop,
        seen,
    };
    let root = |p: &Path| {
        entries(p, hidden).map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", p.display())))
    };
    let (le, re) = match (root(left), root(right)) {
        (Ok(l), Ok(r)) => (l, r),
        (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
    };
    let mut out = Vec::new();
    w.merge(left, right, Path::new(""), le, re, &mut out);
    (!stop.load(Ordering::Relaxed)).then_some(Ok(out))
}

struct Walk<'a> {
    deep: bool,
    hidden: bool,
    stop: &'a AtomicBool,
    seen: &'a AtomicU64,
}

impl Walk<'_> {
    /// Compare the directories `l` and `r`, both at `rel` below the roots.
    fn walk(&self, l: &Path, r: &Path, rel: &Path, out: &mut Vec<Diff>) {
        if self.stop.load(Ordering::Relaxed) {
            return;
        }
        let side = |p: &Path, s: Side| {
            entries(p, self.hidden).map_err(|e| {
                let state = if e.kind() == io::ErrorKind::NotFound {
                    State::Missing(s)
                } else {
                    State::Unreadable(s)
                };
                Diff {
                    rel: rel.to_path_buf(),
                    is_dir: true,
                    state,
                    error: Some(e.to_string()),
                }
            })
        };
        match (side(l, Side::Left), side(r, Side::Right)) {
            (Ok(le), Ok(re)) => self.merge(l, r, rel, le, re, out),
            // One row for the whole subtree: neither side's entries under it are planned.
            (Err(d), _) | (_, Err(d)) => out.push(d),
        }
    }

    fn merge(
        &self,
        l: &Path,
        r: &Path,
        prefix: &Path,
        le: Listing,
        re: Listing,
        out: &mut Vec<Diff>,
    ) {
        let deep = self.deep;
        self.seen.fetch_add(
            (le.entries.len() + re.entries.len()) as u64,
            Ordering::Relaxed,
        );
        for (s, why) in [(Side::Left, &le.incomplete), (Side::Right, &re.incomplete)] {
            if let Some(why) = why {
                out.push(Diff {
                    rel: prefix.to_path_buf(),
                    is_dir: true,
                    state: State::Incomplete(s),
                    error: Some(why.clone()),
                });
            }
        }
        // A name only on one side, where the other could not be read in full: unknown.
        let (left_whole, right_whole) = (le.incomplete.is_none(), re.incomplete.is_none());
        let (le, re) = (le.entries, re.entries);
        let rel = |n: &OsString| prefix.join(n);
        let row = |rel: PathBuf, is_dir: bool, state: State| Diff {
            rel,
            is_dir,
            state,
            error: None,
        };
        let (mut i, mut j) = (0, 0);
        while i < le.len() || j < re.len() {
            let order = match (le.get(i), re.get(j)) {
                (Some(a), Some(b)) => a.0.cmp(&b.0),
                (Some(_), None) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Greater,
            };
            match order {
                std::cmp::Ordering::Less => {
                    let (n, m) = &le[i];
                    let state = if right_whole {
                        State::LeftOnly
                    } else {
                        State::Incomplete(Side::Right)
                    };
                    out.push(row(rel(n), m.is_dir(), state));
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    let (n, m) = &re[j];
                    let state = if left_whole {
                        State::RightOnly
                    } else {
                        State::Incomplete(Side::Left)
                    };
                    out.push(row(rel(n), m.is_dir(), state));
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    let (n, a) = &le[i];
                    let b = &re[j].1;
                    if a.is_dir() && b.is_dir() {
                        if deep {
                            self.walk(&l.join(n), &r.join(n), &rel(n), out);
                        } else {
                            out.push(row(rel(n), true, State::Same));
                        }
                    } else if a.is_dir() != b.is_dir() {
                        out.push(row(rel(n), false, State::Conflict));
                    } else {
                        let state = match newer(stamp(a), stamp(b)) {
                            std::cmp::Ordering::Greater => State::LeftNewer,
                            std::cmp::Ordering::Less => State::RightNewer,
                            std::cmp::Ordering::Equal if a.len() != b.len() => State::Conflict,
                            std::cmp::Ordering::Equal => State::Same,
                        };
                        out.push(row(rel(n), false, state));
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
    }
}

/// What synchronising both ways copies: (source, directory to copy it into). Nothing
/// that could not be decided ([`State::unknown`]) is ever in it.
pub fn plan(left: &Path, right: &Path, diffs: &[Diff]) -> Vec<(PathBuf, PathBuf)> {
    let parent = |root: &Path, rel: &Path| match rel.parent() {
        Some(p) => root.join(p),
        None => root.to_path_buf(),
    };
    diffs
        .iter()
        .filter_map(|d| match d.state {
            State::LeftOnly | State::LeftNewer => Some((left.join(&d.rel), parent(right, &d.rel))),
            State::RightOnly | State::RightNewer => {
                Some((right.join(&d.rel), parent(left, &d.rel)))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn touch(p: &Path, body: &str, age_secs: u64) {
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).unwrap();
        }
        fs::write(p, body).unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_secs);
        let f = fs::File::options().write(true).open(p).unwrap();
        f.set_times(fs::FileTimes::new().set_modified(t)).unwrap();
    }

    #[test]
    fn every_kind_of_difference_and_the_plan_for_it() {
        let d = std::env::temp_dir().join(format!("xdevcmd-cmp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let (l, r) = (d.join("l"), d.join("r"));
        touch(&l.join("only-left"), "x", 100);
        touch(&r.join("only-right"), "x", 100);
        touch(&l.join("newer"), "new", 10);
        touch(&r.join("newer"), "old", 1000);
        touch(&l.join("same"), "s", 500);
        touch(&r.join("same"), "s", 501);
        touch(&l.join("clash"), "aaa", 500);
        touch(&r.join("clash"), "b", 500);
        touch(&l.join("sub/deep"), "d", 5);
        touch(&r.join("sub/deep"), "d", 900);
        fs::create_dir_all(r.join("rdir/inner")).unwrap();

        let got = compare(&l, &r, true, false).unwrap();
        let state = |n: &str| got.iter().find(|d| d.rel == Path::new(n)).map(|d| d.state);
        assert_eq!(state("only-left"), Some(State::LeftOnly));
        assert_eq!(state("only-right"), Some(State::RightOnly));
        assert_eq!(state("newer"), Some(State::LeftNewer));
        assert_eq!(
            state("same"),
            Some(State::Same),
            "one second apart is the same"
        );
        assert_eq!(state("clash"), Some(State::Conflict));
        assert_eq!(
            state("sub/deep"),
            Some(State::LeftNewer),
            "deep compares inside"
        );
        assert_eq!(
            state("rdir"),
            Some(State::RightOnly),
            "one entry, not its contents"
        );
        assert_eq!(state("rdir/inner"), None);

        let p = plan(&l, &r, &got);
        assert!(p.contains(&(l.join("newer"), r.clone())));
        assert!(p.contains(&(l.join("sub/deep"), r.join("sub"))));
        assert!(p.contains(&(r.join("rdir"), l.clone())));
        assert!(
            !p.iter().any(|(s, _)| s.ends_with("clash")),
            "a conflict is never copied"
        );
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn a_deep_compare_can_be_stopped_and_counts_as_it_goes() {
        use std::sync::atomic::{AtomicBool, AtomicU64};
        let d = std::env::temp_dir().join(format!("xdevcmd-cmp-stop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        for side in ["l", "r"] {
            fs::create_dir_all(d.join(side).join("sub")).unwrap();
            fs::write(d.join(side).join("sub/f"), side).unwrap();
        }
        let seen = AtomicU64::new(0);
        let out = compare_until(
            &d.join("l"),
            &d.join("r"),
            true,
            false,
            &AtomicBool::new(false),
            &seen,
        );
        assert!(matches!(out, Some(Ok(_))));
        assert!(
            seen.load(Ordering::Relaxed) >= 4,
            "both levels of both sides counted"
        );
        let stopped = compare_until(
            &d.join("l"),
            &d.join("r"),
            true,
            false,
            &AtomicBool::new(true),
            &seen,
        );
        assert!(stopped.is_none());
        let _ = fs::remove_dir_all(&d);
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-cmp-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("l")).unwrap();
        fs::create_dir_all(d.join("r")).unwrap();
        d
    }

    /// A deep compare of `d/l` and `d/r`.
    fn diffs(d: &Path) -> Vec<Diff> {
        compare(&d.join("l"), &d.join("r"), true, false).unwrap()
    }

    /// A directory on the right that cannot be read: today's plan copied everything
    /// on the left into it, as if it were empty. Now the subtree is one row, and
    /// nothing under it is planned on either side. The seam's ReadDir runs everywhere;
    /// the real `chmod 000` below needs a user other than root.
    #[test]
    fn a_directory_that_cannot_be_read_is_never_planned_into() {
        let d = scratch("unreadable");
        touch(&d.join("l/sub/a"), "a", 100);
        touch(&d.join("l/sub/b"), "b", 100);
        touch(&d.join("r/sub/b"), "b", 100);
        touch(&d.join("l/ok"), "x", 100);
        let _f = crate::faults::fail(
            crate::faults::Point::ReadDir,
            &d.join("r").join("sub").to_string_lossy(),
            1,
            crate::faults::Fault::Os(13),
        );
        let got = diffs(&d);
        let p = plan(&d.join("l"), &d.join("r"), &got);
        assert!(
            !p.iter().any(|(s, _)| s.starts_with(d.join("l/sub"))),
            "nothing under it is planned: {p:?}"
        );
        assert!(
            p.contains(&(d.join("l/ok"), d.join("r"))),
            "the rest still is: {p:?}"
        );
        assert!(
            got.iter().any(|x| Path::new(&x.rel) == Path::new("sub")
                && format!("{:?}", x.state).contains("Unreadable")),
            "{got:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_without_read_permission_is_never_planned_into() {
        use std::os::unix::fs::PermissionsExt;
        if rustix::process::geteuid().is_root() {
            eprintln!("skipped: root reads a directory whatever its mode");
            return;
        }
        let d = scratch("chmod");
        touch(&d.join("l/sub/a"), "a", 100);
        fs::create_dir_all(d.join("r/sub")).unwrap();
        fs::set_permissions(d.join("r/sub"), fs::Permissions::from_mode(0o000)).unwrap();
        let got = diffs(&d);
        fs::set_permissions(d.join("r/sub"), fs::Permissions::from_mode(0o755)).unwrap();
        let p = plan(&d.join("l"), &d.join("r"), &got);
        assert!(p.is_empty(), "{p:?}");
        assert!(
            got.iter()
                .any(|x| x.state == State::Unreadable(Side::Right)),
            "{got:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    /// Two names that are not UTF-8 and read the same once made lossy: two rows, and
    /// each planned source is a file that exists. Before, both became "a\u{fffd}" and
    /// the plan named a source that was not there.
    #[cfg(target_os = "linux")]
    #[test]
    fn names_that_are_not_utf8_are_compared_and_planned_as_they_are() {
        use std::os::unix::ffi::OsStrExt;
        let d = scratch("bytes");
        let (a, b) = (
            std::ffi::OsStr::from_bytes(b"a\xff"),
            std::ffi::OsStr::from_bytes(b"a\xfe"),
        );
        fs::write(d.join("l").join(a), "ff").unwrap();
        fs::write(d.join("l").join(b), "fe").unwrap();
        let got = diffs(&d);
        assert_eq!(got.len(), 2, "{got:?}");
        let p = plan(&d.join("l"), &d.join("r"), &got);
        assert_eq!(p.len(), 2);
        for (src, _) in &p {
            assert!(
                src.exists(),
                "{} is planned and does not exist",
                src.display()
            );
        }
        let _ = fs::remove_dir_all(&d);
    }

    /// An entry on the left that cannot be looked at (a stat fails) while its twin on
    /// the right can: the right one is not "only on the right", and is not planned to
    /// be copied over it. Before, the left entry was dropped without a word and the
    /// right one copied onto it.
    #[test]
    fn an_entry_that_cannot_be_looked_at_leaves_its_directory_unplanned() {
        let d = scratch("stat");
        touch(&d.join("l/x"), "left's", 100);
        touch(&d.join("r/x"), "right's", 100);
        touch(&d.join("r/y"), "y", 100);
        let _f = crate::faults::fail(
            crate::faults::Point::Stat,
            &d.join("l").join("x").to_string_lossy(),
            1,
            crate::faults::Fault::Os(5),
        );
        let got = diffs(&d);
        let p = plan(&d.join("l"), &d.join("r"), &got);
        assert!(
            p.is_empty(),
            "nothing one-sided is planned where the left is unknown: {p:?}"
        );
        assert!(
            got.iter()
                .any(|x| format!("{:?}", x.state).contains("Incomplete")),
            "{got:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_root_that_cannot_be_read_is_an_error_not_a_plan() {
        let d = scratch("root");
        let e = compare(&d.join("l"), &d.join("gone"), true, false).unwrap_err();
        assert!(e.to_string().contains("gone"), "{e}");
        let _ = fs::remove_dir_all(&d);
    }
}
