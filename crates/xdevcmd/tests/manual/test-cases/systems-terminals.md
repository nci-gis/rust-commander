# Each system and terminal

What CI runs only on Linux, or only as a screenshot.

### M-ST-01 xdevcmd inside its own panel
**Platform**: Windows, cmd and PowerShell · **Automated**: Linux, macOS (pty)
**Steps**: 1. Type `xdevcmd` in the panel. 2. `start cmd` from the panel, then `xdevcmd`.
3. `xdevcmd` in a terminal opened from the Start menu.
**Expected**: 1 asks first, Enter ends it; 2 asks, `y` starts it; 3 does not ask. The menu
bar shows the process number.
**Fail if**: a second xdevcmd starts inside the first without asking.

### M-ST-02 Paths in the output
**Platform**: bash with and without the prompt-mark snippet; Windows cmd and PowerShell ·
**Doc**: [shell.md](../../../../../docs/shell.md#paths-in-the-output) · **Automated**: Linux bash (pty)
**Steps**: produce paths (a build's errors, MSBuild's, `C:\` paths); Shift+F12, Tab to a
path, Enter.
**Expected**: the pane goes there, the cursor on the file.
**Fail if**: a path in the output is not offered, or Enter goes elsewhere.

### M-ST-03 A list from the output
**Platform**: bash (`rg -l`, `git status --porcelain`); cmd (`dir /b /s`); PowerShell
(`Get-ChildItem -Name -Recurse`); macOS Terminal.app with and without Option as Meta,
iTerm2 · **Doc**: [shell.md](../../../../../docs/shell.md#a-list-from-the-output) ·
**Automated**: Linux bash (pty)
**Steps**: run the command, Shift+F12, `p`; then Alt+J, Alt+L, and F9 → View → Last list.
**Expected**: a list of the paths; each key reaches it where Option is Meta; `p` and the
menu work everywhere.
**Fail if**: a path missing from the list, or one that is not in the output.

### M-ST-04 A slow mount
**Platform**: Linux and Windows; sshfs with its server stopped, or SMB with the network
off · **Automated**: a slowed folder (test hook), Linux
**Steps**: go into it, then Esc.
**Expected**: the screen keeps answering; Esc stays where you were.
**Fail if**: the screen stops answering, or Esc does nothing.

### M-ST-05 Japanese names across systems
**Platform**: Windows, and the same folder copied from a Mac · **Automated**: sort unit tests
**Steps**: 1. On a Mac, make a folder of Japanese names; copy it to Windows. 2. Open it on
both. 3. Copy it back.
**Expected**: both sort the same and copy back intact (names and bytes).
**Fail if**: a name changed on the way back (`cmp`, or the bytes of the name).

### M-ST-06 F1 Commands
**Platform**: macOS Terminal.app, Windows Terminal, a Linux terminal · **Automated**: screenshots
**Steps**: F1; type part of a command's name; Enter; then Alt+1.
**Expected**: opens, finds by typing, runs with Enter; Alt+1…9 run its rows only where
Option is Meta (Terminal.app: off by default).
**Fail if**: a command not found by part of its name; Enter runs another row.

### M-ST-07 The light theme
**Platform**: Terminal.app (256 colours, a light profile), a light Linux terminal ·
**Automated**: none (real palettes)
**Steps**: Settings → theme `light`; browse, open a dialog, view a file.
**Expected**: readable, the focused frame clear. With 16 colours on a light palette: the
two cursor rows the same grey and marked; archives and errors all red (a known limit;
`ansi` suits 16 colours better).
**Fail if**: text unreadable on the light background; no way to tell which pane has focus.

### M-ST-08 Syntax colours in the viewer
**Platform**: Windows and macOS, 256 and 16 colours · **Automated**: screenshots
**Steps**: F3 on source code; End; a search.
**Expected**: coloured; a jump draws plain first, then in colour.
**Fail if**: the viewer stops answering on a big file, or the colours stay wrong after a jump.

### M-ST-09 Logging a tab
**Platform**: Windows and macOS · **Doc**: [shell.md](../../../../../docs/shell.md#logging-a-tab-to-a-file) ·
**Automated**: Linux, macOS (pty)
**Steps**: right-click a terminal tab → Log to file…, run a command, Stop logging (also
from F1); then onto a full USB stick.
**Expected**: ● while logged; the file has the output; on the full stick the tab shows
`!` and the shell goes on.
**Fail if**: output missing from the file; the shell stops when the stick is full.

### M-ST-10 Help → Documentation
**Platform**: a build from a release archive; also through a symlinked program (as
Homebrew installs it) · **Automated**: screenshots
**Steps**: F9 → Help → Documentation; copy the link; paste it in a browser.
**Expected**: names the `docs/` beside the program and the link at this version's tag;
copying the link works in a real terminal (OSC 52).
**Fail if**: it names another version's docs, or a folder that is not there.
