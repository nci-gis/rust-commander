# Four pillars before 0.4.0-rc.1: an outside review, checked and answered

**Date**: 2026-11-12
**Agent**: Claude Code
**Confidence**: High for the items checked in code below; Medium for the rest (the
review's own evidence labels apply).
**Status**: New. Accepted by the owner as the plan for step 6 of v0.4.0 ("okay nhé bro",
2026-11-12), to run after the current plan (xdevtheme-light, Documentation, L, D).
**Source**: the owner's outside review "xDev Commander v0.4 — Tổng hợp review và blind
challenges" (pinned at `c4f51c4`), kept verbatim in
[attachments/2026-11-12-blind-challenges-review.md](attachments/2026-11-12-blind-challenges-review.md),
and this agent's answer to it.
**Review-by**: 2026-12-15

## Problem

v0.4.0 promises "stable". An outside review of five challenge groups (data structures,
search, viewer, async, recovery) asked whether the architecture holds and what must be
true before a release candidate.

## Finding

The review's thesis holds: keep Rust + ratatui as they are; no mmap, trie, arena, Tokio
or batch WAL for 0.4. What is missing is correctness at a few edges, bounded work, honest
contracts, and proof that the shipped artifact meets the budgets. Grouped as four pillars.

### Checked again on `feat/v0.4.0` at `70ee6fa`

- **B01 compare**: `xdevcmd-core/src/compare.rs:64-77` turns a `read_dir` error into an
  empty list (`unwrap_or_default`) and names go through `to_string_lossy`. An unreadable
  directory reads as "only on the left"; a non-UTF-8 name builds a path that does not
  exist. Confirmed.
- **B04 journal**: `journal.rs:238` `line()` ignores write and flush errors (`let _ =`);
  `batch_rename.rs:107` turns a failed `Journal::begin` into `None` silently. Round 014
  item 10 only reports a copy job that starts without a journal; a write failing mid-job,
  and rename-many, say nothing. Confirmed.
- **B09 size gate**: `ci.yml` runs `cargo test --all` before `cargo build --release`, so
  `tests/budgets.rs` always skips there. The 7 MB budget is not checked by CI, although
  the ROADMAP's release bar implied it was. Confirmed.
- **B02** (cursor lands on `..` after a slow move from a results list): reproduced by the
  reviewer; not re-run here; consistent with Round 022's off-thread reads.

### Where this answer differs from the review

1. **B03 panelize**: no NUL "exact mode" for 0.4. Cheaper and enough: **a whole line that
   exists on disk is taken whole**, before the `:line` heuristics. That fixes names with
   spaces, `report:2026.txt` and all-digit names. NUL input (`find -print0`) is 0.5.
2. **A08 recovery**: a contract, not a WAL, agreed. One gap worth closing: when a copy
   **replaces** an existing file, `sync_data` the temporary file before the rename, so a
   power cut right after cannot leave the old file gone and the new one empty. New files
   are not synced (many small files stay fast). The owner decides.
3. **A03 viewer ↔ D**: progressive indexing and syntax highlighting both change the
   viewer; do them in one round, or D designed for asynchronous indexing, so the viewer
   is not changed twice.
4. **B05, B06, B07**: mostly contracts to publish (what a copy preserves per system; name
   identity vs display vs sort key), plus one fault test for B05 ("the target replaced
   after confirm"). Kept small.
5. **Performance (A01, A05, A07, B08)**: measure first, with **one** harness, not a
   framework. A01's marks membership as a `HashSet` is cheap and right; do it without
   waiting.
6. **Not in the review**: the suite fails when `NO_COLOR=1` is set in the environment;
   the test harnesses must clear it (hermetic tests).
7. **A Windows flake**, seen 2026-11-12 (run 43221291571, passed on re-run):
   `render.rs` `tilde_goes_home_and_ctrl_backslash_to_the_root` — the pane stayed in
   its directory after Ctrl+\\. For pillar 4: find the race (the key before the pane is
   ready?) or wait on the screen, as the pty tests do.

## Recommendation

Step 6 of v0.4.0 (ROADMAP), in this order of pillars; **rc.1 is not cut before pillar 1
is done**:

| Pillar | Contents | Priority |
|---|---|---|
| **1. Data correct** | B01, B02, B04, B03 (whole line first), B05 fault test, `sync_data` on replace (owner's call) | P0 |
| **2. Responsive and bounded** | A06 (request generations, stale results dropped), A07 (debounce with max-wait, a budget per tick, rescan on overflow), A03 (progressive index, with D), B08 (one shared budget), A01 `HashSet` | P1 |
| **3. Honest contracts** | Recovery contract (A08), metadata table (B06), Unicode identity (B07), panelize's limits, the compatibility promise | P1 |
| **4. Release proof** | B09 (size checked after the build, a missing artifact fails), a smoke test on the release artifacts themselves, the hand-test matrix, a benchmark protocol (OS, FS, profile, cache, runs), hermetic tests (`NO_COLOR`) | P0 gate |

Each pillar is a round with its own kickoff; P0 fixes start with a failing regression
test.

## Evidence

- Files: `crates/xdevcmd-core/src/compare.rs`, `crates/xdevcmd-core/src/journal.rs`,
  `crates/xdevcmd-core/src/batch_rename.rs`, `crates/xdevcmd/tests/budgets.rs`,
  `.github/workflows/ci.yml`
- The review: `attachments/2026-11-12-blind-challenges-review.md`

## Promotion candidate?

- [x] Yes, to `plan/ROADMAP.md` (step 6 and the release bar), done the same day with the
  owner's word.
