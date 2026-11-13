# Round 031: Smart Planner v1, around the executor (v0.4.0-beta.1, part 3 of 4, item 42)

**Status**: Do (started 2026-11-13; the owner: "Go as recommended")
**Part of**: v0.4.0, step 6 of `ROADMAP.md`, beta.1; item 42; the owner's decisions of
2026-11-13 (undo for a copy, replace to trash on by default, network pause and ETA
pulled from v2, kept lists follow a move)
**Date started**: 2026-11-13
**Date completed**: YYYY-MM-DD

## Goal

What a person feels after the plan has been confirmed: a copy can be undone; a file
replaced on a volume that has a trash comes back with Ctrl+Z; a network error pauses the
job and asks instead of failing everything left; the job says when it will end, from
what it has really done; and a list kept in the app follows the files a move took
elsewhere instead of reporting them gone.

## Kickoff review (2026-11-13, Round 030 at Review)

### 1. What changed since the outline

- Round 030 is done and at Review (not yet closed). What it leaves for this round:
  `plan::Plan` with rows, groups and `Target { fs, free, writable, ignores_case }`;
  `Expect::Skip`; one walk per job (`Request::planned`); journal `v 3` with `decided`
  and `done` lines; `Ident::encode`/`decode`. Its deviations (Alt+Y and Shift+F5 not on
  `Plan`, Resume not re-planning) do not block anything here.
- If the owner's hand tests of 030 find something, the fix goes in 030 first; this
  round's steps 4 and 5 (ETA, lists) touch nothing 030 changed and can go first if so.

### 2. The code this round touches

