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
