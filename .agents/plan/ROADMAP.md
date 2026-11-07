# Roadmap

> What comes after 0.1.0 (and now 0.2.0), in order, and why that order. Each line becomes a round in
> `cycles/` when work on it starts; a round says how, this file says what and when.
> Reorder freely; strike through what is dropped rather than deleting it.

Written 2026-11-06 from feedback on 0.1.0, a market study the same day (five research
threads, kept outside the repository) and the owner's review of both. The feedback:
Windows and Ubuntu look fine; on macOS the colours are wrong, Alt shortcuts and some
other keys do nothing, and scrolling shows the terminal's history; Total Commander
users want Enter to open files.

---

## Direction

- **xdevcmd's edge (rcmd until 2026-11-07) is a combination, not a feature**: MIT, the Total Commander model,
  native Windows, one static binary to drop on a server, no network, nothing destructive
  done silently. Nothing below thins it.
- **Make it pleasant before making it bigger.** macOS is a third of the platforms and
  is broken in three ways; themes and a Settings screen are what a first-time user
  touches. These come before any large feature.
- **Plugins are for optional extras** (uncommon or nice-to-have features), not the
  core. Low priority, and no in-app fetcher.
- **No SFTP** (owner, 2026-11-06): `ssh` in the terminal panel and the many good
  existing tools cover it.

## Working rules

Agreed with the owner on 2026-11-07, after Round 002 sat in Planning while 003 and 004
were done first:

- **A round gets its number when its work starts**, not when it is first imagined. Ideas
  wait here, in the table below, without a round file.
- **Rounds run in number order, one at a time.** A new round starts only when the one
  before is Complete or explicitly set aside (said in its Do section, with why).
- Round 002 was numbered under the old habit and then waited. At the owner's request it
  is **Superseded** and its plan moved to Round 005, so numbers and order agree again.
  Numbers are never changed afterwards: rounds are append-only (`PDCA.md`).
- Each round works on its own `feat/…` branch and lands in `dev` by pull request.

## Next, in this order

**0.2.0 is out** (published by the owner on 2026-11-07). Rounds 001 and 003–012 are
Complete; 002 is Superseded. Written the same day at the owner's request ("tạo plan cho
những việc kế tiếp"), in this order:

