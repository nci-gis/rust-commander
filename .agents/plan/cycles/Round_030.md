# Round 030: Smart Planner v1, the core (v0.4.0-beta.1, part 2 of 4, item 42)

**Status**: In Progress (the owner's go, 2026-11-13: "Go as recommended"; decisions 1–6
as recommended)
**Part of**: v0.4.0, step 6 of `ROADMAP.md`, beta.1; item 42 ("Notes on the items → 42
Smart Planner v1"); the owner's decisions of 2026-11-13
**Date started**: 2026-11-13
**Date completed**: YYYY-MM-DD

## Goal

F5 and F6 (and Shift+F5, Alt+Y's sync and Resume) become **scan → plan → confirm →
execute**. The planner only reads; the executor stays Round 029's sequential one, fed by
the plan. Everything that can be known before the first byte is known and said before
it, once per group, with a default that never loses data; a job whose plan has nothing
to decide looks exactly like today's F5 dialog. The journal holds the plan, so a resume
knows exactly what is left.

## Kickoff review (2026-11-13, after Round 029 closed)

### 1. What changed since the outline

- Round 029 is Complete (closed by the owner 2026-11-13). What it leaves for this round:
  `Request::expect` with `Expect::{Absent, Replace(Ident)}` and `Worker::still_agreed`
  (a sync replaces only what its plan named, and only while it is still that file);
  `Journal::begin` asked about before anything is done; the batched cross-device move;
  the fault seam (`ReadDir`, `Stat`, `Delay`, `Halt`) now in `reference/testing.md`.
- The owner kept the sync cost (~2.4× for many small files across devices) for beta.1;
  group commit is beta.2. Nothing here should add a second sync per file.

### 2. The code this round touches

| Where | What is there today | What the planner does with it |
|---|---|---|
| `core/ops.rs` `count` / `Count::spawn` (1461) | the F5 dialog's background walk: files and bytes, stoppable | becomes the plan's scan (same thread and stop flag), returning rows, not two numbers |
| `core/ops.rs` `Worker::run` (471) | walks the tree **again** (`scan`) before working; `Existing` decides each conflict; questions mid-job | fed the plan's decisions; its own `scan` goes for copy/move |
| `core/ops.rs` `Request::expect`, `Expect`, `Ident` (241, 198) | the sync's per-destination expectation, checked right before a replace | **the carrier**: every planned copy/move sends its decisions this way (below, critique 1) |
| `core/ops.rs` `resolve_conflict`, `still_agreed` (683, 757) | asks or applies `overwrite_all` / `skip_all` / `update` | a planned row is never asked again unless the target changed since the plan (B05 as is) |
| `core/compare.rs` + `app/compare.rs` | Alt+Y builds `(src, into, Expect)` pairs → `Request::sync` | its rows become the same `Plan` type; no second scan |
| `core/journal.rs` `begin`, `moved`, `Interrupted` (190, 349, 366) | kind, sources, dest; tidy-up lines | records the plan's **decisions** and done markers |
| `app/file_ops.rs` `begin_transfer` (64) | `Modal::Confirm` with `counting: ops::count(..)`, the `U` key cycling `Existing` | the confirm table when there is something to decide; otherwise this dialog unchanged |
| `app/jobs.rs` `resume` (235) | re-runs with `Existing::Resume` on the sources still there | re-plans, applies the recorded decisions, re-checks each `Ident` |
| `core/archive.rs:594` | `rustix::fs::statvfs` for free space before extracting | moved to one helper the pre-checks share |

### 3. Outside check

The outline's models stand (robocopy `/L` classes, `rsync -n --itemize-changes`,
Explorer's grouped conflict dialog, Total Commander's overwrite classes). Two additions:
FreeFileSync's comparison grid (left / action / right per row, actions changeable per
group before "Synchronize") is the closest model for the confirm table; and its
"Database" file is the precedent for **keeping decisions, not snapshots**: it stores the
last synced state and re-derives the plan, rather than replaying a stored list.

### 4. Critique of the outline

1. **The executor does not need a new input type.** `Request::expect` already carries,
   per destination, what the confirmed plan expects and the `Ident` to re-check. Extend
   `Expect` with `Skip` (and `Merge` for a directory onto a directory) and every planned
   F5/F6/Shift+F5 sends its decisions the way Alt+Y already does. Step 6 shrinks from
   "the Worker takes `Plan` rows" (a rewrite of a 3,456-line file the data-safety work
   just settled) to "the Worker stops scanning twice and trusts `expect`". The item's own
   rule is "the executor unchanged"; this keeps it.
