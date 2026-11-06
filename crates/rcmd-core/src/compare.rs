//! Comparing two directories (Alt+K) and planning to make them agree (Alt+Y).
//!
//! Files are compared by size and modification time, with two seconds' slack: FAT
//! and many network shares keep times to two seconds, and a copy onto one would
//! otherwise always look "newer". Content is not read — a compare of two large trees
//! must take seconds, not hours. Same time but a different size is reported as a
//! conflict and never synchronised automatically: there is no telling which is right.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    LeftOnly,
    RightOnly,
    LeftNewer,
    RightNewer,
    /// Same time, different size.
    Conflict,
    Same,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    /// Below both roots, `/`-separated.
    pub rel: String,
    pub is_dir: bool,
    pub state: State,
}

const SLACK_SECS: u64 = 2;

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

fn entries(dir: &Path, hidden: bool) -> Vec<(String, fs::Metadata)> {
    let mut v: Vec<(String, fs::Metadata)> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if !hidden && name.starts_with('.') {
                        return None;
                    }
                    // Links are compared as what they are, never followed.
                    fs::symlink_metadata(e.path()).ok().map(|m| (name, m))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Compare `left` and `right`. With `deep`, directories present on both sides are
/// compared inside too; a directory on one side only is one entry, not its contents.
pub fn compare(left: &Path, right: &Path, deep: bool, hidden: bool) -> Vec<Diff> {
    let mut out = Vec::new();
    walk(left, right, "", deep, hidden, &mut out);
    out
}

fn walk(l: &Path, r: &Path, prefix: &str, deep: bool, hidden: bool, out: &mut Vec<Diff>) {
    let le = entries(l, hidden);
    let re = entries(r, hidden);
    let rel = |n: &str| {
        if prefix.is_empty() {
            n.to_string()
        } else {
            format!("{prefix}/{n}")
        }
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
                out.push(Diff {
                    rel: rel(n),
                    is_dir: m.is_dir(),
                    state: State::LeftOnly,
                });
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                let (n, m) = &re[j];
                out.push(Diff {
                    rel: rel(n),
                    is_dir: m.is_dir(),
                    state: State::RightOnly,
                });
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                let (n, a) = &le[i];
                let b = &re[j].1;
                if a.is_dir() && b.is_dir() {
                    if deep {
                        walk(&l.join(n), &r.join(n), &rel(n), deep, hidden, out);
                    } else {
                        out.push(Diff {
                            rel: rel(n),
                            is_dir: true,
                            state: State::Same,
                        });
                    }
                } else if a.is_dir() != b.is_dir() {
                    out.push(Diff {
                        rel: rel(n),
                        is_dir: false,
                        state: State::Conflict,
                    });
                } else {
                    let state = match newer(stamp(a), stamp(b)) {
                        std::cmp::Ordering::Greater => State::LeftNewer,
                        std::cmp::Ordering::Less => State::RightNewer,
                        std::cmp::Ordering::Equal if a.len() != b.len() => State::Conflict,
                        std::cmp::Ordering::Equal => State::Same,
                    };
                    out.push(Diff {
                        rel: rel(n),
                        is_dir: false,
                        state,
                    });
                }
                i += 1;
                j += 1;
            }
        }
    }
}

/// What synchronising both ways copies: (source, directory to copy it into).
pub fn plan(left: &Path, right: &Path, diffs: &[Diff]) -> Vec<(PathBuf, PathBuf)> {
    let parent = |root: &Path, rel: &str| match rel.rsplit_once('/') {
        Some((p, _)) => root.join(p),
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
        let d = std::env::temp_dir().join(format!("rcmd-cmp-{}", std::process::id()));
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

        let got = compare(&l, &r, true, false);
        let state = |n: &str| got.iter().find(|d| d.rel == n).map(|d| d.state);
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
}
