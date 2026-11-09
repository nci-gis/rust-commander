# Round 015: The terminal's history in the viewer

**Status**: In Progress
**Part of**: [ROADMAP.md](../ROADMAP.md), item 26; Steps 1–4 of "Plan for the terminal
panel" in [Round 014](Round_014.md)
**Date started**: 2026-11-10
**Date completed**: —

## Goal

A shell's output can be read, searched, copied and saved without the outer terminal's
help: Shift+F12 opens the focused shell's history in the F3 viewer, the viewer gains Save
as… and Go to line, the history length becomes a setting, and a terminal tab gets a
small right-click menu. These are the answers tmux gives to the same limits (copy mode,
`history-limit`), applied to what xdevcmd already has.

## Plan

Branch `feat/terminal-history`, from `feat/image-preview` at Round 014's Review
commit (owner, 2026-11-09: rounds run one after another, chained). Steps 1–4 exactly
as agreed on 2026-11-09; Step 5 (logging) stays out.

- [ ] **0. Keymap A: the shell owns its keys** (owner, 2026-11-09: "Option A"; no users yet,
      so changed now). With the terminal panel focused, xdevcmd takes **only the F12 family**:
      F12 (focus, as today), Shift+F12 (history view, step 1), and their Alt+X forms. Every
      other key in `is_global` today (Alt+D, Alt+C, Alt+T, Alt+N, Alt+1…9, the Alt+R group, and
      any other — read `is_global` and list them in Do) stops being global: it works from the
      panes only, so from the shell it is F12 then the key. Mouse on xdevcmd's own chrome (tab
      bar, borders, + button) unchanged. Test that makes the rule checkable: with the panel
      focused, every Alt+letter, Alt+digit and Ctrl+letter except the F12 family reaches the
      PTY (bytes read with `stty raw; od` as in Round 014's test), and none fires an xdevcmd
      action. docs/keys.md, Help (F1) and docs/shell.md say the rule in one line.
- [ ] **1. History view (copy mode).**
  - Core: `Document` gets a second source, bytes in memory, beside the file
    (`Document::from_bytes(title, Vec<u8>)`; a private `Source { File(File), Memory(Vec<u8>) }`
    under `read_up_to`, `bytes_at` and the line index). Nothing of a shell's output touches
    the disk. Line index, search and hex work the same on either. `path` stays the display
    name; `pub in_memory: bool` tells the viewer (Save as, step 2) what it holds.
  - tui: `term.rs` `history_text(&mut vt100::Parser) -> String`, a pure function. Walks
    scrollback offsets from the top down to 0, takes each page's visible rows, joins a row
    onto the next where `row_wrapped` says the line went on. Trailing blanks cut. Original
    offset restored. Runs under the parser's write lock; measure on 9999 × 200 and record.
  - `Action::TerminalHistory`, **Shift+F12** (owner, 2026-11-09, keymap A: inside the shell
    xdevcmd keeps only the F12 family), plus **Alt+X F12** through the existing Alt+X group
    (Terminal.app sends Shift+F12 as F20 — already taken as meant). Works with the shell
    focused. Also reachable from the panes on the last-focused shell. Opens the focused
    shell's text in the viewer at the bottom; title
    "history · <shell> · tab N". Esc goes back to the shell with focus where it was.
  - Tests: unit — bare parser with 10 rows fed `1\r\n…300\r\n` gives exactly 300 lines in
    order, none missing/doubled at page joins; a 250-char line in 80 cols comes back as one
    line. Core: memory document indexes, searches, shows hex as a file with the same bytes.
    Pty: `seq 1 300`, Shift+F12; viewer shows `300` at the bottom and `1` after Home; drag then `y`
    sends OSC 52 with those lines; Esc, typing reaches the shell.
- [ ] **2. Save as… and Go to line in the viewer**, for every document.
  - **F2 Save as…** (free in the viewer). Prefilled `<dir>/xdevcmd-history-<date>-<time>.txt`
    for history, the file's own name for a file. `<dir>` = shell's directory where known
    (`TermPane::cwd`), else the focused pane's. Through both walls: `App::refused` (read-only
    and restricted refuse) and core `policy::guard_write`. An existing name asked about first,
    same question as copy. A **memory** document is written by core
    (`document::save_bytes(path, bytes, replace, private)`): new file with `create_new` and
    mode `0600` on Unix; a replacement through `part_name` then rename. A **file** document is
    saved by an ordinary copy job (`Request::copy` with `rename_to`).
  - **Go to line**: `:` and Ctrl+G open a prompt on the viewer's last row. Takes `40`, `50%`,
    and in hex view an offset (`0x1f00` or decimal). Past the end → end, says so. The Ctrl+G
    arm comes **before** the existing `Char('g')` (Home) arm.
  - Tests: render — `:` `40` Enter puts line 40 at the top; `50%`; Ctrl+G is not Home; hex
    offset. Pty: F2 on history writes the file (0600 checked); F2 to the same name asks, Esc
    keeps the first; read-only mode refuses F2 with the usual message. Core: `save_bytes`
    never replaces without `replace`; a cancelled replacement leaves the old file.
