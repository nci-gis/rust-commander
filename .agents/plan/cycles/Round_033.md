# Round 033: 0.4.0-beta.2: Smart Planner v2 and pillar 2

**Status**: Do (kickoff 2026-11-13; the owner: "bro start r032 nhé", meaning the
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

1. **A**: `scripts/ci-green.sh`, called by `release.yml`'s check job on a tag push. **S.**
2. **E**: the Help row inside F1 switches the size; a pty test for F1 from F1. **S.**
3. **D**: `~` on Windows when `HOME` is not an absolute path; the Ctrl+\ render
   flake. **S.**
4. **B**: item 45. **S.**
5. **C**: item 46, the history only. **S.**
6. **G, first part**: group commit for small files, copies and replaces, measured
   against the owner's beta.1 baseline (sandbox numbers until then). **M.**
7. **F**: pillar 2, one finding at a time (A01, A06, A07, B08, A03). **M–L.**
8. **G, the rest**: item 22's fast copy behind a fallback, the engine named in the plan;
   then lanes, concurrency and checkpointed resume as the numbers say. **M–L.**
9. The cut of beta.2, as Round 032's step 6, with CI's wait in place.

## Do

### 2026-11-13, the owner's decisions ("Go as recommended")

1. Group commit now, the rest of item 43 after beta.1's baseline.
2. Item 22 (reflink, `copy_file_range`, `clonefile`, `CopyFile2`) in beta.2, behind a
   fallback to today's copy, with the engine named in the plan. ROADMAP's Later row for
   22 moves to beta.2 (logged in promotions.md).
3. Item 46: the history only; the status line stays one row.
4. The Help row inside F1 switches the size.

### 2026-11-13, parts A–E

- **A** (40b854e): `scripts/ci-green.sh` waits for a `ci` run on the tag's commit (up to
  60 min) and fails on none or a failed one; `release.yml`'s check job runs it on a tag
  push (`actions: read`). Tried against the real runs: d4ce202 passes, aa410e7 fails.
  Its first real use is beta.2's tag.
- **E, D** (97164c3):
  - F1's row inside F1 switches the size. The unit test
    `f1_inside_f1_switches_size_and_never_runs_itself` pins that F1 from F1 never loops.
  - `~` takes the first of `HOME` and `USERPROFILE` that is an absolute path here.
  - The Windows Ctrl+\ flake: `--screenshot` took its frame while `C:\` was still being
    read past `SLOW` (150 ms), so the pane still showed the old directory. Inputs and
    the frame now wait for such a read. The new render test, with
    `XDEVCMD_TEST_SLOW_DIR`, fails without the change and passes with it.
- **B** (31c3179): item 45. Nothing is said for 300 ms; then "opening …", then "opened …",
  "handed … to <program>" after 5 s, or the error, even a late one. Unit tests with
  `sh` scripts (Unix).
- **C**: item 46. Alt+M, Help → Messages and F1 show the last 50 status messages, each
  whole and with its time, in the viewer. They are recorded at the frame that draws
  them, so every setter is covered. Render test.

### 2026-11-13, the owner's beta.1 test on Windows 11, observation 1

- The owner "probed" a folder of about 1,500 small files (`.git`, source, 35 MB): a few
  milliseconds. Another of about 1,500 files (docs, about 500 MB): 1–1.5 s.
- **The same count of files, with time following the bytes.** The planner's walk reads
  metadata only, never contents (`compare_files`: time and size), so bytes should not
  matter.
- Suspect, not yet measured: on Windows `fs::symlink_metadata` opens each file, and
  Defender's real-time scan may look at it on open, longer for bigger documents.
  `DirEntry::metadata()` on Windows comes from the directory listing itself, with no
  open. The planner calls `symlink_metadata` per source and per destination
  (`plan.rs` around lines 900 and 917).
- Asked of the owner: which action "probe" was (the F5/F6 check, a folder's size, or the
  copy itself); whether a second run is as slow (a cold cache); and whether the folder
  is excluded from Defender. Goes with pillar 2 and item 43.

## Check

## Act
