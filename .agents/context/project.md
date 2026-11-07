# xdevcmd — the project

> Canonical. Read this first in any task in this repository; it is short on purpose.
> Detail lives in `reference/`, each file saying when to read it (table at the end).

A two-pane (orthodox) file manager for the terminal, in Rust, with a terminal panel
below the panes. Command `xdevcmd`; repository `nci-gis/rust-commander`; MIT, XDevTeam.
Modelled on Total Commander and Double Commander **at the level of UI and feature set**.

## Hard rules

1. **Never read Double Commander's source.** Not the Pascal, not their WCX/WDX headers,
   not their `.po` files, not their icons. It is GPL-2.0-or-later; reading and porting
   makes this a derivative work and drags the repository under the GPL. Using the running
   application, its manual, and Christian Ghisler's published plugin specs is fine —
   copyright does not cover layouts, keyboard conventions or feature lists. That is the
   whole reason this project can be MIT.
2. **No telemetry, no network calls.**
3. **A destructive operation is never silent.** Delete, overwrite and move-across-device
   confirm, report what they did, and can be cancelled.
4. **Never panic on a filesystem.** An unreadable directory, a broken link, a vanishing
   file, a non-UTF-8 name: each is a row with an error, never a crash. `unwrap()` on
   anything that touched the filesystem is a bug.
5. **Never type untrusted text into a terminal, or put it on the clipboard, without
   `safety.rs`.** A control character in a file name, typed into a shell, is a command.
   This was exploited once; see `reference/security.md`.

## Conventions that hold everywhere

- **One place for each fact.** Keys: `BINDINGS` in `keys.rs` (the key bar, F1, menus and
  `--print-config` all read it; never write a key into UI text by hand). Name,
  description, version, licence, authors: `[workspace.package]` in `Cargo.toml` (About,
  `--help`, `--version` read it; `scripts/sync-about.sh` copies it to GitHub; a test
  checks the README tagline).
- **Two walls for writes.** `App::refused` is the one place the UI decides what a mode
  allows; `xdevcmd-core/policy.rs` refuses again in the engine. A new write path goes through
  both.
- **Logic lives in `xdevcmd-core`**, which has no terminal code, so it is testable without
  one. Nothing in core depends on ratatui or crossterm.
- **Tests run isolated** from the developer: `XDEVCMD_CONFIG`, `XDG_DATA_HOME`,
  `XDEVCMD_STATE` point at scratch paths. A new test does the same.
- **A fix comes with a test that fails without it.** Several here were proved by putting
  the bug back; do the same.
- **No path a host happens to have** in tests (`/etc` is not on Windows), and no `/` in
  an assertion about a path (`std::path::MAIN_SEPARATOR`).
- `#![forbid(unsafe_code)]` in both crates. New dependencies must pass `cargo deny`.

## Branches

Work happens on **`dev`**; larger pieces on `feat/…` merged into `dev` by pull request.
**`main` is what people install**: it moves only by a pull request from `dev` with CI
green on Linux, Windows and macOS.

## Layout

```
crates/xdevcmd-core/   engine, no terminal code
  listing, sort, format   reading and ordering a directory, sizes and dates
  ops.rs            copy, move, delete, extract, pack on a worker thread; asks, never decides
  policy.rs         read-only and root, installed once; the engine's own wall
  rename.rs  batch_rename.rs  glob.rs  size.rs  find.rs  compare.rs
  archive.rs        zip / tar / tar.gz, with the hostile-archive rules
  attrs.rs  sha256.rs  image.rs  document.rs
crates/xdevcmd-tui/    binary `xdevcmd`
  app.rs            event loop, focus, key and mouse dispatch, App::refused
  keys.rs           Action, BINDINGS, key parsing
  config.rs         config.toml schema and validation
  pane/             files.rs (a listing, results, archive view), tabs.rs, term.rs, terms.rs
  layout.rs         binary split tree
  modal.rs  menu.rs  help.rs  viewer.rs  search.rs  find_dialog.rs  line_edit.rs
  safety.rs         quoting and refusal for anything typed into a shell
  session.rs  bookmarks.rs  opener.rs  usermenu.rs  usage.rs  look.rs  git.rs  theme.rs
  tests/render.rs   --screenshot --keys=…, every platform
  tests/pty.rs      the real binary in a pty, Unix
scripts/sync-about.sh
```

## Run and check

```bash
cargo run -p xdevcmd-tui -- /etc /var                # two directories
./target/release/xdevcmd --screenshot=118x34 --keys=F9 /etc /usr   # look without a terminal
cargo test                                        # ~300 tests, about 15 s
cargo clippy --all-targets -- -D warnings         # also --target x86_64-pc-windows-gnu
cargo fmt --check && cargo deny check
```

`--screenshot` draws one frame as text; use it instead of describing a layout from
memory. Mouse tests locate what they click in a first screenshot, never hard-coded.

## What matters

- **The two-pane shell is a weekend; the tail is the product**: file operations with
  proper overwrite, skip and retry, archives, multi-rename, compare.
- **Measure before optimising, then optimise.** A search of a 337 MB log went from
  1.2 s to 0.2 s by reading in chunks instead of seeking line by line.
- Directory listing on a big tree is the hot path still not profiled.

## Reference

| Read | When |
|---|---|
| [reference/testing.md](../reference/testing.md) | writing or debugging a test, proving a fix |
| [reference/ui.md](../reference/ui.md) | keys, terminal panel, tabs, sessions, panes, mouse, menu bar, usage |
| [reference/features.md](../reference/features.md) | search, filter, navigation, file operations, find, bulk rename, quick view, jobs, looks |
| [reference/archives-and-sync.md](../reference/archives-and-sync.md) | archives, compare, synchronise |
| [reference/security.md](../reference/security.md) | shells, clipboard, running programs, untrusted files, secure modes |
| [reference/platforms.md](../reference/platforms.md) | `#[cfg]`, Windows or macOS behaviour |
| [reference/config.md](../reference/config.md) | adding or changing a setting |
| [reference/gotchas.md](../reference/gotchas.md) | ratatui, crossterm, vt100 or the test harness surprises you |
