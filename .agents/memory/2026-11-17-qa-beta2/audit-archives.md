# Audit: archives and file names (xdevcmd, before 0.4.0-beta.2)

All READ (no binary run; audit only). Test paths: R = crates/xdevcmd/tests/render.rs, P = crates/xdevcmd/tests/pty.rs,
core = crates/xdevcmd-core/src, app = crates/xdevcmd/src. Manual cases: crates/xdevcmd/tests/manual/test-cases/.
How I applied the brief: a test PROVES only if it drives keys (render/pty) AND checks disk (or, for "view/browse",
the file's own bytes on screen) AND would fail with the bug. Matching a message xdevcmd prints = CONFIRMS.
Unit test of an inner function = CONFIRMS.

| # | promise | doc:line | tests | verdict | why (when not PROVES) |
|---|---|---|---|---|---|
| 1 | Enter on a .zip opens it like a folder; title shows where inside; `..` leaves with cursor on archive | guide.md:321 | R:4322 enter_browses_an_archive_and_f3_reads_inside_it | PROVES | |
| 2 | Enter on a .tar opens it like a folder | guide.md:321 | P:1879 an_entry_of_a_big_archive_opens_in_the_viewer; app pane/files.rs:2235 a_big_archive_opens_from_a_thread_and_esc_can_give_up_on_it | PROVES | (pty, real keys; shows entry size then its content) |
| 3 | Enter on a .tar.gz opens it like a folder | guide.md:321, README.md:103 | core archive.rs:1054 pack_then_list_then_extract_round_trips_for_every_format (list only) | CONFIRMS | no user-path test opens a .tar.gz |
| 4 | F3 reads a file inside an archive | guide.md:321 | R:4322 (zip); P:1879 (tar, >8 MiB entry) | PROVES | |
| 5 | F3's temp copy is private and removed when done / when xdevcmd ends | config.md:356 | R:4293 a_file_viewed_from_an_archive_leaves_nothing_behind_when_xdevcmd_ends (checks TMPDIR empty); app tempfiles.rs:172 a_run_gets_a_private_directory_of_its_own, :188 the_sweep_removes_ended_runs_and_nothing_else | PROVES (removal) / CONFIRMS (0700, crash sweep after 1 h) | mode and crash sweep only unit-tested |
| 6 | F5 inside an archive extracts what you marked to the other pane (without the dirs above it) | guide.md:321-322 | R:4340 f5_from_inside_an_archive_extracts_and_writes_are_refused (disk: a/main.rs content); core archive.rs:967 extracting_from_inside_takes_the_browsed_directory_off_the_front | PROVES (cursor item) | marked set (Insert, several items, a folder) never driven |
| 7 | Alt+F5 packs the selection (.zip) | guide.md:323, keys.md:82 | R:4277 packed() helper (b/proj.zip exists) + R:4375 round trip reads content back | PROVES | |
| 8 | Alt+F5 packs into .tar / .tar.gz | keys.md:82 | core archive.rs:1054 | CONFIRMS | format choice via the dialog never driven |
| 9 | Alt+F9 unpacks everything into the other pane | guide.md:323, keys.md:83 | R:4375 alt_f9_unpacks_everything_into_the_other_pane (disk: proj/README) | PROVES | zip only; tar/tar.gz unpack only via unit |
| 10 | An archive is never changed in place (F4/F6/F7/F8/Shift+F6 inside refused) | guide.md:324 | R:4340 (screen "inside an archive" for F8,F6,F7,Shift+F6,F4) | CONFIRMS | text only; archive bytes/mtime never compared before/after; F8 refusal not checked on disk |
| 11 | Names pointing outside the target are never written (relative `../`) | security.md:28, :8 | R:4387 an_archive_with_escaping_names_hides_them_and_writes_nothing_outside (Alt+F9, checks both parents on disk); core archive.rs:871 a_zip_slip_archive_writes_nothing_outside; :849 names_that_climb_out_are_refused | PROVES (zip) | tar with `../` never tested at any level |
| 12 | ...absolute names (`/x`, `C:/x`) never written | security.md:28 | core archive.rs:871 (zip `/tmp/abs.txt`), :849 clean() table | CONFIRMS | unit only; tar absolute names untested |
| 13 | A link already at a file's name is replaced, not followed | security.md:28 | core archive.rs:999 overwriting_a_link_in_the_destination_replaces_the_link_not_its_target; :896 a_symlink_already_in_the_destination_is_not_followed | CONFIRMS | unit with mocked "yes" answer; no F5/Alt+F9 path; see contradiction C2 |
| 14 | Links inside archives are not created | security.md:28 | core archive.rs:914 links_in_a_tar_are_skipped_and_setuid_is_dropped | CONFIRMS | unit; tar symlink only; tar hard link and zip symlink (unix mode 0o120000) untested |
| 15 | Set-id bits are dropped | security.md:28 | core archive.rs:914 (tar 4755 -> 755) | CONFIRMS | unit; zip external attrs with setuid untested; setgid/sticky not separately |
| 16 | An entry bigger than it claims is stopped | security.md:28 | none | MISSING | no test writes an entry whose data exceeds its header size |
| 17 | Refused if it would not fit (free space) | security.md:28, guide.md:325 | none (check_room only tested for ratio) | MISSING | |
| 18 | Refused at >1 GB AND >1000x its size ("zip bomb") | security.md:28, guide.md:325-326 | core archive.rs:1032 an_archive_that_would_unpack_to_far_more_than_its_size_is_refused (synthetic Items, calls check_room) | CONFIRMS | unit on inner fn with fake sizes; no real bomb through Alt+F9/F5; the two cases used (3 GiB/1000 B, 10 B) cannot tell AND from OR, so a legit 2 GB tar at 1:1 being refused would pass |
| 19 | Refused at more than a million files | security.md:28 | none | MISSING | (guide.md:325 also omits it) |
| 20 | Extract over an existing file asks; skip leaves it | security.md:41 (nothing replaced without asking); undocumented TC expectation | core archive.rs:981 an_existing_file_is_asked_about_and_skip_leaves_it | CONFIRMS | unit with mocked answer; overwrite dialog never driven for extract |
| 21 | Extract over a file / pack over an archive: new data synced, then swapped in | security.md:121 | core archive.rs:1087 extract_and_pack_sync_a_replacement_before_renaming_it | CONFIRMS | unit trace of syscalls |
| 22 | A file that appears where a pack goes is kept, not replaced | security.md:39 | core archive.rs:1120 a_file_that_appears_where_a_pack_goes_is_kept | CONFIRMS | unit with fault injection |
| 23 | Killed mid-extract/pack: no short file under its real name; restarted at next start | security.md:52, :55-56, :206-208 | none (xtask/src/tryout.rs and P kill tests cover copy/move only) | MISSING | |
| 24 | --root: archive via link out of root not opened/unpacked; Alt+F5 not outside root | security.md (safe modes) | R:3027-3052 a_flag_can_tighten... region (pack: disk checked; enter/unpack: text) | PROVES (pack) / CONFIRMS (unpack) | |
| 25 | Names kept exactly (non-UTF-8 bytes on Linux); copy acts on the very file | guide.md:289-290 | core ops.rs:5869 with_safe_operations_long_and_non_utf8_names_are_copied_and_replaced; core compare.rs:640 | CONFIRMS | engine-level only; no render/pty test lists a non-UTF-8 name and F5s it |
| 26 | ...rename and delete act on the very file listed (non-UTF-8) | guide.md:290 | none | MISSING | no test renames/deletes a non-UTF-8 name |
| 27 | Invalid UTF-8 shows `�` | guide.md:291 | none for file panes (termlog.rs:828 is the shell log) | MISSING | |
| 28 | Control characters in names are left out when drawn | guide.md:291-292 | none for file panes (core format.rs:199 is viewer lines, shows `·`) | MISSING | no render test lists a name with ESC/BEL/CSI and checks the frame |
| 29 | A control-char name is never typed into a shell / put on clipboard | security.md:26 | P:2395 a_directory_name_cannot_run_a_command_through_alt_c; R:4994 y_and_cap_y_copy...control_characters_are_refused; app safety.rs:235/242/247 | PROVES (Alt+C) / CONFIRMS (clipboard text) | |
| 30 | Names sort as read: file2<file10, case/accents/width folded, kana, NFD with NFC; kanji code-point | guide.md:53-56 | core sort.rs:407,465,489 names_sort_as_people_read_them_in_any_script,521 a_name_sorts_the_same_however_it_was_stored,545 | CONFIRMS | unit on comparator; no render test shows pane order; kanji code-point order not asserted (only "total") |
| 31 | Compare/sync match names by exact bytes | guide.md:293 | core compare.rs:640 names_that_are_not_utf8_are_compared_and_planned_as_they_are | CONFIRMS | unit; Alt+K/Alt+Y never driven with such names |
| 32 | Read.me vs read.me on a case-ignoring drive = case clash (≠), never copied over | guide.md:293-294, :314, :185 | core compare.rs:712, :736 (fault seam), plan.rs:1838 a_fat_drive_..._sees_case_clashes, :1958; manual plan-table.md M-PT-04 | CONFIRMS / MANUAL | no user-path on Linux CI; real drive only by hand (M-PT-04, "Automated: none") |
| 33 | NFC vs NFD may show as two | guide.md:294-295 | none | MISSING | low risk ("may") |
| 34 | New names may not hold `/` (rename) | guide.md:296 | R:2233 a_name_with_a_slash_is_refused_not_turned_into_a_move (disk checked); core rename.rs:226 | PROVES | |
| 35 | ...nor `/` or a control char in new directory | guide.md:296 | app app.rs:1219 mkdir_refuses_a_control_character | CONFIRMS | App-level unit, no keys |
| 36 | ...nor a control char in rename / Save as | guide.md:296 | core rename.rs:226 bad_names_are_refused_before_touching_the_disk; R:895 save_as_refuses_a_name_with_a_control_character (disk checked) | PROVES (save as) / CONFIRMS (rename) | |
| 37 | ...new file and copy as (Shift+F5) refuse `/` and control chars | guide.md:296 | none | MISSING | |
| 38 | Windows: new names refuse `< > : " \| ? * \`, trailing dot/space (and reserved names, undocumented) | guide.md:297 | core plan.rs:1721 windows_names_are_refused_by_their_rules (copy-destination planner, name_problem) | CONFIRMS | that is the F5 plan check, not the rename/mkdir validator; rename.rs:226 has no Windows chars; windows_e2e.rs has 1 smoke test |
| 39 | FIFOs and devices refused before open (F3) | security.md:27 | P:2994 f3_on_a_fifo_says_so_instead_of_freezing; core document.rs:912, :927 (device link) | PROVES (FIFO) / CONFIRMS (device) | |
| 40 | ...also copy, find, quick view | security.md:27 | core ops.rs:6248 copying_a_fifo_fails_fast_instead_of_hanging; find.rs:407; image.rs:235 only_a_regular_file_is_previewed | CONFIRMS | unit only |
| 41 | A picture claiming billions of pixels refused before decode | security.md:30 | core image.rs:303 a_header_claiming_billions_of_pixels_is_refused_before_decoding | CONFIRMS | unit; Alt+Q never driven on one |
| 42 | Enter on a program asks "Run it?", Cancel default | security.md:31, guide.md:174, config.md:104 | R:3519 enter_opens_only_known_documents_without_asking (.exe); R:3850 a_program_offers_a_terminal_tab_first_after_cancel; app opener.rs:385 | CONFIRMS | screen text; "Cancel default" for Enter not asserted (only for o) |
| 43 | o on a program asks "Run it?"; Enter = Cancel | security.md:31 | R:3866 o_opens_with_the_configured_program_and_asks_before_running_one (`!would open` after o,Enter) | CONFIRMS | screenshot-mode stand-in text, nothing launched to observe |
| 44 | ...launcher (.desktop), installer, disk image also ask | security.md:31 | app opener.rs:385 (.desktop, +x script) | CONFIRMS / MISSING | installers (.msi/.pkg/.deb) and disk images (.dmg/.iso/.img) untested |
| 45 | Unknown type asks "Open it?"; known documents open without asking | security.md:31 | R:3519; R:3536 | CONFIRMS | text only |
| 46 | A config every user can write is refused (Unix); group-writable fine | security.md:38, :313 | app config.rs:1457 a_world_writable_config_is_refused_but_group_writable_is_fine | CONFIRMS | unit on load(); start-up path never driven with a 0666 config |
| 47 | Config never read from the directory you browse | security.md:313-314 | none | MISSING | |
| 48 | A config with a mistake is ignored whole and asked about at start | config.md:38-43 | P:408 a_config_with_a_mistake_is_asked_about_at_start; P:438; R:1064; app config.rs:1440 | CONFIRMS | text; whether defaults (not partial values) are in force is not checked |

## Contradictions and doc inconsistencies

- C1 (doc vs doc) guide.md:325-326 lists two refusals (would not fit; >1 GB at >1000x); security.md:28 adds a third (>1 million files). Neither is tested.
- C2 (doc vs test) security.md:28 "a link already at a file's name is **replaced**, not followed". core archive.rs:896 a_symlink_already_in_the_destination_is_not_followed asserts that a link at a **directory** component is not replaced: the entry fails (`log.failed.len() == 1`) and the link is left. Only archive.rs:999 (link at the file's own name) matches the doc wording. Either way nothing is written through the link, so no data risk; the doc does not cover the directory case.
- C3 (wording) guide.md:326 says "zip bomb"; archive.rs:1032 asserts the message says "decompression bomb". Fine for tar, but a user searching the docs for the message will not find it.
- C4 (surface) guide.md:291 says control characters in names are "left out when drawn"; the only display test (format.rs:199) is for viewer lines, which draw them as `·`. That is not a contradiction for names, but nothing tests names.
- C5 (scope) guide.md:322 says F5 extracts "what you marked"; R:4340 extracts only the item under the cursor, with nothing marked.

## Counts

Some rows have two verdicts, one for each part. Primary verdict = the first one listed. 48 promises in all.
- PROVES: 13 (rows 1, 2, 4, 5, 6, 7, 9, 11, 24, 29, 34, 36, 39; in 5, 24, 29, 36 and 39 one part is only CONFIRMS)
- CONFIRMS: 25 (rows 3, 8, 10, 12, 13, 14, 15, 18, 20, 21, 22, 25, 30, 31, 32, 35, 38, 40, 41, 42, 43, 44, 45, 46, 48)
- MISSING: 10 (rows 16, 17, 19, 23, 26, 27, 28, 33, 37, 47)
- MANUAL: 1 part (row 32, M-PT-04, a real case-ignoring drive)

## The 5 most important gaps (writes outside the target and data loss first)

1. **Tar path traversal is untested** (row 11/12). Both the render test and the unit tests for `../` and absolute names build a ZIP. The tar code path (`.tar` and `.tar.gz`) is never tested with a `../` or `/abs` entry, and neither is a tar hard link to an outside path (row 14). If the tar branch skips `clean()`, a file gets written outside the target.
2. **A kill mid-extract or mid-pack is untested** (row 23). The doc says extract and pack are covered by safe operations (security.md:52/55-56): never a short file under its real name. No kill test exists (tryout.rs and pty.rs only kill copy/move), and no test kills an extract over an existing file. Risk: losing the old file.
3. **"An entry bigger than it claims" and ">1 million files" have no test at all** (rows 16, 19), and **"would not fit" has none** (row 17). These are three of the five hostile-archive promises. The bomb test (row 18) is a unit test on fake sizes. It cannot tell the AND rule from an OR rule, and it never sends a real archive through Alt+F9.
4. **Non-UTF-8 and control-character names are never driven through the panes** (rows 26-28). Nothing tests that rename or delete acts on the very file listed. Nothing tests that `�` is shown, or that ESC/CSI in a name are left out of the frame (a terminal-injection promise). A lossy round trip (`a\xff` -> `a\u{fffd}`) in rename or delete could act on the wrong file or on nothing.
5. **"Never changed in place" is checked only by text** (row 10). R:4340 asserts "inside an archive" for F8/F6/F7/Shift+F6/F4, but never compares the archive's bytes before and after. Also, link-in-destination handling and the overwrite question (rows 13, 20) are tested only with a mocked answer and never through F5/Alt+F9. A regression in the UI wiring, such as a replace-all being passed through, would not be caught.
