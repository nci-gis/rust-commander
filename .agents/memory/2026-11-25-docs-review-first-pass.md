# First docs review (docs-review skill, pass 1)

**Date**: 2026-11-25
**Agent**: Claude Code (7 read-only subagents, one per page group)
**Confidence**: High for FALSE items (each has code/test evidence); Medium for UNVERIFIED
**Status**: Archived — the owner agreed to D1–D5 (2026-11-25); applied in `83b0740` (code) and `f912ce6` (docs), ledger written
**Source**: `/docs-review` first full run, code at `26d055b`
**Review-by**: 2026-12-15

## Problem

No page under `docs/` or the root READMEs had ever been checked claim by claim
against the code. The ledger `.agents/ledgers/docs-review.tsv` is empty.

## Finding

About 470 claims checked across 15 pages: ~420 TRUE, 41 FALSE, ~20 UNVERIFIED.
Three FALSE items look like code bugs rather than doc errors. `DOC_PATHS` is
too narrow on almost every page: several FALSE items sit in code the page's
entry does not list, so "code changed" would never have flagged them.

## FALSE, by page (proposed doc fix in brackets)

### docs/guide.md
1. `[viewer] images = "none"` is not a value; the whole config is refused. [use `"blocks"`; also skipped under tmux/screen]
2. "images … turns it off": no off value. [reword]
3. Job error buttons are Retry / Skip / **Abort (Esc)**, not Stop.
4. Alt+C with no room: says how to clear the line. [add]

### docs/config.md
5. Theme `[style]`: "leaving a line out keeps the base's look". A `[style]` table starts from plain (`decor.rs::Decor::from_file` ignores the base). **Doc or code?** (see D2)
6. `commands.toml` (F1 recent commands) missing from "Files xdevcmd keeps" and from the read-only bullet.
7. `%f` "inside single quotes" advice breaks on names with `'` (POSIX quoting is backslash). [recommend `sh -c 'wc -l "$1"' sh %f`]
8. "A relative folder is skipped": not for `XDEVCMD_CONFIG`, which is taken as written.

### docs/shell.md
9. "Q to stay where you started" → Alt+X q.
10. "Anything else after Alt+X goes to the program" → except q.
11. History written to disk also by F2 / "Save history as…", not only by log.
12. Wheel in a full-screen program without mouse support sends arrow keys. [add]
13. "Back (Alt+, or Backspace)": Backspace is Parent.
14. Ctrl+C message differs on Windows and for a new shell.
15. Alt+C wording ("quoted cd").

### docs/security.md (+ SECURITY.md)
16. PATH-lookup claim: an `[[open]]` rule without `terminal=true` uses `Command::new`, which can search `.`/empty PATH entries. **Code bug?** (see D1)
17. "A config others can write is refused": world-writable only (group-writable is fine).
18. "The engine checks again": only root, home and `.`/`..`.
19. "cargo deny on every change": PRs to long-lived branches + weekly.
20. "7 MiB archive": the limit is on the binary.

### docs/development.md
21. `feat/<topic>` pushes do not run CI (only `feat/v[0-9]*`).
22. "about 400 tests, ~30 s": 1,118 tests. [drop the number or say "over 1,000"]
23. "150 ms budget" for bench: a hand check, not enforced.
24. "peak memory": Linux only.
25. release-check compares Cargo.toml/CHANGELOG only, not "another commit".
26. Archive contents miss `lang/`.
27. Debug-log word rule: also keeps `+`, cuts at 40 chars, adds a `job` field.
28. `tests/…` paths need the `crates/xdevcmd/` prefix.

### docs/benchmark.md
29. APFS sample shows memory macOS cannot report.
30. "memory used": Linux only.

### README.md
31. "Windows ConPTY: start-up only": `windows_e2e.rs` also draws, runs an F12 command, quits.
32. "about 950 tests": 1,118.

