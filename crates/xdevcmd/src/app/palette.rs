//! F1, Commands: opening it where focus is, and running a row on what had focus.

use super::*;

use crate::commands::{Commands, Extra, Run, Size};
use crate::keys::Place;

impl App {
    /// Open F1 for where focus is now: its rows come first. Over the viewer, F1 keeps
    /// it underneath and gives it back on Esc.
    pub(super) fn open_commands(&mut self, size: Size, from_menu: bool) {
        let under = match self.modal.take() {
            Some(Modal::View(v)) => Some(Box::new(Modal::View(v))),
            other => {
                self.modal = other;
                None
            }
        };
        let ctx = if under.is_some() {
            Place::Viewer
        } else if self.focus_is_term() {
            Place::Terminal
        } else {
            Place::Files
        };
        let rows = commands::rows(&self.settings.keymap, &self.settings.config.menu, |r| {
            self.row_refused(r)
        });
        let intro = crate::keys::shell_rule(&self.settings.keymap);
        let mut c = Commands::new(
            rows,
            ctx,
            self.recent_commands.clone(),
            intro,
            size,
            self.screen,
        );
        c.from_menu = from_menu;
        c.option_hint = !self.option_hinted;
        c.under = under;
        self.modal = Some(Modal::Commands(Box::new(c)));
    }

    /// Why a row cannot run now: the mode, or an archive. The same answers the keys
    /// get, from the same places.
    pub(super) fn row_refused(&self, r: &Run) -> Option<String> {
        match r {
            Run::Act(a) => self.refused(*a).or_else(|| self.archive_refused(*a)),
            Run::User(_) => self.refused(Action::UserMenu),
            Run::Extra(_) | Run::ReadOnly(_) => None,
        }
    }

    /// Enter on row `row` of the open F1: close it, then run the row on what had focus
    /// when F1 was pressed, as if its key had been pressed there.
    ///
    /// - From the shell, a file pane's command acts on the file pane the shell was
    ///   entered from, and focus moves there so it is seen; the shell's own commands
    ///   (split, new tab) act on the shell.
    /// - Over the viewer, the viewer is closed first, as its Esc would.
    pub(super) fn run_command_row(&mut self, row: usize) {
        let Some(Modal::Commands(mut c)) = self.modal.take() else {
            return;
        };
        let r = &c.rows[row];
        let (run, id) = (r.run.clone(), r.id.clone());
        let from_menu = c.from_menu;
        // Over the viewer: give it back, then close it the way Esc does (a file viewed
        // from a shell's history goes back to that history first, so again).
        self.modal = c.under.take().map(|u| *u);
        for _ in 0..3 {
            if !matches!(self.modal, Some(Modal::View(_))) {
                break;
            }
            self.modal_act_inner(ModalAct::Close);
        }
        let in_shell = self.focus_is_term();
        // A `[[menu]]` command's %f %s %d are the file pane's, whatever it then types
        // into: from the shell, the file pane the shell was entered from.
        let files_first = match run {
            // Not one that acts on the shell (cd here, the last list): from the shell
            // it acts there, as Alt+X and its key does.
            Run::Act(a) => a.place() == Place::Files && !a.acts_on_shell(),
            Run::User(_) => true,
            Run::Extra(_) | Run::ReadOnly(_) => false,
        };
        if in_shell && files_first {
            self.focus_last_files();
        }
        let hint = if from_menu {
            None
        } else {
            self.next_time(&run, in_shell && self.focus_is_term())
        };
        self.status.clear();
        match run {
            Run::Act(a) => self.perform(a),
            Run::Extra(Extra::Settings) => self.perform_cmd(Cmd::Settings),
            Run::Extra(Extra::Theme) => self.perform_cmd(Cmd::ThemeMenu),
            Run::Extra(Extra::About) => self.perform_cmd(Cmd::About),
            Run::Extra(Extra::ConfigInUse) => self.perform_cmd(Cmd::ConfigInUse),
            Run::User(i) => self.run_user_command(i),
            Run::ReadOnly(_) => {}
        }
        // The command's own message, if it has one, says more than the key does.
        if let Some(h) = hint.filter(|_| self.status.is_empty()) {
            self.status = format!("Next time: {h}");
        }
        commands::remember(&mut self.recent_commands, &id);
        // Read-only modes write nothing, this included, and `restore_session = false`
        // keeps nothing between runs, as the session; the list lives for this one.
        if !self.settings.mode.read_only() && self.settings.config.general.restore_session {
            let _ = commands::save_recent(&self.recent_commands);
        }
    }

    /// Focus the file pane used last: where the shell was entered from.
    pub(super) fn focus_last_files(&mut self) {
        let files = self.file_panes();
        if let Some(id) = self
            .mru
            .iter()
            .copied()
            .find(|id| files.contains(id))
            .or_else(|| files.first().copied())
        {
            self.focus = id;
        }
    }

    /// The key that runs `run` where it just ran: in the shell, a shell command's key
    /// needs the prefix first ("Alt+x Alt+s"). `None` for a command with no key.
    fn next_time(&self, run: &Run, in_shell: bool) -> Option<String> {
        match run {
            Run::Act(a) => {
                let keys = self.settings.keymap.keys_for(*a);
                if in_shell && !a.is_global() {
                    // After the prefix only a single key works.
                    let one = keys.iter().find(|k| !k.contains(' '))?;
                    let p = self.settings.keymap.shell_prefix()?;
                    Some(format!("{} {one}", crate::keys::key_name(p.0, p.1)))
                } else {
                    self.footer_key(*a)
                }
            }
            Run::User(i) => {
                let letter = &self.settings.config.menu.get(*i)?.key;
                let menu = self.footer_key(Action::UserMenu)?;
                (!letter.is_empty()).then(|| format!("{menu} {letter}"))
            }
            Run::Extra(_) | Run::ReadOnly(_) => None,
        }
    }
}
