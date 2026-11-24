# beta.3 design review: platform interface, SOLID, deletion ratio

**Date**: 2026-11-24
**Agent**: Claude Code
**Confidence**: Medium
**Status**: New
**Source**: Round 035, after W9 (the owner, 2026-11-24: "xem xét phần interface hóa
cho cả 3 platform … design-patterns, clean code, SOLID. Đo tỷ lệ xóa dòng")
**Review-by**: 2026-12-31

## Problem

After the W1–W9 refactor: are the platform differences behind one interface, does the
code keep to SOLID and clean code, and did beta.3 delete as much as it added?

## Finding

**Deletion ratio** (deleted / added lines under `src/`, non-merge commits
`v0.4.0-beta.2..` incl. #86):

| set | added | deleted | ratio |
|---|---|---|---|
| everything | 8,355 | 7,207 | 0.86 (inflated by W9 moves) |
| without the W9 moves | 1,443 | 438 | **0.30** (beta.2 window: 0.235) |
| refactors W1–W8 only | 856 | 363 | **0.42** |
| walker (5 PRs) | 411 | 72 | 0.18 |

Higher than beta.2, still below yazi (0.53–0.64) and MC (0.74). The walker pulled the
ratio down; the W9 moves say nothing (a move adds and deletes the same lines).

**Platform interface.** 272 `cfg(unix|windows|target_os)` sites in `src`. The OS
primitives are each defined once, but in five homes:

| primitive | where | platforms |
|---|---|---|
| file system kind, network check, free space | `plan.rs` | linux / macos / windows / other |
| journal lock, process alive | `journal.rs` | unix / windows |
| directory sync | `faults.rs` | unix (Windows: nothing to do) |
| cross-device error | `ops/copy.rs` | unix / windows |
| trash location, sticky bit | `ops/remove.rs` | linux / macos / windows |

One concept is defined four times: **"is executable"** (`mode & 0o111`) in
`opener.rs` (×2), `pane/term.rs::on_path`, `theme.rs::is_executable`; the Windows
sides are three different extension lists, each for its own purpose (colour, open,
run in a console), so only the unix bit test is the shared part.

**SOLID / clean code.**
- SRP: `App` 86 → 57 fields (W9b); still the widest type. `Worker` has 44 fields
  and one `impl` over `worker.rs` + `copy.rs` (~1,420 lines).
- OCP / LSP / ISP: closed enums (`Kind`, `Question`, `Answer`) matched exhaustively;
  traits are few and narrow (`archive::Hooks`, 11 methods, one implementer;
  `format::Cells`).
- DIP: `xdevcmd-core` has no UI dependency; the worker talks only through the
  `Event` channel; time, environment and faults are injected (`Walker::tick(now)`,
  `dirs::*_from`, `faults`).
- Patterns in use: Command (`Request`), Observer (`Event` channel), Strategy
  (`fastcopy::Engine`), Facade (`ops::start`), a Task future (`task::Task`).
- Debt left by W9: `pub(super)` widened inside `ops`; `ShellPane`'s fields keep
  their `term_` prefix (a rename, not a move).

## Evidence

- `git show --numstat` per commit over `*/src/*`, `v0.4.0-beta.2..feat/w9-app-state`.
- `grep -rn "cfg(" crates/*/src`; `qa defs`: App fields 57, no file over 2,500 lines.

## Recommendation

**Do**: one `xdevcmd_core::os` module with the primitives above as plain functions,
one body per platform inside each (a compile-time interface: no trait object). Move,
not rewrite. Add `os::has_exec_bit(&Metadata)` for the unix half of "is executable";
keep the three Windows extension lists where their purpose is.
Platform-specific code only where behaviour differs (lock, trash, fs kind, free
space); everything else shared.
**Don't**: a `trait Platform` with three impls: one platform per build, so a trait
adds dispatch and test doubles nobody needs.

## Promotion candidate?

- [ ] `context/` — stable, broadly applicable, seen more than once
- [ ] `skills/` — a reusable procedure with a clear trigger
- [x] Not yet — the owner chose W10 in beta.3 (2026-11-24, option A); done as
  `xdevcmd_core::os`, one move PR.
