# Roadmap

> What comes next, in order, and why. A round says how; this file says what and when.
> Reorder freely; strike through what is dropped rather than deleting it.

**Released**: 0.3.0 (2026-11-10, Latest); **0.4.0-beta.1** (2026-11-13, GitHub
pre-release). **In progress**: 0.4.0-beta.2 on `feat/v0.4.0-beta.2` (Round 033).
Rewritten 2026-11-12 around v0.4.0 as the first stable release; shortened 2026-11-15
(the longer version, with the finished items' design notes, is in git history).

---

## Direction

- **The edge is a combination, not a feature**: MIT, the Total Commander model, native
  Windows, one static binary to drop on a server, no network, nothing destructive done
  silently. Nothing below thins it.
- **xDev Commander** (`xdevcmd`, owner, 2026-11-10): **x** = *extend* (the shell keeps
  its keys, history search and completion) and *cross-platform*; **Dev** = developers and
  devops who live in the terminal; **Commander** = the two-pane family. "A cross-platform,
  two-pane terminal file manager that extends your shell."
- **What only xdevcmd can do is join the shell to the panes**: it holds the shell's
  output, knows its directory, and has panes and a viewer in one process (items 30a, 30b).
- **Every idea ends in an action on a file.** A path in the output is xdevcmd's business;
  a URL, a commit hash, a process id or log analytics are not.
- **Nothing to memorise** (owner, 2026-11-12): every command can be found by typing, its
  key beside it (F1). A key is a shortcut, never the only way.
- **Co-opetition** (owner, 2026-11-13): where others do it well (rclone, rsync, robocopy
  over the network), xdevcmd works with them. The planner is best on local disks and
  across drives; for the network the user runs a trusted tool, and xdevcmd helps (41, 44).
- **Stable before bigger**: v0.4.0 is judged by people meeting xdevcmd for the first time.
- **No network, no telemetry**: remote files come through a mount (sshfs, rclone, SMB,
  NFS); xdevcmd opens no connection (owner, 2026-11-11).

## Working rules

- **A round gets its number when its work starts**; ideas wait here without a file.
- **Rounds run in number order, one at a time**; a new one starts when the one before is
  Complete or explicitly set aside (said in its Do, with why).
- **Each round starts with a kickoff review** in its Plan, and the owner says go before
  any code.
- **No new features after a release candidate**; fixes only.
- Each round works on a `feat/…` branch and lands in `dev` by pull request. Rounds are
  append-only (`PDCA.md`).

## v0.4.0, the first stable release

Release path (owner, 2026-11-13): each pre-release a GitHub pre-release and about a week
of the owner's real use. Pre-releases go to GitHub only; crates.io, binstall, Scoop and
winget start at 0.4.0.

| Release | Contents | Status |
|---|---|---|
| **0.4.0-beta.1** | Items 21, 30a, 30b, 31–33, 35, 37 (light), 38, 40 (Rounds 018–028); pillar 1 (data correct) and **Smart Planner v1**, item 42 (Rounds 029–031); the pre-release plumbing (032) | Released 2026-11-13. Rounds 018–028 at Review until the owner's hand tests; 029–031 Complete; 032 open |
| **0.4.0-beta.2** | **Smart Planner v2**, item 43: group commit for copies and replaces, the platform's fast copy (22), checkpointed resume, finished folders skipped; pillar 2 (responsive and bounded); items 45, 46; and rc.1's planned work, moved here by the owner (2026-11-14): pillars 3 and 4 (compatibility promise, what a copy keeps, fuzzing, smoke test and provenance, the hand-test matrix, the benchmark protocol) | Round 033, code done, CI green. Left: the owner's hand tests (macOS first); Windows and macOS bench numbers, which decide **lanes** (else 0.5); the cut |
| **0.4.0-rc.1** | Fixes only (feature freeze) | — |
| **0.4.0** | All tests and the owner's week green; then packages, then the launch (item 10) | — |

The four pillars come from an outside review (owner, 2026-11-12):
[memory/2026-11-12-four-pillars-for-rc1.md](../memory/2026-11-12-four-pillars-for-rc1.md).
Not for 0.4 (the review agrees): mmap, a trie, a custom arena, Tokio, a batch WAL.

### The release bar (what "stable" means)

1. **No known data-loss or crash bug.** Ongoing: none known open after Round 033's
   review fixes (a copy into itself through a link, `q` quitting at once).
2. **The hand-test matrix**, run and recorded by the owner (`docs/testing-by-hand.md`):
   Linux and Windows done 2026-11-12; macOS to come, and again on beta.2.
3. **Budgets kept by tests**: start-up under 150 ms, 100,000 entries in under 1 s, idle
   CPU under 0.5 %, memory flat in the soak test, **the binary at most 7 MB** checked
   after the release build in CI. Done.
4. **A compatibility promise**: `docs/compatibility.md` (config keys, CLI flags, `[keys]`
   action names, theme files, old journals). Done.
5. **Parsers of untrusted input fuzzed** by hand (`cargo xtask fuzz`). Done; run again
   before each release.
6. **CI on for the pull request into `dev`**, ConPTY tests included. Green on beta.2.
7. **Docs and README for a first-time reader**: a 20–30 s GIF, three lines of why, one
   install command each. The GIF is still to make.

### Packages

| | Status |
|---|---|
| crates.io | since 0.3.0; `release.toml` has `publish = false`, publishing is by hand |
| `cargo binstall` | metadata since Round 019; dry run after publishing 0.4.0 |
| Scoop | templates in `packaging/`; bucket `nci-gis/scoop-xdevcmd`, to be created by the owner |
| winget | templates in `packaging/`; after Scoop |
| Homebrew tap, AUR | the owner's call; suggested: tap `nci-gis/homebrew-xdevcmd`, no AUR without a maintainer |

**Releasing stays the owner's call**; the flow is in `docs/development.md`.

## Later: 0.5 and after

Steered by launch feedback. Not started before 0.4.0 ships.

| Item | Why later |
|---|---|
| 44 **The planner hands work to trusted tools** | The lead of 0.5; note below |
| 41 Useful commands: built-in and your own, type to filter, Enter runs, Alt+Enter types; rows in F1 | Builds on F1; how is open |
| 43 Lanes (small files beside a large one), if beta.2's numbers did not decide them | Linux shows at most ~15 ms to gain on 92 ms |
| 39 which-key for the prefix groups | rc.1 is fixes only |
| 29 One directory history for panes and shells | Alt+J covers the panes |
| 4c, 5b Built-in and your own themes apart; keymap presets | Presets would churn keys right after the promise |
| F1: Enter on a Config in use row opens Settings there; the terminal's background (OSC 11) picks light or dark | Small, after F1 and the light theme meet users |
| Paths marked in the live panel; lists refreshed beyond "run again"; F3 on a slow mount off the UI thread; fuzzing in CI; `[terminal] log = "always"` | Trimmed from 30a, 30b, 36, 35 |
| A docs site (mdBook / GitHub Pages) | Help → Documentation's link then points there |
| 34 Remote | Mounts instead; revisit only with feedback, keeping hard rule 2 |
| 11 File icon sets; 12–13 plugins | Optional extras; plugins not for now |
| 47 **[draft] Go to anything** | A direction, not a commitment; note below |

## Items

Numbers are referenced from rounds, commits and memory, and never change. Sizes: **S** a
day or two, **M** under a week, **L** one to two weeks.

**Complete**:
- 0.2.0: 1–7, 9, 9b, 9c (Rounds 001, 003–010, 012).
- 0.2.1: 8b, 7b, 4b, 5c, 15–19 (Round 013).
- 0.3.0: 20, 24–28 (Rounds 014–017); 8's crates.io part (011, 017, 019).
- 0.4.0-beta.1: 21, 30a, 30b, 31, 32, 33, 35, 37, 38, 40 (Rounds 018–028, at Review);
  36 part 1 (022); 42 (029–031).
- 0.4.0-beta.2: 22, 43 but lanes, 45, 46 (Round 033); 36 part 2, the four pillars
  (Rounds 029–033).

**Open**:

| # | Item | Size | When |
|---|---|---|---|
| 4c | Settings → Theme: built-in and your own apart | S | 0.5 |
| 5b | Keymap presets | M | 0.5 |
| 8 | Packages: binstall, Scoop, winget, tap | S each | after 0.4.0 |
| 10 | Launch: Show HN with a GIF | S | after 0.4.0 and the packages |
| 11 | File icon sets | S–M | later |
| 12, 13 | Plugins v0; plugins over stdin/stdout | S–M, L | not for now |
| 14 | An app to click (launchers; perhaps a window) | S / L–XL | until a real need |
| 29 | One directory history for panes and shells | M | 0.5 |
| 34 | Remote | — | none; mounts instead (owner, 2026-11-11) |
| 39 | which-key for prefix groups (Alt+X, Alt+R) | S | 0.5 |
| 41 | Useful commands | M | 0.5 |
| 43 | Lanes | S–M | beta.2 if the numbers say so, else 0.5 |
| 44 | The planner hands work to trusted tools | M | 0.5 |
| 47 | [draft] Go to anything | L | after 0.4.0 |
| ~~23~~ | ~~Quit from the terminal panel~~ | — | dropped by the owner |
| ~~—~~ | ~~SFTP, remote panes~~ | — | dropped 2026-11-06 |

## Notes on open items

The finished items' designs are in their rounds (F1: 024; the planner: 029–031, 033).

- **43 Lanes**: small files in batches beside a large file, per device. Worth it only if
  creating small files is a large share of a mixed copy; Linux says about 15 ms of 92
  (Round 033). Windows' and macOS' numbers decide (`docs/benchmark.md`).
- **44 The planner hands work to trusted tools** (owner, 2026-11-13):
  - **The plan is always xdevcmd's**; who carries it out is written at its top
    (xdevcmd, the system's fast copy, or a named tool).
  - **Out**: the plan in the tool's own form (rsync and rclone `--files-from`, a robocopy
    job), run in a terminal tab: visible, editable, loggable.
  - **Before**: the tool's dry run (`rsync -n --itemize-changes`, `robocopy /L`,
    `rclone --dry-run`) in the plan table.
  - **Back**: the tool's output read into a list; run again for the failures only.
  - **A suggestion, never a push**: offered when a big job crosses the network and the
    tool is already on `PATH` (nothing fetched), beside "use xdevcmd anyway".
  - Three tools and their machine-readable output only; an unknown version still runs.
