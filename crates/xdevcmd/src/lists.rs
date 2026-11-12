//! Lists of paths kept for the session: what Alt+F7 found, and what was taken out of
//! a shell's output (`p` in its history). A list is not lost by leaving it: Back
//! returns to it, Alt+L shows the active one, and Alt+Down/Alt+J lists them all.
//!
//! In memory only, as a shell's history is: a list is made of what a shell printed,
//! which can name private files, and nothing of it is written to disk.
//!
//! App-wide, not a tab's: a file pane's Back can step into a list made in another
//! pane, so the lists live here, by id, and a pane holds only the id.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Lists kept; the one used least recently goes first.
pub const KEPT: usize = 10;
/// Rows in one list; past this the rest are left out, and the title says so.
pub const ROWS: usize = 10_000;

pub type Id = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub id: Id,
    /// Where it came from: "tab 1 · rg -l foo", "find *.rs".
    pub source: String,
    /// The directory names are shown relative to.
    pub root: PathBuf,
    pub rows: Vec<PathBuf>,
    /// The row the cursor was on when the list was left.
    pub chosen: Option<PathBuf>,
    /// Rows left out: past [`ROWS`], and outside the root xdevcmd is confined to.
    pub capped: bool,
    pub outside: usize,
    /// Rows not checked yet: names taken from a shell's output, which may never have
    /// been files. The first check drops those quietly; only a row that was there and
    /// went is named.
    pub fresh: std::collections::HashSet<PathBuf>,
    /// Lines' candidates for the first check to choose between ([`find::Group`]); gone
    /// once it has.
    pub groups: Vec<xdevcmd_core::find::Group>,
}

#[derive(Default)]
struct Store {
    next: Id,
    /// Most recently used first: the front is the active list.
    lists: VecDeque<List>,
}

static STORE: Mutex<Store> = Mutex::new(Store {
    next: 0,
    lists: VecDeque::new(),
});

fn store() -> MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Rows as a list keeps them: each path once, in order, inside the root, at most
/// [`ROWS`]. Returns them, whether any were cut, and how many were outside the root.
pub fn tidy_rows(paths: impl IntoIterator<Item = PathBuf>) -> (Vec<PathBuf>, bool, usize) {
    let policy = xdevcmd_core::policy::get();
    let mut seen = std::collections::HashSet::new();
    let (mut rows, mut capped, mut outside) = (Vec::new(), false, 0);
    for p in paths {
        if !policy.allows(&p) {
            outside += 1;
            continue;
        }
        if !seen.insert(p.clone()) {
            continue;
        }
        if rows.len() == ROWS {
            capped = true;
            break;
        }
        rows.push(p);
    }
    (rows, capped, outside)
}

/// Keep a new list; it becomes the active one. The least recently used goes past
/// [`KEPT`].
/// `checked`: the rows are known to be there (a search found them); otherwise the
/// first check drops the ones that are not, quietly.
pub fn keep(
    source: String,
    root: PathBuf,
    rows: Vec<PathBuf>,
    capped: bool,
    outside: usize,
    checked: bool,
    groups: Vec<xdevcmd_core::find::Group>,
) -> Id {
    let mut s = store();
    s.next += 1;
    let id = s.next;
    let fresh = if checked {
        Default::default()
    } else {
        rows.iter().cloned().collect()
    };
    s.lists.push_front(List {
        id,
        source,
        root,
        rows,
        chosen: None,
        capped,
        outside,
        fresh,
        groups,
    });
    s.lists.truncate(KEPT);
    id
}

/// List `id`, if it is still kept.
pub fn get(id: Id) -> Option<List> {
    store().lists.iter().find(|l| l.id == id).cloned()
}

/// Make `id` the active list. False if it is no longer kept.
pub fn touch(id: Id) -> bool {
    let mut s = store();
    let Some(i) = s.lists.iter().position(|l| l.id == id) else {
        return false;
    };
    if let Some(l) = s.lists.remove(i) {
        s.lists.push_front(l);
    }
    true
}

