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
- The owner's answer: slow only the first time, about as fast after; the folder is not
  excluded from Defender.
- **This fits a first-open scan.** Defender scans a file when it is first opened and
  remembers the verdict until the file changes; a cold disk cache would do the same.
  Either way, every file the planner stats on Windows is opened once.
- Planned: on Windows, the walk takes size and times from `DirEntry::metadata()`
  (FindFirstFile's data, no open) and calls `symlink_metadata` only where the entry
  cannot say, such as reparse points. It must then be measured on Windows: first run
  and second run, against beta.1's numbers. Part F, first. Not verifiable from this
  Linux sandbox.

### 2026-11-13, part F begins: the planner's walk from the listings

- `plan.rs`: inside a directory, a source's metadata comes from its `DirEntry`, and the
  destination directory is listed once (`There`: folded where case is ignored, so
  `README` finds `Readme` as a stat there would; a name missing from the listing is
  trusted to be absent only when case handling is known). On Windows a reparse point
  is still stat'ed. Top-level sources and their destinations are stat'ed as before.
  The fault seam's `Stat` points are unchanged.
- Test `the_listings_plan_what_a_stat_per_name_plans`: each name planned from inside
  its directory and on its own as a source gets the same class, size, target and kind
  (identical, newer, older, other size, new, file over folder, folder over file,
  merge), plus the case-folded lookup.
- Quick bench, this sandbox (Linux, where a stat opens nothing): plan of 10,000 entries
  7.5 ms (8.3 ms on beta.1). **The Windows effect is not measured**: the owner's run on
  Windows 11 decides (the same folder, first run, against beta.1).
- A risk accepted: NTFS can show a listing's size and time late for a file open for
  writing elsewhere. Then the target looks changed when it is checked again right
  before the replace, and the job asks or skips: the safe side.
- Not changed: the job's own walk still stats each file as it copies. Reading the file
  opens it anyway.

### 2026-11-13, part F: A01, A07, A06

- **A01**: marks were already a `HashSet<PathBuf>` (`pane/files.rs`). Nothing to do.
- **A07** (8fa2d7a): the watcher's callback folds events into one entry per watched
  directory (first and latest change), replacing the unbounded channel. A directory is
  due after 100 ms of quiet, or 1 s after its first change in a storm. An error or a
  `need_rescan` event marks every watched directory. Unit test with synthetic times.
  - **A deadlock was found and fixed on the way**: `sync` held the `watched` lock
    across notify's `watch`, which waits on notify's event thread. Five pty tests hung.
    No lock is held across notify calls now; the lock order is `watched`, then `dirty`.
  - The pty suite passed three runs in a row afterwards.
- **A06** (6942212): stale results were already dropped (one `reading` per pane,
  replaced by the next). What was unbounded is the threads: a read stuck in a dead
  mount cannot be stopped, and each new move left one. Now at most 16 run at once
  across panes (`ReadSlot`); past that a move or reload is refused with a note, and the
  periodic check waits. Unit test on a local counter.
- Left of pillar 2: B08 (one shared budget) and A03 (the viewer's progressive index).

### 2026-11-13, part G: group commit, measured, and the design (for the owner)

Measured in this sandbox (ext4), 1,000 files of 1 KiB written then made durable:

| Way | Time |
|---|---|
| `fdatasync` each, one after another (today) | 5,946 ms |
| `fdatasync` on 8 threads | 1,300 ms |
| `fdatasync` on 32 threads (ext4's journal commits them together) | 326 ms |
| one `syncfs` on the file system | 11 ms |

Design. It changes the order of Round 029's steps for small replacements only:
1. A replacement of a small file (≤ 1 MiB) on a local target is written to its
   temporary name as today (journal `part` line), and its handle is kept in a batch:
   up to 64 files, 16 MiB, or 1 s.
2. **One durability step per batch**:
   - Linux: `syncfs` on the destination's file system (rustix, safe);
   - macOS and Windows: each file's sync on up to 16 threads.
3. Then for each file, in order, exactly today's last steps: the journal's `swap`
   line, `durable`, the target checked once more, the old file to the trash, the
   rename. Nothing is renamed before its data is on disk. The invariant is unchanged;
   only how many syncs it costs.
4. **A failed batch sync renames nothing.** Each file of the batch is written again from
   its source and synced alone, today's path (a failed sync is never tried again on the
   same file: fsyncgate).
5. A kill between 2 and 3 leaves complete temporaries named in the journal, removed at
   the next start, as a kill before a rename is today. Cancel removes them.
6. Unchanged:
   - big files;
   - network targets (each file's error matters there, with retries);
   - new files (not synced, the owner's earlier call);
   - moves (already batched).

Expected: 1,000 small replaces from about 7 s to well under 1 s on ext4. Measured with
`cargo xtask bench` ("replace small files") here and on the owner's machine. `syncfs`
also flushes other programs' dirty data on that file system.

**Correction (the owner asked, 2026-11-13)**: "never slower than the sum of their
syncs" was wrong. If another program has 2 GB waiting to be written, `syncfs` waits for
all of it (about 10 s at 200 MB/s), longer than 1,000 small syncs (about 6 s). Others
are not made wrong: their data only reaches the disk sooner, and no lock or ordering
they rely on changes, so there is no race. But they can be slowed for a moment by the
burst of writing. Revised proposal: syncs of the batch's own files in parallel (up to 32
threads, 326 ms here for 1,000): only our files, the same code on every system.
`syncfs` is dropped, or kept only when the system's dirty data is small
(`/proc/meminfo` `Dirty:`). That choice is the owner's.

Measured again for the owner's question "16 threads, as on Windows and macOS?" (1,000
files of 1 KiB, ext4, two runs each):

| Threads | 1 | 4 | 8 | 16 | 32 | 64 |
|---|---|---|---|---|---|---|
| ms | 6,370–7,553 | 2,667 | 1,316–1,330 | 634–678 | 327 | 167–174 |

The time falls with the threads almost exactly. ext4 makes concurrent syncs durable in
one journal commit (about 6.5 ms here), so the cost is the number of commits, about
files ÷ threads. 16 threads is about 10× today.

On `syncfs`: it is always correct; the only question is how long it waits. That depends
on other programs' unwritten data, not on how small our files are. `Dirty:` in
`/proc/meminfo` says only how things were a moment ago (a download can start right
after), so a check before `syncfs` is a guess, not a guarantee.

### 2026-11-13, the owner on calibrating with CI, and Copy relative path

- **"Fixed, and measured through CI?"** Yes, for the shape, not the exact number. A
  `sync threads` case in `cargo xtask bench` (1, 4, 8, 16, 32, 64 threads, with
  `File::sync_data` as xdevcmd calls it) and a `bench` workflow, run by hand only, on
  Linux, macOS and Windows runners. Caveat: runners' disks are virtual (Azure), with
  their own flush latency, and some may not honour a flush, so their absolute times are
  not a laptop's. They show whether more threads help at all on NTFS and APFS, which
  sets the fixed number. The owner's machines confirm it.
- **Copy relative path** (owner, for beta.2): proposed as item 48. Copy path is in the
  files' right-click menu only today (`Cmd::CopyPath`, `copy_path` in `app/input.rs`).
  Relative to the repository's root when the file is inside one (found by looking for
  `.git` upward, no process run), else to the pane's directory. The same control-
  character guard and OSC 52 as Copy path. A row in F1 and the right-click menu.

### 2026-11-13, the owner's answers: status messages, auto-adjust, a config that fails

- **Item 48 says what it was relative to**, on the status line:
  - "copied src/main.rs, relative to the repository rust-commander";
  - "copied notes.txt, relative to this folder (not in a repository)";
  - several paths: "copied 3 paths, relative to …".
- **Auto-adjust only if CI shows it is worth it**; else a fixed 16. The CI sweep decides.
- **A `[system] sync_threads` setting (1–32)**: not proposed. The owner's "a bad value
  becomes 16" would go against the config's rule that a mistake is an error, never
  silence (`a_misspelt_key_is_an_error_not_silence`). A setting that follows the rule
  is only worth having if CI or the owner's machines show machines differ enough.
  Revisited after the sweep.
- **A config that fails to load** (the owner: "Default | Exit?"). Today the whole file is
  ignored, xdevcmd starts with the defaults, the status line names the error (Messages
  now keeps it), Help → Config in use shows why, and Settings refuses to save over it.
  A status line is easy to miss, so it is proposed as item 49: on the first frame, a
  dialog naming the file, the line and the error, with:
  - **Continue with the defaults** (Enter, the default);
  - **Edit config.toml** (opens it in the editor, as F4);
  - **Quit**.

  It never blocks a non-interactive run (`--screenshot`, `--check-config` is already
  exit 1). It does not count in the start-up budget, since it comes after the first
  frame.

### 2026-11-13, the sync sweep on CI (run 43458792575), and item 48

1,000 files of 1 KiB, `File::sync_data` on N threads, the median of three:

| Threads | Linux (ext4) | macOS (APFS) | Windows (NTFS) |
|---|---|---|---|
| 1 | 334 ms | 861 ms | 103 ms |
| 4 | 88 (3.8×) | 535 (1.6×) | 1,203 (0.1×) |
| 8 | 47 (7.2×) | 364 (2.4×) | 1,190 (0.1×) |
| 16 | 35 (9.7×) | 361 (2.4×) | 1,228 (0.1×) |
| 32 | 20 (16×) | 356 (2.4×) | 1,194 (0.1×) |
| 64 | 21 (16×) | 360 (2.4×) | 1,351 (0.1×) |

- **The shape differs by system, and that settles auto-adjust.**
  - Linux keeps gaining up to 32.
  - macOS stops at 8 (2.4×).
  - Windows is **twelve times slower** with any parallelism. 0.1 ms a sync on one
    thread also suggests the runner's virtual disk does not wait for its flush, so the
    owner's laptop must confirm. Parallel `FlushFileBuffers` on NTFS clearly contend,
    though.
- Proposal: a number per system, measured, not adjusted at run time:
  - Linux 16 (the owner's figure; 32 would gain 1.7× more);
  - macOS 8;
  - Windows 1, today's sequential syncs.

  Auto-adjust would only rediscover these. On Windows it would first try several
  threads and lose.
- **Item 48** (Copy relative path): `relpath.rs` (`.git` looked for upward, a file
  counts, for worktrees), `Cmd::CopyRelativePath` in the right-click menu, and Copy
  path and Copy relative path as F1 rows in Files (refused in restricted mode; from the
  shell they act on the file pane). Unit and render tests.

### 2026-11-13, the owner's probe, measured on CI (run 43461538732)

The owner's idea: in a job of 1,000 or more small files, the job's own first files
synced on 1, 8 and 16 threads in turn (16, 32 and 64 files), and the fastest kept for
the rest. One job of 1,000 files, the median of three:

| Way | Linux | macOS | Windows |
|---|---|---|---|
| fixed 1 | 231 ms | 1,648 ms | 180 ms |
| fixed default (16 / 8 / 1) | 48 ms | 483 ms | 175 ms |
| probe | 52 ms (chose 16) | 414 ms (chose 16, 16, 8) | 467 ms (chose 1) |

- **It always chose right**, and on macOS beat the fixed 8 (that runner is noisy: 16
  threads measured 763 ms in the sweep and better here).
- **Its cost is the trying.** On Linux about 4 ms. On Windows about 290 ms, since its
  8- and 16-thread steps are the slow ones there: 2.7× the fixed default on 1,000
  files. The same 290 ms on a 10,000-file job is about 15 %.
- Options for the owner:
  1. fixed per system only;
  2. probe from 1,000 files;
  3. probe, but start from the system's default, and drop a candidate after its first
     round if it is already clearly slower (twice the default's time per file). On
     Windows that cuts the cost to a round of each, about 40 ms.

### 2026-11-13, the probe with 5,000 files (run 43462505273)

| Way | Linux | macOS | Windows |
|---|---|---|---|
| fixed 1 | 2,136 ms | 4,037 ms | 1,020 ms |
| fixed default (16 / 8 / 1) | 772 ms | 2,221 ms | 1,099 ms |
| probe | 240 ms (chose 16) | 1,769 ms (chose 8, 8, 16) | 1,638 ms (chose 1) |

- **The runners are noisy.** Linux's sweep in the same run: 8 threads 312 ms, 16
  threads 551 ms, 32 threads 320 ms. The probe's 240 ms against the fixed 16's 772 ms,
  with the same choice, is mostly noise. CI shows a shape, not ±20 %.
- **macOS**: the probe beat the fixed 8 again (1,769 ms against 2,221 ms, -20 %), as at
  1,000 files.
- **Windows changed shape with more files**: 8 threads 827 ms against 1,100 ms on one,
  while 4, 16 and more lost. The parallel times stay near 1.2 s whether 1,000 or 5,000
  files, like a fixed cost. So the probe's small samples (32 files on 8 threads)
  measure mostly that cost, chose 1, and paid about 600 ms for trying (+50 % at
  5,000, 2.7× at 1,000). On a laptop's real disk this may differ entirely: the
  runner's 0.1–0.2 ms a sync suggests its flush does not reach a disk.
- Revised proposal: probe on Linux and macOS (from the default, as option 3), where it
  never lost and on macOS won 20 %. Windows syncs one at a time, as today, until the
  owner's machine is measured (`cargo xtask sync-sweep --probe`).

### 2026-11-13, the owner's Windows 11 laptop (xtask from run 43466099106)

`sync-sweep --probe` on the laptop's system drive (`%TEMP%`, Defender on):

| Threads | 2,000 files | 5,000 files | 10,000 files |
|---|---|---|---|
| 1 | 844 ms | 2,207 ms | 5,100 ms |
| 8 | 367 (2.3×) | 799 (2.8×) | 1,892 (2.7×) |
| 16 | 198 (4.3×) | 477 (4.6×) | 1,117 (4.6×) |
| 32 | 131 (6.5×) | 331 (6.7×) | 599 (8.5×) |
| 64 | 104 (8.1×) | 421 (5.2×) | 467 (10.9×) |
| probe (chose 16, 16, 8) | 230 ms | 563 ms | 1,118 ms |
| fixed 1 (the proposed Windows default) | 944 ms | 2,422 ms | 4,852 ms |

- **The runner misled us**: on a real Windows disk, parallel syncs help as much as on
  Linux. One thread there is 0.4–0.5 ms a file. The runner's 0.1 ms meant its flush
  never reached a disk, and its parallel penalty was the virtual disk's.
- The probe found it at once (4× the fixed 1). It never tried 32, which is better
  still (1.5–1.9× over 16).
- Revised, final proposal:
  - default 16 on Linux and Windows, 8 on macOS;
  - the probe tries 1, 8, 16 and 32 (32 is the owner's maximum), starting from the
    default and dropping a candidate after its first round when it is twice the
    default's time per file or worse;
  - jobs of 1,000 small replacements or more probe; smaller jobs use the default.
  - The lesson: CI's virtual disks do not answer questions about flushing. A real
    machine does.

### 2026-11-13, the probe's floor: 2 instead of 1 (the owner)

- The owner: "2, 8, 16 and 32, instead of 1?" Agreed. 1 was there as a floor for a
  device where parallel syncs hurt. No real disk has shown one (only the runner's
  virtual disk), and where flushes are serialized anyway, 2 threads cost about what 1
  does. The 1-thread step was also the probe's most expensive per file.
- Probe candidates now 2, 8, 16, 32 (16, 32, 64 and 128 files). The sweep gained a
  2-thread row. Here (ext4) 2 threads gain only 1.1× (two syncs rarely land in one
  journal commit), and the probe chose 32 every time: 425 ms against the fixed 16's
  643 ms on 1,000 files.
- A sync error is unaffected: that file is written again and synced alone, as today.

### 2026-11-13, part G: group commit, built

- `ops.rs`: a replacement of ≤ 1 MiB on a local drive, not moving and not a batch's
  own fallback, is written to its temporary name as before (journal `part` line), and
  its handle waits in `Group.pending`. A batch is flushed when it holds
  max(64, 4 × threads) files, 16 MiB, or 1 s, and also:
  - before any question (`ask`);
  - before a source is marked done in the journal;
  - before a directory's permissions take writing away;
  - at the end of the job.
- **The flush**:
  1. every data sync on T threads (`sync_all`);
  2. the batch's `swap` lines;
  3. **one** journal commit;
  4. per file, in order: the target checked again, the old file to the trash, the
     rename (`rename_into`, the same code a file on its own uses after its sync).
  - A failed sync, a failed last step or a changed target goes the way `copy_file`
    takes it for a single file: asked about, Retry writes it again from its source
    (never a second sync of the same file).
  - Stopped: temporaries removed, old files kept. A stale `swap` line is harmless:
    recovery renames only when the destination is gone and the temporary exists.
- **Found on the way**:
  - parallel data syncs alone left 1,000 replacements at 6.7 s. The journal's sync per
    file was the next wall, hence the shared commit. `strace` of 1,000: 1,017
    `fdatasync` (1,000 data in parallel, 17 journal);
  - my first version silently rewrote a file whose batch sync failed. Two existing
    fault tests caught it, and the rule stands: a failure is asked about.
- **Threads**: `group_default_threads()` 16, macOS 8. After 1,000 batched files the
  probe tries the default, then 2, 8, 16 and 32, one batch each, and keeps the fastest
  a file (`Group::measured`).
- **Tests** (ops.rs):
  - batches, with each file's sync before its rename (trace);
  - one failed sync in a batch asks about that file alone;
  - stopped with a batch waiting leaves every file whole;
  - a read-only directory gets its batch before its permissions;
  - the probe's order and choice.

  The 298 earlier core tests pass unchanged.
- **Measured** (this sandbox, ext4, `bench-case replace-trash`):

  | Files | Before | Now |
  |---|---|---|
  | 1,000 | 6.8–7.1 s | 0.89 s (7.6×) |
  | 5,000 | 59.7 s | 2.9 s (20.8×) |

  At 5,000 files the probe ran. Not measured: the owner's Windows and macOS machines
  with the real job (`cargo xtask bench`).
- Docs: `docs/security.md` "What is synced, and when"; CHANGELOG.

### 2026-11-13, item 49: a config that fails to load is asked about

- At start, after recovery's question (if any), a config with a mistake opens a dialog
  **Config not loaded**: the path, the error, "xdevcmd runs with the default settings,
  and saves nothing over the file":
  - **Enter** continues on the defaults; the status line keeps saying so and points to
    Help › Config in use;
  - **E** opens the file in the editor (hidden in read-only and restricted modes).
    When the editor closes, the file is read again: right → "… is right now — restart
    xdevcmd to use it"; still wrong → the dialog again with the new error;
  - **Q** quits.
- `modal::Ask` has a third button (`more`, its key and its click) for Quit; every other
  dialog leaves it `None`. New `Cmd::EditConfig`, `Cmd::Note`.
- Only for an interactive start: `--check-config` and the like still print and exit 1.
- **Tests** (pty): the dialog, Enter → defaults, Q → a clean exit; E with a stand-in
  editor that mends the file → "is right now".
- Docs: `docs/config.md`, CHANGELOG.

### 2026-11-13, item 22: the platform's fast copy, behind a fallback

- `fastcopy.rs` (core): the engines, all through rustix, so no unsafe code:
  - Linux: `FICLONE`, else `copy_file_range` in 4 MiB steps (Esc and progress between
    steps), else the old loop from the offsets where the kernel stopped. Refused:
    EXDEV, ENOSYS, EOPNOTSUPP, EINVAL, EBADF, EPERM, EOVERFLOW, ETXTBSY, and a 0 on the
    first step (/proc files). Other errors are the copy's errors.
  - macOS: `fclonefileat(NOFOLLOW | NOOWNERCOPY)` before the temporary file is created
    (it makes the file itself), else the old way.
  - Windows: unchanged. `CopyFile2` (SMB server-side copy, ReFS and Dev Drive block
    cloning) needs FFI, and core is `forbid(unsafe_code)`; `std::fs::copy`
    (CopyFileExW) has neither progress nor Esc. **For the owner.**
- Only the bytes change hands: the temporary name, the journal line, the syncs, group
  commit and the rename are as before. A planted `Write` fault is met once per step.
- **Named**: `Summary::cloned` and `by_kernel`; the status line says "(cloned)", "(by
  the kernel)", or counts when mixed. The F5 dialog says nothing yet (the engine is
  known only once a file is copied).
- `[general] fast_copy` (default on), `Request::fast` (taken per job, as `safe`).
- **Tests** (core): a 9 MiB file arrives whole, the engine named, and off is the own
  loop with nothing said; a /proc file the kernel will not copy is read and written.
- **Measured** (this sandbox, ext4, data in memory, `XDEVCMD_BENCH_FAST=0` against
  the default, 3 runs each): `copy-big` 2 GiB 465 ms against 518 ms; `copy-small`
  5,000 files 106 ms against 127 ms. Not measured: clones (Btrfs, XFS, APFS) and
  server-side copies (NFS, SMB), where the gain is the point.
- Docs: `docs/security.md` "Who moves the bytes", `docs/config.md`, CHANGELOG.
- Found on the way: a local `cargo package --workspace` verified `xdevcmd` against a
  stale build of the overlay's `xdevcmd-core` 0.4.0-beta.1 (cargo keys a registry
  crate by name and version, and the version has not moved since beta.1), so a new
  core module looked missing. It comes back with each new core API until the version
  moves. Clear all three: `rm -rf target/package
  ~/.cargo/registry/src/-*/xdevcmd-core-0.4.0-beta.1 && cargo clean -p xdevcmd-core`.
  CI builds from nothing and is not affected.

### 2026-11-13, the owner on Windows' fast copy

- Deferred: "because we are avoiding the unsafe, defer that till we really need that,
  or just let it to 3rd party." Core stays `forbid(unsafe_code)`; Windows keeps
  xdevcmd's own copy. The third-party route is item 44 (0.5): the plan handed to a
  trusted tool (robocopy, which uses the system's copy), its engine named at the top.

### 2026-11-13, part F: A03 (the viewer's progressive index) and B08 (one budget)

- **A03**, `document.rs`:
  - `open` indexes the first 4 MiB (`INLINE_INDEX`) itself; past that, a thread with
    its own handle on the file indexes the rest. It sends offsets over a bounded
    channel (16 chunks of 256 KiB), so it waits rather than piling up offsets when
    nothing takes them. Dropping the document (Esc) stops it.
  - While it runs, the line it is in the middle of is not counted (`line_count`), so
    every line drawn is whole. Search covers what is indexed and says so ("not in the N
    lines indexed so far; still indexing"). The cap of 4,000,000 lines works as before,
    inline or behind.
  - The viewer takes in what arrived on every tick (`App::background`, also for quick
    view) and draws again. The title says "N lines so far, indexing P%". The syntax scan
    of a file still being indexed reads to the end itself.
  - Memory documents (a shell's history, Messages) are still indexed whole: they are
    already in memory.
  - Measured (warm cache, 1 GiB log of 11.7 M lines, release): `open` 1.5–6 ms against
    114–135 ms for the whole index (to the 4 M cap). Cold disk and network not measured
    (no root here to drop caches); that is where the wait was long.
  - Tests: a 13 MB file opens with part indexed, the last known line whole, a search
    past it says nothing found yet; the rest arrives; the cap reached in the background
    stops at a whole line; dropped while indexing does not wait. pty: F3 on 1,000,000
    lines completes its count with no key pressed, End reaches the last line.
- **B08**, the producers without a bound were found's:
  - `find.rs`: `sync_channel(4096)` for every search and list check (the walk waits);
  - the pane takes at most 10,000 events a tick (`TAKE_PER_TICK`);
  - a search stops at 100,000 rows (`find::MAX_FOUND`) and says "stopped at 100000
    found — narrow the search to see the rest".
  
  Already bounded, checked: the terminal's history (`[terminal] scrollback`, a
  range), the log's queue (sync_channel), directory reads (16 threads), a copy's undo
  record (200,000, `MADE_CAP`), a job's progress events (one per MiB or 250 ms).
- **B08's test** (pty, Linux): a shell printing `yes` without end, a copy of 120,000
  files in the background, a find reaching its cap, F3 on 1,000,000 lines, all at once.
  Debug build: memory 15 → 162 → 162 MB (2 s apart, after the copy ended), 6 threads,
  14 descriptors. Esc closes the viewer at once (the test's own 310 ms wait included);
  F10 answers in 60–135 ms.
  - Found on the way: while the copy runs, memory climbs by its undo record (27 MB for
    120,000 files), bounded by its own cap; "level" is measured after it ends.
- A test of the syntax scan opened a 6 MiB file and needed every line: it waits for the
  index now (`Document::wait_indexed`).

### 2026-11-13, Linux checked on this machine: beta.1 against now (the owner)

`cargo xtask bench` (full, 5 runs a case after a warm-up, ext4, release), the
`v0.4.0-beta.1` tag built in a worktree, then this branch, one after the other:

| Case | beta.1 | now | |
|---|---|---|---|
| replace small files (synced, old to the trash) | 60,682 ms | 2,864 ms | 21× (group commit) |
| copy small files | 234 ms | 204 ms | −13 % (fast copy) |
| copy one large file | 81.3 ms | 76.5 ms | −6 % (fast copy; ext4 has no clone) |
| plan 200,000 (the cap) | 172 ms | 157 ms | −9 % |
| list 100,000 | 119 ms | 117 ms | level |
| undo a copy | 41.0 ms | 41.3 ms | level |
| start-up to the first frame | 10.6 ms | 10.0 ms | level |
| binary | 5.39 MiB | 5.46 MiB | +70 KiB, under 7 MiB |

Peak memory level in every case. Windows and macOS: the owner, on the beta.2 build.

### 2026-11-14, the rest of item 43 before the cut, and a debug log (the owner)

The owner: item 43's rest (lanes, checkpointed resume) before beta.2, and a way to
measure on their Windows and macOS machines and send the results back. Decided ("Go as
recommended"):

- **`XDEVCMD_DEBUG`**, in every build from 0.4.0-beta.2 on (betas and stable): topics
  `jobs` (phase times, engines, group batches, the probe, where a resume starts),
  `errors` (each error, retry and question: the OS code, kind, phase, attempt, answer),
  `all` or `1` for both. **No names**: no file, folder, user or host name; a file is a
  number, the same within a run. One `debug-<time>.jsonl` in the state directory (or
  `XDEVCMD_DEBUG_DIR`), flushed line by line, stops at 16 MiB, the newest few kept. The
  status line says it is on and where; the status history keeps it.
- **No `[debug]` section in the config**: a config line is forgotten and synced to other
  machines; the variable lasts a session (`setx` or a profile for longer).
- **Crash logs** (every build, unchanged) gain the debug log's path and last lines when
  it is on.
- **Docs**: troubleshooting.md "Sending a log with a report", the man page, the
  CHANGELOG; development.md "Reading a debug log".
- **Order**: the log and the xtask `mixed` case first; the owner measures today's code
  on Windows and macOS; meanwhile checkpointed resume (a big file goes on from its last
  synced checkpoint; finished folders are skipped); lanes if the numbers show creating
  small files is a large share; then the cut.
- **Built** (2026-11-14): `xdevcmd-core/src/debuglog.rs` (one atomic load a call site
  when off; a line a job, a batch, an error, never a file); hooks in `ops.rs` (job
  start and end, phases, batches, the probe, retries, failures, questions, the journal);
  `plan::fs_word`; start-up note and Messages; crash log tail. xtask: the `mixed` case
  (one large file, named to come first, and small files), `--only` (no build, so the
  workflow's xtask runs without Rust) and `--dir` (the drive measured), each run's debug
  log in `bench/results/debug-<host>-<date>/<case>/`. Tests: no names in a copy's log
  (core and pty), the topics, a word cannot carry a path, the crash tail, pruning.
- Linux, quick `mixed` (1,000 × 4 KiB and 200 MiB): 92 ms; the log says the small
  files took 15 ms and the large one 56 ms, so lanes could save at most about 15 ms
  here. The owner's Windows and macOS numbers decide.

### 2026-11-14, an independent review: three P0 fixes before beta.2 (the owner)

An outside review of this branch (4 agents) was checked claim by claim against
`69324a2` before acting on it.

- **Held**:
  - **a copy into itself through a link** (`plan.rs`, `ops.rs` copy and move compared
    the text with `starts_with`). Reproduced: `link -> d/inner`, copy `d` into `link`
    went about 500 levels deep and aborted on a stack overflow;
  - **`q` quit at once** (no question without jobs);
  - **`publish = true`** in `release.toml`;
  - the 2 s mtime slack for every file system (in fact up to 2.999 s: `as_secs`);
  - identical rows never dropped from a plan (`conflicts()` counts them);
  - no xattr, ACL or hard links kept, and nothing says so;
  - `p` needing OSC 133.
- **Wrong**:
  - journals have a version line (`v 3`);
  - security.md already says a new file can be short after a power cut;
  - pre-releases going to crates.io was never the plan (ROADMAP, 2026-11-13);
  - README marks Scoop and winget as planned (the commands under it still read as
    usable: for rc.1).
- **Fixed now** (plan approved by the owner):
  - `paths::within` and `paths::same_entry` (dev+ino on Unix, final paths on Windows);
  - `plan::into_itself` and `plan::onto_itself`, the one rule for the plan, copy and
    move. A link to a folder is still copied as a link, and a move to a name differing
    only in case is still a rename;
  - `q` dropped from Quit's defaults; quitting asks when a program runs in any shell
    (`TermPanel::busy_programs`);
  - `publish = false`, with crates.io by hand (`cargo publish -p`) in development.md.
- **For rc.1** (the owner):
  - the mtime slack only on FAT, exFAT and network drives;
  - bounded identical rows;
  - docs/compatibility.md, a platform table, "What xdevcmd doesn't do — and what to
    use" (rsync, robocopy, rclone);
  - Transparency in the README, saying most code is agent-written;
  - docs for running the bench.
- **Channels** (the owner, agreed): GitHub releases with build provenance first,
  crates.io from 0.4.0, a Scoop bucket of our own with checkver, winget after signing
  (SignPath Foundation after 0.4.0).

### 2026-11-14, checkpointed resume (item 43's rest, part one) (the owner: "làm checkpointed resume đi")

- **Built**: with safe operations on, a file larger than `ops::CHECKPOINT` (256 MiB),
  written under its temporary name, is synced every 256 MiB and then a journal line
  `ckpt <part> <src> <dst> <ident> <bytes>` follows. Data first, then the line, so a
  line never claims bytes that are not on disk.
  - **The tidy-up** keeps such a part, if it is xdevcmd's own name, holds at least
    `bytes`, and its job is a copy or move with sources. Its journal is kept too
    (`Interrupted::partial`, `::journal`), so a start that neither resumes nor discards
    offers it again.
  - **Continue** passes them in `Request::partial`. The new job writes them into its
    own journal and commits, then the old journal goes (`journal::forget`).
  - **Taking over** (`Worker::take_over`): only while the source's `Ident` (size,
    mtime, dev+ino or creation time) is unchanged. The part is renamed to this run's
    name, and a `ckpt` line for it is written at once. Then it is cut to the
    checkpoint and both files seek there; the clone is skipped and copy_file_range
    goes on from the offsets. A kept part that no file of the job reaches is removed
    at the end.
  - **The dialog** says "N big file(s) kept part way (size)". `d` "discard them"
    removes the parts and the journal; Esc leaves them for the next start.
  - **The debug log**: `checkpoints`, `checkpoint_ms`, `resumed_bytes` in `job_end`.
- **Tests**:
  - journal: a part is kept, the last line wins, a short part goes, a delete keeps
    nothing, discard removes both;
  - ops: a big file syncs at its checkpoint and a small one does not; resume goes on
    from the kept bytes (marked X, so a copy from the start would show) and cuts what
    is past the checkpoint; a changed source starts again; unused parts and the old
    journal go;
  - pty (Linux): a real `kill -9` after the first checkpoint of a 384 MiB copy, then
    Continue: the file is whole and `resumed_bytes` ≥ 256 MiB.
- **Finished folders** (the owner, after hearing why it was held back: "Build it"):
  built without the extra sync per folder. A folder copied with no failure under it
  (`sum.failed` unchanged, not stopped) gets a `done` line, the tag top-level sources
  already used, so older readers take it the same way.
  - If its last files are still in the group batch, the line waits for that batch
    (`Worker::finished`) and is written only if the batch goes through without a
    failure. Otherwise it is dropped: a resume walks the folder again, which is only
    slower.
  - Copies only: a move removes what it finished.
  - Resume passes `Request::done_dirs`. The scan (`scan_skipping`) and `copy_node`
    skip such a folder while its copy is a folder, counted once as skipped. The new
    journal carries the lines on, for a second kill.
  - Cost: one journal line (a write, no sync) per folder.
  - Power cut: a line can reach the disk before new files' data, the caveat
    security.md already gives for new files.
  - Tests:
    - core: a folder is told only without a failure, whether a file fails or its
      batch's sync fails (each guard broken on purpose fails the test);
    - core: resume skips a finished folder and walks it again once its copy is gone;
    - pty: the kill test finds a nested `done` line;
    - pty: a hand-written journal resumes without walking a finished folder.
- **Not measured yet**: the checkpoint sync's cost. One sync per 256 MiB should be
  small on an SSD; a USB stick is where it shows. The owner's run of a big file with
  `XDEVCMD_DEBUG=jobs` gives `checkpoint_ms` against `ms`.

### 2026-11-14, rc.1's planned work moved into beta.2 (the owner)

The owner: everything planned for rc.1 is done before beta.2, so that rc.1 is
fixes only ("cần làm xong trước beta.2, để rc.1 chỉ tập trung fix bug").
Decided:
- fuzzing with cargo-fuzz, by hand, with an xtask command;
- docs/compatibility.md covers config keys, CLI flags, `[keys]` action names, theme
  files and old journals.

- **Review fixes** (`8ebacb4`):
  - "identical" dates agree to the coarser time's precision, read from the times
    themselves (`compare::precision`: 2 s, 1 s, 10 ms, 1 ms, 1 µs, 100 ns, or exact).
    It needs no file-system detection, which matters because Windows' `fs_kind` cannot
    tell a FAT stick. Two tests had encoded the old 2 s rule (times made µs apart taken
    for the same) and now set equal times;
  - past the plan's cap, identical rows are counted (`identical_dropped`), and a job
    from a confirmed plan leaves identical files without a row.
- **Pillar 4** (`4f3af94`):
  - `scripts/smoke.sh` on every archive in release.yml: its files, ≤ 7 MiB, and run
    where the runner's CPU can (version = tag, the `--print` commands);
  - `actions/attest-build-provenance` in the release job;
  - security.md "Checking a download".
  - The smoke step first runs at the beta.2 tag. A failure there stops the draft, so
    it is safe.
- **Fuzzing**:
  - `fuzz/` (outside the workspace, nightly) has six targets: osc, paths, config,
    theme, archive_name, archive_list. The app's parsers are compiled there with
    `#[path]` (config, theme, decor and keys depend only on each other);
  - `cargo xtask fuzz [--secs N] [TARGET…]`.
  - The first 30 s run found a real violation. The quoted-name form in `found.rs`
    let a tab through (`'G++\t/ '`), against the module's "anything with a control
    character" rule. Fixed, with a regression test.
- **Release bar item 1**:
  - `validate_name` now refuses every control character on every OS (it refused
    only `/` and NUL on Unix), with the message "a control character (U+000A)";
  - MkDir (which allows `a/b`) checks the same and keeps its box open with the reason.
  - The Ctrl+\ render flake was already fixed in part D.
- **Pillar 3 docs**:
  - docs/compatibility.md;
  - guide.md: "What a copy keeps", "What xdevcmd doesn't do, and what to use", "File
    names". It was checked first that ESC and BEL in a name are not drawn (a render
    probe, removed);
  - rsync and robocopy examples in config.md's `[[menu]]`;
  - shell.md: what a list cannot read;
  - development.md: the bench protocol, running the bench on the owner's machines,
    fuzzing.
- **README**:
  - Transparency, in the owner's words, made accurate ("most of the code is
    agent-written");
  - a Platforms table of what is checked where: macOS not used by hand yet; the
    shell reports of zsh, fish, PowerShell and cmd not run; ConPTY's pass-through not
    checked;
  - Scoop and winget moved to "Planned".
- **Left for the owner before beta.2**:
  - the hand-test matrix (testing-by-hand.md), macOS first;
  - the bench numbers on Windows and macOS, which decide lanes;
  - CI on the branch, then the cut.

### 2026-11-14, the branch's first CI run, and its fixes (the owner: "chạy Ci và check kết quả")

- **Run 43632177290 at b40552e**: deny and the cross-check passed; every test job
  failed. It was the first CI on this branch: all of Round 033 had run only here.
- **What failed, and why** (none of it a fault in the program):
  - **F10 after a copy did nothing (Linux, here under load)**: the tests sent the quit
    key once the copied file was on disk, but the job had not ended yet, and its box
    takes every key but Esc and b. Proved here: under load the first F10 and the first
    Ctrl+q alike were taken, and the second ended it. The debug-log test now waits for
    " copied" first; the helper's comment says so.
  - **The 120,000-file copy in B08 timed out on the runner**: it gets 120 s, by its size.
  - **macOS, the 2 GiB copies ended at once**: item 22's fast copy clones on APFS, so
    the lost-terminal and quit-with-a-copy tests had no copy left to cancel. Both now
    run with `fast_copy = false`, as the fallback test already did.
  - **macOS, "would replace a newer file"**: written a few ms apart, the target is
    newer since the mtime precision change. The test gives both files one time.
  - **macOS, the drag test**: it waited for "got:", which the typed command line also
    holds. It now waits for a line that starts with it (two tests).
  - **Windows, relpath**: the test built `src/app` with `/` and got `src/app\main.rs`.
    `relative()` now writes each path with the platform's separator.
- **Checked here**: the pty suite twice under load (48 busy processes, 32 test
  threads): 119 passed each time; before, 3 failed.
- **Run 43633633488 at 7388b0d**: ubuntu x64, deny and the cross-check passed. The
  others failed in xdevcmd-core's own tests, which stopped `cargo test --all` before
  the pty tests on those runners:
  - **macOS, three tests of a 1 GiB copy** (two cancels, the checkpoint's sync): the
    same APFS clone, whole at once. They set `fast = false`.
  - **Windows, "1 ns newer"**: Windows' own clock keeps 100 ns, so the two times are
    one. That line is for the other systems only.
  - **Windows and Linux arm, the finished-folder test**: in `src`, `good` can finish
    while the failing batch is pending (a listing's order is the file system's), and
    waits for it, and is rightly not told. The test copies `good` first on its own.
- **Run 43634839479 at 8bff432**: all green but macOS, with one failure in the pty
  tests, which ran there for the first time: going past pictures sent 4, not 2. The
  test (from 943d6b9, on main) relies on 60 ms between keys against the 120 ms
  settle; the busy runner stretched its own sleeps past that. Each gap is now timed,
  and a gap of 115 ms or more allows one more picture, and only then. Checked:
  - with the settle at 0 it still fails (6 sent, no slow gap);
  - one write, or 20 ms apart, would not catch that (the decode is still running),
    so the 60 ms stays.
- **CI**: `cargo test --all --no-fail-fast` (the owner), so one run shows every failure.

### 2026-11-15, docs/benchmark.md (the owner: "viết docs chạy benchmark cho macOS và Windows")

- **docs/benchmark.md**, for whoever measures: what the `mixed` case does and never
  touches; before starting (names, a quiet machine, antivirus left on, a drive at a
  time); getting `xtask` with or without Rust; macOS, Windows and Linux step by step;
  slow drives; reading the output; what to send.
- **Found while writing it**, now in the doc:
  - a second run with the same `--host` on the same day replaces the first (the file
    is `<version>-<host>-<date>.json`), so a second run on one drive needs a second
    name;
  - on APFS (and Btrfs, XFS) the default fast copy is a clone, so the internal drive
    is measured twice, the second with `XDEVCMD_BENCH_FAST=0`;
  - the bench workflow's artifacts from 13 November predate `mixed` and `--only`, and
    a `bench/` branch ending in `-<digits>` sets that many files: the doc gives
    `bench/beta2-mixed`;
  - the load is printed on Linux only.
- **development.md** keeps the developer's reference and links there; its protocol and
  per-machine steps moved, so each is said once.
- **The bench run** `bench/beta2-mixed` built the three `xtask` downloads. Run as the
  doc says (Linux), it worked, but named its results `unknown-…`: the version was read
  from a Cargo.toml found at run time. It is now built in from the workspace's; the
  doc asks for the run's link, since the version stays the last release's until a cut.
- **CHANGELOG Unreleased** cut from 7.1 KB to 2.9 KB (the owner), nothing dropped; the
  style it follows is `memory/2026-11-15-docs-style.md`, a candidate for `reference/`.

### 2026-11-16, plan A10 approved: D0 (the owner: "Merged PR 24, duyệt nhé")

The plan is `claude/xdevcmd_beta2_plan_A10.md` in the claude.ai project (two review
series R1–R10, blind judges 92/92). The owner approved, with the judges' trims: the key
table cut to 5 tasks (copy, F8, F8 in a panelized list, clean Alt+C, F1), the footer
click on Linux only, the Windows Busy close-tab variant a known issue; and the user
rules and refusal texts as written there. #23 and #24 merged first.

- **Owner-hours ledger** (budget 9.5 h; cut back at 8 h or the end of day 3, whichever
  first; day 1 = 2026-11-16):

  | Date | What | Hours |
  |---|---|---|
  | 2026-11-16 | D0: approve plan, merge #23 #24 | 0.25 |

- **Speed baseline, corrected before it ran.** The plan had side A run `--only plan`,
  `--only list` and `--only mixed` each with `--save-baseline`. Read in xtask
  (main.rs:801), `--save-baseline` writes the whole file, so each run would replace the
  one before. Side A is one full run with `--save-baseline`; `--only` runs then compare
  against it by case name. xtask built at ad065d5 (the feat tip after #23 and #24):
  bench run 43885854166.
- **P5 (#6), red first.** Two tests through the user's path fail before the fix:
  Shift+F8 on 2,000 files stays at "counting…"; F3 on an entry of a .tar over 8 MiB
  never opens. Cause: `if let (Some(r), Some(x)) = (got, slot.take())` empties the
  slot on the first tick (run.rs:355, :387). Red CI run 43885704745 (5d64f54).

### 2026-11-16, plan A10: P1–P5 in pull requests, each red-teamed

Each data-loss fix went red test → fix → an independent red-team agent (the red-team
skill) → its P0/P1 findings red-first and fixed in the same PR. Every red-team found
real holes the fix's own tests missed.

| Fix | PR | Red run | Red-team found (fixed unless said) |
|---|---|---|---|
| P5 #6 count / archive view | #25 | 43885704745 | (not a data path: no red-team) |
| P3 #3 checked resume, Esc keeps | #26 | 43888463088 | P1 two starts offered one job; P1 C hung on a link to a FIFO; P2 a cancelled check counted; **F3 → known issue** (a mid-job Skip is not journaled) |
| P1 #1 typed line + Alt+C | #28 | 43889910529, 43896579060 | **P0 the user menu**; **P0 `\` continuation**; P1 `exec zsh` read as busy; P2 mouse bytes |
| P2 #2 #4 guards, loading | #29 (on #28) | 43898582362, run for ded92b2 | **P0 `link/` with a trailing slash emptied the link's target, home included — also in beta.1**; P1 `./x` rows dropped; P1 only the focused pane's folder guarded; P1 drop-move and user menu unguarded |
| P4 #5 file↔folder | branch feat/p4-file-over-dir (on #29) | run for 774fcc5 | red-team running |
| T1 docs | #27 | — | — |

- **Trust metrics so far:** red-team catches 3×P0, 7×P1, 2×P2 (all fixed) + 1 known
  issue; AI-introduced regressions caught before merge: P2's `./x` rows (my own fix).
- **Flaky, for the cut's list:** `everything_at_once_stays_bounded_and_keys_still_answer`
  failed once under full-suite load on a 2-core machine, passed alone and in a rerun;
  the exec test was flaky (1 in 3) until 919ee51 learnt the shell's name at the first look.
- **Merge order:** #25, #26, #27 any time; #28 before #29; #29 before P4.

### 2026-11-17, Track T: a tester's kit, and How to test (the owner's review of #32)

- **Release**: the build job also builds `xtask` for its six targets and ships it as
  `xdevcmd-bench-<tag>-<target>` (option C: built together, packed apart), after a
  smoke run from the unpacked archive; it never holds the release.
- **`xtask try`** (owner: before the beta.2 cut): `--make`, `--kill-when A|B|C`,
  `--check`, `--tamper`, `--clean`; walked at full size with the real binaries: A, B
  and C each killed mid-job, resumed, `--check` PASS. `xtask --help`, `--version`; it
  never runs or looks for an installed xdevcmd (owner: keep the coupling low).
- **Docs**: `testing-by-hand.md` → `how-to-test.md`, 11 steps in one line for anyone
  who wants to check a version, notes at the end; the maintainer's list moved to
  development.md, "Testing a pre-release by hand".
- **Reviews**: three doc reviews (a new tester, a QA lead, a technical check) and one
  red-team of the kit: no P0; five P1 fixed (kill timing, a manifest that passed when
  emptied, …). A copied xtask wrote its results into the source tree: fixed red-first.
- **For the owner**: `plan/ROADMAP.md` item 2 and closed rounds still say
  `docs/testing-by-hand.md` (history; ROADMAP is the owner's to edit).

### 2026-11-17, what is left before the cut (the owner: "list các việc phần mình và log cho bro")

All A10 fixes are merged: #25–#32 (#33, the guide's contents, open). Left, in order:

- **The owner**
  1. Merge #33. Delete the merged branches `feat/a10-docs`, `feat/a10-docs-guide-toc`
     and `fix/p5-count-live` (the session's proxy refuses ref deletes).
  2. **D0 speed baseline** on the Windows laptop, side A at ad065d5 (bench run
     43885854166): one full `--save-baseline` run, then `--only` cases against it
     (mains power, unattended ≈25 min). Side B at the cut.
  3. **The hand test** (development.md, "Testing a pre-release by hand"), macOS first;
     on Windows also the Alt+C/pwsh charter and the junction case (P4's known issue).
  4. Owner-hours in the ledger above (only 0.25 h is logged; cap 9.5 h, cut back at 8 h).
  5. At the cut: read Check, approve the speed A/B, merge feat → dev, publish the draft,
     say "close 033".
- **Claude Code**
  1. **Key table**: render.rs `shot_keys`, 5 tasks (copy, F8, F8 in a panelized list,
     clean Alt+C, F1), fails if a row needs more keys than beta.1; dead-end rows
     (Ctrl+O; F8 while loading, SLOW_DIR) assert a message; one footer click, Linux only.
  2. **One round-trip per refusal code** (DirtyLine, Busy, Protected, Loading,
     FileOverDir): refuse → golden debug line → press `next` → assert the action; a
     `code()` snapshot. Linux PR, red first where a test can fail.
  3. **Early speed check**, Linux: `--only plan` and `--only list`, beta.1 against the
     merged tree (≈3 min); the owner's side A decides the gate.
  4. **Flaky list**: new tests ×10 on Linux, ×3 on macOS and Windows; each flaky test
     named fixed or ignored (two listed in plan A10's status).
  5. **Round 033 Check**: evidence, red runs, red-team findings, flaky list, speed and
     key tables; the plan A10 doc's status brought up to date.
  6. **The cut**, after the owner's go: PR feat → dev (merge commit, CI, no
     `[skip ci]`), `cargo release 0.4.0-beta.2` (dry run, then `--execute`), author
     check, tag; draft notes (release notes first, refusal table, known issues);
     verify SHA256SUMS, the attestation, `--version`, and the six
     `xdevcmd-bench-v0.4.0-beta.2-<target>` assets.

## Check

Written 2026-11-17 by Claude Code, on `feat/v0.4.0-beta.2` at 5574138 (CI green, all six
systems). What is still the owner's is marked **owner**; the cut waits on it.

### 1. Plan A10's data-loss fixes (#25–#30), each red first

| Fix | PR | Red run | Red-team |
|---|---|---|---|
| P5 #6 count / archive view | #25 | 43885704745 | not a data path |
| P3 #3 checked resume, Esc keeps | #26 | 43888463088 | 2×P1 fixed, F3 known issue |
| P1 #1 typed line + Alt+C | #28 | 43889910529, 43896579060 | 2×P0 + P1 + P2 fixed |
| P2 #2 #4 guards, loading | #29 | 43898582362 | P0 (`link/` emptied its target, also in beta.1) + 4×P1 fixed |
| P4 #5 file↔folder | #30 | run for 774fcc5 | no data loss; 2 silent skips fixed |

### 2. QA: do the tests prove what a user expects? (#35, skill `qa`)

The user docs as the oracle; four audit and four hunt agents on the data paths
(`memory/2026-11-17-qa-beta2.md`, reports beside it).

- **Audit**: 319 promises; 109 PROVES, 137 CONFIRMS (screen text only), 57 MISSING,
  16+ MANUAL. The manual ones are cases in `crates/xdevcmd/tests/manual/`.
- **Hunt**: three P0 classes, each reproduced again by the main session, each fixed red
  first with an independent red-team:

| Finding | PR | Red-team |
|---|---|---|
| P0 F6 replacing a file on one drive: the old file gone for good | #36 | no P0/P1 |
| P0 F5 wrote through a link at the destination that the plan said it skipped | #37 | SIGN WITH AMENDMENTS (taken) |
| P0 a typed name ran inside a quote or here-doc left open | #39 | prompt check REFUSED and reverted; escaping: one P0 (backtick) found and fixed; final SIGN (107,060 fuzz runs, none ran) |
| P1 F2 / Shift+F2 changed non-UTF-8 name bytes | #38 | not a data path |

### 3. Proofs plan A10 asked for (#34)

- **One round trip per refusal code**, with its debug line read back: dirty_line,
  busy (`Ctrl+C`, pressed), protected_cwd/root/home, loading (`delete`, pressed). Red:
  the log wrote `CtrlC`; fixed. Not covered by a UI round trip: protected_not_plain (a
  list drops `.`/`..` rows first; the core table tests it); Windows' busy → close_tab.
- **Key table** (render, measured with the same keys on a v0.4.0-beta.1 build):

| Task | Keys now | beta.1 |
|---|---|---|
| copy a file | 3 | 3 |
| delete to the trash | 3 | 3 |
| delete from a list (Alt+F7's results) | 9 | 9 |
| Alt+C, line empty | 1 | 1 |
| run a command from F1 | 6 | 6 |

  Dead end: Ctrl+O names F12 (fails on beta.1, by design). Footer click on `8 Del`
  (Linux). New and changed tests: 10 runs, no failure.

### 4. Early speed check (Linux, this machine, 2026-11-17)

xtask `--only` at ad065d5 (side A, before the fixes) against 5574138 (side B). Medians:

| Case | A, pass 1 | A, pass 2 | B | B vs A |
|---|---|---|---|---|
| plan 10,000 | 15.3 ms | 16.2 ms | 18.0 ms (3 runs: 18.0, 18.7, 17.5) | **≈ +10 %** |
| plan 100,000 | 169 ms | 170 ms | 162 ms | −4 % |
| plan 200,000 | 349 ms | 375 ms | 351 ms | ±0 |
| list 10,000 | 19.4 ms | 20.5 ms | 16.9 ms | −15 % |
| list 100,000 | 220 ms | 224 ms | 229 ms | +3 % |

A against itself moved up to 7 %, so only plan 10,000 stands out: about 1.6 ms more,
under the ±15 % block, inside the 5–15 % band. By plan A10's rule: "slower by" in the
notes and an rc.1 issue. The gate is the owner's laptop run (D0), not this one.

### 5. Flaky tests, for the cut

| Test | Seen | State |
|---|---|---|
| `everything_at_once_stays_bounded_and_keys_still_answer` | full suite, 2-core | known, passes alone |
| `dragging_the_scrollbar_scrolls_the_list_and_the_cursor_follows` | macOS, once | known |
| `a_far_jump_in_long_lines_waits_for_the_scan` | windows-latest | fixed (#35: 1 s bound) |
| `a_directory_name_cannot_run_a_command_through_alt_c` | ubuntu-24.04-arm, once | passed on rerun; watch |

### 6. Known issues for the release notes (rc.1)

From A10: C after a power cut may replace a file answered "Skip" in the run that died;
Windows junctions; most terminal scenarios run only on Linux and macOS. From QA: F6 of a
folder onto a same-named folder on one drive is a dead end (os error 39, nothing lost);
a killed Alt+Y is not offered Continue; Ctrl+Z refuses a rename-many that swapped names;
Alt+F5 and tar extraction turn non-UTF-8 names into U+FFFD; bash's `read -s` is not seen
as busy; an open single quote in fish or PowerShell; names with a backtick are refused
for sh/bash/zsh; plan 10,000 ≈ 10 % slower on Linux.

### 7. Still to do before the cut

- **owner**: D0 speed A/B on the Windows laptop (side A: bench run 43885854166, side B:
  bench run 44009694319), two passes.
- **owner**: manual cases, at least M-DS-01..07 on macOS and Windows, one line each here.
- **owner**: read this Check, then the go for the cut (PR feat → dev, `cargo release`,
  tag, notes, verify the six `xdevcmd-bench-*` assets).

## Act
