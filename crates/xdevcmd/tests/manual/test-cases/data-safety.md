# Data safety

Promise: however a job ends, no file is cut short under its real name, and no source is
removed before its copy is complete ([security.md](../../../../../docs/security.md#safe-operations)).

The cases a machine can run are run by CI on Linux, macOS and Windows, killed at a
chosen step and started again (`crates/xdevcmd/tests/kill.rs`, Round 036). Their green
run at the cut commit is their evidence; nothing is logged for them here.

| Case | Run by CI as |
|---|---|
| M-DS-01 Kill a copy that replaces files | `a_planned_copy_killed_then_continued_keeps_every_decision`; through the trash, `a_replace_killed_after_the_trash_is_finished_at_the_next_start` |
| M-DS-03 A big copy, killed past a checkpoint | `a_big_copy_killed_after_a_checkpoint_goes_on_from_there` |
| M-DS-04 A checked continue (C) | `a_checked_resume_repairs_a_file_the_journal_called_done` |
| M-DS-05 Move across drives, killed | `a_move_killed_before_removing_its_source_says_both_are_there` |

`xtask try` (checks A to C) still shows the same by hand, for whoever wants to look.

The three below need a real device or network, and stay by hand. Each goes stale only
when a file it vouches for changes (`CASE_PATHS` in `xtask/src/qa.rs`).

### M-DS-02 Pull a USB stick mid-copy
**Platform**: all; FAT32, then exFAT · **Automated**: none (a removed drive)
**Steps**: 1. Copy a folder of a few hundred MB to the stick. 2. Pull it out halfway.
3. Put it back, start xdevcmd.
**Expected**: the job reports an error, not success. Complete files are whole; the one
being written is not there under its real name.
**Fail if**: the job says done; a file under its real name with the wrong size.

### M-DS-06 A network share pulled mid-copy
**Platform**: SMB or NFS (unplug or stop the server); sshfs (kill the ssh connection) ·
**Automated**: retry unit tests with injected errors
**Steps**: 1. Copy a large file onto the share. 2. Take the share away.
**Expected**: "network error — trying again in … s (… of 5)", counting down. Back within
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
