//! Where xdevcmd's folders are: the home, the config, the state (session, journal, crash
//! logs) and the data home (Linux's trash). One definition (Round 035, W5): before it,
//! six places read `HOME`, `USERPROFILE` and the XDG variables, each a little
//! differently (the crash log took `USERPROFILE` on Windows where the rest took `HOME`
//! first; the session and the config fell back to `HOME` alone; only the trash ignored
//! a relative XDG directory, as the spec asks).
//!
//! Each function has a `_from` form that reads from a given lookup, so the rules are
//! tested without touching the process's environment.

use std::ffi::OsString;
use std::path::PathBuf;

/// The variables a home can come from, in order: `HOME` (Unix, and set by Git Bash or
/// MSYS on Windows), then `USERPROFILE` (Windows).
const HOMES: [&str; 2] = ["HOME", "USERPROFILE"];

/// An absolute path from `v`, or `None`: a relative one would land in whatever folder
/// xdevcmd was started from. The XDG spec says to ignore it, and a Cygwin `HOME` of
/// `/home/ann` is not a Windows path.
fn absolute(v: Option<OsString>) -> Option<PathBuf> {
    v.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// The home folder: the first of `HOME`, `USERPROFILE` that is an absolute path.
pub fn home() -> Option<PathBuf> {
    home_from(&|k| std::env::var_os(k))
}

pub fn home_from(get: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    HOMES.iter().find_map(|k| absolute(get(k)))
}

/// Every home folder set, each absolute one once: protected from delete and move, so
/// on Windows both `HOME` and `USERPROFILE` count when they differ.
pub fn homes() -> Vec<PathBuf> {
    homes_from(&|k| std::env::var_os(k))
}

pub fn homes_from(get: &dyn Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in HOMES.iter().filter_map(|k| absolute(get(k))) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// `$xdg/xdevcmd`, else (Windows) `%win%\xdevcmd`, else `~/<under>/xdevcmd`.
fn own_dir(
    get: &dyn Fn(&str) -> Option<OsString>,
    windows: bool,
    xdg: &str,
    win: &str,
    under: &str,
) -> Option<PathBuf> {
    if let Some(d) = absolute(get(xdg)) {
        return Some(d.join("xdevcmd"));
    }
    if windows {
        if let Some(d) = absolute(get(win)) {
            return Some(d.join("xdevcmd"));
        }
    }
    home_from(get).map(|h| h.join(under).join("xdevcmd"))
}

/// The config folder: `$XDG_CONFIG_HOME/xdevcmd`, `%APPDATA%\xdevcmd` on Windows, else
/// `~/.config/xdevcmd` (macOS too: terminal tools live there, and that is where a user
/// looks).
pub fn config_dir() -> Option<PathBuf> {
    config_dir_from(&|k| std::env::var_os(k), cfg!(windows))
}

pub fn config_dir_from(get: &dyn Fn(&str) -> Option<OsString>, windows: bool) -> Option<PathBuf> {
    own_dir(get, windows, "XDG_CONFIG_HOME", "APPDATA", ".config")
}

/// The state folder (session, journal, crash logs, debug log):
/// `$XDG_STATE_HOME/xdevcmd`, `%LOCALAPPDATA%\xdevcmd` on Windows, else
/// `~/.local/state/xdevcmd`.
pub fn state_dir() -> Option<PathBuf> {
    state_dir_from(&|k| std::env::var_os(k), cfg!(windows))
}

pub fn state_dir_from(get: &dyn Fn(&str) -> Option<OsString>, windows: bool) -> Option<PathBuf> {
    own_dir(
        get,
        windows,
        "XDG_STATE_HOME",
        "LOCALAPPDATA",
        ".local/state",
    )
}

/// The data home (Linux's trash is `<it>/Trash`): `$XDG_DATA_HOME`, else
/// `~/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    data_home_from(&|k| std::env::var_os(k))
}

pub fn data_home_from(get: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    absolute(get("XDG_DATA_HOME")).or_else(|| home_from(get).map(|h| h.join(".local/share")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// An absolute path on this system, and one that is not.
    fn abs(rest: &str) -> String {
        if cfg!(windows) {
            format!("C:\\{rest}")
        } else {
            format!("/{rest}")
        }
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let m: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| ((*k).to_string(), OsString::from(v)))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn home_is_the_first_absolute_of_home_and_userprofile() {
        let (h, u) = (abs("h"), abs("u"));
        assert_eq!(
            home_from(&env(&[("HOME", &h), ("USERPROFILE", &u)])),
            Some(h.clone().into())
        );
        assert_eq!(
            home_from(&env(&[("HOME", "rel"), ("USERPROFILE", &u)])),
            Some(u.clone().into())
        );
        assert_eq!(home_from(&env(&[("USERPROFILE", &u)])), Some(u.into()));
        assert_eq!(home_from(&env(&[("HOME", "rel")])), None);
        assert_eq!(home_from(&env(&[])), None);
    }

    #[test]
    fn homes_are_each_absolute_one_once() {
        let (h, u) = (abs("h"), abs("u"));
        assert_eq!(
            homes_from(&env(&[("HOME", &h), ("USERPROFILE", &u)])),
            [PathBuf::from(&h), PathBuf::from(&u)]
        );
        assert_eq!(
            homes_from(&env(&[("HOME", &h), ("USERPROFILE", &h)])),
            [PathBuf::from(&h)]
        );
        assert_eq!(homes_from(&env(&[("HOME", "rel")])), Vec::<PathBuf>::new());
    }

    #[test]
    fn the_state_folder_follows_xdg_then_localappdata_then_home() {
        let (x, l, h) = (abs("x"), abs("l"), abs("h"));
        let all = env(&[("XDG_STATE_HOME", &x), ("LOCALAPPDATA", &l), ("HOME", &h)]);
        assert_eq!(
            state_dir_from(&all, true),
            Some(PathBuf::from(&x).join("xdevcmd"))
        );
        let no_xdg = env(&[("LOCALAPPDATA", &l), ("HOME", &h)]);
        assert_eq!(
            state_dir_from(&no_xdg, true),
            Some(PathBuf::from(&l).join("xdevcmd"))
        );
        assert_eq!(
            state_dir_from(&no_xdg, false),
            Some(PathBuf::from(&h).join(".local/state").join("xdevcmd")),
            "LOCALAPPDATA is Windows' only"
        );
    }

    /// The drift W5 removes: a relative XDG folder or HOME put the session, the
    /// journal and the config under whatever folder xdevcmd was started from.
    #[test]
    fn a_relative_folder_is_never_used() {
        let u = abs("u");
        let get = env(&[
            ("XDG_STATE_HOME", "rel"),
            ("XDG_CONFIG_HOME", ""),
            ("HOME", "rel"),
            ("USERPROFILE", &u),
        ]);
        assert_eq!(
            state_dir_from(&get, false),
            Some(PathBuf::from(&u).join(".local/state").join("xdevcmd"))
        );
        assert_eq!(
            config_dir_from(&get, false),
            Some(PathBuf::from(&u).join(".config").join("xdevcmd"))
        );
        assert_eq!(
            data_home_from(&get),
            Some(PathBuf::from(&u).join(".local/share"))
        );
        assert_eq!(state_dir_from(&env(&[("HOME", "rel")]), false), None);
    }

    #[test]
    fn the_config_folder_follows_xdg_then_appdata_then_home() {
        let (x, a, h) = (abs("x"), abs("a"), abs("h"));
        assert_eq!(
            config_dir_from(
                &env(&[("XDG_CONFIG_HOME", &x), ("APPDATA", &a), ("HOME", &h)]),
                true
            ),
            Some(PathBuf::from(&x).join("xdevcmd"))
        );
        assert_eq!(
            config_dir_from(&env(&[("APPDATA", &a), ("HOME", &h)]), true),
            Some(PathBuf::from(&a).join("xdevcmd"))
        );
        assert_eq!(
            config_dir_from(&env(&[("APPDATA", &a), ("HOME", &h)]), false),
            Some(PathBuf::from(&h).join(".config").join("xdevcmd"))
        );
    }
}
