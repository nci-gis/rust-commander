# Red team: same-drive move over a file goes to the trash first (feat/a10-qa-p0-move-trash)

Commits: a33dbfe (red render test) and 1a3045d (fix). Everything was checked at 1a3045d in a copy
(/tmp/rt-move). Note: partway through this review, /home/claude/rust-commander's working tree
was switched to 3fd68b8, which has no `move_over`. The review checked out 1a3045d in the copy
and used it for every run below.
Probe tests (not in the repo) were added to the copy's ops.rs tests module.

## Verdict: SIGN WITH AMENDMENTS. No P0 or P1 found.

Nothing below loses a file for good where the old code would not have. Every failure path
leaves the old file either back at dst or in the trash.

## 1. New data-loss paths?

| Case | Result | Label |
|---|---|---|
| Rename fails after the trash | The old file is restored to dst and src is kept (author's test, plus the probes below) | REPRODUCED (ops.rs:2869-2877) |
| Rename fails and the restore fails too | `let _ = restore(&t)` (ops.rs:2875) ignores the failure. The old file stays in the trash, and the user sees only the rename error, e.g. "File exists". Probe `rt_swap_after_trash`: Swap at Point::Rename puts another program's file at dst, and rename_noreplace refuses it (EEXIST). Their file is kept, src is kept, and "old" is in the trash, but nothing says so. Not lost, but **P2**: say "the old file is in the trash". | REPRODUCED |
| Trash on another volume / network / Windows non-system drive | `replace_into_trash` is false, so it falls back to `fs::rename` (ops.rs:2856-2857): replaced for good, as before. `replaced_for_good` is not counted for a move (copy counts it at ops.rs:2113), so the summary and undo say nothing. The table marks it ⚠ (docs/guide.md:218). **P2**, existed before this change. | READ |
| Trash refuses with EXDEV (e.g. a bind mount where st_dev matches) | move_over wraps the error with a new io::Error (ops.rs:2862-2867), which drops raw_os_error, so it is not taken for cross-device. Nothing is moved and the move fails, where before it replaced. A safe refusal, but it names no next step. **P2/P3** | READ |
| dst is a symlink | The link itself goes to the trash; its target is untouched (`rt_symlink_dst`) | REPRODUCED |
| dst is a dir (file over a dir) | Refused by other_kind (ops.rs:2985) before move_over runs | READ |
| src dir onto a file | Skipped: "is a file — a folder never replaces it" (`rt_dir_onto_file`) | REPRODUCED |
| src dir onto a non-empty dir | `fs::rename` gives ENOTEMPTY and the move fails (`rt_dir_onto_dir`). This is not the "merge" the commit message describes. It behaved the same before the fix and lost no data. **P3**: wrong claim in the commit/doc comment (ops.rs:2853) | REPRODUCED |
| rename_noreplace fallback (no renameat2) | hard_link, else check-then-rename (noreplace.rs:32-52). dst was just trashed, so the check passes. No new risk | READ |
| EXDEV fallback | Trash, then rename gives EXDEV, then restore (ops.rs:2875), and only then copy_node (ops.rs:2834). copy_node trashes it again by its own logic. Result: exactly one trash entry and one .trashinfo, replaced_to_trash = 1, dst = new (`rt_exdev_fallback_yes`). With an answered conflict (not replace-all) there is no false "changed" re-ask, because ident is kept across the same-volume rename (`rt_exdev_answer_yes_no_false_reask`, asked = 2). If the cross-device question gets Skip, the old file is back at dst, src is kept and the trash is empty (`rt_exdev_fallback_skip`). | REPRODUCED (Linux) |
| EXDEV on macOS / Windows | Every replaced file goes to the Trash or Recycle Bin and back before the copy. A restore from a protected ~/.Trash (trashed.rs:80-87) or from the Recycle Bin by date lookup can fail. Then the old file stays in the trash and copy_node sees dst free. Not lost, but wasteful, and the old file is not counted as replaced. **P2**: decide whether it is cross-device before trashing (compare st_dev / volume). | SUSPECT (not run on those OSes) |
| src vanished after the question | dst is sent to the trash, then rename gives ENOENT, then it is restored (trace: Trash, then Rename). dst ends intact (`rt_src_vanished`). The user's only copy makes a needless round trip through the trash, safe only while the restore works. **P3**: check src exists before trashing. | REPRODUCED |

## 2. Kill between the trash and the rename

The journal is committed by `durable(&dst)` (ops.rs:2784) before move_over. It records no "trashed" fact.
On the next start, resume keeps the sources that are still there (jobs.rs:364) and runs a move with Existing::Resume.
dst is now free, so resolve_conflict returns Some(true) early (ops.rs:1644-1646) and rename_noreplace moves it.
No question, no second trash, nothing lost (`rt_resume_after_crash_between_trash_and_rename`: asked = 0, dst = new, trash = [old]).
Gaps, **P3**: the resumed summary does not count it as replaced into the trash, and a declined resume leaves dst missing with the old file in the trash and no word about it. REPRODUCED (engine-level simulation of the post-kill state; no real kill was run).

## 3. Undo (Ctrl+Z)

Undo::Move::plan (undo.rs:147-177) checks only that the source name is free and the target is there. It moves the new file back and does not refuse.
The old file stays in the trash, which matches docs/guide.md:215-216 ("restore it from the trash by hand after a move"). READ.
The render test `ctrl_z_undoes_a_rename_a_new_directory_and_a_move_after_asking` passes. REPRODUCED, but it does not cover a replacing move.
**P3**: after undo, nothing tells the user that the replaced file is in the trash. The copy has a similar note for for_good (jobs.rs:530-534).

## 4. Resume / still_agreed

move_over is reached only after still_agreed (ops.rs:2786). Resume's Same::Yes "repaired" path and Expect::Replace both set `agreed`, so it is trashed once.
In the EXDEV path the trash happens twice in time, but the first is restored before copy_node, so there is one net entry. REPRODUCED (`rt_exdev_*`).
After a crash the dst is free, so no trash is skipped or doubled. REPRODUCED.

## 5. Are the tests real?

- Engine test `a_move_over_a_file_trashes_the_old_one_and_a_failed_rename_puts_it_back` (ops.rs:3939): with `self.move_over` reverted to `fs::rename` in the copy, it fails (`(1,0) != (1,1)`). It checks the disk: dst contents, src gone, and trash contents. The fault half checks that the restore happened. REPRODUCED
- Render test `a_file_a_move_replaces_goes_to_the_trash` passes at 1a3045d and checks the real trash directory under the sandbox. It was committed red in a33dbfe; that failure was not re-run here (the engine-level revert covers the same path). READ/REPRODUCED (pass)
- Missing from the branch's tests: EXDEV fallback, restore-fails, src-vanished, crash/resume and undo after a replacing move. The probes above cover them; it is worth adding the EXDEV and swap probes to the suite.

## Amendments suggested (none blocking)
1. P2: when the restore fails, report "the old file is in the trash" (ops.rs:2875).
2. P2: count `replaced_for_good` for a move that replaces with no trash (ops.rs:2857), so the summary and undo say so.
3. P2: decide whether a move is cross-device before trashing, to avoid a Trash/Recycle Bin round trip and a restore that can fail on macOS/Windows.
4. P3: check that src exists before trashing; fix the "a folder onto a folder is a merge" claim (ops.rs:2853).
