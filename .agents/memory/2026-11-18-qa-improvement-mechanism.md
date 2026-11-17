# QA improvement: escape loop and the ladder of controls (rc.1 pilot)

**Date**: 2026-11-18
**Agent**: Claude Code
**Confidence**: Medium
**Status**: New
**Source**: The owner's decision after the external repo and docs reviews of 2026-11-17 (PR #43); see 2026-11-17-qa-beta2.md
**Review-by**: 2026-12-31

## Problem

The external reviews found 7 code issues (3 High, 4 Medium) and 15 docs issues that our QA and red-team did not catch. They used the same model family with a different method. Some of the issues broke rules we already had: R13 broke the trust rule in reference/docs.md, and the data-path red-team rule was applied only to the diff. Adding more prose rules would repeat the failure.

## Finding

The misses fall into four root classes:
1. **Red-team scope.** It covered the diff and not the boundary the change sits on.
2. **Feature interactions.** Combinations were never tested: C × checkpoint, Copy As × resume, root × recovery, F × a failed removal.
3. **Facts kept in several places with no completeness check.** Two cases: the line kinds versus may_tidy, and the Request fields versus the journal versus resume.
4. **Rules with no mechanism behind them.**

The ladder of controls, strongest first:
1. A design that makes the defect impossible (a single source of truth).
2. The compiler or a test breaks.
3. A script finds it.
4. A rule in a skill.
5. Judgement.

The question to ask is "can this move up a rung?", not only "can this be coded?". Checks that guard the product belong in the repo, where cargo test, xtask and CI run them. A script kept in a skill runs only when someone remembers to run it. Skill scripts are for the process: an interaction matrix, a scan of docs claims, an evidence ledger. They are stdlib-only Python run with `python3 -I`, or uv with PEP 723 inline metadata only when a dependency is needed. References are data for judgement: modes.md, attack-catalog.md, past-misses.md.

## Evidence

- PR #43: RR01–RR07 and R01–R15, each with its fix and test.
- The root causes are in 2026-11-17-qa-beta2.md, section "External review round".

## Recommendation

The owner approved this on 2026-11-18 for **round rc.1**, as a pilot. It is one root class, taken around the whole loop and measured; it is not built all at once.

- **Loop.** For each escape:
  1. Classify its root class.
  2. Move it to the highest rung the team can reach.
  3. Add a line to past-misses.md.
  4. Measure.
- **Escalation rule.** A class that recurs a second time while it is still only a rule (rung 4) must move to rung 3 or higher.
- **Metric.** The escape rate: P0/P1 findings from an outside review that our QA should have caught. Record it in each round's Check.
- **Pilot class.** Facts kept in several places (RR02, RR04, RR05):
  1. Repo, rung 1: one table of journal line kinds with their path counts, used by the writer, tidy and may_tidy.
  2. Repo, rung 2: a Request field-completeness test using an exhaustive destructure with no `..`.
  3. Skill (owner approval required, logged in plan/promotions.md): references/past-misses.md with the 7 lines from 2026-11-17, and a scan for absolute claims in the docs.
  4. Repo/CI: the link and anchor checker, also run on each packed archive.
  5. Measure: the escape rate at the external-style review before the rc.1 cut.
- **Risks.**
  - A table that a script generates can be read as "done". Scripts list things; people and agents judge them.
  - SKILL.md keeps the judgement and when to run each script. The detail goes in references.
