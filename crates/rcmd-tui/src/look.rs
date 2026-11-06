//! Optional looks, all off by default: colours from `LS_COLORS`, Nerd Font icons.
//!
//! Off by default because both depend on things rcmd cannot see: `LS_COLORS` is
//! written for `ls` on a dark or light background rcmd does not know, and icons are
//! private-use characters that show as boxes without a Nerd Font.

use std::collections::HashMap;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};

use rcmd_core::{Entry, Kind};

#[derive(Debug, Default, Clone)]
pub struct Look {
    pub ls: Option<LsColors>,
    pub icons: bool,
    /// Run git for the status column. Never in restricted mode.
    pub git: bool,
}

static LOOK: OnceLock<Look> = OnceLock::new();

pub fn init(l: Look) {
    let _ = LOOK.set(l);
}

pub fn get() -> &'static Look {
    LOOK.get_or_init(Look::default)
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LsColors {
    dir: Option<Style>,
    link: Option<Style>,
    orphan: Option<Style>,
    exec: Option<Style>,
    file: Option<Style>,
    /// `*.ext` entries, keyed by the lower-case extension.
    ext: HashMap<String, Style>,
    /// Other `*suffix` entries, longest first.
    suffix: Vec<(String, Style)>,
}

/// An SGR sequence as `ls` writes it: `01;34`, `38;5;208`, `38;2;255;128;0`.
fn sgr(spec: &str) -> Option<Style> {
    let codes: Vec<u16> = spec.split(';').filter_map(|c| c.parse().ok()).collect();
    let mut st = Style::new();
    let mut i = 0;
    while i < codes.len() {
        match codes[i] {
            0 => st = Style::new(),
            1 => st = st.add_modifier(Modifier::BOLD),
            2 => st = st.add_modifier(Modifier::DIM),
            3 => st = st.add_modifier(Modifier::ITALIC),
            4 => st = st.add_modifier(Modifier::UNDERLINED),
            c @ 30..=37 => st = st.fg(Color::Indexed((c - 30) as u8)),
            c @ 90..=97 => st = st.fg(Color::Indexed((c - 90 + 8) as u8)),
            38 if codes.get(i + 1) == Some(&5) => {
                st = st.fg(Color::Indexed(*codes.get(i + 2)? as u8));
                i += 2;
            }
            38 if codes.get(i + 1) == Some(&2) => {
                let (r, g, b) = (*codes.get(i + 2)?, *codes.get(i + 3)?, *codes.get(i + 4)?);
                st = st.fg(Color::Rgb(r as u8, g as u8, b as u8));
                i += 4;
            }
            // Backgrounds and the rest are left to the theme: a row's background is
            // the cursor's and the panel's to decide.
            _ => {}
        }
        i += 1;
    }
    Some(st)
}

impl LsColors {
    pub fn parse(text: &str) -> Self {
        let mut l = Self::default();
        for part in text.split(':') {
            let Some((key, spec)) = part.split_once('=') else {
                continue;
            };
            let Some(style) = sgr(spec) else { continue };
            match key {
                "di" => l.dir = Some(style),
                "ln" => l.link = Some(style),
                "or" => l.orphan = Some(style),
                "ex" => l.exec = Some(style),
                "fi" => l.file = Some(style),
                k => {
                    if let Some(e) = k.strip_prefix("*.") {
                        l.ext.insert(e.to_ascii_lowercase(), style);
                    } else if let Some(s) = k.strip_prefix('*') {
                        l.suffix.push((s.to_string(), style));
                    }
                }
            }
        }
        l.suffix.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
        l
    }

    /// The style `ls` would give `e`, if `LS_COLORS` says anything about it.
    pub fn style(&self, e: &Entry, executable: bool) -> Option<Style> {
        if e.symlink {
            return if e.error.is_some() {
                self.orphan.or(self.link)
            } else {
                self.link
            };
        }
        match e.kind {
            Kind::Parent => None,
            Kind::Dir => self.dir,
            Kind::File => {
                if executable {
                    if let Some(s) = self.exec {
                        return Some(s);
                    }
                }
                let (_, ext) = e.stem_ext();
                self.ext
                    .get(&ext.to_ascii_lowercase())
                    .copied()
                    .or_else(|| {
                        self.suffix
                            .iter()
                            .find(|(s, _)| e.name.ends_with(s.as_str()))
                            .map(|(_, st)| *st)
                    })
                    .or(self.file)
            }
        }
    }
}

/// A Nerd Font glyph for `e`.
pub fn icon(e: &Entry) -> char {
    if e.is_parent() {
        return '\u{f062}';
    }
    if e.is_dir() {
        return match e.name.as_str() {
            ".git" => '\u{e5fb}',
            "node_modules" => '\u{e5fa}',
            _ => '\u{f115}',
        };
    }
    let (_, ext) = e.stem_ext();
    match ext.to_ascii_lowercase().as_str() {
        "rs" => '\u{e7a8}',
        "py" => '\u{e606}',
        "js" | "mjs" | "cjs" => '\u{e74e}',
        "ts" | "tsx" => '\u{e628}',
        "go" => '\u{e627}',
        "c" | "h" => '\u{e61e}',
        "cpp" | "cc" | "hpp" => '\u{e61d}',
        "java" | "jar" => '\u{e738}',
        "md" => '\u{e73e}',
        "toml" | "yaml" | "yml" | "ini" | "conf" | "cfg" => '\u{e615}',
        "json" => '\u{e60b}',
        "html" | "htm" => '\u{e736}',
        "css" | "scss" => '\u{e749}',
        "sh" | "bash" | "zsh" | "fish" | "ps1" => '\u{f489}',
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" | "zst" => '\u{f410}',
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "ico" => '\u{f1c5}',
        "mp3" | "flac" | "wav" | "ogg" => '\u{f001}',
        "mp4" | "mkv" | "mov" | "avi" | "webm" => '\u{f03d}',
        "pdf" => '\u{f1c1}',
        "txt" | "log" => '\u{f15c}',
        "lock" => '\u{f023}',
        _ => '\u{f15b}',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: Kind::File,
            size: 0,
            modified: None,
            hidden: false,
            symlink: false,
            error: None,
        }
    }

    #[test]
    fn ls_colors_as_gnu_writes_them() {
        let l = LsColors::parse(
            "di=01;34:ln=01;36:ex=01;32:*.tar=01;31:*.JPG=38;5;208:*README=38;2;1;2;3",
        );
        assert_eq!(
            l.dir,
            Some(
                Style::new()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Indexed(4))
            )
        );
        assert_eq!(
            l.style(&file("a.tar"), false).and_then(|s| s.fg),
            Some(Color::Indexed(1))
        );
        assert_eq!(
            l.style(&file("photo.jpg"), false).and_then(|s| s.fg),
            Some(Color::Indexed(208)),
            "extensions match without case"
        );
        assert_eq!(
            l.style(&file("README"), false).and_then(|s| s.fg),
            Some(Color::Rgb(1, 2, 3))
        );
        assert_eq!(
            l.style(&file("x.tar"), true).and_then(|s| s.fg),
            Some(Color::Indexed(2)),
            "ex wins"
        );
        assert_eq!(
            l.style(&file("plain"), false),
            None,
            "nothing said: the theme decides"
        );
    }

    #[test]
    fn garbage_in_ls_colors_is_ignored_not_fatal() {
        // A truncated 256-colour code is dropped; the rest still parses.
        let l = LsColors::parse("::di:=:*.x=38;5:rs=0:*.y=01");
        assert_eq!(l.style(&file("a.x"), false), None);
        assert!(l.style(&file("a.y"), false).is_some());
    }
}
