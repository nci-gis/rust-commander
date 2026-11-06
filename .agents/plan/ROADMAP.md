# Roadmap

> What comes after 0.1.0, in order, and why that order. Each line becomes a round in
> `cycles/` when work on it starts; a round says how, this file says what and when.
> Reorder freely; strike through what is dropped rather than deleting it.

Written 2026-11-06 from feedback on 0.1.0, a market study the same day (five research
threads, kept outside the repository) and the owner's review of both. The feedback:
Windows and Ubuntu look fine; on macOS the colours are wrong, Alt shortcuts and some
other keys do nothing, and scrolling shows the terminal's history; Total Commander
users want Enter to open files.

---

## Direction

- **rcmd's edge is a combination, not a feature**: MIT, the Total Commander model,
  native Windows, one static binary to drop on a server, no network, nothing destructive
  done silently. Nothing below thins it.
- **Make it pleasant before making it bigger.** macOS is a third of the platforms and
  is broken in three ways; themes and a Settings screen are what a first-time user
  touches. These come before any large feature.
- **Plugins are for optional extras** (uncommon or nice-to-have features), not the
  core. Low priority, and no in-app fetcher.
- **No SFTP** (owner, 2026-11-06): `ssh` in the terminal panel and the many good
  existing tools cover it.

## Order

| # | Item | Size | Round | Status |
|---|---|---|---|---|
| **Now: fix what is broken** | | | | |
| 1 | Colours on terminals without 24-bit colour (macOS Terminal.app) | S | [001](cycles/Round_001.md) | Review: works on the owner's Mac; build-464 cut-off unconfirmed |
| 2 | macOS keys and mouse: Alt shortcuts, other keys, scrolling into the terminal's history | S–M | [003](cycles/Round_003.md) | In Progress: keys done, mouse open |
| **Next: Total Commander feel** | | | | |
| 3 | Enter opens a file with the program its extension belongs to, like Total Commander | M | [002](cycles/Round_002.md) | Planning |
| 4 | Themes as files: built-in themes plus the user's own, chosen by name, switchable while running | M | — | |
| 5 | Keymaps as files: built-in sets (`classic`, `laptop`, `mc`) plus the user's own, chosen by name | M | — | same mechanism as 4 |
| 6 | Settings (File → Settings…): tabs General, Theme, Keys, Panels and layout | L | — | after 4 and 5 |
| 7 | The logo in the app: About screen (and the empty state) drawn with the picture renderer | S | — | |
| 8 | One name, then a Homebrew tap, scoop, winget, `cargo binstall`, AUR | S each | — | needs the name decision |
| 9 | Undo for the last operation | M | — | |
| 10 | Launch: Show HN with a GIF | S | — | after 1–8 |
| **Later: optional extras** | | | | |
| 11 | File icons as an optional extra: icon sets per extension, Nerd Font or plain | S–M | — | builds on 4 |
| 12 | Plugins v0: the user menu with documented variables and the selection, run in a tab | S–M | — | low priority |
| 13 | Plugins: external programs over stdin/stdout, versioned | L | — | low priority, only for a need 12 cannot meet |
| ~~—~~ | ~~SFTP, remote panes, a file-source layer for remote writes~~ | — | — | dropped 2026-11-06 |

Sizes are relative: **S** a day or two, **M** under a week, **L** one to two weeks.

## Notes on the items

- **4 Themes.** `themes/<name>.toml` in the config directory, each a full palette in
  the slots `--print-config` lists; `[theme] name = "…"` picks one and the existing
  per-colour settings still apply on top. Built in: `dark` (today's), `light`,
  `ansi` (the 16 terminal colours, so it follows the user's palette), `blue` (blue
  panels in the tradition of the DOS-era commanders), `high-contrast`. A theme also
  says how it degrades at 256 and 16 colours (item 1). Today the theme is set once at
  start-up and "never changes while running"; switching needs that to change.
- **5 Keymaps** (owner's idea, 2026-11-07: for what a Mac keyboard cannot fix).
  `keymaps/<name>.toml` in the config directory; `[keys] preset = "…"` picks one and
  any key named under `[keys]` still overrides it, as today. Built in, named for who
  they suit:
  - `classic` (default): today's keys, F-keys as in Total Commander.
  - `laptop`: no F-key or Alt needed. Every main action also on Ctrl+letter or a
    plain letter in the file panes, for Mac laptops and compact keyboards.
  - `mc`: Midnight Commander habits, taken from its manual and from using it, never
    from its source (GPL).

  F1, the key bar and the menus already read the one table (`BINDINGS`), so they show
  whichever set is in force. A test per set: no key runs two actions, and none uses a
  key a terminal or shell keeps for itself (Ctrl+C, Ctrl+Z, Ctrl+D; Cmd never reaches
  a terminal program on macOS).
- **6 Settings.** Tabs: **General** (confirmations, trash, session, Esc as Meta),
  **Theme** (pick a theme and the colour depth, with a live preview), **Keys** (pick a
  keymap; the list of keys, editing them later), **Panels and layout** (how many file
  panes, the terminal panel). Security shows the mode but is not editable from inside,
  or a restricted session could lift its own restriction. Writes `config.toml` keeping the user's comments and order. Refused in
  read-only and restricted modes, where rcmd writes nothing.
- **7 Logo.** Quick view already draws PNG and JPEG with half blocks, two pixels a cell,
  in any terminal: the icon can be an embedded PNG drawn the same way. At 256 colours
  it is quantised like everything else.
- **11 File icons** already exist as `[files] icons = true` (needs a Nerd Font). The
  extra is choosing the set (Nerd Font, plain Unicode, none) and per-extension icons in
  a theme-like file.

## Decisions

The owner accepted every recommendation below on 2026-11-06 ("mình đồng ý theo đề xuất
của bro"). What the one name is remains open.

| Decision | Options | Decided |
|---|---|---|
| Colour `auto` | downgrade only old Terminal.app / truecolor only on positive evidence | positive evidence; 256 colours over SSH unless `COLORTERM` arrives |
| Default theme | `dark` (today) / `ansi` | keep `dark`; revisit after the launch |
| Enter on a file | `view` / `open` (Total Commander) | `open` by default, falling back to view where nothing can be opened; `view` by setting |
| Where theme files live | config directory / also beside the binary (portable, as Total Commander's ini) | config directory; also read `themes/` beside the binary for portable installs |
| Name | keep `rcmd` + `rust-commander` / one name | one name before Homebrew and the launch |
| Extension points in secure modes | — | off in read-only and restricted unless whitelisted |

## Not planned

- **SFTP and remote file systems** (see Direction). Round 003's first draft, a spike
  for this, was replaced by the macOS round before it started.
- **Fetching anything** — a plugin list, updates, telemetry. Hard rule 2 stays as it is.
- **Total Commander's native plugins** (WFX/WCX DLLs) and **embedded Lua**: the first
  needs `unsafe` and runs Windows-only binaries in-process; the second runs plugin code
  in-process with no boundary.
