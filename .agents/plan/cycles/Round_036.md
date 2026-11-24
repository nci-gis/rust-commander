# Round 036: end-to-end tests for data safety, fewer cases by hand

**Status**: In Progress (the owner, 2026-11-24: "proceed", after Round 035's freeze at
`f6e6202`; opened the same day: "tạo 1 round riêng cho phần E2E, riêng 2, 6, và 7 vẫn
giữ manual")
**Part of**: **0.4.0-beta.3** (the owner, 2026-11-24: "r36 làm cho beta.3 nhé, sau r35
khi đã freeze code"): after Round 035's code freeze and before the cut, so beta.3 is cut
with 9 hand cells at most instead of 21. Test code and tooling only: the product's code
is frozen, and nothing here changes what xdevcmd does.
**Date started**: 2026-11-24
**Date completed**: YYYY-MM-DD

## Goal

The data-safety cases that a machine can run, run in CI on Linux, macOS and Windows,
so the hand tests shrink from 7 cases × 3 systems (21 cells) to the 3 cases that need
a real device or network. A hand test is asked for again only when a change touches
what that case vouches for, not after any change on the data path (the owner: "số case
ít lại để giảm block, và khi Dev nếu đụng vào phần này mới phải test lại").

## Decisions (the owner, 2026-11-24)

| Case | From now on | Why |
|---|---|---|
| M-DS-01 kill a copy that replaces | **end-to-end in CI**, 3 systems | A kill and a restart; the pty tests do it on Linux |
| M-DS-03 a big copy killed past a checkpoint | **end-to-end in CI**, 3 systems | As 01, with `fast_copy = false` where a copy is a clone |
| M-DS-04 a checked continue (C) | **end-to-end in CI**, 3 systems | pty tests on Linux and macOS already |
| M-DS-05 a move across drives, killed | **end-to-end in CI**, 3 systems | Two devices exist on a runner (Linux: tmpfs and disk, a loop-mounted FAT; Windows: C: and D:) |
| M-DS-02 a USB stick pulled | **by hand** | A real device: a write cache can report success and lose the data at removal, which no injected error shows |
| M-DS-06 a network share taken away | **by hand** | A real server and network; the retry logic itself is unit-tested with injected errors |
| M-DS-07 a hard NFS mount, server gone | **by hand** | The process waits inside the kernel and cannot be killed; the result is a time written down, not a pass or fail; on CI it can hang the runner |

## Plan

- [ ] **E1 The pty harness on Windows.** `tests/pty.rs` is `#![cfg(unix)]`;
  `portable-pty` drives ConPTY. Kill with the child's own kill (TerminateProcess, as
  Task Manager's End task), paths and the shell for Windows. Red first: one kill test
  passing on Linux and failing to build on Windows today.
- [x] **E0 Kill at a marker, small files** (section below): the checkpoint size from the
  fault seam in test builds; the Linux tests that slow and kill on time moved to
  `Halt` markers. Test code and the `faults` seam only.
- [ ] **E2 Cases 01, 03, 04 on all three systems.** The tests that exist for Linux (the
  trash case, the checkpoint case) and for Linux and macOS (resume) run on macOS and
  Windows too, or say why a system cannot (named, not skipped silently). macOS: fast
  copy off for 03 (APFS clones).
- [ ] **E3 Case 05 across two real devices.** Linux: `/dev/shm` → disk, and a
  loop-mounted FAT32 image (sudo on the runner); Windows: C: → D:; macOS: an `hdiutil`
  RAM disk. Kill mid-move, start again, every file on one side or both.
- [ ] **E4 The ledger per case.** `qa ledger` today makes every cell stale when any
  data-path file changes. Each hand case gets the files it vouches for (02: the copy
  and its sync, the part files, FAT names; 06: retry and the network check; 07: cancel
  and waits), and a cell is stale only when one of *its* files changed since its build.
  Cases 01, 03, 04, 05 leave the ledger: CI green at the cut commit is their proof.
- [ ] **E5 The documents.** `tests/manual/test-cases/data-safety.md` keeps 02, 06, 07;
  01, 03, 04, 05 move to a list of what CI proves and where. `docs/how-to-test.md`,
  `docs/development.md` and the hand-test README follow. `xtask try` checks A–C stay
  for an owner who wants to look by hand, no longer required.

## Small and timed by markers, not by speed

Not a speed test (the owner, 2026-11-24: "giảm số files, hoặc file size lại vì ko phải
test perf"). So:

- **The kill lands at a marker, not after a while.** The fault seam's `Halt` writes a
  marker file and blocks at the chosen step (past the trash, past a checkpoint's
  journal line, after a move's copy and before its source goes); the test waits for
  the marker and kills then. A slow runner only takes longer to get there; it cannot
  kill too early or too late. Today's Linux checkpoint test slows each chunk and
  kills on time: replaced by a marker.
- **Few, small files.** 01: three files to replace; 04: about twenty small files; 05:
  about ten. 03 needs a file past a checkpoint, which is 256 MiB today: the checkpoint
  size is read from the fault seam in test builds (`faults` is a dev-dependency's
  feature, never in a release, as `tests/budgets.rs` checks), so 03 copies a few MiB
  with a checkpoint every 1 MiB.
- Each test asserts it was killed at its marker, so a pass means the case was met.

## Risks

- A kill test that is flaky on a slow Windows or macOS runner blocks every PR: kills at
  markers (above), the new tests ×10 on each system before they gate, `wait_for` only.
- A test can pass for the wrong reason (the kill landing before anything was written).
  Each test asserts that the kill landed where the case needs it (past the trash, past
  a checkpoint, mid-move), as the Linux ones do.
- Fewer hand cells means fewer eyes on the real dialogs. The wording is checked by the
  pty tests' assertions; a release still gets the owner's own week of use (0.4.0).
- Per-case staleness can miss a file a case depends on. The list per case is reviewed
  with the red-team skill (one agent), and a change to `DATA_PATH` or a case's list
  needs a test, as the data path has since beta.3.

## Do

- **E0** (2026-11-24): already so; nothing to change. Every data-safety kill test in
  `tests/pty.rs` waits for state, not time: 01 and the planned copy for a `halt|rename`
  marker, 05 for `halt|remove-source`, 03 for the journal's `ckpt` line (the per-chunk
  delay only keeps the window open, ~0.25 s after the line). Files are few (1–4) and
  small, except 03's 384 MiB, which is sparse: a checkpoint is 256 MiB (`ops::CHECKPOINT`)
  and a smaller one would change product code, which the freeze rules out. Times here
  (Linux): 01 4.7 s, planned copy 4.6 s, 05 4.3 s, 03 7.7 s, 04 2.3 s.
  So the risk of a slow runner is the harness's patience, not when the kill lands.

## Check

- [ ] CI runs 01, 03, 04, 05 on all three systems, ×10 green before they gate.
- [ ] `qa ledger` asks for 02, 06, 07 only, and only where their files changed.
- [ ] One deliberate change in a case's files makes its cells stale, and one outside
  them does not (a test).

## Act

**Learnings**:

**Promotions**:
