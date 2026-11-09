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

- [x] **0. Keymap A: the shell owns its keys** (owner, 2026-11-09: "Option A"; no users yet,
      so changed now). With the terminal panel focused, xdevcmd takes **only the F12 family**:
      F12 (focus, as today), Shift+F12 (history view, step 1), and their Alt+X forms. Every
      other key in `is_global` today (Alt+D, Alt+C, Alt+T, Alt+N, Alt+1…9, the Alt+R group, and
      any other — read `is_global` and list them in Do) stops being global: it works from the
      panes only, so from the shell it is F12 then the key. Mouse on xdevcmd's own chrome (tab
      bar, borders, + button) unchanged. Test that makes the rule checkable: with the panel
      focused, every Alt+letter, Alt+digit and Ctrl+letter except the F12 family reaches the
      PTY (bytes read with `stty raw; od` as in Round 014's test), and none fires an xdevcmd
      action. docs/keys.md, Help (F1) and docs/shell.md say the rule in one line.
- [x] **1. History view (copy mode).**
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
- [x] **2. Save as… and Go to line in the viewer**, for every document.
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
- [x] **3. `[terminal] scrollback`.** Default 5000, 100..=9999. Out of range refused with a
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

- 2026-11-10, **step 0, keymap A.** `is_global` before this round held 16 actions, each
  of them xdevcmd's even when typed into the shell: `focus_terminal` (F12),
  `toggle_terminal` (Alt+T), `cd_terminal_here` (Alt+C), `new_terminal` (Alt+N),
  `next_tab` / `prev_tab` (Ctrl+PgDn / Ctrl+PgUp), `go_tab` (Alt+1…9), `next_split`
  (Alt+O), `split_right` (Alt+S), `split_down` (Alt+D), `close_pane` (Alt+W),
  `resize_left/right/up/down` (Alt+Shift+arrows, and the whole Alt+R group, which
  opened in the shell because every action in it was global), `zoom` (Alt+Z). Now
  `is_global` is F12's family only: `focus_terminal` and the new `terminal_history`
  (Shift+F12, Alt+X F12; the view itself is step 1, a status line says so until then).
  The other 15 are `Action::acts_on_shell`. F13–F20 were handled nowhere: nothing bound
  them, so Terminal.app's Shift+F5…F12 did nothing (Alt+X Fn is the answer there).
  **Deviation, why:** the plan says "from the shell it is F12 then the key". In the code
  F12 moves focus to a file pane, and split, close, next split, resize, zoom, tabs and
  go-tab act on *whatever has focus*, so F12 then Alt+S splits a file pane: those 15
  could no longer reach a shell by any key. The sound way inside keymap A: Alt+X, which
  keymap A already takes for Alt+X F12, is xdevcmd's prefix in the shell. Alt+X then a
  key of an `acts_on_shell` action runs it on the shell (Alt+X Alt+S splits it); Alt+X
  then any other key sends both to the program, so nothing is lost (Esc and Enter cancel,
  as in every group). No further key is taken from the shell. A group's first key opens
  in the shell only if the group holds a global action (`Keymap::shell_prefix`), so
  Alt+R is readline's revert-line again. Alt+R after Alt+X is not a nested group: from
  the shell, resizing is Alt+X Alt+Shift+arrow or a drag.
  **Also deviates, why:** `key_bytes` returned nothing for any F-key, so no F-key reached
  a program in the panel before (mc, htop). Keymap A's "every other key is the
  program's" needs them: F1–F12 now go as xterm sends them, modifiers as a parameter
  (`ESC [17;2~` is Shift+F6). Shift+F12 from Terminal.app arrives as `ESC [34~`, which
  crossterm 0.29 numbers **F17** (its table maps 31..=34 to F14..F17; xterm calls it
  F20), not F20 as the plan says: `normalise` takes a bare F17 or F20 as Shift+F12.
  Shift+F5…F11 cannot be mapped back that way, since crossterm gives two of their
  sequences one number. On rxvt, `ESC [34~` is Shift+F10, which now opens the history
  rather than the menu (Alt+X F10 still does); not checked on a real rxvt.
  Tests: pty `inside_the_shell_only_f12s_family_is_xdevcmds` (25 Alt+letters, Alt+1…9,
  Ctrl+A…Z, F1–F11 and Shift+F6, one write each, read back byte for byte by `dd` into a
  file; focus, tab count and splits unchanged) fails with the old global set put back
  (dd never gets its bytes); `alt_x_is_the_prefix…` (Alt+X q reaches the program as
  `ESC x q`; Alt+X Alt+N opens a tab); render `terminal_keys_typed_into_the_shell…`;
  unit tests for the global set, the prefix, F17/F20 and the F-key bytes. 20 render and
  4 pty tests that typed terminal keys into the shell now press Alt+X first. F1 and
  docs/keys.md say the rule in two sentences generated from the keymap; each
  `acts_on_shell` row in F1 says "in the shell: Alt+x first". docs/shell.md, the guide
  and CHANGELOG updated. `reference/ui.md` still describes the old global keys; it is
  read-only to agents, so it is left for the owner.

- 2026-11-10, **step 1, the history view.** Core: `Document` reads through a private
  `Source { File, Memory }` (`read_at`), so `line`, `bytes_at`, search and the line index
  are one code path for both; `Document::from_bytes(title, bytes)`, `pub in_memory`, and
  `memory_bytes()` for Save as (step 2). tui: `pane::term::history_text(&mut Parser)`
  walks the offset from the top of the history to 0 a screen at a time; row `i` of the
  page at offset `o` is row `history − o + i` of the whole and is taken only if it is
  the next one wanted, which is what keeps the overlapping last page from doubling
  lines. `TermPanel::history(tab)` gives the text and the title
  "history · <shell> · tab N"; `App::view_history` opens it in the viewer at its end
  (`Viewer::show_end`, resolved at the first draw when the height is known), with "N
  lines, in memory only — Esc back". The viewer is a dialog, so Esc returns with focus
  where it was. The status line in the shell now also says "Shift+F12 history", and
  View → Terminal history is in the menu bar.
  **Measured** (`history_text_timing`, ignored test, 50×200 screen, 9999 lines of
  history, one row each, 10,048 lines and 2.0 MB out): release 30, 38, 33 ms over three
  runs; debug 138 ms. That is how long the shell's reader waits on the write lock. Not
  worth moving off the drawing thread at this size.
  Tests: unit — 300 lines into a 10-row parser come back as exactly 1…300 with the view's
  offset put back; every history length 1…45 on a 7-row screen (every page remainder)
  exact; a 250-character line in 80 columns comes back as one line, also when a page
  boundary cuts it; empty and prompt-only screens. Taking the "next row" guard out fails
  the first two; ignoring `row_wrapped` fails the third. Core — a memory document and a
  file with the same 3000 lines agree on lines, bytes, hex rows and search; binary sniff
  and empty. Pty — `seq 1 300`, Shift+F12: line 301 is `300`, the prompt on the last
  row, no file under the scratch directory holds the output; Home shows line 1; a drag
  over seq's `1` and `2` and `y` writes `ESC ]52;c;MQoy BEL`; Esc Esc (the first lets go
  of the selection, as the viewer always did) and typing reaches the shell. Second pty
  test: Terminal.app's `ESC [34~`, Alt+X F12, and Shift+F12 from the files all open it.
  **Plan vs code:** the plan's pty check "`300` at the bottom and `1` after Home" holds
  with the command line counted: the history's line 1 is `$ seq 1 300`.
  Not done: a full-screen program on the other screen (htop) has no history in vt100;
  the view then shows that screen only. Not checked by hand with Claude Code or htop.