1. **Leftover safety items** (item 8b, S). Small, and best done before the packages, so
   the first packaged version carries them (0.2.1):
   - Bulk rename: rename without replacing, so a file that appears between the check
     and the rename is never overwritten.
   - Programs in a terminal tab: ignore `.` and empty entries in `PATH`, which would
     look for the program in the browsed directory.
   - Try the batch-file refusal on a real Windows machine (the owner's, or CI).
   - Carried from closed rounds: a test feeding a hostile file name through an
     `[[open]]` rule (005); undo of a move that met an overwrite question (009);
     Enter-opens-files tried by hand on Linux and Windows (005).
   Source: `.agents/memory/2026-11-07-security-review-before-0.2.0.md`.
2. **Packages** (item 8, S each), so nobody has to build from source:
   - `cargo binstall` metadata pointing at the release archives.
   - Linux ARM (`aarch64-unknown-linux-musl`) added to the release builds; servers
     and Raspberry Pi need it. Windows ARM only if someone asks.
   - Homebrew tap, scoop bucket, winget manifest, AUR `xdevcmd-bin`, each filled from
     the release's `SHA256SUMS`.
   - The README's install section rewritten around them.
   Needs the owner first (see below): crates.io, the tap and bucket repositories, an
   AUR account.
3. **Launch** (item 10, S): a short GIF of the main moves, then a Show HN post. Only
   after 2, so a reader can install with one command.
4. Then items 11–13, each numbered when started.

**Waiting on the owner** before step 2:

| Question | Why it matters | Suggested |
|---|---|---|
| Publish `xdevcmd` on crates.io? | `cargo install xdevcmd` works, and the name is held; today both crates say `publish = false` and the name is still free | yes, before the launch; needs a crates.io token kept outside the repository |
| Where do the tap and the bucket live? | Homebrew and scoop each read a separate repository | `nci-gis/homebrew-xdevcmd` and `nci-gis/scoop-xdevcmd`, created by the owner |
| AUR | needs a personal AUR account and SSH key | the owner's call; skip if no one maintains it |
| winget | a pull request to `microsoft/winget-pkgs`, reviewed by Microsoft | yes, after scoop |

**Releasing stays the owner's call**; the flow in `docs/development.md` is unchanged.

**Small fixes, no round of their own** — all done on `feat/v0.2.0` (2026-11-07):

- ~~`rcmd --screenshot … | head` panics~~: stdout goes through `emit()`, which ends
  quietly on a closed pipe; a test fails without it.
- ~~`rcmd --show-keys` has no automated test~~: a pty test (F7, Alt+J, a click, Esc Esc).
- ~~The Alt+R group's Esc path has no test~~: a render test.
- ~~rcmd keeps track of its own temporary files~~: `tempfiles.rs`, one `rcmd-run-<pid>`
  directory per run, user-only; a marker refreshed while running; removed on exit; at
  start, runs whose marker is over an hour old are removed (only marked directories).

Waiting on a person, at any point: the promotions proposed in
Rounds 001 and 003–009; the read-only finding in
`.agents/memory/2026-11-07-read-only-writes-a-temp-copy.md`.

## Order

| # | Item | Size | Round | Status |
|---|---|---|---|---|
| **Now: fix what is broken** | | | | |
| 1 | Colours on terminals without 24-bit colour (macOS Terminal.app) | S | [001](cycles/Round_001.md) | Complete (2026-11-07) |
| 2 | macOS keys and mouse: Alt shortcuts, other keys, scrolling into the terminal's history | S–M | [003](cycles/Round_003.md) | Complete (2026-11-07); Mac history and resize keys moved to 004 |
| **Next: Total Commander feel** | | | | |
| 3 | Enter opens a file with the program its extension belongs to, like Total Commander | M | [005](cycles/Round_005.md) (was [002](cycles/Round_002.md), superseded) | Complete (2026-11-07) |
| 4 | Themes as files: built-in themes plus the user's own, chosen by name, switchable while running | M | [006](cycles/Round_006.md) | Complete (2026-11-07) |
| 5 | Keys that work on every system: two-key bindings (Alt+R then arrows), Alt+J history, Alt+1…9 tabs, clash checks; Esc as Meta removed. Named keymap sets deferred | M | [004](cycles/Round_004.md) | Complete (2026-11-07) |
| 6 | Settings (File → Settings…): tabs General, Files, Theme, Terminal, Keys | L | [007](cycles/Round_007.md) | Complete (2026-11-07) |
| 7 | The logo in the app: About screen (and the empty state) drawn with the picture renderer | S | [008](cycles/Round_008.md) | Complete (2026-11-07; About only) |
| 8 | One name, then a Homebrew tap, scoop, winget, `cargo binstall`, AUR | S each | [011](cycles/Round_011.md) | name: **xdevcmd**, renamed in 011, released as 0.2.0; packages after 8b |
| 8b | Leftover safety items from the 0.2.0 review, and tests carried from 005 and 009 | S | — | next |
| 9 | Undo for the last operation | M | [009](cycles/Round_009.md) | Complete (2026-11-07) |
| 9b | The mouse in Settings (carried from 007) | S | [010](cycles/Round_010.md) | Complete (2026-11-07) |
| 9c | Click a directory in a pane's path; double-click a border to share it evenly | S | [012](cycles/Round_012.md) | Complete (2026-11-07) |
| 10 | Launch: Show HN with a GIF | S | — | after 1–8 |
| **Later: optional extras** | | | | |
| 11 | File icons as an optional extra: icon sets per extension, Nerd Font or plain | S–M | — | builds on 4 |
| 12 | Plugins v0: the user menu with documented variables and the selection, run in a tab | S–M | — | low priority |
| 13 | Plugins: external programs over stdin/stdout, versioned | L | — | low priority, only for a need 12 cannot meet |
| ~~—~~ | ~~SFTP, remote panes, a file-source layer for remote writes~~ | — | — | dropped 2026-11-06 |

Sizes are relative: **S** a day or two, **M** under a week, **L** one to two weeks.

## Notes on the items

- **4 Themes.** `themes/<name>.toml` in the config directory, each a full palette in
  the slots `--print-config` lists; `[theme] name = "…"` picks one and the existing
  per-colour settings still apply on top. Built in: `dark` (today's), `light`,
  `ansi` (the 16 terminal colours, so it follows the user's palette), `blue` (blue
  panels in the tradition of the DOS-era commanders), `high-contrast`. A theme also
  says how it degrades at 256 and 16 colours (item 1). Today the theme is set once at
  start-up and "never changes while running"; switching needs that to change.
- **5 Keymaps and sequences**: see [Round 004](cycles/Round_004.md). Changed on
  2026-11-07 from `classic` / `laptop` / `mc` to one set per system, after Round 003
  found that macOS Terminal.app drops the modifiers of most arrow keys, and Windows
  Terminal is believed to keep Alt+arrows for itself. `mc` may come later.
- **6 Settings.** Tabs: **General** (confirmations, trash, session, Esc as Meta),
  **Theme** (pick a theme and the colour depth, with a live preview), **Keys** (pick a
  keymap; the list of keys, editing them later), **Panels and layout** (how many file
  panes, the terminal panel). Security shows the mode but is not editable from inside,
  or a restricted session could lift its own restriction. Writes `config.toml` keeping the user's comments and order. Refused in
  read-only and restricted modes, where xdevcmd writes nothing.
- **7 Logo.** Quick view already draws PNG and JPEG with half blocks, two pixels a cell,
  in any terminal: the icon can be an embedded PNG drawn the same way. At 256 colours
  it is quantised like everything else.
- **11 File icons** already exist as `[files] icons = true` (needs a Nerd Font). The
  extra is choosing the set (Nerd Font, plain Unicode, none) and per-extension icons in
  a theme-like file.

## Decisions

The owner accepted every recommendation below on 2026-11-06 ("mình đồng ý theo đề xuất
của bro"). The one name was chosen on 2026-11-07: **xdevcmd**.

| Decision | Options | Decided |
|---|---|---|
| Colour `auto` | downgrade only old Terminal.app / truecolor only on positive evidence | positive evidence; 256 colours over SSH unless `COLORTERM` arrives |
| Default theme | `dark` (today) / `ansi` | keep `dark`; revisit after the launch |
| Enter on a file | `view` / `open` (Total Commander) | `open` by default, falling back to view where nothing can be opened; `view` by setting |
| Where theme files live | config directory / also beside the binary (portable, as Total Commander's ini) | config directory; also read `themes/` beside the binary for portable installs |
| Name | keep `rcmd` + `rust-commander` / one name | **xdevcmd** (2026-11-07): command, package, config paths, environment variables. The repository keeps `rust-commander`. `rust-commander` was taken on crates.io by another TUI file manager and `rcmd` by a 2020 crate and a Homebrew cask; xdevcmd was free everywhere checked (crates.io, Homebrew, AUR, scoop, Debian, npm, PyPI, GitHub) |
| Extension points in secure modes | — | off in read-only and restricted unless whitelisted |

## Not planned

- **SFTP and remote file systems** (see Direction). Round 003's first draft, a spike
  for this, was replaced by the macOS round before it started.
- **Fetching anything** — a plugin list, updates, telemetry. Hard rule 2 stays as it is.
- **Total Commander's native plugins** (WFX/WCX DLLs) and **embedded Lua**: the first
  needs `unsafe` and runs Windows-only binaries in-process; the second runs plugin code
  in-process with no boundary.
