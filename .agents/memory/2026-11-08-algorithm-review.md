# Algorithm review of 0.2.x: what was fixed, what is left

**Date**: 2026-11-08
**Agent**: Claude Code
**Confidence**: High for the fixed items (tests); Medium for the open ones (read from the
code or from other tools' documentation, not run)
**Status**: New
**Source**: a research pass the owner asked for ("deep research về các thuật toán dùng
cho app"), five threads: an inventory of xdevcmd's own code, trash restore and delete
UX, sort/match/search/sizes, compare/copy/rename/archives, watching/rendering/previews.
The full report and notes are kept outside the repository.
**Review-by**: 2027-03-01

## Problem

Which of xdevcmd's algorithms are weak next to other file managers (Total Commander,
Double Commander, yazi, nnn, FreeFileSync, rsync, ripgrep, fzf…), and is anything unsafe?

## Finding

Fixed on `feat/v0.2.1` (Round 013), each with a test:

- **Data loss**: a move across devices deleted files skipped at the overwrite question
  (confirmed by a real run before the fix; 0.2.0 has it). Now each source goes only
  once its own copy is complete.
- Overwrites truncated in place (cancel lost old and new): now a temporary name, then
  a rename. Extraction the same, and it no longer writes through a link at the target
  name. Bomb and free-space limits before unpacking.
- One "yes to all" flag answered every kind of question; now one per kind. Sync
  replaces only the planned files.
- Shift+F8 confirmed on Enter; now only Y or a click.
- Linux trash: `.trashinfo` written first (spec), a link's own path recorded,
  per-volume `.Trash-$uid`. Undo of a delete to the trash on all three systems
  (`trashed.rs`; macOS via `NSFileManager.trashItem`'s resulting URL).
- Alt+Y's compare and picture decoding moved off the drawing thread; directory sizes
  use a bounded pool, count hard links once, stay on one file system.
- Directory watching with `notify` (non-recursive, shown directories only), polling
  kept as the fallback; a directory is looked at once when its watch begins.
- robocopy-style "replace only if older" for F5/F6; a fuzzy Ctrl+F filter.

Open, by priority (none started):

1. **Sorting by Unicode**: names are compared with ASCII-only case folding, so
   accented and Vietnamese initials sort after `z`. Option: a lowercase key computed
   once per name (cheap) plus a tie-break on raw bytes; `icu_collator` is 40× slower
   than byte order on 100,000 names (measured 196 ms vs 4.8 ms).
2. **Image protocols**: `ratatui-image` (MIT) speaks Kitty, iTerm2 and sixel with half
   blocks as the fallback: sharp pictures on kitty, Ghostty, iTerm2, WezTerm and
   Windows Terminal; Terminal.app has none. Needs care with ratatui's diffing and the
   terminal panel.
3. **Faster copies**: `std::fs::copy`-style fast paths (copy_file_range, reflink via
   `reflink-copy`, clonefile, CopyFile2) instead of the 256 KiB read/write loop; keep
   progress and cancel.
4. **Memory per entry** (~640 B; nnn-style compact names and index views would be
   ~50 B plus the name).
5. Find in parallel (`ignore` crate), content compare by hash (size first, then
   BLAKE3/xxh3), Windows reserved names in rename, undo of swap renames, an undo
   journal for rename many, git status with a timeout.
6. Unconfirmed from reading: a tar.gz is decompressed up to three times per
   extraction; any shell's output (even a hidden tab) redraws the whole screen.

## Evidence

- Commits on `feat/v0.2.1` from 147e75a to 2ebd54c; Round 013's Do section.
- Tests: `a_move_across_devices_keeps_what_was_skipped`,
  `a_replacement_cancelled_part_way_keeps_the_old_file`,
  `overwriting_a_link_in_the_destination_replaces_the_link_not_its_target`,
  `what_went_to_the_trash_comes_back_and_only_while_it_can`,
  `ctrl_z_puts_back_what_went_to_the_trash`, `enter_never_confirms_a_permanent_delete`,
  `a_file_made_by_another_program_shows_up_at_once` (pty),
  `update_replaces_only_what_is_older_and_skips_the_same_and_newer`,
  `the_filter_is_fuzzy_by_default_and_plain_by_setting`.

## Recommendation

**Do**: take the open list in order, one round each; test any change to the copy/move
engine across two file systems (`/dev/shm` on Linux), not only within one.
**Don't**: judge a safety rule alone: the data-loss bug came from two correct parts
meeting ("skipped" was not "failed").

## Promotion candidate?

- [ ] `reference/security.md` — "test the engine across file systems", once a person
      agrees
- [x] Not yet
