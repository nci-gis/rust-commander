# Gotchas that cost time

> **Read this when**: something in ratatui, crossterm, vt100, the pty tests or `--screenshot` behaves unexpectedly — it may have happened before.

---

## Gotchas that cost time

- **vt100 panics on a one-row screen.** When a line wraps, `grid.rs` does
  `prev_pos.row -= scrolled` with row 0 and underflows; the `unwrap()` after it panics on
  the pty reader thread. The process survives, so the symptom is a shell that silently
  stops updating. Reproduced at 80x24 by splitting the terminal top and bottom and
  running anything with a long line of output (bash's own prompt does not trigger it:
  readline scrolls horizontally on a one-row screen). Two fixes: vt100 is never given
  fewer than 2 rows (`MIN_ROWS` in `pane/term.rs`), and a terminal split that would leave
  a half too small to use is refused. `no_layout_at_any_size_panics_or_overflows` renders
  14 layouts at 6 sizes from 1x1 to 200x60 and keeps it from coming back.
- **`--screenshot` ticks once before the first frame.** Without it, a frame with no
  `--keys` never saw anything a thread delivered (git letters, sizes), because only
  input drove `tick()`; the git column looked broken when it was not.
- **`RCMD_SCREENSHOT_WAIT_MS`** (default 900) is how long `--screenshot` waits for a shell
  prompt. The sweep sets it to 0; anything that asserts on shell output must not.

- **The tab bar shares a row with the resize strip.** The strip that resizes the
  terminal panel is two rows — the file panes' bottom border and the panel's first row —
  and the panel's first row is its tab bar. Dividers were hit-tested first, so clicking
  `+` or a tab started a resize instead. Tab-bar buttons now win; the bar's empty space
  still drags the panel. Found by `the_tab_bar_buttons_are_clickable`.
- **F12 after closing the last terminal** started a new one but left focus on the file
  panes, so the next keystrokes went to the files. `toggle_term` deliberately leaves
  focus alone because startup uses it; `toggle_term_focus` now takes it.

- **`Ctrl+letter` has no case.** A legacy terminal sends Ctrl+R as a control byte, and
  crossterm reports it as lowercase `r`. `keys::normalise` folds case under Ctrl, so a
  binding written `Ctrl+R` still fires. Found by a keymap test, before any user did.
- **`--keys` splits on commas**, so `Alt+,` cannot be written there: use `Alt+comma`
  and `Alt+period`, which the config accepts too.
- **Tests that share a scratch directory race.** `policy.rs` had three tests creating
  and deleting one directory; it failed one run in six once more tests ran beside it.
  Every test gets its own directory, named after it.
- **`--keys` and `[keys]` share one parser** (`keys::parse_key`). `--screenshot` once had
  its own small parser that silently dropped `PageDown`; a help-screen test that should
  have scrolled did not, and the bug looked like it was in the help screen.

- **`Terminal::clear()` reads the cursor position.** In ratatui 0.30 it snapshots the
  cursor so it can restore it, and that query times out as *"The cursor position could
  not be read within a normal duration"* whenever the terminal is slow to answer — a
  laggy SSH link, a multiplexer, anything that is not a local xterm. It killed the app
  on the way back from `F4`. Clear through `backend_mut().clear_region(ClearType::All)`
  and reset the diff state with **two** `swap_buffers()` calls; one resets only one of
  the two buffers, and the diff against the stale frame comes out empty.
- **Do not re-run `ratatui::init()` to resume after suspending.** Same cursor query,
  same failure. Leave and re-enter the alternate screen by hand and keep the same
  `Terminal`.
- **A capped line index must record where it stopped.** `document.rs` stops indexing at
  four million lines; without an `indexed_len`, the last line's end fell through to EOF
  and the viewer drew 300 MB of the file as one row with every newline shown as `·`.

- **Never assert on a pty's raw byte stream.** ratatui only emits the cells that
  changed, so the stream arrives without the spaces between words:
  `Deletedpermanently.Thiscannotbeundone.` The first, Python, pty harness matched against
  that and needed whitespace-squashing hacks. `tests/pty.rs` feeds the bytes through
  vt100 — the parser the shell pane already uses — and asserts on the reconstructed
  screen, where `"This cannot be undone."` simply matches.
- **Clear marks when an operation finishes.** Leaving them set means the next `F8`
  silently includes the files selected for the last `F5`. Found by an end-to-end test
  that deleted more than it selected; `finish_job` clears them now.
- **ConPTY waits for an answer before the shell starts.** `portable-pty` creates the
  Windows pseudoconsole with `PSEUDOCONSOLE_INHERIT_CURSOR`, which makes it ask the
  host terminal where the cursor is (`ESC[6n`) and block until told. vt100 is a parser
  and never answers, so on Windows the shell pane drew nothing and took no input,
  whatever the shell. The output thread in `pane/term.rs` now answers every cursor
  query from the vt100 screen (`cursor_queries`, which also catches a query split
  across two reads). Found by the first person to run rcmd on Windows; CI could not, as
  the pty tests are Unix-only. `the_terminal_answers_where_the_cursor_is` fails without
  the answer.
- **`std::fs::canonicalize` on Windows returns `\\?\D:\…`.** It showed in pane titles
  after going up, and would have reached Alt+C (`cmd.exe` cannot `cd` into it), copy
  path, cd-on-exit, bookmarks and the session. Use `rcmd_core::paths::canonical`
  everywhere a canonical path reaches the user or the policy: it drops the prefix when
  the plain form means the same file, and keeps both sides of the root check in one
  form. Found on the first Windows run.
- **An identical frame is not a silent one.** ratatui's crossterm backend writes a
  colour reset after every draw, empty diff or not, and the terminal hides the cursor
  again every frame: about 600 bytes a second with rcmd idle. `quiet.rs` wraps the
  backend and drops what changes nothing; call `forget()` after anything else has used
  the terminal (an editor), or a needed cursor move is skipped.
- **A file pane scrolls back to its cursor on every frame** (`scroll_into_view` in
  `render`). Changing only `offset` — the wheel, the scrollbar — was undone by the next
  frame; `set_offset` brings the cursor along instead.
- **`Ctrl+\` arrives as `Ctrl+4`** from a legacy terminal (byte 0x1C); `normalise` maps
  it back, as it folds `Ctrl+letter` case.
- **`vt100::Parser` has no `set_size`** — it lives on the screen:
  `parser.screen_mut().set_size(rows, cols)`.

- **Never add `crossterm` or `vt100` as direct dependencies.** `ratatui` pins its own
  crossterm (via `ratatui-crossterm`) and `tui-term` pins its own vt100. Adding either
  directly resolves a second copy, and you get two `Event` enums with identical names in
  an error message that does not mention versions. Use `ratatui::crossterm`, and take the
  vt100 version from `cargo tree -i vt100`. Adding `unicode-width` directly broke
  resolution outright, because ratatui pins it exactly.
- **Enabling mouse capture is on us.** `ratatui::init()` does not do it, and if the
  process dies without `DisableMouseCapture` the user's shell prints escape codes on
  every click. `main.rs` chains a panic hook for exactly this.
