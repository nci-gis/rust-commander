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
