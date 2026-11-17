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

**The programs.** After a release: its `xdevcmd-…` archive and `xdevcmd-bench-…` (which
holds `xtask`), from [Releases](https://github.com/nci-gis/rust-commander/releases).
Before a release (testing the branch to cut):

| What | Where |
|---|---|
| `xdevcmd` | Actions → the **ci** run of the branch's last code commit → Artifacts → `xdevcmd-<system>-<branch>` (macOS: universal, Intel and Apple Silicon) |
| `xtask` | Actions → **bench** → Run workflow on the branch → Artifacts → `xtask-<system>`. The runner's macOS is Apple Silicon: for an Intel Mac, build for `x86_64-apple-darwin` (as `bench/mac-intel-*` did, 2026-11-17) |

An Actions zip drops the executable bit on macOS and Linux: `chmod +x`, and on macOS
`xattr -d com.apple.quarantine`. `xdevcmd --version` says the last release's number until
the cut: the build is still the branch's.

**The sandbox.** In a folder of test data only (`~/tmp`, `C:\tmp`), never on real
files: the checks use the real trash. `xtask try` makes and checks it; the steps are
[How to test](../../../../docs/how-to-test.md), steps 3 and 6 to 9. Two windows: one
runs xdevcmd, the other `xtask try --kill-when A|B|C`, started first.

| Command, from that folder | Does |
|---|---|
| `xtask try --make` | `xdevcmd-try/`, 2.5 GiB (`--small`: 0.8 GiB), with a manifest |
| `xtask try --kill-when A` / `B` / `C` | waits, then kills xdevcmd mid-job |
| `xtask try --tamper` | zeroes one finished copy (check C) |
| `xtask try --check` | every copy against the manifest: PASS, FAIL or NOT DONE |
| `xtask try --reset A` / `B` / `C` | puts back what that check changed, to run it again |
| `xtask try --clean` | removes the sandbox, only what `--make` made |

If xdevcmd still offers an interrupted job when you reset, press **F** (forget it).
`XDEVCMD_DEBUG=all` for the whole session keeps a log to attach to a FAIL.

**Which cases it covers.** Check A is M-DS-01, check B is M-DS-03, check C is M-DS-04:
run them with `xtask try`, then record each as below. The others need real hardware
(a USB stick, a share) and are done by hand.

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
