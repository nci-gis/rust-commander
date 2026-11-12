# Round 030: Smart Planner v1, the core (v0.4.0-beta.1, part 2 of 4, item 42)

**Status**: Planning — outline; the full kickoff review is written when Round 029 is
Complete, and the owner says go then
**Part of**: v0.4.0, step 6 of `ROADMAP.md`, beta.1; item 42 ("Notes on the items → 42
Smart Planner v1"); the owner's decisions of 2026-11-13
**Date started**: YYYY-MM-DD
**Date completed**: YYYY-MM-DD

## Goal

F5 and F6 (and Shift+F5, Alt+Y's sync and Resume) become **scan → plan → confirm →
execute**. The planner only reads; the executor stays Round 029's sequential one, fed by
the plan. Everything that can be known before the first byte is known and said before
it, once per group, with a default that never loses data; a job whose plan has nothing
to decide looks exactly like today's F5 dialog. The journal holds the plan, so a resume
knows exactly what is left.

## Plan (outline)

### Depends on Round 029

- `commit_replace`, the sync rules and the `moved` line: the executor's steps.
- `Ident` and the re-check right before each replace: the plan stores an `Ident` per
  row that replaces, and the executor checks it (B05 generalised: "re-check each target
  right before executing it").
- `Journal::line → Result`, `commit()`, the `v 2` header: the plan's lines and their
  syncs.
- Compare's `Unreadable` / `Missing` / `Incomplete`: the scan reports the same states
  (a source directory that cannot be read is a ⛔ row, not a silent gap).
- The fault seam (`ReadDir`, `Stat`, `Delay`) for the scan's tests.

### Outside check (to be written in full at kickoff)

- **robocopy `/L`** lists what it would do without doing it, in its classes (New File,
  Newer, Older, Same, Extra, Lonely, Mismatch for a file over a directory), and `/XO`,
  `/XN`, `/XC` exclude by class. Model for the plan's row classes and for "asked once
  per group".
- **`rsync -n --itemize-changes`** gives one code per row (`>f+++++++++` a new file,
  `>f.st......` size and time changed, `cd+++++++++` a new directory): a compact, total
  description of a plan. Model for the summary line and the plan table's first column.
- **Explorer's conflict dialog**: "Replace the files in the destination / Skip these
  files / Let me decide for each file", with both files' size and date side by side and
  "Do this for the next N conflicts". Model for the group question and its default.
- **Total Commander's overwrite dialog**: Overwrite, Overwrite all, Skip, Skip all,
  Overwrite all older, Copy all larger, Copy all smaller, Rename, Compare. Taken: the
  classes; not taken: per-file prompts as the default for a big job.

### Steps (each a commit with its tests)

1. **`plan.rs` in core, the model**: `Plan { rows, totals, devices, mode }`; `Row { src,
   dst, action: Create | Replace(Ident) | MergeDir | Skip, class: Identical | Older |
   Newer | FileOverDir | DirOverFile, warnings }`; one walk that replaces the dialog's
   `ops::count` and the worker's own `scan` (today a job walks its tree twice). Same or
   another device from `st_dev` (Unix) and the volume of the path (Windows: the root of
   its prefix; a UNC path is network). **M.**
2. **Modes by measured scan time**: ≤64 files and ≤128 MB always a full plan; above, full
   if the scan ends within ~300 ms, otherwise **streaming** (rows with nothing to decide
   go to the executor as they are planned; questions are gathered into one table shown
   when the scan ends, or earlier if the executor runs out of clean rows). No inline,
   no-scan mode. Numbers measured again at the end of the round. **M.**
3. **Pre-checks**: free space (Unix `rustix::fs::statvfs`); conflicts by class; names the
   target cannot hold (Windows' `<>:"|?*`, trailing dot or space, `CON`, `NUL`, `COM1`…;
   names longer than the target allows in its own units: 255 bytes on ext4, 255 UTF-16
   units on NTFS; two names that collide without case, found by looking up a name with
   its case changed in the target, read only); FAT32's 4 GB (the target's type from
   `statfs` on Linux and macOS); permissions (`access(W_OK)` on Unix). **M.**
4. **The confirm table, three levels**: ⛔ cannot be done (listed before the start; the
   rest can go on without them) · ⚠️ data lost for good (a replace with no trash, a name
   that would be changed by the target) — default "skip those" · ℹ️ undoable or a
   property lost (a replace into the trash, a move across devices, FAT losing
   permissions) — in the summary only. Asked once per group. Shown only when something
   needs deciding; otherwise today's F5/F6 dialog, unchanged, no extra key. A summary
   line: create · replace · skip · needed / free. Render tests at 118×34, 80×24, 60×20.
   **M.**
5. **The journal holds the plan; resume is exact**: the plan's rows written at start (one
   sync); a `done <row>` line per finished row (unsynced, except where Round 029's rules
   sync); Resume reads plan and done lines, re-checks each remaining row's target, and
   runs only what is left. `boot_id` on Linux (Round 029, decision 6). Kill tests at each
   phase through the seam's `Halt`. **M.**
6. **The executor fed by the plan** (`ops::Worker` takes `Plan` rows; `Existing` becomes
   the plan's per-group answers; Alt+Y's `Request::sync` becomes a plan); docs, CHANGELOG.
   **M.**

About two and a half weeks.

### Risks

- **Streaming and questions together**: a clean row started while a question for its
  parent directory (a file over a directory) is pending. Rule: a row waits if any
  ancestor has an open question.
- **Memory on a huge tree**: a million rows at ~200 bytes is ~200 MB. A cap (say 200,000
  rows in memory; past it the plan streams to the journal file and only questions stay in
  memory) or simply streaming mode past the cap.
- **The scan sees one moment; the execution another**: covered by the re-check per row,
  but the summary's "needed / free" can be wrong by the end; free space is checked again
  before each large file.
- **Windows without `unsafe`**: std has no free-space or file-system-type call (see
  decision 1); a check that cannot be made is "unknown", never "fine".
- The F5 dialog must not get slower for the common case (a few files): the floor keeps
  that; a render test and a timing test guard it.

### Open decisions (recommendations at kickoff)

1. **Free space and FAT detection on Windows.** Std cannot ask; options: a small crate
   (`fs4`'s space call, which keeps its `unsafe` inside it) or "unknown" on Windows.
   *Leaning:* "unknown" for beta.1, measured feedback first; the owner decides.
2. **Which operations use the planner in v1.** *Leaning:* F5, F6, Shift+F5, Alt+Y,
   Resume. Not delete, extract or pack.
3. **The streaming numbers** (64 files / 128 MB floor, 300 ms) are the owner's; measured
   again on a USB stick and a NAS before beta.1.
4. **The plan cap** for memory, and what happens past it.
5. **Per-group answers kept for the job only**, or remembered as a preference (leaning:
   job only).

### Not doing

- Lanes, concurrency, group commit, resume mid-file, reflink and `CopyFile2` (v2, item
  43, beta.2). Handing the plan to rsync, robocopy or rclone (item 44, 0.5).

## Do

## Check

## Act
