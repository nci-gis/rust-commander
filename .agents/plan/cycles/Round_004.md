# Round 004: Key sequences, and built-in keymaps for each system

**Status**: Planning
**Part of**: [ROADMAP.md](../ROADMAP.md), item 5
**Date started**: 2026-11-07
**Date completed**:

## Goal

Every action has a key that actually reaches rcmd on Windows, macOS and Linux, in the
terminal people use there by default, and the user picks a whole set of keys by name
(later from Settings → Keys) instead of rebinding key by key.

## Why

Round 003 measured macOS Terminal.app: with Option as Meta, ⌥← / ⌥→ arrive as Alt+B /
Alt+F, but ⌥↑ / ⌥↓ arrive as three separate keys, Option+Shift and Ctrl+Shift drop
their modifiers on arrows, so history (Alt+↓) and resize (Alt+Shift+arrows) cannot be
typed there at all. The owner wants both: a way to type them anywhere (key sequences),
and sets of keys tuned for each system that can be chosen (2026-11-07).

Each terminal keeps some keys for itself. Known or believed, **to measure, not
assume**:

- **macOS Terminal.app**: Cmd+anything; Ctrl+arrows (Mission Control, Spaces); ⌥← / ⌥→
  remapped to word left / right; Fn+↑ / ↓ may scroll the window. Measured in Round 003.
- **Windows Terminal** (the default console on Windows 11): believed to keep Alt+arrows
  (move focus between its panes), Alt+Shift+arrows (resize its panes), Alt+Shift+- / +
  (split), Alt+Enter and F11 (full screen), Ctrl+Shift+T / W / number (tabs),
  Ctrl+Tab. If so, rcmd's Alt+← / → and Alt+Shift+arrows never arrive there either.
- **GNOME Terminal / Ptyxis / Konsole** (Ubuntu and others): believed to keep F10
  (menu) and F11 (full screen), Ctrl+Shift+letter (copy, paste, tabs), and in some,
  Alt+1…9 (tab switch). F10 is rcmd's Quit.

## Design

### 1. Key sequences

- A binding may be a sequence: `"Alt+r Left"` = press Alt+R, let go, then ←. Esc as
  Meta composes with it: Esc R ← is Alt+R ←.
- **Groups repeat.** Sequences that share a first key form a group. After the first
  key, any key of the group runs its action and the group stays open, so ← ← ← ←
  resizes four steps. The group closes on Enter or Esc, a key outside it, or two
  seconds of nothing. While it is open the status line lists the group: `resize: ← → ↑
  ↓ · Enter done`.
- Defaults, on top of today's keys: resize = `Alt+r` then arrows. History gets a
  second key that every default terminal passes, picked from the measurements (step 1).

### 2. Keymaps

- **Built in, named by where they fit:**
  - `windows`: tuned to Windows Terminal (and conhost).
  - `macos`: tuned to Terminal.app (and so also fine in iTerm2, Ghostty).
  - `linux`: tuned to GNOME Terminal / Ptyxis / Konsole.
  - `classic`: today's keys exactly, the same everywhere, Total Commander first.
- **`auto`** (the default): the one for the system rcmd runs on. Over SSH that is the
  server's system, not the one in front of you; the docs say so and suggest setting it.
- They share one base (`classic`) and differ only where a terminal keeps a key: a set
  is the base plus a short list of replacements, so the three cannot drift apart.
- **The user's own**: `keymaps/<name>.toml` in the config directory, the same shape as
  `[keys]` plus `base = "macos"` to start from one.
- `[keys] preset = "macos"` picks one; any action named under `[keys]` still replaces
  that action's keys, as today. `RCMD_KEYS=macos` in the environment overrides the
  preset, per machine, as `RCMD_COLORS` does for colours.

### 3. Conflicts, checked when the config loads

| Case | Result |
|---|---|
| An old config with single keys only | Works unchanged. |
| One action, several keys or sequences | Fine. |
| Sequences sharing a first key | Fine: that is a group. |
| A key bound to an action *and* the first key of a sequence | Both written by the user: an error naming both. One from a built-in set: the user's key wins, the set's sequences starting with it are dropped, and F1 says so. |
| One key (or sequence) for two actions | Both written by the user: an error. A user key that a set also uses: the user wins (rebinding). **Today rcmd lets the later action silently take the key; that changes.** |
| A built-in set | A test per set: no key runs two actions, no prefix clash, and no key the measurements found a terminal keeps for itself. |

### 4. Where it shows

F1, the key bar, the menus, `--print-keys` and `docs/keys.md` already read one table
(`BINDINGS` via the keymap); they show the set in force, sequences as `Alt+R ←`.
`--show-keys` prints the set in use. Settings → Keys (ROADMAP item 6) picks the set.

## Plan

- [ ] 1. **Measure** with `rcmd --show-keys`, in each system's default terminal: every
      key rcmd binds today, plus candidates for replacements. macOS done in Round 003
      (to finish: Shift+↑ / ↓, F-keys with and without Fn). Windows Terminal and one
      Linux terminal: the owner. Record the tables here.
- [ ] 2. Sequences and groups in `keys.rs` / `App::on_key`, with the conflict checks.
- [ ] 3. Keymaps: the base, the three replacement lists from step 1, `auto`, user files,
      `[keys] preset`.
- [ ] 4. F1, key bar, menus, `--print-keys`, `--show-keys`; `docs/keys.md` per set;
      `docs/config.md`; `docs/troubleshooting.md` points to the set for each system.
- [ ] 5. CHANGELOG. On macOS, the first start suggests nothing extra once `auto` picks
      `macos` by itself; the Option hint stays.

Work on a `feat/keymaps` branch; CI runs on its pull request into `dev`.

## Risks

- **Changing someone's keys under them.** `auto` switches existing users on Windows,
  macOS and Linux from today's keys to their system's set. Only keys a terminal keeps
  should differ, so what changes is mostly keys that never worked; anything else is a
  bug in the set. The CHANGELOG lists every difference, and `preset = "classic"` brings
  back today's keys exactly.
- **Over SSH, `auto` is the server's system.** A Mac user on a Linux server gets
  `linux`. Docs and the hint say to set `preset = "macos"` there, or `RCMD_KEYS`.
- **Terminals beyond the default one** (iTerm2, Ghostty, WezTerm, kitty, Alacritty,
  tmux) keep fewer keys; the sets are tuned to the defaults, which keep the most.
- **Help text that names keys** must come from the keymap, never be written by hand
  (already the rule: `context/project.md`, "one place for each fact").

## Do

## Check

- [ ] The measurement tables for all three systems are here, with what was not tested.
- [ ] Tests: sequences, groups (repeat, Enter / Esc / timeout / other key closes), every
      conflict case in the table, one per built-in set, user files, `auto` per OS.
- [ ] Render tests: F1 and the key bar under each set; a sequence on the status line.
- [ ] The owner, on each system, in its default terminal: every key on F1 works.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/ui.md`: keymaps, sequences, the conflict rules
- [ ] → `reference/platforms.md`: the keys each default terminal keeps
