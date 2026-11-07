# Round 007: Settings

**Status**: Complete
**Part of**: [ROADMAP.md](../ROADMAP.md), item 6
**Date started**: 2026-11-07
**Date completed**: 2026-11-07 (closed by the owner: "các round đã okay nhé, bro flip to Complete")

## Goal

Change the common settings from inside rcmd, saved into the user's own `config.toml`
without losing anything they wrote there.

## Plan

- [x] 1. File → Settings…: tabs General, Files, Theme, Terminal, Keys.
- [x] 2. Each row a yes/no or a short list of choices that the config accepts; ←/→ or
      Space; the theme previews as it changes; Esc puts it back.
- [x] 3. Save into the file without a TOML library that would drop comments:
      `config_edit` changes only the changed keys' lines, adds missing keys at the end of
      their section; the edited text is parsed and must equal what the dialog shows, or
      nothing is written.
- [x] 4. Say what applies at once and what at the next start.
- [x] 5. Refused in read-only and restricted modes, and when the file has an error.
- [x] 6. Keys: a tab that says where keys are changed (`[keys]`, F1, `--print-keys`,
      `--show-keys`), not a second editor. Keymap sets stay deferred (Round 004).

## Do

**2026-11-07**, on `feat/v0.2.0`.

- `config_edit.rs`: `set(text, section, key, value)`; keeps comments (a `#` inside a
  string is not one), order, blank lines and CRLF; inserts after the section's last key
  rather than after a comment that introduces the next section; `[[tables]]` end a
  section.
- `settings_dialog.rs`: 21 settings; `get`/`set` per field; every choice is tested to be
  a value `config::parse` accepts, and every field to change exactly its own setting.
- `config::path_in_use()`: the file `load` chose (with `--config`, that one), where
  Settings saves; created if missing, written atomically, owner-only.
- After saving: the new settings are used at once, keeping start-up's mode, root and
  colour depth.
- Which apply at once was decided by reading where each setting is used: those read on
  every use (confirmations, trash, autosave, menu bar, Enter, refresh, pane and shell
  limits, viewer) apply now; those read at start-up (hidden files, sort, icons,
  LS_COLORS, git, colour depth, terminal panel) say "next start".

## Check

- [x] Unit: the editor (comments, new key, new section, same key in another section,
      strings with `#`, CRLF); the dialog's fields and choices.
- [x] Render: Settings saves two changes into a file with comments and a `[[menu]]`,
      keeps the rest byte for byte, and the file passes `--check-config`; read-only
      shows the reason and leaves the file untouched.
- [ ] Mouse in the Settings dialog: not handled (keys only).
- [ ] A config file written in an unusual TOML style (dotted keys, inline tables): the
      read-back check refuses to save; not tested with such a file.

## Act

**Unticked checks, and why the round closes anyway:** Settings takes no mouse input
(keys only); a config file in an unusual TOML style is refused by the read-back check
rather than tested. Both fail safe: nothing is written that does not read back as meant.

**Learnings**:

- **Edit the user's file line by line, then parse it back** and refuse if it does not
  say what was meant: comments and order survive without a TOML-editing dependency.
- **Every pick's default must be among its choices**: "Shells at most" could not be set
  back to its default, 3, until a test checked all of them (`feat/polish`).

**Promotions** (proposed; waiting on the owner):

- [x] → `reference/config.md`: how Settings writes the file, and the read-back rule
