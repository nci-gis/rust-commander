# Round 009: Undo the last operation

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), item 9
**Date started**: 2026-11-07
**Date completed**:

## Goal

Ctrl+Z undoes the last operation where that loses nothing, says plainly where it
cannot, and never undoes something older behind the user's back.

## Plan

- [x] 1. `rcmd_core::undo`: what the last operation was (`Undo`), and what undoing it
      would do now (`plan`), decided against the disk at that moment.
- [x] 2. Undoable: rename (one or many) — turned round, while each new name is still
      there and each old one free; move — a move job back into the directory they came
      from, for what is still where it went (several source directories: refused);
      new directory — removed while it holds nothing but directories; new file —
      removed while empty.
- [x] 3. Not undone, and said so: copy, delete (to the trash or for good), extract,
      pack, synchronise, permissions, link.
- [x] 4. Ctrl+Z asks first, listing what will happen; refused in read-only and
      restricted modes; one step (an undo is not undone).
- [x] 5. Docs, CHANGELOG.

## Do

**2026-11-07**, on `feat/v0.2.0`.

- Every place a write starts records it (`App::remember_job` for jobs; rename, rename
  many, F7 and Shift+F4 directly), so "the last operation" is really the last.
- A move is undone through the same job engine as F6 (progress, overwrite questions,
  across drives), not a bare rename.
- F7 can make several levels (`a/b/c`): the first level that did not exist is what
  undo removes (`first_missing`), and only while the tree holds no file or link.
- `undo::remove` checks again, refuses in read-only mode and outside `--root`.
- Not done: restoring from the trash. The Linux trash rcmd writes could be read back,
  but Windows and macOS keep their own, and a restore that works on one system only
  would surprise; the message points at the system's trash instead.

## Check

- [x] Unit: rename (still there / exists again / gone), move (moved, skipped, deleted
      since; several directories), new directory (empty tree, then a file in it, and
      `remove` refusing too), new file, a copy saying it cannot be undone.
- [x] Render, on real files: rename, F7 and F6 each undone; the question shown; nothing
      to undo; a copy after a rename says it cannot be undone; read-only refuses.
- [ ] Undoing a move that asks an overwrite question on the way back: the job asks as
      F6 does; not tested by itself.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/features.md`: undo, what it covers and why not copies or deletes
