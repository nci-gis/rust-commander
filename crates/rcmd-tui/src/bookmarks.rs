//! The hotlist (Ctrl+D): directories to jump to.
//!
//! Two sources. `[bookmarks]` in the config are yours, named and fixed — rcmd never
//! rewrites the config file, because that would lose its comments and layout. The ones
//! added from inside rcmd live in `bookmarks.toml` beside the session, owner-only, as
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

/// The bookmarks added from inside rcmd. Missing or unreadable is an empty list: a
/// broken bookmarks file must not stop anything else working.
pub fn load() -> Vec<PathBuf> {
    let Some(p) = path() else { return Vec::new() };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| toml::from_str::<File>(&t).ok())
        .map(|f| f.paths.into_iter().map(PathBuf::from).collect())
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

/// `~` and `~/…` are the home directory; anything else is taken as written.
pub fn expand_tilde(s: &str) -> PathBuf {
    let home = || {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    };
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
    let home = std::env::var_os("HOME").map(PathBuf::from);
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
