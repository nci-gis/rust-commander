# Round 033: 0.4.0-beta.2: Smart Planner v2 and pillar 2

**Status**: Planning (kickoff 2026-11-13; the owner: "bro start r032 nhé", meaning the
next round, since 032 is the beta.1 cut and is waiting on the owner's tests)
**Part of**: v0.4.0, step 6 of `ROADMAP.md`, the row for **0.4.0-beta.2**: item 43
(Smart Planner v2, its scope set by beta.1's measurements), pillar 2 (responsive and
bounded), and items 45 and 46
**Branch**: `feat/v0.4.0-beta.2`, from `dev` at cf79d7b (`feat/v0.4.0` is merged into
`dev` and can be deleted)
**Date started**: 2026-11-13
**Date completed**: YYYY-MM-DD

## Goal

A `v0.4.0-beta.2` tag on `dev` where:
- copies and replaces of many small files are no longer bound by one `fdatasync` per
  file, measured against beta.1's baseline;
- the screen stays responsive with bounded memory on large and busy folders (pillar 2);
- `o` and the status line say what happened (items 45, 46).

The data-safety rules of Round 029 still hold.

## Kickoff review (2026-11-13)

### 1. Where things stand

- `v0.4.0-beta.1` is published as a pre-release (Round 032). The owner is testing it on
  Windows, macOS and Linux with `docs/testing-by-hand.md`. Round 032 stays open until
  then.
- **beta.1's numbers do not exist yet.** The only measurements are from this sandbox:
  - replacing 1,000 small files took 7.1 s, about 7 ms a file, in `fdatasync` before
    the rename;
  - a 2,000-file copy took 47 ms, since new files are not synced one by one.

  The owner's `cargo xtask bench --save-baseline` is what item 43 is measured against.

### 2. What is in beta.2

| Part | What | Size |
|---|---|---|
| A | **Release waits for CI**: `release.yml`'s check refuses a tag whose commit has no green `ci` run (Round 032: the tag's build ran in parallel with `dev`'s CI). | S |
| B | **Item 45**: `o` shows "opening …" only after 300 ms, then "opened" when the system's opener hands the file over, or its error; it stops waiting after about 5 s. explorer.exe's exit status means nothing, so on Windows it is "handed over". | S |
| C | **Item 46**: the status line's past, the last 50 messages, each whole, with the time; reached from F1 and a key. The line stays one row. | S |
| D | **Small fixes found in Round 032**: on Windows, a `HOME` that is not an absolute Windows path (Cygwin's `/home/x`) falls back to `USERPROFILE` for `~`; the Windows Ctrl+\ render flake (pillar 4, seen again in run 43409507585). | S |
| E | **F1 from F1, pinned by a test** (owner's question, see 3). | S |
| F | **Pillar 2**: A06 request generations with stale results dropped; A07 watcher debounce with a max wait, a budget per tick and a rescan on overflow; A03 a progressive viewer index; B08 one shared resource budget; A01 marks as a `HashSet`. | M–L |
| G | **Item 43, Smart Planner v2**: group commit for small files, copies and replaces both. Lanes, adaptive concurrency, checkpointed resume and the platform's fast copy (item 22) are each decided after beta.1's numbers. | M–L |

### 3. F1 from F1: is there a loop? (the owner's question)

**No, by construction.** Read in `commands.rs` and `app/palette.rs`:
- **F1 inside F1 is a two-step toggle that ends in closing**: small → large if the
  screen allows, then closed (`commands.rs` `on_key`, `KeyCode::F(1)`). Nothing opens a
  second window.
- **Running a row closes F1 first**: `run_command_row` takes the window out of
  `self.modal` before it runs anything. A row that opens F1 again ("F1 Help", or Help →
  Commands… at the large size) therefore replaces it. There is never more than one
  Commands window, so no stack and no recursion; Esc once closes it.
- **The "Help" row is left out of the small window's matches** (`r.run !=
  Run::Act(Action::Help)`), but it can still appear under "Elsewhere" and in the large
  window's list. Running it there closes and reopens at the small size. This is
  harmless, but it is a row that does nothing useful.

To do (part E): a pty test that pins it.
- F1, F1 → large; F1 → closed.
- F1, type "commands", Enter → one window; Esc → none left.
- Decide what the Help row does inside F1. Recommended: switch size (small ↔ large)
  instead of reopening, so the row means something there.

### 4. Order

A–E first: small, independent, and the owner's beta.1 week runs meanwhile. Then F,
which needs no numbers. Then G, once the baseline is in: its first measurement is
group commit against beta.1's "replace small files" and "copy small files".

### 5. Decisions for the owner

1. **Item 43's start**: group commit now, since its cause is measured (7 ms a file), with
   the rest of 43 after the baseline (recommended). Or all of 43 after the baseline.
2. **Item 22 (the platform's fast copy: reflink, `copy_file_range`, `clonefile`,
   `CopyFile2`) in beta.2**: in, behind a fallback to today's copy and with the engine
   named in the plan, since rc.1 and the week after give it time to soak (recommended);
   or kept for 0.5, as ROADMAP's Later table has it today.
3. **Item 46's second row**: the history only (recommended; a second row costs every
   pane a row on a 24-row terminal); or also `[general] status_lines = 2`.
4. **The Help row inside F1**: switch size (recommended), or hide it there.

## Plan

Written after the decisions above.

## Do

## Check

## Act
