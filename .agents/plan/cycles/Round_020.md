# Round 020: Paths in the shell's output (v0.4 entry B)

**Status**: Planning (kickoff written; waiting for the owner's go)
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
  - **Caveat:** once the history is full (`[terminal] scrollback`), lines drop off the
    top and those places drift. vt100 does not count dropped lines.
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
    - **the text of that line**, to find it again after the history has dropped lines;
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
   - `TermPanel::history` returns them with the text, each command's line found by its
     text near its recorded place (or dropped if it is gone).
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
- **History drift once full**: mitigated by finding each command line by its text.

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

(Not started.)

## Check

- [ ] Recogniser table green, false positives included
- [ ] Command records survive a full history (test)
- [ ] Render: marks, Tab, status line; pty: grep/traceback → Enter → pane, F3 at the line
- [ ] Clippy for three targets, the full suite, `cargo deny`
- [ ] The owner: bash with the snippet, then without; Windows `cmd`/`pwsh` (MSBuild and
  `C:\` paths)
