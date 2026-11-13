# Testing by hand

What the automated tests cannot reach: real USB sticks, real network shares, a pulled
cable, a real power cut, the look of a screen. Run this list on each pre-release
(beta, release candidate) before it is called good, on as many systems as you have, and
note what you saw. It is ordered by what would hurt most if wrong: data first.

Use scratch copies, never your only copy of anything. A run with `--timings` also
records how long each start took (Help → Usage shows it too).

Each check says how to set it up and what you should see. "✗" marks what would be a
bug; write down the system, the terminal and what happened.

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
3. **Move across drives, killed.** Move a folder from your disk to a USB stick and kill
   xdevcmd mid-way. Start again.
   - Every file is on one side or the other, or on both ("copied completely … both are
     kept"). Never on neither.
4. **A network share pulled mid-copy** (SMB or NFS: unplug the cable or stop the
   server; sshfs: kill the ssh connection). Copy a large file onto it.
   - The job box shows "network error — trying again in 1 s (1 of 5)", counting down.
     Bring the share back within about 30 s: the copy finishes by itself. Leave it
     down: after the fifth try, "Retry / Skip / Stop", saying it tried 6 times. Esc
     during a wait stops at once.
   - ✗ the job fails at once without trying again; it never asks; Esc waits for the
     countdown.
5. **A hard NFS mount whose server is gone**: the job may hang inside the system;
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

## What to send

For anything marked ✗, or anything that surprised you: the system and version, the
terminal, what you did, what you saw, and Help → Usage's lines (start-up, terminal).
Never send files you would not share; names can be described instead.
