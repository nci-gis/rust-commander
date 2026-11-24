//! Renaming one entry in place.
//!
//! Small, and easy to get subtly wrong: a rename that lands on an existing name
//! replaces that file without a word, which is the one thing a rename must never do.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Why a new name was refused, worded for the person who typed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    Empty,
    Reserved(String),
    BadChar(char),
    Exists(String),
    ReadOnly,
    Io(String),
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenameError::Empty => write!(f, "a name cannot be empty"),
            RenameError::Reserved(n) => write!(f, "\"{n}\" cannot be used as a name"),
            RenameError::BadChar('/') | RenameError::BadChar('\\') => {
                write!(
                    f,
                    "a name cannot contain a path separator \u{2014} F6 moves files"
                )
            }
            RenameError::BadChar(c) if crate::controls::is(*c) => {
                write!(f, "a name cannot contain {}", crate::controls::named(*c))
            }
            RenameError::BadChar(c) => write!(f, "a name cannot contain {c:?}"),
            RenameError::Exists(n) => {
                write!(
                    f,
                    "\"{n}\" already exists \u{2014} rename will not replace it"
                )
            }
            RenameError::ReadOnly => {
                write!(
                    f,
                    "xdevcmd is in read-only mode \u{2014} nothing is renamed"
                )
            }
            RenameError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Check a single path component. Separators are refused rather than treated as a
/// move: renaming `a` to `x/a` silently relocating the file is not what a rename box
/// is for, and F6 exists.
pub fn validate_name(name: &str) -> Result<(), RenameError> {
    if name.trim().is_empty() {
        return Err(RenameError::Empty);
    }
    if name == "." || name == ".." {
        return Err(RenameError::Reserved(name.to_string()));
    }
    // A control character (a newline, ESC, DEL, C1) is legal in a Unix name and breaks
    // whatever later lists or pastes it: refused everywhere (the release bar, item 1).
    if let Some(c) = name.chars().find(|&c| c == '/' || crate::controls::is(c)) {
        return Err(RenameError::BadChar(c));
    }
    #[cfg(windows)]
    {
        if let Some(c) = name.chars().find(|c| {
            matches!(c, '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') || (*c as u32) < 32
        }) {
            return Err(RenameError::BadChar(c));
        }
        if name.ends_with('.') || name.ends_with(' ') {
            return Err(RenameError::Reserved(name.to_string()));
        }
    }
    Ok(())
}

/// Rename `path` to `new_name` in the same directory. Returns the new path.
///
/// An existing target is refused, with one exception: a change of case only, on a
/// filesystem that ignores case (Windows, macOS by default). There the "existing"
/// target is the file itself, and refusing would make `Report.txt` -> `report.txt`
/// impossible.
///
/// The existence check and the rename are two steps, so a file created in between
/// could still be replaced. std has no portable no-replace rename; for an interactive
/// rename of one file the window is negligible, and it is noted rather than hidden.
/// The name `new` (as typed, from the name shown) with the bytes of `old` kept where
/// `new` did not change it: the same start and the same end are taken from `old` as they
/// are on disk. A name that is not UTF-8 is shown with `\u{fffd}` for each bad run of
/// bytes; editing another part of it no longer writes U+FFFD in its place (QA
/// 2026-11-17, guide.md: names are kept exactly). For a UTF-8 name this is `new`.
#[cfg(unix)]
pub fn keep_bytes(old: &std::ffi::OsStr, new: &str) -> std::ffi::OsString {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    // One unit per char of the name as shown, with the bytes it stands for.
    let mut units: Vec<(char, &[u8])> = Vec::new();
    for chunk in old.as_bytes().utf8_chunks() {
        let valid = chunk.valid();
        for (i, c) in valid.char_indices() {
            units.push((c, &valid.as_bytes()[i..i + c.len_utf8()]));
        }
        if !chunk.invalid().is_empty() {
            units.push(('\u{fffd}', chunk.invalid()));
        }
    }
    let typed: Vec<char> = new.chars().collect();
    let start = units
        .iter()
        .zip(&typed)
        .take_while(|((c, _), t)| c == *t)
        .count();
    let most = units.len().min(typed.len()) - start;
    let end = units
        .iter()
        .rev()
        .zip(typed.iter().rev())
        .take(most)
        .take_while(|((c, _), t)| c == *t)
        .count();
    let mut out: Vec<u8> = Vec::with_capacity(new.len());
    for (_, b) in &units[..start] {
        out.extend_from_slice(b);
    }
    let middle: String = typed[start..typed.len() - end].iter().collect();
    out.extend_from_slice(middle.as_bytes());
    for (_, b) in &units[units.len() - end..] {
        out.extend_from_slice(b);
    }
    std::ffi::OsString::from_vec(out)
}

