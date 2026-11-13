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
  core module looked missing. `cargo clean -p xdevcmd-core` fixed it. CI builds from
  nothing and is not affected.

### 2026-11-13, the owner on Windows' fast copy

- Deferred: "because we are avoiding the unsafe, defer that till we really need that,
  or just let it to 3rd party." Core stays `forbid(unsafe_code)`; Windows keeps
  xdevcmd's own copy. The third-party route is item 44 (0.5): the plan handed to a
  trusted tool (robocopy, which uses the system's copy), its engine named at the top.

## Check

## Act
