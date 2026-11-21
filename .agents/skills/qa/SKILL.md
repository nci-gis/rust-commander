---
name: qa
description: Check that xdevcmd does what its user docs promise, with tests that would fail for a real user, not tests that restate the code. Use before a release cut, after a run of fixes on a data path, or when the owner doubts the tests.
---

## Trigger

This skill activates when:

- a release (beta, rc, stable) is about to be cut;
- several fixes landed on a data path (copy, move, delete, trash, undo, resume,
  archives, typing into the shell);
- the owner asks whether the tests prove the user's expectation or only the code.

Skip it for a one-line fix: the red-team skill and a red test are enough there.

## Why (evidence, 2026-11-17)

- Of 352 end-to-end tests, 101 look at the disk; 251 read only the screen. A test that
  reads only the screen passes while a file is lost.
- Most tests were written by the agent that wrote the code, beside it: they risk
  checking the message it just wrote, not what the user expects.
- `refused next=CtrlC`: every test passed until one acted on the log as a user (or an
  agent) would, by pressing the key it names.

## The oracle

**The user docs are the spec** (`docs/*.md`, README): what xdevcmd promises a user, in
words independent of the code. An expectation comes from a doc line, never from
reading the implementation. Where the docs say nothing, the expectation is what a
Total Commander or Midnight Commander user would assume, and the gap is a doc finding.

A test **proves** an expectation only if it:

1. drives the user's path (keys, clicks, flags in `--screenshot`/pty; not an inner
   function), and
2. checks the outcome the user cares about: the disk first (which files exist, their
   bytes, the trash), the screen second, and
3. would fail on a build with the bug (shown red, or reasoned with file:line).

A test that only matches the text the code prints **confirms**; it does not prove.

## Procedure

1. **Scope.** Pick the areas (data paths first). Write the list of doc files and the
   date to the round's `Check`. Cross each new feature with each mode it can meet:
   verify (C), `--root`, read-only, resume, Copy As, a failed removal. Defects hide
   where features meet (lesson L4).
2. **Audit, one fresh agent per area, in parallel.** Give it the docs and the test
   files, **not** the source or the PR bodies. It lists every promise in its area as
   `doc:line → expectation`, then finds the tests for each, and marks each:
   PROVES / CONFIRMS (screen text only, or an inner call) / MISSING / MANUAL (cannot
   be automated here: a real terminal, a power cut, another OS). Output: one table.
3. **Hunt, one fresh agent per area, in parallel.** It builds and runs the real
   binary in a sandbox (a scratch root; `XDEVCMD_CONFIG`, `XDEVCMD_STATE`,
   `XDG_DATA_HOME`, `HOME` inside it; never the user's files or trash), tries to break
   each promise (names with spaces and odd bytes, links, a file where a folder is,
   kills mid-job, full or read-only targets, slow folders), and reports only what it
   reproduced, with the exact commands. Labels: REPRODUCED / SUSPECT.
4. **Verify before relaying.** The main session reproduces each REPRODUCED finding
   itself. A bug that loses or wrongly changes a file is P0; a wrong refusal or a
   dead end is P1.
5. **Fix only bugs, red first.** P0/P1: a failing test through the user's path,
   then the fix, then the red-team skill on it if it touches a data path. No new
   features; a missing feature is a known issue.
6. **Close the gaps.** MISSING and CONFIRMS on a data path: a test that PROVES, if it
   can run in CI. MANUAL: a case in `crates/xdevcmd/tests/manual/` (ID, platform,
   steps, expected, what a fail looks like).
7. **Record** every defect in
   [references/past-misses.md](references/past-misses.md), with its class, the layer
   that found it and the layer that should have. A class that recurs moves up the
   ladder ([reference/review-lessons.md](../../reference/review-lessons.md)). A
   "later" decision that a user would see adds a line to `docs/known-issues.md`.
8. **Report** to the owner: the audit table's counts, the bugs (fixed, known), the
   tests added, the manual cases, the **escape rate** (past-misses), and what is still
   unproven. Before a cut, `cargo xtask qa ledger` must say PASS: every data-safety
   case on every system has a run in `tests/manual/runs.tsv` built after the last
   commit on a data path, or a waiver (lesson L7). The owner decides whether to cut.

## Rules

- Never trust a pass you have not seen fail: a new test is run against the bug (or
  beta.1) once, and the result is written down.
- A test made more patient to pass needs a second test of the live path.
- Agents share one model: an expectation two agents agree on is still an inference;
  the doc line is the evidence.