/// Windows and others: a name is text (UTF-16 shown as is), nothing to keep.
#[cfg(not(unix))]
pub fn keep_bytes(_old: &std::ffi::OsStr, new: &str) -> std::ffi::OsString {
    new.into()
}

pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, RenameError> {
    if crate::policy::read_only() {
        return Err(RenameError::ReadOnly);
    }
    validate_name(new_name)?;
    let old_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if new_name == old_name {
        return Ok(path.to_path_buf());
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let target = dir.join(keep_bytes(path.file_name().unwrap_or_default(), new_name));

    let case_only = new_name.to_lowercase() == old_name.to_lowercase();
    if case_only && target.symlink_metadata().is_ok() {
        // On a file system that ignores case the "other" name is this file itself.
        if !crate::paths::same_entry(path, &target) {
            return Err(RenameError::Exists(new_name.to_string()));
        }
        fs::rename(path, &target).map_err(|e| RenameError::Io(e.to_string()))?;
        return Ok(target);
    }
    // Refused by the system if the name is taken, even by a file that appeared just now.
    match crate::noreplace::rename_noreplace(path, &target) {
        Ok(()) => Ok(target),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(RenameError::Exists(new_name.to_string()))
        }
        Err(e) => Err(RenameError::Io(e.to_string())),
    }
}

