# Testing

> **Read this when**: adding or changing a test, a test fails in a way you do not understand, or you want to prove a fix by putting the bug back.

---

## How to test

```bash
cargo test                         # 292 tests, about 15 s (the soak test is opt-in)
cargo deny check                   # advisories, licences, sources
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Four layers:

| | |
|---|---|
| `xdevcmd-core` unit tests | sorting, formatting, listing, `ops.rs`, `document.rs` |
| `xdevcmd` unit tests | layout, keymap, config, menu, base64 |
| `tests/render.rs` | runs `--screenshot --keys=…` and asserts on the drawn frame; every platform |
| `tests/pty.rs` | the real binary in a real pty, real key and mouse bytes in, the screen read back through vt100, assertions on the disk; Unix only |

**`tests/pty.rs` is the layer that catches what a frame cannot**: behaviour over time,
and the terminal talking back. Two of its tests were proved by putting the bug back:

- remove `marked.clear()` from `finish_job` and
  `copying_over_a_file_asks_first_then_overwrites` times out waiting for the marks
  to clear;
- resume from `$EDITOR` with `ratatui::init()` + `Terminal::clear()` and
  `f4_hands_the_terminal_to_the_editor_and_takes_it_back` fails with *"The cursor
  position could not be read within a normal duration"* — the original failure, because
  the harness, like a slow SSH link, never answers that query.

It waits on the screen rather than sleeping, so the whole suite runs in about half a
second; 20 consecutive runs and 5 runs under full parallel load were all clean.

Unix only because on Windows the same bytes pass through ConPTY's translation into
console events, and whether SGR mouse input survives that is unverified. `render.rs`
covers the same behaviour on all three platforms.

**Windows has its own, smaller layer under a real ConPTY** (Rounds 018–019):
`tests/winpty/mod.rs` runs `xdevcmd.exe` under portable-pty, answers ConPTY's `ESC[6n`,
and reads the output both as text (for what is printed before the screen) and through
vt100 as a screen. `windows_panel.rs` covers the question asked when started from a
panel; `windows_e2e.rs` covers the panes, a command run in `cmd.exe` in the panel, and
F10. They compile on every system (clippy for `x86_64-pc-windows-gnu` checks them) and
run only on Windows. Keys only: no mouse yet.

**Faults: a data-safety fix is shown failing through the fault seam** (Round 029). A
sync moved after its rename passes every content check, and a full disk or a swapped
target cannot be had on demand. `xdevcmd-core/src/faults.rs` puts `faults::at(Point,
path)` before each guarded call (`Write`, `SyncData`, `SyncDir`, `Rename`,
`RemoveSource`, `JournalBegin`, `JournalLine`, `JournalSync`, `ReadDir`, `Stat`). Rules
match a path fragment (`fail`, or `set` with `Swap`, `Halt`, `Delay`, `Skip`), live while
their guard does, and `faults::trace`/`traced` record the order, for order tests. It
exists only under `cfg(any(test, feature = "faults"))`: the pty and render tests' binary
has it and reads `XDEVCMD_FAULTS` (`kind|point|fragment|arg`, `;` between rules; `|`
because `:` is in Windows paths), and `tests/budgets.rs` checks the release build has
none of it. Places: `faults::places::{other_device, small_fs}` (`XDEVCMD_TEST_OTHER_DEV`,
`XDEVCMD_TEST_SMALL_FS`, mounted by CI; elsewhere skipped, saying so). A new write path
that cannot be undone goes through `faults::at` (or `faults::sync_data`, `sync_dir`,
`ops::commit_replace`) with an order test from the trace. No `#[cfg(test)]` statics per
module for a new hook (the old pattern), and no seam in the release build. To show such a
test fails on the old code: a `git worktree` of the previous commit with the one
`faults::at` put in front of the call it guards, the test written to compile on both.

**Every test runs xdevcmd isolated** from whoever runs it: `XDEVCMD_CONFIG` points at a file
that does not exist, `XDG_DATA_HOME` at a scratch directory, and the pty tests use
`/bin/sh` with a plain prompt. Without that, a developer who had rebound F5 would see
every copy test fail, and a stray F8 in a test would land in their real trash.

`--keys` types into the app before the frame is drawn, so a dialog can be put on screen
and checked without a terminal: `--screenshot=110x24 --keys=Down,Insert,S-F8` renders the
permanent-delete warning. It takes mouse input too — `click:X:Y`, `rclick:X:Y`,
`ctrl-click:X:Y`, `drag:X1:Y1:X2:Y2`, `wheel:X:Y:N` — and redraws after each event, so
every click lands on what is actually on screen at that moment. **`wait`** waits for the
jobs started so far to end before the next input: there are only 30 ms between inputs, so
on a slow machine a copy still running when the next key came replaced that key's message
with its own "1 copied" (a Windows runner, 0.3.0's `ctrl_z_…` test).

**A wait in a pty test must look for something only the output can contain.** `seq 1
500` followed by `wait_for("500")` matched the typed command line and read the screen
before the output arrived. Echo a mark the command line cannot show:
`echo seq-done-$((6*7))` and wait for `seq-done-42` (cmd: `%OS:~0,0%`).

Mouse tests find their coordinates by locating text in a first screenshot (`locate` in
`tests/render.rs`), never by hardcoding them, so a layout change moves the clicks with
it instead of breaking every test.

`ops.rs` is tested against a real filesystem in a temp directory, including the cases
that matter: an existing destination is never overwritten without a question, cancelling
mid-file removes the half-written copy rather than leaving it looking complete, two
files of the same name both survive going to the trash, and copying a directory into
itself is refused instead of recursing until the disk fills.

Logic worth testing lives in `xdevcmd-core` precisely so it can be tested without a terminal.
Sorting, size/date formatting and name splitting all have unit tests — these are the
things that look trivial and are quietly wrong (`file2` vs `file10`, `archive.tar.gz`,
leading zeros). Each of those tests was written before the code was right, and three of
them caught a real bug on the first run.
