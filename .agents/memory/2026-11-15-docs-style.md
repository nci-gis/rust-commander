# How docs are written in this repository

**Date**: 2026-11-15
**Agent**: Claude Code
**Confidence**: High
**Status**: New
**Source**: The owner asked for it (2026-11-15) after Unreleased in CHANGELOG was cut
from 7 KB to 3 KB and docs/benchmark.md was written; drawn from the existing docs.
**Review-by**: n/a

## Problem

No written rule for `docs/`, README and CHANGELOG, so entries grew long (CHANGELOG
bullets of 300–600 characters) and the same steps were told in two places.

## Finding

The style the docs already follow, and the owner's values behind it: safety and
security, no over-promising, tidy docs, convenience, fair to other tools.

- **Plain English, short sentences.** Say what happens, not how it is built. Bold the
  thing, then the effect: "**`q` no longer quits**: F10 and Ctrl+q do."
- **One place per fact.** Steps live in one page; others link to it
  (`[Benchmarks](benchmark.md)`). Moving text means deleting the old copy.
- **No over-promising.** Say what is tested and where (README's Platforms table); say
  what is not done and what to use instead (rsync, robocopy, rclone). Numbers come
  with their conditions (Linux, warm cache, 5,000 files).
- **Safety first in steps.** Never tell a reader to turn off antivirus or a security
  feature; a command that unblocks a download says "only for a file from this
  repository". Say what a tool never touches and that nothing is sent.
- **Commands** in fenced blocks, one shell per block (`bash`, `powershell`), runnable as
  they are; placeholders that look like placeholders (`STICK`, `mbp-m2-apfs`).
- **CHANGELOG**: one or two lines a bullet, what a user notices, detail left to the docs.
  Most important first in each group. Released sections are never rewritten.
- **Docs ship with the program** (`docs/*.md` in the archives): no links to private or
  agent files (`.agents/`), no dates that go stale in user pages.
- **New page**: add it to `docs/README.md`'s list; check every `#anchor` it links to.

## Evidence

- `docs/benchmark.md`, `docs/compatibility.md`, `docs/guide.md` ("What xdevcmd doesn't
  do"), README Platforms table.
- CHANGELOG Unreleased before and after the cut (commit after 232286c).

## Recommendation

**Do**: read this before writing in `docs/`, README or CHANGELOG.
**Don't**: explain the implementation to a user, repeat a page's steps elsewhere, or
claim what has not been checked.

## Promotion candidate?

- [x] `reference/` — as `reference/docs.md`, trigger "writing or editing `docs/`,
  README or CHANGELOG" (the owner's call)
- [ ] `skills/`
- [ ] Not yet
