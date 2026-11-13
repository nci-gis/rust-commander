//! The session: building panes from it at start, saving it, and the cwd file on exit.

use super::*;

impl App {
    /// Build the app from a saved session. `None` when the session does not fit the
    /// current config — more panes than `max_panes` allows — so the caller starts fresh.
    pub fn from_session(s: &Session, settings: Settings, outcome: Outcome) -> Option<Self> {
        if s.panes.pane_count() > settings.config.files.max_panes {
            return None;
        }
        let home = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut app = Self::new(&home, &home, settings, outcome);
        app.panes.clear();
        app.next_id = 0;
        app.tree = app.build_saved(&s.panes, &home);
        let files = app.file_panes();
        app.focus = files.get(s.focus).or(files.first()).copied().unwrap_or(0);
        app.term_height = s.terminal.height_pct.clamp(10, 90);
        app.restore_terminal = Some((s.terminal.clone(), s.terminal_focused));
        Some(app)
    }

    pub(super) fn build_saved(&mut self, s: &Saved, fallback: &Path) -> Node {
        match s {
            Saved::Pane { active, tabs } => {
                let mut panes: Vec<FilePane> = tabs
                    .iter()
                    .map(|t| {
                        let key = match t.sort.as_str() {
                            "ext" => SortKey::Ext,
                            "size" => SortKey::Size,
                            "modified" => SortKey::Modified,
                            _ => SortKey::Name,
                        };
                        let sort = xdevcmd_core::Sort {
                            key,
                            reverse: t.reverse,
                            dirs_first: self.settings.sort.dirs_first,
                        };
                        let dir = session::resolve_dir(&t.path, fallback);
                        FilePane::new(&xdevcmd_core::policy::get().clamp(&dir), sort, t.hidden)
                    })
                    .collect();
                if panes.is_empty() {
                    let hidden = self.settings.config.files.show_hidden;
                    panes.push(FilePane::new(fallback, self.settings.sort, hidden));
                }
                let mut it = panes.into_iter();
                let mut ft = FileTabs::new(it.next().expect("at least one tab"));
                for p in it {
                    ft.push(p);
                }
                ft.select(*active);
                let id = self.alloc_id();
                self.panes.insert(id, Pane::Files(Box::new(ft)));
                Node::Leaf(id)
            }
            Saved::Split { axis, pct, a, b } => {
                let axis = if axis == "rows" {
                    Axis::Rows
                } else {
                    Axis::Cols
                };
                let a = self.build_saved(a, fallback);
                let b = self.build_saved(b, fallback);
                Node::split(axis, (*pct).clamp(10, 90), a, b)
            }
        }
    }

    /// What is open now, as a session to save.
    pub fn snapshot(&mut self) -> Session {
        let (files_tree, height) = match (&self.tree, self.term_shown) {
            (Node::Split { pct, a, .. }, true) => ((**a).clone(), 100 - *pct),
            _ => (self.tree.clone(), self.term_height),
        };
        let panes = self.save_node(&files_tree);
        let mut leaves = Vec::new();
        files_tree.leaves(&mut leaves);
        let focus_is_term = self.focus_is_term();
        let last_files = self
            .mru
            .iter()
            .copied()
            .find(|id| leaves.contains(id))
            .unwrap_or(self.focus);
        let focus_leaf = if focus_is_term {
            last_files
        } else {
            self.focus
        };
        let shown = self.term_shown;
        let tabs = self.panel().map(|p| p.tab_dirs()).unwrap_or_default();
        Session {
            version: session::VERSION,
            focus: leaves.iter().position(|id| *id == focus_leaf).unwrap_or(0),
            terminal_focused: focus_is_term,
            panes,
            terminal: SavedTerminal {
                shown,
                height_pct: height,
                tabs,
            },
        }
    }

    pub(super) fn save_node(&self, n: &Node) -> Saved {
        match n {
            Node::Leaf(id) => {
                let Some(t) = self.panes.get(id).and_then(Pane::as_tabs) else {
                    return Saved::Pane {
                        active: 0,
                        tabs: Vec::new(),
                    };
                };
                let tabs = t
                    .tabs()
                    .iter()
                    .filter_map(|f| {
                        Some(SavedTab {
                            // A path that is not UTF-8 cannot go in TOML; that tab
                            // reopens at its pane's fallback instead.
                            path: f.cwd.to_str()?.to_string(),
                            sort: match f.sort.key {
                                SortKey::Name => "name",
                                SortKey::Ext => "ext",
                                SortKey::Size => "size",
                                SortKey::Modified => "modified",
                            }
                            .to_string(),
                            reverse: f.sort.reverse,
                            hidden: f.show_hidden,
                        })
                    })
                    .collect();
                Saved::Pane {
                    active: t.active_index(),
                    tabs,
                }
            }
            Node::Split { axis, pct, a, b } => Saved::Split {
                axis: if *axis == Axis::Rows { "rows" } else { "cols" }.to_string(),
                pct: *pct,
                a: Box::new(self.save_node(a)),
                b: Box::new(self.save_node(b)),
            },
        }
    }

    /// Write the session on the way out. A failure is not worth stopping a quit for,
    /// and there is no screen left to report it on.
    pub(super) fn save_session(&mut self) {
        // Read-only means xdevcmd writes nothing, its own state included.
        if !self.settings.config.general.restore_session || self.settings.mode.read_only() {
            return;
        }
        if let Some(path) = session::default_path() {
            let s = self.snapshot();
            if session::save(&path, &s).is_ok() {
                self.saved = Some(s);
            }
        }
    }

    /// Save the session when it has changed, every `[general] autosave_secs`. Saving only on
    /// quit lost everything when the window was closed instead — the X of Windows
    /// Terminal, closing an SSH session — because then xdevcmd is killed, not asked.
    pub(super) fn autosave(&mut self) {
        let every = self.settings.config.general.autosave_secs;
        if every == 0
            || !self.interactive
            || self.last_autosave.elapsed() < Duration::from_secs(every)
        {
            return;
        }
        self.last_autosave = Instant::now();
        let now = self.snapshot();
        if self.saved.as_ref() != Some(&now) {
            self.save_session();
        }
    }

    /// The directory you were in, for the shell that started xdevcmd to cd to. The file
    /// is one the caller named, so it is written even in read-only mode — it is the
    /// caller's output, not xdevcmd's state. Raw bytes on Unix: a name that is not UTF-8
    /// is still a directory you can cd to.
    pub(super) fn write_cwd_file(&self) {
        let Some(out) = &self.cwd_file else { return };
        let dir = self.last_files_cwd();
        #[cfg(unix)]
        let bytes = {
            use std::os::unix::ffi::OsStrExt;
            dir.as_os_str().as_bytes().to_vec()
        };
        #[cfg(not(unix))]
        let bytes = dir.to_string_lossy().into_owned().into_bytes();
        let _ = std::fs::write(out, bytes);
    }

    /// The directory of the file pane used last, even when the terminal has focus.
    pub(super) fn last_files_cwd(&self) -> PathBuf {
        let files = self.file_panes();
        let id = std::iter::once(self.focus)
            .chain(self.mru.iter().copied())
            .find(|id| files.contains(id));
        id.and_then(|id| self.panes.get(&id))
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
            .unwrap_or_else(|| self.focused_cwd())
    }
}
