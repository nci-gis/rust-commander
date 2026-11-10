# Final review before publishing 0.3.0 and moving to v0.4: plan, readiness, market

**Date**: 2026-11-10
**Agent**: Claude Code
**Confidence**: High for the repository facts (read at `ecc2f85`); Medium for the market
(GitHub API and Homebrew analytics read the same day, HN threads; Reddit unreachable);
Low where marked *unverified*
**Status**: New — for the owner's decisions (end of file)
**Source**: the owner, 2026-11-10: "push xong đánh giá lại plan và điểm mạnh cạnh tranh trên
thị trường của xdevcmd. Đây là vòng đánh giá cuối trước khi move qua v0.4.0 để publish."
Two passes: an inventory of the repository (features, roadmap, open items, release
mechanics, tests, code size) and a market study of terminal file managers in 2026.
**Review-by**: 2027-02-01 (the market moves; stars and releases go stale in months)

## 1. Where 0.3.0 stands

`feat/release-0.3.0` at `ecc2f85`, pushed. It carries Rounds 014–017 stacked (each at
Review, none merged): `origin/dev` is 109 commits behind. 86 key actions; 617 tests
(core 171, unit 191, render 166, pty 86, packaging 3); CI matrix Linux x86_64 + ARM,
Windows, macOS, cross-checks, cargo-deny, `cargo package`.

### Before publishing — blockers and near-blockers

