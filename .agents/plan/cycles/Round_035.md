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

| Measure | beta.2 | Target | beta.3 (`929361d`) |
|---|---|---|---|
| `qa defs` root confinement | 2 | 2 (journal's own check is by design) | 2 |
| `qa defs` path identity | 3 | 3 | 3 |
| `qa defs` atomic write | 2 | **1** | **1** ✓ |
| `qa defs` refusal of writes | 7 | **1** (one place builds the message) | 3, one per role: the decision (`App::refused`), the wording (`Mode::turned_off`), what an action writes (`Action::writes`) |
| `qa defs` control characters in names | 10 | **≤ 2** | **2** ✓ |
| `qa defs` home and state folders | 6 | **1** | 3, all in `dirs.rs` (target 1 missed) |
| `qa defs` journal kinds | 1 | 1 | 1 |
| `qa defs` display width and text fit (W1) | 8 (`format.rs` ×6, `commands.rs::width`, `links.rs::fit`) | **6, all in `format.rs`** | 9, all in `format.rs` ✓ |
| `qa defs` chars counted (W1; a width when it sizes a column) | 40 | none used as a width (the rest named in W2's PR) | 14, none a width ✓ |
| `qa defs` `struct App` fields | 85 | < 60 (stretch < 40) | **57** ✓ (stretch < 40 not met) |
| `qa defs` files over 2,500 lines of code before tests | `ops.rs` 3,597 (6,565 with tests) | none | none ✓ (`ops/copy.rs` ~1,420) |
| deleted / added lines in `src` | 0.235 (#46–#65) | higher | 0.30 (W1–W8 alone 0.42; W9/W10 moves excluded) ✓ |

## Plan

Order: what beta.4 touches first, then what the data path holds, the riskiest last.

- [x] **W1 Measure.** `qa defs` gains a *display width* concept (the functions that turn
  text into columns), and a line count of `App`'s fields and of the largest files, so
  every PR shows its Δ. xtask only.
- [x] **W2 Display width and text fit** (§6.3 #3). One `width(&str)` and one family of
  fit/elide in `format.rs`; `commands.rs::width`, `links.rs::fit` and the counts of
  `char`s used as columns go through it. Red test: a wide (CJK) and a combining (VI)
  name in each place a copy counted `char`s.
- [x] **W3 Control characters in names** (10 → ≤ 2). One `is_control_in_name` (and its
  message, "a control character (U+XXXX)") for rename, mkdir, links, config, found
  paths and the terminal-safety check.
- [x] **W4 Refusal messages** (§6.3 #5's rest; 7 → 1). `Action::effect()` already sorts
  every action (H-05); the message text is built in one place from it, so beta.4
  translates it once.
- [x] **W5 Home, state and XDG folders** (§6.3 #2, H-09; 6 → 1). One module answers
  "where is the home, the state, the config folder", on all three systems.
- [x] **W6 Atomic write** (§6.3 #4, H-06; 2 → 1). Session and document writes go
  through one `save_bytes` (temp file, sync, no-replace or replace rename). Data path.
- [x] **W7 Text forms** (§6.3 #1 and #6). SortKey by `FromStr`/`Display`, Axis by
  serde; the CLI options from one table, the parser tested against it.
- [x] **W8 One job type** (§6.3 #8) for the copy, delete, archive and search workers.
  Data path.
- [x] **W9 Split `ops.rs` and `App`'s state** (§6.3 #9), by component. Data path.
  First to be cut if the timebox is near.
- [x] **W10 OS primitives** (added 2026-11-24, the owner's option A after the design
  review in `memory/2026-11-24-beta3-design-review.md`): `xdevcmd_core::os`, one
  function per question, a body per platform inside it. Data path (journal lock).

### To check at the cut (as for beta.2)

- [ ] The baseline table filled with beta.3's numbers.
- [x] CI green with no re-run; the new tests ×10 on Linux. 2026-11-24 on `929361d`: CI
  45324179624 green, 7/7 jobs, first run; the 109 tests added since beta.2 ×10 on
  Linux, 1,090 passed, none failed. To redo if the cut commit is later.
- [ ] Hand-test ledger PASS at the cut commit (21 cells), the builds taken from the CI
  artifact of that commit (`docs/development.md`); plus one case: a copy killed on
  beta.2, continued on beta.3.
- [x] Speed A/B against `v0.4.0-beta.2` (interleaved on a noisy machine). 2026-11-24,
  this session's 2-core VM (load ~1), release xtask of each side, `--only` per case,
  A then B, then B then A; medians, "B vs A" positive = B slower:

  | Case | A | B | pass 1 | B | A | pass 2 |
  |---|---|---|---|---|---|---|
  | plan 10,000 | 17.7 ms | 17.3 ms | −3 % | 16.4 ms | 19.6 ms | −17 % |
  | plan 100,000 | 181 ms | 202 ms | +12 % | 180 ms | 184 ms | −2 % |
  | plan 200,000 | 381 ms | 393 ms | +3 % | 394 ms | 410 ms | −4 % |
  | list 10,000 | 21.9 ms | 20.8 ms | −5 % | 21.7 ms | 20.4 ms | +7 % |
  | list 100,000 | 250 ms | 233 ms | −7 % | 244 ms | 237 ms | +3 % |
  | mixed (large + small) | 3.31 s | 3.73 s | +13 % | 3.41 s | 3.49 s | −2 % |

  **Passes**: no case slower in both passes; the two that were slower in one (plan
  100,000, mixed) are faster in the other, this VM's drift (Round 033: A against itself
  +14 %). B is `66e6930`, the freeze after Round 036.
- [x] Fuzz (six targets, 60 s each). 2026-11-24 at `66e6930`, stable with
  `RUSTC_BOOTSTRAP=1` as for beta.2. **The fuzz crate did not build**: `config.rs` reads
  `crate::walker` since W0, and `fuzz/src/lib.rs` (outside the workspace, no CI) did not
  compile `walker.rs`; fixed in the PR that records this (fuzz crate only). Then: osc
  3,677,508 runs, paths 955,130, config 489,560, theme 601,350, archive_name 2,624,957,
  archive_list 113,324; no crash, `fuzz/artifacts/` empty.
- [ ] The release draft: assets named `xdevcmd-testkit-*`, and the notes' links open.

## Do

- **W0, the walker** (the owner, 2026-11-24, an exception to "no new features":
  "đưa vào beta.3 luôn … lên trước refactor để xem luôn"): a small ASCII agent walks
  the status line a step a second when `[general] animation` is on (off by default),
  waits before a message, and goes on from where it stopped. New module `walker.rs`;
  one field on `App` (86); drawn by `draw_status` on the 1 s heartbeat it already had.
  **A deliberate test of the rules** (the owner, 2026-11-24: "đây là 1 lần mình thử
  phá rule đã chốt để xem kết quả có bị stale ko … sau đó review xem ảnh hưởng"). To
  review at the Check: did W0 cost the timebox (time spent on it vs W1–W9); did it
  make evidence stale (`walker.rs`, `draw.rs`, `config.rs`, `settings_dialog.rs` are
  not on the data path, so the hand-test ledger should stay fresh; confirm with `qa
  ledger`); did it pull in more scope (requests that followed it); did it break the
  "no assertion changed" rule (it did not: new tests only).
- **CI by batch** (#75, the owner: CI once a piece is done): merges into the
  integration branch in a burst cancel each other's runs (the concurrency group), so
  only the last one finishes. W1's new test failed on Windows (a path written with
  `/`), #72's own run was cancelled by #73's, and the branch stayed red on Windows
  from #72 to #75 unseen until the run by hand on the W3–W5 stack (45252575167). Fixed
  in #78; green on all seven jobs (45254421358). For the Check: a burst of merges needs
  its last run read, not assumed.
- W1 #72, W2 #73 merged 2026-11-24: width definitions all in `format.rs` (9),
  `chars().count()` 40 → 14, none a width.
- **Walker follow-ups, scope data for the Check**: after W0 came four requests, each a
  PR (2026-11-24): hold on the status line 5–15 s (`animation_hold_secs`); walk on the
  file pane's bottom edge; across the whole screen; on the *lowest* file pane's edge,
  not the focused one's (#85, the owner's screenshot with the right side split). One
  exception, five PRs.
- **W9** (2026-11-24), moves only: #84 split `ops.rs` (3,597 code lines) into `ops/`
  (largest `copy.rs`, ~1,420); checked by script that every original line range is in
  its new file, equal modulo whitespace and `pub(super)`; 82 tests before and after.
  `qa defs`: no file over 2,500 lines (a `tests.rs` counts as tests). Then 33 `App`
  fields grouped into `Pictures`, `Hits`, `ShellPane`, `Watching`: App fields 86 → 57.

- **Code freeze: `66e6930`** (2026-11-24, after Round 036 and its test trash, on the
  owner's word; CI 45373140431 green). Was: **`f6e6202`** (2026-11-24; CI 45349736217 green, 7/7 jobs). Reviewed by
  three independent red-team rounds (data path; app, CI and xtask; the check of #90),
  each SIGN WITH AMENDMENTS, the amendments fixed in #90, #93, #94; what they left open
  is in `memory/2026-11-24-beta3-review-carryover.md` (rc.1, beta.4). From here the
  product's code changes only for a bug a test finds; Round 036 changes test code and
  tooling only. Hand-test evidence for the cut is built from this commit or later.

## Check

## Act
