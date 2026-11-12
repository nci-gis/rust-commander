//! The paths in a shell's history (Shift+F12) that exist on disk: found in each line as
//! it is shown, resolved against the directory its command ran in, and kept only if
//! they are there.
//!
//! What a line says is not trusted: any program's output, or a file `cat`ed, can print
//! any path. So a path here is only ever offered; every action on it is one the user
//! starts, shows the absolute path first, and is checked as from a file pane (the
//! policy root, read-only mode). Paths on another machine (`\\server\…`, `//host/…`)
//! are never looked up: a lookup there can hang the screen.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// A path in a line, that exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Where it is in the line as shown, in characters.
    pub chars: Range<usize>,
    pub path: PathBuf,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub dir: bool,
    /// What a relative path was resolved against; `None` for an absolute one.
    pub base: Option<Base>,
}

/// Which directory a relative path was taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    /// The directory its command ran in (the shell's prompt marks said where).
    Command,
    /// The shell's directory now: the line came before any command the shell
    /// reported, or its command's directory is not known.
    Now,
    /// The shell's directory now: this shell sends no prompt marks.
    Unmarked,
}

pub struct Links {
    /// The line each command's output starts on, and its directory; by line.
    commands: Vec<(usize, Option<PathBuf>)>,
    dir_now: Option<PathBuf>,
    /// The links of each line looked at, by line.
    found: HashMap<usize, Vec<Link>>,
    /// What each path looked up turned out to be: a directory, a file, or nothing.
    exists: HashMap<PathBuf, Option<bool>>,
    /// The link chosen: its line, and which of that line's.
    pub selected: Option<(usize, usize)>,
    /// Which shell this is, for a list's source: "sh · tab 1".
    pub label: String,
    /// A program on another machine running in the shell now (ssh): its output names
    /// that machine's files, which may happen to exist here too.
    pub remote: Option<String>,
}

impl Links {
    pub fn new(mut commands: Vec<(usize, Option<PathBuf>)>, dir_now: Option<PathBuf>) -> Self {
        commands.sort_by_key(|c| c.0);
        Self {
            commands,
            dir_now,
            found: HashMap::new(),
            exists: HashMap::new(),
            selected: None,
            label: String::new(),
            remote: None,
        }
    }

    /// The lines of command `k`'s output: from where it starts to the line before the
    /// next command's (its prompt), or to the end.
    fn output_of(&self, k: usize, total: usize) -> Range<usize> {
        let start = self.commands[k].0;
        let end = self
            .commands
            .get(k + 1)
            .map_or(total, |c| c.0.saturating_sub(1))
            .max(start);
        start..end.min(total)
    }

    /// The output of the command line `i` belongs to, and that command's line as typed
    /// (the line before its output). `None` without prompt marks, or before the first.
    pub fn command_at(&self, i: usize, total: usize) -> Option<(Range<usize>, Option<usize>)> {
        let k = self.commands.partition_point(|c| c.0 <= i).checked_sub(1)?;
        let r = self.output_of(k, total);
        r.contains(&i)
            .then(|| (r, self.commands[k].0.checked_sub(1)))
    }

    /// The last command's output, and its line as typed.
    pub fn last_command(&self, total: usize) -> Option<(Range<usize>, Option<usize>)> {
        let k = self.commands.len().checked_sub(1)?;
        Some((self.output_of(k, total), self.commands[k].0.checked_sub(1)))
    }

    /// Every path on line `i` (shown as `shown`), resolved but not looked up: a list's
    /// rows are checked on a thread, not here.
    pub fn paths(&self, i: usize, shown: &str) -> Vec<PathBuf> {
        xdevcmd_core::found::paths_in(shown)
            .into_iter()
            .filter_map(|f| self.resolve(i, &f.path).map(|(p, _)| p))
            .collect()
    }

    /// The directory line `i`'s relative paths are taken from.
    pub fn base_dir(&self, i: usize) -> Option<PathBuf> {
        self.base_of(i).map(|(d, _)| d.to_path_buf())
    }

    /// What the status line says about the chosen link, with a warning while ssh runs.
    pub fn describe_chosen(&self) -> Said {
        let Some(l) = self.chosen() else {
            return Said::default();
        };
        let mut said = describe(l);
        if let Some(p) = &self.remote {
            said.before = format!("{p} is running: this is looked up on this machine \u{2014} ");
        }
        said
    }

