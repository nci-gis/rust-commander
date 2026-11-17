# Audit: delete, trash, undo, protected folders, rename, read-only

Scope: docs as oracle; tests read (READ, file:line). Nothing run. Paths relative to repo root.
R = crates/xdevcmd/tests/render.rs, P = crates/xdevcmd/tests/pty.rs, U = crates/xdevcmd-core/src/undo.rs (tests from :366),
O = crates/xdevcmd-core/src/ops.rs (tests from :3533), PR = crates/xdevcmd-core/src/protect.rs (tests :194).

Verdicts: PROVES (user path + disk + fails on bug), CONFIRMS (inner function or screen text only), MISSING, MANUAL (cannot be automated here / other OS).

| # | promise | doc:line | tests | verdict | why (when not PROVES) |
|---|---|---|---|---|---|
| 1 | F8 moves the selection to the trash (recoverable) | guide.md:15,135; keys.md:69; README.md:90; config.md:344 | R:269 ctrl_z_puts_back_what_went_to_the_trash; R:246 f8_goes_to_the_trash_and_shift_f8_says_it_cannot_be_undone; O:6002 trash_moves_the_file_and_records_how_to_restore_it | PROVES (Linux) | R:269 checks file gone then back on disk; does not look inside data/Trash or compare bytes. macOS/Windows trash: MANUAL |
| 2 | F8 asks first; Esc/cancel deletes nothing | guide.md:140; config.md:52 | R:1848 the_key_bar_and_dialog_buttons_are_clickable (cancel click, alpha.txt exists) | PROVES | |
| 3 | Shift+F8 deletes for good, not via the trash | guide.md:135; config.md:53; keys.md:70 | P:1907 shift_f8_warns_then_deletes_for_real (file gone, no data/Trash) | PROVES | |
| 4 | Shift+F8 only on Y, never Enter | guide.md:135; security.md:29 | R:353 enter_never_confirms_a_permanent_delete; P:1907 | PROVES | |
| 5 | Alt+X F8 = Shift+F8 (asked) | guide.md:379; troubleshooting.md:73 | R:362 alt_x_then_an_f_key_does_what_shift_and_that_key_does | CONFIRMS | screen text + file exists, but Y never pressed: a build where Alt+X F8 opened the trash dialog with the same words would pass only if text matches; deletion path not driven |
| 6 | `delete_to_trash = false`: F8 deletes for good; Shift+F8 always for good | config.md:53 | R:3473/3557/3646 only check the setting is written | MISSING | no test runs F8 with the setting off and checks disk/trash |
| 7 | A file replaced (copy/move/sync) goes to the trash on the same drive; Ctrl+Z brings it back | guide.md:215-217; security.md:154-159 | R:5478 a_replaced_file_goes_to_the_trash_and_ctrl_z_brings_it_back; O:3817 a_replaced_file_goes_to_the_trash_first_and_undo_brings_it_back | PROVES (Linux, F5) | F6/Alt+Y replace not driven; macOS any local drive, Windows system drive only: MANUAL |
| 8 | Elsewhere and on network shares a replace is for good, table marks it ⚠ | guide.md:218 | crates/xdevcmd-core/src/plan.rs:1701 a_replace_into_the_trash_is_information_not_a_loss | CONFIRMS | unit of the plan; no user-path test of the ⚠ when no trash is reachable |
| 9 | Table says ℹ "a file replaced goes to the trash"; summary counts them | guide.md:218-219 | R:5478 (text "1 replaced (the old one in the trash)") | CONFIRMS | screen text |
| 10 | Kill between trash and rename: next start renames the new file in, says so; old in the trash | security.md:157-159 | P:1447 a_replace_killed_after_the_trash_is_finished_at_the_next_start; crates/xdevcmd-core/src/journal.rs:1418 | PROVES (Linux) | |
| 11 | A failed rename after the trash puts the old file back | security.md:157 | O:3878 a_failed_rename_after_the_trash_puts_the_old_file_back | CONFIRMS | fault-injected unit, no user path |
| 12 | Ctrl+Z undoes a rename (F2) after showing what it will do | guide.md:223-224; keys.md:73 | R:3693 ctrl_z_undoes_a_rename_a_new_directory_and_a_move_after_asking | PROVES | |
| 13 | Ctrl+Z undoes rename-many (Shift+F2) | guide.md:223 ("one or many") | none | MISSING | U:378 covers Undo::Rename with one pair only |
| 14 | Ctrl+Z undoes a move | guide.md:16,223 | R:3693 | PROVES | |
| 15 | Ctrl+Z undoes a copy (removes what it made, source stays) | guide.md:16,223,228 | R:3693 (disk), R:3722 (text) | PROVES | |
| 16 | Ctrl+Z undoes an Alt+Y sync | guide.md:223 | none | MISSING | no test drives Alt+Y then Ctrl+Z |
| 17 | Ctrl+Z undoes a new directory (F7) | guide.md:223; keys.md:73 | R:3693 | PROVES | |
| 18 | Ctrl+Z undoes a new file (Shift+F4) | guide.md:223; keys.md:73 | U:484 a_new_file_goes_only_while_it_is_empty | CONFIRMS | unit only; no Shift+F4,Ctrl+Z key test |
| 19 | Ctrl+Z undoes F8 (puts things back) | guide.md:16,224; how-to-test.md:45 | R:269 | PROVES (Linux) | |
| 20 | Undo of a trashed item whose old name is taken: refused, nothing replaced | guide.md:226-227 | O:6035 what_went_to_the_trash_comes_back_and_only_while_it_can | CONFIRMS | unit of trashed::restore |
| 21 | Undo checks disk first: a file renamed since / name taken again is left alone | guide.md:226-227 | U:378 a_rename_turns_round_while_the_disk_still_says_so | CONFIRMS | unit of plan(); no key path |
| 22 | Undo: a new directory with something in it now is left alone | guide.md:226-227 | U:464 a_new_directory_goes_only_while_it_holds_nothing_but_directories | CONFIRMS + CONTRADICTION | test asserts a directory holding only (empty) sub-directories IS removed; doc says "something in it" is left alone (see Contradictions) |
| 23 | Undo of a move asks before overwriting what appeared meanwhile | undocumented (TC/mc expectation; security.md:34 "nothing is replaced without asking") | U:423 undoing_a_move_asks_before_overwriting_what_appeared_meanwhile | CONFIRMS | unit |
| 24 | Undo of a copy keeps files changed since / folders with new content, and the job lists them | guide.md:228-230; security.md:160-162 | U:568 undoing_a_copy_keeps_what_changed_since; U:601 a_copy_with_nothing_left_as_made_says_so | CONFIRMS | real copy job + unmake in core, but not through keys; "job lists it" not checked on screen |
| 25 | Undo of a copy brings replaced files back from the trash | guide.md:229-230; security.md:161 | R:5478 | PROVES (Linux) | |
| 26 | A copy of more than 200,000 items is not undoable; Ctrl+Z says so | guide.md:231 | U:496 what_cannot_be_undone_says_so (generic Cannot("a copy") message); P:3084 only a comment | MISSING | no test crosses the 200,000 cap; the generic message unit does not exercise the cap |
| 27 | Shift+F8 is not undoable; Ctrl+Z says so | guide.md:231 | R:3722 ctrl_z_says_what_cannot_be_undone_and_refuses_what_changed | CONFIRMS | screen text only (file is gone either way) — acceptable |
| 28 | A new run starts with nothing to undo | undocumented | R:269, R:3722 | CONFIRMS | text |
| 29 | Kept lists follow F6 / F2 / Shift+F2 / Ctrl+Z | guide.md:232-233 | R:3963 a_row_moved_from_a_results_tab_follows_it (F6); R:3992 (F2); crates/xdevcmd/src/lists.rs:334 rows_follow_what_was_moved_or_renamed | CONFIRMS | F6 and F2 driven (screen); Shift+F2 and Ctrl+Z not driven |
| 30 | Root, home, every folder a pane shows, a shell's folder, and folders above are never deleted/moved/renamed | security.md:36 | PR:209 a_guarded_folder_is_refused_however_it_is_written; P:5000 a_list_of_root_home_and_dots_offers_nothing_to_delete | CONFIRMS | unit of protected() + list path screen text (F8 refused, Enter never pressed). No key test from a normal pane: F8/Shift+F8/F6/F2 on the directory the other pane shows, on a shell's cwd, on an ancestor via Ctrl+\ |
| 31 | ... however written: `src/..` | security.md:36 | P:4953 a_listed_row_naming_a_folder_above_is_never_deleted; PR:209 | PROVES (list path) | disk checked after Shift+F8,y; but the fix makes `src/..` not a row, so the guard itself is not reached by P:4953 |
| 32 | ... however written: `link/` (trailing slash through a link) | security.md:36 | PR:250 a_link_to_a_guarded_folder_is_refused; O:5974 a_delete_never_goes_through_a_link_by_its_trailing_slash | CONFIRMS | units; no key path can produce `link/` except lists — none driven |
| 33 | The engine checks again (guard re-checked inside the job) | security.md:36 | O:5974 (link/ only) | MISSING | no engine-level test that a delete/move request for home/root/a pane folder is refused |
| 34 | Guards also cover move and rename (not only delete) | security.md:36 | none | MISSING | all user-path guard tests use F8/Shift+F8 |
| 35 | Writes wait while a pane is still opening a folder | security.md:36 | P:5106 a_write_action_while_a_folder_opens_waits_for_it | CONFIRMS | F8 only; checks "still opening" text and no dialog, never confirms; F5/F6/F2/Shift+F8/F7 while opening not tested |
| 36 | Deleting from a list names the list in the confirmation | security.md:291-292 | none | MISSING | |
| 37 | A mark hidden by the filter is dropped, so F8 cannot reach it | undocumented (guide.md:304 filter) | R:3194 a_mark_the_filter_hides_is_dropped_so_f8_cannot_reach_it | CONFIRMS | dialog text; never confirmed, no disk check |
| 38 | Shift+F8 is not covered by safe operations (not resumed) | security.md:56-57 | P:1836 an_interrupted_permanent_delete_is_not_offered_again | CONFIRMS | hand-written journal, not a killed real Shift+F8; disk checked |
| 39 | A delete continues with what is left (after a kill) | security.md:78-79 | none | MISSING | see Contradictions (P:1836 asserts a `kind delete` journal is NOT offered) |
| 40 | F2 / Shift+F6 rename; name part selected, extension kept | guide.md:132; keys.md:67 | R:2188, R:2204 typing_replaces_the_name_keeps_the_extension_and_follows_the_file; P:2315 shift_f6_and_f2_rename_through_real_key_bytes | PROVES | |
| 41 | Rename onto an existing name is refused, nothing replaced | security.md:34; guide.md:140 | R:2217 an_existing_name_is_refused_and_the_dialog_stays_open | PROVES | |
| 42 | Rename refused by the system if the name was taken a moment earlier (race) | security.md:34 | crates/xdevcmd-core/src/noreplace.rs:68,108 | CONFIRMS | unit of the no-replace rename; race not drivable |
| 43 | New names may not hold `/` (rename not turned into a move) | guide.md:296-297 | R:2233 a_name_with_a_slash_is_refused_not_turned_into_a_move | PROVES | |
| 44 | New names may not hold a control character (rename, mkdir, new file, copy as) | guide.md:296-297 | crates/xdevcmd-core/src/rename.rs:226; crates/xdevcmd/src/app.rs:1219 mkdir_refuses_a_control_character; R:895 (viewer save-as) | CONFIRMS | units; cannot type a control char through `--keys` for F2/Shift+F4/Shift+F5 |
| 45 | On Windows not `< > : " \| ? * \`, no trailing dot/space | guide.md:297 | crates/xdevcmd-core/src/plan.rs:1721 windows_names_are_refused_by_their_rules (copy plan) | MISSING | covers copy destinations, not new names typed into F2/F7/Shift+F4; windows_e2e.rs has no rename test; MANUAL on Windows |
| 46 | Shift+F4 on an existing name does not empty it | undocumented (guide.md:140) | R:4185 shift_f5_copies_beside_itself_and_shift_f4_creates_a_file | PROVES | |
| 47 | Shift+F2: every new name shows before Enter; Enter renames | guide.md:237-238 | R:4018 shift_f2_renames_with_a_pattern_and_shows_it_first | PROVES | |
| 48 | Shift+F2: nothing renamed if one name is wrong (two files one name) | guide.md:238 | R:4043 a_pattern_that_gives_two_files_one_name_renames_nothing; crates/xdevcmd-core/src/batch_rename.rs:307 | PROVES | |
| 49 | Shift+F2: a name that would replace another (unselected) file is refused | security.md:34 | batch_rename.rs:315 a_name_that_would_replace_another_file_is_refused | CONFIRMS | unit only |
| 50 | Pattern tokens [N] [N2-5] [N3-] [E] [P] [C] [C10+5:3] [[, Find/Replace, Case | guide.md:240-251 | crates/xdevcmd-core/src/name_mask.rs:297-360; R:4018 ([C], extension case) | CONFIRMS | units; [P] not seen in the listed unit names |
| 51 | The counter follows the pane's order | guide.md:251 | R:4018 (two files in name order only) | CONFIRMS | not tested with a non-name sort |
| 52 | F4 in Shift+F2: names in the editor, xdevcmd asks before renaming; swaps work | guide.md:253-254 | P:3350 shift_f2_renames_what_the_editor_changed_after_asking | PROVES | |
| 53 | Editor must wait (Windows `code --wait`) | guide.md:254-255 | none | MANUAL | Windows |
| 54 | Rename many: killed run's files put back under their names | security.md:55-56,61-62 | crates/xdevcmd-core/src/journal.rs:1199 a_dead_runs_parts_are_removed_and_renames_put_back | CONFIRMS | unit; no kill-and-restart pty test for Shift+F2 |
| 55 | Rename many without its journal renames nothing (with "rename anyway") | security.md:113-114 | batch_rename.rs:246, :352 | CONFIRMS | unit; "rename anyway" not driven |
| 56 | `--read-only`: copy, move, delete, rename, edit off; nothing on disk changes | security.md:238,245; README.md:107 | R:2866 read_only_turns_every_write_off_and_says_so; P:3231 read_only_mode_changes_nothing_on_disk_and_saves_no_session | PROVES | |
| 57 | Read-only: new file, copy as, link, permissions off | security.md:245 | R:4231 the_new_writes_are_off_in_read_only_mode | PROVES | |
| 58 | Read-only: Shift+F2, Alt+Y, Ctrl+Z off | security.md:245 | R:4158 bulk_rename_is_off_in_read_only_mode; R:4576 synchronising_is_off_in_read_only_mode; R:3722 (undo) | CONFIRMS | text only; no disk check |
| 59 | Read-only: menu and mouse paths also refused | security.md:245 | R:4804 what_the_mode_turns_off_is_refused_from_the_menu_too; R:2903 read_only_also_blocks_the_mouse_paths | CONFIRMS | disk checked but dialog never confirmed |
| 60 | `--restricted`: also no copy/move/delete/rename | security.md:239,245 | R:2926 restricted_has_no_shell_and_no_way_to_start_one (shell only) | MISSING | no write-key test under --restricted |
| 61 | A command-line flag can make the config stricter, never looser (`mode = "read_only"` in config) | security.md:251-252 | R:3073 (root only) | MISSING | no test of `[security] mode = "read_only"` vs flags for writes |

## Contradictions (doc vs test)

1. guide.md:226-227 says undo leaves alone "a new directory with something in it now"; U:464 asserts a directory that holds only sub-directories (`a/b/c`) IS removed by undo. Either the doc should say "with a file in it", or user-made empty folders inside an F7 folder are removed silently.
2. keys.md:73 says Ctrl+z undoes "the last rename, move, new directory or new file"; guide.md:16/223-224 and R:3693 (copy undo), R:269 (trash undo), R:5478 add copy, F8 and sync. keys.md is stale (it is also the list F1 users read).
3. guide.md:16 lists copy, move, rename, delete-to-trash only; guide.md:223 adds Alt+Y sync, new directory, new file. Not a test contradiction; doc-internal.
4. security.md:78-79 "A delete continues with what is left" vs P:1836 asserting an interrupted `kind delete` journal is not offered again ("Continue it?" absent). Consistent only if "delete" at :79 means F8-to-trash and the journal `kind delete` is Shift+F8; nothing tests the F8 resume, so the doc line has no test behind it.

## Counts


PROVES 22 · CONFIRMS 27 · MISSING 11 · MANUAL 1 (61 promises; macOS/Windows halves of rows 1, 7 are MANUAL too)

## Five most important gaps (data-loss risk first)

1. Protected folders from a normal pane (rows 30, 33, 34): no key-driven test deletes/moves/renames the folder the other pane shows, the shell's cwd, an ancestor or home and then checks the disk; the only user-path tests go through shell lists and press F8 without confirming. "The engine checks again" has no test except `link/`.
2. Writes while a folder is opening (row 35): only F8, only up to the refusal text; Shift+F8,y / F6 / F2 while opening (the #4 bug class: acting on the old listing's cursor) are untested on disk.
3. `delete_to_trash = false` (row 6): F8 then deletes for good; no test checks that path still asks, or that Shift+F8 vs F8 semantics hold with the setting off.
4. Undo of rename-many and Alt+Y sync (rows 13, 16) and the 200,000-item cap (row 26): undo of the multi-file operations moves many files back; untested through keys, and the cap (what Ctrl+Z says past it, and that it does not half-undo) is untested.
5. Undo removing a new directory that now holds sub-directories (row 22, contradiction 1) and undo-of-copy keep rules only at unit level (row 24): user content inside undone folders is the remaining delete path not proven end to end.
