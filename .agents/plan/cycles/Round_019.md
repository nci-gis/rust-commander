# Round 019: Foundation for v0.4 — split app.rs, Windows end to end, housekeeping

**Status**: Review
**Part of**: v0.4.0, entry A of `.agents/memory/2026-11-10-v0.4.0-plan.md`
**Date started**: 2026-11-10
**Date completed**: YYYY-MM-DD

## Goal

Make the rest of v0.4 (B–G) cheap to add and Windows checkable, with no change a user
can see. Every later round lands in `app.rs`, so it is split first.

## Plan (kickoff review)

### 1. What changed since the plan entry

- **Round 018 is done** (Review). Windows asks `[y/N]` before starting from a panel; the
  menu bar shows `xdevcmd <pid>`; the status-line gap is fixed; `--keys` has `wait`.
- **`tests/windows_panel.rs` exists.** It is a working ConPTY harness on the Windows
  runner: portable-pty, an answer to ConPTY's `ESC[6n`, exit codes. A.2 grows from it
  rather than starting fresh.
- **The owner, 2026-11-10: "Chỉ push, chưa cần CI".** Commits are pushed to
  `feat/v0.4.0` without starting CI. Verification is the local check set, plus the owner
  on Windows. CI runs at the pull request into `dev`.
- `app.rs` is 7,739 lines. One `impl App` block runs from line 558 to 7355 and holds 170
  methods; free helpers and tests follow it.

### 2. Map: the seams already in `impl App` (line numbers at `a5a317b`)

| File (new) | Methods (first → last) | Lines |
|---|---|---|
| `app/session.rs` | `from_session` … `autosave`, `write_cwd_file`, `last_files_cwd` | ~420 |
| `app/run.rs` | `run`, `stop_step`, `turn`, `background`, `end_on_signal`, `job_running_at_all` | ~380 |
| `app/panes.rs` | `alloc_id` … `split_focused`, `close_focused`, `toggle_zoom`, `swap_panes`, `mirror_pane`, `other_file_pane`, `pane_at`, `with_files` | ~450 |
| `app/shell.rs` | `toggle_term` … `go_shell_dir` (panel, history, Save as, typed names, follow), `run_in_terminal` | ~950 |
| `app/input.rs` | `feed_key` … `perform` (groups, `on_key`, `refused`), `perform_cmd`, `copy_text`, `copy_path`, `settings_outcome`, `footer_key` | ~930 |
| `app/mouse.rs` | `on_mouse` … `open_context_menu`, `on_modal_mouse` … `selection` | ~920 |
| `app/ops.rs` | `transfer_targets` … `apply_input`, `begin_batch`, `finish_batch`, `begin_copy_as` … `show_checksum` | ~830 |
| `app/modals.rs` | `on_modal_key` … `modal_act_inner`, `begin_search` … `open_hotlist`, `save_bookmarks` | ~520 |
| `app/open.rs` | `current_file` … `edit_externally`, `open_with` … `launch`, `open_user_menu`, `run_user_command` | ~620 |
| `app/jobs.rs` | `reload_all` … `finish_job`, `pump_background_jobs`, `ask_before_quitting`, `remember_job` … `resume`, `watch_panes`, `note_trashed`, `ask_undo`, `undo` | ~520 |
| `app/compare.rs` | `compare_pair` … `show_sync_plan` | ~200 |
| `app/archives.rs` | `focused_archive` … `show_extracted` | ~240 |
| `app/pictures.rs` | `Graphic`, `PicKey`, `Prepared`, `prepare`, `draw_quick_view`, `draw_picture`, `measure_cells`, `sync_graphics` | ~450 |
| `app/draw.rs` | `bar_items`, `open_bar`, `draw_bar`, `holding`, `draw`, `draw_frame`, `draw_status`, `draw_footer` | ~560 |
| `app/settings.rs` | `open_settings`, `save_settings`, `theme_menu`, `set_theme` | ~160 |

`app.rs` keeps:
- the `App` struct;
- `new` and `boot`;
- `ModalAct`, `KeyReader`, `terminal_alive` and `tty_gone`;
- the small free helpers (`plural`, `digit`, `base64`, …);
- the tests module.

That leaves about 1,100 lines. The table is a proposal: methods that sit between seams
go where their callers are, and the commit says where each went.

### 3. Outside check (done)

- **Rust privacy, tried in a scratch crate.** A child module `app/x.rs` that starts with
  `use super::*;` sees `App`'s private fields, `app.rs`'s private helpers and its `use`
  imports. A moved method needs `pub(super)` to be called from `app.rs` or a sibling
  module. So **"moved verbatim" is verbatim plus visibility**: `fn` becomes `pub(super)
  fn` on each moved method that is called from another file. That is the only allowed
  edit.
