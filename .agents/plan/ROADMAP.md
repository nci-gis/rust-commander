# Roadmap

> What comes next, in order, and why that order. Latest release: **0.3.0** (2026-11-10);
> **v0.4.0, the first stable release**, is in progress on `feat/v0.4.0`. Each line
> becomes a round in `cycles/` when work on it starts; a round says how, this file says
> what and when. Reorder freely; strike through what is dropped rather than deleting it.

Written 2026-11-06 from feedback on 0.1.0 and a market study; **rewritten 2026-11-12**
around v0.4.0 as the first stable release, from the scope proposal
(`memory/2026-11-11-v0.4.0-scope-proposal.md`) and the owner's decisions of 2026-11-11
and 2026-11-12 (recorded under Decisions below).

---

## Direction

- **xdevcmd's edge is a combination, not a feature**: MIT, the Total Commander model,
  native Windows, one static binary to drop on a server, no network, nothing destructive
  done silently. Nothing below thins it.
- **The name, and who it is for** (owner, 2026-11-10). **xDev Commander**, `xdevcmd` to
  type. **x** = *extend* (it extends the shell, never replaces it: the shell keeps its
  keys, its history search and its completion) and *cross-platform* (Windows native,
  macOS, Linux). **Dev** = developers and devops: people who live in the terminal, build
  and deploy from it. **Commander** = the two-pane file manager family (Total Commander,
  Double Commander, One Commander, mc). One line: "xDev Commander: a cross-platform,
  two-pane terminal file manager that extends your shell."
- **What only xdevcmd can do is join the shell to the panes.** It holds the shell's
  output (in memory), knows the shell's directory and has file panes and a viewer in one
  process. Paths in the output and lists made from it (items 30a, 30b) are built on that.
- **The boundary for every idea: it ends in an action on a file.** A path in the shell's
  output is xdevcmd's business; a URL, a commit hash, a process id or log analytics are
  not.
- **Nothing to memorise** (owner, 2026-11-12). Every command can be found by typing, from
  anywhere, with its key shown beside it (F1, item 38). A key is a shortcut, never the
  only way.
- **Stable before bigger.** v0.4.0 is judged by people meeting xdevcmd for the first time:
  a smaller release done well is the stable one (the release bar below).
- **No network, no telemetry.** Remote files come through a mount the system or a trusted
  tool provides (sshfs, rclone, SMB, NFS); xdevcmd opens no connection (owner,
  2026-11-11). Plugins are for optional extras, later, with no in-app fetcher.

## Working rules

- **A round gets its number when its work starts**, not when it is first imagined. Ideas
  wait here without a round file.
- **Rounds run in number order, one at a time.** A new round starts only when the one
  before is Complete or explicitly set aside (said in its Do section, with why).