/// The active list.
pub fn active() -> Option<List> {
    store().lists.front().cloned()
}

/// Every list kept, the active one first.
pub fn all() -> Vec<List> {
    store().lists.iter().cloned().collect()
}

/// Remember the row the cursor was on, for the way back.
pub fn set_chosen(id: Id, row: Option<PathBuf>) {
    if let Some(l) = store().lists.iter_mut().find(|l| l.id == id) {
        l.chosen = row;
    }
}

/// A check of list `id` ended: drop the rows found gone, and say which of them had
/// been there (the rest were never files). Every row is checked from now on.
pub fn checked(id: Id, gone: &[PathBuf]) -> Vec<PathBuf> {
    let mut s = store();
    let Some(l) = s.lists.iter_mut().find(|l| l.id == id) else {
        return Vec::new();
    };
    let gone: std::collections::HashSet<&PathBuf> = gone.iter().collect();
    l.rows.retain(|r| !gone.contains(r));
    let went = gone
        .into_iter()
        .filter(|p| !l.fresh.contains(*p))
        .cloned()
        .collect();
    l.fresh.clear();
    l.groups.clear();
    went
}

/// Add `paths` to list `id`: rows already there stay where they are, the rest go at
/// the end, up to [`ROWS`]. `source` is added to the list's own. Returns how many
/// were added and how many it already had; `None` if the list is no longer kept.
///
/// `groups`: the new lines' candidates. Only rows added here are decided between: a row
/// the list already had stays whatever a new line says.
pub fn add(
    id: Id,
    paths: Vec<PathBuf>,
    groups: Vec<xdevcmd_core::find::Group>,
    source: &str,
) -> Option<(usize, usize)> {
    let (paths, capped, outside) = tidy_rows(paths);
    let mut s = store();
    let l = s.lists.iter_mut().find(|l| l.id == id)?;
    // A line's whole is a row to decide when the list is checked (most lines name
    // nothing whole: a prompt, a message), not a path to count yet.
    let choices: std::collections::HashSet<PathBuf> =
        groups.iter().flat_map(|(w, _)| w.iter().cloned()).collect();
    let (mut added, mut had) = (0, 0);
    for p in paths {
        let counted = !choices.contains(&p);
        if l.rows.contains(&p) {
            had += usize::from(counted);
        } else if l.rows.len() < ROWS {
            l.fresh.insert(p.clone());
            l.rows.push(p);
            added += usize::from(counted);
        } else {
            l.capped = true;
        }
    }
    for (wholes, parts) in groups {
        let new = |v: Vec<PathBuf>| -> Vec<PathBuf> {
            v.into_iter().filter(|p| l.fresh.contains(p)).collect()
        };
        let (wholes, parts) = (new(wholes), new(parts));
        if !wholes.is_empty() {
            l.groups.push((wholes, parts));
        }
    }
    l.capped |= capped;
    l.outside += outside;
    if !l.source.contains(source) {
        l.source = format!("{} + {source}", l.source);
    }
    drop(s);
    touch(id);
    Some((added, had))
}

/// The directory to show a list's names relative to: the deepest one holding them all.
pub fn common_root(rows: &[PathBuf], fallback: &Path) -> PathBuf {
    let mut it = rows.iter();
    let Some(first) = it.next() else {
        return fallback.to_path_buf();
    };
    let mut root = first.parent().map(Path::to_path_buf).unwrap_or_default();
    for r in it {
        while !r.starts_with(&root) {
            match root.parent() {
                Some(p) => root = p.to_path_buf(),
                None => return fallback.to_path_buf(),
            }
        }
    }
    if root.as_os_str().is_empty() {
        fallback.to_path_buf()
    } else {
        root
    }
}

