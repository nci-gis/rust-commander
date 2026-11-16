# Testing by hand

For colleagues testing a pre-release: **Day 0** (about 45 minutes), then **a week** of
your normal file work, then **what to send**. The maintainer's full list is at the end.
Never use your only copy of anything: everything below happens in a sandbox folder.

## Day 0

**1. Get three downloads** from [Releases](https://github.com/nci-gis/rust-commander/releases),
for your system: this version's `xdevcmd-…` archive, the previous version's, and this
version's `xdevcmd-bench-…` (it holds `xtask`, the sandbox and benchmark tool). Unpack
each into a folder of its own. The builds are not signed: see
[troubleshooting](troubleshooting.md#macos) ([Windows](troubleshooting.md#windows)) for
the first start. About 4 GiB free disk.

**2. Turn the debug log on for the week** (it holds no file, folder, user or host name;
the five newest sessions' logs are kept):

```bash
echo 'export XDEVCMD_DEBUG=all' >> ~/.zshrc     # macOS; ~/.bashrc on Linux
```
```powershell
setx XDEVCMD_DEBUG all                         # Windows: in new windows
```

**3. Make the sandbox** in a folder of your choice (`--small` on a slow disk: 400 MiB):

```bash
/path/to/xdevcmd-bench-…/xtask try --make         # Windows: …\xtask.exe try --make
```

It makes `xdevcmd-try/`: `big.log`, `many/` (100,000 names), `tree/`, `replace-src/`
and `replace-dst/` (the same names, the destination older), `big/big.bin` (1 GiB) and an
empty `copy-to/`. Start xdevcmd on it: `/path/to/xdevcmd xdevcmd-try xdevcmd-try/copy-to`.

**On a Mac**: F-keys need Fn (or "Use F1, F2, etc. keys as standard function keys" in
System Settings → Keyboard); End is Fn+→; for Alt+ keys, Terminal → Settings → Profiles
→ Keyboard → "Use Option as Meta key".

**4. Six tasks, scored**, with the previous version, then with this one: 1 (no thought)
to 5 (gave up).

1. F3 on `big.log`; End; `/` and search for `INFO`; Esc.
2. Enter on `many`, then Esc at once.
3. On `tree`: F5 and read the plan; Esc (nothing copied).
4. F8 on a file in `replace-dst`, Enter; Ctrl+Z puts it back.
5. F1, type `sync`, Enter; Esc.
6. Anything you looked for and did not find: note it.

**5. Three data checks, this version only.** To kill xdevcmd, from another terminal:
`pkill -9 xdevcmd` (macOS, Linux) or `Stop-Process -Name xdevcmd -Force` (PowerShell).
After each, `xtask try --check` must end **no file lost or wrong**.

| | Do | You should see |
|---|---|---|
| A | In `replace-src`, Ctrl+A, F5 into `replace-dst`; choose **replace**; kill it mid-way. Start again: Enter | "ended in the middle of a job"; it finishes without asking again |
| B | F5 `big` into `copy-to`; kill it past 30 %. Start again: Enter | "kept part way"; the bar starts near where it stopped |

On a Mac (and Linux on Btrfs or XFS) the copy of B is a clone, done at once: start
xdevcmd for B with `XDEVCMD_CONFIG=xdevcmd-try/no-fast-copy.toml` before its path.
| C | F5 `tree` into `copy-to`; kill it mid-way. Run `xtask try --tamper`. Start again: **C** | "1 found different … copied again" |

**6. Clean up**: `xtask try --clean`. What the checks sent to the trash stays there.

**Lost or wrong file**, now or in the week (gone, short, empty, or not the one you
chose): stop and report it the same day as P0. Before starting xdevcmd again, copy the
debug log and the state folder ([troubleshooting](troubleshooting.md#sending-a-log-with-a-report));
keep the trash.

**Optional**: the speed on your own drives, unattended, any day: [Benchmarks](benchmark.md).
With Rust 1.90+, the automated tests at this version's tag:
`cargo test --workspace --locked --no-fail-fast` ([Development](development.md)).

## A week of real use

Use it for your normal file work, where you would use Explorer, Finder, mc or Total
Commander. **F1** finds every command by typing; **F10** quits. Worth doing in the week:
copy and move folders onto folders that share some names (read the table before
Enter); F8 and Ctrl+Z; F3 on large logs, F4; the shell (F12, Alt+C, Alt+G); compare
(Alt+K) and sync (Alt+Y); find (Alt+F7); a USB stick or a share if you have one.

## What to send

At the end of the week, one comment on the release page (nothing wrong is worth
sending too):

```text
version:      xdevcmd --version
system/shell: e.g. Windows 11, pwsh 7.4, Windows Terminal
lost data:    no (if yes: what you did, and is it in the trash?)
friction:     1–5 for each of the six tasks, the previous version / this one
checks A–C:   the last lines of each `xtask try --check`
refusals:     any time xdevcmd refused something it should have done, and when
attached:     the debug logs; bench/results/*.json if you ran the benchmark
```

And one [issue](https://github.com/nci-gis/rust-commander/issues) per problem, as it
happens. Say the system and version
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

## For the maintainer: the full list

Before a pre-release is called good, on real hardware the CI cannot reach. "✗" marks a
bug. Most of what is not here runs in CI on every system (render.rs, the engine's
tests) or, on Linux and macOS, in a real terminal (pty.rs).

### 1. Data survives what goes wrong

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
   again. Copy to another drive, or with `[general] fast_copy = false`: on the same
   APFS, Btrfs or XFS volume the copy is a clone, instant, with no checkpoint.
   - The dialog says "kept part way"; Continue's bar starts near where it stopped, and
     the copy is the same as the source (`cmp`; on Windows `fc.exe /b`).
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
     down: after the fifth try, the Failed dialog (R retry, S skip, Esc abort), saying it tried 6 times. Esc
     during a wait stops at once.
   - ✗ the job fails at once without trying again; it never asks; Esc waits for the
     countdown.
7. **A hard NFS mount whose server is gone**: the job may hang inside the system;
   Esc stops it once that call returns. Note how long it took.

### 2. Undo, and the trash

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
4. **Where there is no trash**: on macOS and Windows a network share, and on Windows
   any drive but the system drive (Linux makes a `.Trash-<uid>` on any writable mount). The table marks a replace ⚠ and its default is skip.
   - ✗ a replace there that claims to go to the trash.
5. **F8 on a Windows USB stick.** Delete a file with F8 (to the Recycle Bin).
   - Note what Windows does: asks to delete for good, or deletes. Either is to be
     written down; a silent delete for good would be a bug to report.

### 3. Before a copy: the plan

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

### 4. While it runs

1. **The time left**: a copy of a few GB to a USB stick. After a few seconds the job box
   says "about N min left"; it should be roughly right, not swing wildly.
   - ✗ a number from the first second; a number while nothing moves.
2. **Lists follow a move**: Alt+F7 a search, F6 one of the results elsewhere. The row
   stays in the list, under its new place.

### 5. Start-up and the screen

1. **Start-up time**: `xdevcmd --timings` (or Help → Usage). Note the total and the
   "terminal check" step on each terminal you use: on a local terminal the whole start
   should be well under 150 ms. Over SSH, or in a terminal that does not answer, the
   terminal check is the slow step.
2. **The look**: the table, the undo question, the job box with its countdown and time
   left, in Windows Terminal (cmd, PowerShell), Terminal.app, iTerm2, GNOME Terminal,
   kitty, tmux and over SSH. Vietnamese and CJK names in each.

### 6. Each system and terminal

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
6. **F1 Commands** in macOS Terminal.app, Windows Terminal and a Linux terminal: it
   opens, finds by typing, runs with Enter; Alt+1…9 run its rows only where Option is
   Meta (Terminal.app: off by default).
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
