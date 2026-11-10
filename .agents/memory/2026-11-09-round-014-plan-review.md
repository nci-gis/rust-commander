# Round 014: review of the plan and the branch, and a detailed plan for what is left

**Date**: 2026-11-09
**Agent**: Claude Code
**Confidence**: High for the findings marked *verified* (read in the code, or measured);
Medium for the external facts (sources below, checked on crates.io the same day);
Low where marked *to verify*
**Status**: New — every finding fixed on `feat/release-0.3.0` (2026-11-10, Round 017 Do);
D2–D4 settled by the work in Rounds 014–015
**Source**: the owner asked after a server reset: "dựng môi trường và phân tích kỹ lại plan
và hiện trạng code => lên plan chi tiết, phản biện, research cho những giải pháp tốt hơn".
Three passes: a map of the code the terminal-panel plan touches, a review of
`feat/image-preview` after 01ce5ac, and research on terminal emulation, copy mode, OSC 52,
memory and logs.
**Review-by**: 2026-12-15 (vt100 / tui-term releases move fast; recheck before acting)

## Update 2026-11-10 — rechecked on `feat/release-0.3.0` at 6dedc73

Rounds 014 (close-out), 015 (history in the viewer), 016 (shell bridge) and 017 (0.3.0
ready) are at Review. CI run 42733412500 on 6dedc73: green. Here: fmt, clippy `-D
warnings` on Linux/Windows/macOS targets, `cargo deny`, `cargo package --workspace` pass;
596 tests pass, 2 ignored.

| Item | State at 6dedc73 |
|---|---|
| Finding 1, vt100 freeze | fixed 2026-11-10 on `feat/release-0.3.0` (`3c11665`): vt100 0.18.1, tui-term dropped (Round 017, Do) |
| R1 resume, R2 liveness, R3 `kind` panic | fixed in Round 014 (`Existing::Resume`, flock / share_mode + host + run tag, `strip_prefix`), each with a test |
| R4 nested start | fixed `d9cc6ca` (tty compared; Windows notes instead of refusing) |
| R5 permissions | fixed `ae1a3b0` (0600 / 0700, `~` in crash logs) |
| R6 policy in recovery | fixed `1fa5bf8` (read-only and `--root` in core) |
| R7 probe in screen | fixed `2dd425e` (`graphics::multiplexer`; late replies read) |
| R8 F10 wait | fixed `e7b1e23` (`stop_step` in the main loop) |
| Step 1–4 of the terminal panel | done in Round 015 on 0.16 (`history_text` pages with `set_scrollback`); key is Shift+F12, not Alt+V |

## Where things stand (2026-11-09)

- Environment rebuilt: Rust 1.99.1 (MSRV in `Cargo.toml` is 1.88), targets
  `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`, `cargo-deny` 0.20.2.
- Baseline on `feat/image-preview` at 1c5bcd0, all green: `cargo fmt --check`; clippy
  `-D warnings` on Linux, Windows and macOS targets; `cargo deny check`; tests core 150,
  tui 140 + 150 render + 42 pty (1 ignored: the soak test), 1 doc.
- Round 014: pictures, safe operations, crash log, completions/man, Shift+F10 menu,
  Alt+X group, terminal naming, wheel forwarding, nested-start refusal are built. The
  terminal-panel plan (steps 1–5) is not started. CI has never run on the branch; the
  owner has not yet seen pictures on a real graphics terminal nor resumed a killed copy.

## Finding 1 — vt100 0.16.2 can freeze xdevcmd with 8 bytes (*verified*, measured)