| Where | What is there today | What this round does |
|---|---|---|
| `core/undo.rs` `Undo` (16), `Plan` (37) | Rename, Move, MadeDir, MadeFile, Trashed, `Cannot`; `remove_empty_tree` | `Undo::Copy { made, replaced }`, decided against the disk as the others are |
| `app/jobs.rs` `remember_job` (131), `undo` (454) | a copy is `Cannot("a copy")`; a move is remembered from the request | a copy remembered from what the job **reports it made**, at its end (like the trash's `trash_pending`) |
| `core/ops.rs` `copy_bytes` (1159), `commit_replace` (1594) | new file: temp name, `rename_noreplace`, not synced (owner's rule: new files stay fast); replace: temp, `sync_data`, rename over | replace: temp, sync, **old one to the trash**, rename; the made/replaced list into `Summary` |
| `core/ops.rs` `send_to_trash`, `volume_trash`, `trash_into` (1700–1880) | F8's trash on each system | reused for the replaced file |
| `core/ops.rs` `copy_file` loop (1098) | an error → `Question::Failed` → Retry / Skip / Abort | transient errors retried with backoff first, then the same question, saying what kind of error |
| `core/plan.rs` `Fs`, `fs_kind`, `short_of_space` (348) | FAT / exFAT / NTFS / Other / Unknown; "needs" counts a replace as written | + `Network`; whether a replace can go to the trash; a replace into the trash frees nothing |
| `core/ops.rs` `Event::Progress { bytes_done }`, `PROGRESS_STEP` = 1 MiB | sent per MiB written | also files done, and at least every 250 ms, so small files move the ETA |
| `app` job box (`modal.rs` 455, `draw.rs` 405) | bytes done of total | an ETA after ~3 s |
| `xdevcmd/src/lists.rs` `checked` (155) | rows found gone are dropped | `relink(from, to)` for what a move, a rename or an undo really moved |

### 3. Outside check

- **Undo of a copy**: Explorer's Ctrl+Z after a copy deletes the copies (to the Recycle
  Bin, asking when they were changed); Finder removes them. Neither checks the copy is
  still what it made beyond "changed since"; the outline's `Ident` rule is the same idea,
  stricter. Explorer's undo of a copy is a job with progress: ours should be too.
- **Replace into the trash**: macOS Finder's "Replace" moves the old item to the Trash;
  Explorer's replace deletes for good. So the macOS model is the precedent; it also
  means the trash grows by every replaced file, which a sync of thousands will show.
- **Network errors**: robocopy retries (`/R:n /W:s`, default a million tries 30 s apart,
  famously too many); rsync gives up and leaves resume to a rerun; FreeFileSync retries a
  set number of times then asks with "Ignore / Ignore all / Retry". FreeFileSync's
  shape is the outline's: a few automatic retries, then the question.
- **ETA**: Explorer's estimate is notorious for swinging; the usual fix is a moving
  average over a window of seconds plus files per second for small files (outline), and
  not showing it until it has settled.

### 4. Critique of the outline

1. **What a copy made comes from the job, not the plan.** The plan is a moment; the job
   may be cancelled, skip a row, or meet a file that appeared. The Worker reports each
   thing it created (`Made { path, ident, dir }`) and each file it replaced (`Replaced {
   path, ident_of_new, trashed }`) in `Summary`, capped at the plan's `CAP` (200,000):
   past it the copy is `Cannot("a copy of more than 200,000 items")`. The `Ident` is
   taken after the rename, from the file under its real name.
2. **Undo of a copy is a job**, as undoing a move already is: thousands of removals,
   cancellable, with a summary of what was kept because it changed. Directories go only
   when empty after their own files went (`remove_empty_tree`'s rule). A replaced file
   comes back from the trash only after its replacement is removed, and only while that
   is still the copy's (its `Ident`); otherwise both are left and listed.
3. **Replacing into the trash opens a gap** where today's rename is atomic: between the
   old file leaving and the new one being renamed in, nothing is at `dst`. Order: new
   file synced; journal line `trash <dst> <where it went>`, synced; old file to the
   trash; rename. A crash in the gap: the journal finds the complete, synced temp file
   and finishes the rename (forward, since the new one is what was confirmed), and says
   where the old one is. One order on every system; Linux could avoid the gap with a
   hard link into the trash, but FAT has no links and two paths double the tests — not
   in v1.
4. **The free-space figure changes**: a file replaced into the trash on the same volume
   frees nothing, so "needs" counts it whole (today it is already counted whole — the
   upper bound in 030's Check becomes exact for those rows).
5. **"Same volume trash" per system** (the outline's risk), decided in the plan, ⚠️ when
   unknown:
   - Linux: the home trash on the same `st_dev`, or `volume_trash`'s top directory
     usable → ℹ️. Network mounts too (a `.Trash-$uid` on an NFS share is the spec's).
   - macOS: local volumes (APFS, HFS+, msdos, exfat: Finder keeps `.Trashes` on them) →
     ℹ️; `smbfs`, `nfs`, `afpfs`, `webdav` → ⚠️. If `trashItem` fails anyway, the job
     asks (today's `TrashUnavailable` question), never replaces silently.
   - Windows: **no Recycle Bin on removable and network drives**, and whether the shell
     then deletes for good or asks is not something this code can see (no FFI,
     `forbid(unsafe_code)` in core). Recommend: ℹ️ only on the system drive
     (`%SystemDrive%`), ⚠️ elsewhere in beta.1 (decision 1). **The same question applies
     to F8 today**: F8 on a USB stick on Windows should be tried by hand.
6. **Network errors already stop and ask** (`Question::Failed`, Retry/Skip/Abort), and
   the job waits while a question is open. So "pause" is the open question itself; no
   separate Pause button. What is new: automatic retries with backoff for transient
   errors before asking, the question saying "network error" and how many tries were
   made, and Esc ending a backoff at once (sleep in 100 ms slices on the cancel flag).
   A retry starts the file again from zero (resuming inside a file is v2).
7. **Which errors are transient**: by error code, wherever the target is —
   `ETIMEDOUT`, `ENOTCONN`, `EHOSTDOWN`, `EHOSTUNREACH`, `ECONNRESET`, `ESTALE`;
   Windows 53, 59, 64, 67, 121, 1231. `EIO` only on a network target (on a local disk it
   is a failing disk; retrying it is wrong).
8. **Network targets on Linux need `/proc/self/mountinfo`**: every FUSE mount reports
   the same `f_type` (0x65735546), whether `sshfs`, `rclone` or a local `ntfs-3g`. The
   mount's type string (`fuse.sshfs`, `fuse.rclone`, `nfs4`, `cifs`, `smb3`) is read from
   mountinfo for the target's mount (longest matching mount point). NFS (0x6969) and
   SMB/CIFS (0xFE534D42, 0xFF534D42) are known from `f_type` alone. macOS:
   `f_fstypename`. Windows: a UNC path (`\\server\share`, `\\?\UNC\`); a mapped
   drive letter stays Unknown.
9. **Sync new files on a network target**: the owner's rule is "new files stay fast";
   on a network target the price is one flush per file, and it is where write errors
   are otherwise lost (SMB and NFS report them at `fsync`/close). Recommend: yes, only
   for a target the plan found to be network (decision 3); measured in beta.1.
10. **ETA needs events for small files**: today `Progress` comes once per MiB, so 10,000
    small files send nothing. Add `files_done` and send at least every 250 ms. The rate
    is a pure struct in the UI (`Rate::observe(at, bytes, files)`), tested with given
    instants, no clock seam needed. Shown after 3 s and only when it has been within
    ±25 % for the last two seconds; "about 4 min", rounded, never seconds past a minute.
11. **Kept lists follow what really moved**, not what was asked: after a move job, for
    each source that is gone with its destination there, `relink(from, to)` on every
    list (the row and any row under it). Renames (F2, Shift+F6) and undoing a move call
    the same function, so a list follows both ways. Cheap: lists are in memory.

### 5. Steps (each a commit; every behaviour first as a failing test)

1. **The job reports what it made** — `Summary.made`, `Summary.replaced` (cap),
   `Ident` after the rename; unit tests: new files, new directories, merged directories
   (not "made"), a skipped row (absent), a cancelled job (only what finished). **S.**
2. **Undo for a copy** — `Undo::Copy`; `remember_job` keeps it at the job's end; undo
   as a job; keeps and lists anything changed (`Ident`), a directory that holds
   something else, a file that was replaced (until step 3). Tests in `undo.rs` on a
   temp tree, a pty test: F5, Ctrl+Z, the copy gone, a changed copy kept. **M.**
3. **Replace into the trash** — `Target::trash` in the plan (critique 5), ℹ️/⚠️ group
   levels follow it; `commit_replace` takes the trash step in the order of critique 3;
   journal `trash` line and its recovery; Undo restores. Order test from the fault
   trace; kill tests with `Halt` at the trash and at the rename; tests use a temp trash
   root as F8's do (never the real trash). **M.**
4. **Network: detection, backoff, the question** — `Fs::Network`, mountinfo on Linux;
   transient errors (critique 7); backoff 1, 2, 4, 8, 15 s (decision 2), the delays a
   field of `Request` so tests run with zeros; the job box line "network error — trying
   again in 4 s (3 of 5)"; Esc during backoff; sync before the rename on a network
   target (decision 3). Tests through the seam's `fail` with a raw OS error. **M.**
5. **ETA** — `files_done` in `Progress`, the 250 ms floor; `Rate` with tests (steady,
   slowdown, a burst of small files, a stall); the job box. **S.**
6. **Kept lists follow a move** — `lists::relink`; after a move, a rename, an undo.
   Unit tests, one render test (Alt+L after F6). **S.**
7. **Docs and CHANGELOG** — guide: undo of a copy, replace into the trash and where it
   is not, what a network error does, the ETA; security.md: the trash step and its
   recovery. **S.**

About a week and a half, as the outline said; step 3 is the one that may run over.

### 6. Decisions for the owner

1. **Windows replace into the trash**: ℹ️ only on the system drive, ⚠️ (skip by default)
   on every other drive in beta.1 (recommended, critique 5), or try the Recycle Bin on
   every local drive and accept that a removable drive may delete for good.
2. **Backoff**: 5 tries, 1 · 2 · 4 · 8 · 15 s (about 30 s), then the question
   (recommended), or fewer/longer.
3. **Sync new files on a network target**: yes, network targets only (recommended,
   critique 9), or keep "new files stay fast" everywhere.
4. **Undo of a copy**: one step until the next operation (as every undo here), capped at
   200,000 items, run as a job (recommended, critiques 1–2).
5. **Alt+Y's replaces go to the trash too** (they go through the same `commit_replace`;
   recommended, since a sync's replaces are the ones most likely to be regretted), and
   Alt+Y becomes undoable by the same `Undo::Copy` — or Alt+Y stays as it is this round.

### 7. Risks

- The gap in critique 3 is new: before this round a replace could never leave `dst`
  empty. The kill tests at both points and the journal's forward recovery are the
  guard; a hand test kills a real copy mid-way.
- A trash filling with every replaced file of a big sync: the job's summary says how
  many went to the trash and how much space they hold.
- Backoff on a dead mount can hang in the system call itself (a hard NFS mount blocks
  forever, not an error); retries do not help there, and Esc can only stop the job once
  the call returns. Said in the guide; not solvable without a second thread per call.
- Undo removing files is a delete: it must never remove what it did not make. The
  `Ident` check and "a directory only when empty" are the guard; a test edits a copied
  file and another drops a new file into a copied directory before undoing.

## Plan (outline)

### Depends on Round 030

- The plan's rows say what was **created** and what was **replaced**, and on which
  device: undo and the trash read them.
- ℹ️ vs ⚠️ for a replace depends on whether the trash is on the same volume: the confirm
  table shows the answer this round computes.
- The executor's per-row loop is where pause, retry and backoff go.

### Steps (each a commit with its tests)

1. **Undo for a copy**: `Undo::Copy { created: Vec<(PathBuf, Ident)> }`; Ctrl+Z removes
   what the copy made, **only while each is still what it made** (its `Ident`), and
   directories only while they hold nothing else; anything changed is listed and kept.
   `remember_job` today says "a copy cannot be undone". **M.**
2. **Replace into the trash, on by default where the trash is on the same volume**: the
   old file goes to the trash (Linux: our own `volume_trash`/`trash_into`; macOS:
   `NSFileManager trashItem` already used; Windows: the Recycle Bin), then the new one is
   renamed in; Ctrl+Z puts the old one back. No trash, or one on another volume: a real
   replace, flagged ⚠️ in the plan. Order (Round 029's rule): the new file synced first,
   then the old one trashed, then the rename; a crash in between leaves the old file in
   the trash and the new one complete under its temporary name, both found by the
   journal. **M.**
3. **A network error pauses and asks**: retry / skip / pause, with backoff (1, 2, 4, 8 …
   up to 30 s, a few tries) for errors that pass (`ETIMEDOUT`, `ESTALE`, `ENOTCONN`, EIO
   on a network mount; Windows 59, 64, 121) before asking; a target found to be on a
   network mount (`statfs` type: nfs, cifs/smb2, fuse.sshfs, fuse.rclone; a UNC path)
   also syncs new files, since SMB and NFS report write errors only at `fsync` or
   close (ROADMAP note). Tests through the seam's `Fail` and `Delay`, a fake clock. **M.**
4. **ETA**: from the throughput measured on the real work (an exponentially weighted
   moving average over a few seconds of bytes, plus files per second for small files),
   shown after ~3 s, no separate probe. Unit tests with a fake clock: steady, a slowdown,
   a burst of small files. **S.**
5. **Kept lists follow a move** (`lists.rs` `relink(from, to)`; called when a move job
   ends, per moved source and its children); a row that was moved is relinked, not
   reported as gone. **S.**
6. Docs (guide: undo, trash, pause, ETA), CHANGELOG. **S.**

About a week and a half.

### Risks

- **"Same volume" for the trash**: Linux knows (`st_dev`, `volume_trash`); macOS and
  Windows decide in the OS, and a network volume may delete at once. Where it cannot be
  known, ⚠️ (data lost for good), never ℹ️.
- **Backoff and cancel**: a job sleeping in backoff must still stop at Esc at once.
- **Undo after the disk moved on**: the `Ident` check is the guard; a test edits a copied
  file and undo keeps it.
- **ETA honesty**: a mount that caches writes (rclone) can finish "early" and upload
  later; the ETA and the summary say "written", not "uploaded" (ROADMAP note).

### Open decisions (recommendations at kickoff)

1. **Replace to trash with no same-volume trash**: real replace flagged ⚠️ with "skip
   those" the default (the owner's rule) — confirmed, or ask per job?
2. **Backoff's limits** (how many tries, the longest wait) before the question.
3. **Undo for a copy kept how long**: like every undo here, one step, until the next
   operation.
4. **Sync new files on a network target** (above): leaning yes, it is where silent
   write errors happen.

### Not doing

- Adaptive concurrency, lanes, group commit (v2). Uploading progress of cached mounts.

## Do

### 2026-11-13, the owner's decisions

"Go as recommended": all five as the kickoff recommends.

1. Windows: a replace goes to the Recycle Bin (ℹ️) only on the system drive; ⚠️ (skip by
   default) on every other drive in beta.1.
2. Backoff: 5 tries, 1 · 2 · 4 · 8 · 15 s, then the question.
3. New files synced before their rename on a network target only.
4. Undo of a copy: one step, until the next operation; capped at 200,000 items; a job.
5. Alt+Y's replaces go to the trash too, and Alt+Y is undone the same way.

### 2026-11-13, step 1–2: what a copy made, and its undo (f47bc76)

- `ops::Made { path, ident, old }`, `Summary::{made, made_over, replaced_for_good}`,
  `MADE_CAP` = `plan::CAP`; recorded only for `Kind::Copy` (F5 and Alt+Y), in the order
  made, the `Ident` taken after the rename. Merged-into directories, skipped rows and a
  move record nothing.
- `Undo::Copy { made, what, for_good }` → `Plan::Unmake { made, kept }`; `undo::unmake`
  is a job (`Handle::spawn`, no questions): last made first, a file only while its
  `Ident` is the same, a directory only through `remove_dir`. Kept items are listed in
  the summary with why.
- App: `expect_copy_undo` / `note_copied` (the trash's pending pattern), `Job::title`
  ("Undo the copy"); Alt+Y no longer says "a synchronisation cannot be undone".
- Tests: core order/what-is-made, undo removes / keeps changed / nothing left; render:
  F5 → Ctrl+Z → Enter removes the copy; "Undo the copy of x.txt?"; a permanent delete
  still says it cannot be undone (replaces the old "a copy cannot be undone" check).

### 2026-11-13, step 3: replace into the trash (3a1338a)

- `plan::Target::trash` from `plan::trash_keeps(dir)`: Linux `ops::volume_trash_usable`
  (reads only; the home trash on the same device, or the mount's `.Trash/$uid` /
  `.Trash-$uid` usable or makeable); macOS by `f_fstypename` (apfs, hfs, msdos, exfat);
  Windows `on_system_drive` (decision 1).
- Executor: in `copy_bytes`' replace, inside `commit_replace`'s `ready`: journal `swap
  <tmp> <dst>`, `durable`, `still_agreed`, then `faults::at(Point::Trash)` +
  `send_to_trash`; a failed rename restores the old file. `Summary::replaced_to_trash`;
  `Made::old` for the undo. New fault point `trash`.
- Journal recovery: a `swap` whose `dst` is empty and whose temp file is ours and
  there is renamed in (`rename_noreplace`) before the part files are tidied;
  `Interrupted::swapped`, said at start ("N replacement(s) finished …").
- Table: `Plan::level(g)` shows Newer/Older/Differs as ℹ when the trash keeps them; a
  head line "ℹ a file replaced goes to the trash[: Ctrl+Z brings it back]". **Still
  asked, skip first**: the outline's "ℹ in the summary only" would have replaced files
  without a question for the first time since 0.1; kept as a question (deviation).
- Test isolation: unit tests never trash without `trash_root`; the test binary
  (`feature = "faults"`) trashes only on Linux, where XDG_DATA_HOME is the sandbox — on
  macOS/Windows CI it would have filled the runner's real Trash.
- `Cmd::Resume` boxed (clippy `large_enum_variant` after `Interrupted` grew).
- Tests: order from the trace (sync-data < journal-sync < trash < rename); rename
  failing after the trash puts the old file back; journal tidy finishes a swap and
  leaves one whose old file is still there; Windows drive rule; render: replace → trash
  → Ctrl+Z brings it back (Linux); pty: halt at the rename after the trash, kill -9,
  restart says "1 replacement finished" and the new file is in place — **shown failing
  with the journal's `swap` recovery taken out** (timed out waiting for the message).

### 2026-11-13, step 4: network (2752e52)

- `Fs::Network`: Linux `f_type` NFS/SMB2/CIFS/SMB1/Ceph/AFS, and for FUSE the mount
  table's type (`mount_type_in`, octal escapes, longest mount point) — sshfs, rclone,
  s3fs, gcsfuse …; macOS smbfs/nfs/afpfs/webdav/cifs/ftp; Windows UNC only.
- `ops::transient(e, network)`: TIMEDOUT, NOTCONN, HOSTDOWN, HOSTUNREACH, CONNRESET,
  CONNABORTED, NETDOWN, NETUNREACH, NETRESET, STALE; EIO only on a network target;
  Windows 53, 59, 64, 67, 121, 1229, 1231, 1236.
- `Request::backoff` (`BACKOFF_SECS` 1·2·4·8·15), `Event::Retrying`, `Worker::wait` in
  50 ms steps on the cancel flag; after the last try the question says "(a network
  error: tried 6 times)". The job box's third line counts down.
- A new file on a network target is synced before its rename (decision 3).
- Tests: retried then done without a question; asked after the tries; EIO on a local
  disk asked at once; Esc during a 30 s wait ends in < 5 s; mount-table and UNC
  parsing; the job box's line.

### 2026-11-13, step 5: ETA (7bc9e24)

- `Event::Progress { bytes_done, files_done }`, sent per MiB and at least every 250 ms
  while files finish. `xdevcmd/src/eta.rs`: work = bytes + 64 KiB per file, an EWMA
  (τ 5 s); shown after 3 s, when the last 2 s of estimates agree within 25 %, and not
  after 3 s without progress; "about 45 s", "about 4 min", "about 1 h 20 min".
- Tests with given instants: steady, slowdown (hidden while it falls, then longer), a
  burst of small files, a stall, rounding.

### 2026-11-13, step 6: kept lists follow a move (4923723)

- `Summary::relocated` (each top-level source of a move that went, and where);
  `lists::relink`; the app's `follow_moves` / `follow_renames` after a move job (F6,
  undo of a move), F2, Shift+F2 and its undo. A results tab showing a moved row turns
  into its kept list first, then is shown again relinked.
- Found by the render test: `to.join("")` added a trailing slash ("main.rs/"), which
  is no file, so the row was reported gone; the unit test had passed because `PathBuf`
  equality ignores it.

### 2026-11-13, step 7: docs and CHANGELOG

- `docs/guide.md` (network retries and sync, undo of a copy, replace into the trash,
  the ETA, lists that follow), `docs/security.md` (the trash step, its recovery, what
  undo removes), CHANGELOG Unreleased → Added (five entries).

## Check

- [x] Step 1–2: a copy reports what it made; Ctrl+Z removes what is still as made,
  keeps and lists the rest; Alt+Y the same
- [x] Step 3: replace into the trash per volume; journal `swap` and its recovery; the
  table's ℹ; undo brings the old file back; the kill test fails without the recovery
- [x] Step 4: network detection, retries with backoff, Esc during a wait, the
  question after the tries, new files synced on a network target
- [x] Step 5: the ETA (progress per file and per 250 ms; settled, never during a stall)
- [x] Step 6: kept lists and results tabs follow moves and renames
- [x] Step 7: guide, security, CHANGELOG
- [x] fmt, clippy ×3 (Linux, `x86_64-pc-windows-gnu`, `x86_64-apple-darwin`),
  `cargo test --workspace --no-fail-fast` locally (core 297, app 253, render 212, pty
  112)
- [ ] CI on the branch
- [ ] **Not verified**: a real network mount (an SMB or NFS share pulled mid-copy, an
  sshfs that drops; the retries are tested through the fault seam only, and the
  mount-table parse on a sample); the replace into the trash on macOS and Windows (the
  test binary does not trash there, by design); a Windows USB stick and F8 there (the
  Recycle Bin question of the kickoff, critique 5); the ETA by eye on a real slow copy;
  undo of a very large copy (only the cap's arithmetic, not 200,000 files)
- [ ] **Deviations for the owner**: a replace into the trash is ℹ but still asked, skip
  first (step 3); no setting to turn replace-into-trash off; the network sync covers
  copies, not a move's files (synced with their batch already); errors while making a
  directory or walking the source are not retried, only a file's copy

## Act
