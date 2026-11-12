//! Kept lists in the panes: Alt+L shows the active one, `p` and `P` in a shell's
//! history make one or add to one. The lists themselves are in [`crate::lists`].

use super::*;
use crate::lists;
use crate::viewer::ListRequest;

impl App {
    /// Show list `id` (or the active one) in the focused file pane, or, from a
    /// terminal, in the file pane used last, which takes the focus. The place it
    /// replaces goes on that tab's back list.
    pub(super) fn show_list(&mut self, id: Option<lists::Id>) {
        let Some(id) = id.or_else(|| lists::active().map(|l| l.id)) else {
            let key = self.footer_key(Action::TerminalHistory).unwrap_or_default();
            self.status = format!(
                "no list yet \u{2014} Alt+F7 finds files; p in a shell's history ({key}) takes its paths"
            );
            return;
        };
        let Some(pane) = self.last_files_id() else {
            return;
        };
        self.focus = pane;
        self.note_focus();
        let Some(f) = self.focused_files() else {
            return;
        };
        // Shown here already: checked again (rows just added), not recorded twice.
        let again = f.list_id() == Some(id);
        f.open_list(id, !again);
        if let Some(n) = f.notice.take() {
            self.status = n;
        }
    }

    /// `p` or `P` in a shell's history. `p` makes a new list and shows it; `P` asks
    /// which list to add the paths to, the active one first.
    pub(super) fn make_list(&mut self, req: ListRequest, add: bool) {
        let kept = lists::all();
        if add && !kept.is_empty() {
            let n = req.count();
            let mut items: Vec<Item> = kept
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let label = format!(
                        "{} \u{b7} {}",
                        xdevcmd_core::format::elide_middle(&l.source, 50),
                        l.rows.len()
                    );
                    Item::cmd(label, digit(i), Cmd::AddToList(Some(l.id)), true)
                })
                .collect();
            items.push(Item::sep());
            items.push(Item::cmd("New list", "n", Cmd::AddToList(None), true));
            // The history stays behind the menu, and comes back if it is cancelled.
            if let Some(Modal::View(v)) = self.modal.take() {
                self.return_to = Some(v);
            }
            self.pending_list = Some(req);
            self.modal = Some(Modal::Menu(Menu::new(
                Some(format!(
                    "Add {n} path{} to\u{2026}",
                    if n == 1 { "" } else { "s" }
                )),
                items,
                self.pane_anchor(),
            )));
            return;
        }
        self.new_list(req);
    }

    /// A list of `req`'s paths, shown in a file pane; the history closes.
    fn new_list(&mut self, req: ListRequest) {
        let (rows, capped, outside) = lists::tidy_rows(req.paths);
        if rows.is_empty() {
            let why = if outside > 0 {
                "those paths are outside the root xdevcmd is confined to"
            } else {
                "no paths in those lines"
            };
            return self.note_in_viewer(why);
        }
        let fallback = req.base.unwrap_or_else(|| self.focused_cwd());
        let root = lists::common_root(&rows, &fallback);
        let id = lists::keep(req.source, root, rows, capped, outside, false, req.groups);
        self.close_history();
        self.show_list(Some(id));
    }

    /// The menu's answer to `P`: add to list `id`, or make a new one.
    pub(super) fn add_to_list(&mut self, id: Option<lists::Id>) {
        let Some(req) = self.pending_list.take() else {
            return;
        };
        let Some(id) = id else {
            return self.new_list(req);
        };
        match lists::add(id, req.paths, req.groups, &req.source) {
            Some((added, had)) => {
                self.close_history();
                self.show_list(Some(id));
                let had = match had {
                    0 => String::new(),
                    n => format!(", {n} already there"),
                };
                self.status = format!("{added} added{had} \u{2014} checking them");
            }
            None => self.status = "that list is no longer kept".into(),
        }
    }

    /// The history viewer goes: a list was made from it.
    fn close_history(&mut self) {
        if matches!(self.modal, Some(Modal::View(_))) {
            self.modal = None;
        }
        self.return_to = None;
    }

    fn note_in_viewer(&mut self, why: &str) {
        match self.modal.as_mut() {
            Some(Modal::View(v)) => v.note(why),
            _ => self.status = why.into(),
        }
    }
}
