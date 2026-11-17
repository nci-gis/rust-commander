# Audit: COPY, MOVE and RESUME (xdevcmd, before 0.4.0-beta.2)

Oracle: the user docs. Everything below is READ (file:line); nothing was run.
Paths: G = docs/guide.md, S = docs/security.md, K = docs/keys.md, CFG = docs/config.md, HT = docs/how-to-test.md,
R = crates/xdevcmd/tests/render.rs, P = crates/xdevcmd/tests/pty.rs (all `#![cfg(unix)]`),
O = crates/xdevcmd-core/src/ops.rs (tests), PL = …/plan.rs, J = …/journal.rs, C = …/compare.rs, U = …/undo.rs,
E = crates/xdevcmd/src/eta.rs, A = crates/xdevcmd/src/app.rs, T = xtask/src/tryout.rs,
M = crates/xdevcmd/tests/manual/test-cases/*.md (hand-run cases).

Verdicts: PROVES = key-driven (render/pty), checks disk, would fail on the bug. CONFIRMS = engine/unit test or screen text only.
MISSING = no test found. MANUAL = needs a real power cut, drive, OS, terminal or network mount.

| # | Promise | doc:line | Tests | Verdict | Why (if not PROVES) |
|---|---|---|---|---|---|
| 1 | F5 opens a dialog naming what and where; nothing is written while it is open | G:130, G:182; CFG:52 | R:223 f5_asks_before_copying_and_names_the_destination; R:5453 (b/older.txt unchanged while open) | PROVES | |
| 2 | A newer file at the destination is a ⚠ group, **skip** by default; Enter asks nothing and keeps it | G:183, G:193-195 | R:295 f5_can_replace_only_older_files; P:632 copying_over_a_file_skips_it_unless_told_otherwise | PROVES | |
| 3 | Table: one line per group, worst first (⛔, ⚠, ℹ) | G:192-194 | R:5453 a_copy_onto_files_that_are_there_shows_the_plan_once_per_group; P:1342 (Down from "newer" to "older") | CONFIRMS | Screen text; "worst first" only implied by Down navigation; no ⛔ row shown in any render/pty test |
| 4 | ⛔ cannot be done: left out with a reason (file over folder or the reverse, into itself) | G:184, G:192-193; S:37 | PL:1386 every_kind_of_destination_gets_its_class; PL:1494, PL:1508; O:3762 a_file_never_replaces_a_folder_nor_the_reverse; O:6167, O:6193 | CONFIRMS | Engine/plan only; no key-driven test shows the ⛔ row and checks the folder survives Enter |
| 5 | Names differing only in case on a case-ignoring drive: a ⚠ case clash | G:184-185, G:194; G:293-294 | PL:1838, PL:1958; C:712, C:736 | MANUAL | CI drives are case-sensitive (M plan-table M-PT-04); unit tests CONFIRM |
| 6 | Names or sizes the drive cannot take (FAT, Windows names) | G:185 | PL:1721 windows_names_are_refused_by_their_rules; PL:1838 a_fat_drive_refuses_bad_names_and_big_files… | MANUAL | Needs a FAT stick (M-PT-03); units CONFIRM |
| 7 | Free space checked; first line shows "space needed and free" | G:185, G:196 | PL:1902 free_space_counts_what_is_written_and_a_move_on_one_device_needs_none | CONFIRMS | Unit only; M-PT-05 manual. See contradiction C3 |
| 8 | A destination you cannot write to is said before the start | G:186 | PL:1939 | CONFIRMS | Unit only |
| 9 | Space turns a ⚠ group to replace, then ask each; the job does what the table says | G:195-196 | P:585 copying_over_a_file_asks_first_then_overwrites; R:5509 the_table_changes_a_group_and_the_copy_does_what_it_says | PROVES | |
| 10 | First line sums up create · replace · skip | G:196 | P:632 ("create 1 · replace 0 · skip 1"); R:5509 ("replace 1"); R:5453 ("create 1") | CONFIRMS | Screen text (counts match the disk outcome checked in the same tests) |
| 11 | Nothing to decide: the plain dialog with the U line | G:188 | R:5527 a_copy_with_nothing_to_decide_is_the_dialog_it_always_was | CONFIRMS | Screen text only; no copy run |
| 12 | U rules for a file that turns up during the job: ask each / only if newer (skip same) / leave all / replace all | G:188-190; S:169-170 | O:5666 update_replaces_only_what_is_older_and_skips_the_same_and_newer; O:6268 yes_to_all_stops_asking; PL:1991 | CONFIRMS | No test presses U in the dialog; "leave all" has no test |
| 13 | Identical files (size and date) are left, and counted | G:190-191, G:194; S:163 | R:5453 (screen "1 already there, identical"); O:4384 a_planned_job_leaves_identical_files_without_a_row | CONFIRMS | No key-driven test checks the identical file was not rewritten (date/inode) |
| 14 | Enter before the check ends starts the job, asking as it goes | G:197 | none (M-PT-02) | MISSING | Timing-dependent but automatable with the `delay` fault seam |
| 15 | Tab in the dialog checks again for the other pane | G:197-198 | none | MISSING | |
| 16 | A file is replaced only while still the one the check/plan saw; changed since, you are asked ("It changed since you answered") | G:199-200, G:316-317; S:149-153 | O:5503, O:5544, O:5583, O:5625 a_file_made_where_the_plan_saw_none_is_asked_about; PL:2026 | CONFIRMS | Engine only; no pty test changes the file between the table and Enter |
| 17 | At an overwrite question, U applies "only if newer" to the rest | G:200 | O:5756 answering_update_at_a_question_applies_it_to_the_rest | CONFIRMS | Engine; never driven by the U key |
| 18 | Never an overwrite without asking; Overwrite/Skip answered on disk | README:98; S:34 | P:585 (O overwrites); P:1750 (s keeps); O:4090; O:4121 | PROVES | |
| 19 | A file that appears while xdevcmd writes is asked about, not replaced | S:34 | O:4121 a_file_that_appears_while_copying_or_moving_is_asked_about_not_overwritten | CONFIRMS | Engine only (race needs the fault seam; exists) |
| 20 | Esc cancels; a replaced file stays the old one; a new half-copy leaves nothing | G:202, G:206-207; S:194-195 | O:5403 a_replacement_cancelled_part_way_keeps_the_old_file; O:6207; O:5798 | CONFIRMS | No pty test presses Esc in a running job box and checks disk (only F10 / lost terminal paths) |
| 21 | B sends the job to the background | G:202 | P:1096 (sends `b`, asserts only the quit); A:1093 | CONFIRMS | Background behaviour not asserted e2e |
| 22 | Time left: from the last seconds; nothing for 3 s or while nothing moves | G:203-204 | E:127, E:146, E:181, E:198, E:208 | CONFIRMS | Unit; M-RJ-01 manual |
| 23 | A replaced file goes to the trash (same drive), table says ℹ, summary counts it, Ctrl+Z brings it back | G:215-219; S:154-159 | R:5478 a_replaced_file_goes_to_the_trash_and_ctrl_z_brings_it_back; O:3817 | PROVES | Linux only; macOS any local drive / Windows system drive are MANUAL (M undo-trash) |
| 24 | Elsewhere and on network shares a replace is for good and the table marks it ⚠ | G:217-218 | PL:1667, PL:1688, PL:1701 | CONFIRMS | Plan classification only; real share MANUAL |
| 25 | A failed rename after the trash puts the old file back | S:157 | O:3878 | CONFIRMS | Engine with fault |
| 26 | Killed between trash and rename: next start renames the new file in, says "1 replacement finished" | S:157-159 | P:1447 a_replace_killed_after_the_trash_is_finished_at_the_next_start; J:1418 | PROVES | Real SIGKILL at a fault-seam halt; disk and trash checked |
| 27 | Undo a copy removes what it made and brings replaced files back from the trash | G:223, G:228-230; S:160-162 | R:5478; U:544 | PROVES | |
| 28 | Undo of a copy keeps a file changed since / a folder you put something into, and lists it | G:228-229 | U:568 undoing_a_copy_keeps_what_changed_since; U:601 | CONFIRMS | Unit only |
| 29 | A copy of more than 200,000 items is not undoable; Ctrl+Z says so | G:231 | U:496 what_cannot_be_undone_says_so (message only) | MISSING | The 200,000 threshold is never tested. See contradiction C2 |
| 30 | Undo of an Alt+Y sync | G:223 | none | MISSING | |
| 31 | Shift+F5 copies beside itself under a new name | G:131; K:78 | R:4185 shift_f5_copies_beside_itself_and_shift_f4_creates_a_file | PROVES | |
| 32 | Shift+F5 onto a taken name still asks | README:98 | O:6139 copy_as_puts_a_second_copy_beside_the_first_and_still_asks… | CONFIRMS | Engine only |
| 33 | Safe operations cover Shift+F5 and Alt+Y too (kill → clean-up → continue) | S:55 | none (all kill tests are F5/F6) | MISSING | No kill test for Alt+Y sync or Shift+F5 |
| 34 | Killed: no file is ever cut short under its real name | S:10, S:52; G:206-208 | P:1580 (big.bin absent after SIGKILL); P:1342 (b-fresh absent); T:777 | PROVES | |
| 35 | Next start removes the run's temporary files, says so, offers to continue; Enter does | S:61-62; G:208-209 | P:1218 an_interrupted_copy_is_tidied_up_and_resumed_at_the_next_start; J:1199 | PROVES | State is a hand-written journal + part file, not a real kill; start-up path and disk are real |
| 36 | Continue skips what is there and identical | S:64-65; G:209-210 | P:1218 (screen "1 copied"); O:5697 resuming_skips_what_is_identical_and_asks_about_the_rest | CONFIRMS | No assertion that the identical file was left untouched |
| 37 | Continue skips a folder the run finished, without looking (a file added since is not copied) | S:65-66 | P:1713 resume_does_not_walk_a_folder_the_run_before_finished; O:4413 | PROVES | |
| 38 | Table decisions survive a kill: replace/leave carried out, nothing asked again, finished sources not redone | S:81-84; G:209 | P:1342 a_planned_copy_killed_then_continued_keeps_every_decision; J:1446 | PROVES | Real SIGKILL |
| 39 | On continue, a file to be replaced that changed since is asked about | S:84 | none on resume (O:5503 is a live job) | MISSING | |
| 40 | Big file: synced every 256 MiB; next start "kept part way"; Continue goes on from it | S:67, S:86-89; HT:57-60 | P:1580 a_big_copy_killed_after_a_checkpoint_goes_on_from_there; O:4600; O:4299 | PROVES | Real SIGKILL; bytes and resumed_bytes checked |
| 41 | Checkpoint kept only while the source is the same file (size, date, identity); otherwise restarted | S:89-90 | O:4600 resume_goes_on_from_a_kept_checkpoint_while_the_source_is_unchanged | CONFIRMS | Engine only |
| 42 | "Discard them" removes the kept parts; Esc leaves them for the next start | S:90-91 | J:1260 a_checkpointed_part_is_kept_with_its_journal_until_discarded | CONFIRMS | No key-driven test of Discard / Esc |
| 43 | One sync per 256 MiB, none for smaller files | S:91 | O:4299 a_big_file_is_synced_at_its_checkpoints_and_a_small_one_is_not | CONFIRMS | Trace-level |
| 44 | Continue restarts a smaller cut-short file | S:67 | P:1218 (part "half" removed, b.txt = "b") | PROVES | |
| 45 | Continue asks about any other file already there; a file kept the first time is asked about again, not replaced as older | S:68-69; G:210 | P:1750 resume_from_app_does_not_replace_a_kept_newer_source_file | PROVES | |
| 46 | C (check): every copied file re-read against its source; what differs copied again | S:11, S:70-72; G:210-211; HT:62-64 | P:1274 c_at_the_interrupted_dialog…; P:1788 a_checked_resume_repairs_a_file_the_journal_called_done; A:1138; O:4447, O:4507, O:4544 | PROVES | Power-cut damage simulated by writing wrong bytes with right size/date |
| 47 | C includes finished folders | S:71 | none (P:1788 marks a file `done`, not a folder) | MISSING | |
| 48 | C sends the differing old copy to the trash | S:72 | none | MISSING | No test checks the trash after C |
| 49 | Known gap: a file answered Skip in the dead run is replaced by C if size and date match | S:72-73 | none | MISSING | Documented data-loss gap with no test pinning its scope |
| 50 | Esc at the Interrupted dialog leaves the job for the next start | S:74 | J:1479 a_job_offered_again_keeps_its_journal_until_answered | CONFIRMS | No key-driven Esc-then-restart test |
| 51 | Move across drives killed between copy and removal: both kept, said ("copied completely … both are kept") | S:76-78 | P:1510 a_move_killed_before_removing_its_source_says_both_are_there; J:1390; O:5728 | PROVES | Disk + start dialog; the post-Continue summary is engine-only (O:5728). Skips silently with no second FS |
| 52 | Move across drives removes a source only once its copy is on disk (data, folder, journal synced) | S:29, S:53, S:135-139 | O:5092, O:5287, O:5134, O:5212 | CONFIRMS | Order via fault trace; real power cut MANUAL (M-DS-05) |
| 53 | Move across drives: what you skipped stays | S:29 | O:4187 a_move_across_devices_keeps_what_was_skipped | CONFIRMS | No pty F6 cross-device with a skip |
| 54 | Move on one drive is a rename: everything stays | G:260 | O:5955 a_move_on_one_filesystem_removes_the_source | CONFIRMS | Only checks the source went; inode/attributes not checked |
| 55 | A journal per job; removed when the job ends | S:99-101; CFG:352 | P:1218 (journal dir empty after); J:1148 | PROVES | |
| 56 | Lock: another window's live job is left alone; a dead run is tidied | S:102-106 | J:991, J:1056, J:1072, J:1098, J:1121 | CONFIRMS | Two real windows never run together |
| 57 | A journal from another machine is left alone | S:106-108 | J:1011 | CONFIRMS | Shared NFS home MANUAL |
| 58 | Temp name `.name.xdevcmd-part-<run>-<n>`; a later run with the same pid keeps its files | S:95-98 | O:5837; O:5869; J:1034 | CONFIRMS | |
| 59 | Journal cannot be written: Stop (default, Esc) changes nothing; Continue without it, summary says so | S:109-111 | P:1141 a_job_without_its_journal_asks_first_and_says_so; O:4722; O:5784 | PROVES | |
| 60 | Journal failing part-way: asked once before the next irreversible step, never trusted again | S:112-113 | O:4667; O:5242; J:1361 | CONFIRMS | Engine only |
| 61 | A replaced file is synced before its rename; failed sync keeps the old one; Retry copies again | S:121-126 | O:4779; O:4991 | CONFIRMS | Trace order; real power cut MANUAL |
| 62 | Small replaced files synced in batches; a failed sync asked alone; thread probe | S:127-134 | O:4849, O:4881, O:4909, O:4939, O:4960 | CONFIRMS | Engine; speed claims are bench/manual |
| 63 | Power cut: replaced = old or complete new; new file may be short → C | S:53 | P:1788 (simulated); T `--tamper` | MANUAL | Real power cut cannot run in CI |
| 64 | Closing the terminal / SIGHUP / SIGTERM: jobs cancelled, temp files removed, session saved | G:212; S:196-197 | P:1043 a_lost_terminal_cancels_cleans_and_saves_even_before_the_signal (EIO path, signals swallowed); P:1009 (SIGHUP, no job) | CONFIRMS | No test sends SIGHUP/SIGTERM while a copy runs |
| 65 | A second signal ends it at once | S:198 | none | MISSING | |
| 66 | F10 with jobs: asks, cancels, waits for clean-up | S:199 | P:1096 quitting_with_a_copy_running_cancels_it_cleans_up_and_ends; A:1181 | PROVES | |
| 67 | F10: a job stuck after 5 s → asked again | S:199-200 | none | MISSING | Automatable with the delay seam |
| 68 | Fast copy: clone, else copy_file_range, else read/write; status says "(cloned)" / "(by the kernel)" | S:176-190; CFG:57 | O:3996 the_fast_copy_brings_the_file_whole_and_is_named…; O:4051 | CONFIRMS | Engine; clone on Btrfs/XFS/APFS is MANUAL (CI temp is ext4/tmpfs) |
| 69 | A step the drive refuses falls back where it stopped, no question; a real error is the copy's | S:186-188 | O:4051 (/proc file, fallback from the start) | CONFIRMS | Fallback mid-file never tested |
| 70 | A big file copied by clone or with safe ops off is restarted, not resumed | S:206-207 | none | MISSING | |
| 71 | Safe operations off: new file under its own name | S:216-217 | O:3714 / O:5869 (loops over safe on/off) | CONFIRMS | |
| 72 | A replacement needs room; filling the disk keeps the old file | S:209 | O:4256 a_replacement_that_fills_the_disk_keeps_the_old_file | CONFIRMS | |
| 73 | Copy keeps contents | G:264 | P:585, P:1580, R:4539, … | PROVES | |
| 74 | Copy keeps files' modification date | G:265 | none (P:1218 sets the date by hand, "a copy keeps the date") | MISSING | Resume's "identical" skip depends on it |
| 75 | Folders' dates not kept | G:265 | none | MISSING | |
| 76 | Unix mode bits kept (+x); read-only flag on Windows | G:266 | O:4065 a_copy_keeps_the_executable_bit; O:5178 a_read_only_file_moves_across_devices | CONFIRMS | Engine; Windows MANUAL |
| 77 | Owner and group: the copy is yours | G:267 | none | MANUAL | Needs another user/root |
| 78 | Access and creation dates set to now | G:268 | none | MISSING | |
| 79 | xattrs, ACLs, tags, streams not copied (APFS clone may carry xattrs) | G:269 | none | MISSING | Low risk; APFS part MANUAL |
| 80 | Symlinks copied as links (Linux, macOS); on Windows not copied and said | G:270 | O:5055 (read_link == "target") | CONFIRMS | Engine; Windows MANUAL |
| 81 | Hard links: each name becomes its own file | G:271 | none | MISSING | |
| 82 | Sparse files written out to full size | G:272 | P:1580 (sparse source, size and marks checked) | CONFIRMS | Allocation not checked |
| 83 | Network error that may pass: retried after 1, 2, 4, 8, 15 s with "network error — trying again in 4 s (3 of 5)", then Retry/Skip/Stop; Esc during a wait stops at once | G:100-102 | O:3644, O:3656, O:3677, O:3695 | CONFIRMS | Zero delays; the schedule and the message text never asserted; real mount MANUAL (M-DS-06) |
| 84 | Each new file synced before it gets its name on a network mount | G:103-105 | none (PL:1667 only detects network mounts) | MISSING | Testable with a fault seam treating a dir as network; real mount MANUAL |
| 85 | Hard NFS mount gone: Esc stops once the call returns | G:108-109 | none | MANUAL | M-DS-07 |
| 86 | Alt+K marks what the other side lacks or has older; then F5 copies it | G:305; K:111 | R:4493 alt_k_marks_what_the_other_side_lacks_and_f5_copies_it | CONFIRMS | Never checks only-a.txt arrived; asserts the older target is LEFT (see C1) |
| 87 | Alt+Y shows the plan, nothing until yes, then both trees agree; ≠ left alone | G:306, G:314; K:112 | R:4539 alt_y_shows_the_plan_then_makes_both_trees_agree | PROVES | |
| 88 | Alt+Y: a big tree counts on the status line; Esc stops | G:306 | C:523 a_deep_compare_can_be_stopped_and_counts_as_it_goes | CONFIRMS | Engine only |
| 89 | ⚠ unreadable folder: never taken for empty, counted in the title, nothing under it copied | G:312; S:201-202 | R:4520 a_directory_that_cannot_be_read_is_shown_in_the_plan…; C:575, C:610, C:700 | CONFIRMS | R:4520 checks the screen only; never presses Enter to verify nothing is copied/deleted |
| 90 | ? a name whose twin may be among what could not be read: never copied | G:313 | C:671 an_entry_that_cannot_be_looked_at_leaves_its_directory_unplanned | CONFIRMS | Engine only; the ? mark itself never shown in a test |
| 91 | Sync replaces only what its plan named; changed or new since → asked | G:316-317; S:151-153 | O:5470 a_sync_replaces_only_what_its_plan_named; O:5625 | CONFIRMS | Engine only |
| 92 | Names compared as stored bytes (non-UTF-8) | G:293 | C:640 | CONFIRMS | |
| 93 | "Identical" precision per drive (FAT 2 s, exFAT 10 ms, HFS+ 1 s) | S:163-167 | C:402 times_are_the_same_only_as_finely_as_the_coarser_keeps_them | CONFIRMS | Real FAT/exFAT MANUAL |
| 94 | Drag onto the other pane copies after asking | G:358 | P:1999 dragging_a_file_onto_the_other_pane_copies_it_after_asking | PROVES | |
| 95 | Copy with no second pane refuses (undocumented) | — | R:669 copying_with_no_second_pane_refuses_instead_of_guessing | CONFIRMS | Screen text only |
| 96 | FIFOs refused before opened | S:27 | O:6248 copying_a_fifo_fails_fast_instead_of_hanging | CONFIRMS | |
| 97 | Windows specifics (no folder sync, FAT rename window, look compares 3 fields, temp files visible, close button not caught) | S:218-229 | windows_e2e.rs:12 (shell only) | MANUAL | No copy/move test on Windows at all |
| 98 | The `xtask try` checks A/B/C pass | HT:51-67 | T:777, T:825, T:848, T:884, T:906 (sandbox make/check/tamper/clean only) | MANUAL | The kill flows are run by a person |

## Contradictions between docs and tests

- **C1 — Alt+K "then F5 copies it" (G:305; K:111 "mark what is newer or missing there")** vs R:4493, which asserts that after Alt+K, F5, Enter the marked
  `newer.txt` (the other side has an older copy) is **not** copied ("left, as the plan said"; the table's ⚠ group defaults to skip).
  A user who follows the doc gets the lacking files copied and the older ones silently left unless they press Space.
- **C2 — K:73 "Ctrl+z Undo the last rename, move, new directory or new file"** leaves out copy, Alt+Y sync and the trash; G:223-224 and R:5478 (undoes a copy) say otherwise.
- **C3 — G:185 says the dialog checks free space** without a platform limit; the manual case M-PT-05 (plan-table.md:34) says "Windows does not know free space yet". Not a test assertion; a doc-vs-doc gap.
- Minor: S:76-78 places "copied completely … both are kept" in the summary after Continue; P:1510 finds it in the start-up dialog ("Both are kept") and never presses Continue.
- Minor (test hygiene, READ P:1336-1341): a stray `#[cfg(unix)]` plus doc comment for the move test sits on `a_planned_copy_killed_then_continued_keeps_every_decision`. Harmless because P is `#![cfg(unix)]`, but the move test lost its doc comment.

## Counts

| Verdict | Rows |
|---|---|
| PROVES | 23 |
| CONFIRMS | 50 |
| MISSING | 18 |
| MANUAL | 7 |
| Total | 98 |

Rows 23, 40, 46, 51 PROVE on Linux; their macOS/Windows/real-hardware parts are also MANUAL (counted once, as PROVES). Many CONFIRMS rows also have a MANUAL part (power cut, real share, FAT, Windows) noted in Why.

## The 5 most important gaps (data-loss risk first)

1. **C's known gap and C's trash step are untested (rows 48–49, S:72-73).** After a power cut, C replaces a file the user answered Skip for. The doc calls it a known gap but no test pins its scope, and no test checks that the differing old copy really goes to the trash. That makes it a silent overwrite path with no guard.
2. **"Replace only if still the same file" is engine-only, and missing on resume (rows 16, 39, 91; S:84, S:149-153, G:316-317).** No key-driven test changes a destination between the table/plan and Enter, or between kill and Continue. A UI path that skips the re-check would pass every test.
3. **Network promises have no end-to-end test (rows 83–84; G:100-105).** Nothing tests the sync-before-name on network mounts. The retry schedule (1/2/4/8/15 s) and the message are never asserted, because the unit tests use zero delays. On NFS/SMB a failed write reported at close is exactly how a "done" file ends up short.
4. **Safe operations for Alt+Y sync and Shift+F5 are not tested under a kill (rows 30, 33; S:55).** Neither is undo of a sync (G:223). Every kill/resume test is F5 or F6, but sync replaces files on both sides.
5. **The modification date kept on copy is untested (row 74; G:265).** "Identical" skips on Continue, the table's ℹ group and Alt+K/Alt+Y all assume the copy keeps its source's date. The resume tests set the date by hand, so a regression would make every Continue/C re-ask or re-copy, or misclassify files as newer or older.
   Close runners-up: the ⛔ row is never shown end-to-end (row 4), and SIGHUP/SIGTERM with a copy running plus "second signal ends at once" are untested (rows 64–65).
