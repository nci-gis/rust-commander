# Round 021: Panelize, a panel from the shell's output (v0.4 entry C)

**Status**: In Progress (the owner's go, 2026-11-11)
**Part of**: v0.4.0, entry C of `.agents/memory/2026-11-10-v0.4.0-plan.md`, trimmed as in
`.agents/memory/2026-11-11-v0.4.0-scope-proposal.md`; ROADMAP item 30, part 2
**Date started**: 2026-11-11
**Date completed**: YYYY-MM-DD

## Goal

A list of paths printed in the shell (`rg -l`, `git status --porcelain`, `find`, `grep
-n`, a build's errors) becomes a tab in a file pane. In that tab F5, F6, F8, marking,
rename, F3 and F4 work as on any listing.

This makes Round 020 a workflow: run a command in the shell, then act on the files it
named with the panes' keys.

**A list, once made, is not lost** (the owner, 2026-11-11). Leaving it to open a file
or a directory, Back (Alt+,) brings it back, and Alt+Down/Alt+J lists the recent ones. On the
way back each row is checked again: a path gone since is removed, and the user is told
which. Alt+F7's results and lists from the history are kept the same way.

## Plan (kickoff review)

### 1. What changed since the plan entry

- **Round 020 shipped what C needs to read the output:**
  - `xdevcmd_core::found::paths_in`, the recogniser;
  - `links.rs`, which takes each line's base from its command's directory;
  - `ShellReports.commands`, where each command's output starts and the directory it
    ran in, read at the prompt.
- **The scope proposal trimmed C** to the history view, "run into a panel", and Alt+F7's
  results as a panel. §4 below trims it again: one of the three already exists, and one
  is a new kind of trust.
- **The owner's answers after the first draft (2026-11-11):**
  - remote is out of 0.4.0, plugins postponed; 0.4.0 is about being stable, with no
    network and no telemetry;
  - lists are kept, and come back (the Goal above);
  - a slow read gets a progress bar under the pane. That is for Q (with directories
    read off the UI thread); this round uses it only if Q's bar exists by then,
    otherwise the title's count stays (§4);
  - the binary stays at most 7 MB. Nothing here adds a dependency.

### 2. Map: what exists, and what is missing

- **A results tab already is a panel.** `pane/files.rs:110`, `Results`, made by
  `FilePane::results(query, what, sort)` (`app/modals.rs:205`, Alt+F7):
  - Each row carries its full path. The name shown is relative to the search's root, or
    the full path when the row is outside it.
  - F5, F6, F8, marking, F3 and F4 work on the rows because they act on `Entry::path`.
  - A refresh re-checks the rows, it does not search again: deleted or moved-away rows
    go, the rest get their size and time again (`files.rs:391`).
  - Enter on a file goes to its directory with the cursor on it (`files.rs:658`). `..`
    goes back to the root.
  - Nothing can be copied *into* a results tab (`app/file_ops.rs:27`). Compare, the
    directory watcher, Alt+G and `follow` all skip one.
  - The title is ` find {what} · N found`. Results arrive from a thread through
    `find::Search` (an `mpsc` of `Event::Found` / `Progress` / `Done`), and
    `poll_results` takes them in.
  - The session saves a results tab as its root directory; the results are not kept.
- **So "Alt+F7's results become a panel" is already done** (0.3.0). Nothing to build.
- **From Round 020:**
  - `paths_in(line)` gives candidates with byte ranges, lines and columns;
  - `Links` resolves a line's relative paths against its command's directory, or the
    shell's directory now, and checks existence with a cache;
  - the viewer has a line selection (drag; `y` copies it) and, in a history, a chosen
    path.
- **xdevcmd has never run a command and read its output.** The user menu *types* into
  the terminal (`app/open.rs:405`). The editor, the opener and "Run in a terminal tab"
  start programs that own the screen or a tab; none is read back.

### 3. Outside check (2026-11-11)

- **mc's External panelize** (`Ctrl+X !`) runs a command and shows its output as a
  listing to work on. Saved commands are kept under labels.
  - Its own tracker shows the cost of the shortcuts it took. Absolute paths in the
    output failed to open until 4.8.20 (#2942). The feature is still missing from mc's
    help and man page (#3767, open). One guide describes the panel as a temporary
    directory of symbolic links.
  - Lesson: test absolute, relative and mixed lists, and keep the rows real paths, not
    links.
- **Far's Temporary panel** (TmpPanel, a standard plugin) holds paths to real files,
  with no copies and no directory structure. Find's "Panel" button sends results to it.
  That is the model here: a list of real paths, as the results tab already is.
- **Total Commander's "Feed to listbox"** sends search results to a panel, as Alt+F7
  does here.
- Sources:
  - [mc(1), External panelize](https://source.midnight-commander.org/man/mc.html)
  - [mc #2942](https://midnight-commander.org/ticket/2942)
  - [mc #3767](https://midnight-commander.org/ticket/3767)
  - [Softpanorama on external panelize](https://softpanorama.org/OFM/MC/external_panelize.shtml)
  - [Far: SetFindList (TmpPanel)](https://documentation.help/Far-Manager/setfindlist.html)
  - [Far Manager](https://en.wikipedia.org/wiki/Far_Manager)

### 4. Critique of the plan entry

- **Alt+F7 into a panel: drop it.** It has worked since 0.3.0 (§2).
- **"Run into a panel" is a new kind of trust, for little gain.** It would make xdevcmd
  run a command itself and read its output, for the first time. That means:
  - a shell per system (`sh -c`, `cmd /c`, `pwsh -c`), the user's aliases and functions
    missing, which surprises;
  - cancelling, stderr, a size cap, and a hang with no screen to show it;
  - a fresh look at the restricted mode, which turns the terminal off; a dialog that
    runs commands would need the same wall.

  The shell already does all of that, and the history already holds its output. **Run
  the command in the shell, then make a panel of its output.** It costs one key more
  than a dialog, and needs no new trust. Out of 0.4.0 (decision 1).
- **Which lines?**
  - **A selection** (drag in the history, as for `y`): its lines. This works in every
    shell, with or without prompt marks.
  - **With prompt marks and no selection**: the output of the command the chosen path
    is in, else of the last command. That is the "rg -l, then one key" path.
  - **Without either**: nothing is guessed. The status says to drag over the lines
    first. The whole history would mix every command's names (decision 3).
- **The same file many times** (`grep -n` gives one line per match): one row, the first.
- **Directories in the list** are rows like any other. Enter goes into one, which ends
  the list, as in a results tab.
- **Stat thousands of names off the UI thread.** A `find /` printed 50,000 lines.
  - Resolution (a join, no I/O) is done at once.
  - A thread stats each name and sends the ones that exist, as `find::Search` does, so
    `poll_results` and the title's progress are reused unchanged.
  - Names on another machine (`\\server\…`, `//host/…`) are skipped, as in Round 020.
  - Rows are capped (10,000, decision 4); the title says when the cap was hit.
- **Outside the root** (`--root`): those rows are dropped and counted in the title. The
  operations check the root again anyway.
- **Forged output, and F8.** A `cat` of a hostile file can print any names, and mark-all
  plus F8 in that tab would delete the real files it named. Hard rule 3 already makes
  the confirmation list what it deletes. New: when the tab is a list, the confirmation
  also says where the list came from ("names from tab 1's output"). It is small, and it
  is the one place the source matters.
- **Found while planning, in Round 020 too: ssh.** In a shell running `ssh`, the output
  is the other machine's. `/etc/hosts` printed there exists here too, so Round 020
  offers the local file, and a panel would list local files.
  - The commands record no "this was remote".
  - The cheap, honest fix: when the shell is in `ssh` *now* (`ShellState::Remote`, which
    Alt+G already uses), making a panel is refused, and the history's status for a
    chosen path adds "ssh is running: looked up on this machine".
  - Recording each command's program, to know which lines were remote, is left for 0.5
    (decision 5).
- **Rename and batch rename across directories.** Rows in one list sit in many
  directories. F6 in place and Shift+F2 act on the full path, so they should work, but
  no test covers a results tab with rows in different directories. One is added (step 5).
- **A list is lost today on the first step out of it** (the owner's point). Checked in
  the code:
  - F3 or F4 on a row opens over the tab, and Esc comes back to the list. Fine.
  - **Enter on a file** (to its directory), **Enter on a directory**, `..` and Alt+G all
    go through `FilePane::navigate`, which drops `results`. Back (Alt+,) then goes to
    the search's root directory: `back` and `forward` hold only `PathBuf`s
    (`pane/files.rs:50`), and Alt+Down/Alt+J's menu (`app/modals.rs:338`) is built from them.
  - So: **a place is a directory or a list.** `back`/`forward` hold `Place::Dir(path)`
    or `Place::List(id)`. Leaving a list pushes it, as leaving a directory does, so
    Alt+, after Enter on a row returns to the list, cursor on that row.
  - **The lists themselves live in the app**, not the tab: the last 10 (`Lists`,
    app-wide, by id), each with its rows, its source ("rg -l foo · tab 1", "find
    *.rs"), its root and the row last chosen. Closing a list's tab (Ctrl+W) keeps it.
    One pane's Back can reach a list made in another, which is what Alt+Down/Alt+J is for.
  - **Alt+Down/Alt+J** shows the recent lists above the directories, marked as lists, with
    their row counts. Choosing one opens it in the focused tab.
  - **On the way back each row is checked again**, on the list thread (no stat on the
    UI thread). Rows gone since are removed and **named**: "2 gone since: a.rs, b.rs"
    (the first few, then "and N more"). Today a results refresh drops rows silently;
    that changes too.
  - **Alt+F7's results are kept the same way** once the search ends, or when it is
    left: what it found so far, with "(stopped)" in the source.
- **Kept in memory, not in a file.**
  - The history's rule is "in memory only": a list is made of the shell's output, which
    can name private files.
  - A temporary file would need its own permissions, cleanup after a crash, and a place
    under `--root` rules.
  - The cost: lists are gone when xdevcmd quits. Keeping them in the session file (0600)
    is a later option, if the owner asks (decision 6).
- **Session**: a list tab reopens as its root directory, as a results tab does today.
- **Progress while a list is checked**: the title's count ("checking… 1,200 of 4,000")
  until Q's bar under the pane exists; then the bar.

### 5. Steps

1. **The list thread** (`xdevcmd_core::find`): `find::list(paths) -> Search`. It sends
   `Found` for each path that exists, deduplicated by path, and skips names on another
   machine. `Progress` and `Done { scanned, unreadable }` work as for a search, and
   `Done` also gives the paths that were gone. The same thread re-checks a kept list.
   Unit tests: missing, duplicate and network names; the cap; cancel; the gone ones. S.
2. **Kept lists and places** (`pane/files.rs`, `app`):
   - `Lists`, app-wide: the last 10, each with an id, its rows, its source, its root and
     the row last chosen.
   - `Results` gains its source (`Find` or `List`) and the list's id.
   - `back` and `forward` hold `Place::Dir` or `Place::List`. Leaving a list pushes it,
     and Back and Forward reopen it, re-checked, cursor on the row last chosen.
   - Rows gone since are removed and named on the status line.
   - Alt+Down/Alt+J: the recent lists first, then the directories.
   - Alt+F7's results are kept when the search ends or is left.
   - The title is ` list {source} · N`, with "capped at 10,000" and "N outside the
     root" when they apply.
   - The delete confirmation says where a list came from.

   M.
3. **From the history view** (`viewer.rs`, `app/shell.rs`):
   - **`p`** makes a list from the selected lines. With no selection, it takes the
     command at the chosen path, else the last command (needs prompt marks). With
     neither, it says to drag over the lines first.
   - Each line's paths are resolved as in Round 020 (`Links`, without the existence
     check), in the order printed.
   - The new tab opens in the last file pane and takes the focus, and the history
     closes.
   - Refused while `ssh` runs in that shell, and in the restricted mode (it has no
     terminal).

   `Viewer` gains the selected range, and a way to get each command's lines. M.
4. **From the tab menu**: "List from the last command's output", with prompt marks only;
   without them, the item says to use Shift+F12 and drag. S.
5. **Tests**:
   - unit:
     - step 1;
     - `Place` in back/forward;
     - the line ranges for a selection, for the chosen path's command, and for the last
       command;
     - the source on the delete confirmation.
   - render:
     - a list tab from a fixed list, with relative, absolute and mixed names (mc #2942),
       and its title;
     - Enter on a row then Alt+, back to the list, cursor on that row;
     - a row deleted meanwhile is named and removed on the way back;
     - Alt+Down/Alt+J lists the kept lists;
     - Alt+F7's results, left and brought back;
     - rename in place and Shift+F2 on rows in two directories.
   - pty:
     - `printf` paths → Shift+F12 → drag → `p` → F5 copies them to the other pane;
     - with hand-written prompt marks, `p` with nothing selected takes the last command;
     - read-only: the list opens; F5 into it, F6 and F8 are refused;
     - `--root`: rows outside are dropped and counted;
     - a fake `ssh` (Remote): refused.
6. **Docs**:
   - docs/shell.md, "A list from the output";
   - docs/keys.md and F1: `p` in the history; Alt+, and Alt+Down/Alt+J with lists;
   - docs/security.md: a list holds only names, it stays in memory, and what F8 says;
   - CHANGELOG.

Size: M, about 4–5 days. Kept lists add about a day to the first draft's S–M. The panel
exists; this is a source, a key and a store, not a new pane.

Risks:
- **A big history**: the stat thread and the cap.
- **ssh output taken as local**: the refusal now; recording each command's program
  in 0.5.
- **Many files acted on from text anyone can print**: real paths only, and the
  confirmation names the source.
- **Back/Forward changes type** (`PathBuf` → `Place`): every user of `back`, `forward`
  and `history()` moves to it. The compiler finds them; the render test for Back and
  Forward must stay green.

### 6. The owner's decisions

Settled (2026-11-11):
- remote out of 0.4.0; plugins postponed; the binary at most 7 MB;
- lists kept and brought back, gone rows named and removed;
- **the list model** (asked by the owner: two terminals, which list do they go into?):
  - **each extraction makes its own list**; nothing is merged unseen;
  - **the active list** is the one used last (shown in a pane, opened by Alt+L, chosen
    in the list menu, or returned to by Back), not the newest;
  - **`p`** in the history makes a new list; **`P`** (Shift+P) opens a menu "Add N
    paths to…", the active list first (Enter), the others by number, `n` a new list;
    the rows already there stay, the status says "added 5, 2 already there", and the
    list's source names both;
  - **Alt+L** shows the active list in the focused file pane, from a terminal through
    Alt+X then Alt+L, in the last file pane, which takes the focus;
- **macOS** (the owner: Alt+Down may not work there): every Alt key needs "Use Option
  as Meta key" (`keyfix.rs` says so once), and Alt+arrows are less dependable than
  Alt+letters. So the lists menu is **Alt+Down or Alt+J** (the pair the directory
  history already has), Alt+L is a letter, `p`/`P` have no modifier, and the menu bar
  gets Commands → "Lists…" (no Option needed). The actions have `[keys]` names.
- the six open decisions, on the recommendations ("ok, cập nhật kickoff rồi bắt đầu
  code"):
  1. no "run a command into a list" dialog in 0.4.0; the shell plus `p`;
  2. `p` (and `P`);
  3. no selection and no prompt marks: say to drag first;
  4. 10,000 rows per list, 10 lists kept;
  5. ssh: refuse a list while ssh runs, and warn in Round 020's status; each command's
     program recorded in 0.5;
  6. lists in memory only, gone on quit.

## Do

The owner (2026-11-11): "ok, cập nhật kickoff rồi bắt đầu code", on the settled model
and the six recommendations. Work on `feat/v0.4.0`, pushed, no CI.

- **Step 1, the list thread**: `find::list(paths)` and `find::check`, with
  `Event::Gone`. Each path once, in order; names on another machine count as gone,
  without being looked up. One unit test covers order, duplicates, network names,
  stop, and the thread's `Done`.
- **Step 2, kept lists and places**:
  - `lists.rs` is an app-wide store (a `Mutex` static, like `policy::get()`), so a
    pane's Back can reach a list made in another pane. It holds the last 10, the one
    used last at the front.
  - `pane/files.rs`:
    - `back`/`forward` hold `Place::{Dir, List}`;
    - `open_list` shows a list here and records the place it replaces;
    - `place()` keeps a search's results as a list the first time they are left (or
      when the search ends);
    - `leave_results` remembers the row the cursor was on.
  - `Results` gained `list`, `find`, `total`, `gone`, `select`, `capped` and
    `outside`; `poll_results` handles `Gone`, then puts the cursor back on the row.
  - **Deviation, found while writing it:** the recogniser offers words that never were
    files ("see", "warning"). A new list would then report them as "gone since" on its
    first check. So `List::fresh` holds the rows not checked yet: they go quietly, and
    only a row that was there and went is named (`lists::checked`). Rows added with
    `P` are fresh too.
  - Title: ` list {count} · {source}{left out}`. The count comes first and what was
    left out last, because a narrow pane cuts the middle of a title (the first draft
    put the count after the source, and a pty test saw it cut).
- **Step 3, from the history**:
  - `p` and `P` in the viewer (`list_request`): the selection, else the chosen path's
    command, else the last command; without either, it says to drag first.
  - `Links` gained `command_at`, `last_command`, `paths` (resolved, not looked up),
    `base_dir`, `label` and `remote`. The source is "sh · tab 1 · {command as typed}"
    (`command_text`, after the prompt) or "… · lines a–b".
  - `app/lists.rs`:
    - `show_list` (Alt+L): in the focused file pane, or from a terminal in the last
      one, which takes the focus;
    - `make_list` / `new_list`;
    - `add_to_list`, for the `P` menu ("Add N paths to…", active first, `n` new); Esc
      on that menu brings the history back.
  - `show_list` / Alt+L is in `acts_on_shell`, so Alt+X then Alt+L works from the
    shell.
  - Alt+Down/Alt+J lists the kept lists above the directories, titled "History and
    lists". View has "History and lists…" and "Last list".
  - **Deviation:** the plan said F9 → Commands → "Lists…". The menu bar has File, View
    and Help, so the entries are in View, next to History.
- **Step 4, the tab menu**: "List from the last command…" runs the same `list_request`
  through a viewer that is never shown. Without prompt marks it says to open the
  history and drag.
- **Step 5, the rest of §4**:
  - the delete confirmation says "From the list: …" or "Found by: find …";
  - while ssh runs (`History::remote`, from `ShellState::Remote`), `p` and `P` are
    refused, and a chosen path's status warns "ssh is running: this is looked up on
    this machine";
  - rows outside the root are dropped and counted in the title.
- **Found and fixed on the way: a drag that starts on a path.** Round 020 chose a path
  when the mouse button went *down* on it, so a drag starting on an underlined path
  could not select lines. A click now acts on release on the same cell (`pressed_link`).
- **Tests**:
  - unit: `find::check`; `lists` (active order, `fresh` and `checked`, `add`, caps,
    `common_root`, `some_names`); `command_text`.
  - render:
    - `alt_f7_results_are_kept_and_back_returns_to_them` (Back, and Alt+J);
    - `a_result_in_a_subdirectory_is_renamed_where_it_is`.
  - pty:
    - `a_list_from_the_history_is_worked_on_and_come_back_to`: drag → `p`; the
      never-a-file row goes quietly; `*`, F5 copies both; Enter on a row; Alt+, back
      with "1 gone since: y.txt";
    - `the_last_commands_output_makes_a_list_and_alt_l_brings_it_back`: marks → `p`;
      Alt+L from a pane, and through Alt+X from the shell; Alt+J; `P`, Esc, `P`, Enter
      → "0 added, 1 already there";
    - `no_list_is_made_while_ssh_runs`, with a stand-in `ssh` script;
    - `a_list_leaves_out_what_is_outside_the_root`.
  - Not written: a read-only pty test. Read-only refuses F5, F6 and F8 the same way
    in any pane, which existing tests cover; that a list *opens* in read-only is not
    tested.
- **Docs**:
  - docs/shell.md, "A list from the output";
  - docs/security.md;
  - F1's viewer rows;
  - docs/keys.md, regenerated with `--print-keys`;
  - CHANGELOG.
- **Seen and not explained:** in about six full runs of the pty suite, a wait timed
  out twice, in two different tests, under the full suite's load only.
  - Once the screen showed a menu label one cell off ("rintf" for "printf"). The text
    the app had built was right (logged, then the logging removed).
  - The four full runs after that all passed.
  - Worth a look in Q, with the soak test.

## Check

- [x] List thread: missing, duplicate, network names, stop (unit). The cap is in
  `lists::tidy_rows` (unit).
- [x] A list tab: relative and absolute rows, title, rename in a subdirectory (render).
  A mixed relative/absolute list from the shell is not tested on its own.
- [x] Kept lists: Enter → Alt+, back to the list, gone rows named, Alt+J, Alt+F7 kept
- [x] pty: drag → `p` → F5; marks → `p` takes the last command; `--root`; ssh. No
  read-only pty test (above).
- [x] Clippy `-D warnings` for Linux, windows-gnu and darwin; the full suite: 653
  passed, 0 failed (with the flake above seen in earlier runs)
- [ ] The owner: bash with `rg -l` and `git status --porcelain`; Windows `cmd`/`pwsh` with
  `dir /b /s` and `Get-ChildItem -Name -Recurse`; macOS Terminal.app with and without
  Option as Meta, and iTerm2 (Alt+J, Alt+L, F9 → View → Last list)
