# Pre-rc review: every finding and where it went

**Date**: 2026-11-26
**Agent**: Claude Code
**Confidence**: High (status read from the commits named)
**Status**: New
**Source**: `memory/2026-11-25-pre-rc-independent-review.md` (4 lenses). The owner,
2026-11-26: "Tất cả findings của research đã fix hoặc log lại hết chưa?"; after beta.4
only rc.N, fixes only.
**Review-by**: at rc.1's plan

## Problem

The pre-rc review found more than one beta takes. This file lists each finding once, with
its status, so none is lost between beta.4 and 0.4.0.

## Finding

Status: **fixed** (commit) · **rc** (a fix, no new behaviour) · **0.4.0** (before the
stable release, not code) · **later** (0.4.x / 0.5) · **owner** (a decision) · **kept**
(judged not a defect, with why).

### Algorithms and data

| # | Finding | Status |
|---|---|---|
| A1 | Plan folded case where the case rule is unknown: a stale file skipped as identical | fixed `6871143` (test: 40 pairs) |
| A2 | Deep trees overflowed a 2 MiB thread stack: the app aborted | fixed `6871143` (walk_thread; 1,900-level test) |
| A3 | Same-drive move of a folder onto a folder: "0 moved, 1 failed" | fixed `5f75aad` (merge; two tests) |
| A4 | Kill tests use SIGKILL: page cache survives, so fsync order is untested | later: a harness that drops unsynced writes (faults.rs records sync order) |
| A5 | The listing-vs-stat test skipped the unknown-case rule | fixed `6871143` |
| A6 | Cross-drive move walks and removes by path: a folder swapped for a link mid-walk is followed (SUSPECT, not reproduced) | later: walk by handle (openat) in 0.5 |
| A7 | `Ident` lacks ctime: same size and restored mtime counts as unchanged | later |
| A8 | Journal recovery of a big rename-many is O(n²) (`journal.rs` unren) | later |
| A9 | Group-commit thread probing looks over-built | kept: measured gains on Windows (security.md) |
| A10 | Plan and job judge conflicts in two places (`plan.rs`, `copy.rs`) | later: a differential test plan-vs-job |

### User interaction

| # | Finding | Status |
|---|---|---|
| U1 | README "open in the current directory" vs session restore | fixed (README quick start, 2026-11-26) |
| U2 | `--help` said Q | fixed `5f75aad` |
| U3 | vi/ja missing after `cargo install` / binstall | fixed `019018c` (built in) |
| U4 | "xdevcmd <pid>" top-right reads like a version | owner: it is deliberate (shells use it); relabel "pid N" is new UI text, so rc only if called a bug |
| U5 | Ctrl+G appends typed path to the prefilled one | rc |
| U6 | Letters in panes do nothing; no vim preset | later (docs snippet for `[keys]`) |
| U7 | "1 items"; F1 list narrow at 160 cols | rc ("1 items"); later (F1 width) |
| U8 | README text-art screen out of date | 0.4.0 (replaced with the GIF/screenshot after rc.1) |
| U9 | GNOME Terminal takes F10/F1/Alt+F7; tmux/screen Ctrl+A | rc (troubleshooting.md) |
| U10 | Windows newcomer install path weak (unsigned zip) | 0.4.0 (Scoop bucket) |
| U11 | README never leads with resume or the shell workflow | 0.4.0 (launch README) |

### Release and sustainability

| # | Finding | Status |
|---|---|---|
| R1 | Languages not embedded for binstall/cargo install | fixed `019018c` |
| R2 | Journal: an unknown line is skipped, the rest acted on; a newer job read by an older version could finish wrongly | **owner**: a must-understand marker changes the journal format, so beta.4 or never-in-0.4 (rc is fixes only). Today's risk is only a downgrade mid-job |
| R3 | No frozen-surface test for the compatibility promise | rc (test only) |
| R4 | B3 and C1–C9 planned in rc.1 change behaviour | fixed: B3 in beta.4 `c37a1b1`; C1–C9 are fixes, stay in rc |
| R5 | Release toolchain and actions not pinned | rc (CI only) |
| R6 | Size headroom under 7 MiB | measured: fat LTO `4f89fa2`; +145 KB for built-in languages; Windows measured at the cut |
| R7 | Intel macOS archive never run; Windows smoke runs `--version` only | 0.4.0 (owner hand test) |
| R8 | README overclaims (every PR, each release by hand, provenance always) | fixed `5f75aad` |
| R9 | security.md "cannot hang" vs NFS hard mounts | kept: the line is about hostile names, files and archives; the NFS case is in known-issues |
| R10 | known-issues "as of 0.4.0-beta.3" | at the cut |
| R11 | crates.io publish is two manual steps | 0.4.0 (`--dry-run` both, ROADMAP) |
| R12 | packaging/README archive contents out of date | fixed (2026-11-26) |
| R13 | ROADMAP: winget "needs a signed build" | ROADMAP softened; verify before planning item 48 |
| R14 | No MSRV policy for 0.4.x | rc (compatibility.md) |
| R15 | Bus factor 1, crates.io ownership; no Dependabot/CODEOWNERS | owner |
| R16 | `.mailmap` and commit authors carry a personal e-mail | owner (git history is public by nature) |

