# Read-only mode still writes a temporary copy for F3 inside an archive

**Date**: 2026-11-07
**Agent**: Claude Code
**Confidence**: High
**Status**: Promoted
**Source**: documenting the files rcmd keeps (docs/config.md, "Files rcmd keeps")
**Review-by**: n/a

## Problem

`reference/security.md`, the secure-modes table, says of `read_only`: "session file
written on quit: **no** — rcmd writes nothing". Is that true of everything?

## Finding

Not quite. F3 on a file inside an archive extracts it to the system temporary
directory (`rcmd-view-<pid>-<n>/`) in every mode, read-only and restricted included,
and removes it when the viewer closes (and, since 2026-11-07, when rcmd ends). Nothing
of the user's is written; the session and bookmarks are not; but "writes nothing" is
wider than what happens.

## Evidence

- `crates/rcmd-tui/src/app.rs`: the archive-view path creates `rcmd-view-…` with no
  mode check; `impl Drop for App` removes it.
- Measured: `TMPDIR=… rcmd --read-only --screenshot … F3` inside `proj.zip` shows the
  file, and a watcher saw `rcmd-view-<pid>-0` appear during the run; it was gone after.

## Recommendation

**Do**: a person decides which is right, then either the table says "writes nothing of
yours; a file viewed inside an archive is copied to the temporary directory and
removed", or read-only views archive members from memory and the table stays as it is.
**Don't**: leave the table promising more than the code does.

## Promotion candidate?

- [x] `context/` or `reference/` — the wording of `reference/security.md`, by a person
- [ ] `skills/`
- [ ] Not yet
