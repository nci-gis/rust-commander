# Draft skill: docs-review (to promote to skills/docs-review/SKILL.md)

**Date**: 2026-11-25
**Agent**: Claude Code
**Confidence**: Medium
**Status**: Promoted (skills/docs-review/SKILL.md, 2026-11-25)
**Source**: Round 038 (the owner, 2026-11-25: a skill the user calls when wanted, a
ledger of the last review per page with its hash)
**Review-by**: 2026-12-31

## Problem

Docs go stale when the code moves and nobody re-reads the prose. Two cases in beta.4
(guide on F7, the FAT mount) were found only by chance.

## Finding

A review that is asked for, cheap to aim (what changed since last time) and strict on
evidence (every claim tied to code or a test) catches what tests cannot: sentences.

## Evidence

- Files: `docs/guide.md` (File names, corrected in fc7deb3), `Round_037.md` W8 note.
- OpenWiki's "Grounded Claims": facts tied to versioned source evidence.

## Recommendation: the skill, as it would be saved

```markdown
---
name: docs-review
description: Check that the user docs still say what the code does. Use when asked to
  review the docs, before a release if wanted, or after a change to behaviour; reports
  per page with evidence and the ledger rows to write.
---

## Trigger
The owner asks ("review the docs", "docs staling?", a page named). Never on its own.

## Steps
1. `cargo xtask docs status`: pages never reviewed, page changed, code changed, ok.
   Review those not ok, or the pages named. Out of scope: `.agents/`, released
   CHANGELOG sections, generated files with a test.
2. One independent sub-agent per page (it has not written the page or the code). It:
   - lists every claim the page makes: keys, defaults, what is refused, what happens
     on failure, paths, names, numbers;
   - for each, finds the evidence: a test that asserts it (its name), or the code that
     does it (`file::symbol`), read, not guessed;
   - marks it true, false (what the code does instead), or unverified (no evidence).
3. Report per page: false claims first, then unverified, then a count of true ones.
   For each false one, a fix to the page (or, if the docs are right and the code is
   wrong, say so: that is a bug, not a doc fix).
4. With the owner's agreement: apply the fixes, then write the page's row in
   `docs-review.tsv` (path, sha256 of the page now, the commit read, date, by, result,
   note). Never write a row for a page not read in full.

## Rules
- Evidence or it is not checked. "Looks right" is unverified.
- A claim about another program (ls, mc, a terminal) is out of scope: say so.
- Do not edit `.agents/`; do not change code in this skill (report bugs instead).
```
