# QA brief (xdevcmd, before 0.4.0-beta.2)

xdevcmd is a two-pane terminal file manager (Rust). Repo: /home/claude/rust-commander
(read-only for you: do NOT edit, commit, push or run `git` commands that change anything).

The oracle is the USER DOCS, not the code: docs/guide.md, docs/security.md,
docs/shell.md, docs/config.md, docs/troubleshooting.md, docs/how-to-test.md, README.md.
An expectation must come from a doc line (cite file:line). Where the docs say nothing,
use what a Total Commander / Midnight Commander user would assume, and mark it
"undocumented".

A test PROVES an expectation only if it (1) drives the user's path (keys/clicks through
`--screenshot=WxH --keys=...` or the pty harness; not an inner function), (2) checks the
outcome the user cares about — the disk first (files, bytes, the trash), the screen
second — and (3) would fail on a build with the bug. A test that only matches text the
program prints CONFIRMS, it does not prove. Unit tests of inner functions CONFIRM unless
the function IS the whole user-visible behaviour.

Test files: crates/xdevcmd/tests/render.rs (screenshot mode: `--screenshot=110x24
--keys=Down,F5,Enter,wait`; `wait` lets a job finish), crates/xdevcmd/tests/pty.rs (a
real pty, with a shell), crates/xdevcmd/tests/windows_e2e.rs, unit tests under
crates/*/src (`#[cfg(test)]`), xtask/src/tryout.rs (`xtask try`, a kill-and-resume
sandbox).

Prebuilt binaries (do not run cargo build): /tmp/qa/bin/xdevcmd, /tmp/qa/bin/xtask.
`xdevcmd --help` lists flags; keys: docs/keys.md.

Sandbox rule for anything you run: work only under /tmp/qa/<your-area>/; set
XDEVCMD_CONFIG=<sandbox>/config.toml (or a missing file), XDEVCMD_STATE=<sandbox>/state/session.toml,
XDG_DATA_HOME=<sandbox>/data, HOME=<sandbox>/home, and run from inside the sandbox.
Never touch files outside /tmp/qa, never the real trash. The machine has 2 cores and
other agents run beside you: allow for slowness; never kill processes you did not start.

Screenshot mode example:
  cd $SB && XDEVCMD_CONFIG=$SB/none.toml XDEVCMD_STATE=$SB/state/session.toml \
  XDG_DATA_HOME=$SB/data HOME=$SB/home /tmp/qa/bin/xdevcmd --screenshot=110x24 \
  --keys=Down,F5,Enter,wait $SB/a $SB/b
For live interaction (kills mid-job, shell typing): tmux (new-session -d, send-keys,
capture-pane -p), with the same environment.

Labels for every claim: REPRODUCED (you ran it; give the exact commands and output),
READ (file:line), SUSPECT (unverified). Be concrete and brief. Do not pad.
