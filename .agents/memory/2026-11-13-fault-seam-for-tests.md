# The fault seam: how a data-safety test is written

**Date**: 2026-11-13
**Agent**: Claude Code
**Confidence**: High
**Status**: Promoted (2026-11-13, `reference/testing.md`, "Faults")
**Source**: Round 029 (pillar 1), steps 0–7
**Review-by**: 2027-05-13

## Problem

Data-safety fixes (sync before rename, journal failures, a file replaced after the
user's answer) cannot be shown failing on a real disk on demand, and an order bug (a
sync moved after the rename) passes every content check.

## Finding

`xdevcmd-core/src/faults.rs` is the seam: `faults::at(Point, path)` before each guarded
call (`Write`, `SyncData`, `SyncDir`, `Rename`, `RemoveSource`, `JournalBegin`,
`JournalLine`, `JournalSync`, `ReadDir`, `Stat`), rules matched on a path fragment
(`fail`, `set(.., Action::Swap/Halt/Delay/Skip)`), held by a guard, and a per-fragment
trace for order (`faults::trace` / `faults::traced`). It exists only under
`cfg(any(test, feature = "faults"))`; `xdevcmd`'s dev-dependency turns the feature on
for the binary the pty/render tests run, which read `XDEVCMD_FAULTS`
(`kind|point|fragment|arg`, `;`-separated; `|` because `:` cuts Windows paths).
`tests/budgets.rs` checks the release build has none of it.

Test places: `faults::places::{other_device, small_fs}` (`XDEVCMD_TEST_OTHER_DEV`,
`XDEVCMD_TEST_SMALL_FS`, mounted by CI, skipped with a line elsewhere).

Proving "fails on today's code" for a seam-based test: run it in a `git worktree` of
the previous commit with the one `faults::at` call put in front of the old call it
guards, tests written to compile on both (assert on `format!("{q:?}")` text, not on new
variants).

## Evidence

- Files: `crates/xdevcmd-core/src/faults.rs`, `.github/workflows/ci.yml`,
  `crates/xdevcmd/tests/budgets.rs`, the step entries in `plan/cycles/Round_029.md` Do.

## Recommendation

**Do**: a new write path that cannot be undone goes through `faults::at` (or
`faults::sync_data` / `sync_dir`, `ops::commit_replace`), with an order test from the
trace.
**Don't**: add `#[cfg(test)]` statics per module for a new hook (`appear_in_test`,
`writing_in_test` are the old pattern), or put a seam in the release build.

## Promotion candidate?

- [x] `context/` — a paragraph in `reference/testing.md` ("Faults"), next to the four
  layers; human call
- [ ] `skills/`
- [ ] Not yet
