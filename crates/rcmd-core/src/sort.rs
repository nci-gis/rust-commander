use std::cmp::Ordering;

use crate::entry::{Entry, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Ext,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy)]
pub struct Sort {
    pub key: SortKey,
    pub reverse: bool,
    /// Directories above files regardless of the key. The orthodox default, and the one
    /// people complain loudest about when it is missing.
    pub dirs_first: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            reverse: false,
            dirs_first: true,
        }
    }
}

pub fn sort_entries(entries: &mut [Entry], s: Sort) {
    entries.sort_by(|a, b| {
        // `..` is never sorted; it is the way out of the directory and lives at the top.
        match (a.kind == Kind::Parent, b.kind == Kind::Parent) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }

        if s.dirs_first {
            match (a.is_dir(), b.is_dir()) {
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {}
            }
        }

        let ord = match s.key {
            SortKey::Name => natural_cmp(&a.name, &b.name),
            SortKey::Ext => {
                let (_, ae) = a.stem_ext();
                let (_, be) = b.stem_ext();
                natural_cmp(ae, be).then_with(|| natural_cmp(&a.name, &b.name))
            }
            SortKey::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| natural_cmp(&a.name, &b.name)),
            SortKey::Modified => a
                .modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name, &b.name)),
        };

        if s.reverse {
            ord.reverse()
        } else {
            ord
        }
    });
}

/// Compare names the way a person reads them: `file2` before `file10`.
///
/// Plain lexicographic ordering puts `file10` first, which is wrong often enough to be
/// worth the forty lines. Case is ignored for ordering, then used as a tiebreak so the
/// result is deterministic rather than dependent on directory order.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();

    loop {
        let (Some(&x), Some(&y)) = (ai.peek(), bi.peek()) else {
            return match (ai.peek(), bi.peek()) {
                (None, None) => a.cmp(b),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                _ => unreachable!(),
            };
        };

        if x.is_ascii_digit() && y.is_ascii_digit() {
            let an = take_digits(&mut ai);
            let bn = take_digits(&mut bi);
            let at = an.trim_start_matches('0');
            let bt = bn.trim_start_matches('0');
            // Longer digit run (ignoring leading zeros) means a larger number.
            let ord = at.len().cmp(&bt.len()).then_with(|| at.cmp(bt));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            let xl = x.to_ascii_lowercase();
            let yl = y.to_ascii_lowercase();
            if xl != yl {
                return xl.cmp(&yl);
            }
            ai.next();
            bi.next();
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&c) = it.peek() {
        if c.is_ascii_digit() {
            s.push(c);
            it.next();
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn e(name: &str, kind: Kind, size: u64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
            size,
            modified: None,
            hidden: false,
            symlink: false,
            error: None,
        }
    }

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn numbers_sort_as_numbers() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("file10", "file9"), Ordering::Greater);
        assert_eq!(natural_cmp("a1b2", "a1b10"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value() {
        // 007 and 8 compare as 7 and 8, not as text where "0" < "8" by accident.
        assert_eq!(natural_cmp("v007", "v8"), Ordering::Less);
        assert_eq!(natural_cmp("v008", "v7"), Ordering::Greater);
        // Equal in value, so the raw string breaks the tie and the order is stable
        // rather than dependent on the order the directory happened to be read in.
        assert_eq!(natural_cmp("v007", "v7"), Ordering::Less);
        assert_eq!(natural_cmp("v7", "v007"), Ordering::Greater);
    }

    #[test]
    fn case_does_not_drive_the_ordering() {
        // "alpha" sorts next to "Alpha", not away in a separate uppercase block.
        assert_eq!(natural_cmp("alpha", "Beta"), Ordering::Less);
        assert_eq!(natural_cmp("Beta", "alpha"), Ordering::Greater);
        // Same word, different case: ordered by the raw bytes so it is deterministic.
        assert_eq!(natural_cmp("Alpha", "alpha"), Ordering::Less);
    }

    #[test]
    fn parent_is_first_and_dirs_lead_files() {
        let mut v = vec![
            e("zeta.txt", Kind::File, 1),
            e("alpha", Kind::Dir, 0),
            e("..", Kind::Parent, 0),
            e("beta.txt", Kind::File, 2),
            e("omega", Kind::Dir, 0),
        ];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["..", "alpha", "omega", "beta.txt", "zeta.txt"]);
    }

    #[test]
    fn reverse_leaves_parent_and_dir_grouping_alone() {
        let mut v = vec![
            e("..", Kind::Parent, 0),
            e("a.txt", Kind::File, 1),
            e("b.txt", Kind::File, 2),
            e("dir", Kind::Dir, 0),
        ];
        sort_entries(
            &mut v,
            Sort {
                reverse: true,
                ..Sort::default()
            },
        );
        assert_eq!(names(&v), ["..", "dir", "b.txt", "a.txt"]);
    }

    #[test]
    fn file10_regression() {
        let mut v = vec![
            e("file10", Kind::File, 0),
            e("file9", Kind::File, 0),
            e("file1", Kind::File, 0),
        ];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["file1", "file9", "file10"]);
    }
}
