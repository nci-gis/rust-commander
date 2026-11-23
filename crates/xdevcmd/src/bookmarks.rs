//! The hotlist (Ctrl+D): directories to jump to.
//!
//! Two sources. `[bookmarks]` in the config are yours, named and fixed — xdevcmd never
//! rewrites the config file, because that would lose its comments and layout. The ones
//! added from inside xdevcmd live in `bookmarks.toml` beside the session, owner-only, as
//! the session is: both list where you work.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::session;

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    paths: Vec<String>,
}

fn path() -> Option<PathBuf> {
    session::state_file("bookmarks.toml")
}

/// The bookmarks added from inside xdevcmd. Missing or unreadable is an empty list: a
/// broken bookmarks file must not stop anything else working.
pub fn load() -> Vec<PathBuf> {
    let Some(p) = path() else { return Vec::new() };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| toml::from_str::<File>(&t).ok())
        .map(|f| {
            f.paths
                .iter()
                .map(|p| xdevcmd_core::paths::plain(std::path::Path::new(p)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn save(list: &[PathBuf]) -> std::io::Result<()> {
    let p = path().ok_or_else(|| std::io::Error::other("no state directory"))?;
    let f = File {
        // TOML holds text; a path that is not UTF-8 cannot be bookmarked.
        paths: list
            .iter()
            .filter_map(|p| p.to_str().map(str::to_string))
            .collect(),
    };
    let text = toml::to_string_pretty(&f).map_err(std::io::Error::other)?;
    session::write_private(&p, text.as_bytes())
}

/// The home directory: `HOME`, then `USERPROFILE`, the first that is an absolute path
/// here. On Windows a `HOME` left by Cygwin or an old MSYS (`/home/ann`) is not one, so
/// `USERPROFILE` is used, rather than "no home directory".
pub fn home_dir() -> Option<PathBuf> {
    pick_home(std::env::var_os("HOME"), std::env::var_os("USERPROFILE"))
}

fn pick_home(
    home: Option<std::ffi::OsString>,
    profile: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    [home, profile]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .find(|p| p.is_absolute())
}

/// `~` and `~/…` are the home directory; anything else is taken as written.
pub fn expand_tilde(s: &str) -> PathBuf {
    let home = home_dir;
    if s == "~" {
        if let Some(h) = home() {
            return h;
        }
    }
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        if let Some(h) = home() {
            return h.join(rest);
        }
    }
    PathBuf::from(s)
}

/// A path for a person to read: the home directory shortened to `~`.
pub fn tidy(p: &Path) -> String {
    let home = home_dir();
    match home.as_deref().and_then(|h| p.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_that_is_not_a_path_here_gives_way_to_userprofile() {
        let p = |s: &str| Some(std::ffi::OsString::from(s));
        #[cfg(windows)]
        let (abs, other) = (r"C:\Users\ann", r"D:\home");
        #[cfg(not(windows))]
        let (abs, other) = ("/home/ann", "/srv/ann");
        // Cygwin's HOME as Windows sees it, or anything relative: not a home here.
        assert_eq!(pick_home(p("home/ann"), p(abs)), Some(PathBuf::from(abs)));
        #[cfg(windows)]
        assert_eq!(pick_home(p("/home/ann"), p(abs)), Some(PathBuf::from(abs)));
        assert_eq!(pick_home(p(other), p(abs)), Some(PathBuf::from(other)));
        assert_eq!(pick_home(None, p(abs)), Some(PathBuf::from(abs)));
        assert_eq!(pick_home(p("rel"), None), None);
        assert_eq!(pick_home(None, None), None);
    }

    #[test]
    fn tilde_is_home_and_nothing_else_is() {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return;
        };
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/src"), home.join("src"));
        assert_eq!(expand_tilde("/tmp/~x"), PathBuf::from("/tmp/~x"));
        assert_eq!(expand_tilde("~other"), PathBuf::from("~other"));
        assert_eq!(
            tidy(&home.join("src")),
            format!("~{}src", std::path::MAIN_SEPARATOR)
        );
    }
}