/// "a.rs, b.rs and 3 more": the first few names of `paths`, for the status line.
pub fn some_names(paths: &[PathBuf]) -> String {
    const SHOWN: usize = 3;
    let names: Vec<String> = paths
        .iter()
        .take(SHOWN)
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    match paths.len().saturating_sub(SHOWN) {
        0 => names.join(", "),
        more => format!("{} and {more} more", names.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The store is one for the process, and tests run at once: each test works only
    // with the ids it made.

    #[test]
    fn a_list_is_kept_and_the_one_used_last_is_active() {
        let a = keep(
            "a".into(),
            "/".into(),
            vec!["/x".into()],
            false,
            0,
            true,
            Vec::new(),
        );
        let b = keep(
            "b".into(),
            "/".into(),
            vec!["/y".into(), "/z".into()],
            false,
            0,
            false,
            Vec::new(),
        );
        assert!(touch(a));
        let order: Vec<Id> = all()
            .into_iter()
            .map(|l| l.id)
            .filter(|i| [a, b].contains(i))
            .collect();
        assert_eq!(order, [a, b], "a, used last, comes first");
        set_chosen(a, Some("/x".into()));
        assert_eq!(get(a).unwrap().chosen, Some(PathBuf::from("/x")));
        // b's rows were never checked: the first check drops /y quietly.
        assert!(checked(b, &["/y".into()]).is_empty());
        assert_eq!(get(b).unwrap().rows, [PathBuf::from("/z")]);
        // From then on, a row that goes is named.
        assert_eq!(checked(b, &["/z".into()]), [PathBuf::from("/z")]);
        assert!(get(b).unwrap().rows.is_empty());
    }

    #[test]
    fn adding_keeps_the_old_rows_and_names_both_sources() {
        let a = keep(
            "tab 1 · rg".into(),
            "/".into(),
            vec!["/p".into(), "/q".into()],
            false,
            0,
            true,
            Vec::new(),
        );
        let got = add(
            a,
            vec!["/q".into(), "/r".into(), "/r".into(), "/w".into()],
            vec![(vec!["/w".into()], vec!["/q".into(), "/r".into()])],
            "tab 2 · find",
        );
        assert_eq!(
            got,
            Some((1, 1)),
            "r added once; q was there; w is a line's whole, decided later"
        );
        let l = get(a).unwrap();
        assert_eq!(
            l.rows,
            [PathBuf::from("/p"), "/q".into(), "/r".into(), "/w".into()]
        );
        assert_eq!(l.source, "tab 1 · rg + tab 2 · find");
        assert!(
            l.fresh.contains(&PathBuf::from("/r")),
            "added rows are checked quietly first"
        );
        assert_eq!(
            l.groups,
            [(vec![PathBuf::from("/w")], vec![PathBuf::from("/r")])],
            "/q was there before: a new line's whole does not decide it"
        );
        assert_eq!(add(0, vec![], vec![], "x"), None, "no such list");
    }

    #[test]
    fn rows_are_each_path_once_and_capped() {
        let many: Vec<PathBuf> = (0..ROWS + 5)
            .map(|i| PathBuf::from(format!("/f{i}")))
            .collect();
        let (rows, capped, outside) = tidy_rows(many.into_iter().chain([PathBuf::from("/f0")]));
        assert_eq!((rows.len(), capped, outside), (ROWS, true, 0));
        let (rows, capped, _) = tidy_rows(vec!["/a".into(), "/a".into()]);
        assert_eq!((rows.len(), capped), (1, false));
    }

    #[test]
    fn names_show_relative_to_the_deepest_common_directory() {
        let rows: Vec<PathBuf> = vec!["/s/p/src/a.rs".into(), "/s/p/docs/b.md".into()];
        assert_eq!(common_root(&rows, Path::new("/f")), PathBuf::from("/s/p"));
        assert_eq!(common_root(&[], Path::new("/f")), PathBuf::from("/f"));
        let some: Vec<PathBuf> = (0..5)
            .map(|i| PathBuf::from(format!("/d/{i}.rs")))
            .collect();
        assert_eq!(some_names(&some), "0.rs, 1.rs, 2.rs and 2 more");
    }
}