- 2026-11-10, **step 2, Save as… and Go to line.** Viewer: F2 returns `Action::SaveAs`;
  `:` and Ctrl+G open a prompt on the last row (`Prompt::{Search, Line}` replaces the
  search-only `typing`). The Ctrl+G arm sits before `Home | Char('g')`, which matched
  'g' whatever the modifiers. Text takes `40` or `50%`; hex takes `0x1f00`, a decimal
  offset or a share; past the end goes to the end and says so; anything else says what
  it takes. App: `begin_save_as` asks `App::refused(Action::CopyAs)` (saving creates a
  file, which is what Copy as… is refused for; no new Action, since every Action needs a
  key row and this one is the viewer's own F2), then keeps the viewer in `save_from` and
  opens the Input. Memory documents go to core `document::save_bytes(path, bytes,
  replace, private)`: `guard_write`, then `create_new` with mode `0600`; a name taken
  gets the copy's question ("Already exists", on disk / incoming sizes) as an Ask,
  Replace writes under `ops::part_name` and renames over. A file document is an ordinary
  copy job with `rename_to`. The viewer comes back once the dialogs are closed
  (`back_to_viewer`, after every modal action and after a clean job closes its own box);
  a file extracted from an archive for viewing is not removed while Save as… holds it.
  The prefill is `<dir>/xdevcmd-history-<date>-<time>.txt`, `<dir>` the shell's
  directory recorded when the history was opened (`TermPanel::history` now returns
  `TermPane::cwd` too), else the file pane's; under `--root` the same checks as Alt+F5.
  **Found and fixed on the way:** a clean copy closes its progress box from `pump_job`,
  not through `modal_act`, so the first try lost the viewer after saving a file;
  `f2_on_a_viewed_file…` caught it. **Also changed:** a message on the viewer's last row
  was cut off behind the key hints on a 100-column screen ("999 is past the end", the
  read-only refusal); keys are now dropped from the end, down to Esc, until the message
  fits.
  Tests: render — `: 40 Enter` puts line 40 on top, `Ctrl+G 50% Enter` line 150 of 300,
  `:999` the end and "past the end", `:x` refused, hex `0x40` and `64` both row
  `00000040`; Ctrl+G after two PageDowns asks and does not go to the top (fails with the
  Ctrl+G arm moved after `g`); F2 offers the file's name, Esc is back in the viewer,
  `--read-only` refuses with "creating files is turned off". Core — `save_bytes` never
  replaces without `replace` (the file untouched, `AlreadyExists`); a replacement
  leaves no part file; a failed replacement (a directory at the name) leaves it and no
  part; `0600` for a new file and for a replacement of a `0644` one; read-only refuses
  both (`tests/read_only.rs`). Pty — F2 Enter on history writes one
  `xdevcmd-history-*` in the shell's directory, `0600`, holding the output; F2 to that
  name asks, Esc keeps it byte for byte; read-only refuses and writes nothing; F2 on a
  viewed file copies it and the viewer comes back.
  Not verified: Windows (`rename` over an existing file is `MoveFileExW` with replace in
  std; a new file there gets its directory's permissions, no `0600`), not compiled here.

- 2026-11-10, **step 3, `[terminal] scrollback`.** `Terminal::scrollback`, default 5000,
  `config::SCROLLBACK` 100..=9999; out of range is refused naming the range
  ("expected 100 to 9999 lines"). Passed through `TermPanel::new` to every
  `TermPane::spawn` / `spawn_argv`; saving Settings sets it on the running panel
  (`set_scrollback`), so shells started after that keep the new length and running ones
  keep theirs. Settings → Terminal, "History lines per shell" (100, 1000, 2000, 5000,
  9999; a value written by hand stays among them, as for the other rows), takes effect
  at once for new shells; its help line is drawn from `scrollback_help(lines, cols)`:
  lines × the screen's width less the border × `size_of::<vt100::Cell>()`, which is
  **32 bytes** here, so 5000 lines at 120 columns shows "about 18 M a shell", and 9999
  at 120 is 36.6 MiB (the Round 014 plan's ≈ 38 MB was 10⁶ bytes).
  Tests: config — 99, 10000 and 0 refused naming the range, 100 and 9999 taken, 5000 by
  default; `--print-config` prints `scrollback = 5000` and still passes
  `--check-config`; the cost is lines × columns × a cell (unit); render — the Settings
  row and its "Full, about 18 M a shell at 120 columns … from now on"; pty — with
  `scrollback = 200`, `seq 1 500` then Shift+F12 holds 200–240 lines (history and the
  screen's rows) and Home no longer reaches the command line. docs/config.md and
  CHANGELOG.

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
