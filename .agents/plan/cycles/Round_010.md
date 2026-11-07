# Round 010: The mouse in Settings

**Status**: Complete
**Part of**: [ROADMAP.md](../ROADMAP.md), carried from [Round 007](Round_007.md)
**Date started**: 2026-11-07 (owner: "dùng chuột trong settings có thể mở 1 round mới và target v0.2.0")
**Date completed**: 2026-11-07 (owner: "xong thì flip r10 Complete luôn nhé"; merged in #7)

## Goal

File → Settings… works with the mouse as every other dialog in rcmd does: a click picks
a tab, a row, or changes a value; the buttons save or cancel; the wheel moves between
rows. Round 007 shipped it keys-only.

## Plan

- [x] The dialog records where it drew each tab, row, value arrow and button, as the
      other dialogs do, so a click hits exactly what is on screen.
- [x] A click on a tab opens it; on a row, selects it; on `‹` steps the value back, on
      the value or `›` forward; on a check box, toggles it. Theme changes preview, as
      with the keys.
- [x] The buttons: Save and Cancel (Close in read-only modes, which never save).
- [x] The wheel moves the selected row.
- [x] Render tests for each, through `--keys=click:x:y`.

## Do

Built on `feat/settings-mouse`, cut from `feat/polish` so both land in one pull request
(one CI run, as the owner prefers).

- `SettingsDialog` records `tab_hits` and, per row, the line, its `‹` and its value
  while drawing; `on_mouse` returns the same `Outcome` as `on_key`.
- The app's handling of an `Outcome` (preview a theme, put it back on cancel, save)
  moved into `settings_outcome`, shared by keys and the mouse, so a click previews a
  theme exactly as ← / → do.
- A click on a row selects it; on `‹`, steps back; anywhere else on the value
  (including `›`), forward; a check box toggles. No drag, no right click: nothing in
  the dialog needs them.

## Check

- [x] Render tests: a tab, a row, `‹` and `›`, a check box, Save writes the file, Cancel
      does not, the wheel (`settings_works_with_the_mouse`).
- [x] Read-only: a click changes what is shown, the one button closes, and nothing is
      saved (added to `settings_in_read_only_mode_shows_but_never_saves`).
- [ ] By hand in a real terminal: not done by an agent; `--screenshot` replays the same
      mouse events the terminal would send, but not a terminal's own quirks.

Record what you could **not** verify as explicitly as what you could.

## Act

**Unticked check, and why the round closes anyway:** the mouse was not tried by hand in a
real terminal; `--screenshot` replays the same events a terminal sends, and CI passed on
Linux, macOS and Windows (#7).

**Learnings**:

- **Record hit areas while drawing**, as the other dialogs do: the click handler then
  needs no layout arithmetic of its own, and cannot disagree with the screen.
- **Keys and clicks should end in one handler** (`settings_outcome`): a click on a theme
  previews it exactly as an arrow key does, with nothing to keep in step.

**Promotions**:

- (none yet)
