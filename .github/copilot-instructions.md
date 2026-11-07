# copilot-instructions.md — xdevcmd

<!-- a2scaffold:start -->

> Loaded by GitHub Copilot for every request in this repository. Copilot does
> not expand imports, so this stub restates what the others import.

## Shared knowledge base

`.agents/` holds the directory layout, authority rules, load order, and the
pair-programming workflow. Treat it as part of these instructions.
This harness does not expand `@` imports, so **open it first**:
[AGENTS.md](../.agents/AGENTS.md).

<!-- a2scaffold:end -->

<!--
Seeded once, then yours. This section sits outside the generated block, so
re-running `a2scaffold` will not touch it. Replace the placeholders below with
your project's real principles and keep them in step with the canonical list in
`.agents/context/philosophy.md`.
-->

## Philosophy

These principles decide the close calls. The full version — with rationale
and worked examples — lives in
[philosophy.md](../.agents/context/philosophy.md).

1. **Ask before assuming.** The human partner owns intent. Confirm before
   changing public behaviour.
2. **Small steps, frequent checks.** Incremental edits with test runs beat
   large rewrites.
3. **Stability over speed.** Knowledge is promoted only after it is
   validated, never on first sighting.
4. **Explicit over implicit.** No hidden fallbacks, no silent network
   access, no magic defaults.
5. **Preserve what works.** Existing contracts and conventions stay intact
   unless the human decides otherwise.

<!--

## Your instructions here

The title above, the principles above, and anything below this comment are
yours. Re-running `a2scaffold` replaces only the block between the markers.

-->
