# xdevcmd

<!-- a2scaffold:start -->

> Claude Code reads this file, not `AGENTS.md`. It is a stub: the project
> knowledge lives in `.agents/`, imported below.

## Shared knowledge base

`.agents/` holds the directory layout, authority rules, load order, and the
pair-programming workflow. Treat it as part of these instructions.
Agents that expand `@` imports load it inline; others open it directly.

@.agents/AGENTS.md

<!-- a2scaffold:end -->

## Your instructions here

Claude Code only. Project knowledge is in `.agents/` — start with
`.agents/context/project.md`.

- Commit in this repository with `GIT_AUTHOR_NAME`/`GIT_AUTHOR_EMAIL` unset, so the
  repository's own identity in `.git/config` is the one recorded; check
  `git log -1 --format='%an <%ae>'` before pushing.
