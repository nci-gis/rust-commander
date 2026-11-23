# Round 035: 0.4.0-beta.3: refactor, one definition per concept

**Status**: In Progress (the owner, 2026-11-24: "start 035")
**Part of**: v0.4.0, `ROADMAP.md`, the row for **0.4.0-beta.3** (refactor; the owner,
2026-11-23), from the cosf R1–R7 synthesis §6.3 (`03_research/cosf/rounds/
2026-11-19_tong-hop-r1-r7.md`, in the owner's cosf folder)
**Branch**: `feat/v0.4.0-beta.3`, from `dev` at `e3e528e` (Release 0.4.0-beta.2)
**Date started**: 2026-11-24
**Date completed**: YYYY-MM-DD

## Goal

Each concept that is defined in several places gets one definition, so a fix lands in
one place and cannot drift. xdevcmd does the same things as beta.2: no new feature, no
change a user sees except where a copy had drifted (each such case named in its PR).
The concepts that beta.4's languages touch go first (display width and text fit,
control characters in names, refusal messages).

## Rules for this round

- **Timebox: 2026-11-30** (the owner). Not done by then → Plan B (ROADMAP): rc.1 from
  `v0.4.0-beta.2`, the refactor and the languages to 0.5. Checkpoint 2026-11-27: if
  W1–W5 are not merged, W8 and W9 are dropped from this round.
- **No behaviour change.** Existing tests pass with their assertions untouched; a PR
  that has to change an assertion is not a refactor and says why, or is split. New
  tests are welcome (the single definition gets its own).
- **One concept per PR**, red test first where a drifted copy is found (it is a bug).
  A PR touching the data path (`qa.rs` DATA_PATH) gets a red-team review, as in beta.2.
- **Measured before and after** (the numbers below), in each PR's description.
- Old state stays readable: a journal, session or bookmark file written by beta.2 is
  read by beta.3 (a fixture spelled by hand, as for H-07).

## Baseline at `v0.4.0-beta.2` (2026-11-24)

| Measure | beta.2 | Target |
|---|---|---|
| `qa defs` root confinement | 2 | 2 (journal's own check is by design) |
| `qa defs` path identity | 3 | 3 |
| `qa defs` atomic write | 2 | **1** |
| `qa defs` refusal of writes | 7 | **1** (one place builds the message) |
| `qa defs` control characters in names | 10 | **≤ 2** |
| `qa defs` home and state folders | 6 | **1** |
| `qa defs` journal kinds | 1 | 1 |
| `qa defs` display width and text fit (W1) | 8 (`format.rs` ×6, `commands.rs::width`, `links.rs::fit`) | **6, all in `format.rs`** |
| `qa defs` chars counted (W1; a width when it sizes a column) | 40 | none used as a width (the rest named in W2's PR) |
| `qa defs` `struct App` fields | 85 | < 60 (stretch < 40) |
| `qa defs` files over 2,500 lines of code before tests | `ops.rs` 3,597 (6,565 with tests) | none |
| deleted / added lines in `src` | 0.235 (#46–#65) | higher |

## Plan

Order: what beta.4 touches first, then what the data path holds, the riskiest last.

- [ ] **W1 Measure.** `qa defs` gains a *display width* concept (the functions that turn
  text into columns), and a line count of `App`'s fields and of the largest files, so
  every PR shows its Δ. xtask only.
- [ ] **W2 Display width and text fit** (§6.3 #3). One `width(&str)` and one family of
  fit/elide in `format.rs`; `commands.rs::width`, `links.rs::fit` and the counts of
  `char`s used as columns go through it. Red test: a wide (CJK) and a combining (VI)
  name in each place a copy counted `char`s.
- [ ] **W3 Control characters in names** (10 → ≤ 2). One `is_control_in_name` (and its
  message, "a control character (U+XXXX)") for rename, mkdir, links, config, found
  paths and the terminal-safety check.
- [ ] **W4 Refusal messages** (§6.3 #5's rest; 7 → 1). `Action::effect()` already sorts
  every action (H-05); the message text is built in one place from it, so beta.4
  translates it once.
- [ ] **W5 Home, state and XDG folders** (§6.3 #2, H-09; 6 → 1). One module answers
  "where is the home, the state, the config folder", on all three systems.
- [ ] **W6 Atomic write** (§6.3 #4, H-06; 2 → 1). Session and document writes go
  through one `save_bytes` (temp file, sync, no-replace or replace rename). Data path.
- [ ] **W7 Text forms** (§6.3 #1 and #6). SortKey by `FromStr`/`Display`, Axis by
  serde; the CLI options from one table, the parser tested against it.
- [ ] **W8 One job type** (§6.3 #8) for the copy, delete, archive and search workers.
  Data path.
- [ ] **W9 Split `ops.rs` and `App`'s state** (§6.3 #9), by component. Data path.
  First to be cut if the timebox is near.

### To check at the cut (as for beta.2)

- [ ] The baseline table filled with beta.3's numbers.
- [ ] CI green with no re-run; the new tests ×10 on Linux.
- [ ] Hand-test ledger PASS at the cut commit (21 cells), the builds taken from the CI
  artifact of that commit (`docs/development.md`); plus one case: a copy killed on
  beta.2, continued on beta.3.
- [ ] Speed A/B against `v0.4.0-beta.2` (interleaved on a noisy machine).
- [ ] Fuzz (six targets, 60 s each).
- [ ] The release draft: assets named `xdevcmd-testkit-*`, and the notes' links open.

## Do

- **W0, the walker** (the owner, 2026-11-24, an exception to "no new features":
  "đưa vào beta.3 luôn … lên trước refactor để xem luôn"): a small ASCII agent walks
  the status line a step a second when `[general] animation` is on (off by default),
  waits before a message, and goes on from where it stopped. New module `walker.rs`;
  one field on `App` (86); drawn by `draw_status` on the 1 s heartbeat it already had.
- W1 #72, W2 #73 merged 2026-11-24: width definitions all in `format.rs` (9),
  `chars().count()` 40 → 14, none a width.

## Check

## Act
