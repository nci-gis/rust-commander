# Red-team: folder copied onto a destination link (branch feat/a10-qa-p0-link-dest, 3fd68b8 + ddca7e1)

Reviewer: independent red-team agent, 2026-11-17. Worked in a copy (/tmp/rt-link,
CARGO_TARGET_DIR=/tmp/rt-link-target); /home/claude/rust-commander was not touched.
Extra tests written in the copy only (ops.rs `rt_*`, plan.rs `rt_every_kind_of_link_*`);
they are not on the branch.

## Verdict

SIGN WITH AMENDMENTS. No P0 or P1 found. The fix closes the write-through on every path
that reaches `copy_node` with a folder source, and the test is real. Amendments are P2.

## 1. Every path into copy_node with a folder source

| Path | How it reaches the check | Result | Label |
|---|---|---|---|
| F5 (copy) | `run` -> `copy_into` (ops.rs:1206) -> `copy_node` -> check at ops.rs:1843-1858 | skipped, nothing outside | REPRODUCED (`rt_dest_link_kinds_copy`, render test) |
| F5 with "replace all" | same; the check comes before any rule | skipped | REPRODUCED (`rt_dest_link_kinds_copy`, second run) |
| Shift+F5 (copy as) | `copy_into` with `rename_to` (ops.rs:1206) -> same `copy_node` | same code path | READ |
| Drag and drop | `Cmd::Transfer` -> `Request::copy/mv` (app/input.rs:935-937) -> same worker | same code path | READ |
| F6, same volume, top-level name is a link | `move_into` -> `resolve_conflict` -> `other_kind` (ops.rs:2959-2965): a link is not a folder -> skipped before any rename | skipped, source kept | REPRODUCED (`solo` in `rt_cross_volume_move_keeps_skipped_source`) |
| F6 across volumes (copy fallback), link deeper in a merged folder | `move_into` EXDEV -> `copy_node` (ops.rs:2842) -> check | skipped; **before the fix this wrote through the link too** | REPRODUCED (test fails at the "through the link" assert on the parent commit) |
| Alt+Y sync | `compare::entries` uses `symlink_metadata` (compare.rs:158-161); folder vs link = `Conflict`, never in `compare::plan` (compare.rs:369-391); a forced pair `(a/d, b)` with `b/d` a link is skipped by the check | nothing written | REPRODUCED (`rt_sync_pair_onto_link`; pre-fix the forced pair wrote through) |
| Resume / Continue (`done_dirs`) | ops.rs:1835 `dst.is_dir()` follows the link, so a finished folder whose copy is now a link is skipped (silently); an unfinished one hits the new check | nothing written | REPRODUCED (`rt_resume_done_dir_is_now_a_link`) |
| Extract from archive | separate code: `archive::target` refuses a link in any non-last component (archive.rs:303-318); a dir entry whose own name is a link: `create_dir_all` is a no-op through it, and every file under it is refused by `target` | not written through | READ |

Regressions checked (none):
- Destination PANE is itself a link (`pane -> real`), F5 a folder into it, also a merge: copies into `real/d`. REPRODUCED (`rt_dest_pane_is_a_link`). `copy_into` joins the name to the pane path; `symlink_metadata` only refuses the last component.
- Link deeper in the SOURCE tree: copied as a link, nothing skipped. REPRODUCED (`rt_link_in_source_tree`).
- A FILE onto a link at the destination: replaced via temp name + rename, or `create_new` (ops.rs:2213-2218, 2276-2280); never opened through the link. READ (unchanged by this branch).
- Full suites with the fix: xdevcmd-core 347 passed; xdevcmd 270 + 131 + 217 (render) passed. REPRODUCED.

## 2. Move across volumes keeps the source

`rt_cross_volume_move_keeps_skipped_source` (source on /dev/shm, destination on /):
`top/ldir/x` still in the source, `top/` kept (its `remove_dir` fails, not empty: ops.rs:1902-1905),
`top/plain` moved and removed from the source, link target folder empty. REPRODUCED.
The skip returns at ops.rs:1857, before any child is queued in `removing`, so nothing under
the skipped folder can be removed. READ.

## 3. Plan and job agree

Plan: `Some(_) => Class::DirOverFile` for anything not a real folder (plan.rs:1087-1091; `There` lists
with entry metadata, no following). Job: `filter(|m| !m.is_dir())` on `symlink_metadata` (ops.rs:1849).

| Destination name | Plan (Copy and Move) | Job (copy) | Label |
|---|---|---|---|
| link to folder | DirOverFile | skipped, "is a link" | REPRODUCED |
| link to file | DirOverFile | skipped, "is a link" | REPRODUCED |
| dangling link | DirOverFile | skipped, nothing created at the target | REPRODUCED |
| link loop (`l -> l`) | DirOverFile | skipped | REPRODUCED |

Outcomes agree in all four. Wording does not fully agree (P2 below).

## 4. Is the test real?

Yes. With ops.rs reverted to the parent commit, `a_folder_copied_onto_a_link_to_a_folder_writes_nothing_through_it`
fails at render.rs:5563 (the read_dir of `outside`: `new.txt` was written through). With the fix
it passes. It drives the real keys (Ctrl+a, F5, Enter) and checks the disk, the link, the table
and the summary. REPRODUCED.
Gap: it covers only copy + link to a folder at the top level. Move across volumes, dangling,
link to a file, loop, nested merge are not covered on the branch (my `rt_*` tests cover them and
fail before the fix for the cross-volume move, nested copy, resume and sync cases).

## Findings

P0: none. P1: none.

- **P2 - the refusal names no next safe action.** "is a link - a folder is never copied through it"
  (ops.rs:1852). A real setup (dotfiles: `~/.config/nvim -> ~/dotfiles/nvim`, then F5 an `nvim`
  folder to merge it) used to merge and now is skipped. That is the intended rule, but the skill's
  rule is "a refusal must name the next safe action": e.g. "open the link and copy into it". READ.
- **P2 - same case, three wordings.** Plan table: "folder where a file is: skipped, never replaced"
  (plan.rs:242) for a link to a folder; copy job: "is a link ..." (ops.rs:1852); move (top-level,
  via `other_kind`) says "is a file - a folder never replaces it" (ops.rs:2963) for the same link.
  REPRODUCED (summary errors in `rt_cross_volume_move_keeps_skipped_source`).
- **P2 - tests on the branch cover one shape only** (see section 4). Suggest adding a core test for
  the cross-volume move (it wrote through before the fix) and dangling/file/loop. REPRODUCED.
- **P3 - resume skips a finished folder whose copy is now a link without a note.** ops.rs:1835 uses
  `dst.is_dir()` (follows links); counted as skipped, no reason in the summary. No write. REPRODUCED.
- **P3 - check-then-act window.** Between `symlink_metadata` (ops.rs:1843) and the children's writes,
  a folder replaced by a link would be followed. Pre-existing, needs a hostile concurrent writer. SUSPECT.
- **Note - Windows junctions.** Rust reports junctions as links (`is_dir()` false), so they should be
  skipped too. Not tested here (Linux only). SUSPECT.
