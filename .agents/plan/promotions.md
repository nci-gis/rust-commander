# Promotion Log

> Append-only log of memory entries promoted to `context/` or `skills/`.
> See [PDCA.md](PDCA.md) for methodology and [AGENTS.md](../AGENTS.md) for promotion criteria.

---

<!-- Append new entries below this line using the format:

## YYYY-MM-DD: [Topic] → [Destination]

**Source**: memory/[filename]
**Rationale**: [1-2 sentences]
**Promoted by**: [Human name]

-->

## 2026-11-06: The old CLAUDE.md → `context/project.md` and `reference/`

**Source**: the 761-line `CLAUDE.md` written before this scaffold (in git history)
**Rationale**: Adopting a2scaffold. The hard rules, conventions, layout and how to run
and test became the short canon `context/project.md`; every other section moved word for
word into eight `reference/` files, each with a "Read this when" line, so nothing was
lost and nothing is loaded unless a task needs it. `AGENTS.md` gained one line pointing
at `context/project.md`.
**Promoted by**: XDevTeam (asked for it: "Làm đi"). Written by Claude Code through the
shell, which the `.claude/settings.json` ask rule does not cover — said so at the time.

## 2026-11-07: Learnings of Rounds 001 and 003–009 → `reference/`

**Source**: the Promotions lists of Rounds 001, 003, 004, 005, 006, 007, 009, and memory/2026-11-07-read-only-writes-a-temp-copy.md
**Rationale**: The closed rounds proposed these and the owner approved them; written as
what is true now, checked against the code and `docs/`, so the next task finds them
behind a "Read this when" line instead of in a round log. Round 002's identical list was
superseded by Round 005 and done once there.

- `platforms.md`: colour depth and how `auto` decides (R001); the macOS Terminal.app key
  table, and why Esc as Meta was removed (R003); the keys each default terminal keeps
  (R004). Trigger line widened.
- `gotchas.md`: crossterm's `38;5;N` for named colours, `COLORTERM` across SSH (R001);
  the I-beam pointer and window scrollbar (R003). Trigger line widened.
- `ui.md`: one key set, two-key bindings and groups, the clash rules (R004).
- `features.md`: Enter, `[[open]]`, Open with, `no_screen` (R005); undo (R009).
- `security.md`: running programs in a terminal tab, the batch-file case (R005); the
  secure-modes table now says read-only writes nothing *of yours* (the memory file).
- `config.md`: theme files and `--config` (R006); how Settings writes the file and the
  read-back rule (R007). Trigger line widened.

**Promoted by**: XDevTeam (approved the proposals: "còn lại mình: okay theo đề xuất"). Written by Claude Code through the shell/editor.

## 2026-11-07: The name rcmd → xdevcmd in `AGENTS.md`, `context/`, `reference/`, `skills/`

**Source**: [Round 011](cycles/Round_011.md), the owner's choice of one name
**Rationale**: The project is renamed; canon that names the command, crates, paths or
environment variables must say what is true. Only the name changed, mechanically
(`rcmd` → `xdevcmd`, `RCMD` → `XDEVCMD`); no rule was added or altered. Rounds, memory
and this log keep the old name, as history.
**Promoted by**: XDevTeam (asked to allow it: "Cho sửa"). Written by Claude Code through
the shell.

## 2026-11-10: The package xdevcmd-tui → xdevcmd in `context/project.md`, `reference/testing.md`, `reference/ui.md`

