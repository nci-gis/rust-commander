# Audit: the shell panel and what xdevcmd types into it

Oracle: docs/shell.md (S), docs/security.md (Sec), docs/config.md (C), docs/guide.md (G), docs/keys.md (K).
Tests: pty.rs (P, `#![cfg(unix)]`, real pty + sh), render.rs (R, `--screenshot`), unit `#[cfg(test)]` modules (U).
All findings READ (file:line); nothing below was run. PROVES = user path + disk/real outcome + would fail on the bug.

| # | promise | doc:line | tests | verdict | why (when not PROVES) |
|---|---|---|---|---|---|
| 1 | Inside the shell xdevcmd takes only F12, Shift+F12, Alt+X; every other key (incl. Alt+C, Alt+G, F1, Ctrl+*) reaches the program byte for byte | S:8-9, S:46, S:59, G:335, K:6 | P:3828 inside_the_shell_only_f12s_family_is_xdevcmds (dd to keys.bin); U keys.rs:1799 | PROVES | |
| 2 | Alt+X then a terminal key acts on the shell: Alt+N new tab | S:47 | P:3896 alt_x_is_the_prefix...; R:2333 | PROVES | screen status "terminal 2/2" is the outcome |
| 3 | Alt+X Alt+S / Alt+D split, Alt+O cycle | S:47, G:337 | R:2034 split_keys_split_the_shell_when_it_has_focus | CONFIRMS | status text only |
| 4 | Alt+X Alt+W closes | S:47, G:337 | R:2049 closing_shells_then_the_last_one_removes_the_panel | CONFIRMS | screen only |
| 5 | Alt+X Alt+1..9 tab N | S:47-48 | R:417 alt_digit_in_the_terminal_picks_the_terminals_tab | CONFIRMS | status "1/2" only; Alt+2..9 not tried |
| 6 | Anything else after Alt+X goes to the program with it (ESC x + key) | S:48, G:337, K:6 | P:3896 (bytes `\x1bxq` on disk) | PROVES | |
| 7 | Enter after Alt+X goes to the program too | S:48, G:337 | P:3921 alt_x_then_enter_in_the_shell_reaches_the_program | PROVES | |
| 8 | Esc cancels Alt+X, nothing sent | S:48-49 | P:3921 (keys.bin starts with `z`) | PROVES | |
| 9 | Alt+X F1 opens Commands from the shell; F1 alone is the program's | S:49-50, K:6 | P:3585 from_the_shell_f1_is_the_programs...; R:1544 | PROVES | |
| 10 | Alt+X F12 = Shift+F12 (history) | G:338, troubleshooting:74 | P:4406 terminal_apps_shift_f12_and_alt_x_f12_open_the_history_too | CONFIRMS | screen only |
| 11 | Shift+F12 history is never written to disk unless the tab is logged | S:54-55, Sec:294 | P:4034 history_is_never_written_outside_the_shell | PROVES | |
| 12 | Alt+C types `cd <dir>` quoted for your shell (bash/zsh) | S:10, S:63, G:339 | P:2520 (after Ctrl+C, Alt+C, `pwd > here.txt` lands in sub); P:2656; U safety.rs:153 | PROVES | |
| 13 | Same quoting for fish, PowerShell, cmd | S:63, Sec:302 | U safety.rs:153 each_shell_gets_its_own_quoting | MANUAL | unit only; no fish/pwsh/cmd run (README: "not run") |
| 14 | A name with a control character is refused before any question (Alt+C) | S:93-94, Sec:26, Sec:302-303 | P:2395 a_directory_name_cannot_run_a_command_through_alt_c (no PWNED on disk); U safety.rs:235-247 | PROVES | idle shell only: "before any question" with a busy/unknown shell not driven |
| 15 | Same refusal for Alt+Enter names and for [[menu]] %f/%s | S:93-94, Sec:302-303 | U safety.rs:186 names_are_words_for_each_shell; U usermenu.rs:146 | CONFIRMS | no keys-driven test with an evil name for Alt+Enter or Shift+F9 |
| 16 | cmd: a name with `%` (or `!`) is refused whole | S:94, Sec:303 | U safety.rs:179, :186; usermenu.rs:146 | MANUAL | Windows; unit only |
| 17 | Alt+Enter types marked names (else cursor), each quoted, a space after, no Enter | S:11, S:64, Sec:305 | P:2881 alt_enter_types_the_marked_names_quoted_without_running_them | PROVES | |
| 18 | Bare names when the shell is in their dir, full paths otherwise | S:64 | P:2881; U app.rs:956, :982 | PROVES | |
| 19 | Ctrl+Enter = Alt+Enter | S:64, G (F1 row) | R:2269 (F1 row text names Ctrl+Enter) | MISSING | no test sends Ctrl+Enter (CSI 13;5u / modifyOtherKeys) |
| 20 | "Type into the shell" in the files' right-click menu types the names | S:64 | R:2269 the_context_menu_types_into_the_shell | CONFIRMS | only checks the row exists; never clicked, nothing typed checked |
| 21 | Names wrapped in bracketed paste when the shell asks | Sec:303-304 | P:2951 typed_names_go_in_bracketed_paste_when_the_shell_asks_for_it (bytes on disk) | PROVES | |
| 22 | Only Alt+C and own commands end with Enter | Sec:305 | P:2881 (one prompt, nothing ran); P:3401 (menu runs) | PROVES | |
| 23 | Alt+G takes the pane to the shell's directory; types nothing | S:10, S:65 | P:2695 alt_g_takes_the_pane_to_the_shells_directory | CONFIRMS | "types nothing" never asserted (no check of the shell line / keys.bin) |
| 24 | Alt+G refused while a program runs | S:65 | P:2718 alt_g_refuses_while_a_program_runs... (pane unchanged) | PROVES | |
| 25 | Alt+G refused when the directory is gone | S:65 | P:2718 | PROVES | |
| 26 | Alt+G refused when the shell runs ssh | S:65 | none (U term.rs:1566 state_of → Remote only) | MISSING | |
| 27 | Alt+G refused when the directory is unknown, or outside xdevcmd's root | S:65, Sec:272-273 | none | MISSING | no `--root` + Alt+G test; no unknown-dir test |
| 28 | Linux: a directory printed into the terminal (forged OSC 7) is not used | Sec:266-267, Sec:272 | P:2796 alt_g_on_linux_ignores_a_directory_printed_into_the_terminal | PROVES | |
| 29 | macOS: dir from /usr/sbin/lsof by full path, only when a key asks, 2 s limit; /bin/ps by full path | Sec:263-264, S:214 | P:2695/2718 are cfg(macos) too; nothing on PATH/timeout | MANUAL | macOS CI only, and no test of full-path/never-PATH or the 2 s bound |
| 30 | follow=true: the last-used pane follows the shell's cd, at the prompt only | S:67-68, C:84-85 | P:2747 with_follow_on_the_pane_follows... | PROVES | Linux only |
| 31 | follow: moving the pane yourself changes nothing until the next cd; pane never moves the shell | S:68-69 | P:2747 (Backspace, pane stays) | PROVES | "never moves the shell" not asserted |
| 32 | follow waits while a dialog is open | S:69-70 | none | MISSING | |
| 33 | follow leaves a pane showing an archive or find results alone | S:70 | none | MISSING | |
| 34 | follow off by default; nothing moves | S:67, C:84 | P:2857 follow_is_off_by_default_and_nothing_moves; U config.rs:1354; R:3441 | PROVES | |
| 35 | follow on macOS needs OSC 7; Windows only OSC 9;9/7 | S:214-215 | none | MANUAL | |
| 36 | A `C` mark with no `D` = busy (follow waits; sends ask) | S:225-226, Sec:276-277 | P:2824 follow_waits_while_the_last_mark_says_a_command_runs; U term.rs:1566 | PROVES | for follow; for typing (Alt+C with a C mark) unit only |
| 37 | A `C` before any prompt mark is ignored (a cat'ed file cannot make a shell busy) | S:226-228 | U shell_report.rs:479, :589 | CONFIRMS | unit only |
| 38 | A reported directory with a control character is ignored; another host's OSC 7 ignored | S:222-223, S:228, Sec:275 | U shell_report.rs:341, :489 | CONFIRMS | unit only |
| 39 | Typed names bare only when the system gave the shell's dir (never from OSC 7) | Sec:274 | U app.rs:956 typed_names_are_bare_only_in_a_directory_the_system_gave | CONFIRMS | unit of the decision, not keys |
| 40 | History Save as… never offers a reported (OSC 7) directory | Sec:275 | P:4128 save_as_from_a_shell_in_a_non_utf8_directory_offers_the_panes | MISSING | only the non-UTF-8 case; no forged-OSC 7 case |
| 41 | Shell surely at its prompt + send_when_idle=true (default): typed at once | S:79, C:81 | P:2520 (second Alt+C, cd lands); R:1996; U config.rs:1346, term.rs:1667 | PROVES | |
| 42 | send_when_idle=false: three-button question; S sends | S:79, C:81-83, Sec:280-281 | P:2656 with_sending_without_asking_off_alt_c_asks_and_s_sends (pwd on disk); R:1996 | PROVES | |
| 43 | Running program: asks Open a new shell (default) / Cancel; nothing typed into it (Alt+C) | S:80, Sec:15, Sec:35 | P:2421 (cat → typed.txt empty), P:2458 pipeline, P:2569 exec'd, P:4549 program tab | PROVES | |
| 44 | Same for Alt+Enter | S:80 | P:2917 alt_enter_types_nothing_into_a_running_program | PROVES | |
| 45 | Same for Shift+F9 and F1 menu rows | S:80 | P:3423 (sleep, no disk check of the program), P:3547 (cat, typed.txt empty) | PROVES | |
| 46 | `ssh` in the shell: asks New shell / Cancel, nothing typed | S:80, Sec:35 | U term.rs:1566 (Remote), :1667 (AskNewShell) | MISSING | no keys-driven send with ssh running, though a stand-in ssh exists (P:4878) |
| 47 | State unknown: asks New shell / Send (S) / Cancel | S:81, Sec:265 | U term.rs:1566, :1621, :1667 | CONFIRMS | the real Unknown state (unreadable /proc, Windows w/o marks) is never driven; P:2656 shows the dialog only for Idle+setting off |
| 48 | Line typed and not run: Alt+C goes to a new shell; the line stays | S:82, G:339, Sec:35 | P:2480 alt_c_never_adds_to_a_line_typed_and_not_run (no `made*` on disk, new shell pwd) | PROVES | "your line stays" checked by status text only |
| 49 | Same for your own commands (Shift+F9) | S:82 | P:2589 the_user_menu_never_adds... | PROVES | F1 menu row on a dirty line not tested |
| 50 | Alt+Enter's names still join a typed line | S:82 | none | MISSING | |
| 51 | "Typed" = any key since last Enter or Ctrl+C (Linux, macOS); a line ending in `\` counts | S:87-88 | U term.rs:1532 a_line_is_typed_until_enter_or_ctrl_c; P:2628 alt_c_after_a_continued_line_opens_a_new_shell | PROVES | `\` case; history-recall (Up) only unit |
| 52 | Open a new shell starts a tab in the pane's directory; for Alt+C that is all | S:84-85 | P:2421 (pwd > where.txt) | PROVES | |
| 53 | Names/command typed once the new shell's prompt is up | S:85-86 | P:2917 ('a b' at new prompt), P:3423 | PROVES | |
| 54 | No prompt within a few seconds: shell stays open, nothing typed, status says so | S:86 | U term.rs:1639 prompt_ready | MISSING | timeout path never driven (e.g. a shell that never prints) |
| 55 | Every shell tab in use: nothing typed, status says press Ctrl+C there | S:91-92 | P:2520 alt_c_with_a_typed_line_and_no_room... (max_terminals=1) | PROVES | only for a typed line + Alt+C; busy-program + no room, Alt+Enter and menu not tested |
| 56 | Over 4000 bytes always asks, even at an idle prompt | S:95-96 | U term.rs:1652 a_long_line_is_asked_about_even_at_an_idle_prompt | CONFIRMS | LONG_LINE constant not asserted = 4000; no keys-driven long selection / long %S menu |
| 57 | Linux: state from /proc; exec'd program seen on Linux only | S:97-98 | P:2569; U term.rs:1621 | PROVES | |
| 58 | macOS: state from `ps` | S:97, S:214 | P:2421/2917/3423 cfg(macos) | MANUAL | needs macOS CI/hand |
| 59 | Windows: only prompt marks; without them every send asks; a prompt-mark-printing program can look idle | S:98-99, S:215, S:230-232, Sec:278-281 | U term.rs:1566 (sees=false rows) | MANUAL | ConPTY pass-through unchecked (S:305) |
| 60 | Shift+F9 types a [[menu]] command; %f/%s names quoted; output visible | C:295, C:330 | P:3401 shift_f9_types_a_menu_command... (shell echoes GOT it's here.txt DONE) | PROVES | |
| 61 | %F, %S, %d, %D, %% placeholders | C:325-328 | U usermenu.rs:133 placeholders_are_quoted_for_the_shell | CONFIRMS | unit only |
| 62 | Template mistakes / %D without a second pane named | C:325-328 (undocumented error) | U usermenu.rs:158 | CONFIRMS | undocumented |
| 63 | [[menu]] rows in F1 type through the same door, from the shell too | G:403, G:408-410 | P:3496, P:3519, P:3547 | PROVES | |
| 64 | Paths in history underlined: compiler/grep, Python, MSBuild, ls, ~/, C:\, .\, quoted names | S:103-105 | U found.rs:281, :343; P:4617, P:4693 | CONFIRMS | unit for most forms; E2E only `src/x:30:2`, `notes.txt`, Python |
| 65 | URLs, version numbers, `\\server`, `//host` not offered (never looked up) | S:105-107, Sec:285-286 | U found.rs:328; U links.rs:327 | CONFIRMS | "cannot hang" not tested |
| 66 | Tab / Shift+Tab next/prev path; line under the viewer shows full path, line, base dir | S:111 | P:4617 | PROVES | |
| 67 | Enter or a click: file pane goes there, cursor on the file | S:112 | P:4617, P:4693 (click) | PROVES | |
| 68 | F3 views at its line; Esc back to history | S:113 | P:4617, P:4693 | PROVES | |
| 69 | F4 edits at its line | S:114 | P:4617 (fake vim writes args to disk) | PROVES | |
| 70 | Line number reaches editors (vi..joe `+N`, hx/subl/zed `file:N:C`, code/codium/cursor/windsurf `-g`, notepad++ `-nN`); others: file alone, status says so | S:121-124 | U app/open.rs:578 each_editor_is_told_the_line_its_own_way | CONFIRMS | unit covers nvim, vim, hx, code, notepad++, notepad only; kak, mg, joe, micro, nano, emacs, subl, zed, codium, cursor, windsurf not asserted |
| 71 | Esc lets go of the path; a second Esc closes the history | S:115 | P:4693, P:4953 | CONFIRMS | |
| 72 | Relative path from the directory its command ran in (with marks) | S:117-118, Sec:288-289 | P:4693 a_path_is_taken_from_the_directory_its_command_ran_in; U links.rs:307 | PROVES | |
| 73 | Without marks: shell's dir now, said; path not found there not offered | S:118-120 | P:4617 ("from" line); U links.rs:327 | CONFIRMS | |
| 74 | A path is only offered; checked as from a file pane (root; F4 off in read-only) | S:125-127, Sec:283-287 | none for --root or --read-only from the history | MISSING | |
| 75 | `p` on dragged lines makes a list; missing paths dropped | S:137, Sec:290-291 | P:4755 a_list_from_the_history_is_worked_on... (F5 copies rows to disk) | PROVES | |
| 76 | `p` with nothing selected: the chosen path's command's output, else the last command (marks) | S:137 | P:4817 (last command) | MISSING | "the command the chosen path is in" not tested |
| 77 | `P` adds to a list: menu, last first (Enter), others by number, `n` new; no duplicates | S:138 | P:4817 (Enter, "0 added, 1 already there") | CONFIRMS | number and `n` not tested; dedupe by status text |
| 78 | Tab menu "List from the last command…"; without marks and nothing dragged, nothing guessed | S:140-142, G:343 | none (R:2122 does not check this row) | MISSING | |
| 79 | In a list F3/F4/F5/F6/F8/marking/renaming work; Enter on a file goes to its dir | S:144-146 | P:4755 (F5, Enter); P:4953, P:5000 (F8 guards) | CONFIRMS | F3/F4/F6/rename in a list not driven |
| 80 | A list holds at most 10,000 rows; nothing outside the root; title says rows were left out | S:146-147, Sec:291 | P:5037 a_list_leaves_out_what_is_outside_the_root; U lists.rs:463 | CONFIRMS | root PROVES; cap value not asserted = 10,000 |
| 81 | Lists kept, last 10, in memory only | S:149 | U lists.rs:371 | MISSING | 10-cap not tested |
| 82 | Back returns to the list on that row; Alt+L; Alt+X Alt+L from the shell; Alt+J | S:151-155 | P:4755, P:4817 | PROVES | "on that row" not asserted; View menu entries not tested |
| 83 | Coming back re-checks rows; "1 gone since: y.txt" | S:156-157 | P:4755 | PROVES | |
| 84 | A whole-line existing path is taken whole (`a b.txt`, `report:2026.txt`) | S:161-163 | P:4913 panelize_takes_a_whole_line_that_names_a_file; U find.rs:485 | PROVES | |
| 85 | Not read: newline names, NUL output, control-char lines, quoted name with tab | S:164-166 | U found.rs:227 | CONFIRMS | tab case unit; NUL/newline untested |
| 86 | While ssh (mosh, telnet) runs: no list; a chosen path says it is looked up here | S:168-169, Sec:292 | P:4878 no_list_is_made_while_ssh_runs; U term.rs:1547 (mosh) | PROVES | ssh only; telnet not tested |
| 87 | Deleting from a list names the list in the confirmation | Sec:291-292 | none | MISSING | |
| 88 | Log to file… from the tab menu; ● shows; menu then offers Stop logging | S:176-179, G:343-347 | P:5201, P:5170 helpers; R:2145 | PROVES | |
| 89 | Log read before the screen: nothing lost past scrollback | S:177-178 | P:5270 a_log_keeps_what_scrolled_past_the_scrollback | PROVES | |
| 90 | Default name `xdevcmd-<shell>-<date>-<time>.log` in the shell's dir (Linux/macOS), else the pane's | S:181-182 | R:2145 (name shown); U termlog.rs:944 | CONFIRMS | default directory never checked; P tests type their own path |
| 91 | Existing file appended to; new one 0600 | S:182-183, Sec:296-297 | P:5201 (mode 0600); U termlog.rs:960 | PROVES | append: unit only |
| 92 | Appending keeps the file's permissions and says so when others can read it | Sec:297-298 | U termlog.rs:1045 (readable_by_others true) | CONFIRMS | the warning to the user is not asserted |
| 93 | Refused: link, FIFO, device (by the open itself) | S:184-185, Sec:298 | U termlog.rs:998, :1045 | CONFIRMS | unit of open; device not tested; not via the prompt |
| 94 | Refused: another user's file | S:185-186, Sec:298-299 | U termlog.rs:1026 | CONFIRMS | real check only as root, else skipped silently |
| 95 | Refused: outside --root | S:186 | none | MISSING | |
| 96 | Refused: read-only / restricted mode | S:186, Sec:299 | P:5369 read_only_mode_refuses_logging_a_tab (disk empty); P:3270 restricted has no shell | PROVES | |
| 97 | text: escapes removed, CR progress kept once, header/footer | S:188-191 | P:5201 | PROVES | |
| 98 | text: a full-screen program is one line | S:190 | U termlog.rs:906 | CONFIRMS | |
| 99 | raw: the bytes as received; Tab switches | S:187, S:192 | P:5239 a_raw_log_keeps_the_bytes; R:2145 | PROVES | |
| 100 | One log per tab; a split opened later is not in it | S:193-194 | none | MISSING | |
| 101 | Ends on stop / shell exit / tab close / quit, after writing what was queued | S:195-196 | P:5201, P:5303, P:5344 | PROVES | |
| 102 | Hung disk: each end waits up to 2 s per log, then says it may be incomplete | S:196-198 | U termlog.rs:1203 (close with 50 ms) | CONFIRMS | 2 s bound and the message on stop/quit not driven |
| 103 | Crash: footer and queue (≤ ~2 MB) lost, written part stays | S:198-199 | none | MISSING | |
| 104 | Shell never held by the disk > 0.5 s; slow disk stops the log, says why | S:200-202 | U termlog.rs:1203 (patience 50 ms, not 0.5 s) | CONFIRMS | real bound not asserted |
| 105 | Failed write (full disk): log stops, status says why, text log ends with a line saying so, ● → red ! until stopped | S:201-203 | U termlog.rs:1131 (raw, status), :1162; U pane/terms.rs:863 (!, said once) | CONFIRMS | the closing line in a text log file is never asserted |
| 106 | Logs keep secrets a command prints; nothing logged unless asked, per tab | Sec:294-300 | P:4034 (nothing written by default) | CONFIRMS | |
| 107 | xc (bash/zsh): F10 → cd to xdevcmd's last dir | S:20-28 | P:3284 quit_writes_the_directory_for_the_shell... (--cwd-file content, no newline) | CONFIRMS | the snippet itself is never run |
| 108 | xc: **Q** stays where you started | S:23 | P:3284 asserts a lone Q does NOT quit; Alt+X q is the no-cd quit (K:16) | MISSING | CONTRADICTION, see below |
| 109 | xc for fish / PowerShell | S:31-41 | none | MANUAL | marked "Not run yet" |
| 110 | bash OSC 7/133 snippet works | S:237-253 | U shell_report.rs:501 the_bash_snippet_in_the_docs_is_understood (recorded bytes) | CONFIRMS | recorded bytes, not the snippet run |
| 111 | zsh / fish / PowerShell / cmd snippets | S:255-303 | none | MANUAL | marked "Not run" |
| 112 | Quitting with a program in a shell asks first | (G, quit; undocumented here) | U app.rs:1198; P:325 quit_by answers it | CONFIRMS | |

## Doc lines that contradict tests

1. **docs/shell.md:23** "Quit with F10 to go to that directory, or **Q** to stay where you started." pty.rs:3303-3310 (`quit_writes_the_directory_for_the_shell_and_quit_without_cd_does_not`) asserts "A lone Q is a stray key now, not a quit (B4)" and quits with Alt+X q; docs/keys.md:16 agrees with the test. A user following shell.md presses Q and nothing happens. READ.
2. **docs/security.md:275** "the history's Save as… does not offer it [a reported directory]": the only test (pty.rs:4128) covers a non-UTF-8 directory, not a forged OSC 7. Not a contradiction, but the promise has no test. READ.
3. **crates/xdevcmd/tests/manual/README.md** lists `test-cases/systems-terminals.md` ("What runs only on Linux in CI, by hand elsewhere"); the file does not exist. So every MANUAL row above (macOS ps/lsof, Windows marks, ConPTY, fish/zsh/pwsh/cmd snippets) has no written manual case. READ (`ls manual/test-cases`).

## Counts

PROVES 51 · CONFIRMS 35 · MISSING 18 · MANUAL 8 (112 promises)

## The 5 most important gaps (something typed or run where it should not be)

1. **ssh in the shell (S:80, Sec:35)**: no keys-driven test that Alt+C / Alt+Enter / Shift+F9 ask and type nothing while `ssh` runs; only the unit `state_of`→Remote and `decide`. A `cd` typed into ssh runs on the other machine. A stand-in ssh already exists in pty.rs:4878.
2. **Unknown state (S:81, Sec:265)**: the real Unknown path (unreadable /proc or ps, Windows without marks) is never driven end to end; only P:2656, which reaches the 3-button question from an Idle shell with the setting off. If Unknown were wired to Send, no test would fail.
3. **Over 4000 bytes (S:95-96)**: only `decide_for(.., LONG_LINE+1)` unit; no test that a long Alt+Enter selection or `%S` menu command asks at an idle prompt, nor that LONG_LINE is 4000. dash/sh/cmd drop the line silently.
4. **All tabs in use with a busy program, and the new-shell timeout (S:86, S:91-92)**: only Alt+C with a typed line and max_terminals=1 is tested. Busy program + no room, Alt+Enter/menu + no room, and "no prompt within a few seconds → nothing typed" are not driven; a fallback that typed into the old shell would pass.
5. **Control characters through Alt+Enter and the user menu, and Alt+Enter onto a typed line (S:82, S:93-94)**: the PWNED test drives only Alt+C; Alt+Enter and Shift+F9 refusals are unit-only, the "before any question" order is not checked with a busy shell, and the documented "Alt+Enter's names still join" a typed line has no test at all.
