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
Its checks A, B and C show by hand what CI runs as M-DS-01, 03 and 04 (no longer
logged: see data-safety.md). Every option of the tool:
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

Every field is there, and text in quotes on the **Expected** line is quoted from
`docs/` (`…` for a part that varies): the docs are the oracle, not the code.
`cargo test --test manual_cases` checks both.

Check the disk, not only the screen: `cmp` / `fc.exe /b`, `ls -la`, the trash.

## Recording a run

A data-safety run is also a row in [runs.tsv](runs.tsv): tab-separated, under its
header. The kit writes it for you, checked as the ledger will check it:

```bash
xtask manual list                       # the cases logged by hand: M-DS-02, 06, 07
xtask manual show M-DS-02               # one case
xtask manual log                        # guided: pick the cases, P/F/W each, a note
```

The guided log asks which cases, the result of each, one note, and fills in the
terminal and the shell. One case by flags still works:
`xtask manual log M-DS-02 PASS --note "FAT32, then exFAT"`, or
`xtask manual log M-DS-06 WAIVED --note "no NFS server"`. M-DS-01, 03, 04 and 05 are
not logged: CI runs them (data-safety.md), and `xtask try --check` is for looking.

It refuses a row that would not count (a waiver without a reason, a build of
uncommitted code) and appends it to this file
inside the repository, else to `runs.tsv` in the current folder, to commit later. Use
the xtask that came with the xdevcmd you test: `built_from` is the commit xtask was
built from (`xtask --version` shows it).

**Log on the machine that ran the test.** A row takes its `os` from where `manual log`
runs, so a result relayed from another machine lands under the wrong system and the
ledger counts it there (2026-11-23: macOS and Linux results logged on Windows). Log
each system's rows there, or write them by hand with the right `os`.

By hand: `built_from` is the commit the tested build came from, as a SHA or
`git describe` (`…-g<sha>`), never a branch name or HEAD; a `-dirty` build counts as no
evidence. `os` is exactly `linux`, `macos` or `windows` (the terminal and the rest go in
their columns). `result` is PASS, FAIL or WAIVED (a waiver says why in `note`); `check`
is what `xtask try --check` said; `evidence` names the `--check` output or debug log
kept. `cargo xtask qa ledger` reads it before a cut: every case on every system needs a
PASS (or a waiver) built from code whose files *that case* vouches for (`CASE_PATHS` in
`xtask/src/qa.rs`) are the ones being cut, and a fresh FAIL anywhere blocks, whatever
rows follow it.

One line per case in the round's `Check` (`.agents/plan/cycles/Round_NNN.md`):

```text
M-DS-02 · macOS 15, exFAT stick, Terminal.app · PASS
M-DS-02 · Windows 11, FAT32 stick · FAIL: job said done, file 0 bytes (issue #NN)
M-DS-06 · not run (no NFS server)
```

A FAIL that lost or changed a file is a P0: stop, keep the debug log
(`XDEVCMD_DEBUG=all`) and the state folder, do not empty the trash, file it the same day.