| # | What | Why | Who | Size |
|---|---|---|---|---|
| P1 | **Run CI on `ecc2f85`** | The last green Windows/macOS run is `e672c90`; the 14 commits since (vt100 0.18 and `draw_screen`, `ttyname` + rustix "fs", the screen probe, quit in the loop, journal modes, the `\\?\` fix, wrapping) were compiled for Windows/macOS by clippy here, never run there. The `\\?\` fix is only checked on Windows | owner (or agent on the owner's word) | — |
| P2 | **README links on crates.io** | README is the crate's readme and uses relative links (`assets/logo-*.svg`, `docs/*.md`, `#macos` anchors): broken on crates.io, the first page a `cargo install` user sees (Round 017 risk, unfixed) | agent | S |
| P3 | **README promises** | Scoop and winget are named "coming with 0.3.0" before the bucket repo and the winget package exist | owner trims at release, or agent rewords now | S |
| P4 | **Smoke by hand on Windows and macOS** | Windows has no end-to-end test (pty tests are Unix only); pictures, ConPTY + OSC 7/133, PowerShell/cmd/fish snippets, macOS `lsof`, hang-up, `ttyname` never seen on a real machine | owner | — |
| P5 | **Close Rounds 014–017 and merge** | "One round at a time" was set aside four times in a row; PR chain into `dev`, then `cargo release minor --no-publish` (its full dry run never ran: sandbox network) | owner | — |
| P6 | Recheck crate names; delete `ci/scratch` | names were free on 2026-11-10; the scratch branch is still on the remote | owner | — |

### After publishing — housekeeping (owner: these files are not the agents' to edit)

- `ROADMAP.md` is stale in many places: header "now 0.2.0"; macOS "broken in three ways"
  (fixed in 0.2.x); two steps numbered 6; Rounds 015–017 and 0.3.0 missing from "Next";
  item 20 "in progress, local only"; 24–26 "(local)" though pushed; 8b/7b/4b/5c/15/16
  "owner to test" though 0.2.1 is published; "both crates say `publish = false`" (now
  true); binstall/Homebrew/AUR in step 3 but excluded by Round 017; pending promotions
  that `promotions.md` already records; plugins (12, 13) both "later" and "not for now".
- Read-only canon out of date: `reference/ui.md:23,35,153,189` (global Alt keys, replaced
  by keymap A); `reference/gotchas.md:93,96-100` (tui-term pins vt100; `Parser` has no
  `set_size` — both false since `3c11665`).
- Stale boxes: Round 014 Plan items "running jobs at the end" and "safe operations"
  shipped but unticked; Round 017's Do/Check lacks `d23b7dd` and `ecc2f85`.
- `docs/development.md:118` still says `git push origin v0.2.0`.

## 2. The market (2026-11-10)

Sources and the full table: the research pass; key URLs at the end.

| Tool | Two-pane | Native Windows | One static binary | Shell | Remote | Safety | Stars / brew installs 30 d |
|---|---|---|---|---|---|---|---|
| yazi (MIT, Rust) | no (Miller columns) | yes, needs `file(1)` | yes | cd-on-quit | SFTP VFS | trash | 43.3k / 5,711 |
| superfile (MIT, Go) | multi-panel | "not fully supported" | yes | — | no (top request) | trash, no undo | 24.0k / 430 |
| nnn (BSD, C) | no | WSL | yes | spawn shell | sshfs | trash | 22.1k / 357 |
| broot (MIT, Rust) | panels | yes | yes | alt+enter cd | no | — | 13.1k / 1,607 |
| mc (GPL, C) | yes | unofficial port | no | subshell, Ctrl+O | SFTP/FTP VFS | no trash (asked since 2013) | mirror / 989 |
| Far Manager (BSD) | yes | Windows only | portable dir | command line | SFTP plugin | — | 2.3k |
| far2l (GPL) | yes | no | AppImage | built-in VT | NetRocks (SFTP, SMB, S3…) | — | 2.2k |
| vifm (GPL) | yes | yes, binaries | AppImage | `:!` | FUSE | undo, trash | 3.3k / 276 |
| MiddayCommander (MIT, Go, 2025–26) | yes, mc-style | yes | yes | Ctrl+O | SFTP panels | confirms | 93 |
| tfm-tui (MIT, TS, 2026) | yes | Linux only | binary | embedded terminal | "network" | trash, undo | 356 |

What users say (HN, GitHub issues): mc is loved for the shell toggle, names into the
command line, **Panelize** ("worth the price of admission") and SFTP; hated for
non-rebindable keys, Esc delay, Tab stolen from the shell, no trash, "maintenance mode".
New tools are judged on first-run friction (superfile's HN thread: binary name, deletes
"in minutes", themes fetched from the network, Nerd Fonts, glibc) and, since 2026, on
**data safety and tests** (an AI-written two-pane clone was taken apart for "no tests"
and "catastrophic data loss").

### xdevcmd's edge: real, crowded, overclaimed

- **Real, defensible**: one MIT program with the **same keys on Windows, macOS and Linux**,
  two panes, one static binary down to Linux ARM musl. Far is Windows only, far2l not on
  Windows, mc on Windows unofficial, yazi not two-pane (and needs extras on Windows),
  superfile weak on Windows. The closest analogue is MiddayCommander (MIT, Go, two-pane,
  Windows, SFTP) — tiny today, worth watching.
- **Real, rare**: **crash-safe operations** — temp-then-rename for every copy, a journal,
  resume after a kill (none found in competitors' docs, *unverified*); plus undo of a
  delete to the trash, read-only/`--root`, no network. Undo alone is not unique (vifm,
  felix, tfm-tui).
- **Real, new**: the **shell ↔ pane bridge** (OSC 7/133 follow, marked names typed at the
  prompt only when it is idle, the history in the viewer). An embedded shell is not new
  (mc subshell, far2l VT, Far command line, tfm-tui); the bridge appears to be
  (*unverified*).
- **Crowded**: image previews (yazi leads), themes, fuzzy filter, archives.
- **Overclaimed**: "Total Commander's feature set" while there is **no remote file system**
  — mc, Far, far2l, yazi and MiddayCommander all browse SFTP. It is the most requested
  gap in yazi (#611) and superfile (#134), and the main reason an mc or Far user would
  not switch.

### Positioning

Lead: **"Total Commander for the terminal — one MIT binary, the same keys on Windows,
macOS and Linux servers."** Proof line: **"It never loses a file: journaled copies that
resume after a kill, undo, read-only and root modes, no network."** The shell bridge is
the third line now and becomes the headline once v0.4's panelize lands.

## 3. Critique of the plan for v0.4

What ROADMAP has: item 30 (shell output to files: paths in the history view, panelize)
as the lead, 29 (one directory history), 21 (Unicode sort), 22 (fast copies), 4c/5b
(Settings themes and keys), packages, launch.

- **Keep 30 as the lead.** It upgrades mc's most-praised feature (Panelize) and makes
  the bridge demonstrable. Make sure it covers a built-in find/grep straight into a
  panel, not only the shell's output.
- **Decide remote deliberately; do not leave it as "not planned" by default.** Hard rule
  2 says no network calls, and ROADMAP says no SFTP. Options that respect the rule's
  intent: (a) a panel over the user's own `ssh` (xdevcmd spawns the system `ssh`/`sftp`
  the user invoked, as mc's shell link does; nothing is fetched, no telemetry) — the rule
  would need its wording changed by the owner; (b) keep "no remote" and say plainly in the
  README that `ssh` in the terminal panel plus a copy of xdevcmd on the server is the
  answer. Either is defensible; silence is not, because it is the first question the
  launch thread will ask.
- **Syntax-highlighted viewer, with search matches highlighted**: cheap, visible in the
  GIF, closes the gap with yazi. Watch binary size (syntect's syntax sets are large);
  measure before choosing.
- **Before any of it: split `app.rs`** (7,729 lines, ~30% of the crate, 206 functions).
  Every v0.4 feature lands there; a split along the existing seams (dispatch, jobs,
  modals, settings, graphics) is cheaper now than after 30.
- **Windows needs an end-to-end test path**: it is a headline platform with render tests
  only. A ConPTY-based harness, or at least a scripted smoke in CI that starts the binary
  under ConPTY and reads a frame.
- **Packages before the launch** (as ROADMAP says): a Homebrew tap at 0.3.0; Homebrew core
  needs 225 stars when the owner submits (75 if a user does) and a repository 30 days old.
- **Launch**: a 20–30 s GIF — two panes, the shell below, Alt+G, names typed into the
  shell, then a copy killed and resumed (nobody else can show that); lead the Show HN with
  the safety and the 600+ tests; answer superfile's first-run complaints in the README
  (one binary named `xdevcmd`, no Nerd Font needed, nothing fetched, deletes confirmed).

## Decisions for the owner

1. Run CI on `ecc2f85` now (P1).
2. The version: publish what is on this branch as **0.3.0** (CHANGELOG, README, Round 017
   and `cargo release minor` all say 0.3.0) and start v0.4 after — or skip to 0.4.0.
3. Remote: (a) a panel over the user's `ssh` with hard rule 2 reworded, or (b) "no remote",
   said plainly.
4. v0.4 order: proposed `app.rs` split → 30 (panelize, find into panel) → syntax viewer →
   29; packages and launch around 0.4.

## Evidence

- Repository: `ecc2f85`; ROADMAP lines and Round files as cited above; test counts by
  `#[test]` per file; `wc -l` per crate.
- Market: GitHub API and `formulae.brew.sh` analytics, 2026-11-10;
  https://news.ycombinator.com/item?id=45271481 (mc, 2025),
  https://news.ycombinator.com/item?id=40318714 (superfile),
  https://news.ycombinator.com/item?id=47734912 (AI-written clone),
  https://github.com/sxyazi/yazi/issues/611, https://github.com/yorukot/superfile/issues/134,
  https://github.com/MidnightCommander/mc/issues/3072, https://github.com/kooler/MiddayCommander,
  https://github.com/vifm/vifm, https://github.com/elfmz/far2l,
  https://docs.brew.sh/Package-Acceptance-Policy.

## Promotion candidate?

- [ ] `context/` — the positioning lines, once the owner settles them
- [x] Not yet — decisions pending
