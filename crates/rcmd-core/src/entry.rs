use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// The synthetic `..` row. Always sorts first, never marked, never deleted.
    Parent,
    Dir,
    File,
}

impl Kind {
    pub fn is_dir(self) -> bool {
        matches!(self, Kind::Parent | Kind::Dir)
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
    /// Always 0 for directories: computing a directory's real size means walking it, which
    /// is a separate, cancellable operation and not something a listing may block on.
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hidden: bool,
    /// This entry is a symlink. `kind` describes what it points at, so a link to a
    /// directory is `Dir`; a broken link is `File` with `error` set.
    pub symlink: bool,
    /// Set when the entry could not be stat'ed. The row is still shown — a file you lack
    /// permission to stat must not remove the rest of the directory from view.
    pub error: Option<String>,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind.is_dir()
    }

    pub fn is_parent(&self) -> bool {
        self.kind == Kind::Parent
    }

    /// Name split into stem and extension, orthodox-style.
    pub fn stem_ext(&self) -> (&str, &str) {
        if self.is_dir() {
            (self.name.as_str(), "")
        } else {
            split_name(&self.name)
        }
    }
}

/// What a non-regular file is, worded for a person — or `None` for a regular file,
/// a directory or a symlink. Decided from `stat`, never by opening: opening a FIFO
/// blocks until something writes to it, which is how F3 on one used to freeze the UI.
pub fn special_kind(ft: &std::fs::FileType) -> Option<&'static str> {
    if ft.is_file() || ft.is_dir() || ft.is_symlink() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if ft.is_fifo() {
            return Some("a named pipe (FIFO)");
        }
        if ft.is_char_device() || ft.is_block_device() {
            return Some("a device");
        }
        if ft.is_socket() {
            return Some("a socket");
        }
    }
    Some("not a regular file")
}

/// Split a filename on its **last** dot.
///
/// A leading dot belongs to the stem, so `.bashrc` is a name with no extension rather than
/// an extension with no name. `archive.tar.gz` is `("archive.tar", "gz")` — the same
/// choice Total Commander makes, and the reason its Ext column sorts the way users expect.
pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        None | Some(0) => (name, ""),
        Some(i) => (&name[..i], &name[i + 1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::split_name;

    #[test]
    fn splits_on_the_last_dot() {
        assert_eq!(split_name("archive.tar.gz"), ("archive.tar", "gz"));
        assert_eq!(split_name("notes.md"), ("notes", "md"));
    }

    #[test]
    fn a_leading_dot_is_not_an_extension() {
        assert_eq!(split_name(".bashrc"), (".bashrc", ""));
        assert_eq!(split_name(".config"), (".config", ""));
    }

    #[test]
    fn handles_degenerate_names() {
        assert_eq!(split_name(""), ("", ""));
        assert_eq!(split_name("Makefile"), ("Makefile", ""));
        assert_eq!(split_name("trailing."), ("trailing", ""));
        assert_eq!(split_name("..gitignore"), (".", "gitignore"));
    }
}
