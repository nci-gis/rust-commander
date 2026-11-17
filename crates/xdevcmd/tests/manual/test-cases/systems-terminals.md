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

### M-ST-03 A list from the output
**Platform**: bash (`rg -l`, `git status --porcelain`); cmd (`dir /b /s`); PowerShell
(`Get-ChildItem -Name -Recurse`); macOS Terminal.app with and without Option as Meta,
iTerm2 · **Doc**: [shell.md](../../../../../docs/shell.md#a-list-from-the-output) ·
**Automated**: Linux bash (pty)
**Steps**: run the command, Shift+F12, `p`; then Alt+J, Alt+L, and F9 → View → Last list.
**Expected**: a list of the paths; each key reaches it where Option is Meta; `p` and the
menu work everywhere.

### M-ST-04 A slow mount
**Platform**: Linux and Windows; sshfs with its server stopped, or SMB with the network
off · **Automated**: a slowed folder (test hook), Linux
**Steps**: go into it, then Esc.
**Expected**: the screen keeps answering; Esc stays where you were.
**Fail if**: the screen stops answering, or Esc does nothing.

### M-ST-05 Japanese names across systems
**Platform**: Windows, and the same folder copied from a Mac · **Automated**: sort unit tests
**Expected**: both sort the same and copy back intact (names and bytes).

### M-ST-06 F1 Commands
**Platform**: macOS Terminal.app, Windows Terminal, a Linux terminal · **Automated**: screenshots
**Expected**: opens, finds by typing, runs with Enter; Alt+1…9 run its rows only where
Option is Meta (Terminal.app: off by default).

### M-ST-07 The light theme
**Platform**: Terminal.app (256 colours, a light profile), a light Linux terminal ·
**Automated**: none (real palettes)
**Expected**: readable, the focused frame clear. With 16 colours on a light palette: the
two cursor rows the same grey and marked; archives and errors all red (a known limit;
`ansi` suits 16 colours better).

### M-ST-08 Syntax colours in the viewer
**Platform**: Windows and macOS, 256 and 16 colours · **Automated**: screenshots
**Steps**: F3 on source code; End; a search.
**Expected**: coloured; a jump draws plain first, then in colour.

### M-ST-09 Logging a tab
**Platform**: Windows and macOS · **Doc**: [shell.md](../../../../../docs/shell.md#logging-a-tab-to-a-file) ·
**Automated**: Linux, macOS (pty)
**Steps**: right-click a terminal tab → Log to file…, run a command, Stop logging (also
from F1); then onto a full USB stick.
**Expected**: ● while logged; the file has the output; on the full stick the tab shows
`!` and the shell goes on.

### M-ST-10 Help → Documentation
**Platform**: a build from a release archive; also through a symlinked program (as
Homebrew installs it) · **Automated**: screenshots
**Expected**: names the `docs/` beside the program and the link at this version's tag;
copying the link works in a real terminal (OSC 52).
