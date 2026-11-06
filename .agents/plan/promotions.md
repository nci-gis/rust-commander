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
