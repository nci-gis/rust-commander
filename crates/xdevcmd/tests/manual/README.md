# Manual tests

What the automated tests cannot reach: real disks pulled out, real kills on real
hardware, a terminal CI does not have (Terminal.app, Windows Terminal, ConPTY), network
mounts, the look on a real screen. Everything else is automated: `render.rs`
(screenshots), `pty.rs` (a real terminal and shell), the engine's unit tests,
`xtask try` (kill and resume in a sandbox).

The expected result of each case comes from the user docs (`docs/*.md`), not from the
code: a case checks what xdevcmd promises a user.

## When

Before a pre-release is called good (beta, rc), on the maintainer's machines: every
case whose **Platform** you have. A case not run is written down as not run, never as
passed.

## Before you start: the test environment

Set it up as [How to test](../../../../docs/how-to-test.md) does, steps 1 to 3: the
release's archives from Releases, the debug log on, the sandbox made with `xtask try`.
Its checks A, B and C are M-DS-01, M-DS-03 and M-DS-04 here. Every option of the tool:
`xtask try --help`, `xtask bench --help`. Use test data only (`~/tmp`, `C:\tmp`): the
checks use the real trash.

## Cases

| File | Area |
|---|---|
| [data-safety.md](test-cases/data-safety.md) | Kills, pulled drives, network errors: no file lost or cut short |
| [undo-trash.md](test-cases/undo-trash.md) | Undo, the trash, replaced files |
| [plan-table.md](test-cases/plan-table.md) | The table before a copy, on real drives |
| [running-jobs.md](test-cases/running-jobs.md) | Time left, lists that follow a move |
| [startup-screen.md](test-cases/startup-screen.md) | Start-up time, the look in real terminals |
| [systems-terminals.md](test-cases/systems-terminals.md) | What runs only on Linux in CI, by hand elsewhere |

## A case

```markdown
### M-XX-NN Title
**Platform**: … · **Doc**: the promise (docs/….md#…) · **Automated**: what CI covers, or none
**Setup**: …
**Steps**: 1. … 2. …
**Expected**: what the user should see and find on disk
**Fail if**: what counts as a bug (✗)
```

Check the disk, not only the screen: `cmp` / `fc.exe /b`, `ls -la`, the trash.

## Recording a run

One line per case in the round's `Check` (`.agents/plan/cycles/Round_NNN.md`):

```text
M-DS-01 · macOS 15, APFS, Terminal.app · PASS
M-DS-02 · Windows 11, FAT32 stick · FAIL: job said done, file 0 bytes (issue #NN)
M-DS-06 · not run (no NFS server)
```

A FAIL that lost or changed a file is a P0: stop, keep the debug log
(`XDEVCMD_DEBUG=all`) and the state folder, do not empty the trash, file it the same day.
