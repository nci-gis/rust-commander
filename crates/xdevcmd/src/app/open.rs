//! Viewing, editing and opening files: F3, F4, Enter, Open with, rules, and the user menu.

use super::*;

impl App {
    /// The file the cursor is on, or nothing when it is a directory or `..`.
    pub(super) fn current_file(&self) -> Option<PathBuf> {
        let f = self.panes.get(&self.focus).and_then(Pane::as_files)?;
        let e = f.current()?;
        (!e.is_dir()).then(|| e.path.clone())
    }

    pub(super) fn begin_view(&mut self) {
        if self.focused_archive().is_some() {
            return self.view_in_archive();
        }
        let Some(path) = self.current_file() else {
            self.status = "F3 views a file, not a directory".into();
            return;
        };
        // A link inside the root can point anywhere; what it points at is what F3 reads.
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        match Document::open(&path) {
            Ok(doc) => {
                let v = &self.settings.config.viewer;
                let viewer = Viewer::open(doc, v.wrap, v.tab_width);
                self.modal = Some(Modal::View(Box::new(viewer)));
            }
            Err(e) => self.status = format!("cannot view {}: {e}", path.display()),
        }
    }

    pub(super) fn begin_edit(&mut self) {
        match self.current_file() {
            Some(p) if !xdevcmd_core::policy::get().allows(&p) => {
                self.status = "that file is outside the root xdevcmd is confined to".into();
            }
            Some(p) => self.pending_edit = Some(p),
            None => self.status = "F4 edits a file, not a directory".into(),
        }
    }

    /// Hand the real terminal to `$EDITOR` and take it back afterwards.
    ///
    /// Running the editor inside the shell pane was the other option, and it is worse:
    /// the pane is a third of the screen, and an editor that small is not an editor.
    /// Suspending is what every orthodox manager does, and the editor gets the terminal
    /// it expects — full size, its own alternate screen, its own mouse handling.
    pub(super) fn edit_externally(&mut self, terminal: &mut Term, path: &Path) -> Result<()> {
        let configured = self.settings.config.commands.editor.trim().to_string();
        let spec = Some(configured)
            .filter(|s| !s.is_empty())
            .ok_or(())
            .or_else(|_| std::env::var("VISUAL"))
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| {
                // Something that certainly exists, rather than a failure to launch.
                if cfg!(windows) { "notepad" } else { "vi" }.to_string()
            });
        // EDITOR may carry arguments, as in `code -w` or `emacsclient -nw`.
        let mut parts = spec.split_whitespace();
        let Some(program) = parts.next() else {
            self.status = "EDITOR is set to an empty string".into();
            return Ok(());
        };
        let mut args: Vec<std::ffi::OsString> = parts.map(Into::into).collect();
        // F4 on a path in a shell's history: at its line, for the editors whose way of
        // saying so is known.
        let at = self.edit_at.take();
        let (file, passed) = editor_args(program, path, at);
        args.extend(file);

        // Step out of the alternate screen by hand and step back into it, rather than
        // calling ratatui::restore() and ratatui::init() again. Re-initialising asks
        // the terminal for its cursor position and dies when the answer is slow —
        // "The cursor position could not be read within a normal duration" — which is
        // exactly what happens over a laggy SSH link or inside a multiplexer. Keeping
        // the same Terminal avoids the question entirely.
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();

        let outcome = std::process::Command::new(program).args(&args).status();

        let _ = enable_raw_mode();
        let _ = execute!(stdout(), EnterAlternateScreen, EnableMouseCapture);
        // Not `terminal.clear()`: in ratatui 0.30 that snapshots the cursor position
        // first, and the query is what times out on a slow link — the very failure we
        // just moved away from. Clear the screen through the backend instead, then
        // reset BOTH diff buffers so the next draw repaints every cell. One swap only
        // resets one of them, and the diff against the stale frame comes out empty.
        let _ = terminal.backend_mut().clear_region(ClearType::All);
        // The editor left the cursor wherever it liked.
        terminal.backend_mut().forget();
        terminal.swap_buffers();
        terminal.swap_buffers();
        // And with it any picture the terminal drew.
        self.graphic_shown = None;
        self.kitty_held = None;

