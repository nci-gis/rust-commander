use std::fs;
use std::io;
use std::path::Path;

use crate::entry::{Entry, Kind};

/// Read one directory into rows.
///
/// Individual entries that cannot be stat'ed become rows carrying an `error` rather than
/// aborting the listing: losing a whole directory because one file is unreadable is the
/// behaviour that makes a file manager feel broken on `/proc`, on network mounts, and on
/// anything being written to while you look at it.
pub fn read_dir(dir: &Path, show_hidden: bool) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();

    if let Some(parent) = dir.parent() {
        out.push(Entry {
            name: "..".to_string(),
            path: parent.to_path_buf(),
            kind: Kind::Parent,
            size: 0,
            modified: None,
            hidden: false,
            symlink: false,
            error: None,
        });
    }

    for dent in fs::read_dir(dir)? {
        let Ok(dent) = dent else { continue };

        let name = dent.file_name().to_string_lossy().into_owned();
        let path = dent.path();

        let link_meta = fs::symlink_metadata(&path);
        let hidden = is_hidden(&name, link_meta.as_ref().ok());
        if hidden && !show_hidden {
            continue;
        }
        out.push(build(path, name, link_meta, hidden));
    }

    Ok(out)
}

/// One row for `path`, which a listing found or a search turned up. `name` is what the
/// row shows: the file name in a listing, the path below the search root in results.
pub fn entry_at(path: &Path, name: String) -> Entry {
    let link_meta = fs::symlink_metadata(path);
    let hidden = is_hidden(&name, link_meta.as_ref().ok());
    build(path.to_path_buf(), name, link_meta, hidden)
}

fn build(
    path: std::path::PathBuf,
    name: String,
    link_meta: io::Result<fs::Metadata>,
    hidden: bool,
) -> Entry {
    let symlink = link_meta
        .as_ref()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    // Resolve through the link so a link to a directory behaves like one.
    //
    // A broken link must NOT fall back silently: `fs::metadata` fails but
    // `symlink_metadata` succeeds, so an `.or()` here would turn a dangling link into
    // an ordinary-looking file and hide the breakage from the user entirely.
    let (kind, size, modified, error) = if symlink {
        match fs::metadata(&path) {
            Ok(m) => {
                let kind = if m.is_dir() { Kind::Dir } else { Kind::File };
                let size = if m.is_dir() { 0 } else { m.len() };
                (kind, size, m.modified().ok(), None)
            }
            Err(e) => {
                // Dangling. Show the link's own size, the way `ls -l` does.
                let (size, modified) = link_meta
                    .as_ref()
                    .map(|m| (m.len(), m.modified().ok()))
                    .unwrap_or((0, None));
                (Kind::File, size, modified, Some(e.to_string()))
            }
        }
    } else {
        match link_meta {
            Ok(m) => {
                let kind = if m.is_dir() { Kind::Dir } else { Kind::File };
                let size = if m.is_dir() { 0 } else { m.len() };
                (kind, size, m.modified().ok(), None)
            }
            Err(e) => (Kind::File, 0, None, Some(e.to_string())),
        }
    };

    Entry {
        name,
        path,
        kind,
        size,
        modified,
        hidden,
        symlink,
        error,
    }
}

/// On Unix a leading dot is the whole story. On Windows the filesystem carries a hidden
/// attribute instead — but a developer there still wants `.git` out of the way, so both
/// count.
fn is_hidden(name: &str, meta: Option<&fs::Metadata>) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if let Some(m) = meta {
            if m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
                return true;
            }
        }
    }
    let _ = meta;
    name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::read_dir;
    use std::fs;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rcmd-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lists_entries_and_synthesises_parent() {
        let d = scratch("basic");
        fs::write(d.join("a.txt"), b"hello").unwrap();
        fs::create_dir(d.join("sub")).unwrap();

        let rows = read_dir(&d, false).unwrap();
        let names: Vec<_> = rows.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".."));
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"sub"));

        let f = rows.iter().find(|e| e.name == "a.txt").unwrap();
        assert_eq!(f.size, 5);
        assert!(!f.is_dir());
        assert!(rows.iter().find(|e| e.name == "sub").unwrap().is_dir());

        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn hidden_files_are_filtered_unless_asked_for() {
        let d = scratch("hidden");
        fs::write(d.join(".secret"), b"").unwrap();
        fs::write(d.join("plain"), b"").unwrap();

        let visible = read_dir(&d, false).unwrap();
        assert!(!visible.iter().any(|e| e.name == ".secret"));

        let all = read_dir(&d, true).unwrap();
        assert!(all.iter().any(|e| e.name == ".secret" && e.hidden));

        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_broken_symlink_is_a_row_not_an_error() {
        let d = scratch("broken-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(d.join("nowhere"), d.join("dangling")).unwrap();

        let rows = read_dir(&d, false).unwrap();
        #[cfg(unix)]
        {
            let e = rows.iter().find(|e| e.name == "dangling").unwrap();
            assert!(e.symlink);
            assert!(e.error.is_some(), "a dangling link should carry its error");
        }
        let _ = rows;

        fs::remove_dir_all(&d).unwrap();
    }
}
