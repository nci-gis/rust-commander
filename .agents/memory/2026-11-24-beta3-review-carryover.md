# beta.3 review: what was not fixed before the freeze

**Date**: 2026-11-24
**Agent**: Claude Code
**Confidence**: High (each item from an independent review, cited by file:line)
**Status**: New
**Source**: the three beta.3 red-team rounds (data path; app, CI and xtask; the check of
#90), 2026-11-24. Fixed before the freeze: #90, #93, #94. The owner, 2026-11-24: the
rest goes to rc, logged so none is missed ("bro ghi log lại để ko bị missed nhé").
**Review-by**: at rc.1's plan

## Problem

The reviews found more than the freeze should take. These are the findings left open,
each with where it goes. Line numbers are at `feat/v0.4.0-beta.3` after #93/#94.

## Finding

### rc.1 (fixes, no new behaviour)

| # | What | Where | Effect |
|---|---|---|---|
| C1 | A worker read with a bare `try_recv`: dead and "not yet" look the same | `app/run.rs` (confirm's plan, the delete count), `pane/files.rs` `poll_opening` | "counting…" / "opening…" for ever if the thread panics; Enter and Esc still work. Use `task::Task` (W8) |
| C2 | A `Task` replaced without `stop()` | `app/file_ops.rs` (a second Alt+H) | The first checksum keeps reading to its end (as before beta.3) |
| C3 | Widths still summed per char | `pane/files.rs` `crumbs` (click positions), `commands.rs` filter cursor; other copies of width: `modal.rs`, `usage.rs`, `wrap.rs` | With an emoji in a folder name or filter, a click or the cursor is a column off. `qa defs` misses them (it searches `fn` names) |
| C4 | The release notes' `sed` | `.github/workflows/release.yml` | Also rewrites links inside code spans, turns image links into blob pages, leaves `#anchor` links broken |
| C5 | Walker details | `app/draw.rs`, `walker.rs` | Covers a progress label on the same edge for a step; vanishes during its hold when the status line is full; the same message twice gets no hold |
| C6 | A picture that cannot be decoded says "stopped on an internal error" | `app/run.rs` | "could not decode this picture" would tell more |
| C7 | Copies of the sort words | `settings_dialog.rs`, `sort.rs` FromStr error | Spelled again by hand (L1); the words are pinned by tests since #90 |
| C8 | The CLI table test checks that each option is accepted, not what it sets | `main.rs` | A swapped field would pass |
| C9 | A test reads `HOME` raw | `bookmarks.rs` tests | Fails under a relative `HOME`, which `dirs` now ignores |

### beta.4 (languages): text built in pieces

| # | What | Where |
|---|---|---|
| L1 | `Mode::turned_off(what, next)` splices English phrases from `Action::effect` and a `next` with its own dash | `config.rs`, `keys.rs` |
| L2 | `"{} mode"` built a second way | `app/open.rs` |
| L3 | `DIED` glued onto other text | `app/run.rs`, `app/compare.rs` |

### 0.4.x / measures

| # | What | Where |
|---|---|---|
| M1 | `fs_kind` off Linux and macOS calls `unc`, compiled only on Windows or in tests: a BSD build fails (as before beta.3) | `os.rs` |
| M2 | `qa defs` "App fields" drops when fields are grouped without state going away: a weak measure of the refactor | `xtask/src/qa.rs` |
| M3 | `app/input.rs`, `app/modals.rs` held data-path glue; moved to `app/jobs.rs` (#94). Round 036 E4 gives each hand case its own files | done in #94; E4 for the rest |

## Evidence

The three review reports (2026-11-24) in the session, each finding labelled
REPRODUCED / READ with file:line; the PRs that fixed the rest: #90, #93, #94.

## Recommendation

**Do**: open rc.1's plan from this table; each C row is one PR with a red test first.
**Don't**: fold C rows into beta.3 after the freeze; they do not lose data.

## Promotion candidate?

- [ ] `context/` — stable, broadly applicable, seen more than once
- [ ] `skills/` — a reusable procedure with a clear trigger
- [x] Not yet — a work list, closed when rc.1 is planned
