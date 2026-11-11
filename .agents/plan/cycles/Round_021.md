# Round 021: Panelize, a panel from the shell's output (v0.4 entry C)

**Status**: Planning (kickoff written; waiting for the owner's go)
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
- **Session**: a list tab reopens as its root directory, as a results tab does. Said in
  the docs; keeping lists across runs is not asked for.

### 5. Steps

1. **The list thread** (`xdevcmd_core::find`): `find::list(paths, hidden) -> Search`
   sends `Found` for each path that exists, deduplicated by path, skipping network
   names, with `Progress` and `Done { scanned, unreadable }` as a search does. Unit
   tests: missing, duplicate and network names; the cap; cancel. S.
2. **A list tab** (`pane/files.rs`):
   - `Results` gains its source: `Find` or `List { from }`;
   - `FilePane::list(root, paths, from, sort)` opens on the list thread;
   - the title is ` list {from} · N`, plus "capped at 10,000" and "N outside the root"
     when they apply;
   - the delete confirmation says where a list came from.

   Everything else is the results tab's. S.
3. **From the history view** (`viewer.rs`, `app/shell.rs`):
   - **`p`** makes a panel of the selection's lines, else of the command at the chosen
     path, else of the last command (prompt marks), else says to drag first;
   - each line's paths are resolved as Round 020 does (`Links`, a resolve without the
     existence check), in the order printed;
   - the new tab opens in the last file pane and takes the focus, and the history
     closes;
   - refused while `ssh` runs in that shell, and in the restricted mode (no terminal).

   `Viewer` gains the selected range and a way to get the commands' line ranges. M.
4. **From the tab menu**: "Panel from the last command's output", with prompt marks
   only; without them, the item says to use Shift+F12 and drag. S.
5. **Tests**:
   - unit: 1; the line ranges for a selection, the chosen path's command and the last
     command; the source on the delete confirmation;
   - render: a list tab from a fixed list, with relative, absolute and mixed names (mc
     #2942) and its title; rename in place and Shift+F2 on rows in two directories;
   - pty:
     - `printf` paths → Shift+F12 → drag → `p` → F5 copies them to the other pane;
     - with hand-written prompt marks, `p` with nothing selected takes the last command;
     - read-only: the panel opens, and F5 into it, F6 and F8 are refused;
     - `--root`: rows outside are dropped and counted;
     - a fake `ssh` (Remote): refused.
6. **Docs**:
   - docs/shell.md, "A panel from the output";
   - docs/keys.md and F1: `p` in the history;
   - docs/security.md: a list is only names, and what F8 says;
   - CHANGELOG.

Size: S–M, about 3–4 days. The panel exists, so this is a source and a key, not a new
pane. The plan's M assumed a virtual panel to build.

Risks:
- **A big history**: the stat thread and the cap.
- **ssh output taken as local**: the refusal now; recording each command's program
  in 0.5.
- **Many files acted on from text anyone can print**: real paths only, and the
  confirmation names the source.

### 6. The owner's decisions

1. **"Run a command into a panel" (a dialog; xdevcmd runs it and reads its output)**:
   out of 0.4.0, the shell plus `p` instead (recommended), or in.
2. **The key in the history view**: `p` (recommended; free in the viewer, the viewer's
   keys are single letters) or another.
3. **No selection and no prompt marks**: say to drag first (recommended), or every path
   in the whole history.
4. **The cap**: 10,000 rows (recommended), or another number.
5. **ssh**: refuse making a panel while ssh runs, and add the warning to Round 020's
   status (recommended now); record each command's program in 0.5.

## Do

(Not started.)

## Check

- [ ] List thread: missing, duplicate, network names, cap, cancel (unit)
- [ ] A list tab: relative, absolute and mixed rows; title; rename across directories
- [ ] pty: drag → `p` → F5; marks → `p` takes the last command; read-only; `--root`; ssh
- [ ] Clippy for three targets, the full suite
- [ ] The owner: bash with `rg -l` and `git status --porcelain`; Windows `cmd`/`pwsh` with
  `dir /b /s` and `Get-ChildItem -Name -Recurse`
