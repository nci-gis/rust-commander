# Roadmap

> What comes next, in order, and why that order. Latest release: **0.3.0** (2026-11-10);
> **v0.4.0** is in progress on `feat/v0.4.0`. Each line becomes a round in
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
- **The name, and who it is for** (owner, 2026-11-10). **xDev Commander**, `xdevcmd` to
  type. **x** = *extend* (it extends the shell, never replaces it: the shell keeps its
  keys, its history search and its completion) and *cross-platform* (Windows native, macOS,
  Linux). **Dev** = developers and devops: people who live in the terminal, build and
  deploy from it. **Commander** = the two-pane file manager family (Total Commander,
  Double Commander, One Commander, mc). The one-line description, chosen from six
  candidates scored on what each says (Round 017's Do): "xDev Commander: a cross-platform,
  two-pane terminal file manager that extends your shell."
- **What only xdevcmd can do is join the shell to the panes.** It holds the shell's output
  (in memory), knows the shell's directory and has file panes and a viewer in one
  process. v0.4 builds on that: the shell's output turned into files to act on (item 30).
- **The boundary for every idea: it ends in an action on a file.** A path in the shell's
  output is xdevcmd's business; a URL, a commit hash, a process id or log analytics are
  not (a log viewer or a plugin's).
- **Make it pleasant before making it bigger.** macOS's colours, keys and scrolling were
  broken in 0.1.0 and fixed in 0.2.x (items 1–2); themes and a Settings screen are what a
  first-time user touches. Polish still comes before any large feature.
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

## Where things stand

- **Released**: 0.2.0 (2026-11-07), 0.2.1 (2026-11-08), **0.3.0** (2026-11-10: GitHub
  release with six builds; `xdevcmd` and `xdevcmd-core` on crates.io). Rounds 001 and
  003–017 are Complete; 002 is Superseded. The history of each step is in its round and
  in `CHANGELOG.md`.
- **On `feat/v0.4.0`, at Review**: [Round 018](cycles/Round_018.md) (Windows asks before
  starting from its panel; the pid on the menu bar) and [Round 019](cycles/Round_019.md)
  (entry A below). Both are pushed without CI by the owner's choice (2026-11-10); CI runs
  at the pull request into `dev`.

## Next: v0.4.0, in this order

The detailed plan, with each entry's tests and risks:
`.agents/memory/2026-11-10-v0.4.0-plan.md`. **Each round starts with a kickoff review**
written into its Plan (what changed, the code it touches, an outside check, a critique,
steps and the owner's decisions), and the owner says go before any code.

1. **A. Foundation** (item 32, S–M), Round 019:
   - `app.rs` split into 15 files;
   - Windows end to end under ConPTY;
   - `cargo binstall` metadata;
   - this housekeeping.
2. **B. Paths in the shell's output** (item 30, part 1, M): marked in the history view,
   Enter goes there, F3/F4 open at the line.
3. **C. Panelize and find into a panel** (item 30, part 2, M).
4. **D. Syntax-highlighted viewer** (item 33, S–M): measure the binary-size cost first.
5. **E. One directory history** (item 29, S–M).
6. **F. Remote: the owner's decision first** (item 34). Either:
   - a panel over the user's own `ssh`, which needs hard rule 2 reworded; or
   - no remote, said plainly.
   Until decided, Not planned below stands.
7. **G. Settings: built-in and your own** (items 4c, 5b): in 0.4.0 or 0.5.
8. Optional if there is room: items 21 and 22.
9. **Release 0.4.0, then the launch** (item 10): a GIF and Show HN, after the packages.

**Packages** (item 8):
- crates.io: done with 0.3.0.
- `cargo binstall`: the metadata is in Round 019 and works from 0.4.0.
- Scoop and winget: templates in `packaging/`; submitting them is the owner's.
- Homebrew tap and AUR: the owner's call.

**Waiting on the owner**:

| Question | Why it matters | Suggested |
|---|---|---|
| Where do the tap and the bucket live? | Homebrew and scoop each read a separate repository | `nci-gis/homebrew-xdevcmd` and `nci-gis/scoop-xdevcmd`, created by the owner |
| AUR | needs a personal AUR account and SSH key | the owner's call; skip if no one maintains it |
| winget | a pull request to `microsoft/winget-pkgs`, reviewed by Microsoft | yes, after scoop |
| F: remote | the most asked-for gap, against hard rule 2 as worded | decide at F's kickoff |

**Releasing stays the owner's call**; the flow in `docs/development.md` is unchanged.

**Small fixes, no round of their own** — all done on `feat/v0.2.0` (2026-11-07):

- ~~`rcmd --screenshot … | head` panics~~: stdout goes through `emit()`, which ends
  quietly on a closed pipe; a test fails without it.
- ~~`rcmd --show-keys` has no automated test~~: a pty test (F7, Alt+J, a click, Esc Esc).
- ~~The Alt+R group's Esc path has no test~~: a render test.
- ~~rcmd keeps track of its own temporary files~~: `tempfiles.rs`, one `rcmd-run-<pid>`
  directory per run, user-only; a marker refreshed while running; removed on exit; at
  start, runs whose marker is over an hour old are removed (only marked directories).

Promotions are recorded in `promotions.md`; a memory still waiting on a person says so
in its own header.

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
| 8 | One name, then a Homebrew tap, scoop, winget, `cargo binstall`, AUR | S each | [011](cycles/Round_011.md), [017](cycles/Round_017.md), [019](cycles/Round_019.md) | name **xdevcmd** (011); crates.io with 0.3.0; binstall metadata in 019 (from 0.4.0); Scoop and winget templates in `packaging/`, submitted by the owner; tap and AUR the owner's call |
| 8b | Leftover safety items from the 0.2.0 review, and tests carried from 005 and 009 | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 7b | The About logo on macOS (and checked by hand on Windows) | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 4b | Example theme files in the release archives; `--print-theme` | S | [013](cycles/Round_013.md) | Complete (0.2.1); seven example themes since 0.3.0 |
| 4c | Settings → Theme: built-in and your own apart; "Make my copy" | S | — | v0.4 entry G, or 0.5 |
| 5b | Keymap presets (deferred in 004); Keys tab shows the preset's key and yours | M | — | v0.4 entry G, or 0.5 |
| 5c | Ctrl+G beside Ctrl+L to go to a path | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 14 | An app to click: launchers on Windows and macOS (14a); perhaps its own window (14b) | S / L–XL | — | deferred until there is a real need (owner) |
| 15 | Idle CPU near zero: redraw only on change (measured about 5 % of a core) | S–M | [013](cycles/Round_013.md) | Complete (0.2.1; 0.2–0.5 %) |
| 16 | Undo a delete to the trash (Linux, macOS, Windows); Linux trash by the spec, per volume | M | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 17 | Directory watching with the system's events, polling as fallback | M | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 18 | Copy/move "replace only if older" (robocopy) | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 19 | Fuzzy Ctrl+F filter, best first | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 20 | Sharp image previews: Kitty, iTerm2, sixel, half blocks as fallback | M | [014](cycles/Round_014.md) | Complete (0.3.0; our own code, not `ratatui-image`) |
| 21 | Sort names by Unicode, not ASCII-only folding | S | — | proposed |
| 22 | Faster copies: reflink, copy_file_range, clonefile, CopyFile2 | M | — | proposed |
| ~~23~~ | ~~Quit from the terminal panel~~ | — | — | dropped by the owner: inconsistent keys confuse more than they help |
| 24 | Save the session on every way out, signals included | S | [014](cycles/Round_014.md) | Complete (0.3.0) |
| 25 | Safe operations (a setting, on): temp-then-rename for every copy, a journal, clean-up and resume at the next start; proper quit waits; a closed terminal cancels, cleans, saves | M | [014](cycles/Round_014.md) | Complete (0.3.0) |
| 26 | Terminal copy mode: history in the viewer (Shift+F12), Save as… and Go to line in the viewer, `[terminal] scrollback`, the terminal tab's menu; keymap A (only the F12 family is xdevcmd's inside the shell) | M | [015](cycles/Round_015.md) | Complete (0.3.0) |
| 27 | Shell ↔ pane bridge: the shell's directory on every system, go there, type names into the command line, `[terminal] follow` | M | [016](cycles/Round_016.md) | Complete (0.3.0) |
| 28 | Release prep for 0.3.0: package `xdevcmd`, crates publishable, Scoop and winget templates | S | [017](cycles/Round_017.md) | Complete (0.3.0, published 2026-11-10) |
| 29 | One directory history for panes and shells | M | — | v0.4 entry E |
| 30 | The shell's output to files: in the history view, a path (with `:line[:col]`, Python's and JS's trace forms, MSBuild's `(line,col)`, Windows paths) that exists on disk is marked; Enter takes the pane to it, F3/F4 open it at that line; **panelize** turns chosen lines (or a command's whole output: `rg -l`, `git status`, `find`) into a panel for F5/F6/F8. Paths resolved against the shell's directory when the command ran (prompt marks), never trusted beyond moving a pane | M | — | v0.4 entries B and C, the lead feature (owner, 2026-11-10) |
| 31 | Windows: ask before starting from a terminal panel; the pid on the menu bar | S | [018](cycles/Round_018.md) | Review (on `feat/v0.4.0`) |
| 32 | Foundation: `app.rs` split, Windows end to end, `cargo binstall` metadata, canon housekeeping; CJK names fitted in columns (the owner's find) | S–M | [019](cycles/Round_019.md) | Review (on `feat/v0.4.0`) |
| 33 | Syntax-highlighted viewer (F3, quick view), off by setting and past a size cap | S–M | — | v0.4 entry D |
| 34 | Remote: a panel over the user's own `ssh`, or no remote said plainly | S–L | — | v0.4 entry F, the owner's decision first |
| 9 | Undo for the last operation | M | [009](cycles/Round_009.md) | Complete (2026-11-07) |
| 9b | The mouse in Settings (carried from 007) | S | [010](cycles/Round_010.md) | Complete (2026-11-07) |
| 9c | Click a directory in a pane's path; double-click a border to share it evenly | S | [012](cycles/Round_012.md) | Complete (2026-11-07) |
| 10 | Launch: Show HN with a GIF | S | — | after 0.4.0 and the packages |
| **Later: optional extras** | | | | |
| 11 | File icons as an optional extra: icon sets per extension, Nerd Font or plain | S–M | — | builds on 4 |
| 12 | Plugins v0: the user menu with documented variables and the selection, run in a tab | S–M | — | not for now (owner, 2026-11-09) |
| 13 | Plugins: external programs over stdin/stdout, versioned | L | — | not for now (owner, 2026-11-09) |
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
- The owner's word of 2026-11-09: **no SFTP; no plugins for now.** v0.4's entry F asks
  once more, for a panel over the user's own `ssh` only (xdevcmd opening no socket
  itself); until the owner decides, this stands.
