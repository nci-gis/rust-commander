# Round 022: Stable quality, first part (v0.4 entry Q)

**Status**: Review (set aside for the owner's hand tests, 2026-11-12)
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

The owner (2026-11-11): Q before L and D, "Go". The three open decisions are taken on
the recommendations: 150 ms; the old listing stays until the new one arrives; F3 on a
slow mount is left for later.

- **Step 1, directories off the UI thread** (`pane/files.rs`):
  - `read_on_thread` resolves the path, takes its time, and reads its entries with
    `xdevcmd_core::read_dir_counted` (new; it counts as it goes).
  - `read_bounded` waits up to `SLOW` (150 ms). Past that it leaves a `Reading` (what
    it is for, the channel, the count, the start, and a cursor target).
  - `navigate` is now a read plus `arrive`, the old body: policy, back list, new
    listing. A move replaces one on its way. `select_path` during a pending move is
    kept for when it arrives (Enter on a search result, Go to).
  - `reload` reads directories the same way; results and archives are read as before.
    A reload while a read is on its way is skipped.
  - `check_now` makes **one** read: the time, and the entries for a directory of up to
    2,000. Before, it was a stat, a full read to compare, and a third read to show the
    change. `compare` shows the read it already has.
  - `apply` / `show` / `after_listing` split the old `reload` body. `listing_changed`
    leaves a directory's size out of the comparison (it is worked out, or not, and
    is not the listing).
  - Back and Forward no longer `is_dir()` each place on the UI thread. The read says
    "not found", and Back tries the one before.
  - `poll_reading` runs in the run loop; Esc stops a read ("stopped reading … — still
    here"). A list or an archive opened meanwhile cancels the read.
  - The test hook `XDEVCMD_TEST_SLOW_DIR=<text>` delays the read of a directory whose
    path contains `<text>` by two seconds. It only ever delays.
- **Step 2, the bar**: `FilePane::progress` and `draw_progress`, on the pane's bottom
  border:
  - `▱▱▰▰▰▱` runs back and forth while the total is unknown, and fills when it is
    known;
  - the text: "opening slowdir… 12,400 · 3 s · Esc stops", or "reading" for a re-read;
  - "searching… N looked at" for Alt+F7, "checking N of M" for a list.
  - **Deviation:** the periodic check never shows the bar. With a slow directory open,
    the first version put "reading…" up at every check, a bar every few seconds for
    nothing anyone asked. The pty test caught it through the hook.
  - The count and the seconds show only once they are above 0.
- **Step 3, budgets** (`tests/budgets.rs`):
  - `the_release_binary_is_at_most_7_mb` checks `target/release/xdevcmd` when it
    exists, and says so when not: 4.89 MiB (5.13 MB) today;
  - `a_100000_entry_directory_is_listed_and_sorted_in_under_a_second` is ignored by
    default; run by hand it took **0.85 s** in a debug build.
- **Step 4, the flake: caught, and it was the tests.** Six full runs of the pty suite
  failed once: `a_path_is_taken_from_the_directory_its_command_ran_in` timed out, its
  history holding 3 lines.
  - The test waited for "line 3, in f", which the **typed command** also contains, so
    Shift+F12 could come before the output was printed.
  - The same pattern was in two tests' "PROMPTED". That one is worse: the test could
    go on before xdevcmd had read the prompt mark at which it takes the directory.
  - Fixed so each marker appears only in the output (`line %d … 3`, `PROMP%s … TED`).
    After that, six full runs out of six passed.
  - Round 021's "rintf" screen (one cell off) was not seen again. It is noted there.
- **Found by the flake's screen, fixed: `a/../b` in a link's path.** Round 020 showed a
  relative path joined, not resolved ("left/a/../b"). A link is now its canonical path
  (`Links::on_line`, after the existence check). The links unit tests canonicalise
  their trees, which macOS needs (`/var` is `/private/var`).
- **Step 5, docs**:
  - docs/guide.md: what a slow directory looks like; "Remote files through a mount"
    (sshfs / SSHFS-Win, rclone, SMB, NFS), and the lists keys;
  - CHANGELOG.
- **Tests**:
  - pty: `a_slow_directory_shows_a_bar_and_esc_stops_it`: the old listing stays with
    the bar, the move completes, and Esc stops a second one; a read arriving after Esc
    goes nowhere.
  - The full render suite passed unchanged after step 1 (174).
  - **Not written:** a test of a second move replacing a pending one. The hook is read
    once per process, so unit tests cannot set it per test; the code path is the first
    line of `navigate`.

- 2026-11-12, **set aside at Review** for the owner's hand tests (this round's Check), with the ROADMAP rewritten around v0.4.0 the same day (owner: "clear proceed"). The next round starts from it; CI on all three systems for the whole branch comes first.

## Check

- [x] A late read: the old listing stays, the bar shows, the move completes on
  arrival; Esc stops it; a read after Esc goes nowhere (pty). A second move replacing a
  pending one is not tested (see Do).
- [x] The full render suite unchanged (174); the pty suite 6/6 after the flake fix
- [x] The bar: reading (pty). Alt+F7 and list checking use the same `progress()`, and
  have no test of their own on the bar.
- [x] Binary-size test (4.89 MiB); the 100,000-entry benchmark: 0.85 s (debug)
- [x] The flake: caught and fixed (a test racing its own typed command)
- [x] Clippy `-D warnings` for Linux, windows-gnu and darwin; the full suite, 655 passed, 0 failed
- [ ] The owner: a real slow mount (sshfs with the server stopped, or an SMB share with
  the network off) on Linux and Windows; Esc there
