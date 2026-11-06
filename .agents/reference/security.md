# Security and secure modes

> **Read this when**: anything that types into a shell, puts text on the clipboard, runs a program, opens a file someone else made, or decides what read-only / restricted allow.

---

## Security

rcmd shows data other people chose (file names, file contents), runs programs (a shell,
an editor), and changes files with your permissions. Each of these was attacked on
2026-11-06 before being called safe. What held, what did not, and what changed:

| Attack | Before | Now |
|---|---|---|
| Escape sequences in a file name, to retitle the window or write the clipboard | held — ratatui drops control characters when it writes text | unchanged |
| A directory named `evil<Ctrl+U>touch PWNED<Enter>`, then Alt+C | **ran `touch PWNED`** — readline acts on Ctrl+U and Enter before the shell sees the quotes | refused: `safety::unsafe_to_type` |
| Alt+C while a program owns the terminal (an editor, a `sudo` password prompt) | typed the `cd` into it | refused, Linux and macOS (`/proc`, `ps`); Windows cannot tell yet |
| Copy path on a name with control characters, then paste into a terminal | would run, like typing it | refused |
| F3 on a FIFO | **froze the whole UI**: `open` on a FIFO blocks | refused from `stat`, before opening |
| F5 on a FIFO | **hung, and Esc could not cancel** a blocked read | refused: `entry::special_kind` |
| F3 on `/dev/zero`, or a link to it | indexing never ends | refused |
| A config file anyone can write (it names the shell and editor rcmd runs) | loaded | refused when world-writable (Unix) |
| `o` on a file named `a&calc.exe`, or a script | — | no shell anywhere in the path; programs ask first, Cancel by default |
| A filter hiding marked files, then F8 | — | hidden rows are unmarked |
| F9 with a file named `it's.txt` or one holding Ctrl+U | — | quoted per shell; control characters stop the command |
| Alt+F7 into a tree with a link to `/` or a FIFO | — | links listed, never followed; only regular files opened |
| A zip whose names climb out, carry links or set-id bits, or lie about their size | — | see Archives: hidden, skipped, dropped, stopped |
| A PNG header claiming 60000×60000 pixels | — | refused before decoding: 64 MiB file, 50 M pixel caps |
| git status in a repository with hostile config | — | off by default; fsmonitor disabled; filters cannot be, so documented |

Each row that was an attack is now a test in `tests/pty.rs`, and the command injection
test was proved by removing the guard: it reports `a command ran: [.../PWNED]`.

**Alt+C quotes per shell.** POSIX shells get `cd '…'` with `'\''`, fish escapes `\` and
`'`, PowerShell gets `Set-Location -LiteralPath '…'` with `''`, and cmd.exe gets
`cd /d "…"` — refusing a path with `%`, `!`, `^` or `"`, which cmd.exe cannot quote.
All of it ends with `\r`, the Enter key, which ConPTY expects too.

**Scanning.** `cargo deny check` (installed user-level with `cargo install --locked
cargo-deny`) checks dependencies against the RustSec advisory database, rejects any
licence not in `deny.toml` — a GPL or LGPL crate fails it, proved by adding
`sequoia-openpgp` — and allows crates.io only. CI runs it on every push and weekly,
because advisories are published after code is written. `#![forbid(unsafe_code)]` is
set in both crates: any unsafe code rcmd runs is in a dependency.

### Secure modes

| | `normal` | `read_only` | `restricted` |
|---|---|---|---|
| copy, move, delete, rename, mkdir, F4 edit, drop | yes | **no** | **no** |
| session file written on quit | yes | **no** — rcmd writes nothing | **no** |
| view (F3), navigate, copy path | yes | yes | view and navigate only |
| shell (F12, Alt+N, Alt+C), clipboard | yes | yes | **no** — no shell is ever started |

Set by `[security] mode` in the config or `--read-only` / `--restricted`. **A flag can
only tighten the config**: the effective mode is the stricter of the two, so a wrapper
that sets `restricted` cannot be escaped by adding `--read-only`. `[security] root` /
`--root` confine navigation to a directory, resolved through symlinks, so a link
inside the root pointing out of it is refused for both entering and F3; `--root` must
be inside a configured root. The root confines rcmd, not a shell — `restricted` is
what removes the shell.

**Two walls, each tested alone.** `App::refused` is the one place the UI decides: the
key bar hides what is off, the context menu greys it out, every input path — keys,
mouse, drag-and-drop, menus — asks it and says why. Behind it, `rcmd-core/policy.rs`
is installed once (`OnceLock`, so nothing can loosen it at runtime) and the
copy/move/delete engine, rename and mkdir refuse on their own. With the UI gate removed
on purpose, the UI tests fail and the pty test that presses F8, Shift+F8, F5, F6, F2
and F7 in read-only mode still finds the disk untouched.

The modes guard against mistakes and hostile content, not against the person running
rcmd: anyone can start it without the flag, or point `RCMD_CONFIG` elsewhere. A real
boundary is the operating system's — Landlock on Linux, the App Sandbox on macOS,
AppContainer on Windows.

**Not covered.** rcmd runs with your privileges and has no sandbox; a file you choose
to open in `$EDITOR` is that editor's problem. On Windows the busy-shell check and the
config permission check do nothing yet. Nothing here fuzzes the parsers.