2. **Streaming is the riskiest part and not needed to earn the trust.** Executing while
   questions are open needs the ancestor rule, a partial plan in the journal, and a
   resume of a plan that was never finished. A full plan behind a cancellable progress
   line ("Planning… 41,200 files, 3.1 GB", Esc to stop, **Start now** to fall back to
   today's ask-as-you-go job) covers the slow network and huge-tree cases with today's
   tested path. Recommend: streaming to beta.2 with the lanes (decision 2).
3. **The journal should hold the decisions, not every row.** A million-row plan written
   and synced at the start is a large write before the first byte, and a resume must
   re-check every remaining target anyway. Record what cannot be re-derived: each
   per-group answer, each row that replaces (path + `Ident`) or is skipped, and `done`
   markers per top-level source (unsynced). Resume = scan again, apply the recorded
   decisions, re-check each `Ident`, ask about anything new. Exact where it matters,
   small on disk, and the memory cap stops mattering for the journal (decision 3).
4. **Case collisions cost one `read_dir` per target directory, not one lookup per name**:
   read each target directory once, keep its names folded, compare. On a network target
   that is the difference between N round trips and one per directory.
5. **FAT32 is not "FAT"**: exFAT has no 4 GB limit. Linux `statfs` `f_type` 0x4d44 is
   vfat (both FAT32 and FAT16); exFAT is 0x2011BAB0. macOS `f_fstypename` "msdos" vs
   "exfat". Windows: unknown (decision 1).
6. **The F5 dialog must stay instant.** The plan runs on `Count::spawn`'s thread and
   pattern; the floor (≤64 files, ≤128 MB) is a scan that finishes before the dialog's
   first frame on a local disk. A timing test guards it (budgets.rs, release build).

### 5. Steps (each a commit; every fix or behaviour first as a failing test)

1. **`core/plan.rs`, the model and the scan** — `Plan { rows, groups, totals, target:
   Volume }`, `Row { src, dst, action: Create | Replace(Ident) | Merge | Skip, class:
   New | Identical | Older | Newer | FileOverDir | DirOverFile, blocked: Option<Why> }`;
   built on `Count::spawn`'s thread; unreadable sources are rows with a `Why` (compare's
   states), never gaps. Unit tests on a temp tree, the seam's `ReadDir`/`Stat` for the
   unreadable cases. **M.**
2. **Pre-checks** — free space (the `statvfs` helper; Windows unknown), names the target
   cannot hold (Windows' characters and reserved names, trailing dot/space, length in the
   target's units), case collisions (critique 4), FAT32's 4 GB (critique 5), write
   permission. Each check that cannot be made is "unknown", shown as such. **M.**
3. **The confirm table** — three levels (⛔ / ⚠️ / ℹ️), asked once per group, the default
   never the losing one; the summary line `create · replace · skip · needed / free`;
   only when there is something to decide, otherwise today's dialog unchanged. Render
   tests at 118×34, 80×24, 60×20; a test that a plan with nothing to decide draws the
   old dialog byte for byte. **M.**
4. **The executor fed by decisions** — `Expect::{Skip, Merge}`; copy/move send `expect`;
   `Worker::run` no longer scans copy/move twice (totals from the plan); a planned row is
   not asked again unless its target changed (existing B05 tests stay green). Alt+Y's
   pairs become a `Plan`. **S–M.**
5. **Decisions in the journal; exact resume** — journal `v 3`: `answer`, `replace <path>
   <ident>`, `skip <path>`, `done <source>` lines; Resume re-plans and applies them; kill
   tests at each phase through `Halt` (planning, after confirm, mid-row, between rows).
   An old `v 2` journal still resumes the old way. **M.**
6. **Docs and CHANGELOG** — `docs/guide.md` (the confirm table, what each level means),
   `docs/safety.md` or the recovery contract (what Resume now knows). **S.**

About two weeks without streaming; two and a half with it.

### 6. Decisions for the owner

1. **Windows free space and FAT type**: "unknown" for beta.1 (recommended; no new
   dependency), or the `fs4` crate (its `unsafe` inside a dependency, as the project's
   rule allows).
2. **Streaming**: to beta.2 with the lanes (recommended, critique 2), or in this round.
3. **The journal holds decisions, not every row** (recommended, critique 3).
4. **Operations in v1**: F5, F6, Shift+F5, Alt+Y, Resume; not delete, extract, pack
   (the outline's leaning; agreed).
5. **Group answers for the job only**, not remembered (agreed).
6. **The floor and the cap**: ≤64 files / ≤128 MB always planned first (outline);
   past 200,000 rows the plan keeps only rows with a decision and counts the rest
   (recommended), measured again before beta.1.

### 7. Risks

- Changing what F5 shows is the most visible change since 0.1: the "nothing to decide
  → old dialog" rule and its byte-for-byte render test are the guard.
- A plan is a moment; the execution is later: every replace re-checks its `Ident`
  (Round 029), free space is checked again before each file over 64 MiB.
- Windows has the weakest `Ident` (creation time, not a file index): said in the
  recovery contract (pillar 3).

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

### 2026-11-13, the owner's decisions

All six as recommended ("Go as recommended"): (1) Windows free space and FAT type are
"unknown" for beta.1; (2) streaming goes to beta.2 with the lanes, this round plans in
full behind a cancellable progress line with **Start now** as the fallback; (3) the
journal keeps decisions, not rows; (4) F5, F6, Shift+F5, Alt+Y, Resume; (5) group
answers for the job only; (6) the 64 files / 128 MB floor, and past 200,000 rows only
rows with a decision are kept.

### 2026-11-13, step 1: the model and the scan

- `xdevcmd-core/src/plan.rs`: `Plan { kind, dest, rows, totals, same_device }`, `Row {
  src, dst, dir, bytes, class, target: Option<Ident>, blocked }`, `Class` (New,
  Identical, Older, Newer, Differs, FileOverDir, DirOverFile, Merge), `Blocked`
  (Unreadable, Special, IntoItself, SameFile). One walk: each destination looked at
  once (not under a directory that is itself new); a directory over a file is one row
  with its subtree's size. `start` runs it on its own thread with progress every 512
  entries; dropping the handle stops it. Past `CAP` (200,000) only rows with a decision
  are kept, the rest counted (`dropped`). `same_device`: `st_dev` on Unix, the drive or
  share on Windows.
- Not wired yet: the F5 dialog still counts with `ops::count` (step 3), the worker
  still scans (step 4).
- Tests (8, core): all new; every class with the old file's `Ident`; an unreadable
  directory and an unreadable file as blocked rows through the seam (`ReadDir`,
  `Stat`); into itself and onto itself; Shift+F5's rename; a stopped scan; the cap; the
  thread's progress and end. fmt; clippy for Linux, windows-gnu and darwin.

## Check

## Act
