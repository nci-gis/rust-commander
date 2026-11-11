# Round 022: Stable quality, first part (v0.4 entry Q)

**Status**: In Progress (the owner's go, 2026-11-11: "làm theo đề xuất của bro. Go")
**Part of**: v0.4.0, entry Q of `.agents/memory/2026-11-11-v0.4.0-scope-proposal.md`
(§3, the bar the release must clear)
**Date started**: 2026-11-11
**Date completed**: YYYY-MM-DD

## Goal

Make 0.4.0 stable before adding anything (the owner chose Q before L and D). Three
parts:

- **A slow directory never freezes the screen.** A dead network mount, a sleeping disk
  or a huge directory are read off the UI thread. A bar under the pane shows the
  progress, and Esc stops it. This is also what remote by mount needs (the owner: remote
  later, "by trusted tools, a network mount").
- **The budgets are kept by checks**, not by memory: the binary at most 7 MB, and the
  listing of a 100,000-entry directory.
- **The rest of §3**, in the order below, each a step the owner can stop at.

## Plan (kickoff review)

### 1. What changed since the scope proposal

- Rounds 020 and 021 shipped. Round 021 left two things for Q:
  - its bar (a list being checked shows a count in the title until Q's bar exists);
  - a flake seen twice under the full pty suite's load.
- The owner's ideas since then:
  - a loading/progress bar right under the pane;
  - remote later through mounts;
  - the 7 MB binary budget.

### 2. Map: what blocks the UI thread today

Found by reading `pane/files.rs` (2026-11-11):

- **`FilePane::navigate`**:
  - `paths::canonical(path)` (a `realpath`), then `reload()`, which calls `read()` →
    `listing::read_dir`.
  - `read_dir` stats every entry (`symlink_metadata`). One `stat` on a dead mount can
    block for minutes (NFS hard mounts, SMB with the server gone, sshfs without
    `-o reconnect`).
- **`reload()`** runs after every move, on Refresh, on a watcher event, after a job,
  and on a sort change. It calls `dir_mtime`, which is one more stat.
- **`check_now()`** (the periodic check, `[files] auto_refresh_ms`): `dir_mtime`, then
  `listing_changed()`, which **reads the whole directory again** (up to 2,000 entries)
  every check. If anything changed it then reads it a third time in `reload`.
- **`go_back` / `go_forward`**: `is_dir()` on each place.
- **Already off the thread:**
  - archives over 8 MB (`Opening`, Esc stops; the model to follow);
  - Alt+F7 and lists (`find::Search`);
  - directory sizes (`Sizer`), git status, every copy, move and delete job.
- **Results tabs re-check their rows** on a refresh (`read()`, one stat per row). That
  is bounded by the rows, and a list's own check is on a thread since Round 021. Left
  as it is.

### 3. Outside check

Not searched this time. The pattern is the common one: yazi and ranger read
directories on a worker and draw what has arrived; Explorer and Finder show a spinner
in the place of the listing. The archive `Opening` here already does it for archives.

### 4. Critique (what the step must get right)

- **Fast directories must not change at all.** Not a frame of "reading…" for a local
  directory, and every render test, which expects the listing at once, must still
  pass. So the read starts on a thread and is **waited for briefly (150 ms)**. Within
  that it is used as today. Only past it does the pane show the bar and carry on
  without it.
- **Moving into a slow directory:**
  - the pane stays on the old listing, with the bar naming the new one;
  - when the listing arrives, the move completes, as an ordinary move would;
  - Esc stops it, and the pane stays where it was;
  - another move replaces the pending one.

  This is what a user expects from Explorer and Finder: nothing half-entered.
- **A thread blocked in the kernel cannot be killed.** Esc only stops waiting for it.
  The thread ends when the mount answers or times out, and its result is dropped. Only
  one is pending per tab, so a hung mount cannot pile them up.
- **The periodic check does one read, not three.** `mtime`, the entries and the
  comparison happen together on the thread. While a read is pending, checks are
  skipped.
- **Back and Forward** no longer `stat` each place on the UI thread. The move itself
  finds out (a gone directory is an error that arrives, and Back tries the next one).
- **The bar** goes on the pane's bottom border row:
  - a moving block where the total is not known (a directory), a filling one where it
    is (a list being checked);
  - text: "reading /mnt/nas/photos… 12,400 · 3 s · Esc stops";
  - it appears only after the 150 ms, so no flicker;
  - Alt+F7's "searching… N looked at" and a list's "checking… N of M" move there too;
  - plain characters at 16 colours; the theme's accent and glow where `[style]` has
    them.
- **What this does not fix:** a file operation on a dead mount (copy, delete) is already
  a job on a thread with Cancel. Opening a file in the viewer (F3) still reads on the UI
  thread. That is one file; it is noted here and left for later.

### 5. Steps

1. **Directories off the UI thread**:
   - `listing::read_dir_counted` (core): counts entries as it goes, for the bar;
   - `FilePane`:
     - a `Reading` (one per tab): what it is for (a move, a reload, a check), its
       channel, the count and the start;
     - `navigate`, `reload` and `check_now` go through it, with the 150 ms wait;
     - `poll_reading` in the run loop, `stop_reading` on Esc;
   - Back and Forward without the `is_dir` stat.

   Tests:
   - unit: a read that answers late (a test hook that delays the thread) leaves the
     old listing up, then moves; Esc stops it; a second move replaces the first;
   - the full render suite unchanged (fast directories).

   M.
2. **The bar** (`pane/files.rs` render, `decor` aware): reading, list checking, Alt+F7
   searching. Render test through the same delay hook. S.
3. **Budgets as checks**:
   - a test that fails if `target/release/xdevcmd` is over 7 MB, when it exists (run
     at release and in CI later);
   - an ignored-by-default benchmark test: 100,000 entries listed and sorted in under
     1 s, its time printed (run by hand and at the release).

   S.
4. **The flake**: run the pty suite in a loop under load, catch a failing screen, and
   fix it or explain it. S, or written down if it does not show.
5. **Docs**:
   - docs/guide.md: what a slow directory looks like, and Esc;
   - a new section "Remote files through a mount" (sshfs / SSHFS-Win, rclone mount,
     SMB, NFS), saying xdevcmd opens no connection itself;
   - CHANGELOG.

   S.

Later in Q (rounds of their own, each with its own kickoff):
- the compatibility promise ("From 0.4.0"; the owner decides what it covers);
- fuzzing the untrusted parsers (cargo-fuzz needs nightly: run by hand, CI later);
- Unicode name sorting (item 21);
- the hand-test matrix as a checklist;
- the README for a first-time reader;
- CI on at the pull request into `dev`.

### 6. The owner's decisions

Settled: Q before L and D ("làm theo đề xuất của bro").

Open, with a recommendation; step 1 starts on these unless the owner says otherwise:
1. **The wait before the bar**: 150 ms (recommended; under it a read looks instant),
   or another.
2. **Moving into a slow directory**: stay on the old listing until the new one arrives
   (recommended), or switch at once to an empty pane with the bar.
3. **F3 on a file on a slow mount** stays on the UI thread for now (noted), or joins
   this round (about a day more).

## Do

(Not started.)

## Check

- [ ] A late read: old listing stays, the bar shows, it moves on arrival; Esc stops; a
  second move replaces it (unit)
- [ ] The full render and pty suites unchanged
- [ ] The bar: reading, list checking, Alt+F7 (render)
- [ ] Binary-size test; the 100,000-entry benchmark run and its time recorded
- [ ] The flake: caught and fixed, or written down
- [ ] The owner: a real slow mount (sshfs with the server stopped, or an SMB share with
  the network off) on Linux and Windows; Esc there