- **ConPTY on the runner works** (Round 018). Its output is a rendered VT stream, so
  `vt100::Parser` can read a frame from it as on Unix.

### 4. Critique of the plan entry

- **"One commit per seam, the suite green at each"** stays.
  - Fifteen moves, one at a time, each checked with `cargo build`, the full test suite
    and clippy for three targets.
  - Review with `git diff --color-moved=dimmed-zebra -w`: a pure move shows no plain
    `+`/`-` lines except the visibility changes and the new file's header (`//!` doc,
    `use super::*;`).
  - A line-count check per commit: lines removed from `app.rs` equal lines added
    elsewhere, minus the header.
- **"Then tidied" is dropped from this round.** Tidying mixed into a move hides
  behaviour changes. Anything worth tidying is listed in Check, for a later round or
  for B–E as they touch the code.
- **Windows end to end is smaller than planned**, thanks to Round 018's harness:
  - lift `Run` into `tests/windows_e2e.rs`, add a `vt100` screen, and assert on frames;
  - cases: start → the panes are drawn → F12 → a prompt in the shell (`cmd`) → `echo` →
    the output is seen → F10 ends with 0;
  - no copies or mouse yet.
  It runs only on Windows, so here it is compile-checked through clippy
  `x86_64-pc-windows-gnu`. The owner runs it on Windows (`cargo test -p xdevcmd --test
  windows_e2e`).
- **Housekeeping (A.3) is the owner's**: these files are canon. Listed below, edited only
  on a yes.
- **Packages (A.4)**: Homebrew, Scoop and winget are the owner's accounts. The agent part
  is the `cargo binstall` metadata and the README's install section.

### 5. Steps

1. **Split `app.rs`** into the 15 moves above, as 15 commits, one per seam. Each commit:
   - moves the methods verbatim and changes only their visibility;
   - adds the new file's header and the `mod` line in `app.rs`;
   - is checked with fmt, clippy for three targets, `cargo test --workspace`, the
     `--color-moved` review and the line-count check.
2. **`tests/windows_e2e.rs`**: `Run` with a vt100 screen; the case in §4. Leave
   `windows_panel.rs` as it is, or move both into one file with a shared harness.
3. **`cargo binstall` metadata** in `crates/xdevcmd/Cargo.toml`
   (`[package.metadata.binstall]`):
   - `pkg-url` follows the release asset names
     (`xdevcmd-v{version}-{target}.tar.gz`, `.zip` on Windows);
   - `bin-dir` is `xdevcmd-v{version}-{target}/{bin}{binary-ext}`.
   The README's install section gains `cargo binstall xdevcmd`. Check:
   `cargo binstall --dry-run xdevcmd` on the owner's machine (it goes to the network, so
   not from here).
4. **Housekeeping, only on the owner's yes, logged in `promotions.md`:**
   - `plan/ROADMAP.md`: statuses, numbering and the v0.4 order, from the final review.
   - `reference/ui.md`: keys added in 0.3.0 (Alt+G, Alt+Enter, Shift+F12, Alt+X, Shift+F10)
     and the pid on the menu bar.
   - `reference/gotchas.md`: vt100 0.18 (NonZero sizes; the CSI 65535 @ fix) and why
     tui-term was dropped.
   - `reference/testing.md:51`: the `wait` token.
   - `context/project.md` needs no change (the unsafe rule stands).
5. **CHANGELOG**: nothing for steps 1–2; one line for binstall.

Tests that fail without the change: steps 1 and 3 change no behaviour, and the unchanged
suite is the guard. Step 2's tests are new coverage, not a fix.

Sizes: step 1 about a day, step 2 half a day, step 3 an hour, step 4 an hour once
authorised.

Risks:
- **A move that changes behaviour.** Mitigated by moves only, the `--color-moved`
  review and the full suite at each commit.
- **A `cfg(unix)` method lost on another target.** Mitigated by clippy for the three
  targets at each commit.
- **Windows e2e flakes, or ConPTY differs on the owner's Windows build.** Mitigated by
  generous waits and answering `ESC[6n`.
- **Conflicts with work outside this branch.** None open: `dev` has nothing newer.

### 6. The owner's decisions

1. Go for step 1 with this layout, or name another split.
2. Step 2: a separate `windows_e2e.rs`, or merge it with `windows_panel.rs`.
3. Step 3: binstall, yes or no.
4. Step 4: which of the canon edits to authorise.
5. Confirm: no CI until the pull request into `dev`.

## Do

The owner: "Go with defaults" — the split as tabled, a separate `windows_e2e.rs`,
binstall yes, no canon edits, no CI until the pull request into `dev`.