- **Each round starts with a kickoff review** written into its Plan (what changed, the
  code it touches, an outside check, a critique, steps, the owner's decisions), and the
  owner says go before any code.
- **No new features after a release candidate**; fixes only.
- Each round works on its own `feat/…` branch (v0.4.0's rounds share `feat/v0.4.0`) and
  lands in `dev` by pull request. Rounds are append-only (`PDCA.md`).

## Done

- **0.2.0** (2026-11-07), **0.2.1** (2026-11-08), **0.3.0** (2026-11-10: GitHub release
  with six builds; `xdevcmd` and `xdevcmd-core` on crates.io). Rounds 001, 003–017
  Complete; 002 Superseded. Details: each round and `CHANGELOG.md`.
- **v0.4.0 so far, on `feat/v0.4.0`, code done, at Review for the owner's hand tests**:

  | Round | What | Item |
  |---|---|---|
  | [018](cycles/Round_018.md) | Windows asks before starting from its own panel; the pid on the menu bar | 31 |
  | [019](cycles/Round_019.md) | Foundation: `app.rs` split, Windows end to end, binstall metadata, CJK columns | 32 |
  | [020](cycles/Round_020.md) | Paths in the shell's output: marked in the history, Enter goes there, F3/F4 at the line | 30a |
  | [021](cycles/Round_021.md) | Lists from the shell's output (panelize), Alt+F7 into a list, lists kept for the session | 30b |
  | [022](cycles/Round_022.md) | Slow directories off the UI thread, the progress bar, budgets as tests, remote by mount (docs) | 36 (part 1) |
  | [023](cycles/Round_023.md) | Names sorted by Unicode's default order, in any script | 21 |
  | (memory) | xdevtheme, the default look; Neon and Glass as files; a missing theme keeps the config | 37 (part 1) |

## Next: v0.4.0, the first stable release

Decided by the owner on 2026-11-12 ("L, D: v0.4.0; có rc.1"; F1 as below; "clear
proceed"). Each step is a round with its kickoff. About 2.5–3 weeks, then a week of rc.

| Order | Item | Size |
|---|---|---|
| 0 | ~~CI on all three systems for the whole branch~~ (green, run 43181663296); the owner's hand tests: Linux and Windows done 2026-11-12, macOS to come | S |
| 1 | **F1 Commands** (item 38), Round [024](cycles/Round_024.md), at Review: the command palette in two sizes; Help menu reshaped; Config in use | M |
| 2 | **xdevtheme-light** (item 37, part 2), Round [025](cycles/Round_025.md), at Review: Dawn brought back from git and tuned | S |
| 3 | **Documentation** (item 40), Round [026](cycles/Round_026.md), at Review: Help → Documentation to `docs/` at the installed version's tag, `docs/README.md` as the index, `docs/` in the release archives | S |
| 4 | **L. Log a terminal tab to a file** (item 35), Round [027](cycles/Round_027.md), at Review: per tab from the tab menu, `text` or `raw`, a bounded writer that stops and says so | S |
| 5 | **D. Syntax-highlighted viewer** (item 33), Round [028](cycles/Round_028.md), in progress: measured on day one (syntect 7.25 MiB, over; tree-sitter further over), so a small lexer of our own; checkpoints from a background scan shaped for pillar 2's progressive indexing (A03) | S–M |
| 6 | **Q. Four pillars** (item 36, part 2; owner, 2026-11-12), each a round: **1. Data correct** (P0) · **2. Responsive and bounded** · **3. Honest contracts** · **4. Release proof** (P0 gate). Contents in the table below; the release bar below is what they must meet. **rc.1 is not cut before pillar 1 is done** | M–L |
| 7 | *If a day is spare:* **which-key** for the prefix groups (item 39) | S |
| 8 | **0.4.0-rc.1** (GitHub pre-release) → a week of the owner's real use, fixes only → **0.4.0** → packages → launch (item 10) | — |

### Step 6: the four pillars

From an outside review of v0.4 (the owner's, 2026-11-12) and this agent's answer to it:
[memory/2026-11-12-four-pillars-for-rc1.md](../memory/2026-11-12-four-pillars-for-rc1.md)
(the review itself is in `memory/attachments/`). IDs are the review's.

| Pillar | Contents | Priority |
|---|---|---|
| **1. Data correct** | B01 compare: an unreadable directory or a non-UTF-8 name never becomes a sync plan; B02 the cursor lands on the chosen file after a slow move; B04 a journal that cannot be written stops or says so (copy mid-job, rename-many); B03 panelize takes a whole line that exists before the `:line` heuristics; B05 one fault test (target replaced after confirm); `sync_data` before replacing an existing file (owner's call) | P0 |
| **2. Responsive and bounded** | A06 request generations, stale results dropped; A07 watcher debounce with max-wait, a budget per tick, rescan on overflow; A03 progressive viewer index (with D); B08 one shared resource budget; A01 marks as a `HashSet`; one measurement harness, not a framework | P1 |
| **3. Honest contracts** | Recovery contract (A08: what survives a kill, what a power cut can lose); what a copy preserves per system (B06); name identity vs display vs sort key (B07); panelize's limits; the compatibility promise | P1 |
| **4. Release proof** | B09 the size checked after the release build, per target, a missing artifact fails (today CI's test runs before the build and skips); a smoke test on the release archives; the hand-test matrix; a benchmark protocol (OS, FS, profile, cache, runs); hermetic tests (`NO_COLOR` cleared); the Windows Ctrl+\\ render flake (run 43221291571) | P0 gate |

Not for 0.4 (the review agrees): mmap, a trie, a custom arena, Tokio, a batch WAL.

**If it runs late**, the suggested cut order is D, then L; F1 Commands stays (it decides a
newcomer's first five minutes). Cutting is the owner's call.

### The release bar (what "stable" means for 0.4.0)

1. **No known data-loss or crash bug.** Every open finding from Rounds 014–023 fixed or
   written down as harmless; MkDir and Rename refuse control characters on Unix (Round
   015's backlog).
2. **A hand-test matrix, run and recorded** by the owner from a checklist the agent
   writes: Windows Terminal (cmd, pwsh) and the old console; Terminal.app and iTerm2;
   GNOME Terminal, kitty, tmux, over SSH. The same script on each: copy, move, delete,
   undo, archives, the shell bridge, paths and lists from the output, logging, F1, F3/F4,
   Settings, quit with jobs, CJK and Vietnamese names.
3. **Budgets, kept by tests where possible**: start-up under 150 ms; 100,000 entries
   listed and sorted in under 1 s; idle CPU under 0.5 %; memory flat in the soak test;
   **the binary at most 7 MB** (owner: important), checked on each release target in CI
   at the pull request, **after** the release build (pillar 4: today it is not).
4. **A compatibility promise in `docs/`** ("From 0.4.0"): what it covers (config keys,
   CLI flags, action names in `[keys]`, the theme-file format) is the owner's call at Q's
   kickoff. Display names in F1 are not covered.
5. **The parsers of untrusted input fuzzed** by hand (cargo-fuzz, nightly): OSC 7, 9;9
   and 133, the path recogniser, config and theme files, archive names.
6. **CI on for the pull request into `dev`**, the Windows ConPTY tests included.
7. **Docs and README for a first-time reader**: a 20–30 s GIF (a build error → Shift+F12
   → Enter), three lines of why, install in one command each; `docs/guide.md` checked
   against what ships.

### Packages and waiting on the owner

| | Status / question | Suggested |
|---|---|---|
| crates.io | done with 0.3.0 | — |
| `cargo binstall` | metadata in Round 019; works from 0.4.0; dry run after publishing | — |
| Scoop | templates in `packaging/`; bucket repository | `nci-gis/scoop-xdevcmd`, created by the owner |
| winget | templates in `packaging/`; PR to `microsoft/winget-pkgs` | after Scoop |
| Homebrew tap, AUR | the owner's call | tap `nci-gis/homebrew-xdevcmd`; skip AUR unless someone maintains it |
| Q: compatibility promise | what it covers | config keys, CLI flags, `[keys]` action names, theme format |

**Releasing stays the owner's call**; the flow in `docs/development.md` is unchanged.

## Later: 0.5 and after

Steered by launch feedback. Not started before 0.4.0 ships.

| Item | Why later |
|---|---|
| 41 Useful commands (owner's "readiness"): built-in examples and your own, type to filter, Enter runs, Alt+Enter types; rows in F1's "Your commands" | Builds on F1; how to build it is open (owner, 2026-11-12) |
| F1: Enter on a row of Config in use opens Settings at that setting | Small, after F1 has met users |
| Settings: the terminal's background detected (OSC 11) to pick light or dark | xdevtheme-light covers it by hand first |
| 29 One directory history for panes and shells | Alt+J covers the panes; item 30a already records each command's directory |
| 4c, 5b Settings: built-in and your own themes apart; keymap presets | Not a reason to choose xdevcmd; presets would churn keys right before the promise |
| 22 Faster copies (reflink, `copy_file_range`, clonefile, CopyFile2) | Changes the copy engine; needs time to soak |
| 34 Remote | Decided: none in 0.4.0, mounts instead (docs ship). Revisit only with feedback, keeping hard rule 2 |
| Marking paths in the live panel; a refresh model for lists beyond "run again" | Trimmed from 30a and 30b |
| F3 on a slow mount off the UI thread; fuzzing in CI; `[terminal] log = "always"` | Trimmed from 36 and 35 |
| A docs site (mdBook / GitHub Pages) | Item 40's link then points there; nothing else changes |
| 39 which-key | If not done in 0.4.0 |
| 11 File icon sets; 12–13 plugins | Optional extras; plugins not for now (owner, 2026-11-09, 2026-11-11) |

## Order (every item, for reference)

Item numbers are referenced from rounds, commits and memory, and never change.

| # | Item | Size | Round | Status |
|---|---|---|---|---|
| 1 | Colours on terminals without 24-bit colour (macOS Terminal.app) | S | [001](cycles/Round_001.md) | Complete (0.2.0) |
| 2 | macOS keys and mouse | S–M | [003](cycles/Round_003.md) | Complete (0.2.0) |
| 3 | Enter opens a file with its program, like Total Commander | M | [005](cycles/Round_005.md) | Complete (0.2.0) |
| 4 | Themes as files, switchable while running | M | [006](cycles/Round_006.md) | Complete (0.2.0) |
| 5 | Keys that work on every system: two-key bindings, Alt+J, Alt+1…9, clash checks | M | [004](cycles/Round_004.md) | Complete (0.2.0) |
| 6 | Settings (File → Settings…) | L | [007](cycles/Round_007.md) | Complete (0.2.0) |
| 7 | The logo in About | S | [008](cycles/Round_008.md) | Complete (0.2.0) |
| 8 | One name, then packages | S each | [011](cycles/Round_011.md), [017](cycles/Round_017.md), [019](cycles/Round_019.md) | crates.io with 0.3.0; binstall from 0.4.0; Scoop, winget, tap: see Packages |
| 8b, 7b, 4b, 5c | Leftovers from the 0.2.0 review; About logo on macOS; example themes; Ctrl+G | S | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 4c | Settings → Theme: built-in and your own apart | S | — | 0.5 |
| 5b | Keymap presets | M | — | 0.5 |
| 9, 9b, 9c | Undo; the mouse in Settings; click a path, double-click a border | M, S, S | [009](cycles/Round_009.md), [010](cycles/Round_010.md), [012](cycles/Round_012.md) | Complete (0.2.0) |
| 10 | Launch: Show HN with a GIF | S | — | after 0.4.0 and the packages |
| 11 | File icon sets | S–M | — | later |
| 12 | Plugins v0 | S–M | — | not for now |
| 13 | Plugins over stdin/stdout | L | — | not for now |
| 14 | An app to click (launchers; perhaps a window) | S / L–XL | — | deferred until a real need |
| 15, 16, 17, 18, 19 | Idle CPU; undo a delete to the trash; watching; replace only if older; fuzzy filter | S–M | [013](cycles/Round_013.md) | Complete (0.2.1) |
| 20 | Sharp image previews | M | [014](cycles/Round_014.md) | Complete (0.3.0) |
| 21 | Sort names by Unicode | S | [023](cycles/Round_023.md) | Review (v0.4.0) |
| 22 | Faster copies | M | — | 0.5 |
| ~~23~~ | ~~Quit from the terminal panel~~ | — | — | dropped by the owner |
| 24–28 | Session on every way out; safe operations; terminal copy mode and keymap A; shell ↔ pane bridge; 0.3.0 release prep | S–M | [014](cycles/Round_014.md)–[017](cycles/Round_017.md) | Complete (0.3.0) |
| 29 | One directory history for panes and shells | M | — | 0.5 |
| 30a | Paths in the shell's output (was 30, part 1) | M | [020](cycles/Round_020.md) | Review (v0.4.0) |
| 30b | Lists from the shell's output, Alt+F7 into a list, kept lists (was 30, part 2) | M | [021](cycles/Round_021.md) | Review (v0.4.0) |
| 31 | Windows: ask before starting from a terminal panel; the pid on the menu bar | S | [018](cycles/Round_018.md) | Review (v0.4.0) |
| 32 | Foundation | S–M | [019](cycles/Round_019.md) | Review (v0.4.0) |
| 33 | Syntax-highlighted viewer, off by setting and past a size cap | S–M | [028](cycles/Round_028.md) | In progress (v0.4.0) |
| 34 | Remote | — | — | none in 0.4.0, mounts instead (owner, 2026-11-11) |
| 35 | Log a terminal tab to a file | S | — | v0.4.0, step 4, Round 027 |
| 36 | Stable-quality pass (the release bar) | M | [022](cycles/Round_022.md) (part 1) | part 1 Review; part 2 v0.4.0, step 6 |
| 37 | xdevtheme: the default look, Neon and Glass; **xdevtheme-light** | S | (memory) | part 1 done; light v0.4.0, step 2, Round [025](cycles/Round_025.md) |
| 38 | F1 Commands: the command palette | M | [024](cycles/Round_024.md) | Review (v0.4.0) |
| 39 | which-key for prefix groups (Alt+X, Alt+R) | S | — | v0.4.0 if a day is spare, else 0.5 |
| 40 | Documentation from the app: versioned link, `docs/README.md`, docs in the archives | S | [026](cycles/Round_026.md) | Review (v0.4.0) |
| 41 | Useful commands (readiness) | M | — | 0.5 |
| ~~—~~ | ~~SFTP, remote panes~~ | — | — | dropped 2026-11-06 |

Sizes are relative: **S** a day or two, **M** under a week, **L** one to two weeks.

## Notes on the items

- **38 F1 Commands** (owner, 2026-11-12, design C of four):
  - **One component, two sizes, one key.** F1 opens the small palette: a filter on top,
    the list below. F1 again (or when the screen is small, it simply stays small)
    expands it into the large window. The text typed is kept across the change. The
    large window is today's Help, with **no tabs**: settings move to their own window.
  - **Rows:** Name · Key · Description. Narrow: Name · Key, with the selected row's full
    description on the last line. Read-only rows (mouse gestures, the viewer's own keys,
    the terminal tab bar) are shown but cannot run.
  - **Groups:** with an empty filter the large size shows the groups as today (to browse);
    the small one shows recent commands, then the ones for where F1 was pressed. Typing
    gives one flat list, best match first, each row tagged with its group (`Files: Copy`);
    typing a group's name filters to it.
  - **Context:** commands for where F1 was pressed (file pane, terminal, viewer, archive)
    first, the rest under a dim "elsewhere" line.
  - **Running:** Enter or double-click closes F1 and runs on what had focus before. **Alt+1…9**
    runs the first nine rows (digits alone go into the filter). A command refused in
    read-only or restricted mode is shown with the reason. After running a command that
    has a key, the status line says "Next time: F5".
  - **Your `[[menu]]` commands** are rows in F1 (group "Your commands"); Shift+F9 stays as
    the quick menu.
  - **Keys:** `help` always keeps F1; `[keys]` may add more, which act in the panes only
    (keymap A). From the shell: Alt+X F1. Inside F1, Tab and the digit keys are no longer
    tab switches; ↑/↓ move the selection; Esc clears the filter, then closes.
  - **Help menu:** Commands… (F1, opens the large size) · Config in use (read-only, the
    loaded config file in its own syntax, with a filter; no key) · Usage (Shift+F1) ·
    Documentation (item 40) · About. "Config in use", not "Settings in use", so it is not
    taken for File → Settings…, which edits the common settings.
  - Display names are separate from `[keys]` action names, so they can change freely.
- **40 Documentation**: `https://github.com/nci-gis/rust-commander/tree/v{version}/docs`
  from `CARGO_PKG_VERSION`, not `main` (main describes features the installed version may
  not have). `docs/README.md` gives the reading order. `docs/` ships in the release
  archives for machines without a browser or network; Help → Documentation names the
  local copy first. A docs site later only changes the link.
- **35 Log a tab**: per tab, off by default, from the tab menu "Log to file…", a ● on the
  tab while it runs. `text` (escapes stripped, lines a CR rewrote kept once) or `raw`.
  File `0600`, appended. Refused in read-only and restricted modes. The docs' "never
  written to disk" becomes "unless you turn on a log for that tab".
- **33 Highlighting**: off past a size cap and by setting; measured against the 7 MB
  budget first (Round 028: syntect is +2.24 MB, its regex engine, so over even
  trimmed; our own lexer instead, colours from the theme's slots).
- **37 xdevtheme-light**: Terminal.app's default profile is light, so it is a first
  impression; a file theme, not the default.
- **4 Themes, 5 Keymaps, 6 Settings, 7 Logo, 11 Icons**: see their rounds.

## Decisions

| Decision | Decided |
|---|---|
| Colour `auto` | truecolour only on positive evidence; 256 colours over SSH unless `COLORTERM` arrives (2026-11-06) |
| Default theme | **xdevtheme** (owner, 2026-11-11); the old look is `dark` |
| Enter on a file | `open` (Total Commander), falling back to view; `view` by setting (2026-11-06) |
| Where theme files live | config directory, and `themes/` beside the binary (2026-11-06) |
| Name | **xdevcmd** (2026-11-07); the repository keeps `rust-commander` |
| Extension points in secure modes | off in read-only and restricted unless whitelisted |
| v0.4.0 | the first stable release, judged by the release bar; an rc.1 with a week of the owner's use first (owner, 2026-11-11, 2026-11-12) |
| Remote | none in 0.4.0; mounts through the system or trusted tools, documented (owner, 2026-11-11) |
| L and D | in 0.4.0 (owner, 2026-11-12) |
| F1 | design C, as in the note on item 38; Config in use separate (owner, 2026-11-12) |
| Plugins | not for now, low priority (owner, 2026-11-09, 2026-11-11) |

## Not planned

- **SFTP and remote file systems inside xdevcmd**; remote comes through a mount.
- **Fetching anything**: a plugin list, updates, telemetry. Hard rule 2 stays as it is.
- **Total Commander's native plugins** (WFX/WCX DLLs) and **embedded Lua**: the first
  needs `unsafe` and runs Windows-only binaries in-process; the second runs plugin code
  in-process with no boundary.
- **Our own history search or completion in the shell**: the shell's job (fzf, atuin,
  PSReadLine).