`CSI 65535 @` (insert 65535 blanks) takes **676 ms** in vt100 0.16.2 and **2 µs** in
0.18.1 (scratch program, release build, 40×120). The panel's reader thread processes
output under the parser's **write lock** (`pane/term.rs:238`) and the draw takes a read
lock (`:537`), so the whole UI stops while it runs. An 8 KB file of these, `cat` in the
panel (or a hostile server's banner over `ssh`), freezes xdevcmd for about 11 minutes.
Upstream: vt100-rust issue #42, fixed by 0.17/0.18.

**This moves the vt100 upgrade to the front**, before any terminal-panel feature.

## Finding 2 — vt100 0.18 removes the plan's main constraint (*verified* on crates.io and the 0.18.1 CHANGELOG)

vt100 0.17.0 / 0.18.0 / 0.18.1 (18–19 Oct 2026, MIT) add:
- `Callbacks::on_scroll(row, alternate_screen)` — called when a row leaves the top of
  the scrollback. The plan's step 5 says "vt100 does not say when a line leaves its
  history"; it now does.
- Text capture: `write_contents_formatted_basic(.., Full)` writes scrollback + screen with
  wrapped rows joined; `capture::basic_formatted_to_plain` makes it plain text. Step 1 no
  longer needs to page through `set_scrollback` under the write lock (which also re-widens
  every row to full width — open PR #49 — undoing 0.18's memory saving).
- Rows trimmed of trailing blanks in scrollback: measured 9999 lines × 120 cols of ~60-char
  lines = 18 MB (0.16.2: 37 MB; 200 cols 62 MB → 18 MB). Full-width coloured lines are
  unchanged.
- `scrollback_len()`, `clear_scrollback()`, `CSI 3 J`, emoji width (VS16).
- Breaking: sizes are `NonZeroU16`; diff/format methods return `String`; `set_size` moves
  rows between screen and scrollback as xterm does.
- Still missing: changing the scrollback capacity after creation, reflow, search.

**Blocker**: `tui-term` 0.3.4 (latest) depends on `vt100 ^0.16.2` (optional feature). A
Dependabot PR to move it to 0.18.1 is open upstream (a-kenji/tui-term #384, 2026-11-01).
Also: `crates/xdevcmd-tui/Cargo.toml:31-34` says vt100 is not a direct dependency on
purpose, but line 38 has `vt100 = "0.16"` — the comment is stale either way.

## Finding 3 — the branch: what to fix before the owner tests (*verified* in the code)

| # | Severity | Where | What happens | Fix |
|---|---|---|---|---|
| R1 | Medium-High (hard rule 3) | `app.rs:5335-5352` `resume`; `journal.rs` records only kind, sources, dest, trash | "Continue" re-runs every copy/move with `Existing::Update`: an older destination file the user would have skipped is replaced without a question (a move then removes the source). `rename_to` is not journaled: resuming Shift+F5 "copy as" copies onto the same directory, hits `dst == src`, is skipped, and reports done — the copy never exists | journal `existing` and `rename_to`; resume with the original policy (falling back to Ask), skip only equal size+mtime; tests: an older different file survives, copy-as keeps its name |
| R2 | Medium | `journal.rs:164-213` `recover_in` / `alive` | liveness is the pid only. A home shared over NFS (two hosts, one `~/.local/state`): host B's start treats A's running job as dead, deletes A's live part files and renames back A's rename-many files mid-job. Windows: a write blocked > 120 s (`STALE`) and a second instance do the same. A reused pid keeps a dead journal "alive" forever | hold an exclusive lock on the journal file while the job runs; recovery acts only if it gets the lock. `File::try_lock` is std from **1.89** (MSRV is 1.88: raise it, or use `rustix::fs::flock`, already in the tree, plus `LockFileEx`-free Windows fallback of today's age rule); hostname in the journal name |
| R3 | Low-Medium (hard rule 4) | `journal.rs:233` `line[5..]` | a journal line that is exactly `kind` (cut-off write, hand edit) panics at start, before the journal is removed: **every start crashes** until the file is deleted by hand | `strip_prefix("kind ")`; test feeding `tidy("kind")` |
| R4 | Low-Medium | `pane/term.rs:186` sets `XDEVCMD_PANEL`, `main.rs:477` checks it | every child inherits it: `code .`, `kitty &`, Windows `start`/`wt` open new windows where xdevcmd is refused while the first one runs (Windows trusts the variable without checking the pid) | Unix: also export the pty's tty name and refuse only if `ttyname(0)` matches; elsewhere warn instead of exit; tests: dead pid and garbage value not refused |
| R5 | Low (personal data) | `crash.rs:55` `fs::write`; `journal.rs:71` no mode | crash logs (panic message, absolute paths incl. `/home/<user>`) and journals (full paths) are 0644 in a 0755 directory | 0600 files, 0700 `journal/`; `~` for `$HOME` in the crash text |
| R6 | Low (two walls) | `journal::tidy` | start-up recovery deletes/renames without `policy::guard_write` / `allows`: under `--root X` it touches paths outside X | check the policy per path |
| R7 | Low | `main.rs:388` | the probe is skipped only for `TMUX`; inside GNU screen the Kitty query (APC) becomes the hardstatus text. A reply arriving after 1.5 s (XTVERSION) is read by crossterm as Alt+P and letters typed into the shell | also skip for `STY` / `TERM=screen*`; after the deadline, drain ~200 ms and drop reply-shaped bytes |
| R8 | Low (UX) | `app.rs:2478-2525` | F10 with jobs: the 5 s wait is a sleep loop in the handler, so the screen freezes and the planned "stopping…" is never drawn | make it a state the loop draws |

Round items that the code shows **done** though the boxes are unticked: "Running jobs at
the end" (except R8 and Windows' close event, documented as needing `unsafe`) and "Safe
operations" (setting, journal, recovery, Continue, Help, docs) — the boxes are the
owner's to tick, after R1–R3.

## Finding 4 — the terminal-panel plan, step by step (critique)

**Step 1, history view (Alt+V)** — keep the idea (it is what kitty's `show_scrollback`
and Zellij's EditScrollback do, and better on privacy than Zellij's temp file); change
the details:
- Text from `write_contents_formatted_basic(Full)` + `basic_formatted_to_plain` (0.18),
  not page-by-page `set_scrollback` (needs the write lock, re-widens rows).
- **Alt+V clashes** (*to verify*): fish binds Alt+V to edit the command line (we believe;
  check `bind` in fish 3/4), and on macOS Option+V types `√` unless Option-as-Meta.
  Alternatives: also reachable from the tab menu (step 4), and a second key in the Alt+X
  group (Alt+X then V) which works everywhere. The key must be added to `is_global()`
  (`keys.rs:772-791`) or it goes to the shell.
- Limits to say in Help: while a full-screen program runs, vt100 reads the alternate grid
  (no history); in 0.16 lines scrolled while a scroll region is set were never kept
  (`grid.rs:566`) — *to verify* whether 0.18 still does that.
- `Document` (core) needs a `File | Vec<u8>` source: four read sites
  (`document.rs:92,143,165,215`) and `path` (title only). Small.
- The viewer is one modal (`app.rs:187`): its prompts (Save as, Go to line, overwrite
  question) must live inside `Viewer` as `/` does, or opening them loses the viewer.
- "Open at the bottom" means last line at the bottom; End/`G` today puts it at the top and
  the page height is known only after the first draw.
- `y` copies through OSC 52: whole history can be 0.6–2 MB of text (~0.8–2.7 MB base64).
  xterm's default limit is 100 000 bytes (*unverified*). Above ~75 KB, say so and offer
  Save as instead of sending.
- OSC 52 is missing in VTE terminals (GNOME Terminal, Tilix…) and needs
  `set -g set-clipboard on` inside tmux (its default `external` ignores applications).
  Proposed fallback with no new dependency: a `copy_command` setting (as Zellij), used
  only when not over SSH; on Windows PowerShell `Set-Clipboard` (clip.exe garbles UTF-8).

**Step 2, Save as (F2) and Go to line** — keep.
- Write 0600 for history, `create_new` / `rename_noreplace` for "never overwrite without
  asking" (`session::write_private` ends with a plain `rename`, which replaces). Through
  both walls (`App::refused`, `policy::guard_write` + `allows`).
- **Ctrl+G is already taken** in the viewer by accident: `KeyCode::Char('g')` ignores
  modifiers (`viewer.rs:241`), so Ctrl+G jumps to the top today. Match modifiers.
- Update F1's hand-written viewer key list (`help.rs:252-265`).

**Step 3, `[terminal] scrollback`** — keep the setting, simplify:
- Applies to shells started after the change (as tmux `history-limit` and kitty
  `scrollback_lines` do). Settings has no free number field: a `Kind::Pick` list
  (1000 / 2000 / 5000 / 9999) with `with_own` for a hand-written value.
- With 0.18 the memory note can say "up to ~38 MB per shell at 120 columns, usually much
  less" — the 32 bytes/cell worst case still holds for full-width lines.
- Better later, not now: kitty's two tiers (a modest live vt100 scrollback for the wheel,
  older lines kept as text through `on_scroll` — 30–60× smaller). It also makes step 5
  cheap. Only if 9999 lines turns out too little.

**Step 4, tab menu** — keep. Needs `TermPanel::tab_at(x, y)`, a right-click branch for the
terminal before the files-pane branch (`app.rs:1967`), and `Cmd` variants that carry the
tab index (`Cmd::Act(Action)` cannot).

**Step 5, logging** — the plan's sound version (strip escapes from the raw stream) is
PuTTY's "printable output", which logs backspaces and redraws as garbage. With 0.18:
log rows from `on_scroll` (rendered text, skipping `alternate_screen` rows), and the rest
with `Full` when the tab closes. Off by default, per tab, a visible mark, 0600
`create_new` in a 0700 directory, size cap. Caveat: a crash loses what had not yet left
the scrollback. Still the owner's decision.

## Recommended order

| Phase | What | Size | Needs |
|---|---|---|---|
| A | R3, R5, R1, R2, R6, R7, R4, R8 — each its own commit with a test that fails without it | M (2–3 days) | decision D3 |
| B | vt100 0.18.1 (fixes Finding 1); run every pty test; recheck wheel, mouse forwarding, resize | S–M | decision D1 |
| C1 | `Document` in-memory source + history view + its key(s) | M | D2 |
| C2 | Save as + Go to line (+ the Ctrl+G modifier fix) | S | — |
| C3 | `[terminal] scrollback` for new shells | S | — |
| C4 | tab right-click menu | S | — |
| D | CI once (Linux, macOS, Windows, ARM), owner's manual tests (pictures, kill+resume, safe operations), round closed by the owner, PR into `dev`, 0.3.0 | — | owner |
| later | step 5 logging, copy-command fallback, two-tier scrollback | — | owner |

A before B because the owner's resume test (D) should not run on R1; B before C because
step 1 is built on 0.18's capture API.

## Decisions for the owner

- **D1 — how to get vt100 0.18** while tui-term pins 0.16: (a) wait for tui-term with
  0.18 (#384, open), using the time for phase A; (b) depend on tui-term with
  `default-features = false` and implement its public `Screen`/`Cell` traits on a newtype
  over vt100 0.18 (*verified*: `PseudoTerminal<'a, S: Screen>` in `widget.rs:114,228`;
  the vt100 impl is behind the default `vt100` feature, `vt100_imp.rs`; a newtype is
  needed because both trait and type are foreign);
  (c) draw the vt100 screen ourselves (tui-term's widget is small) and drop tui-term.
  Recommended: (b), falling back to (c); (a) leaves the 676 ms freeze in.
- **D2 — the history key**: Alt+V (plan) plus Alt+X then V and the tab menu, or another key.
- **D3 — R2's lock**: raise MSRV to 1.89 for `File::try_lock`, or `rustix::fs::flock` on
  Unix and keep the age rule on Windows.
- **D4 — OSC 52 fallback**: a `copy_command` setting (no dependency), or nothing (Save as
  is the way out).

## Evidence

- Code: paths and lines above, at 1c5bcd0.
- Measured: `scratchpad/dos` (vt100 0.16.2 vs 0.18.1, `CSI 65535 @`); memory by a counting
  allocator (research pass), 40 rows, 20 000 lines fed, scrollback 9999.
- External: crates.io versions of vt100 and tui-term (checked 2026-11-09);
  vt100 0.18.1 CHANGELOG; https://github.com/doy/vt100-rust (issue #42, PRs #49, #50);
  tui-term PR #384; tmux clipboard https://github.com/tmux/tmux/wiki/Clipboard;
  Zellij options https://zellij.dev/documentation/options.html; kitty
  https://sw.kovidgoyal.net/kitty/conf/; Windows Terminal historySize
  https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-advanced;
  VTE and OSC 52 https://gitlab.gnome.org/GNOME/vte/-/issues/2495; PuTTY logging
  https://the.earth.li/~sgtatham/putty/0.83/htmldoc/Chapter4.html.

## Promotion candidate?

- [ ] `context/` — "pin an emulator crate's version only with a reason, and recheck it each
      round" is a candidate after the upgrade lands
- [x] Not yet — decisions pending