- **47 [draft] Go to anything** (owner, 2026-11-13; may change or be dropped):
  (a) a search box over places you go (visited folders, history, hotlist, by frequency
  and recency, plus the current tree), with a key besides Ctrl+Space (macOS's input
  switch); (b) an index of folders you choose, kept on disk while xdevcmd runs, its age
  shown, the system's index first (mdfind, plocate, Windows Search); (c) F1's commands on
  what is found; (d) last, MCP over local stdio, off by default, read-only, chosen
  folders only. (a) needs no index; a daemon only if (b) shows it is needed. The index
  never leaves the machine except through (d), which the user turns on.

## Decisions

| Decision | Decided |
|---|---|
| Colour `auto` | truecolour only on positive evidence; 256 colours over SSH unless `COLORTERM` arrives (2026-11-06) |
| Default theme | **xdevtheme** (owner, 2026-11-11); the old look is `dark` |
| Enter on a file | `open` (Total Commander), falling back to view; `view` by setting (2026-11-06) |
| Where theme files live | config directory, and `themes/` beside the binary (2026-11-06) |
| Name | **xdevcmd** (2026-11-07); the repository keeps `rust-commander` |
| Extension points in secure modes | off in read-only and restricted unless whitelisted |
| v0.4.0 | the first stable release, judged by the release bar (owner, 2026-11-11, 2026-11-12) |
| Remote | none in 0.4.0; mounts, documented (owner, 2026-11-11) |
| F1 | design C (Round 024); Config in use separate (owner, 2026-11-12) |
| Plugins | not for now, low priority (owner, 2026-11-09, 2026-11-11) |
| Copy and move | the Smart Planner in 0.4: v1 in beta.1, v2 in beta.2; warnings before anything that cannot be undone (owner, 2026-11-13) |
| Release path | beta.1 → beta.2 → rc.1 (fixes only) → 0.4.0 (owner, 2026-11-13); rc.1's planned work done in beta.2 (owner, 2026-11-14) |
| Where pre-releases go | **GitHub only**, marked pre-release; a crates.io version can never be deleted, only yanked (owner, 2026-11-13) |

## Not planned

- **SFTP and remote file systems inside xdevcmd**; remote comes through a mount.
- **Fetching anything**: a plugin list, updates, telemetry. Hard rule 2 stays.
- **Total Commander's native plugins** (WFX/WCX DLLs) and **embedded Lua**: the first
  needs `unsafe` and runs Windows-only binaries in-process; the second runs plugin code
  in-process with no boundary.
- **Our own history search or completion in the shell**: the shell's job (fzf, atuin,
  PSReadLine).