    /// The directory relative paths on line `i` are taken from: the last command that
    /// started at or before it, else the shell's directory now.
    fn base_of(&self, i: usize) -> Option<(&Path, Base)> {
        let at = self.commands.partition_point(|c| c.0 <= i);
        let command = at.checked_sub(1).map(|k| &self.commands[k]);
        match command.and_then(|c| c.1.as_deref()) {
            Some(d) => Some((d, Base::Command)),
            None => {
                let how = if self.commands.is_empty() {
                    Base::Unmarked
                } else {
                    Base::Now
                };
                self.dir_now.as_deref().map(|d| (d, how))
            }
        }
    }

    /// The links on line `i`, whose text as shown is `shown`.
    pub fn on_line(&mut self, i: usize, shown: &str) -> &[Link] {
        if !self.found.contains_key(&i) {
            let mut links = Vec::new();
            for f in xdevcmd_core::found::paths_in(shown) {
                let Some((path, base)) = self.resolve(i, &f.path) else {
                    continue;
                };
                let Some(dir) = self.look_up(&path) else {
                    continue;
                };
                // As it really is: `a/../b` is shown, and acted on, as `b`.
                let path = xdevcmd_core::paths::canonical(&path).unwrap_or(path);
                let start = shown[..f.span.start].chars().count();
                let len = shown[f.span.clone()].chars().count();
                links.push(Link {
                    chars: start..start + len,
                    path,
                    line: f.line,
                    col: f.col,
                    dir,
                    base,
                });
            }
            self.found.insert(i, links);
        }
        self.found.get(&i).map_or(&[], Vec::as_slice)
    }

    /// The link chosen, if any.
    pub fn chosen(&self) -> Option<&Link> {
        let (i, k) = self.selected?;
        self.found.get(&i)?.get(k)
    }

    /// `raw` as an absolute path, with what it was resolved against; `None` for a path
    /// on another machine, or a relative one with nothing to resolve it against.
    fn resolve(&self, i: usize, raw: &str) -> Option<(PathBuf, Option<Base>)> {
        if raw.starts_with("\\\\") || raw.starts_with("//") {
            return None;
        }
        let p = crate::bookmarks::expand_tilde(raw);
        if p.is_absolute() {
            return Some((p, None));
        }
        let (base, how) = self.base_of(i)?;
        Some((base.join(p), Some(how)))
    }

    /// Whether `path` is there: `Some(true)` a directory, `Some(false)` anything else.
    fn look_up(&mut self, path: &Path) -> Option<bool> {
        if let Some(known) = self.exists.get(path) {
            return *known;
        }
        let found = std::fs::metadata(path).ok().map(|m| m.is_dir());
        self.exists.insert(path.to_path_buf(), found);
        found
    }
}

/// What the status line says about a chosen link, in its parts: a warning, the path,
/// then the keys and what the path was resolved against. Kept apart so that a path too
/// long for the line is cut in its middle, not at the end with everything after it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Said {
    pub before: String,
    /// The absolute path, with its line and column.
    pub path: String,
    pub after: String,
}

impl Said {
    /// All of it, as the message line holds it.
    pub fn text(&self) -> String {
        format!("{}{}{}", self.before, self.path, self.after)
    }

    /// The text for `width` columns. A path is a directory's name, then its own; on a
    /// narrow line, or in a long temporary directory (macOS's is `/private/var/folders/
    /// …/T/`, the links resolved), cutting the end lost the name and the keys. So the
    /// path gives up its middle, as much as the whole needs and never below half the
    /// line; what still does not fit is cut at the end as before.
    pub fn fit(&self, width: usize) -> String {
        let n = |s: &str| s.chars().count();
        let total = n(&self.before) + n(&self.path) + n(&self.after);
        if total <= width {
            return self.text();
        }
        let need = width.saturating_sub(n(&self.before) + n(&self.after));
        let room = need.max(width / 2);
        let path = xdevcmd_core::format::elide_middle(&self.path, room);
        format!("{}{path}{}", self.before, self.after)
    }
}

