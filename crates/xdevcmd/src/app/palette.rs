//! F1, Commands: opening it where focus is, and running a row on what had focus.

use super::*;

use crate::commands::{Commands, Extra, Run, Size};
use crate::keys::Place;

impl App {
    /// Open F1 for where focus is now: its rows come first.
    pub(super) fn open_commands(&mut self, size: Size, from_menu: bool) {
        let ctx = if self.focus_is_term() {
            Place::Terminal
        } else {
            Place::Files
        };
        let rows = commands::rows(&self.settings.keymap, &self.settings.config.menu, |r| {
            self.row_refused(r)
        });
        let intro = crate::keys::shell_rule(&self.settings.keymap);
        let mut c = Commands::new(rows, ctx, Vec::new(), intro, size, self.screen);
        c.from_menu = from_menu;
        c.option_hint = !self.option_hinted;
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

    /// Enter on row `row` of the open F1: close it, then run the row.
    pub(super) fn run_command_row(&mut self, row: usize) {
        let Some(Modal::Commands(mut c)) = self.modal.take() else {
            return;
        };
        let run = c.rows[row].run.clone();
        self.modal = c.under.take().map(|u| *u);
        match run {
            Run::Act(a) => self.perform(a),
            Run::Extra(Extra::Settings) => self.perform_cmd(Cmd::Settings),
            Run::Extra(Extra::Theme) => self.perform_cmd(Cmd::ThemeMenu),
            Run::Extra(Extra::About) => self.perform_cmd(Cmd::About),
            Run::User(i) => self.run_user_command(i),
            Run::ReadOnly(_) => {}
        }
    }
}
