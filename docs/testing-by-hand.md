# Testing by hand

What the automated tests cannot reach: real USB sticks, real network shares, a pulled
cable, a real power cut, the look of a screen, and people other than the author using it
for real work. Two ways to help, on each pre-release (beta, release candidate):

- **A week of real use** (anyone): the next section. No extra time beyond your normal
  file work, plus two checks of 15 minutes.
- **The full list** (the maintainer, before a pre-release is called good): sections 1–6,
  ordered by what would hurt most if wrong: data first.

Use scratch copies, never your only copy of anything. A pre-release is not yet promised
safe for your only copy.

Each check says how to set it up and what you should see. "✗" marks what would be a
bug; write down the system, the terminal and what happened.

## A week of real use

1. **Get it.** Pre-releases are on
   [Releases](https://github.com/nci-gis/rust-commander/releases), marked Pre-release:
   one program to unpack, nothing to install. The builds are not signed: macOS and
   Windows warn the first time, see [troubleshooting](troubleshooting.md#macos)
   ([Windows](troubleshooting.md#windows)). To check a download is the real one:
   [security.md, "Checking a download"](security.md#checking-a-download).
2. **Use it** for your normal file work, where you would use Explorer, Finder, mc or
   Total Commander. **F1** lists every command and its key, found by typing; **F10**
   quits. Worth doing in the week, in any order:
   - copy and move folders between the panes (F5, F6), one of them onto a folder that
     already has some of the same files: read the table before Enter;
   - delete to the trash (F8), then undo (Ctrl+Z);
   - view a large log (F3), edit a file (F4);
   - the shell panel: F12 in and out, Alt+T shows or hides it, Alt+C sends the pane's
     folder to the shell, Alt+G brings the shell's folder to the pane;
   - compare (Alt+K) or synchronise (Alt+Y) two folders; find by name and content
     (Alt+F7);
   - a USB stick or a network share, if you have one;
   - note anything you looked for and did not find, or a key you expected to work.
3. **Two data checks** (15 minutes, scratch folders): 1.1 (kill a copy) and 1.4 (a
   checked continue) below.
4. **Tell us**, as [What to send](#what-to-send) says, including "nothing went wrong in a
   week": a comment on the release is enough.

## 1. Data survives what goes wrong

1. **Kill a copy that replaces files.** Two folders with the same file names, the
   destination's files older. F5, choose **replace** for the older ones, and while it
   runs, `kill -9` xdevcmd (Task Manager → End task on Windows). Start it again.
   - You see "The last run of xdevcmd ended in the middle of a job" and, if it was
     between the trash and the rename, "replacement finished … the old one is in the
     trash". Continue finishes the job without asking again what you already chose.
   - ✗ a file that is empty, short, or missing under its own name; an old file gone
     without being in the trash.
2. **Pull a USB stick mid-copy** (FAT32, then exFAT). Copy a folder of a few hundred
   MB to it, pull it out halfway, put it back, start xdevcmd.
   - The job reports an error, not success. Files that were complete are whole;
     the one being written is not there under its real name.
   - ✗ the job says done; a file under its real name with the wrong size.
3. **A big copy, killed past a checkpoint.** Copy a file of 1 GB or more, kill
   xdevcmd (`kill -9`, or close the terminal window) once the bar is past 30 %. Start
   again.
   - The dialog says "kept part way"; Continue's bar starts near where it stopped, and
     the copy is the same as the source (`cmp`, or `fc /b` on Windows).
   - ✗ the bar starts at 0 %, or the copy differs.
4. **A checked continue (C).** As in 3 but with a folder of small files: kill
   xdevcmd mid-way, then overwrite one finished copy with zeros of the same size and
   give it the source's date (`truncate -s0 f && truncate -s <size> f && touch -r src f`).
   Start again and press **C**.
   - The summary says "1 found different … copied again"; the file equals its source.
   - ✗ the zeroed file is kept, or Enter (not C) reads every file.
5. **Move across drives, killed.** Move a folder from your disk to a USB stick and kill
   xdevcmd mid-way. Start again.
   - Every file is on one side or the other, or on both ("copied completely … both are
     kept"). Never on neither.
6. **A network share pulled mid-copy** (SMB or NFS: unplug the cable or stop the
   server; sshfs: kill the ssh connection). Copy a large file onto it.
   - The job box shows "network error — trying again in 1 s (1 of 5)", counting down.
     Bring the share back within about 30 s: the copy finishes by itself. Leave it
     down: after the fifth try, "Retry / Skip / Stop", saying it tried 6 times. Esc
     during a wait stops at once.
   - ✗ the job fails at once without trying again; it never asks; Esc waits for the
     countdown.
7. **A hard NFS mount whose server is gone**: the job may hang inside the system;
   Esc stops it once that call returns. Note how long it took.

## 2. Undo, and the trash

1. **Undo a copy.** F5 a folder of a few files, then Ctrl+Z, Enter.
   - It asks "Undo the copy of …?" and lists what it will remove. After it, the copies
     are gone and the originals untouched.
2. **Undo keeps what you changed.** F5 a folder, edit one of the copies, drop a new
   file into the copied folder, then Ctrl+Z.
   - The edited copy and the new file stay, and their folder too; the summary says why.
3. **A replaced file comes back.** Copy over an older file (choose replace), Ctrl+Z.
   - The new file is gone and the old one is back, from the trash. The table before the
     copy said "ℹ a file replaced goes to the trash: Ctrl+Z brings it back".
   - On Linux: any disk, a USB stick too. On macOS: a local disk and a USB stick. On
     Windows: the system drive (C:) only.
4. **Where there is no trash**: a network share, and on Windows any drive but the
   system drive. The table marks a replace ⚠ and its default is skip.
   - ✗ a replace there that claims to go to the trash.
5. **F8 on a Windows USB stick.** Delete a file with F8 (to the Recycle Bin).
   - Note what Windows does: asks to delete for good, or deletes. Either is to be
     written down; a silent delete for good would be a bug to report.

## 3. Before a copy: the plan

1. **The table.** Copy a folder onto one that has some of the same files, newer and
   older and of another size.
   - One line per group, worst first; ↑↓ moves between groups, Space changes skip /
     replace / ask each; the summary line counts what is created, replaced and skipped,
     and the space needed.
   - Nothing to decide (all new): the plain F5 dialog, as before.
2. **Enter before the check ends** (a big folder, a slow disk): the copy starts and asks
   about each existing file as it meets it. **Tab** checks again for the other pane.
3. **A USB stick in FAT32**: a file of 4 GiB or more is ⛔ "left out: 4 GiB or more,
   onto a FAT drive"; a name with `:` or `?` is ⛔ "a name the destination cannot
   hold"; on exFAT the 4 GiB file is fine.
4. **A case-insensitive drive** (macOS's default disk, Windows, a FAT stick):
   `Readme.txt` copied where `README.txt` is: listed as differing only in case.
5. **Free space**: a copy bigger than the free space is ⛔ "it needs …, and only … is
   free there". (Windows says nothing here yet: free space is unknown there.)
6. **A big tree** (100,000 files or more): the dialog opens at once, the count grows,
   and the memory in Help → Usage stays reasonable.

## 4. While it runs

1. **The time left**: a copy of a few GB to a USB stick. After a few seconds the job box
   says "about N min left"; it should be roughly right, not swing wildly.
   - ✗ a number from the first second; a number while nothing moves.
2. **Lists follow a move**: Alt+F7 a search, F6 one of the results elsewhere. The row
   stays in the list, under its new place.

## 5. Start-up and the screen

1. **Start-up time**: `xdevcmd --timings` (or Help → Usage). Note the total and the
   "terminal check" step on each terminal you use: on a local terminal the whole start
   should be well under 150 ms. Over SSH, or in a terminal that does not answer, the
   terminal check is the slow step.
2. **The look**: the table, the undo question, the job box with its countdown and time
   left, in Windows Terminal (cmd, PowerShell), Terminal.app, iTerm2, GNOME Terminal,
   kitty, tmux and over SSH. Vietnamese and CJK names in each.

3. **Numbers**: the copy benchmark on your own machine, as
   [Benchmarks](benchmark.md) says, once on each system you have.

## 6. Each system and terminal

What the automated tests run only on Linux, or only as a screenshot.

1. **xdevcmd inside its own panel** (Windows, cmd and PowerShell): `xdevcmd` typed in
   the panel asks first, Enter ends it; `start cmd` from the panel, then `xdevcmd`, asks
   and `y` starts it; a terminal opened from the Start menu does not ask. The menu bar
   shows the process number.
   - ✗ a second xdevcmd starts inside the first without asking.
2. **Paths in the output** ([Shell](shell.md#paths-in-the-output)): in bash with the
   prompt-mark snippet and without it, and in Windows cmd and PowerShell (a build's
   MSBuild errors, `C:\` paths). Shift+F12, Tab to a path, Enter: the pane goes there.
3. **A list from the output** ([Shell](shell.md#a-list-from-the-output)): `rg -l` and
   `git status --porcelain` in bash; `dir /b /s` in cmd; `Get-ChildItem -Name -Recurse`
   in PowerShell; then `p`. On macOS: Terminal.app with and without Option as Meta, and
   iTerm2: Alt+J, Alt+L, and F9 → View → Last list.
4. **A slow mount**: sshfs with its server stopped, or an SMB share with the network
   off, on Linux and Windows. Go into it, then Esc.
   - ✗ the screen stops answering, or Esc does nothing.
5. **Japanese names**: a folder of them on Windows, and the same folder copied from a
   Mac (macOS writes some names decomposed). Both sort the same and copy back intact.
6. **F1 Commands** in macOS Terminal.app (Alt+1 without Option as Meta), Windows
   Terminal and a Linux terminal: it opens, finds by typing, runs with Enter.
7. **The light theme** in Terminal.app (256 colours, a light profile) and a light Linux
   terminal: readable, the focused frame clear. With 16 colours, on a real light palette,
   the two cursor rows are the same grey and marked, archive and error files are all
   red (a known limit; the `ansi` theme suits 16 colours better).
8. **Syntax colours in the viewer** (F3 on source code) on a real terminal, with 256 and
   16 colours, on Windows and macOS. A jump (End, a search) draws plain first, then in
   colour.
9. **Logging a tab** ([Shell](shell.md#logging-a-tab-to-a-file)) on Windows and macOS:
   right-click a terminal tab → Log to file…, run a command, Stop logging (also from
   F1). The tab shows ● while logged. On a full USB stick the tab shows `!` and the
   shell goes on.
10. **Help → Documentation** in a build from a release archive: it names the `docs/`
    beside the program and the link at this version's tag. The link opens on GitHub;
    copying it works in a real terminal (it goes through the terminal, OSC 52), and
    through a symlinked program (as Homebrew installs it).

## What to send

One [issue](https://github.com/nci-gis/rust-commander/issues) per problem: anything
marked ✗, or anything that surprised you. Say the system and version
(`xdevcmd --version`), the terminal and shell, what you did and what you saw, and Help →
Usage's lines (start-up, terminal). For a copy, a move or a crash, attach a debug log:
[troubleshooting, "Sending a log with a report"](troubleshooting.md#sending-a-log-with-a-report).
It holds no file, folder, user or host name.

Never paste file names, paths or contents you would not share in public; describe them
("a folder of 3,000 small files on a network share").

Put the level in the title:

| Level | When |
|---|---|
| **P0** | Data lost, the wrong file touched, a file short or empty under its real name |
| **P1** | A crash or hang, cannot quit, the terminal left broken |
| **P2** | A wrong or misleading screen or doc |
| **P3** | Cosmetic |

A P0 or P1 holds the release until it is fixed, so send it even when unsure.
