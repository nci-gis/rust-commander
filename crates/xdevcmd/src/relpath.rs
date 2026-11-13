//! Copy relative path (item 48): paths written from the repository's root when the pane
//! is inside one, else from the pane's directory, and the status line says which.

use std::path::{Path, PathBuf};

/// What the paths were made relative to.
#[derive(Debug, PartialEq, Eq)]
pub enum Base {
    /// A repository's root: the nearest folder above (or at) the pane's directory that
    /// holds a `.git` entry (a directory, or a file in a worktree or submodule).
    Repository(PathBuf),
    /// The pane's own directory: no repository there.
    Folder(PathBuf),
}

/// The repository holding `dir`, found by looking for `.git` upward: no `git` is run.
pub fn repository_of(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|d| d.join(".git").symlink_metadata().is_ok())
        .map(Path::to_path_buf)
}

/// `paths` (in `cwd`) written relative to the repository holding `cwd`, else to `cwd`.
/// A path outside the base (a list's row from elsewhere) is written whole.
pub fn relative(paths: &[PathBuf], cwd: &Path) -> (Vec<String>, Base) {
    let base = match repository_of(cwd) {
        Some(root) => Base::Repository(root),
        None => Base::Folder(cwd.to_path_buf()),
    };
    let from = match &base {
        Base::Repository(p) | Base::Folder(p) => p,
    };
    let out = paths
        .iter()
        .map(|p| match p.strip_prefix(from) {
            Ok(r) if r.as_os_str().is_empty() => ".".to_string(),
            Ok(r) => r.display().to_string(),
            Err(_) => p.display().to_string(),
        })
        .collect();
    (out, base)
}

/// The status line after copying: what, and relative to what.
pub fn said(texts: &[String], base: &Base) -> String {
    let what = match texts {
        [one] => one.clone(),
        many => format!("{} paths", many.len()),
    };
    let to = match base {
        Base::Repository(root) => format!(
            "relative to the repository {}",
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.display().to_string())
        ),
        Base::Folder(_) => "relative to this folder (not in a repository)".into(),
    };
    format!("copied {what}, {to}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_a_repository_paths_start_at_its_root_and_outside_at_the_folder() {
        let root = std::env::temp_dir().join(format!("xdevcmd-relpath-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (repo, plain) = (root.join("proj"), root.join("plain"));
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("src/app")).unwrap();
        std::fs::create_dir_all(&plain).unwrap();

        let here = repo.join("src/app");
        let (t, b) = relative(&[here.join("main.rs")], &here);
        assert_eq!(b, Base::Repository(repo.clone()));
        assert_eq!(t, [Path::new("src/app/main.rs").display().to_string()]);
        assert_eq!(
            said(&t, &b),
            format!("copied {}, relative to the repository proj", t[0])
        );
        // The root itself, nothing selected.
        assert_eq!(relative(std::slice::from_ref(&repo), &repo).0, ["."]);

        let (t, b) = relative(&[plain.join("a.txt"), plain.join("b.txt")], &plain);
        assert_eq!(b, Base::Folder(plain.clone()));
        assert_eq!(t, ["a.txt", "b.txt"]);
        assert_eq!(
            said(&t, &b),
            "copied 2 paths, relative to this folder (not in a repository)"
        );

        // A worktree's or submodule's .git is a file: still a repository.
        let wt = root.join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), "gitdir: ../proj/.git/worktrees/wt").unwrap();
        assert_eq!(repository_of(&wt), Some(wt.clone()));
        let _ = std::fs::remove_dir_all(&root);
    }
}
