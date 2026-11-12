# Round 029: Pillar 1, data correct (v0.4.0-beta.1, part 1 of 4)

**Status**: Review (steps 0–8 done, CI green 2026-11-13; the owner's go was 2026-11-13: "OK, go"; every open decision taken as recommended)
**Part of**: v0.4.0, step 6 of `ROADMAP.md` ("Step 6: three pre-releases", beta.1); pillar 1
of `memory/2026-11-12-four-pillars-for-rc1.md` (B01–B05, A08's sync gap); the owner's
decisions of 2026-11-12 and 2026-11-13 (below, §1)
**Date started**: 2026-11-13
**Date completed**: YYYY-MM-DD

## Goal

Before the Smart Planner is built on top of it, make the engine it stands on keep its
word: **before any step that cannot be undone, everything needed to recover is on
disk**; a journal that cannot be written is never taken for one that was; a plan made
from a directory that could not be read is never carried out; a file replaced after the
user agreed to replace something else is asked about again; and the two small selection
and parsing bugs the review found are gone. Every fix starts as a test that fails on
`feat/v0.4.0` today.

This is the data-safety core of the first stable release. Nothing here is visible as a
feature; what changes for a user is that a power cut, a full disk, a busy NAS or an
unreadable directory no longer has a quiet way to lose a file.

## Plan (kickoff review)

### 1. What changed, and where things stand

- **beta.1 is four rounds** (owner, 2026-11-13: "beta.1 = pillar 1 + Smart Planner v1,
  không vội"; GitHub pre-release only). The split, and why:

  | Round | What | Why here |
  |---|---|---|
  | **029** (this) | Pillar 1: B01–B05, the sync rules, a journal that is durable and says when it fails, the fault seam every later test uses | The planner's journal format, its re-check before each target and its "unreadable" rows are built from this round's pieces. Bugs first, each with a test that fails today |
  | [030](Round_030.md) | Smart Planner v1, the core: scan → plan → confirm → execute; modes by measured scan time; pre-checks; three warning levels; the journal holds the plan; exact resume | The one large new piece; lands and is used before anything consumes its plan |
  | [031](Round_031.md) | Smart Planner v1, around the executor: undo for a copy, replace into the trash, a network error pauses and asks, ETA, kept lists follow a move | Each consumes 030's plan (what was created or replaced, which device, which rows are left); small enough to land separately and to cut if beta.1 runs late |
  | [032](Round_032.md) | Pre-release plumbing and the cut: `release.yml` takes `-beta.N`/`-rc.N` from `dev`, `docs/development.md`, the beta.1 hand tests, the tag | Exercised only by a tag; last, so the tag carries everything |

  Item 42 is two rounds rather than one because a round must be "independently
  landable" (`DoD.md`) and reviewable in a sitting; 030 alone is about two weeks.
- **The owner's decisions this round carries out** (binding, 2026-11-12/13):
  - `sync_data` the temporary file before a rename that replaces an existing file; sync
    before deleting a source in a move across devices, every file; the journal's line
    synced before such steps and at job start. New files on the same device: no sync.
  - Pillar 1's fixes: B01 (an unreadable directory or a non-UTF-8 name never becomes a
    sync plan; `OsString` paths; Missing / Unreadable / Incomplete), B02 (the cursor lands
    on the chosen file after a slow move: the selection intent is resolved when the move
    completes), B04 (journal write, flush and begin failures surfaced, rename many too),
    B03 (panelize takes a whole line that exists, whole, before the `:line` heuristics),
    B05 (one fault test: the target replaced after confirm).
  - No new crates unless unavoidable; `#![forbid(unsafe_code)]` stays in our crates.
- **Since the review** (pinned at `c4f51c4`; checked again at `70ee6fa` in the
  four-pillars memory): Rounds 024–028 landed on `feat/v0.4.0` (F1, light theme, docs,
  tab logging, highlighting); none touched `ops.rs`, `journal.rs`, `compare.rs` or
  `batch_rename.rs`. Re-read for this kickoff at `0fcb2c7`, with three findings the
  review did not have (§4, marked **new**).

### 2. Map: the code each step touches

**Core, `crates/xdevcmd-core/src/`**

- **`ops.rs`** (2,311 lines)
  - `Request` (`existing`, `overwrite_confirmed`, `pairs`, `safe`), `Request::sync`.
  - `Worker::run`: `Journal::begin` (~400; a failure becomes `sum.journal_off`, said only
    at the end); the sync path builds `planned` from **whatever exists when the job
    starts** (~428–436: `.filter(|dst| dst.symlink_metadata().is_ok())`).
  - `resolve_conflict` (~556): the user's Yes is about the file it described
    (`existing_bytes`), but nothing remembers which file that was.
  - `copy_file` (~733) → `copy_bytes` (~792): a replacement is written to `part_name(dst)`,
    `flush`ed, `drop(w)` (a close error is lost), then `fs::rename(&target, dst)`. **No
    `sync_data`.** A new file with safe operations goes `part` → `rename_noreplace`.
  - `write_all_from` (~840): `BUF` = 256 KiB, `w.flush()` (a no-op for `File`).
  - `copy_symlink` (Unix, ~708): `let _ = fs::remove_file(dst);` then `symlink`: an
    approved replacement removes the old entry **before** making the new one.
  - `moved` (~699): during a move across devices, `fs::remove_file(src)` right after the
    copy is counted done. **No sync of the copy, no sync of its directory, no journal
    line.** `copy_node` removes an emptied source directory the same way.
  - `move_into` (~859): `fs::rename(src, &dst)` when the destination was approved;
    `is_cross_device` (~1216, EXDEV / ERROR_NOT_SAME_DEVICE) falls back to copy + delete.
  - `identical` / `Same` (~1031–1072): `Existing::Resume`'s size-and-date test.
  - Test seams already there: `appear_in_test`, `writing_in_test` (`#[cfg(test)]` statics);
    `other_device` in the tests (`/dev/shm`).
- **`journal.rs`** (934 lines)
  - `Journal::begin_in` (~180): `.new` → locked → renamed; lines written by `line`.
  - `line` (~238): `let _ = writeln!(…); let _ = flush();` **errors dropped, nothing
    synced**; `part`, `renamed`, `unrenamed` return `()`.
  - The module comment says lines are "written and flushed, not synced", on purpose.
  - `recover_in` / `claim` / `tidy`: tolerant of unknown tags (`_ => {}`), so new tags are
    read by 0.3.0 as nothing and by this version as facts.
- **`batch_rename.rs`** `apply` (~96): `Journal::begin(…).and_then(Result::ok)` turns a
  failure into `None` silently; `ren` lines unsynced before the files are parked under
  `.xdevcmd-rename-<run>-<i>`.
- **`archive.rs`**: extraction's replacement `fs::rename(&part, dst)` (~479) and pack's
  `fs::rename(&part, dest)` (~628): the same replace, the same missing sync.
- **`document.rs`** `save_bytes` / `write_new` (~335–370) already `sync_all`s before its
  rename, as does `session.rs` (~126): the pattern exists in two places; this round makes
  one helper of it.
- **`compare.rs`** (325 lines)
  - `entries` (~64): `read_dir(..).unwrap_or_default()`, `to_string_lossy` names, entries
    whose `symlink_metadata` fails dropped silently.
  - `Diff { rel: String, .. }`, `/`-joined; `Walk::walk` merges two sorted `String` lists.
  - `plan` (~208): `(left.join(&d.rel), parent(right, &d.rel))`: a lossy name joins into
    a path that does not exist.
- **`noreplace.rs`**: `rename_noreplace`; on Windows and on file systems without
  `RENAME_NOREPLACE` or hard links (FAT, exFAT, some SMB) it falls back to
  check-then-`rename`. Read only here; its window goes into the contract (pillar 3).
- **`found.rs`** `paths_in` / `candidate` (~57, ~150): the `path:line` split.

**App, `crates/xdevcmd/src/`**

- **`app/compare.rs`**: `compare_panes` (Alt+K marks), `begin_sync` / `poll_comparing`,
  `show_sync_plan` (the plan dialog, `Cmd::Sync(pairs)`).
- **`app/jobs.rs`**: `recover_interrupted` (the "Interrupted" dialog), `resume`
  (`Existing::Resume`), `remember_job`, `pump_job`.
- **`app/input.rs`** ~802 and **`app/jobs.rs`** ~370: `batch_rename::apply(&moves,
  ops::safe())`, the result shown as an `Outcome`.
- **`app/modals.rs`** / **`modal.rs`**: the job box's question for `ops::Question` (a new
  variant must be drawn and answered there).
- **`pane/files.rs`**
  - `activate` (~943): in results, `set_cwd(dir)` then `self.cwd.join(name)` and
    `select_path`: **B02**: after a slow move `cwd` is still the results' directory.
  - `navigate` / `read_bounded` (~1009–1045): `Reading { select: None, .. }`, always
    `None`; `poll_reading` (~1047) already selects `r.select` on arrival: the slot exists
    and nobody fills it.
  - `test_delay` (~143, `XDEVCMD_TEST_SLOW_DIR`): the slow-directory hook Round 022 added.
- The same set-then-select pattern in four more places: `app/shell.rs` ~244 (a link's
  Go), `app/file_ops.rs` ~283 (a typed path to a file), `app/panes.rs` ~194 (both panes
  on one directory), and `pane/files.rs` `activate` above.
- **`links.rs`** `Links::paths` (~100) and **`viewer.rs`** ~324 (panelize gathers
  `paths(i, shown)` per line); **`lists.rs`** (`fresh`, `checked`, `tidy_rows`) and
  `pane/files.rs` `poll_results` (~441–470): rows from a shell are checked on a thread
  and dropped quietly if they never existed.

**CI and tests**

- `.github/workflows/ci.yml`: one `test` job on ubuntu-latest, ubuntu-24.04-arm,
  windows-latest, macos-latest; `cargo clippy --all-targets --all-features`.
- `ops.rs` tests: `other_device` (`/dev/shm`), `run_answering`,
  `a_replacement_cancelled_part_way_keeps_the_old_file`,
  `a_move_across_devices_keeps_what_was_skipped`, `resuming_*`.
- `tests/pty.rs`: `an_interrupted_copy_is_tidied_up_and_resumed_at_the_next_start`,
  `resume_from_app_does_not_replace_a_kept_newer_source_file`.

### 3. Outside check (2026-11-13), only what applies to this round

The planner's references (robocopy `/L`, `rsync --itemize-changes`, Explorer's conflict
dialog, Total Commander's overwrite options) belong to Round 030 and are there.

- **SQLite, "Atomic Commit In SQLite"**: the rollback journal is written **and
  flushed to the platter** before the database file is changed; on Unix the
  **directory** is synced after the journal is created, or the journal itself can vanish
  in a power cut; deleting the journal is the commit. Taken: (1) the journal synced at
  job start, **its directory too** (Unix), before the first step it protects; (2) a
  line synced before the step it describes; (3) the end of a job is the journal's removal,
  which needs no sync (a journal that comes back after a power cut offers a finished job
  again, and Resume skips what is identical). Not taken: a journal of old contents (a
  rollback journal); the trash plays that part in Round 031.
- **PostgreSQL's fsync errors ("fsyncgate", 2018)**: on Linux a failed `fsync` may drop
  the dirty pages and clear the error, so **a retried `fsync` can succeed with the data
  gone**. PostgreSQL now panics rather than retry. Taken: a sync that fails means the
  temporary file is lost; it is removed and the copy started again from the source,
  never synced a second time.
- **rsync** writes each file under a temporary name beside it and renames it into
  place, as xdevcmd does since 0.2.1; it does not sync by default, and a `--fsync`
  option was long carried as a patch (rsync-patches, `fsync.diff`). The default trade
  is the same as the owner's: new files fast, durability where it is asked for.
- **ext4's `auto_da_alloc`** (2009) flushes a file renamed over another, which made
  "write then rename" safe there without `fsync`; XFS and others give no such promise.
  The explicit `sync_data` makes it true everywhere, not only on ext4's defaults.
- **macOS**: `fsync` does not empty the drive's cache; `F_FULLFSYNC` does, at a cost of
  milliseconds. To verify in step 2 by reading std's source: Rust's `sync_all` and
  `sync_data` on Apple targets call `fcntl(F_FULLFSYNC)`. If so, a replace and a move
  across devices pay a full flush per file there; measured, not guessed.
- **Windows**: `FlushFileBuffers` (std's `sync_data`/`sync_all`) works on files; a
  directory cannot be synced from std, and `MoveFileEx(MOVEFILE_WRITE_THROUGH)` would
  need `unsafe` or a new crate. NTFS journals its metadata, so a rename is either there
  or not after a crash, but the newest one may be lost. Written into the contract
  (pillar 3), not worked around.
- **robocopy `/Z`** (restartable mode) resumes a file from its middle; xdevcmd resumes
  whole files only. Unchanged here (Smart Planner v2, item 43).
- Sources:
  - [SQLite: Atomic Commit In SQLite](https://www.sqlite.org/atomiccommit.html)
  - [PostgreSQL wiki: Fsync Errors](https://wiki.postgresql.org/wiki/Fsync_Errors)
  - [Dan Luu: fsyncgate, the thread](https://danluu.com/fsyncgate/)
  - [rsync-patches: fsync.diff](https://mattmccutchen.net/rsync/rsync-patches.git/blob/HEAD:/fsync.diff)

### 4. Critique: what must be got right

**Durability (the owner's principle)**

- **The order for a replace**, in one helper used by every replacing write (`ops`
  `copy_bytes`, `archive` extract and pack, `document::save_bytes`):
  write the temporary file → `sync_data` it, **the `File` still open** (today `drop(w)`
  comes first and a close error is lost; the sync is what surfaces NFS and SMB write
  errors) → the journal's line for the step synced → re-check the target (B05) → `rename`.
  A sync that fails: the temporary file is removed and the step reported as failed, with
  Retry meaning "copy it again from the source" (fsyncgate), never a second sync.
- **The order for a move across devices**, per file: copy (under a temporary name, or in
  place with safe operations off) → `sync_data` the copy → rename into place → **sync
  the destination's directory** (Unix: `File::open(dir)?.sync_all()`; without it the
  synced file can still lose its name in a power cut, and the source is gone) → journal
  line `moved <src> <dst>` synced → remove the source. A source directory is removed only
  once empty, as today. Windows: the directory sync is not available (§3); said in the
  contract.
- **What is not synced, on purpose** (the owner's call): a new file on the same device,
  and the end of a job. **Said plainly, a consequence (new)**: with safe operations on,
  a *kill* never leaves a short file under a real name, but a *power cut* can (the data
  of a new file may not be on disk when its rename is). And Resume's "identical" test is
  size and date only; a file whose size and date reached the disk but whose data did not
  would be taken as done. Not fixed here; decision 6 and the contract (pillar 3).
- **The journal at job start**: the file synced, and on Unix its directory, before the
  job's first step. One sync per job (a few milliseconds), not per file.
- **The sync rules apply whatever the safe-operations setting** (decision 1): the
  temporary name for a replace was already "kept always" (Round 014); the setting decides
  only whether there is a journal and whether a new file goes under a temporary name.
- **Cost, to measure before deciding (decision 4)**: a replace or a cross-device move
  now pays a sync per file (and on macOS possibly a full flush). The owner's measure is
  10,000 × 1 KiB and 300 MiB, as Round 014's: on `/dev/shm` the sync is free; on a USB
  stick or an SD card it is not. The numbers go in Do.

**The journal (B04)**

- `Journal::line` returns `io::Result<()>`; so do `part`, `renamed`, `unrenamed`, and a
  new `commit()` (sync the lines written since the last one; nothing to do if none). A
  journal that failed once is **broken** and stays so: a later line could succeed and
  make it look whole.
- **A broken journal before a step that cannot be undone**: the worker stops and asks
  `Question::NoJournal { error }`: "Safe operations cannot write their journal (disk
  full?). Without it, a kill or power cut now cannot be tidied up or resumed." Buttons:
  **Stop** (the default: nothing more that cannot be undone is done) · Continue without
  the journal (once for the job; the summary says so). Never asked twice in one job.
- **A journal that cannot be begun** (state directory full, read-only, a file where the
  directory goes): asked **at the start**, before anything is done, the same question
  (decision 2). Today the job runs and says so at the end.
- **Rename many** (`batch_rename::apply`): the files are under temporary names between
  its two steps, so its journal is its whole safety. Every `ren` line is written and
  **synced once before the first file moves**; a `begin` or that sync failing means
  nothing is renamed and the dialog says why, with "rename anyway" as a second button
  (decision 3). An `unren` line failing later is harmless (`tidy` skips a temporary name
  that no longer exists) and is not asked about.
- **Old journals**: new tags (`moved`, a `v 2` line) are ignored by 0.3.0's `tidy`, and
  this version reads 0.3.0's journals as before. A test reads a 0.3.0 journal.

**B05: the file that was agreed to is the file replaced**

- **Identity**: `Ident { kind, len, mtime, dev+ino on Unix }` (Windows: kind, len,
  mtime, creation time; std has no stable file index). Taken when the question is asked
  (what the user saw) or when a plan is made; checked again right before the rename that
  replaces. Different → `Question::Conflict` again, with "it changed since you answered".
  "Replace all" and Update are rules, not a file: they are re-applied to what is there,
  not compared with a snapshot.
- **New: the sync plan replaces files nobody agreed to replace.** `Worker::run` builds
  `planned` from every destination that exists **when the job starts**. A file created
  at a "only on the left" destination between Alt+Y's compare and Enter is overwritten
  without a question. The plan must carry what it expected per row (`Absent` or
  `Replace(Ident)`); the worker compares, and anything else is asked.
- **New: names that differ only in case (macOS, Windows).** `compare` sorts by exact
  `String`; on a case-insensitive volume `A.txt` (left) and `a.txt` (right) are "only
  left" and "only right", and with the bug above each copy silently replaces the other:
  one file's content is lost on both sides. Fixed twice: the plan's `Absent` row now
  asks, and compare looks a one-sided name up on the other side (one `symlink_metadata`):
  found means a clash, shown and left to a person.
- **New: replacing a link is not atomic.** `copy_symlink` removes the destination, then
  makes the link; a failure in between (permission, a full directory) loses the old
  file. The link is made under a temporary name and renamed over, like a file.
- **What cannot be closed**: between the re-check and the rename another program can
  still replace the target; no portable rename compares first. The window shrinks from
  "the whole copy" to microseconds; the contract says so. `rename_noreplace`'s fallback
  on Windows and FAT has the same kind of window (check, then rename).

**B01: compare**

- **Names are `OsString`** end to end: `Diff.rel` becomes a relative `PathBuf` (the
  display text is made where it is drawn); both sides sort by the same `OsStr` order.
  Two non-UTF-8 names that read the same lossily are two rows.
- **States for what could not be decided**: `Unreadable(side, error)` (a directory whose
  `read_dir` fails: the whole subtree is one row, and **neither side's entries under it
  are planned**, since "only on the other side" means nothing there), `Missing(side)` (a
  directory gone while compared), `Incomplete(side)` (some entries could not be read or
  stated: a one-sided name in that directory is not planned, because its twin may be one
  of the unread). A root that cannot be read is an error, not a plan.
- **Shown**, in the sync dialog as rows with ⚠ and a count in the title, never as
  "nothing to do"; Alt+K marks nothing under them and its status line names them.
- A test **as root** cannot make a directory unreadable with `chmod 000`; that test
  skips when the effective uid is 0 (the sandbox here is root; CI is not), and the seam
  (§7) covers the same path everywhere.

**B02, B03**

- **B02**: one entry point, `FilePane::go_to_file(dir, name)`, which stores the intent
  (`Select::Name(OsString)`) on the pending move and resolves it **against the listing
  that arrives** (`cwd.join(name)` after canonicalising, so a link in the path works).
  A move that replaces the pending one drops its intent; Esc drops it; an intent wins
  over "came up out of a directory". The five call sites in §2 move to it. A name that
  is not in the new listing (hidden, filtered) leaves the cursor at the top and says so.
- **B03**: a line that **as a whole** names something that exists is that path, before
  any heuristic. Lookups must not run on the UI thread for 10,000 lines, so the line's
  candidates go to the list as a group (`whole`, `parts`) and the list's existing check
  thread decides: `whole` exists → the parts are dropped; otherwise `whole` is dropped
  quietly and the parts are checked as today. A line with only one candidate is
  unchanged. Trailing whitespace: the line as shown, then without trailing spaces.

**Tests**

- **Each fix first lands its test failing** on today's code (the commit message names
  the run that failed), as Round 014 did; `cargo test` stays green at each commit.
- **Order is tested, not trusted**: the seam records the calls it sees, so a test can
  assert "sync, then journal sync, then rename, then remove source". Without that, a
  sync moved after the rename would pass every content check.

### 5. Steps

Each step is one commit with its tests; the failing test is written first. Sizes: S a
day or less, M two to three days. Clippy on Linux, windows-gnu and darwin and the full
suite at each step.

0. **The fault seam and the test places** (`faults.rs` new in core; `ci.yml`): §7's
   seam, `#[cfg(any(test, feature = "faults"))]`, a no-op otherwise; `XDEVCMD_FAULTS`
   read only with the feature (pty tests); a call trace for order tests; CI mounts a
   small file system (Linux: tmpfs by `sudo mount`; macOS: a RAM disk by `hdiutil`) and
   sets `XDEVCMD_TEST_SMALL_FS`, `XDEVCMD_TEST_OTHER_DEV` (Windows: the runner's D:).
   Tests: the seam fails the n-th write of a path and only that path; a release build
   holds no `XDEVCMD_FAULTS` string (`budgets.rs`, after the build). **S.**
1. **B04, a journal that is durable and says when it fails** (`journal.rs`,
   `ops::Worker::run`, `Question::NoJournal`, the job box, `batch_rename::apply`, the
   rename-many dialog). `line → Result`, `commit()`, `begin` synced with its directory,
   `v 2` line. Failing first: (a) the seam fails a journal line before a replace →
   today the replace goes ahead and nothing is said; (b) rename many with a file where
   the journal directory goes → today the files are renamed with no journal and no word;
   (c) `begin` fails → today asked nothing until the end. Also: a 0.3.0 journal is still
   tidied. **M.**
2. **A replace is durable** (`ops::commit_replace`, the one helper; `copy_bytes`,
   `archive` extract and pack, `document::save_bytes`; `copy_symlink` through a
   temporary link). Failing first: the call trace of a replace has no `SyncData` before
   `Rename`; a link replace whose creation fails loses the old file. Also: a sync error
   removes the temporary file, keeps the old one, and Retry copies again (the trace
   shows a second write, not a second sync). Verify std's Apple `sync_data`. **M.**
3. **A move across devices is durable** (`Worker::moved`, `copy_node`'s directory
   removal, `journal` `moved` lines, `Interrupted` and its dialog). Failing first: the
   trace has `RemoveSource` with no `SyncData`/`SyncDir`/`JournalSync` before it.
   Kill test (pty, Unix): the seam halts after the journal commit, the test kills the
   process, the next start says both the source and its complete copy are there. A sync
   failure keeps the source and removes the copy. Measured: 10,000 × 1 KiB and 300 MiB to
   `/dev/shm`, the owner's USB stick by hand (decision 4). **M.**
4. **B01, compare never plans what it could not read** (`compare.rs` `entries`, `Walk`,
   `Diff`, `State`, `plan`; `app/compare.rs` marks, plan dialog, status). Failing first:
   a `chmod 000` directory on the right makes today's plan copy everything into it; two
   non-UTF-8 names (Linux) plan a source that does not exist; the seam makes a stat fail
   mid-walk and the other side's file is planned. **M.**
5. **B05, the file agreed to is the file replaced** (`Ident`; `resolve_conflict` keeps
   it; `commit_replace` and `move_into` re-check; `Request::sync` carries `Expect` per
   pair, `planned` removed; compare's case-clash lookup). Failing first: **the fault
   test**: the hook replaces the target after the user answered Yes → today it is
   overwritten; a file appearing at a planned "only left" target → today overwritten;
   `A.txt`/`a.txt` (runs where the temp directory ignores case: macOS and Windows CI).
   **M.**
6. **B02, the cursor lands on the chosen file** (`FilePane::go_to_file`, `Reading.select`,
   five call sites). Failing first, with `XDEVCMD_TEST_SLOW_DIR`: Enter on a file in
   results whose directory is slow → today the cursor is on `..`. Also: a second move
   drops the first's intent; Esc drops it. **S.**
7. **B03, a whole line that exists is taken whole** (`links.rs` `paths` → groups,
   `lists.rs` first check, `pane/files.rs` `poll_results`). Failing first: lines
   `report:2026.txt`, `a b.txt`, `2026` (all three exist) panelize into the wrong rows
   today. Also: a line that does not exist as a whole gives today's rows. **S.**
8. **Docs** (`docs/security.md` "Safe operations": what is synced and when, the new
   question, what a power cut can still do; `docs/guide.md`: compare's new rows; the
   CHANGELOG). The full recovery contract is pillar 3 (rc.1); this writes only the facts
   that changed. **S.**

About two weeks (5 M, 4 S). Steps 6 and 7 do not depend on the others and can be taken
first if a short step is wanted to start.

### 6. The owner's decisions

Settled: §1. Open below; **the coder follows each recommendation unless the owner
objects**, and nothing waits on them except decision 4's measurement.

1. **The sync rules and the setting.** *Recommended:* they apply whether safe operations
   are on or off; the setting decides only the journal and new files' temporary names.
   A replace already goes through a temporary name either way.
2. **A journal that cannot be begun.** *Recommended:* asked at the start ("continue
   without / stop", stop the default), not only said at the end as today. Alternative:
   asked only before the first step that cannot be undone (no question for a copy of
   new files).
3. **Rename many without its journal.** *Recommended:* nothing renamed, the reason shown,
   and a second button "rename anyway". Alternative: refuse outright.
4. **A sync per file in a move across devices.** *Recommended:* per file first, measured
   (10,000 × 1 KiB) on the owner's USB stick and on macOS; if the move becomes more than
   twice as slow there, this round batches (copy and sync up to 64 files or 64 MiB, one
   journal line for the batch, then remove those sources), which keeps "every file
   synced before its source goes". Otherwise batching waits for v2's group commit.
5. **Failure points compiled into the shipped binary?** *Recommended:* no: a cargo
   feature `faults` (`#[doc(hidden)]`), on only for tests; the release build is checked
   not to have it. Delays may stay plain environment hooks, as `XDEVCMD_TEST_SLOW_DIR`
   is today, since they only ever wait.
6. **After a power cut, Resume trusts size and date.** *Recommended:* say it in the
   contract now; in Round 030, where the journal holds the plan, record Linux's
   `boot_id` and, after a reboot, copy again the files the dead run itself created
   (their creation time after its start) instead of taking them as done. Alternative:
   sync new files too (the owner decided against it for speed).
7. **Windows: no directory sync.** *Recommended:* accept and write it down; no `unsafe`,
   no new crate for `MoveFileEx(WRITE_THROUGH)`.
8. **Case-only clashes in compare.** *Recommended:* found by looking the name up on the
   other side, shown as "same name, different case: left alone", never planned. On a
   case-sensitive disk the lookup finds nothing and the row stays one-sided, as today.
9. **CI's small file systems.** *Recommended:* yes (Linux tmpfs via `sudo mount`, macOS
   RAM disk via `hdiutil`), about ten seconds a run; tests skip when the variables are
   unset, so a developer's machine needs nothing.

### 7. The beta.1 test plan (all four rounds)

**Principles.** No `unsafe`, no root on a developer's machine, nothing mounted by a test
itself. Every fault has two kinds of proof where possible: the seam (every platform,
deterministic) and one real kernel error (where CI can make one). Order is asserted from
the seam's trace, not inferred from results.

**The fault seam** (`xdevcmd-core/src/faults.rs`, step 0)

- `faults::at(Point, &Path) -> io::Result<()>`, called before each real call it guards:
  `Write` (each chunk in `write_all_from`), `SyncData`, `SyncDir`, `Rename`,
  `RemoveSource`, `JournalBegin`, `JournalLine`, `JournalSync`, `ReadDir` and `Stat`
  (compare and the planner's scan), and in Round 031 `Read` for network errors.
- Rules, process-wide (the worker runs on its own thread, so not thread-local), each
  matching a path fragment so parallel tests in their own temporary directories never
  meet: `Fail { point, path, nth, error }` (an `io::Error` built from an `ErrorKind`,
  `StorageFull` included, or a raw OS code: ENOSPC, EIO, ETIMEDOUT, ESTALE, ENOTCONN;
  `from_raw_os_error` is safe), `Delay { point, path, by }` (network latency, Round
  031), `Halt { point, path }` (writes a marker file and blocks, for kill tests), and
  `Swap { point, path, with }` (B05: replaces the target at that moment).
- A trace of `(Point, path)` per matching rule set, for order tests.
- Compiled with `cfg(any(test, feature = "faults"))`; otherwise `at` is an inline
  `Ok(())`. The `xdevcmd` crate enables the feature in its dev-dependencies, so the
  binary the pty tests run has it (to confirm in step 0: dev-dependency features apply
  to the test build's binary target under resolver 2), and parses `XDEVCMD_FAULTS` (for
  example `fail:sync-data:b.txt:28`, `halt:remove-source:a.txt`). A test in `budgets.rs`
  checks the release binary has no such string.

**Real errors, without root**

- **EFBIG** (Linux and macOS, every machine): a pty test starts xdevcmd through
  `sh -c 'trap "" XFSZ; ulimit -f N; exec xdevcmd …'`. A write past N blocks fails with
  EFBIG from the kernel instead of killing the process (the ignored SIGXFSZ survives
  `exec`): the real error path through std, not the seam's. Checked here: `dd` under it
  gets "File too large" and exits normally.
- **ENOSPC** on CI: GitHub's Linux runners allow `sudo`, so a step mounts
  `tmpfs -o size=4m` at `/mnt/xdev-small` and hands it to the runner user; macOS
  attaches a RAM disk (`hdiutil attach -nomount ram://8192`, `diskutil erasevolume`).
  `XDEVCMD_TEST_SMALL_FS` names it; tests skip without it. A user-namespace mount would
  avoid `sudo`, but Ubuntu 24.04 restricts unprivileged namespaces; `sudo` on CI is
  simpler and honest. `/dev/full` (Linux) gives ENOSPC on every write, for a writer unit
  test that takes any `Write`.
- **Permission denied**: `chmod 000` on Unix, skipped when running as root.

**Across devices**

- Linux: `/dev/shm` (tmpfs), already used by `other_device`; on CI also the small tmpfs.
- macOS: the RAM disk. Windows: the runner's two volumes (`C:` temp, `D:` workspace),
  named by `XDEVCMD_TEST_OTHER_DEV`. A test needing a second device skips without one,
  and says so in its output.

**Kills and power cuts**

- **A kill at each phase** (pty, Unix): `Halt` at the point, wait for its marker, kill
  the process with SIGKILL, start again, assert what the "Interrupted" dialog says and
  what is on disk. Phases: mid-write of a new file; after the sync, before the rename of
  a replace; after the journal's `moved` line, before the source is removed; between
  rename many's two steps. Windows: the same through `windows_e2e.rs` where it can kill
  (`TerminateProcess` via `taskkill /F`), else by hand.
- **A power cut cannot be made in CI.** What is checked is the order (trace tests) and
  the reasoning in the contract. A later candidate, not in beta.1: LazyFS (a FUSE file
  system that drops unsynced data on command), runnable on GitHub's Linux runners with
  `sudo`; noted for pillar 4.

**Network** (Round 031)

- The seam's `Delay` and `Fail` with ETIMEDOUT / ESTALE / ENOTCONN / EIO (Windows: 59,
  64, 121) at `Read`/`Write`, n-th time then succeeding: the job pauses, asks, retries
  with backoff, and carries on; a fake clock for the backoff and the ETA so no test
  sleeps for seconds. A real mount stays a hand test (the owner's NAS over SMB and an
  sshfs mount), with the checklist in Round 032.

**Platforms**

- CI runs everything on Linux x86_64 and ARM, macOS and Windows; the seam tests run on
  all four. What only one platform can show: non-UTF-8 names (Linux; Windows with an
  unpaired surrogate built by `OsString::from_wide`), case-insensitive clashes (macOS,
  Windows), EFBIG (Linux, macOS), directory sync (Unix).
- By hand, per pre-release (the checklist in Round 032): a USB stick (FAT32 and exFAT)
  for the 4 GB check, the sync cost and case clashes; an SMB share for pause and retry;
  pulling the stick mid-copy (the nearest thing to a power cut on a laptop).

### Not doing (this round)

- The Smart Planner itself (Round 030); undo for a copy, replace into the trash, network
  pause and ETA, kept lists following a move (Round 031); the release plumbing (032).
- The recovery contract as a document (A08's contract, pillar 3, rc.1): this round only
  updates the facts in `docs/security.md`.
- Syncing new files, group commit, resuming a file from its middle (v2, item 43).
- `MoveFileEx(WRITE_THROUGH)` and a directory sync on Windows; closing
  `rename_noreplace`'s check-then-rename fallback on FAT and Windows (contract).
- Panelize's NUL mode (`find -print0`), 0.5. Pillar 2 (A06, A07, B08), beta.2.

## Do

Coder's log, newest last. "Failing first" is the run of the new test on the code before
the fix, kept here because the commit carries test and fix together.

### 2026-11-13, step 0: the fault seam and the test places

- `xdevcmd-core/src/faults.rs`: `Point` (the ten of §7), `at(point, path)`, rules
  `Fail { nth, error }` (an `ErrorKind` or a raw OS code), `Delay`, `Halt { marker }`,
  `Swap { target, body }` and `Skip` (leave a sync out, for step 3's measurement only),
  each held by a guard and matched on a path fragment; a trace per fragment for order
  tests; wrappers `sync_data(&File, path)` and `sync_dir(dir)` (Unix; EINVAL/ENOTSUP from
  a file system that cannot sync a directory taken as nothing to sync; Windows: nothing,
  decision 7). `XDEVCMD_FAULTS` (`fail|<point>|<frag>|<n>[|<os code>]`,
  `halt|<point>|<frag>|<marker>`, `delay|<point>|<frag>|<ms>`, `;`-separated; `|` and not
  the plan's `:`, which cuts a Windows path in two, changed in step 4) read once,
  only in a build with the seam.
- Compiled under `cfg(any(test, feature = "faults"))`; otherwise `at` is an inline
  `Ok(())` and nothing else of it exists. `xdevcmd` turns `faults` on through a
  dev-dependency on its own engine. **Confirmed** (resolver 2): the program `cargo test`
  builds for the pty/render tests holds `XDEVCMD_FAULTS`; `cargo build --release` does
  not (string search, 0 matches; the binary is stripped, so `nm` proves nothing).
  `tests/budgets.rs`
  `the_fault_seam_is_in_the_tests_program_and_not_in_the_release_build` checks both; CI
  now runs `cargo test -p xdevcmd --test budgets` again after `cargo build --release`, so
  the release half is checked there (before, the release size check always skipped on
  CI: `cargo test` runs before the release build).
- First guarded call: `ops::write_all_from`, `Point::Write` per chunk.
- Test places: `faults::places::{other_device, small_fs}` (`XDEVCMD_TEST_OTHER_DEV`, else
  `/dev/shm` on Linux; `XDEVCMD_TEST_SMALL_FS`), each skipping with a line on stderr.
  `ci.yml`: Linux mounts two tmpfs (4 MiB to fill, 64 MiB to move to) with `sudo mount`;
  macOS two HFS+ RAM disks (`hdiutil attach -nomount ram://…`, `diskutil erasevolume`);
  Windows names `$RUNNER_TEMP\xdev-other` (D:, the temp directory is on C:).
  Two mounts, not one: a test filling the small one must not fail another test moving
  files onto it at the same moment.
- Tests: the seam (nth pass of one path only; a dropped rule is gone; trace order; swap
  once; `XDEVCMD_FAULTS` parsing), and a real ENOSPC:
  `a_replacement_that_fills_the_disk_keeps_the_old_file` (passes on today's code: the
  place works; checked here with a 4 MiB tmpfs mounted by hand, and skipped without it).
- Nothing to fail first in this step: it adds the seam, no fix.

### 2026-11-13, step 1: B04, a journal that is durable and says when it fails

- `journal.rs`: `line`, `part`, `renamed`, `unrenamed` return `io::Result`; new
  `commit(about)` syncs the lines written since the last one (`sync_data`), nothing if
  none; a failed line or sync sets `broken` and every later call fails (`broken()` says
  why). `begin_in` passes `JournalBegin`, writes a `v 2` line after `host`, then syncs
  the file and (Unix) its directory before returning; any failure there drops the
  half-made journal (its `Drop` removes the file). Module comment rewritten: "flushed,
  not synced" is no longer the rule.
- `ops.rs`: `Question::NoJournal { error }`; `Worker::go_without_journal` asks it once
  (Yes goes on and fills `journal_off`; anything else stops, `abort`), and drops the
  journal file either way. Asked at the start when `begin` fails (decision 2) and, through
  `Worker::durable(about)` (= `commit`), before each step that cannot be undone: the
  rename that replaces in `copy_bytes`, a link replaced in `copy_symlink`, the same-device
  `fs::rename` over an approved destination in `move_into`, the source removal in `moved`,
  and archive extract/pack replacing (new `archive::Hooks::durable`). A `part` line that
  fails breaks the journal without a question (a new file is not irreversible); the
  summary then says "its journal failed: …" (`Worker::finish`, now the one place a job
  ends).
- `batch_rename::apply` → `apply_with(moves, Option<io::Result<Journal>>)`: every `ren`
  line written and **one commit** before the first file moves; `begin`, a line or that
  sync failing → nothing renamed, `Outcome::no_journal`. `unren` failures ignored (tidy
  skips a temporary name that is gone). The journal now names the files as its sources.
- App: the job box draws `NoJournal` ("Safe operations: no journal", **S** stop / Esc,
  **C** continue without the journal; red, like Failed). Rename many and its undo go
  through `App::rename_many`: on `no_journal` a dialog says nothing was renamed and why,
  with **rename anyway** (`Cmd::BatchRenameAnyway { moves, undoing }`, refused in
  read-only like `BatchRename`).
- Failing first (each run against the step-0 commit in a worktree, tests compiled there
  unchanged by asserting on `{:?}` text):
  - (a) `a_journal_that_fails_before_a_replace_is_asked_about_and_stop_keeps_the_old_file`:
    asked only `Conflict`, no `NoJournal`; the file was replaced.
  - (b) `rename_many_whose_journal_cannot_be_begun_renames_nothing_and_says_why`:
    `Outcome { renamed: 2, errors: [] }`.
  - (c) `a_journal_that_cannot_be_begun_is_asked_about_before_anything_is_done`: nothing
    asked (`[]`), the file copied.
- Also: `rename_many_with_a_file_where_its_journal_goes_renames_nothing` (the real
  ENOTDIR, and a `ren` line failing), `a_journal_that_failed_once_stays_broken`,
  `a_journal_of_0_3_0_is_still_tidied_and_new_tags_are_skipped`. The pty test
  `a_job_without_its_journal_says_so` became `…_asks_first_and_says_so` (Stop copies
  nothing; continue copies and says so).
- Not tested end to end: the rename-many dialog itself (the suite has no test driving
  Shift+F2's editor); the core outcome it shows is.

### 2026-11-13, step 2: a replace is durable

- `ops::commit_replace(f, tmp, dst, ready)`, the one helper: `sync_data` on the temporary
  file **still open** → close → `ready()` (the worker's `durable`: the journal's line
  synced; step 5 adds the re-check) → `Rename` point → `fs::rename`. Anything short of
  the rename removes the temporary file; the old `dst` is untouched. A failed sync is
  never retried on the same file: the error goes up, the copy loop asks `Failed`, and
  Retry writes the file again from its source (fsyncgate).
- Used by `ops::copy_bytes` (replacing), `archive::write_entry` (extract over a file;
  `write_part` now hands back the open `File`), `archive::pack` (the archive writers own
  and close the file, so it is opened again for writing and synced: the sync is of the
  file whichever handle asks; on Linux an unseen writeback error is still reported to a
  new handle) and `document::save_bytes` (Save as… replacing; it used `sync_all` before,
  now `sync_data` through the helper). `session.rs` (app crate) already syncs and is left.
- `copy_symlink` (Unix): a replacing link is made under `part_name(dst)` and renamed over
  the old entry (journal part line, `durable`, `Rename`), never "remove, then make".
- New files on the same device: still no sync (the owner's rule). A `Rename` point now
  also sits before `rename_noreplace` in `copy_bytes`, for order tests.
- **A bug caught on the way**: the first cut of `document::write_new` removed `path` on
  any error, including `AlreadyExists` from `create_new`: Save as… without "replace"
  onto an existing file would have deleted it. `save_bytes_never_replaces_without_being_told`
  failed on it at once; fixed before commit (`create` fails before anything is made, and
  only what was made is removed).
- **macOS** (std 1.97.0, `library/std/src/sys/fs/unix.rs`): `File::sync_data` and
  `File::sync_all` both call `fcntl(fd, F_FULLFSYNC)` on every Apple target (`fsync` is
  not used: it does not empty the drive's cache there). So every replace on macOS pays a
  full flush of the drive's write cache; its cost is measured in step 3.
- Failing first (against the step-1 commit; the link test with the `Write` point put in
  front of the old `symlink` call, which is how a failing `symlink()` looks there):
  - `a_replacement_is_synced_before_it_is_renamed_into_place`: `no sync-data in ["write"]`.
  - `a_failed_sync_keeps_the_old_file_and_retry_copies_again`: only `Conflict` asked; no
    sync to fail, the file replaced.
  - `a_link_that_cannot_be_made_does_not_lose_the_file_it_was_to_replace`: reading the
    old file: `NotFound`: lost.
  - `extract_and_pack_sync_a_replacement_before_renaming_it`, `saving_over_a_file_syncs_before_renaming`:
    empty trace (no sync, no rename point).

### 2026-11-13, step 3: a move across devices is durable

- Per file, in this order, every file's copy on disk before its source goes:
  copy (temporary name, or in place with safe operations off) → rename into place →
  **its data synced** → **its directory synced** (Unix) → the journal's `moved <src> <dst>`
  line, **synced** → `RemoveSource`. A directory made on the other side syncs its parent
  before anything under it loses its source; an emptied source directory is removed
  (through `RemoveSource`) after its files' batch. `Interrupted::moved` lists the pairs a
  dead run left with both files there; the start-up dialog says "N file(s) of a move had
  been copied completely, and its source is still there … Both are kept; nothing was
  removed." Resume then leaves such a source (`Summary::left`, as before).
- **Measured** (this sandbox: `/tmp` on a virtio disk, ext4-backed `/dev/vda`; the other
  device tmpfs `/dev/shm`; release build; the journal in `/tmp`;
  `measure_a_move_across_devices_with_and_without_syncs`, the "no sync" run leaving out
  `SyncData`, `SyncDir`, `JournalSync` through the seam's `Skip`):

  | Move | per file, unbatched | no sync | ratio |
  |---|---|---|---|
  | 10,000 × 1 KiB, disk → tmpfs | 3.86 s | 1.83 s | 2.1× |
  | 10,000 × 1 KiB, tmpfs → disk | 11.86 s | 1.42 s | **8.4×** |
  | 3 × 100 MiB, disk → tmpfs | 0.43 s | 0.30 s | 1.4× |
  | 3 × 100 MiB, tmpfs → disk | 0.53 s | 0.16 s | 3.3× |

  More than twice as slow, so **batched** (decision 4): sources are removed 64 files or
  64 MiB at a time (`MOVE_BATCH_FILES`, `MOVE_BATCH_BYTES`), and a batch first syncs each
  copy's data (written all first, synced after, so the system writes them out together),
  then each distinct directory once, then the journal once, then removes the sources.
  First cut (data synced per file before its rename, the rest batched): tmpfs → disk
  7.03 s vs 1.52 s, 4.6×. Final (three runs, tmpfs → disk):

  | Move | batched | no sync | ratio |
  |---|---|---|---|
  | 10,000 × 1 KiB, disk → tmpfs | 1.40 s | 1.37 s | 1.0× |
  | 10,000 × 1 KiB, tmpfs → disk | 4.12 / 4.32 / 4.60 s | 1.75 / 1.85 / 1.81 s | **~2.4×** |
  | 3 × 100 MiB, tmpfs → disk | 0.50–0.55 s | 0.16–0.19 s | ~3× |

  Still above 2× onto a real disk: what is left is one data sync per file (small files)
  and the disk writing what the unsynced run leaves in memory (large files, where "no
  sync" has not written the data at all when it reports). Going further needs a sync of
  the whole file system per batch (`syncfs`, Linux only) or group commit: v2 (item 43).
  The owner's USB stick and a Mac are still to measure (CI: a `workflow_dispatch` run now
  has a step "measure the sync cost of a move across devices", RAM disk → the runner's
  disk, 2,000 × 1 KiB and 32 MiB, printed in the log, never failing the run).
- **Deviations**: (1) a `moved` line per file, not one per batch: lines cost nothing
  unsynced and the next start can then name each pair; one journal sync per batch, as
  planned. (2) A copy whose **data** sync fails is removed and its source kept (as
  planned), and is counted failed, not done; nothing is asked, it is in the summary's
  errors. A **directory** sync that fails keeps both the source and the complete copy
  (removing a complete, data-synced copy would gain nothing). (3) A copy under its real
  name is not synced until its batch: a kill never leaves a short file under a real name
  with safe operations on (written under a temporary name), a power cut can, as for any
  new file (decision 6's contract), but never with its source already gone.
- Failing first (against the step-2 commit, with the `RemoveSource` point put in front
  of the old `remove_file` for the trace and the halt):
  - `a_move_across_devices_removes_a_source_only_once_its_copy_is_on_disk`: trace
    `… Write, Rename, JournalSync, RemoveSource`: no `SyncData`, no `SyncDir`.
  - `a_move_across_devices_whose_copy_cannot_be_synced_keeps_the_source`: `done: 1,
    failed: 0`, source removed.
  - pty `a_move_killed_before_removing_its_source_says_both_are_there` (SIGKILL while the
    seam holds the job at `RemoveSource`): the next start's dialog never says "copied
    completely" (no `moved` line existed).
- Also: `a_move_killed_before_removing_a_source_says_both_are_there` (tidy lists only the
  pairs still both there).

### 2026-11-13, step 4: B01, compare never plans what it could not read

- `compare.rs`: names are `OsString` end to end (`Diff::rel` a relative `PathBuf`, both
  sides sorted by the same `OsStr` order, hidden told by the first byte); the shown text
  is made where it is drawn. New `Side`, and `State::Unreadable(side)` (a `read_dir` that
  fails: the subtree is one row, nothing under it planned on either side),
  `Missing(side)` (gone while compared: `NotFound` on the directory), `Incomplete(side)`
  (an entry that could not be read or stated, `NotFound` excepted: the directory gets a
  row with the first error, and every name in it only on the *other* side becomes
  `Incomplete` instead of one-sided). `Diff::error` carries why; `State::unknown()`;
  `plan` never takes an unknown row. A root that cannot be read is an error:
  `compare` → `io::Result<Vec<Diff>>`, `compare_until` → `Option<io::Result<…>>`.
  `ReadDir` and `Stat` points in `entries`.
- App: Alt+K's status line adds "⚠ <dir> not read in full" (or a count) and marks
  nothing unknown; Alt+Y's dialog draws ⚠ rows ("cannot be read in the other (…) —
  nothing under it is copied", "went away …", "not all of it could be read …") and
  "? name  not copied: … could not be read in full here", and the title gains
  "· ⚠ N not read"; a plan with only such rows is shown, never "already agree". A root
  that cannot be read: "cannot compare: …" on the status line.
- Seam format changed: `XDEVCMD_FAULTS` fields are separated by `|`, not `:` (the
  render test passing a Windows path in a rule would have cut it at `C:`).
- Failing first (against the step-3 commit, `ReadDir`/`Stat` points put in front of the
  old calls):
  - `a_directory_that_cannot_be_read_is_never_planned_into`: the plan copied
    `l/sub/a` and `l/sub/b` into the unreadable `r/sub`.
  - `names_that_are_not_utf8_are_compared_and_planned_as_they_are`: `l/a\u{fffd}` is
    planned and does not exist.
  - `an_entry_that_cannot_be_looked_at_leaves_its_directory_unplanned`: `r/x` planned
    into `l`, over the left `x` that could not be stated.
- Also: `a_directory_without_read_permission_is_never_planned_into` (real `chmod 000`;
  skips as root, so here it skipped: runs on CI, which is not root),
  `a_root_that_cannot_be_read_is_an_error_not_a_plan`, and the render test
  `a_directory_that_cannot_be_read_is_shown_in_the_plan_not_taken_for_empty` (the seam
  through `XDEVCMD_FAULTS` in the real binary: row, title count, "cannot compare").

### 2026-11-13, step 5: B05, the file agreed to is the file replaced

- `ops::Ident` (kind, size, mtime; Unix `(dev, ino)`, elsewhere the creation time) and
  `ops::Expect { Absent, Replace(Ident) }`, both public.
- `resolve_conflict` takes the `Ident` of what it shows in the question and keeps it
  (`Worker::agreed`) on Yes; "Replace all" and Update stay rules, re-applied to whatever
  is there. `Worker::still_agreed` runs right before the replace: in `commit_replace`'s
  `ready` (before the journal's sync and the rename) and before `move_into`'s same-device
  `fs::rename`. A different file (or none) marks it `changed` and goes back to the
  question, now `Question::Conflict { changed: true, .. }`, drawn as "Already exists, and
  changed … It changed since you answered or since the plan was made: asked again."
  Kept, not consumed, so a move that falls back to copying checks it again at the copy.
  Archive extract and pack: a new `Hooks::recheck`; a changed target is kept and the
  entry reported failed ("it changed since you answered — kept as it is") rather than
  asked again (the archive walk has no loop back to a question; small, said here).
- Sync: `Request::sync(Vec<(src, into, Expect)>)` fills `Request::expect` (destination →
  expectation); `overwrite_confirmed` and the worker's `planned` (every destination
  that existed when the job started) are gone. A destination is replaced unasked only
  when it is still the `Ident` the compare saw; `Absent` and anything else there is
  asked, as changed. `compare::Diff` carries `left`/`right` idents; `plan` returns the
  triple; `Cmd::Sync` carries it.
- Case clashes (decision 8): a one-sided name is looked up on the other side (one
  `symlink_metadata`); found means the volume ignores case: `State::CaseClash`, shown
  "≠ name  same name, different case on the other side — left alone", never planned.
- Failing first (against the step-4 commit):
  - **the fault test** `a_file_replaced_after_the_user_agreed_is_asked_about_again` (the
    seam's `Swap` at the replacement's `SyncData`, after the Yes): one question only,
    the other program's file overwritten.
  - `a_move_onto_a_file_replaced_after_the_user_agreed_asks_again`: one question, the
    replaced file overwritten by the rename.
  - `a_file_made_where_the_plan_saw_none_is_asked_about` (run there with the old
    two-element `Request::sync`): asked 0 times, overwritten.
  - `names_that_differ_only_in_case_are_a_clash_where_case_is_ignored`: **not run
    here** (Linux, case-sensitive: it skips and checks the plain plan); it runs on the
    macOS and Windows runners, whose temporary directories ignore case.
- What cannot be closed: between the re-check and the rename another program can still
  replace the target (no portable compare-and-rename); microseconds instead of the whole
  copy. For the contract (pillar 3).

### 2026-11-13, step 6: B02, the cursor lands on the chosen file

- `FilePane::go_to_file(dir, name)`: moves, and puts the intent on the pending move as
  `Select::Name(OsString)` (the slot `Reading.select`, now `Option<Select>`, which
  `select_path` already filled with a path that was wrong for a slow move), resolved by
  name against the listing that arrives, so a link in the path, or macOS's
  `/var` → `/private/var`, still lands. A name not in that listing (hidden, filtered
  out, gone) leaves the cursor at the top with a notice "… is not shown here". A move
  that replaces the pending one, and Esc, drop the intent with the read (`navigate`
  clears `reading`; `stop_reading` takes it); the intent is applied after `arrive`, so
  it wins over "came up out of a directory".
- Call sites moved to it: Enter on a result (`activate`), a typed path to a file
  (`app/file_ops.rs`), a link's Go (`app/shell.rs`), both panes on one directory
  (`app/panes.rs`, now skipping `..` under the cursor). The fifth in §2 is `activate`
  itself; `select_path` stays for a path in the listing as it is (after a rename, a
  new file, leaving an archive).
- Failing first (against the step-5 commit): pty
  `enter_on_a_result_in_a_slow_directory_lands_on_the_file` (`XDEVCMD_TEST_SLOW_DIR`):
  status line ` ..  ·  3 items`: the cursor on `..`.
- Also: unit test `a_file_to_land_on_is_found_in_the_listing_that_arrives` (a read
  sent by hand: lands by name; a second move drops the intent; Esc drops it; a hidden
  name: top and a notice).

### 2026-11-13, step 7: B03, a whole line that exists is taken whole

- `Links::line_paths(i, shown)`: the line's wholes (as shown, then without trailing
  spaces, resolved like any path; none when the line holds control characters or a
  URL, or when the whole is one of the paths found) and the paths `found::paths_in`
  finds, as a `find::Group`. Panelize (`viewer.rs`) gathers them per line into
  `ListRequest { paths, groups, .. }`; `lists::List.groups` keeps them until the first
  check, which runs `find::check_grouped` on the list's own thread (never the screen's):
  each line's wholes first, in order: the first that is there is the row and the
  line's other candidates go as `Gone` (dropped quietly: never-checked rows); a path
  found in lines that were all taken whole goes too; everything else is checked as
  before. `lists::add` keeps only groups of rows it newly added (a row the list had
  stays whatever a new line says).
- Counting: a line's whole is a row to decide, not a path yet, so "Add N paths to…" and
  "N added, M already there" count the paths found (or, with none, the lines). Seen
  through the existing pty test `the_last_commands_output_makes_a_list_…`, which said
  "Add 2 paths" (the prompt line `$ ` became a whole) until counting was made so.
- Failing first (against the step-6 commit): pty
  `panelize_takes_a_whole_line_that_names_a_file` (lines `report:2026.txt`, `a b.txt`,
  `2026`, all files; `b.txt` there too): the list held only `b.txt`.
- Also: `find::tests::a_whole_line_that_exists_wins_over_the_paths_found_in_it` (a line
  not there whole gives today's rows; a path also named by a line not taken whole
  stays); `lists` tests updated.
- Not done: a list of many lines can reach the 10,000-row cap sooner, since wholes are
  rows until checked (said here; no test).

**CI between steps.** Run 43300172507 (at step 3): Linux x86/ARM failed on
`a_file_on_another_volume_goes_to_that_volumes_trash_and_back`, which hard-coded
`/dev/shm` and now got CI's `/mnt/xdev-other`: fixed in 62f634b (the volume's top is the
place's parent). macOS failed on `the_tab_menu_follows_its_tab_when_an_earlier_one_ends`
(a terminal-tab timing test, untouched here; passed on the next run). Run 43304134468
(at step 5): **all green** on the four systems. Its opt-in measurement step:

| Runner, RAM disk → runner's disk | synced (batched) | no sync | ratio |
|---|---|---|---|
| macOS (APFS, `F_FULLFSYNC`), 2,000 × 1 KiB | 2.111 s | 0.664 s | 3.2× |
| macOS, 1 × 32 MiB | 0.028 s | 0.017 s | 1.6× |
| ubuntu-latest, 2,000 × 1 KiB | 0.935 s | 0.311 s | 3.0× |
| ubuntu-latest, 1 × 32 MiB | 0.032 s | 0.020 s | 1.6× |

About 0.7 ms per small file on the macOS runner (a VM: its full flush may not reach a
real drive's cache; a real Mac is still to measure). The case-clash compare test passed
on macOS and Windows, whose temporary directories ignore case.

### 2026-11-13, step 8: docs

- `docs/security.md` "Safe operations": **corrected** the old claim that no file is ever
  left short under its real name after "a power cut" (true for a kill only: a new file
  is not synced, decision 6); new paragraphs "When the journal cannot be written",
  "What is synced, and when", "What survives a kill, and what survives a power cut",
  "Replaced only if it is still the file you agreed to", "Windows" (no directory sync,
  decision 7; FAT's check-then-rename), and limits for macOS's `F_FULLFSYNC` and
  compare's ⚠ rows. The full recovery contract stays pillar 3.
- `docs/guide.md` (Find, filter, compare): the ⚠ / ? / ≠ rows and the plan's re-check.
- `CHANGELOG.md` Unreleased: Added (durability, the journal question, re-check before
  replace, compare's unknown rows) and Fixed (B01–B05, the link replace, B02, B03).
- `.agents/memory/2026-11-13-fault-seam-for-tests.md`: how a data-safety test is written,
  with a promotion candidate for `reference/testing.md` (read-only for agents).
- No test needed: `every_toml_example_in_the_docs_is_a_valid_config` and the docs tests
  pass.

### 2026-11-13, end of the round: checks and CI

- Local (this sandbox, Linux x86_64, as root): `cargo fmt --all --check` clean; `cargo
  clippy --workspace --all-targets -- -D warnings` and with `--all-features` clean;
  `cargo test --workspace` twice in a row, all green both times (each run includes the
  pty suite: 108 passed, 1 ignored).
- CI run 43310782838 (at 792c335): green on ubuntu-latest, macOS, Windows, the
  cross-check and cargo-deny; **ARM failed** on the new pty test
  `enter_on_a_result_in_a_slow_directory_lands_on_the_file`: a race in the test, not
  the code (it waited for "zz-target", which the find dialog shows as typed, then
  pressed End before the result row had arrived; Enter went up out of the results).
  Fixed to wait for "1 found"; three local runs green.
- CI run **43312618853** (at bc6622f): green on all six jobs after one re-run of the
  failed macOS job: attempt 1 failed on `a_log_keeps_what_scrolled_past_the_scrollback`
  (Round 027's terminal-log test, "Log to file" not on screen in time; not touched by
  this round, a macOS pty timing flake like the tab-menu one at 43300172507); attempt 2
  green. The release-build seam check ran on every system (no "no release build" line).
  Its measurement step again: macOS 2,000 × 1 KiB 1.817 s vs 0.552 s (3.3×); ubuntu
  0.692 s vs 0.238 s (2.9×).
- **Unverified**: the cost on the owner's USB stick and on a real Mac (decision 4 asks
  for both; the runners are VMs); a power cut itself (only the order is tested, from the
  seam's trace); the rename-many "no journal" dialog end to end (no test drives
  Shift+F2's editor); the Windows half of the kill test (`windows_e2e.rs` has no kill
  test yet); the real `chmod 000` compare test here (root; it runs on CI).

### 2026-11-13, review and QA fixes (the coordinator's list)

Each failing test run before its fix; the non-root ones as `nobody` through
`setpriv --reuid=65534 --regid=65534 --clear-groups <test binary>` (this sandbox is
root; CI is not).

- **1 (blocker) A move's batch synced each copy through a handle opened afresh.** A
  read-only copy (its mode is copied before the sync) then failed with EACCES for any
  normal user, failing the move (Windows: the copy left), and on NFS/SMB a write error
  told only to the handle that wrote could be missed and the source removed. Now
  `Worker::removing` holds `(src, dst, Option<File>)`: the handle `copy_bytes` wrote with
  (`Worker::moved_file`, at most 64 at once), synced through; `None` for a replacement
  (synced before its rename) and a link. `remove_file_anyway` clears Windows' read-only
  attribute before removing a read-only source or a failed copy. Failing first, as
  `nobody`: `a_read_only_file_moves_across_devices`: `done: 0, failed: 1 … Permission
  denied (os error 13)`; it skips as root with a line saying so. Also
  `a_copy_whose_sync_fails_keeps_its_source_and_the_rest_moves` (3 files, the second's
  sync fails at its own handle: its source kept, its copy removed, the others moved;
  passed before too, through the old path: a guard, not a regression test). The whole
  core suite passes as root and as `nobody` (the `chmod 000` compare test ran there).
- **2 The re-check ran before the journal step.** `durable` can wait minutes on the
  `NoJournal` question; the rename then ran on a look taken before it. Now `durable`
  first and `still_agreed` / `Hooks::recheck` last, right before `Rename`, in
  `copy_bytes`, `move_into`, archive `write_entry` and `pack` (§4's order). Failing
  first: `a_file_replaced_while_the_journal_question_is_open_is_asked_about_again`:
  asked `Conflict`, `NoJournal` and nothing more; the file replaced while the question
  was open was overwritten.

## Check

## Act
