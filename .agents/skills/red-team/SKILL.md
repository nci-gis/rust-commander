---
name: red-team
description: Independent multi-role critique and blind judging before a consequential decision. Use before approving a plan, cutting a release, or merging a change on a data path; after any plan was simplified; and whenever the AI is fixing a bug it introduced itself.
---

## Trigger

This skill activates before an approval that is costly to undo:

- a plan the owner is asked to approve (release plan, fix plan, migration);
- a release cut (beta, rc, stable);
- a change on a data path (copy, move, delete, trash, journal, resume);
- a round that **removed** something from a plan (subtraction);
- a fix for a bug the AI introduced itself.

Skip it for small, reversible work: the cost is real (one round = 4 agents).

## Why (evidence, 2026-11-15/16)

- Two bugs in B2 were written by the AI and passed its own tests; independent
  review found both. One "fix" (443829a) made a test harness more patient and
  hid a live bug.
- Four plan versions in a row (A6–A9) each carried a fatal flaw — a speed gate
  that could not run, a `src/..` row that deletes its parent, false refusals —
  and only the final red-team round caught them. Subtraction reopened a
  data-loss path that an earlier round had closed.
- A rubric with no usability or speed criterion produced a safe but blunt plan.
  What the rubric does not ask, the panel does not optimise.

## Procedure

1. **Fix the rubric first.** Write the criteria and weights to a file before
   round 1. Changing them later is a decision entry made *before* new data,
   with the reason — never after seeing scores.
2. **Panel of four, in parallel, each in a fresh agent:** PM (priorities,
   cost, scope), BA (requirements, user's view, wording), Dev (design verified
   in code), QA (proof that would have caught the original bug). Give them the
   brief, the rubric and the current plan — **not** the author's rationale or a
   PR body. Each must try to *disprove*, cite `file:line`, label claims
   REPRODUCED / READ / SUSPECT, and score the plan on the rubric.
3. **Vary the round's question** so rounds don't repeat each other:
   open critique → pre-mortem ("it is 3 weeks later and it went badly") →
   sharpen (the strengths the rubric rewards) → subtraction (cut what doesn't
   pay) → **final red-team** (blocking issues only; SIGN / SIGN WITH
   AMENDMENTS / REFUSE). The final red-team is mandatory after any
   subtraction and must check what was cut against what earlier rounds needed.
4. **Synthesize** each round into the next version, self-contained (no "as in
   the previous version"). Resolve contradictions by naming them.
5. **Verify before relaying.** The main session re-checks the 2–3 most severe
   claims in the code itself; drop or downgrade what it can't confirm.
6. **Blind judging.** At least two independent judges score all versions on the
   fixed rubric: labels shuffled, headings stripped, plans self-contained,
   **no hints about what to check**, no access to the rounds. Report both
   tables, the ranks, the margin and the judges' confidence. If the last
   version does not win, say so.
7. **Report honestly:** all agents share one model; hinting a judge biases it;
   "best" means best among the versions judged.
8. **The human decides.** Agents propose; the owner approves the plan, merges,
   closes rounds and changes the rubric.

## A change, not a plan (one agent)

For a pull request on a data path, one fresh agent is enough. It gets the branch,
not the PR body or the author's reasoning.

- **Scope:** the diff, then every sibling (other copies of the same concept, found
  by name and behaviour), then the callers. Lessons L1–L3 in
  [reference/review-lessons.md](../../reference/review-lessons.md).
- **No sibling proof, no merge:** a data-path fix shows each sibling either fixed or
  not affected, with `file:line`.
- **Evidence-anchored:** every finding carries a reproduction or `file:line`, labelled
  REPRODUCED / READ / SUSPECT. A score with no evidence is dropped.
- **Verdict:** SIGN / SIGN WITH AMENDMENTS / REFUSE. Amendments are fixed, then
  checked again. What the round found goes into the QA skill's past-misses.

## Rules that came out of it

- No proof, no merge: a fix whose proof step was skipped goes to the next
  release, it does not ship unproven.
- A test fix that makes a harness wait longer needs a test of the live path too.
- A refusal must name the next safe action; a false refusal is a bug.
- A gate (speed, CI) must be shown to fire before it is trusted.

## Output

Per round: one file per role (`R<n>_<role>.md`) and the next plan version.
At the end: the judges' tables, the ranking and the chosen plan, saved where
the owner keeps decisions (Round file `Check`, the release plan).
