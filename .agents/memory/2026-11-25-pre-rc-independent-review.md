# Independent review before the rc freeze (4 lenses)

**Date**: 2026-11-25
**Agent**: Claude Code (4 read-only subagents: algorithms, UX, market, release)
**Confidence**: High for items marked verified; Medium otherwise
**Status**: Archived — every finding has a row in `2026-11-26-pre-rc-findings-ledger.md`
**Source**: the owner: "deep research and review … sau beta.4 là cut rc.X, không thêm tính năng"
**Review-by**: 2026-12-10

## Facts (HEAD 4f89fa2)
19 days old, 874 commits, 1 human author, ~87 % AI co-authored. ~67k lines Rust + 13k
test lines; 1,122 tests. 2 stars, ~95 release downloads, crates.io 12. README: no GIF.

## Defects (verified in code by the main session)
1. plan.rs:753 `folded = ignores_case != Some(false)`: with case unknown (destination
   folder name has no letters, e.g. `2026`, `照片`), a case-sensitive FS folds `a.txt`
   and `A.txt`; a stale file can be planned Identical → Skip, no question. Reproduced
   by the reviewer (25/40 pairs). Fix: fold only on `Some(true)`; else exact keys,
   `complete = false`. P0.
2. Deep trees: planner (plan.rs:632 `thread::spawn`, recursion at :936) and copy walk
   (copy.rs:325) overflow the default 2 MiB stack at ~1,300–1,600 levels (release):
   abort, whole app dies. Fix: `thread::Builder::stack_size` or an explicit stack. P0.
3. Languages not embedded: lang.rs:316 reads `lang/` beside the binary or config dir
   only; binstall/`cargo install` give the binary alone → no vi/ja. P0 for beta.4.
4. `--help` (main.rs:86) "Q quits without it": the key is Alt+X q.
5. Same-drive move of a folder onto an existing folder: ENOTEMPTY, "0 moved, 1 failed"
   though the plan shows Merge (copy.rs:1249/1339); open since 2026-11-17 QA. Before rc.

## Release / promise (reviewer, not all re-checked)
- B3 (sync before job end) and C1–C9 planned for rc.1 change the data path: belongs in
  beta.4/beta.5, not a fixes-only rc.
- Journal: an unknown line is skipped and the rest acted on; a newer job read by an
  older version could finish wrongly. Add a must-understand marker before 0.4.0.
- No golden test pinning config keys / action names / flags for the 0.4 promise.
- Toolchain and actions not pinned in release.yml.
- Overclaims: README "whole test suite at every pull request", "tests each release by
  hand", "provenance on every release"; security.md "cannot hang" vs NFS hard mounts.
- Bus factor 1 incl. crates.io ownership; no Dependabot.

## UX
Strengths seen: kill -9 mid-copy → "Interrupted", Continue finished correctly; F12,
Alt+C, Alt+G, Alt+Enter work with no setup; quit warns about a running program.
Gaps: README never leads with resume or the shell workflow; ASCII screen out of date;
"xdevcmd <pid>" top-right reads like a bug (it is deliberate); Ctrl+G appends to the
prefilled path; GNOME Terminal takes F10/F1/Alt+F7 by default (undocumented).

## Market
yazi ~43k stars (async, plugins, previews, MP4 hero); superfile ~20k (looks, winget/
Scoop/Homebrew, Product Hunt). Same niche, newer: Rat Commander (Rust, two-panel, more
features, Oct 2026), Oxide, Zeta (dual pane + terminal). Windows has Far Manager.
r/opensource bans AI-built projects (Oct 2026, verified); Rust project policy: disclose
LLM content (verified); an r/rust rule was claimed but not verified.
Moat: crash-safe resume (no rival shows it), shell-integrated two-pane workflow, vi/ja.
Scenarios (judgment): niche 100–500 stars ~55 %; breakout ~10 %; <50 stars ~35 %.

## Recommendation
See the session reply: P0 fixes in beta.4; GIF scripted (vhs) right after the rc.1 UI
freeze; soft launch at rc (VI/JA communities, r/commandline), hard launch at 0.4.0 with
packages and an open AI-use statement.