        let line = match at {
            Some((n, _)) if !passed => {
                format!(" \u{2014} line {n} not passed: xdevcmd knows no line flag for {program}")
            }
            _ => String::new(),
        };
        self.status = match outcome {
            Ok(s) if s.success() => format!("{program} closed{line}"),
            Ok(s) => format!("{program} exited with {s}"),
            Err(e) => format!("cannot run {program}: {e}"),
        };
        self.reload_all();
        Ok(())
    }

    /// `o`: the system's own application for the file under the cursor. Something
    /// that would run — a program, a script, a launcher — is asked about first, with
    /// Cancel the choice Enter makes.
    pub(super) fn open_with(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "o opens a file \u{2014} Enter goes into a directory".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        if opener::runs_something(&path) {
            return self.ask_to_run(path);
        }
        self.launch(&path);
    }

    /// "Run it?" for a program, a script or a launcher: Cancel first, then, for a
    /// console program, a terminal tab (its output stays on screen), then the system.
    pub(super) fn ask_to_run(&mut self, path: PathBuf) {
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let name = Self::describe(std::slice::from_ref(&path));
        let mut items = vec![Item::cancel("Cancel", "Esc")];
        if opener::console_program(&path) {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            items.push(Item::cmd(
                format!("Run {name} in a terminal tab"),
                "",
                Cmd::RunInTerminal {
                    argv: vec![path.clone().into_os_string()],
                    cwd,
                },
                true,
            ));
        }
        items.push(Item::cmd(
            format!("Run {name} with the system"),
            "",
            Cmd::OpenFile(path),
            true,
        ));
        self.status = format!("{name} is a program \u{2014} opening it runs it");
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Run it?".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Enter on a file, as in Total Commander: an `[[open]]` rule for its extension,
    /// else "Run it?" for a program, else the system's program; and where nothing can
    /// be opened (over SSH, no display, read-only), the viewer, saying why.
    pub(super) fn enter_file(&mut self) {
        if self.settings.config.files.enter == "view" || self.focused_archive().is_some() {
            return self.begin_view();
        }
        let Some(path) = self.current_file() else {
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            return self.begin_view();
        }
        let screen = opener::no_screen();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let rule = self.settings.config.open.iter().position(|r| {
            r.matches(&ext, crate::config::this_os()) && !(r.needs_display && screen.is_some())
        });
        if let Some(i) = rule {
            return self.apply_rule(i, path);
        }
        if self.refused(Action::OpenWith).is_some() {
            let why = format!("{} mode", self.settings.mode.label());
            return self.view_instead(&why);
        }
        if opener::runs_something(&path) {
            return self.ask_to_run(path);
        }
        let configured = !self.settings.config.commands.opener.trim().is_empty();
        if let (Some(why), false) = (screen, configured) {
            return self.view_instead(&why);
        }
        // A type it does not know as a document: what the system (or the configured
        // opener) does with it is its choice, and some types run code. Asked, not done.
        if !opener::document(&path) {
            return self.ask_to_open(path);
        }
        self.launch(&path);
    }

    /// "Open it?" for a file whose type is not a known document: Cancel first, then the
    /// system's program, then the viewer.
    pub(super) fn ask_to_open(&mut self, path: PathBuf) {
        let name = Self::describe(std::slice::from_ref(&path));
        let items = vec![
            Item::cancel("Cancel", "Esc"),
            Item::cmd(
                format!("Open {name} with the system's program"),
                "",
                Cmd::OpenFile(path),
                true,
            ),
            Item::act(
                "View it",
                self.footer_key(Action::View).unwrap_or_default(),
                Action::View,
                true,
            ),
        ];
        self.status = format!("{name}: not a type xdevcmd knows as a document \u{2014} open it?");
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Open it?".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Enter could not open the file: view it, and say why where it will be seen — in
    /// the viewer, which covers the status line.
    pub(super) fn view_instead(&mut self, why: &str) {
        let note = format!("{why}: viewing it instead");
        self.begin_view();
        if let Some(Modal::View(v)) = &mut self.modal {
            v.note(&note);
        }
        self.status = note;
    }

    /// Rule `i` of `[[open]]` on `path`.
    pub(super) fn apply_rule(&mut self, i: usize, path: PathBuf) {
        // Checked here as well as on Enter: Open with… and the mouse reach this too.
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let Some(rule) = self.settings.config.open.get(i).cloned() else {
            return;
        };
        match rule.action.as_str() {
            "view" => return self.begin_view(),
            "edit" => return self.perform(Action::Edit),
            _ => {}
        }
        // A command runs a program: not where programs may not run.
        if self.refused(Action::OpenWith).is_some() {
            let why = format!("{} mode", self.settings.mode.label());
            return self.view_instead(&why);
        }
        let argv = rule.argv(&path);
        if rule.terminal {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            return self.run_in_terminal(argv, cwd);
        }
        let name = Self::describe(std::slice::from_ref(&path));
        if !self.interactive {
            self.status = format!("would open {name} with {}", rule.command[0]);
            return;
        }
        let mut cmd = std::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        self.status = match opener::spawn(cmd, name.clone(), self.notes.0.clone()) {
            Ok(()) => format!("opened {name} with {}", rule.command[0]),
            Err(e) => e,
        };
    }

    /// Shift+O: every way to open the file under the cursor — the `[[open]]` rules that
    /// match it, the system's program, the viewer, the editor.
    pub(super) fn open_with_menu(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "Open with\u{2026} is for a file".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let screen = opener::no_screen();
        let mut items = Vec::new();
        for (i, r) in self.settings.config.open.iter().enumerate() {
            if r.matches(&ext, crate::config::this_os()) {
                let usable = !(r.needs_display && screen.is_some());
                items.push(Item::cmd(
                    r.label(),
                    if r.terminal { "in a tab" } else { "" },
                    Cmd::ApplyRule(i, path.clone()),
                    usable,
                ));
            }
        }
        items.push(Item::act(
            "System default",
            self.footer_key(Action::OpenWith).unwrap_or_default(),
            Action::OpenWith,
            self.refused(Action::OpenWith).is_none() && screen.is_none(),
        ));
        items.push(Item::act(
            "View",
            self.footer_key(Action::View).unwrap_or_default(),
            Action::View,
            true,
        ));
        items.push(Item::act(
            "Edit",
            self.footer_key(Action::Edit).unwrap_or_default(),
            Action::Edit,
            self.refused(Action::Edit).is_none(),
        ));
        if opener::console_program(&path) {
            let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
            items.push(Item::cmd(
                "Run in a terminal tab",
                "",
                Cmd::RunInTerminal {
                    argv: vec![path.clone().into_os_string()],
                    cwd,
                },
                !self.settings.mode.read_only(),
            ));
        }
        items.push(Item::cancel("Cancel", "Esc"));
        let name = Self::describe(std::slice::from_ref(&path));
        self.modal = Some(Modal::Menu(Menu::new(
            Some(format!("Open {name} with")),
            items,
            self.pane_anchor(),
        )));
    }

    pub(super) fn launch(&mut self, path: &Path) {
        let name = Self::describe(&[path.to_path_buf()]);
        let cmd = match opener::command(path, &self.settings.config.commands.opener) {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        // Under --screenshot nothing is started: say what would have been.
        if !self.interactive {
            self.status = format!(
                "would open {name} with {}",
                cmd.get_program().to_string_lossy()
            );
            return;
        }
        self.status = match opener::spawn(cmd, name.clone(), self.notes.0.clone()) {
            Ok(()) => format!("opened {name}"),
            Err(e) => e,
        };
    }

    pub(super) fn open_user_menu(&mut self) {
        let menu = &self.settings.config.menu;
        if menu.is_empty() {
            self.status =
                "no commands yet \u{2014} add [[menu]] entries to config.toml (Help \u{203a} Config in use shows it)"
                    .into();
            return;
        }
        let mut items: Vec<Item> = menu
            .iter()
            .enumerate()
            .map(|(i, m)| Item::cmd(m.label.clone(), m.key.clone(), Cmd::UserCommand(i), true))
            .collect();
        items.push(Item::sep());
        items.push(Item::cancel("Cancel", "Esc"));
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Commands".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Type entry `i` of `[[menu]]` into the terminal, placeholders filled and quoted.
    pub(super) fn run_user_command(&mut self, i: usize) {
        let Some(item) = self.settings.config.menu.get(i).cloned() else {
            return;
        };
        let full = self.screen;
        if self.term_id.is_none() || !self.term_shown {
            self.toggle_term(full);
        }
        let Some(id) = self.term_id else {
            return;
        };
        let cx = crate::usermenu::Context {
            shell: match self.panes.get(&id) {
                Some(Pane::Term(t)) => t.shell_name(),
                _ => String::new(),
            },
            current: self
                .panes
                .get(&self.focus)
                .and_then(Pane::as_files)
                .and_then(|f| f.current())
                .filter(|e| !e.is_parent())
                .map(|e| e.path.clone()),
            dir: self.focused_cwd(),
            selection: self.selection(),
            other: self.other_pane_cwd(),
        };
        let line = match crate::usermenu::expand(&item.command, &cx) {
            Ok(l) => l,
            Err(e) => {
                self.status = format!("{}: {e}", item.label);
                return;
            }
        };
        let Some(target) = self.panel_ref().and_then(TermPanel::focused_key) else {
            return;
        };
        self.focus = id;
        // Typed like everything else typed into a shell: the one decision, which asks
        // where the shell is busy or cannot be read (owner, 2026-11-09).
        let line = format!("{line}\r");
        self.send_to_shell(ShellSend {
            target,
            here: line.clone(),
            new_shell: Some(line),
            dir: cx.dir,
            paste: false,
            what: format!("ran {}", item.label),
        });
    }
}

/// The arguments that open `path` in `program` at a line (and column), and whether the
/// line was passed: editors each say it their own way, and one not in this table gets
/// the file alone (its status says so) rather than a flag it may take for a file name.
fn editor_args(
    program: &str,
    path: &Path,
    at: Option<(u32, Option<u32>)>,
) -> (Vec<std::ffi::OsString>, bool) {
    let file = path.as_os_str().to_os_string();
    let Some((line, col)) = at else {
        return (vec![file], true);
    };
    let name = Path::new(program)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let joined = |with_col: bool| {
        let mut s = file.clone();
        s.push(format!(":{line}"));
        if let (true, Some(c)) = (with_col, col) {
            s.push(format!(":{c}"));
        }
        s
    };
    match name.as_str() {
        "vi" | "vim" | "nvim" | "gvim" | "view" | "nano" | "emacs" | "emacsclient" | "micro"
        | "kak" | "mg" | "joe" => (vec![format!("+{line}").into(), file], true),
        "hx" | "helix" | "subl" | "sublime_text" | "zed" => (vec![joined(true)], true),
        "code" | "code-insiders" | "codium" | "cursor" | "windsurf" => {
            (vec!["-g".into(), joined(true)], true)
        }
        "notepad++" => (vec![format!("-n{line}").into(), file], true),
        _ => (vec![file], false),
    }
}

#[cfg(test)]
mod tests {
    use super::editor_args;
    use std::path::Path;

    fn args(program: &str, at: Option<(u32, Option<u32>)>) -> (Vec<String>, bool) {
        let (a, passed) = editor_args(program, Path::new("/s/a.rs"), at);
        (
            a.iter().map(|s| s.to_string_lossy().into_owned()).collect(),
            passed,
        )
    }

    #[test]
    fn each_editor_is_told_the_line_its_own_way() {
        let at = Some((42, Some(7)));
        assert_eq!(
            args("nvim", at),
            (vec!["+42".into(), "/s/a.rs".into()], true)
        );
        assert_eq!(args("/usr/bin/vim", at).0[0], "+42");
        assert_eq!(args("hx", at), (vec!["/s/a.rs:42:7".into()], true));
        assert_eq!(
            args("code", at),
            (vec!["-g".into(), "/s/a.rs:42:7".into()], true)
        );
        assert_eq!(args("Code.exe", at).0[0], "-g");
        assert_eq!(args("notepad++.exe", at).0[0], "-n42");
        assert_eq!(args("code", Some((3, None))).0[1], "/s/a.rs:3");
        // Unknown: the file alone, and said so.
        assert_eq!(args("notepad", at), (vec!["/s/a.rs".into()], false));
        // No line asked for: the file alone, nothing to say.
        assert_eq!(args("notepad", None), (vec!["/s/a.rs".into()], true));
    }
}