/// What the status line says about a chosen link: its absolute path and line, what it
/// was resolved against, and the keys.
pub fn describe(l: &Link) -> Said {
    let at = match (l.line, l.col) {
        (Some(n), Some(c)) => format!(":{n}:{c}"),
        (Some(n), None) => format!(":{n}"),
        _ => String::new(),
    };
    // Keys before the base: on a narrow screen the end is what is cut.
    let keys = if l.dir {
        "Enter goes there"
    } else {
        "Enter goes there, F3 views, F4 edits"
    };
    let from = match l.base {
        Some(Base::Command) => " \u{b7} from its command's directory",
        Some(Base::Now) => " \u{b7} from the shell's directory now",
        Some(Base::Unmarked) => {
            " \u{b7} from the shell's directory now: no prompt marks (docs/shell.md)"
        }
        None => "",
    };
    Said {
        before: String::new(),
        path: format!("{}{at}", l.path.display()),
        after: format!(" \u{2014} {keys}{from}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small tree of its own for each test: they run at once.
    fn tree(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xdevcmd-links-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("one/src")).unwrap();
        std::fs::create_dir_all(d.join("two")).unwrap();
        std::fs::write(d.join("one/src/a.rs"), "x").unwrap();
        std::fs::write(d.join("two/b.txt"), "x").unwrap();
        // Links are shown as they really are (macOS's /var is /private/var).
        xdevcmd_core::paths::canonical(&d).unwrap()
    }

    #[test]
    fn each_line_takes_its_commands_directory() {
        let d = tree("command");
        // Command 1 ran in one/ from line 0; command 2 in two/ from line 3.
        let mut l = Links::new(
            vec![(3, Some(d.join("two"))), (0, Some(d.join("one")))],
            Some(d.clone()),
        );
        let got = l.on_line(1, "src/a.rs:3:4 and b.txt").to_vec();
        assert_eq!(got.len(), 1, "b.txt is not in one/: {got:?}");
        assert_eq!(got[0].path, d.join("one/src/a.rs"));
        assert_eq!((got[0].line, got[0].col), (Some(3), Some(4)));
        assert_eq!(got[0].chars, 0..12);
        assert_eq!(got[0].base, Some(Base::Command));
        let got = l.on_line(4, "b.txt src/a.rs").to_vec();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, d.join("two/b.txt"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn without_prompt_marks_the_shells_directory_now_is_the_base() {
        let d = tree("unmarked");
        let mut l = Links::new(Vec::new(), Some(d.join("one")));
        let got = l.on_line(0, "  src").to_vec();
        assert_eq!(got.len(), 1);
        assert!(got[0].dir);
        assert_eq!(got[0].base, Some(Base::Unmarked));
        assert!(describe(&got[0]).text().contains("no prompt marks"));
        // An absolute path needs no base, and says none.
        let abs = format!("{}", d.join("two/b.txt").display());
        let got = l.on_line(1, &abs).to_vec();
        assert_eq!(got[0].base, None);
        // Missing files, URLs and other machines are not offered.
        assert!(l
            .on_line(2, "nope.rs https://x.y/z //host/share/a")
            .is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn characters_not_bytes_place_a_link() {
        let d = tree("chars");
        std::fs::write(d.join("one/設計.md"), "x").unwrap();
        let mut l = Links::new(Vec::new(), Some(d.join("one")));
        let got = l.on_line(0, "ü 設計.md").to_vec();
        assert_eq!(got[0].chars, 2..7);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A path too long for the line loses its middle, not its name and the keys after
    /// it (macOS's temporary directory, links resolved, is long; CI run 43177254223).
    #[test]
    fn a_long_path_is_cut_in_its_middle_and_keeps_its_name_and_keys() {
        let said = Said {
            before: String::new(),
            path: "/private/var/folders/h5/t85nd63d71x27_8gl5gw3l_w0000gn/T/x-12/left/notes.txt"
                .into(),
            after: " \u{2014} Enter goes there, F3 views, F4 edits \u{b7} from the shell's \
                    directory now"
                .into(),
        };
        let got = said.fit(95);
        assert!(got.starts_with("/private/var/"), "{got}");
        assert!(
            got.contains(
                "/left/notes.txt \u{2014} Enter goes there, F3 views, F4 edits \u{b7} from"
            ),
            "{got}"
        );
        assert!(got.contains('\u{2026}'), "{got}");
        // The path keeps half the line, at least: the base is what is cut after it.
        assert!(got.chars().count() > 95, "{got}");
        // One that fits is left whole, and so is the whole when the line has room.
        assert_eq!(said.fit(500), said.text());
        let short = Said {
            path: "/tmp/a/notes.txt".into(),
            ..said.clone()
        };
        assert_eq!(short.fit(95), short.text());
        // The warning while ssh runs stays whole: it is the point.
        let warned = Said {
            before: "ssh is running: this is looked up on this machine \u{2014} ".into(),
            ..said
        };
        assert!(warned
            .fit(95)
            .starts_with("ssh is running: this is looked up"));
    }
}
