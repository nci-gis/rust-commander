# Config

> **Read this when**: adding or changing a setting in `config.rs`, how a config file is found, validated or rejected, theme files, or how Settings saves into `config.toml`.

---

## Config

`config.toml`, looked for at `$RCMD_CONFIG`, `$XDG_CONFIG_HOME/rcmd/config.toml`,
`%APPDATA%\rcmd\config.toml` on Windows, then `~/.config/rcmd/config.toml` (macOS too —
terminal tools live there, not in `~/Library`).

- **Every field is optional.** A one-line file is valid; the rest stays default.
- **A wrong file is rejected whole**, never half-applied: bad TOML, a misspelt key
  (`deny_unknown_fields`), a colour that is not a colour, an unknown action under
  `[keys]`. rcmd starts on the defaults and the status line says `config ignored — …`
  with the field named. A config applied halfway is harder to reason about than one not
  applied at all.
- **Listing an action under `[keys]` replaces all its default keys**, so the old key
  stops working. Otherwise a rebind leaves two keys live and the config is lying.
- **Deleting always confirms.** There is deliberately no setting for it (hard rule 3).
  `confirm_transfer` only skips the question *before* a copy or move starts; overwrites
  still ask mid-job.
- `--print-config` output must pass `--check-config`; a test holds that, so the printed
  template can never drift from the schema.

## Themes

`[theme] name` picks a built-in theme (`theme::BUILT_IN`: `dark`, the default, `light`,
`ansi`, `blue`, `high-contrast`) or a file `themes/<name>.toml`. Files are looked for
(`config::theme_dirs`) beside **the config file in use**, then beside the rcmd binary
(a portable copy). A file has `base` (a built-in, default `dark`) and any of the
`[theme]` colour names; colours set in `[theme]` itself go on top of whichever theme is
chosen. View → Theme… switches while running (`theme::set`; `t()` returns a copy, so a
frame never mixes two themes); `name` in the config keeps the choice.

- **`--config` moves the directory.** `load` records the directory of the file it
  actually read (`CONFIG_DIR`). Theme files used to be looked for beside the *default*
  config even under `--config other.toml`; found because the docs test runs every theme
  example (a block whose first line is `# themes/x.toml`) through `--config`.
- **A file cannot take a built-in name**, so tests and docs never depend on what a user
  keeps in their directory. A name with `/`, `\` or a leading dot is refused before any
  file is read. A bad colour names the file; an unknown name lists the themes.
- At 16 colours the default `dark` gives way to `ansi`; a theme the user chose is kept
  and fitted to the depth.

## How Settings writes the file

File → Settings… saves into the file `load` chose (`config::path_in_use()`, the
`--config` one included), creating it if missing. No TOML-editing dependency:
`config_edit::set` edits the text.

- Only the line holding a changed key is touched, its trailing comment kept. A missing
  key goes after the last key of its section (not after a comment that introduces the
  next section), a missing section at the end. Comments (a `#` inside a string is not
  one), order, blank lines and CRLF are kept; `[[tables]]` end a section.
- **The read-back rule**: the edited text is parsed with `config::parse` and must equal
  exactly what the dialog shows, or nothing is written and the status line says to edit
  the file by hand. A file in a style the line editor does not follow (dotted keys,
  inline tables) fails safe this way; no test uses such a file yet.
- Written atomically and owner-only (`session::write_private`). Refused in read-only and
  restricted modes, and while the file has an error.
- After saving, the new settings apply at once, keeping start-up's mode, root and colour
  depth. Settings read on every use take effect now; those read at start-up (hidden
  files, sort, icons, `LS_COLORS`, git, colour depth, terminal panel) say "next start".
- Every choice a row offers must be a value `config::parse` accepts, and each row's
  default must be among its choices: "Shells at most" could not be set back to 3 until a
  test checked all of them.
