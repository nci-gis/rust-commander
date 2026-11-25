---
name: docs-review
description: Check that the user docs still say what the code does, page by page, with evidence for every claim; reports what is false or unverified and the ledger rows to write. Use when the owner asks to review the docs or a page, or after a change to behaviour.
---

## Trigger

This skill activates when the owner asks ("review the docs", "docs staling?", a page
named). It never runs on its own, and adds no step to a release (the owner, 2026-11-25).

Scope: `docs/*.md`, `README.md`, `SECURITY.md`, `crates/xdevcmd-core/README.md`, as
listed in `xtask/src/docs.rs` (`DOC_PATHS`). Out: `.agents/` (history), released
`CHANGELOG.md` sections, generated pages with a test (`docs/keys.md`).

## Procedure

1. **What a machine can check**: `cargo xtask docs check` (links and `#anchors`,
   `xdevcmd --flags` and `xtask` words, `XDEVCMD_*`, `[section] key`). Fix what it
   reports first; it is exact and cheap.
2. **What to read**: `cargo xtask docs status`. Review the pages not `ok` (never
   reviewed, page changed, code changed), or the pages the owner named. A page named
   is read in full even when `ok`.
3. **One independent sub-agent per page** (it wrote neither the page nor the code it
   checks). It:
   - lists every claim the page makes: keys and what they do, defaults, what is
     refused and why, what happens on failure, paths, names, numbers, limits;
   - for each, finds the evidence and reads it: a test that asserts it (its name), or
     the code that does it (`path::symbol`). Not line numbers: they move;
   - marks each **true**, **false** (with what the code does instead) or
     **unverified** (no evidence found). "Looks right" is unverified.
4. **Report**, per page: false claims first, then unverified, then the count of true
   ones. For each false claim, the fix to the page, or, if the page is right and the
   code is wrong, a bug to report (this skill changes no code).
5. **With the owner's agreement**: apply the doc fixes, then write the page's row in
   `.agents/ledgers/docs-review.tsv`, replacing its old row:
   `path`, `hash` (`git hash-object <page>`, after the fixes), `commit` (the commit the
   code was read at, `git rev-parse HEAD`), `date`, `by`, `result` (`ok` | `fixed` |
   `open`), `note` (what is open). Never a row for a page not read in full.

## Rules

- Evidence or it is not checked.
- A claim about another program (mc, ls, a terminal) is out of scope: say so.
- A page whose `DOC_PATHS` look wrong (it speaks of code not listed): say so in the
  report; the list changes in code, with its test.
- Do not edit `.agents/` beyond the ledger; do not change code.

## Why

Two beta.4 statements were false for days and found by chance: the guide said F7
refused Windows' characters (it did not, fc7deb3), and the plan said CI had a FAT mount
(it had not). Tests cover behaviour; nothing covered the sentences. The shape follows
OpenWiki's "grounded claims" (facts tied to source evidence), without the tool.
