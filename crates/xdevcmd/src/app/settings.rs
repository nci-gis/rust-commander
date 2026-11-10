//! Settings → the dialog, saving it, and the theme menu.

use super::*;

impl App {
    /// File → Settings….
    pub(super) fn open_settings(&mut self) {
        let cannot_save = if self.settings.mode.read_only() {
            Some(format!(
                "{} mode \u{2014} xdevcmd writes nothing, its settings included",
                self.settings.mode.label()
            ))
        } else if let Outcome::Rejected { .. } = self.config_outcome {
            Some(
                "config.toml has an error (F1 \u{203a} Settings says which): fix it by hand first"
                    .into(),
            )
        } else {
            None
        };
        self.modal = Some(Modal::Settings(Box::new(SettingsDialog::new(
            &self.settings.config,
            crate::config::available_themes(),
            cannot_save,
        ))));
    }

    /// Enter in Settings: write what changed into the config file, keeping the rest of
    /// it as it was, then use the new settings. The edited file is read back first, and
    /// nothing is written unless it says exactly what the dialog showed.
    pub(super) fn save_settings(&mut self) {
        let Some(Modal::Settings(d)) = self.modal.take() else {
            return;
        };
        let changes = d.changed();
        if changes.is_empty() {
            self.status = "settings: nothing changed".into();
            return;
        }
        // The dialog offers no Save in these modes; refused here too, not on its word.
        if self.settings.mode.read_only() {
            self.status = format!(
                "{} mode \u{2014} xdevcmd writes nothing, settings included",
                self.settings.mode.label()
            );
            return;
        }
        let Some(path) = crate::config::path_in_use() else {
            self.status = "nowhere to save: no config location (HOME is not set)".into();
            return;
        };
        // A config.toml that is a link (dotfiles managed with stow, chezmoi) is
        // written where it points, so the link stays a link. Through `paths::canonical`:
        // std's gives Windows' verbatim form (`\\?\C:\…`), which then showed in the
        // status line and in F1's Settings page.
        let path = xdevcmd_core::paths::canonical(&path)
            .or_else(|_| {
                // A link whose target does not exist yet (stow before the first save):
                // write the target, so the link stays.
                std::fs::read_link(&path).map(|t| match path.parent() {
                    Some(dir) if t.is_relative() => dir.join(t),
                    _ => t,
                })
            })
            .unwrap_or(path);
        let before = match std::fs::read(&path) {
            Ok(b) => match String::from_utf8(b) {
                Ok(t) => t,
                Err(_) => {
                    self.status = format!("{} is not UTF-8: change it by hand", path.display());
                    return;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                self.status = format!("cannot read {}: {e}", path.display());
                return;
            }
        };
        let mut text = before;
        for (f, v) in &changes {
            text = crate::config_edit::set(&text, f.section, f.key, &settings_dialog::value(f, v));
        }
        let parsed = match crate::config::parse(&text) {
            Ok(s) if s.config == d.now => s,
            Ok(_) | Err(_) => {
                self.status = format!(
                    "{} is written in a way Settings cannot change safely: change it by hand",
                    path.display()
                );
                return;
            }
        };
        if let Err(e) = session::write_private(&path, text.as_bytes()) {
            self.status = format!("cannot save {}: {e}", path.display());
            return;
        }
        // The new settings, keeping what start-up decided: the mode (a flag may have
        // made it stricter), the root, the colour depth.
        let mut s = parsed;
        s.mode = self.settings.mode;
        s.root = self.settings.root.clone();
        s.depth = self.settings.depth;
        crate::pane::files::set_fuzzy(s.config.files.filter == "fuzzy");
        xdevcmd_core::ops::set_safe(s.config.general.safe_operations);
        if self.interactive {
            self.graphics_found = crate::graphics::detect(
                &s.config.viewer.images,
                |k| std::env::var(k).ok(),
                &self.probe,
            );
            self.graphics = crate::graphics::effective(self.graphics_found, self.cell_known);
        }
        let scrollback = s.config.terminal.scrollback;
        if let Some(p) = self.panel() {
            p.set_scrollback(scrollback);
        }
        self.settings = s;
        self.config_outcome = Outcome::Loaded(path.clone());
        let name = self.settings.config.theme.name.clone();
        self.set_theme(&name);
        let later = d.need_restart();
        self.status = if later.is_empty() {
            format!("settings saved to {}", path.display())
        } else {
            format!(
                "settings saved; at the next start: {}",
                later.join(", ").to_lowercase()
            )
        };
    }

    /// View → Theme…: every theme, built in and from `themes/`, the current one marked.
    pub(super) fn theme_menu(&mut self) {
        let current = self.theme_name.clone();
        let items: Vec<Item> = crate::config::available_themes()
            .into_iter()
            .map(|name| {
                let mark = if name == current { "\u{2022}" } else { "" };
                Item::cmd(name.clone(), mark, Cmd::SetTheme(name), true)
            })
            .chain(std::iter::once(Item::cancel("Cancel", "Esc")))
            .collect();
        self.modal = Some(Modal::Menu(Menu::new(
            Some("Theme".into()),
            items,
            self.pane_anchor(),
        )));
    }

    /// Switch themes now. The colours set in `[theme]` stay on top, as at start-up.
    pub(super) fn set_theme(&mut self, name: &str) {
        let s = &self.settings;
        match crate::config::theme_for(name, &s.config.theme, s.depth) {
            Ok(t) => {
                theme::set(t);
                self.theme_name = name.to_string();
                self.status = format!(
                    "theme: {name} \u{2014} [theme] name = \"{name}\" in config.toml keeps it"
                );
            }
            Err(e) => self.status = e,
        }
    }
}