- [ ] **3. `[terminal] scrollback`.** Default 5000, 100..=9999. Out of range refused with a
      message naming the range. Settings → Terminal with the cost when full, from panel width
      and `size_of::<vt100::Cell>()`; says it applies to shells started from now on. Passed to
      `TermPane::spawn`. Tests: config bounds (99, 10000 refused; 100, 9999 taken),
      `--print-config` shows it; a shell started with 200 keeps 200 lines.
- [ ] **4. The menu on a terminal tab.** Right-click on a tab in the panel's tab bar
      (`tab_hits`). Inside the shell, right-click stays the program's. Three items with keys:
      **View history** (Shift+F12), **Save history as…**, **Close tab**. Act on the tab clicked,
      not the focused one. Not in it: new tab/split, Paste, Clear history. Tests: render,
      `rclick` on the second of two tabs shows three items; Close tab closes that tab and
      leaves the focused one; View history opens the second tab's text.
- [ ] **5. Docs and hand-over.** docs/keys.md regenerated (`--print-keys`), docs/config.md,
      the guide, CHANGELOG Unreleased. Status → Review; set aside; Round 016 starts from it.

**Risks**
- Keymap A is a behaviour change for anyone used to Alt+N/Alt+D from the shell (no users
  yet, owner). F-keys are taken by some full-screen programs (mc, htop use F1–F10; F12 rarely).
- vt100's scrollback API is page-based; joins are where lines get lost or doubled.
- The write lock while walking history pauses the shell's reader; measured.
- Save-as prefill knows the shell's directory on Linux only until Round 016.

**Files likely touched**: `crates/xdevcmd-core/src/document.rs`, `crates/xdevcmd-tui/src/viewer.rs`,
`pane/term.rs`, `pane/terms.rs`, `app.rs`, `keys.rs`, `config.rs`, `settings_dialog.rs`,
`menu.rs`, `tests/render.rs`, `tests/pty.rs`, `docs/keys.md`, `docs/config.md`,
`docs/guide.md`, `CHANGELOG.md`.

**Not doing**: Step 5 logging; drag-select in the live shell; Shift+PgUp/PgDn/Home/End for
history; Paste or Clear history in the menu; a second viewer; any temporary file for history.

## Do

- 2026-11-10, round opened on `feat/terminal-history` from `feat/image-preview` at
  Round 014's Review commit (`6ecac43`). The owner's decisions of 2026-11-09 are in the
  plan above: keymap A (step 0), the history view on **Shift+F12**, not Alt+V (the plan
  text was changed to say so, with the owner's leave to edit it), and the principle that
  xdevcmd complements the shell and never competes: a program in the panel gets its keys
  and its mouse.

## Check

- [ ] fmt, clippy, `cargo test --workspace`, `cargo deny check`.
- [ ] 300 lines out exactly as 300 in; a wrapped line comes back whole.
- [ ] Nothing of the history written anywhere until Save as….
- [ ] Save as… refuses in read-only/restricted, asks before replacing, writes 0600.
- [ ] `:`/Ctrl+G, with Ctrl+G not taken as `g`.
- [ ] Scrollback bounds and the memory figure in Settings.
- [ ] The tab menu acts on the tab clicked.
- [ ] By hand, owner: Shift+F12 with Claude Code/htop in the panel; OSC 52 copy in Windows
      Terminal and iTerm2; Save as… where OSC 52 is missing.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : —
