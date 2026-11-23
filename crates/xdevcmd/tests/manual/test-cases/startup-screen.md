# Start-up and the screen

### M-SS-01 Start-up time
**Platform**: every terminal you use · **Automated**: Linux, at the cut
**Steps**: `xdevcmd --timings` (or Help → Usage).
**Expected**: on a local terminal the whole start well under 150 ms. Over SSH, or in a
terminal that does not answer, "terminal check" is the slow step (at most 1.5 s).
Write down the total and the terminal check.
**Fail if**: over 150 ms on a local terminal that answers; over 1.5 s anywhere.

### M-SS-02 The look
**Platform**: Windows Terminal (cmd, PowerShell), Terminal.app, iTerm2, GNOME Terminal,
kitty, tmux, over SSH · **Automated**: screenshots only (no real terminal)
**Steps**: show the plan table, the undo question, the job box with countdown and time
left; folders with Vietnamese and CJK names.
**Expected**: aligned, readable, nothing cut mid-character.
**Fail if**: a column out of line, a character cut in half, a name unreadable.
