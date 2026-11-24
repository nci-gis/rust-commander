# Round 036: end-to-end tests for data safety, fewer cases by hand

**Status**: Planning (opened by the owner, 2026-11-24: "tạo 1 round riêng cho phần E2E,
riêng 2, 6, và 7 vẫn giữ manual")
**Part of**: v0.4.x, `ROADMAP.md` (After beta.2: 0.4.x). Test tooling, not a feature:
when it runs is the owner's call; proposed after the beta.3 cut, so beta.3's hand tests
are run as they stand.
**Date started**: YYYY-MM-DD
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

## Risks

- A kill test that is flaky on a slow Windows or macOS runner blocks every PR: run the
  new ones ×10 on each system before they gate, and keep `wait_for` (no sleeps).
- A test can pass for the wrong reason (the kill landing before anything was written).
  Each test asserts that the kill landed where the case needs it (past the trash, past
  a checkpoint, mid-move), as the Linux ones do.
- Fewer hand cells means fewer eyes on the real dialogs. The wording is checked by the
  pty tests' assertions; a release still gets the owner's own week of use (0.4.0).
- Per-case staleness can miss a file a case depends on. The list per case is reviewed
  with the red-team skill (one agent), and a change to `DATA_PATH` or a case's list
  needs a test, as the data path has since beta.3.

## Do

## Check

- [ ] CI runs 01, 03, 04, 05 on all three systems, ×10 green before they gate.
- [ ] `qa ledger` asks for 02, 06, 07 only, and only where their files changed.
- [ ] One deliberate change in a case's files makes its cells stale, and one outside
  them does not (a test).

## Act

**Learnings**:

**Promotions**:
