# Security review before 0.2.0: what was fixed, what is left

**Date**: 2026-11-07
**Agent**: Claude Code
**Confidence**: High (fixed items have tests; open items are from reading)
**Status**: New
**Source**: a security review of `git diff ede93de..dev -- crates/` (0.1.0 → 0.2.0),
done before tagging 0.2.0, with proof-of-concept runs for the confirmed findings
**Review-by**: 2027-02-01

## Problem

0.2.0 added ways to run programs (Enter opens files, `[[open]]` rules, programs in a
terminal tab), writes (Settings), theme files, temporary files and undo. Was anything
open before release?

## Finding

Fixed before the tag (each with a test):

- A program run in a terminal tab has no shell, and `foreground_program` took it for
  a shell at its prompt, so Alt+C and the user menu typed into it. Program tabs are now
  always busy.
- Open with…, `apply_rule`, `ask_to_run`, F4 and F7 did not check `--root` (Enter and
  F3 did). F7 also followed `..` out through `create_dir_all`.
- Enter launched the system's handler for any type not on a short denylist; on every
  system some types run code (`.py`, `.jar`, `.scf`, `.terminal`, Wine `.exe`…). Now
  the denylist is longer, and Enter opens without asking only an allowlist of document
  types (`opener::document`); others ask "Open it?".
- Quick view of a picture opened FIFOs and devices (`metadata().len()` is 0 for both):
  a hang, or `/dev/zero` read until out of memory. Now only regular files, and the JPEG
  read is bounded.
- Batch files in a terminal tab: `,` `;` `=` are cmd's argument splitters, and a batch
  path with a space gets quoted, which `cmd /c` may strip. Both refused.
- The temp marker was refreshed only from the event loop, so a long editor session let
  a second xdevcmd sweep live files, and `write_private` then re-created the directory
  with default permissions. Now a thread refreshes it, and a gone directory is never
  re-created.
- Undo's `remove_dir_all` after an emptiness check could delete a file created in
  between; now bottom-up `remove_dir`. Settings save checks the mode itself and writes
  through a symlinked config. Theme names refuse `:`.

A second, final review (over the fixes and Round 012) found and this branch fixed:
Alt+F5 packing outside `--root`; archives reached through a link out of the root;
Shift+F4 on a link out; a configured opener skipping the "Open it?" question; a FIFO
swapped in between the check and the open; batch files found on PATH by a bare
`.cmd` name; the temp directory used by name rather than identity.

## Open (not fixed in 0.2.0)

- `batch_rename::apply` checks the target is free, then `rename`s, which overwrites a
  file that appears in between. A no-replace rename (`renameat2(RENAME_NOREPLACE)` on
  Linux, `renamex_np(RENAME_EXCL)` on macOS, `MoveFileExW` without
  `MOVEFILE_REPLACE_EXISTING` on Windows) closes it.
- portable-pty on Unix resolves relative `PATH` entries against the tab's working
  directory (the browsed directory). Only matters with `.` or an empty entry in PATH.
- The batch-file refusal is unit-tested everywhere but has not been tried on Windows.
- Active content in documents Enter still opens without asking (PDF, RTF, docx)
  depends on the viewer's own safety; the allowlist keeps out what runs scripts or
  macros by design.

## Evidence

- Pull request "Docs and security review before 0.2.0" on `feat/pre-release-review`.
- Tests: `alt_c_does_not_type_into_a_program_run_in_a_terminal_tab` (pty),
  `the_root_holds_against_dotdot_backspace_and_symlinks`,
  `enter_opens_only_known_documents_without_asking`,
  `settings_writes_through_a_linked_config_and_keeps_the_link` (render),
  `only_a_regular_file_is_previewed` (image.rs), the `batch_tests` in `pane/term.rs`.

## Recommendation

**Do**: fix the open items in a round after 0.2.0; try the batch-file refusal on a
Windows machine.
**Don't**: treat an extension denylist as a boundary — what "open" runs is the
system's choice; keep the allowlist in front of it.

## Promotion candidate?

- [x] `reference/security.md` — the Enter allowlist, program tabs never typed into,
      root checks on every path that opens or writes, by a person
- [ ] `skills/`
- [ ] Not yet
