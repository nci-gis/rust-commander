# Round 031: Smart Planner v1, around the executor (v0.4.0-beta.1, part 3 of 4, item 42)

**Status**: Planning — outline; the full kickoff review is written when Round 030 is
Complete, and the owner says go then
**Part of**: v0.4.0, step 6 of `ROADMAP.md`, beta.1; item 42; the owner's decisions of
2026-11-13 (undo for a copy, replace to trash on by default, network pause and ETA
pulled from v2, kept lists follow a move)
**Date started**: YYYY-MM-DD
**Date completed**: YYYY-MM-DD

## Goal

What a person feels after the plan has been confirmed: a copy can be undone; a file
replaced on a volume that has a trash comes back with Ctrl+Z; a network error pauses the
job and asks instead of failing everything left; the job says when it will end, from
what it has really done; and a list kept in the app follows the files a move took
elsewhere instead of reporting them gone.

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

## Check

## Act
