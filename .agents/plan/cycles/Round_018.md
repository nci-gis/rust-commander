# Round 018: Windows asks before starting inside its panel; pid on the menu bar

**Status**: Review
**Part of**: v0.4.0 (`.agents/memory/2026-11-10-v0.4.0-plan.md`); a fix the owner found
before Round A, on `feat/v0.4.0`
**Date started**: 2026-11-10
**Date completed**: YYYY-MM-DD

## Goal

The owner, on Windows: "I cannot start xdevcmd inside its terminal on Linux/macOS, but on
Windows I can, in cmd or pwsh". Make Windows stop the accidental nested start, without
blocking a new window opened from the panel. Also: let the user see which xdevcmd is
which, and fix the status message drawn over the terminal's keys (seen in the same
screenshot: "Shift+F12 historystarted from the terminal panel…").

## Plan (kickoff review)

1. **Why 0.3.0 started on Windows.** It was deliberate. Round 014's review (R4) found that
   the variable alone refused new windows (`start cmd`, `wt`). Unix now compares tty names
   (rustix, no unsafe). Windows can only tell consoles apart through the console API,
   which `#![forbid(unsafe_code)]` rules out. A wrong refusal blocks the user; a nested
   start only confuses. So 0.3.0 started and noted it on the status line.
2. **Considered and dropped**: an exact check, with `GetConsoleProcessList` plus a
   Toolhelp snapshot (a process on my console whose parent is the panel's xdevcmd). It
   was written, then removed at the owner's question "unsafe code need to be careful?".
   It would be the crate's first unsafe, against `context/project.md`'s rule. It cannot
   be run here, only in Windows CI. The gain is a UX nicety.
3. **Chosen, by the owner**: ask, as 0.3.0 already does where it cannot tell (Alt+C on
   Windows).
   - A `[y/N]` line before anything is drawn. Enter is no and exits 1; no is the common
     case, a typo in the panel.
   - The wording names only what is known: "This shell runs inside xdevcmd N: in its
     terminal panel, or in a window opened from that panel". Not "an instance is
     running": the question never appears for a terminal opened from the Start menu
     while another xdevcmd runs, so that wording would mislead.
   - No status-line note.
   - Unix is unchanged: exact.
4. **pid on screen** (owner: About is not seen, "thường ko mở"). It goes at the right of
   the menu bar, dim, hidden where the menus need the room. Not in About; Usage already
   shows it. Not on the status line, which is full.
5. Tests that fail without each change; Windows end to end under ConPTY (`windows_panel.rs`).

## Do

- `main.rs`:
  - `Nested::Note` → `Nested::Ask`.
  - `start_anyway(pid)` asks on stderr and reads stdin; with no terminal, it starts.
  - `said_yes` accepts `y` or `yes` in any case; anything else is no.
  - The status note and `App::say` (added for it in Round 017) are removed.
- `app.rs` `draw_bar`: `xdevcmd <pid>` right-aligned in `fg_dim` when it fits, with a gap
  of two after the menus.
- `app.rs` status line: the right half was drawn at `lw`, straight after the left half.
  The two-cell gap meant to sit between them was at the end of the line. It is now drawn
  at `lw + gap`.
- Tests:
  - unit `only_yes_starts_another`;
  - `without_terminal_names_unix_refuses_and_windows_asks`;
  - render `the_menu_bar_shows_which_xdevcmd_this_is`;
  - render `a_terminal_split_too_small_to_use_is_refused` now asserts the gap. It failed
    without the fix ("historytoo small").
  - `tests/windows_panel.rs` (Windows only): Enter → exit 1; `y` → it draws and `q` ends it.
- CHANGELOG Unreleased: Added, Changed, Fixed.
- **Windows CI, first run (42879412666)**: `ctrl_z_says_what_cannot_be_undone_and_refuses_what_changed`
  failed. It was not this change: the test passed on Windows for PRs #15 and #16.
  `--screenshot` waits only 30 ms between keys, so on the runner the copy (F5) was still
  running when Ctrl+Z came, and its "1 copied" replaced Ctrl+Z's answer. The fix is a
  `wait` token for `--keys` (until running jobs end, as before the frame) and
  `…,F5,Enter,wait,Ctrl+z` in the test. Not reproducible on Linux, which is too fast;
  only Windows CI shows it. `windows_panel.rs` did not run there, because cargo stops at
  the first failing test binary.
- Owner's call: `.agents/reference/testing.md:51` lists the `--keys` tokens and could add
  `wait`; it is canon, so it is not edited here.

## Check

- [x] fmt; clippy `-D warnings` for Linux, x86_64-pc-windows-gnu and x86_64-apple-darwin
- [x] Linux: unit, render, pty suites green
- [ ] `windows_panel.rs` in Windows CI (`gh workflow run ci --ref feat/v0.4.0`)
- [ ] The owner on Windows:
  - `xdevcmd` in the panel (cmd, pwsh) asks; Enter ends it;
  - `start cmd` from the panel, then `xdevcmd`, asks; `y` starts;
  - a terminal from the Start menu does not ask;
  - the menu bar shows the pid.

Not verified here: the question through a real ConPTY (Linux cannot run it), and how
`wt` launched from the panel behaves: whether Windows Terminal hands the variable on is
not known.
