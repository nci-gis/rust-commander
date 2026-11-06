# Round 006: Themes as files

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), item 4
**Date started**: 2026-11-07
**Date completed**:

## Goal

Choose how rcmd looks by name: a few built-in themes, and the user's own as small files,
switchable while rcmd runs.

## Plan

- [x] 1. Built-in themes: `dark` (today's), `light`, `ansi`, `blue`, `high-contrast`.
- [x] 2. `[theme] name`; the per-colour settings in `[theme]` still apply on top.
- [x] 3. Theme files: `themes/<name>.toml` beside the config, then beside the binary
      (portable); `base` names a built-in, the rest are the `[theme]` colour names.
      Built-in names cannot be taken by a file, so tests and docs never depend on what
      a user has in their directory.
- [x] 4. Switch while running: View → Theme…. Saving the choice is Round 007's.
- [x] 5. Docs (config.md), CHANGELOG.

## Do

**2026-11-07.** Started while Round 005 was in review, on the owner's go-ahead to work
through to v0.2.0 on one branch (`feat/v0.2.0`, one pull request at the end, to spend
CI once).

- `theme.rs`: `Theme` is `Copy`; `BUILT_IN`, `built_in(name)`; the theme lives in a
  `RwLock` (`set`), and `t()` returns a copy, so a frame never mixes two themes.
- `config.rs`: `ThemeSpec.name`; `ThemeFile` (every colour optional, `base`); `theme_dirs`,
  `available_themes`, `named_theme`, `theme_for`; names with `/`, `\` or a leading dot
  are refused before any file is read. `Settings.depth` keeps start-up's colour depth.
- At 16 colours the default `dark` still gives way to `ansi`; a chosen theme is kept and
  fitted.
- `App`: View → Theme… lists the themes, marks the current one, `Cmd::SetTheme`.
- **Bug found by the docs test:** theme files were looked for beside the *default*
  config even under `--config other.toml`. `load` now records the directory of the file
  it actually reads (`CONFIG_DIR`). The docs test checks theme examples (a block whose
  first line is `# themes/x.toml`) through `--config`, so it covers this.

## Check

- [x] Every built-in theme keeps cursor rows, panel, bar, border and dim text apart at
      24-bit and 256 colours (`ansi` at 16).
- [x] Config: a name picks the theme, `[theme]` colours go on top, a path is refused.
- [x] Render: a theme file beside the config passes `--check-config` and appears in
      View → Theme…; a bad colour names the file; an unknown name lists the themes;
      choosing a theme switches it.
- [ ] How the new themes *look*: `--screenshot` is text only. Not seen in a terminal by
      anyone yet.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/config.md`: theme files and where they are looked for
