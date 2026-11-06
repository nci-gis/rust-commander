# Config

> **Read this when**: adding or changing a setting in `config.rs`, or how a config file is found, validated or rejected.

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
