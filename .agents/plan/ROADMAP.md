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

1. **0.2.1: fixes found after 0.2.0** (items 7b, 4b, 8b; S each):
   - **The About logo is wrong on macOS** (owner, 2026-11-07). The 14-pixel logo was
     only checked on Linux, in the screenshot tests; never on macOS or Windows. Likely
     causes, to confirm with a screenshot from the owner: Terminal.app has no 24-bit
     colour, so the logo is drawn in 256 colours; and its line spacing can leave gaps
     between the half-block rows. Fix, then check by hand on Terminal.app, iTerm2,
     Windows Terminal and the old Windows console, and record each result.
   - **No theme files in the release archives** (owner, 2026-11-07). The five themes are
     built into the program, so they work, but a user who wants their own has nothing to
     start from. Ship a `themes/` folder of examples in each archive (they also work
     beside the binary, for a portable copy), and add `xdevcmd --print-theme <name>`,
     which prints a built-in theme as a file to edit.
   - **Leftover safety items** from the 0.2.0 review:
     - Bulk rename: rename without replacing, so a file that appears between the check
       and the rename is never overwritten.
     - Programs in a terminal tab: ignore `.` and empty entries in `PATH`, which would
       look for the program in the browsed directory.
     - Try the batch-file refusal on a real Windows machine (the owner's, or CI).
     - Carried from closed rounds: a test feeding a hostile file name through an
       `[[open]]` rule (005); undo of a move that met an overwrite question (009);
       Enter-opens-files tried by hand on Linux and Windows (005).
     Source: `.agents/memory/2026-11-07-security-review-before-0.2.0.md`.
   Done before the packages, so the first packaged version carries them. Work is in
   [Round 013](cycles/Round_013.md) on `feat/v0.2.1`.
   **Added to 0.2.1 by the owner** ("Yes, add both", 2026-11-07):
   - **Idle CPU** (item 15, S–M): measured at about 5 % of a core with nothing
     happening, because the screen is redrawn every 40 ms. Redraw only when something
     changed (a key, a resize, a job, the terminal tab's output, a listing refresh);
     target under 0.5 %. Matters on laptops and on servers with many sessions.
   - **Ctrl+G also goes to a path** (item 5c, S): Ctrl+L stays (the address bar key in
     browsers, Explorer and GNOME Files); Ctrl+G is added beside it (Go to folder in
     macOS Finder, and the habit of others). Checked free in the default keys.
2. **Sharp image previews** (item 20, M), the round after 0.2.1 (owner, 2026-11-08:
   "Yes, next round"): quick view and About draw real pixels where the terminal can,
   through `ratatui-image` (MIT), which detects Kitty graphics (kitty, Ghostty), iTerm2
   images (iTerm2, WezTerm, VS Code) and sixel (Windows Terminal, foot, xterm), and
   keeps today's half blocks everywhere else. macOS Terminal.app supports none of them
   and keeps the half blocks. To settle in the round: the picture redrawn only when it
   changes (these protocols do not fit ratatui's cell diffing), nothing drawn under an
   open dialog or over the terminal panel, resizing, detection over SSH and inside tmux,
   and a setting to turn it off. Starts when Round 013 is Complete.
3. **Packages** (item 8, S each), so nobody has to build from source:
   - `cargo binstall` metadata pointing at the release archives.
   - ~~Linux ARM (`aarch64-unknown-linux-musl`) added to the release builds~~: done
     early, in 0.2.1 ([Round 013](cycles/Round_013.md)). Windows ARM only if someone
     asks.
   - Homebrew tap, scoop bucket, winget manifest, AUR `xdevcmd-bin`, each filled from
     the release's `SHA256SUMS`.
   - The README's install section rewritten around them.
   Needs the owner first (see below): crates.io, the tap and bucket repositories, an
   AUR account.
4. **Launch** (item 10, S): a short GIF of the main moves, then a Show HN post. Only
   after 3, so a reader can install with one command.
   **Also in 0.2.1, from the algorithm review** (owner, 2026-11-08; details in
   `.agents/memory/2026-11-08-algorithm-review.md`): the cross-device move data-loss fix,
   Shift+F8 on Y only, whole-file replacement, per-question "all", safe unpacking, the
   Linux trash by the spec, undo of a delete to the trash (item 16), background compare
   and decoding, bounded directory sizes, `notify` watching (item 17), robocopy-style
   "replace only if older" (item 18) and a fuzzy filter (item 19).
5. **Settings shows built-in and your own, for themes and keys** (items 4c and 5b):
   - Theme tab: the list in two parts, built in and yours (with the folder each file
     came from), and a "Make my copy" button that writes the chosen theme into
     `themes/` (what `--print-theme` does) for editing. S.
   - Keys: the keymap presets deferred in Round 004 (`windows`, `macos`, `linux`,
     `classic`, perhaps `mc`), then the Keys tab as two columns, the preset's key and
     yours, with what you changed marked. M.
6. **An app to click, on Windows and macOS** (item 14): **deferred, only when there is
   a real need** (owner, 2026-11-07: "defered app cho windows và macOS, chỉ khi thực sự
   có nhu cầu"). If it comes, in two steps:
   - 14a, with the packages (S): Windows already runs `xdevcmd.exe` in a console
     window when it is double-clicked, with its icon; add a Start-menu shortcut
     through scoop and winget. macOS: an `xdevcmd.app` that opens the user's terminal
     running xdevcmd, with the icon, through a Homebrew cask. Both still draw in a
     terminal.
   - 14b, later and only if wanted (L–XL): xdevcmd in its own window, drawing the
     cells itself (a GPU window behind ratatui) instead of in a terminal. That would
     end every terminal difference (colours, keys, fonts) at once, but it is a second
     front end to keep up, and on macOS a downloaded app needs Apple's signing and
     notarisation (a paid developer account), on Windows a code-signing certificate,
     or users see warnings. Decide after the launch, from what users ask for.
6. Then items 11–13, each numbered when started. **Plugins are items 12 and 13**,
   low priority by the owner's choice (2026-11-06): first the user menu we already
   have (`[[menu]]`, Shift+F9) gets documented variables and the selection; separate
   plugin programs only for a need the menu cannot meet. No Lua, no Total Commander
   DLLs, nothing fetched (see Not planned).

**Waiting on the owner** before step 3:

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
| 8b | Leftover safety items from the 0.2.0 review, and tests carried from 005 and 009 | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1`; Windows by hand still open |
| 7b | The About logo on macOS (and checked by hand on Windows) | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1`, owner to test |
| 4b | Example theme files in the release archives; `--print-theme` | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1`, owner to test |
| 4c | Settings → Theme: built-in and your own apart; "Make my copy" | S | — | after the packages |
| 5b | Keymap presets (deferred in 004); Keys tab shows the preset's key and yours | M | — | after the packages |
| 5c | Ctrl+G beside Ctrl+L to go to a path | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1`, owner to test |
| 14 | An app to click: launchers on Windows and macOS (14a); perhaps its own window (14b) | S / L–XL | — | deferred until there is a real need (owner) |
| 15 | Idle CPU near zero: redraw only on change (measured about 5 % of a core) | S–M | [013](cycles/Round_013.md) | done on `feat/v0.2.1` (0.2–0.5 %), owner to test |
| 16 | Undo a delete to the trash (Linux, macOS, Windows); Linux trash by the spec, per volume | M | [013](cycles/Round_013.md) | done on `feat/v0.2.1`, owner to test on Mac and Windows |
| 17 | Directory watching with the system's events, polling as fallback | M | [013](cycles/Round_013.md) | done on `feat/v0.2.1` |
| 18 | Copy/move "replace only if older" (robocopy) | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1` |
| 19 | Fuzzy Ctrl+F filter, best first | S | [013](cycles/Round_013.md) | done on `feat/v0.2.1` |
| 20 | Sharp image previews: Kitty, iTerm2, sixel via `ratatui-image`, half blocks as fallback | M | — | next round, after 013 (owner, 2026-11-08); Terminal.app has none |
| 21 | Sort names by Unicode, not ASCII-only folding | S | — | proposed |
| 22 | Faster copies: reflink, copy_file_range, clonefile, CopyFile2 | M | — | proposed |
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