### docs/language.md
33. `{x}` / `{{` rule for texts without params: `Catalog::text` never substitutes, but `xtask lang check` flags it. **Code or doc?** (see D3)
34. "A broken file is named on the status line": only when its stem is the code in use, and only the first note.
35. Key bar cut "to keep all ten": only when there is room.
36. "lang check lists all of the above": overclaim.

### docs/troubleshooting.md
37. `TERM=linux`: only the default/`dark` theme gives way to the 16-colour theme; a chosen theme is fitted to 16.
38. Debug/crash logs go beside `XDEVCMD_STATE` when set. [add]

### docs/how-to-test.md
39. `--small` keeps the full 1 GiB big file (≈1.1 GiB made, ≈2.1 with copies), but doc and `HELP` say 0.6/1.1. **Code bug** (`..Sizes::SMALL` with `big: sizes.big`). (see D4)
40. `--clean` also refuses while the launched xdevcmd runs, or if the folder is not `xdevcmd-try` with a marker.

### docs/known-issues.md
41. Non-replacing rename row: folders and links are check-then-rename on Windows on every drive and wherever `RENAME_NOREPLACE` is missing; files on FAT/some shares.

## UNVERIFIED (kept as is unless the owner wants tests)

- shell.md: `xc`, the bash snippet and completion scripts are run by no test.
- compatibility.md: "a journal it cannot read is left alone" — `journal::tidy` skips unknown lines but acts on known ones. Reword to that.
- config.md: 256-colour gradient "coarser"; shell programs keep their colours; "delete always asks" (comment only).
- troubleshooting.md: terminal panel background; "nothing is ever sent" (by absence).
- how-to-test.md: "CI runs these checks for every release" — CI runs on push/PR, no release trigger. Reword.
- known-issues.md: temp-file tidy "this machine and account only" — no enforcing code found.

## Omissions worth adding (not false)

- config.md: world-writable config refused with the same dialog; E not offered in read-only/restricted; `auto_refresh_ms` takes 0 or 100–60000.
- how-to-test.md / development.md: nothing mentions `--timings` or `xtask manual`.

## DOC_PATHS proposal (`xtask/src/docs.rs`)

Widen each entry to the code each page actually describes, per the subagent
lists: e.g. config.md + `decor.rs`, `commands.rs`, `dirs.rs`, `color.rs`,
`app/settings.rs`, `app/open.rs`, `session.rs`, `safety.rs`, `crash.rs`,
`debuglog.rs`, `tempfiles.rs`, `viewer.rs`, `walker.rs`, `lang.rs`;
shell.md + `keys.rs`, `app/input.rs`, `termlog.rs`, `mouse.rs`, …;
compatibility.md → `main.rs`, `config.rs`, `keys.rs`, `theme.rs`, `journal.rs`,
`budgets.rs`, `smoke.sh`; drop unused entries (`graphics.rs` from
troubleshooting; `manual.rs`, `tests/manual`, `faults.rs` from how-to-test).
Trade-off: wider lists mean more "code changed" noise; `known-issues.md`
already maps to all of `crates`.

## Decisions for the owner

- **D1** `[[open]]` program lookup: fix code (resolve with `on_path`, like the shell does) or soften security.md?
- **D2** theme `[style]`: fix code (start from `base.decor`) or fix the doc?
- **D3** `{x}` in texts without params: make `Catalog::text` and `lang check` agree (which way?) or just fix the doc?
- **D4** `tryout --small`: fix the code (use SMALL's 512 MiB) or change doc + HELP to 1.1/2.1?
- **D5** Apply all doc fixes 1–41 + UNVERIFIED rewordings + widen `DOC_PATHS`, then write ledger rows.

## Evidence

- Subagent reports in session `session_012XwdeedZGaoDURkD3DoHEw`, each with file/function/test per claim.
- Code at `26d055b` on `feat/v0.4.0-beta.4`.

## Recommendation

D1–D4: fix the code (all four are small, testable, and the doc states the
safer or intended behaviour). D5: yes, in one commit per page group so the
ledger rows point at reviewable diffs.