- **Step 1, the split.** 15 move commits, `8da7be5` (draw) … `752bc8d` (panes), then
  `74c0c60`, which drops the twelve `// ── … ──` section headers left with nothing under
  them (comments only).
  - Done by script (scratch only, not in the repository):
    - a parser for `impl App`'s methods (each method with its docs and attributes, up to
      its closing `    }`);
    - a mover that moves the named methods in their order and turns `fn` into
      `pub(super) fn`;
    - a checker. With whitespace and rustfmt's added trailing commas ignored, each new
      file's methods are exactly HEAD's text, and `app.rs` is HEAD's file minus them.
  - Every commit passed fmt, the checker, clippy for Linux, windows-gnu and darwin, and
    the whole suite (616 tests).
  - Deviations:
    - `session` and `ops` clashed with `use crate::session::{self, …}` and
      `use xdevcmd_core::ops::{self, …}`, so the files are `persist.rs` and `file_ops.rs`.
    - `Graphic`, `PicKey`, `Prepared` and `prepare` stay in `app.rs`. They are types that
      `App`'s fields hold, so moving them means changing visibility; methods only.
    - The first checker compared lines and refused a signature that rustfmt rewrapped
      after `pub(super)` (`show_sync_plan`). The second compares text without whitespace,
      which is stricter about order and still blind to formatting.
  - Result:
    - `app.rs` is 1,130 lines (from 7,739); the rest is in 15 files of 165 to 880 lines.
    - 170 `impl App` methods before and after.
    - `new` and `boot` stay in `app.rs`.
- **Step 2.** `tests/winpty/mod.rs` is the ConPTY harness lifted from `windows_panel.rs`,
  plus a vt100 screen.
  - `windows_e2e.rs` checks: panes drawn, a file listed after Ctrl+R; F12 opens cmd in the
    pane's directory; `echo e2e-%OS:~0,0%done` shows `e2e-done`; F12, then F10 exits 0.
  - Compile-checked by clippy for windows-gnu (`e11467b`). Not run here.
- **Step 3.** `[package.metadata.binstall]` with a Windows zip override (`2e9f37e`).
  - `tests/packaging.rs` fills the templates in for the six release targets and compares
    the result with `release.yml`'s archive names. Breaking the Windows `pkg-fmt` failed
    it.
  - README: `cargo binstall xdevcmd` (from 0.4.0). CHANGELOG: one line.
- **Step 4, done 2026-11-11** at the owner's word ("Sửa các file canon"): ROADMAP,
  `reference/ui.md`, `reference/gotchas.md` and `reference/testing.md`, logged in
  `promotions.md`.
- **The owner's find, 2026-11-11**: on Windows, Japanese file names pushed Ext, Size and
  Modified out of line and cut the date (`d41d311`).
  - Cause: names were fitted by characters (`elide_middle`, `format!("{:<w$}")`), and a
    CJK character takes two columns.
  - Fix: `xdevcmd-core` measures with unicode-width (`elided`, `fit_start`, `fit_end`,
    `text_cells`); `crumbs` uses `elided`. `--screenshot` reads the drawn frame, because
    the backend's copy kept stale letters in wide characters' second cells.
  - Tests: unit, and a render test at three widths. Both fail without the fix.
  - The owner also asked how to see a long name whole. The status line already showed
    the cursor's full name; of the options offered (the cursor row expands, Ext folded
    into Name, a columns setting) the owner chose **the cursor row expands**. A name the
    column cuts takes the cursor's row, over Ext, Size and Modified. The test
    `the_cursor_row_shows_a_long_name_whole` fails without the change.

## Check

- [x] Each move commit: build, three clippy targets and the full suite green. A pure move
  is shown by the text checker, which is stricter than `--color-moved`.
- [x] `app.rs` is 1,130 lines; 170 `impl App` methods before and after
- [x] `windows_e2e.rs` and `windows_panel.rs` (now on the shared harness) compile for
  windows-gnu
- [ ] The owner runs on Windows: `cargo test -p xdevcmd --test windows_e2e --test windows_panel`
- [ ] binstall dry run by the owner, once 0.4.0 is on crates.io (the metadata ships with
  the crate)
- [x] fmt, clippy for three targets, 617 tests, `cargo deny`, `cargo package` (63 files in
  xdevcmd)

Not verified here: any Windows test run (no CI, by the owner's choice), and binstall
against a real release.

Left for later rounds, as B–E touch the code (tidying, which this round did not do):
- `input.rs` (880 lines) and `shell.rs` (851) are the largest files;
- `perform` and `perform_cmd` are two big dispatch functions in `input.rs`.