### Market

| # | Finding | Status |
|---|---|---|
| M1 | No GIF; record when? | ROADMAP: after rc.1, scripted |
| M2 | Launch venues and AI-use perception (r/opensource bans AI-built projects) | ROADMAP 0.4.0 row: soft launch at rc, real at 0.4.0, a plain build statement |
| M3 | Rivals in the niche (Rat Commander, Oxide, Zeta); Far on Windows | kept as context for the launch README (lead with resume + shell) |

### beta.4 red-team (2026-11-26; three agents, all SIGN WITH AMENDMENTS)

| # | Finding | Status |
|---|---|---|
| T1 | Ctrl+Z after a same-drive merge moved the destination's own folder back | fixed (undo says it cannot; render test fails without the fix) |
| T2 | A new file whose batched sync failed stayed under its name, counted done | fixed (removed, not counted; test) |
| T3 | New links not in the fresh batch: a folder of links only was not synced | fixed (their folder synced; test) |
| T4 | 128 open handles per fresh batch: EMFILE under macOS' 256 | fixed (32) |
| T5 | `walk_thread` panicked if a 256 MiB stack could not be reserved | fixed (falls back to the default stack) |
| T6 | Same-drive move renames have no folder sync before "done" (btrfs) | rc (sync the destination folders before the done line) |
| T7 | A merge counts top-level children; the bar adds a folder's 4096 bytes | rc (cosmetic) |
| T8 | `only_dirs`/`remove_empty_tree` in undo recurse on the UI thread (1 MiB stack on Windows) | rc |
| T9 | English words as `{params}`, `format!` status lines, the compare "copy N" button, palette groups, "zoomed" matched by its English | fixed (one key per variant; vi, ja) |
| T10 | `xtask lang check` did not report missing keys | fixed (a note each; the shipped files are whole) |
| T11 | A file could claim `$__code = "auto"` or `"en"` | fixed (skipped) |
| T12 | Zero-width characters, U+061C, U+2028/9, BOM allowed in translations | fixed (both checks) |
| T13 | A broken language file is named only when its stem is the code; `lang_note` hidden behind config/theme notices | rc |
| T14 | `[[open]]` program lookup is Unix only | documented (security.md: Windows uses the system's search) |
| T15 | A killed terminal writes a spurious crash log (EIO in restore) | rc |
| T16 | smoke.sh did not check `lang/` in the archive | fixed |
| T17 | language.md link broken in the offline docs | fixed |
| T18 | CHANGELOG missed fat LTO and the testkit changes | fixed |
| T19 | `cargo install` from crates.io builds without the workspace's fat LTO and strip | 0.4.0 (note in the install docs) |
| T20 | docs status: 13 pages changed since the first review | at the cut (review the changed pages) |
| T21 | release-check: version, dated CHANGELOG section, branch on dev | at the cut (the release flow) |
| T22 | Check of the amendments: a lone new file whose own batch sync failed was still counted done (the sync ran before the caller counted it) | fixed (the batch syncs after the file is counted; `made` entry dropped; test fails without the fix) |
| T23 | A background merging move finishing later clears a newer move's undo | rc (tie the undo to its job); only blocks an undo |
| T24 | A merge killed after `remove_dir(src)` and before the journal line: the resume sees no merge, undo could move the folder back | rc (SUSPECT; record the merge in the journal or refuse undo after a resume) |
| T25 | Core English around translated texts (`Undo::plan`'s frame, `Summary::describe`, "undone: …", Mark/Unmark menu labels) | rc / known issue "some words stay English" (core text not translated in 0.4) |
| T26 | ZWNJ/ZWJ refused would block Persian and Indic translations | fixed (allowed again; ZWSP, BOM, U+2028/9 still refused) |
| T27 | pty flakes on 2026-11-26: `a_list_from_the_history_is_worked_on_and_come_back_to` (CI ubuntu-latest, run 45734947983) and `alt_c_after_a_continued_line_opens_a_new_shell` (local); both passed on re-run, neither touches the changed code | rc (find the fixed waits behind them; past-misses W03) |
| T28 | R2 decided by the owner (2026-11-26): must understand = parse and understand; the code names the layouts it reads | fixed `bf149c3` (v F R, app line, JOURNAL_READS; a newer journal left whole) |
| T29 | ja not yet read by a speaker | fixed `6ea8376` ($__status = prereview, shown in Settings) |

## Evidence

Commits on `feat/v0.4.0-beta.4`: `6871143`, `019018c`, `5f75aad`, `c37a1b1`, `00ff48a`;
ROADMAP edit logged in `plan/promotions.md` (2026-11-26).

## Recommendation

Open rc.1's plan from the **rc** rows here and C1–C9. Ask the owner about R2 before the
beta.4 tag, since after it the journal format is frozen for 0.4.

## Promotion candidate?

No.