impl From<io::Error> for RenameError {
    fn from(e: io::Error) -> Self {
        RenameError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    /// QA 2026-11-17: what was not edited keeps its bytes; a UTF-8 name is the text.
    #[cfg(unix)]
    #[test]
    fn keep_bytes_keeps_what_was_not_edited() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let k = |old: &[u8], new: &str| keep_bytes(OsStr::from_bytes(old), new).as_bytes().to_vec();
        assert_eq!(k(b"plain.txt", "other.txt"), b"other.txt");
        assert_eq!(
            k("\u{e9}t\u{e9}".as_bytes(), "\u{e9}t\u{e9}2"),
            "\u{e9}t\u{e9}2".as_bytes()
        );
        assert_eq!(k(b"a\xff", "a\u{fffd}X"), b"a\xffX");
        assert_eq!(k(b"a\xff", "Xa\u{fffd}"), b"Xa\xff");
        assert_eq!(
            k(b"\xfe-mid-\xff", "\u{fffd}-new-\u{fffd}"),
            b"\xfe-new-\xff"
        );
        // A name typed new from scratch is what was typed.
        assert_eq!(k(b"a\xff", "b"), b"b");
        // Two bad runs, one removed: the one kept keeps its byte.
        assert_eq!(k(b"\xfe-\xff", "\u{fffd}-"), b"\xfe-");
    }

    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn dir() -> PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("xdevcmd-rename-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A link renamed to another case of its own name is renamed, not refused as a
    /// taken name (P8 H-04: Windows compared a link by its name as written). Runs where
    /// the drive ignores case: Windows, and macOS by default.
    #[test]
    fn a_link_renamed_to_another_case_of_its_name_is_renamed() {
        let d = dir();
        fs::write(d.join("t"), b"x").unwrap();
        crate::attrs::symlink(&d.join("t"), &d.join("ln")).unwrap();
        if !d.join("LN").exists() {
            let _ = fs::remove_dir_all(&d);
            return; // a case-sensitive file system: two names, two entries
        }
        let new = rename(&d.join("ln"), "LN").unwrap();
        assert!(fs::symlink_metadata(&new).unwrap().file_type().is_symlink());
        let names: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(names.iter().any(|n| n == "LN"), "{names:?}");
        let _ = fs::remove_dir_all(&d);
    }

    /// Where case counts, a hard link named in another case is another file: refused
    /// as taken, not "renamed" by a rename onto its own inode that changes nothing.
    #[cfg(unix)]
    #[test]
    fn a_hard_link_in_another_case_is_a_taken_name() {
        let d = dir();
        fs::write(d.join("a"), b"x").unwrap();
        if d.join("A").exists() {
            let _ = fs::remove_dir_all(&d);
            return; // case is ignored: there cannot be two
        }
        fs::hard_link(d.join("a"), d.join("A")).unwrap();
        assert!(matches!(
            rename(&d.join("a"), "A"),
            Err(RenameError::Exists(_))
        ));
        assert!(d.join("a").exists() && d.join("A").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn renames_a_file_and_returns_where_it_went() {
        let d = dir();
        fs::write(d.join("old.txt"), b"x").unwrap();
        let new = rename(&d.join("old.txt"), "new.txt").unwrap();
        assert_eq!(new, d.join("new.txt"));
        assert!(!d.join("old.txt").exists());
        assert_eq!(fs::read(&new).unwrap(), b"x");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn renames_a_directory_with_its_contents() {
        let d = dir();
        fs::create_dir_all(d.join("src/inner")).unwrap();
        fs::write(d.join("src/inner/f"), b"f").unwrap();
        rename(&d.join("src"), "dst").unwrap();
        assert_eq!(fs::read(d.join("dst/inner/f")).unwrap(), b"f");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn never_replaces_an_existing_file() {
        let d = dir();
        fs::write(d.join("a.txt"), b"keep me").unwrap();
        fs::write(d.join("b.txt"), b"b").unwrap();
        let e = rename(&d.join("b.txt"), "a.txt").unwrap_err();
        assert_eq!(e, RenameError::Exists("a.txt".into()));
        assert_eq!(fs::read(d.join("a.txt")).unwrap(), b"keep me");
        assert!(
            d.join("b.txt").exists(),
            "a refused rename leaves the source alone"
        );
        fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_hard_link_is_not_mistaken_for_a_case_change() {
        // Same inode, different name: rename(2) would do nothing and report success,
        // leaving the user wondering why the old name is still there.
        let d = dir();
        fs::write(d.join("a"), b"x").unwrap();
        fs::hard_link(d.join("a"), d.join("b")).unwrap();
        assert_eq!(
            rename(&d.join("a"), "b").unwrap_err(),
            RenameError::Exists("b".into())
        );
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_change_of_case_only_works() {
        let d = dir();
        fs::write(d.join("Report.txt"), b"x").unwrap();
        let new = rename(&d.join("Report.txt"), "report.txt").unwrap();
        assert_eq!(new.file_name().unwrap(), "report.txt");
        assert_eq!(fs::read(&new).unwrap(), b"x");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn keeping_the_same_name_is_not_an_error() {
        let d = dir();
        fs::write(d.join("same"), b"x").unwrap();
        assert_eq!(rename(&d.join("same"), "same").unwrap(), d.join("same"));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn bad_names_are_refused_before_touching_the_disk() {
        assert_eq!(validate_name(""), Err(RenameError::Empty));
        assert_eq!(validate_name("   "), Err(RenameError::Empty));
        assert_eq!(validate_name(".."), Err(RenameError::Reserved("..".into())));
        assert_eq!(validate_name("a/b"), Err(RenameError::BadChar('/')));
        for c in ['\0', '\n', '\t', '\x1b', '\x7f', '\u{9b}'] {
            let name = format!("a{c}b");
            assert_eq!(
                validate_name(&name),
                Err(RenameError::BadChar(c)),
                "{name:?}"
            );
        }
        assert_eq!(
            RenameError::BadChar('\n').to_string(),
            "a name cannot contain a control character (U+000A)"
        );
        assert!(
            validate_name("báo cáo tuần.docx").is_ok(),
            "non-ASCII is a normal name"
        );
        assert!(validate_name(".bashrc").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_renamed_not_its_target() {
        let d = dir();
        fs::write(d.join("target"), b"t").unwrap();
        std::os::unix::fs::symlink(d.join("target"), d.join("link")).unwrap();
        rename(&d.join("link"), "link2").unwrap();
        assert!(d.join("target").exists(), "the target keeps its name");
        assert!(fs::symlink_metadata(d.join("link2"))
            .unwrap()
            .file_type()
            .is_symlink());
        fs::remove_dir_all(&d).unwrap();
    }
}
