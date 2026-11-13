//! Help → Documentation (Round 026): where the documentation for this very version is.
//!
//! The link names the tag of the version that was built (`v` + `CARGO_PKG_VERSION`),
//! never `main`, which describes features this build may not have. A build from the
//! branch carries the version of its last release until `cargo release` bumps it, so it
//! points at that release's docs. xdevcmd shows the link and can copy it; it never opens
//! it (no network, not even through a browser it starts).

use std::path::{Path, PathBuf};

/// A page of `docs/` at this version's tag, as a `&'static str` for the tables that need
/// one (F1's read-only rows). [`docs_url`] gives the same text; a test holds them equal.
macro_rules! docs_page {
    ($page:literal) => {
        concat!(
            env!("CARGO_PKG_REPOSITORY"),
            "/blob/v",
            env!("CARGO_PKG_VERSION"),
            "/docs/",
            $page
        )
    };
}
pub(crate) use docs_page;

/// The documentation at this version's tag: `docs/` itself, or one page of it.
pub fn docs_url(page: Option<&str>) -> String {
    url_for(env!("CARGO_PKG_VERSION"), page)
}

/// [`docs_url`] for any version: `0.4.0-rc.1` → tag `v0.4.0-rc.1`; build metadata
/// (`+…`) is never part of a tag and is dropped.
fn url_for(version: &str, page: Option<&str>) -> String {
    let repo = env!("CARGO_PKG_REPOSITORY");
    let tag = version.split('+').next().unwrap_or(version);
    match page {
        None => format!("{repo}/tree/v{tag}/docs"),
        Some(p) => format!("{repo}/blob/v{tag}/docs/{p}"),
    }
}

/// The copy of `docs/` a release archive puts beside the binary, found as `themes/` is:
/// beside the program as it was started, then beside the file a link points to.
pub fn local_copy() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dirs: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(d) = std::fs::canonicalize(&exe)
        .ok()
        .as_deref()
        .and_then(Path::parent)
    {
        if !dirs.iter().any(|x| x == d) {
            dirs.push(d.to_path_buf());
        }
    }
    dirs.iter().find_map(|d| copy_beside(d))
}

/// `dir/docs`, when it holds the index (`README.md`).
fn copy_beside(dir: &Path) -> Option<PathBuf> {
    let docs = dir.join("docs");
    docs.join("README.md").is_file().then_some(docs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = env!("CARGO_PKG_REPOSITORY");

    #[test]
    fn the_link_names_the_versions_tag_not_main() {
        assert_eq!(url_for("0.4.0", None), format!("{REPO}/tree/v0.4.0/docs"));
        assert!(!docs_url(None).contains("main"));
        assert_eq!(
            docs_url(None),
            format!("{REPO}/tree/v{}/docs", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn a_pre_release_keeps_its_suffix_and_build_metadata_is_dropped() {
        assert_eq!(
            url_for("0.4.0-rc.1", None),
            format!("{REPO}/tree/v0.4.0-rc.1/docs")
        );
        assert_eq!(
            url_for("0.4.0+20261112.abc", None),
            format!("{REPO}/tree/v0.4.0/docs")
        );
    }

    #[test]
    fn a_page_is_a_file_at_the_tag() {
        assert_eq!(
            url_for("0.3.0", Some("troubleshooting.md")),
            format!("{REPO}/blob/v0.3.0/docs/troubleshooting.md")
        );
        // The table's static text is the function's.
        assert_eq!(
            docs_page!("troubleshooting.md"),
            docs_url(Some("troubleshooting.md"))
        );
    }

    #[test]
    fn the_local_copy_is_a_docs_directory_with_its_index() {
        let dir = std::env::temp_dir().join(format!("xdevcmd-docs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        assert_eq!(copy_beside(&dir), None, "no index, no copy");
        std::fs::write(dir.join("docs").join("README.md"), "# docs\n").unwrap();
        assert_eq!(copy_beside(&dir), Some(dir.join("docs")));
        assert_eq!(copy_beside(&dir.join("nowhere")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
