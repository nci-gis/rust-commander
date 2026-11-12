# Round 020: Paths in the shell's output (v0.4 entry B)

**Status**: Review (set aside for the owner's hand tests, 2026-11-12)
**Part of**: v0.4.0, entry B of `.agents/memory/2026-11-10-v0.4.0-plan.md`; ROADMAP item
30, part 1
**Date started**: 2026-11-11
**Date completed**: YYYY-MM-DD

## Goal

In the shell's history view (Shift+F12), paths in the output that exist on disk are
marked and can be acted on:
- Enter takes the file pane there, with the cursor on the file;
- F3 and F4 open the file at its line.

This is xdevcmd's own ground: the shell's output and the file panes in one process
(ROADMAP, Direction).

## Plan (kickoff review)

### 1. What changed since the plan entry

- **Round 019 split `app.rs`.** The work lands in:
  - `app/shell.rs` (`view_history`);
  - `app/open.rs` (`begin_view`, `begin_edit`, `edit_externally`);
  - `app/modals.rs` (the viewer's keys go through `on_modal_key`).
- **The owner uses Japanese file names on Windows.** Paths are found and measured in
  columns (`xdevcmd_core::format`, Round 019); the recogniser must take any Unicode name.
- The owner asked for no CI until the pull request. Windows parsing is covered by unit
  tests that run everywhere; the ConPTY tests run on the owner's machine.

### 2. Map: what exists, and what is missing

- **The history view**, `App::view_history` (`app/shell.rs:182`):
  - it takes `TermPanel::history(tab)` (text, the shell's cwd *now*, `full_screen`);
  - it opens a plain `Viewer` on a `Document`;
  - it keeps `history_dir` (one directory for the whole history; Save as uses it).
- **The viewer** (`viewer.rs`, 679 lines):
  - keys in `Viewer::key`: Esc/q, arrows, `:`/Ctrl+G go to, `/` `n` `N` search, `h` hex,
    `w` wrap, `y` copy, F2 Save as;
  - **Tab, Enter, F4 and the mouse are free.** F3 closes it today.
  - `go_to(&str)` is private.
- **What the shell says** (`pane/shell_report.rs`): `ShellReports` is vt100's callback.
  - It keeps only the **last** OSC 133 mark and the **last** OSC 7/9;9 directory.
  - **Nothing records where each command began or the directory it ran in.**
  - On Linux the directory comes from `/proc`, not OSC 7: OSC 7 can be forged, and it
    moves panes.
- **vt100 0.18**: `Screen::scrollback_len()` and `cursor_position()` give a line's place
  in the history when a mark arrives.
  - Once the history is full (`[terminal] scrollback`), lines drop off the top and those
    places drift. **But vt100 calls `Callbacks::on_scroll` for every row that leaves the
    history**: dropped when it is full, and every row erased by `CSI 3 J`. Counting
    those calls gives each line an absolute number that never drifts. This was found
    while answering the owner's question about `clear`, below.
  - **`clear` and Ctrl+L, tried against vt100 0.18.1 (2026-11-11):**
    - `clear` sends `\e[H\e[2J\e[3J` under `TERM=xterm-256color`, the panel's TERM. It
      erases the whole history (8 lines → 0), and each erased row passes through
      `on_scroll`.
    - `clear -x` and Ctrl+L (`\e[H\e[2J`) keep the history. The lines that were on the
      screen at that moment are wiped, not pushed into it.
    - Windows `cls` (cmd) and `Clear-Host` (pwsh) through ConPTY: not tried; probably
      `\e[3J` too. The owner can check this on Windows.
- **The editor**: `edit_externally` runs `$EDITOR file`. There is no line argument.

### 3. Outside check (2026-11-11)

- **VS Code's terminal** links only files verified to exist, in many line and column
  forms (`file:1:2`, `file:line 1, column 2`). Checking can be turned off where it is
  slow (remote). Same principle here: mark only what exists.
- **iTerm2's Semantic History** resolves relative paths against the working directory
  that shell integration reports. That is the analogue of "the directory when the
  command ran".
- **kitty's hints kitten** (`--type=linenum` opens the file at the line in an editor) and
  **WezTerm's Quick Select** (matches on screen, picked by a typed label) are
  keyboard-first. Tab between marked paths is the same idea, simpler.
- Sources:
  - [VS Code terminal basics](https://code.visualstudio.com/docs/terminal/basics)
  - [iTerm2 shell integration](https://www.iTerm2.com/shell_integration.html)
  - [kitty hints](https://sources.debian.org/src/kitty/0.46.1-1/docs/kittens/hints.rst)
  - [WezTerm Quick Select](https://wezterm.org/quickselect.html)

### 4. Critique of the plan entry

- **"Resolved against the shell's directory when the command ran" needs data that does
  not exist yet.**
  - New: at each OSC 133 `C`, `ShellReports` records a command:
    - the line it starts on (`scrollback_len + cursor row`);
    - its absolute number (the rows `on_scroll` has seen so far, plus its place now), so
      it is found again however many lines have dropped; a command whose lines were
      erased (`clear`) is dropped with them;
    - the directory it ran in.
  - Where the directory comes from:
    - Linux: `/proc/<shell pid>/cwd`, read in the callback (one read per command);
    - macOS: the last OSC 7;
    - Windows: OSC 9;9 or 7.
  - At view time each line of output belongs to the last command that started before
    it.
- **Without prompt marks there are no commands.** That is the default: the snippets in
  docs/shell.md are opt-in, and only bash's was run.
  - The fallback is the shell's directory now.
  - When a path is selected, the status line says which base was used ("the shell's
    directory now: this shell sends no prompt marks, docs/shell.md").
  - Never silent, never guessed beyond that.
- **Forged output.** A file `cat`ed into the shell can print any path, and an OSC 7 that
  bends a relative one.
  - The actions are only ever started by the user: move a pane, view, edit. Each is
    checked by the policy (`--root`; read-only refuses F4), as from a file pane.
  - **The resolved absolute path is on the status line before Enter.** Nothing runs on
    its own.
- **Existence checks on the UI thread.**
  - A `stat` on a dead network path can hang.
  - Check only the lines on screen, cache each answer for the view's life, and never
    check UNC (`\\server\…`) or `//host/…` names: they are shown unmarked.
  - Tab looks ahead through the document lazily, a screenful at a time.
- **"Mark in the live panel too?"** No, as the plan says. The live screen belongs to the
  program in it (keymap A), and marking there would mean drawing over it. History view
  only.
- **Bigger than it reads: the editor at a line.** `$EDITOR` has no standard for it. The
  known ones:
  - `vi`, `vim`, `nvim`, `nano`, `emacs`, `micro`, `hx`, `kak`: `+N file`;
  - `code`, `codium`: `-g file:N[:C]`;
  - `notepad++`: `-nN`.
  - Any other editor gets the file alone, and the status line says the line was not
    passed. A table, unit-tested.

### 5. Steps

1. **The recogniser**, `xdevcmd_core::paths_in` (new module `found.rs`). It is a pure
   function: one line in, a list of candidates out, each with a byte range, the path
   text, and the line and column if any. Forms:
   - `path:line[:col]` (grep -n, rustc's `--> src/main.rs:3:5`, gcc, eslint);
   - `File "x.py", line 12` (Python);
   - `at f (x.js:3:7)` (Node);
   - `x.cs(12,5)` and `x.cs(12)` (MSBuild);
   - Windows `C:\…`, `.\x`, `..\x`;
   - `~/…`;
   - plain relative and absolute paths;
   - bare names, as `ls` prints them, and `'a b'` (GNU ls quoting).
   - Refused: URLs (`scheme://`), versions (`1.2.3`), times (`12:30:45`), IPv6, anything
     with control characters.
   - A table test, a row for each form and each false positive. S.
2. **Commands in `ShellReports`**:
   - at `C`, record (line, text of the line, directory), with the shell's pid passed in
     at spawn for Linux;
   - keep the last 200 commands, oldest dropped;
   - `ShellReports` counts the rows `on_scroll` reports. `TermPanel::history` returns
     the commands with the text, each at its line now (its absolute number minus the
     rows dropped), or leaves it out if its line has gone.
   - Unit tests through vt100 with marks, including a full history that has dropped
     lines. S–M.
3. **The history view as a link view** (`viewer.rs`). The viewer gets an optional link
   layer: the candidates on screen, checked to exist (cached; UNC skipped), drawn
   underlined in the accent colour.
   - **Tab / Shift+Tab** select the next or previous path, scrolling to it. The status
     line shows the absolute path and its line, and the base it was resolved against.
   - **Enter**: the last file pane goes to it, with the cursor on the file (on a
     directory, goes into it); the view closes.
   - **F3**: views the file at its line. The history view is left as it was, and Esc
     comes back to it. Today F3 closes the view.
   - **F4**: edits the file at its line.
   - **Mouse**: a click selects, a double-click is Enter.
   - A stale path, or one outside the root, says so and does nothing. M.
4. **The editor at a line**: the table in §4, `edit_externally(path, line)`. S.
5. **Tests**:
   - unit tests for 1, 2 and 4;
   - render tests: a history with each form, Tab's status line, a URL and a version not
     marked, a missing file not marked;
   - a pty test: in the panel, `grep -n` and a Python traceback; Shift+F12, Tab, Enter;
     the pane is in that directory with the cursor on the file; F3 opens it at the line.
6. **Docs**: docs/shell.md, "Paths in the output", with what the prompt-mark snippets
   add; F1 rows from `BINDINGS`; CHANGELOG.

Sizes: about a week (M) in all. Risks:
- **False positives in prose** (a word that is also a file name nearby): mitigated by
  marking only what exists.
- **Slow `stat`**: mitigated by checking only what is on screen, the cache, and the
  network-path skip.
- **A wrong base without marks**: mitigated by the status line naming the base.
- **History drift once full**: mitigated by absolute line numbers counted through
  `on_scroll`.

### 6. The owner's decisions

1. History view only, not the live panel (recommended: history view only).
2. Keys: Tab/Shift+Tab, Enter, F3, F4, click (recommended as listed). F3 then views the
   path instead of closing the history view; Esc and q still close it.
3. Without prompt marks: resolve against the shell's directory now, and say so
   (recommended), or mark absolute paths only.
4. Bare names (as `ls` prints them) marked when they exist (recommended), or only names
   with a `/`, a `\` or a line number.
5. The editor-at-a-line table (recommended), or always the file alone.

## Do

The owner (2026-11-11): "bắt đầu round B nhé", with the recommended answer to each of
§6's five decisions. Work on `feat/v0.4.0`, pushed, no CI.

- **Step 1, the recogniser**: `xdevcmd-core/src/found.rs`, `paths_in(line)`. Quoted
  names with spaces first, then words split at whitespace, quotes, brackets, `|`, `,`,
  `;` and `=` (so `--manifest-path=Cargo.toml` gives the file). A drive's colon is not
  a line's; a sentence's trailing `.` or `,` is cut. Python's `", line N"` and MSBuild's
  `(N,M)` are read after the path. A table test of 18 forms, and one of 7 refusals.
- **Step 2, the commands**: `ShellReports` counts line breaks in the rows vt100 hands
  `on_scroll` (as a capture writes them, so a wrapped line counts once), and at each
  `133;C` records the line its output starts on (the breaks so far, plus those in a
  capture of the screen made with a copy of the stream's state, minus the blank rows
  below the cursor). 200 kept. `history()` gives the text and its first line's number,
  so each command maps to a line of the text or is dropped.
  - **Deviation (found by the pty test):** the directory is read at the prompt
    (`133;A`), not at `C`. The output is parsed behind the shell: with
    `printf C…; cd ../b`, `/proc` already said `b` when `C` was read. At the prompt the
    shell is waiting for a line, so its directory holds still.
- **Step 3, the link view**: `links.rs` (resolve, existence cache, UNC and `//host`
  skipped, `~` expanded) and a layer in `viewer.rs`: links underlined in the accent,
  the chosen one lit; Tab/Shift+Tab look up to 2,000 lines on, lazily, and bring the
  path into view; Esc lets go first; Enter goes; F3/F4 view/edit (on a directory they
  say Enter goes there); the hint row gains "Tab paths". `app/shell.rs::history_link`
  checks the policy root on the canonical path for every action; F4 goes through
  `refused(Action::Edit)`, so it is off in the read-only modes.
  - **F3** keeps the history in `App::return_to`; closing that viewer brings the
    history back where it was, the path still chosen.
  - **Deviation:** the mouse is "a click chooses, a click on the chosen path goes"
    rather than "a double-click goes". A double-click is still Enter (its first click
    chooses, its second goes), and two slower clicks show the full path in between.
  - The status for a chosen path is path, keys, then the base: on a narrow screen the
    base is what is cut.
- **Step 4, the editor at a line**: `editor_args` in `app/open.rs`, by the program's
  file stem, lower-cased (so `Code.exe` and `/usr/bin/vim` match): `+N file` (vi, vim,
  nvim, gvim, view, nano, emacs, emacsclient, micro, kak, mg, joe); `file:N:C` (hx,
  helix, subl, sublime_text, zed); `-g file:N:C` (code, code-insiders, codium, cursor,
  windsurf); `-nN file` (notepad++). Anything else gets the file alone, and the status
  says the line was not passed. After the editor, the history's message line shows how
  it ended (the status line is under the viewer).
- **Step 5, tests**: unit tests for 1, 2 and 4 (`found`, `shell_report`, `links`,
  `editor_args`); pty tests `a_path_in_the_history_is_viewed_edited_and_gone_to`
  (Shift+Tab, F3 at line 30 and back, F4 with a stand-in `vim` given `+30 file`, Enter
  to the pane) and `a_path_is_taken_from_the_directory_its_command_ran_in` (hand-written
  marks, a Python traceback, `cd` away, F3 at the line, click, click).
  - **Deviation:** no separate render tests. Every behaviour the plan listed for them
    is in the pty tests or unit tests, through the real shell.
- **Step 6, docs**: docs/shell.md "Paths in the output"; docs/security.md (the shell
  panel); F1's viewer rows; CHANGELOG Unreleased → Added.
- Cleanups: `TermPane::history_text` (replaced by `history_commands`) removed.

- 2026-11-12, **set aside at Review** for the owner's hand tests (this round's Check), with the ROADMAP rewritten around v0.4.0 the same day (owner: "clear proceed"). The next round starts from it; CI on all three systems for the whole branch comes first.
- 2026-11-12, **macOS CI, 3 pty tests** (run 43177254223). Causes: (1) the status line names a chosen path in full and `canonical()` resolves macOS's temp dir to `/private/var/folders/…/T/`, so the line was cut before the file name and the keys (`a_path_in_the_history_…`, and its F3 note "… — Esc back to the history"); (2) macOS `ps -o comm=` names a `#!` script after its interpreter, so the stand-in `ssh` script was not seen as remote (`no_list_is_made_while_ssh_runs`); (3) under (1), `a_path_is_taken_from_the_directory_…`: macOS has no /proc, a command's directory is the shell's last OSC 7, which the hand-written prompt never sent. Fix: `links::Said` (warning / path / keys+base), drawn by `Said::fit` with the path cut in its middle, never below half the line (`dd6b214`); `program_name` takes the script's name for a known interpreter (`dd6b214`); the test's prompt sends OSC 7 too (`31c139a`). Reproduced on Linux with `TMPDIR` a symlink to a macOS-length dir (fail without, pass with), and (3) with /proc reading disabled; unit tests `a_long_path_is_cut_in_its_middle_and_keeps_its_name_and_keys`, `a_script_is_named_for_itself_not_its_interpreter`. CI run 43181663296 all green.

## Check

- [x] Recogniser table green, false positives included
- [x] Command records survive a full history (test)
- [x] pty: Python traceback → Shift+Tab → F3 at the line, click → pane; output
  `path:line:col` → F3, F4 (`+30`), Enter → pane with the cursor on the file
- [x] Clippy `-D warnings` for Linux, windows-gnu and darwin; the full suite, 641 passed, 0 failed
- [ ] `cargo deny` (no new dependencies this round)
- [ ] The owner: bash with the snippet, then without; Windows `cmd`/`pwsh` (MSBuild and
  `C:\` paths)
