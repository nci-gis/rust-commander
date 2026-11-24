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
- **E1, E2** (2026-11-24): `tests/winpty` (the ConPTY harness of the two Windows tests)
  became `tests/harness`, for all three systems: a scratch directory a killed run
  leaves for the next, `kill_hard` (`kill -9`; TerminateProcess on Windows), the
  journal's path encoding per system, a second device. `tests/kill.rs` holds the five
  kill tests, moved from `pty.rs` (Unix only): M-DS-01 (planned copy at a rename; with
  the trash on Linux only, the test build's trash being Linux's), M-DS-03 (no longer
  Linux only), M-DS-04, M-DS-05. Windows found one test bug at once: a path joined
  from `"a-tree/sub"` keeps its `/` where the journal writes `\` (CI 45352301675).
- **E3**: already there. CI mounts a second device on every system (Linux tmpfs, macOS
  a RAM disk, Windows the runner's other volume), and M-DS-05 ran on all four runners
  (none said "skipped"). Not done: a loop-mounted FAT32 for FAT's rules on Linux; FAT
  stays with M-DS-02 by hand.
- **E4**: `CASE_PATHS` in `xtask/src/qa.rs`: 02, 06, 07 each name the files they vouch
  for; a cell is stale only when one of them changes. Test red without it.
- **E5**: `data-safety.md` keeps 02, 06, 07 and says where CI runs the rest; README
  follows; `xtask manual`'s log takes the case list (its tests keep the seven).
- **Red-team** (final round after the subtraction, 2026-11-24): SIGN WITH AMENDMENTS.
  Amended: M-DS-05 cannot pass by skipping (CI sets `XDEVCMD_REQUIRE_OTHER_DEV`); a
  folder of ten moved and killed; M-DS-04 with a real kill, a zeroed finished copy and
  "1 found different"; M-DS-03 compared MiB by MiB; `CASE_PATHS` + `noreplace.rs`,
  `file_ops.rs`, `modal.rs`, `app/modals.rs`; README, how-to-test, development.md.
  ×10 again: run 45357438349, 4/4 runners; PR CI 45357443903 green 7/7.
- **The trash on every system** (the owner, 2026-11-24: "bro sửa code luôn nhé, phạm vi
  nhỏ"): a test build's trash is `XDEVCMD_TEST_TRASH` when set (`faults::test_trash`,
  read by `plan::trash_keeps` and `ops::send_to_trash`; nothing in a release), so the
  kill between the trash and the rename runs on macOS and Windows too. **A change to
  product code past Round 035's freeze**, on the owner's word: `plan.rs`,
  `ops/remove.rs` and `faults.rs` are on the data path, so hand evidence must be built
  from this PR's merge or later (M-DS-02 and 06 list `plan.rs`; all three list `ops/`
  and `faults.rs`). The read-only docs follow, on the owner's word too:
  `reference/testing.md`, `skills/qa/SKILL.md` (logged in `promotions.md`).

## Check

- [x] CI runs 01, 03, 04, 05 on all three systems, ×10 green before they gate:
  run 45354712957 (a throwaway workflow on `tmp/r36-x10`, never merged), 10 of 10 on
  ubuntu, ubuntu ARM, macOS and Windows; 7–15 s a run.
- [x] `qa ledger` asks for 02, 06, 07 only, and only where their files changed.
- [x] One deliberate change in a case's files makes its cells stale, and one outside
  them does not (a test): `a_hand_cell_goes_stale_only_when_its_own_files_change`.

## Act

**Learnings**:

**Promotions**:
