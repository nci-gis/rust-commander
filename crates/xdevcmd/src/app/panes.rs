//! Panes and layout: ids, focus, splits, zoom, swap and mirror, and which pane is where.

use super::*;

impl App {
    pub(super) fn alloc_id(&mut self) -> PaneId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(super) fn file_panes(&self) -> Vec<PaneId> {
        let mut leaves = Vec::new();
        self.tree.leaves(&mut leaves);
        leaves
            .into_iter()
            .filter(|id| self.panes.get(id).is_some_and(|p| !p.is_term()))
            .collect()
    }

    pub(super) fn focused_tabs(&mut self) -> Option<&mut FileTabs> {
        self.panes.get_mut(&self.focus)?.as_tabs_mut()
    }

    pub(super) fn focused_files(&mut self) -> Option<&mut FilePane> {
        self.panes.get_mut(&self.focus)?.as_files_mut()
    }

    pub(super) fn focused_cwd(&self) -> PathBuf {
        self.panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .map(|f| f.cwd.clone())
            .or_else(|| {
                self.file_panes()
                    .first()
                    .and_then(|id| self.panes.get(id))
                    .and_then(Pane::as_files)
                    .map(|f| f.cwd.clone())
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    pub(super) fn focus_is_term(&self) -> bool {
        self.panes.get(&self.focus).is_some_and(Pane::is_term)
    }

    pub(super) fn focus_next_file(&mut self) {
        let files = self.file_panes();
        if files.is_empty() {
            return;
        }
        let i = files.iter().position(|id| *id == self.focus);
        self.focus = match i {
            Some(i) => files[(i + 1) % files.len()],
            None => files[0],
        };
    }

    pub(super) fn split_focused(&mut self, axis: Axis) {
        if self.focus_is_term() {
            if !self.terminal_room() {
                return;
            }
            let cwd = self.focused_cwd();
            if let Some(Pane::Term(p)) = self.panes.get_mut(&self.focus) {
                if let Err(e) = p.split(axis, &cwd) {
                    self.status = e.to_string();
                }
            }
            return;
        }
        let max = self.settings.config.files.max_panes;
        if self.file_panes().len() >= max {
            self.status =
                format!("at most {max} file panes \u{2014} [files] max_panes in config.toml");
            return;
        }
        let cwd = self.focused_cwd();
        let id = self.alloc_id();
        if self.tree.split_leaf(self.focus, axis, id) {
            let (sort, hidden) = (self.settings.sort, self.settings.config.files.show_hidden);
            self.panes
                .insert(id, Pane::files(FilePane::new(&cwd, sort, hidden)));
            self.focus = id;
        }
    }

    pub(super) fn close_focused(&mut self) {
        if self.focus_is_term() {
            // Closing a terminal ends its shell. Hiding the panel is Alt+T.
            let empty = match self.panes.get_mut(&self.focus) {
                Some(Pane::Term(p)) => p.close_focused(),
                _ => false,
            };
            if empty {
                self.remove_terminal_panel();
            }
            return;
        }
        if self.file_panes().len() <= 1 {
            self.status = "that is the last file pane".into();
            return;
        }
        let gone = self.focus;
        if self.tree.remove_leaf(gone) {
            self.panes.remove(&gone);
            // Back to the pane used last, not whichever happens to be first in the tree.
            let files = self.file_panes();
            self.focus = self
                .mru
                .iter()
                .copied()
                .find(|id| files.contains(id))
                .or_else(|| files.first().copied())
                .unwrap_or(0);
        }
    }

    pub(super) fn with_files(&mut self, f: impl FnOnce(&mut FilePane)) {
        if let Some(p) = self.focused_files() {
            f(p);
        }
    }

    pub(super) fn pane_at(&self, x: u16, y: u16) -> Option<PaneId> {
        self.rects
            .iter()
            .find(|(_, r)| r.contains(Position::new(x, y)))
            .map(|(id, _)| *id)
    }

    pub(super) fn toggle_zoom(&mut self) {
        if self.zoom.take().is_some() {
            self.status.clear();
        } else {
            self.zoom = Some(self.focus);
            let key = self.footer_key(Action::Zoom).unwrap_or_default();
            self.status = format!("zoomed \u{b7} {key} to put it back");
        }
    }

    pub(super) fn note_focus(&mut self) {
        if self.mru.first() != Some(&self.focus) {
            let f = self.focus;
            self.mru.retain(|id| *id != f);
            self.mru.insert(0, f);
            self.mru.truncate(64);
        }
    }

    /// The other file pane: the one used last, else any.
    pub(super) fn other_file_pane(&self) -> Option<PaneId> {
        let files = self.file_panes();
        self.mru
            .iter()
            .copied()
            .find(|id| *id != self.focus && files.contains(id))
            .or_else(|| files.iter().copied().find(|id| *id != self.focus))
    }

    pub(super) fn swap_panes(&mut self) {
        if self.focus_is_term() {
            return;
        }
        let Some(other) = self.other_file_pane() else {
            self.status = "there is no other pane".into();
            return;
        };
        let (Some(a), Some(b)) = (self.panes.remove(&self.focus), self.panes.remove(&other)) else {
            return;
        };
        self.panes.insert(self.focus, b);
        self.panes.insert(other, a);
        self.status = "panes swapped".into();
    }

    pub(super) fn mirror_pane(&mut self) {
        if self.focus_is_term() {
            return;
        }
        let Some(other) = self.other_file_pane() else {
            self.status = "there is no other pane".into();
            return;
        };
        let cwd = self.focused_cwd();
        let at = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current())
            .filter(|e| !e.is_parent())
            .map(|e| e.path.clone());
        if let Some(f) = self.panes.get_mut(&other).and_then(Pane::as_files_mut) {
            match at.as_deref().and_then(Path::file_name) {
                Some(name) => f.go_to_file(&cwd, name),
                None => f.set_cwd(&cwd),
            }
        }
        self.status = format!("both panes on {}", tidy(&cwd));
    }
}