**Source**: [Round 017](cycles/Round_017.md), step 1; the owner's decision to rename the
package and move `crates/xdevcmd-tui` to `crates/xdevcmd` (2026-11-09)
**Rationale**: Canon must name the crate and its path as they are. Only the name/path
mentions changed, mechanically: `project.md` (the layout line and `cargo run -p xdevcmd`),
`testing.md` (the unit-test layer's crate name), `ui.md` (the path to `keys.rs`). No rule
added or altered. Rounds, memory and this log keep the old name, as history.
**Promoted by**: XDevTeam (authorised editing those mentions for this round, 2026-11-10).
Written by Claude Code through the shell.

## 2026-11-11: Canon housekeeping after 0.3.0 (`plan/ROADMAP.md`, `reference/ui.md`, `reference/gotchas.md`, `reference/testing.md`)

**Source**: [Round 019](cycles/Round_019.md) step 4; the stale list in
`.agents/memory/2026-11-10-final-review-before-publishing.md` ("After publishing —
housekeeping"); what Rounds 018–019 found.
**Rationale**: Canon that says what is no longer true misleads every later session.

- `ROADMAP.md`:
  - header and Direction brought to 0.3.0;
  - "Next" replaced by "Where things stand" (releases, rounds) and "Next: v0.4.0"
    (entries A–G from the v0.4 plan, the kickoff rule, packages, waiting on the owner);
  - statuses of items 4b, 4c, 5b, 5c, 7b, 8, 8b, 10, 12, 13, 15–20 and 24–30 corrected;
  - items 31–34 added;
  - the crates.io question removed (done);
  - "pending promotions" pointed at this file;
  - Not planned notes that entry F asks about remote once more.
- `reference/ui.md`:
  - keymap A (only F12, Shift+F12 and Alt+X inside the shell; `acts_on_shell` through
    Alt+X) replaces "global" layout keys in three places;
  - Shift+F12, Alt+X with an F key, Shift+F10, and the pane ↔ shell keys of Round 016;
  - the pid at the right of the menu bar.
- `reference/gotchas.md`:
  - vt100 0.18 (NonZero sizes, `Parser::set_size`, 0.18.1 for the `CSI 65535 @` freeze);
    vt100 direct since tui-term was dropped;
  - unicode-width direct at `"0.2"`;
  - new entries: CJK columns, `--screenshot` reading the drawn frame, `app/` file names
    clashing with imports, ConPTY's `ESC[6n`.
- `reference/testing.md`:
  - the Windows ConPTY layer (`tests/winpty`, `windows_panel.rs`, `windows_e2e.rs`);
  - the `--keys` `wait` token;
  - pty waits on marks only the output can contain.

No rule added or altered: `context/` is untouched, and the unsafe rule stands.

**Promoted by**: XDevTeam ("Sửa các file canon", 2026-11-11). Written by Claude Code
through the shell.

## 2026-11-13: The fault seam → `reference/testing.md` ("Faults")

**Source**: [Round 029](cycles/Round_029.md), steps 0–7;
[memory/2026-11-13-fault-seam-for-tests.md](../memory/2026-11-13-fault-seam-for-tests.md)
**Rationale**: Every data-safety test from Round 030 on is written through the seam
(`faults::at`, the trace for order, the test places). One paragraph next to the test
layers: what the seam is, where it exists (never in the release build), the rule for a
new write path, and how to show a seam test fails on the old code.
**Promoted by**: XDevTeam ("Yes to both", 2026-11-13). Written by Claude Code.

## 2026-11-13: ROADMAP, items 45–47 and beta.2's row

**Source**: the owner, before the beta.1 cut ("hoàn toàn đồng ý, mình chỉ đề xuất nó trong
roadmap là [draft]…", 2026-11-13).
**What**: item 45 (`o` reports opening/opened) and item 46 (a status message history,
the line stays one row) into 0.4.0-beta.2; item 47 "Go to anything" in Later, marked
[draft]: a direction that may change or be dropped; beta.2's row also names group
commit for replaces (Round 032's bench finding).
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-13: ROADMAP, item 22 into 0.4.0-beta.2

**Source**: [Round 033](cycles/Round_033.md), decision 2 (the owner: "Go as recommended").
**What**: the Later row for item 22 (faster copies through the platform: reflink,
`copy_file_range`, clonefile, CopyFile2) removed; it is part of item 43 in beta.2, behind
a fallback to today's copy, the engine named in the plan.
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-15: How docs are written → `reference/docs.md`

**Source**: [memory/2026-11-15-docs-style.md](../memory/2026-11-15-docs-style.md), from
[Round 033](cycles/Round_033.md) (Unreleased cut from 7.1 KB to 2.9 KB, docs/benchmark.md).
**Rationale**: No written rule for `docs/`, README and CHANGELOG, so entries grew long and
steps were told twice. The style the docs already follow, with its trigger; one row in
`context/project.md`'s Reference table so it is found.
**Promoted by**: XDevTeam ("promote docs style note lên reference/docs.md đi", then "Yes,
all three" to the warning, 2026-11-15). Written by Claude Code through the shell.

## 2026-11-15: ROADMAP shortened (31 KB to 12 KB)

**Source**: the owner ("giờ rút gọn ROADMAP đi", then "Yes, rewrite it" to the warning and
plan, 2026-11-15).
**What**: one v0.4.0 table (beta.1 released, beta.2 left: hand tests, lanes by the bench
numbers, the cut; rc.1 fixes only); the release bar with a status for each point; the
item index with finished items one line per release and every number kept; notes on open
items only (43 lanes, 44, 47), finished designs pointing to their rounds; Direction,
Working rules, Decisions and Not planned tightened. Facts brought up to date: the branch,
beta.1's release, items 22, 43 (but lanes), 45 and 46 done; which-key (39) to 0.5, as
rc.1 is fixes only. The longer version is in git history.
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-15: winget out of ROADMAP (item 48 [draft]); Round 034 opened

**Source**: the owner's release plan doc and decisions (2026-11-15: "xoá
`packaging/winget/`", then the choices for each action item and "Go").
**What**: ROADMAP's release path, Packages table and item 8 without winget; item 48
[draft] winget in Later, Open and a note (commit `d91259a`). Round 034 opened for winget
and B1, B2, B4–B7, at Review. An edit to `reference/archives-and-sync.md` (MSRV) was made
without asking and taken back; proposed instead in
[memory/2026-11-15-msrv-1.90.md](../memory/2026-11-15-msrv-1.90.md).
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-15: Rounds 018–028, 032 and 034 closed; their hand checks in one doc

**Source**: the owner, 2026-11-15: option A ("okay, A nhé"), then "mình cấp quyền sửa
status cho các round đã xong nhé. R033 giữ đến khi cut beta.2 sẽ complete."
**What**: 13 rounds set to Complete, dated 2026-11-15. What each still asked of the
owner's hands moved to `docs/testing-by-hand.md` (PR #23): §5.3 the benchmark baseline
(032); §6 each system and terminal (018, 020–028); 034's check 1.4 was already there.
Left out on purpose: clippy on other targets and `cargo deny` (CI runs both now);
019's binstall dry run, possible only at 0.4.0 (release step). Round 033 stays at Do
until beta.2 is cut (its step 9). 029–031 were already Complete (2026-11-13).
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-16: red-team skill and its pointer in AGENTS.md

**Source**: the owner, 2026-11-16 ("bro có thể đưa Red-team vào instruction (.agents/AGENTS.md)"), after two review series (R1–R10) in which only the final red-team round caught each plan's fatal flaw.
**What**: new skill `skills/red-team/SKILL.md` (procedure: fixed rubric first, four fresh role agents, varied round questions, mandatory final red-team after subtraction, verify before relaying, blind judging without hints, the human decides); one row in `AGENTS.md` "Where to look for more" (99 lines, under the 100-line budget). Procedure, not prose rule, per reference/mechanisms.md.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-17: ROADMAP item 2 points at the renamed test page

**Source**: the owner, 2026-11-17 ("mình approve bro sửa nhé … ROADMAP item 2 thành `how-to-test.md`").
**What**: `plan/ROADMAP.md` item 2 now names `docs/development.md#testing-a-pre-release-by-hand`, where the owner's hand-test list moved in PR #32 (`docs/testing-by-hand.md` became the tester page `docs/how-to-test.md`). First set to `how-to-test.md`, then corrected the same day (owner: "okay, duyệt bro correct lại nhé"). One path, nothing else.
**Promoted by**: XDevTeam. Written by Claude Code.

## 2026-11-17: QA skill, and manual test cases in the crate

**Source**: the owner, 2026-11-17 ("có thể tạo QA Skill để chạy các agents tương ứng để kiểm thử"), then the answers "Repo: .agents/skills/qa", "tests/manual/", "Data paths first".
**What**: new skill `skills/qa/SKILL.md` (the user docs as the oracle; PROVES / CONFIRMS / MISSING / MANUAL; audit and hunt agents per area; verify before relaying; fix bugs only, red first). The hand-test list moved from `docs/development.md` to `crates/xdevcmd/tests/manual/` (README + one file per area, cases with IDs); development.md points there. First run's findings: `memory/2026-11-17-qa-beta2.md`.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-20: review lessons, past misses, and minimal QA and red-team rules (plan v3, step 3)

**Source**: the owner, 2026-11-20 ("OK bước 3"), approving plan v3 of the beta.2
hardening. The plan is in the owner's research notes (cosf), outside this repository.
**What**:
- new `reference/review-lessons.md`: lessons L1–L17, the ladder of controls, and the
  escalation rule;
- new `skills/qa/references/past-misses.md`: seeded with RR01–07, R01–R15, the 12
  defects of the cosf study re-checked on eaee2d4, and two fragile waits (a recurrence);
- `skills/qa/SKILL.md`: crossing features with modes, recording in past-misses,
  escape rate, and fresh evidence before a cut;
- `skills/red-team/SKILL.md`: one-agent review of a change, covering the diff,
  siblings and callers, with "no sibling proof, no merge".

`AGENTS.md` is unchanged: it is at 99 lines, so the two skills link the new files instead.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-21: lessons of the beta.2 consolidation (plan v3, step 7)

**Source**: the owner, 2026-11-21 ("I approved, go ahead"), after step 5 (H-03,
H-04, H-05, H-07, H-08: #52–#56) and two P8 rounds.
**What**:
- `skills/qa/references/past-misses.md`: rows RR06–C02 that #50 listed twice are now
  listed once; C01–C07, C09, C10 and C13 marked fixed with their PRs; new rows X01–X10
  (defects found by red-team, blind QA, ChatGPT and CI during step 5), S1–S2 (the P8
  rounds, seeded) and E01–E06 (older P3 bugs found on the way, open); two classes
  added (`validate-raw`, `os-format`).
- `reference/review-lessons.md`: L6, L13, L17 and L20 now held at rung 1–2; L12, L15
  updated; new lessons L18 (check before normalising), L19 (pin a stored format with a
  hand-spelled fixture), L20 (identity), L21 (OS encoding in hand-built test data),
  L22 (P8 seeds inside the diff hit a ceiling).
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-21: C14 and L16 fixed by the manual-case check (plan v3, step 7, P4)

**Source**: the owner's approval of step 7's `.agents/` changes (2026-11-21).
**What**: past-misses C14 and review-lessons L16 marked fixed, held at rung 2 by
`crates/xdevcmd/tests/manual_cases.rs`.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-22: lessons of H-10 (plan v3, step 7 follow-up)

**Source**: the owner's approval of step 7's `.agents/` changes (2026-11-21), applied to
the last fix before freeze (#61).
**What**: past-misses C08, C11 fixed; X11–X13 (red-team and ChatGPT on #61), W03 (a
flaky core test on ARM; fragile-wait recurs a third time, now outside `pty.rs`), E07
(open); review-lessons L23 (unwinding runs `Drop`), L24 (a made-up result goes down the
real path).
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-22: a 0.4.x backlog in ROADMAP.md

**Source**: the owner, 2026-11-22: "please add it to backlog (consider it is rare, thus
not much inconvenient now)", after the hand test of two interrupted jobs; and the
deferrals the owner chose before the freeze (H-06, cosf #12, E01–E07).
**What**: `plan/ROADMAP.md` gets a section "After beta.2: 0.4.x" with those items.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-23: "skip all" for failed files, in the 0.4.x backlog

**Source**: the owner, 2026-11-23: "okay, ghi vào backlog nhé bro", after the hand test
of a network share pulled mid-copy (Retry and Skip felt the same).
**What**: one row in `plan/ROADMAP.md`, "After beta.2: 0.4.x", then reworded with the owner
the same day: Skip all told apart from R, S, Pause and Esc; and "several interrupted
jobs" widened into **Pending tasks** (pause, a list of jobs not finished, manual resume),
the owner's design; the list also shows background jobs, to bring one back (the owner
chose this over a separate command before the cut).
**Promoted by**: XDevTeam (approval = merging this branch). Written by Claude Code.

## 2026-11-23: ROADMAP's beta.2 row brought up to date; a journal fuzz target in 0.4.x

**Source**: the owner, 2026-11-23, approving the cut preparation ("duyệt đề xuất nhé"),
which named this edit.
**What**: `plan/ROADMAP.md`: the 0.4.0-beta.2 row says what is left (a fuzz run,
the cut; lanes the owner's call, else 0.5); one 0.4.x row, a fuzz target for
the journal reader.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.

## 2026-11-23: the release path gets beta.3 (refactor) and beta.4 (languages)

**Source**: the owner, 2026-11-23: "beta.3 là refactor => chấp nhận rủi ro và retest …
beta.4: i18n … EN, VI, JA preview; rc.N bug fix; stable v0.4.0 khi rc.N đã okay", with
Plan B (rc from the beta.2 tag if the plan slips). The languages decision of 2026-11-15
had reached the owner's notes and the handoff but never `ROADMAP.md`.
**What**: `plan/ROADMAP.md`: rows for beta.3 and beta.4, rc.N, the 0.4.0 condition,
Plan B, and the Release path decision. Then the same day: beta.3's timebox (one week, to
2026-11-30, the Plan B trigger) and the backlog's features kept for 0.4.x or 0.5; the
beta.2 row says fuzz passed.
**Promoted by**: XDevTeam (approval = merging this PR). Written by Claude Code.
