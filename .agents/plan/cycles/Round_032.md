# Round 032: Pre-release plumbing and 0.4.0-beta.1 (v0.4.0-beta.1, part 4 of 4)

**Status**: Planning — outline; the kickoff is written when Round 031 is Complete
**Part of**: v0.4.0, step 6 of `ROADMAP.md`; Decisions "Release path" and "Where
pre-releases go" (owner, 2026-11-13: GitHub only, marked pre-release, never "Latest";
crates.io, Scoop, winget and binstall start at 0.4.0)
**Date started**: YYYY-MM-DD
**Date completed**: YYYY-MM-DD

## Goal

A `v0.4.0-beta.1` tag on `dev` builds the six archives and opens a draft GitHub release
marked pre-release; nothing goes to crates.io, Scoop or winget. Then beta.1 is cut, and
the owner uses it for about a week.

## Plan (outline)

### Steps (each a commit)

1. **`release.yml`** (`check` job): a tag matching `v<x.y.z>-beta.N` or `-rc.N` must be
   on `origin/dev` (or main); a stable tag `v<x.y.z>` must still be on main; anything
   else is refused. The CHANGELOG check escapes the dots of the version in its regular
   expression (today `0.4.0` matches `0x4y0`; harmless, but a pre-release has more
   dots). The `release` job passes `--prerelease --latest=false` for such a tag, and
   keeps `--draft`. A `workflow_dispatch` with a `tag` input that runs only `check`, so
   the rules are tried before a real tag. **S.**
2. **`docs/development.md`, "Pre-releases"**: `cargo release 0.4.0-beta.1 --no-publish
   --no-tag --no-push` on `dev`, the CHANGELOG heading `## [0.4.0-beta.1] - date`, the
   tag on `dev`, the draft, Publish; steps 5 and 6 (crates.io, Scoop, winget) skipped;
   how a later beta and the rc follow; deleting a bad pre-release tag. **S.**
3. **The beta.1 hand-test checklist** (the owner's, a week of use): Round 029's §7 "by
   hand" list (a USB stick in FAT32 and exFAT, an SMB share, a stick pulled mid-copy,
   a kill mid-move), the planner's tables, undo, the trash, pause and retry, on Linux,
   Windows and macOS. **S.**
4. **The cut**: `feat/v0.4.0` into `dev` by pull request with CI green on all four
   runners; the version raised; CHANGELOG dated; tag; the draft read; Publish by the
   owner. **S.**

### Risks

- The workflow is exercised only by a tag: the `workflow_dispatch` dry run covers the
  check; the build part is unchanged from 0.3.0.
- Help → Documentation (`crates/xdevcmd/src/docs.rs`) links to the tag `v{version}`:
  for beta.1 that is `v0.4.0-beta.1`, which will exist. Checked by hand on the draft.
- `cargo release` with `publish = true` in `release.toml`: every pre-release command
  says `--no-publish`; a pre-release on crates.io could never be deleted.

### Open decisions (recommendations at kickoff)

1. **CHANGELOG for pre-releases**: a dated section per pre-release, and 0.4.0's section
   saying what changed since 0.3.0 in full (leaning), or one growing section.
2. **Where the plumbing lands**: in this round (leaning) or pulled forward as a small
   commit earlier, since it does not depend on 029–031.

### Not doing

- crates.io, Scoop, winget, binstall for a pre-release (owner, 2026-11-13).

## Do

## Check

## Act
