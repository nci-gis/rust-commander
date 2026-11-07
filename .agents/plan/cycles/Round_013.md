# Round 013: 0.2.1 — the macOS logo, theme files, leftover safety items

**Status**: In Progress
**Part of**: [ROADMAP.md](../ROADMAP.md), items 7b, 4b, 8b, and 5c and 15 (added by the
owner: "Yes, add both", 2026-11-07)
**Date started**: 2026-11-07 (owner: "bro tạo nhánh feat/v0.2.1 và làm nhé. Chỉ làm
trên nhánh này, ko tạo PR tới khi mình test okay nhé")
**Date completed**: —

## Goal

Fix what the owner found after 0.2.0 — the About logo striped on macOS, no theme files
in the release — and close the safety items the 0.2.0 review left open, so the first
packaged version carries them.

## Plan

- [x] 7b. The About logo drawn with background colour only: one colour per cell, no
      half blocks, so the terminal's font cannot stripe it. Any picture cell whose two
      halves are one colour is drawn the same way.
- [x] 4b. `themes/` with three examples (Nord, Gruvbox dark, Solarized light) in every
      release archive; `xdevcmd --print-theme NAME`.
- [x] 15. Idle CPU near zero: draw only when something changed, else once a second.
- [x] 5c. Ctrl+G beside Ctrl+L to go to a path.
- [x] 8b. Bulk rename without replacing a file that appears between check and rename;
      F2 and the Linux trash the same.
- [x] 8b. Programs in a terminal tab: ignore `.` and empty `PATH` entries.
- [x] 8b. Carried tests: a hostile file name through an `[[open]]` rule; undo of a move
      that met an overwrite question.
- [ ] By hand, by the owner: the logo on Terminal.app, iTerm2, Windows Terminal and the
      old console; the batch-file refusal on Windows; Enter opening files on Linux and
      Windows.

## Do

- 2026-11-07, logo: the owner's screenshot from macOS showed the 0.2.0 logo striped
  and stepped. Every cell was `▀` with two colours; Terminal.app draws that character
  from the font, and with line spacing it does not fill its half of the cell. Redrawn
  by hand in `scripts/logo_pixels.py` on a 14 × 7 grid of cells (a cell is about twice
  as tall as wide, so two columns per row is 45°), written as a 14 × 14 PNG with every
  row twice; `paint_picture` paints a cell with equal halves as a space in background
  colour. `assets/icon-14.png` became `assets/about-logo.png`. Three layouts were
  drawn and compared; the one with the chevron's rows overlapping by a cell (a joined
  stroke) and the corners cut was kept.
- 2026-11-07, themes: the five themes are built in, so 0.2.0 shipped no files and a
  user had nothing to start from. `themes/*.toml` are copied into each archive by
  `release.yml`, beside the binary, where `theme_dirs` already looks. The theme parser
  was split out of `named_theme` (`theme_from_text`) so a test loads every shipped file
  and checks its rows stay apart at 24-bit and 256 colours. `color_name` now writes the
  16 named colours as `0`–`15`, which `--print-theme ansi` needed (it wrote `reset`).
- 2026-11-07, measured on the release binary in a 200 × 50 pty (the owner asked about
  speed and memory): first frame about 10 ms (200 ms with 100,000 files in a pane);
  memory 6 MiB (64 MiB with the 100,000 files); 200 Page Downs through them 40 ms of
  CPU; binary 4 MB. **Idle, xdevcmd uses about 5 % of a core**: the loop redraws every
  40 ms whether or not anything changed. Not fixed here; proposed in the roadmap.
- 2026-11-07, idle CPU (owner added it to the round): `perf` is not allowed on this
  machine without sudo, so the loop's parts were switched off one at a time in a
  throwaway build: drawing was the whole cost (5.1 % → 0.3 % without it; background
  polling 0.2 %). Now the loop draws when something changed: an event of any kind, a
  shell's output (a flag its reader thread sets, `term::take_output`), a job open or
  running, or `background()` reporting a change (the `poll_*` functions already said
  so); and every second otherwise (`HEARTBEAT`), for what changes without telling.
  Measured after: 0.2–0.5 % idle; first frame and scrolling unchanged; the pty tests,
  which type into shells and read the screen, pass.
- 2026-11-07, renames: `noreplace::rename_noreplace` in the core crate uses
  `renameat2(RENAME_NOREPLACE)` on Linux and `renameatx_np(RENAME_EXCL)` on macOS
  through rustix (already in the tree through crossterm; the crate keeps
  `forbid(unsafe_code)`). Elsewhere, or where the file system does not support it, a
  file is hard-linked under the new name (which fails if taken) and the old name
  removed; only a directory or a link there falls back to check-then-rename, where
  the worst case is an empty directory replaced. Used by rename many, F2 (not for a
  change of case only, where the "other" name is the file itself) and the Linux trash.
  Not used by F6's move, which asks its overwrite question first and keeps the race
  between that answer and the rename; noted for later.
- 2026-11-07, PATH: `on_path` looks a bare program name up in PATH's absolute
  directories only, on Unix, before handing it to portable-pty. Windows is unchanged:
  portable-pty searches PATH relative to xdevcmd's own directory there, not the tab's.
- 2026-11-07, carried tests: `OpenRule::argv` (taken out of `apply_rule`) and a test
  that runs `/bin/sh` with a name holding `;`, `$( )`, backticks and quotes: the program
  gets it as one argument and nothing runs. Undo of a move: a file that takes the old
  name after the plan gets F6's question; Skip keeps it, Yes replaces it.

## Check

- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
      --workspace`: all pass (2026-11-07).
- [x] Tests: `the_logo_is_one_colour_per_cell_of_the_size_about_expects`,
      `about_draws_its_logo_without_half_blocks`, `quick_view_draws_a_picture_in_half_blocks`
      (two colours: half blocks; one colour: none),
      `the_example_theme_files_load_and_keep_their_rows_apart`,
      `a_printed_theme_reads_back_as_the_same_theme`.
- [x] Tests added: `noreplace` (taken, free, the fallback),
      `a_file_that_appears_after_the_plan_is_never_overwritten`,
      `a_program_is_never_found_through_a_relative_path_entry`,
      `a_hostile_file_name_reaches_an_open_rule_as_one_argument`,
      `undoing_a_move_asks_before_overwriting_what_appeared_meanwhile`.
- [x] `cargo check` for macOS and Windows targets on Linux (the rename code is per
      system).
- [ ] Not verified: the no-replace rename on a real Mac and Windows (CI runs the tests
      there once the pull request opens); the logo on a real Mac or Windows screen (only Linux, and the
      plain-text screenshot cannot show colours); the archive layout, until a release
      build runs.

## Act

**Learnings**:

- A character's look depends on the terminal and its font; a cell's background does
  not. For anything that must look the same everywhere, draw with backgrounds.

**Promotions**:

- [ ] → context/ : —
