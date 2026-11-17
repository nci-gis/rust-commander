# Data safety

Promise: however a job ends, no file is cut short under its real name, and no source is
removed before its copy is complete ([security.md](../../../../../docs/security.md#safe-operations)).

### M-DS-01 Kill a copy that replaces files
**Platform**: all, on a real disk · **Automated**: pty kill tests · **With xtask**: `try` check A
**Setup**: two folders with the same file names, the destination's files older.
**Steps**: 1. F5, choose **replace** for the older ones. 2. While it runs, `kill -9`
xdevcmd (Windows: Task Manager → End task). 3. Start it again.
**Expected**: "The last run of xdevcmd ended in the middle of a job"; if it was between
the trash and the rename, "replacement finished … the old one is in the trash".
Continue finishes without asking again what you chose.
**Fail if**: a file empty, short or missing under its own name; an old file gone and
not in the trash.

### M-DS-02 Pull a USB stick mid-copy
**Platform**: all; FAT32, then exFAT · **Automated**: none (a removed drive)
**Steps**: 1. Copy a folder of a few hundred MB to the stick. 2. Pull it out halfway.
3. Put it back, start xdevcmd.
**Expected**: the job reports an error, not success. Complete files are whole; the one
being written is not there under its real name.
**Fail if**: the job says done; a file under its real name with the wrong size.

### M-DS-03 A big copy, killed past a checkpoint
**Platform**: all · **Automated**: checkpoint unit tests · **With xtask**: `try` check B
**Setup**: a file of 1 GB or more. Copy to another drive, or set
`[general] fast_copy = false` (on the same APFS, Btrfs or XFS volume a copy is a clone:
instant, no checkpoint).
**Steps**: 1. F5. 2. Past 30 %, `kill -9` xdevcmd or close the terminal window.
3. Start again, Continue.
**Expected**: the dialog says "kept part way"; the bar starts near where it stopped; the
copy equals the source (`cmp`; Windows `fc.exe /b`).
**Fail if**: the bar starts at 0 %, or the copy differs.

### M-DS-04 A checked continue (C)
**Platform**: all · **Automated**: pty resume tests (Linux, macOS) · **With xtask**: `try` check C
**Steps**: 1. As M-DS-03 with a folder of small files: kill mid-way. 2. Overwrite one
finished copy with zeros of the same size and the source's date
(`truncate -s0 f && truncate -s <size> f && touch -r src f`). 3. Start again, press **C**.
**Expected**: "1 found different … copied again"; the file equals its source.
**Fail if**: the zeroed file is kept, or Enter (not C) reads every file.

### M-DS-05 Move across drives, killed
**Platform**: all, disk → USB stick · **Automated**: engine tests on one machine's mounts
**Steps**: 1. F6 a folder to the stick. 2. Kill xdevcmd mid-way. 3. Start again.
**Expected**: every file on one side or the other, or on both ("copied completely …
both are kept").
**Fail if**: a file on neither side.

### M-DS-06 A network share pulled mid-copy
**Platform**: SMB or NFS (unplug or stop the server); sshfs (kill the ssh connection) ·
**Automated**: retry unit tests with injected errors
**Steps**: 1. Copy a large file onto the share. 2. Take the share away.
**Expected**: "network error — trying again in 1 s (1 of 5)", counting down. Back within
about 30 s: the copy finishes by itself. Left down: after the fifth try, the Failed
dialog (R retry, S skip, Esc abort), saying it tried 6 times. Esc during a wait stops at
once.
**Fail if**: the job fails at once; it never asks; Esc waits for the countdown.

### M-DS-07 A hard NFS mount whose server is gone
**Platform**: Linux or macOS with NFS · **Automated**: none
**Steps**: 1. Start a copy onto the mount. 2. Stop the server. 3. Esc.
**Expected**: the job may hang inside the system call; Esc stops it once that call
returns. Write down how long it took.
**Fail if**: Esc never takes effect after the server is back.
