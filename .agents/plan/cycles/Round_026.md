# Round 026: Documentation from the app (v0.4.0, step 3, item 40)

**Status**: In Progress
**Part of**: v0.4.0, step 3 of `ROADMAP.md` ("Next"); item 40 (the owner's go,
2026-11-12: "ok")
**Date started**: 2026-11-12
**Date completed**: YYYY-MM-DD

## Goal

From inside xdevcmd, reach the documentation **for the version installed**: Help →
Documentation (and a row in F1) names the local copy that ships in the release archive,
then the link to `docs/` at that version's tag. `docs/README.md` gives the reading
order. A machine without a browser or a network still has the docs.

## Plan (kickoff review)

### 1. Where it starts

- ROADMAP, note on 40: `https://github.com/nci-gis/rust-commander/tree/v{version}/docs`
  from `CARGO_PKG_VERSION`, not `main` (main describes features the installed version
  may not have); `docs/README.md` the reading order; `docs/` in the release archives,
  named first; a docs site later only changes the link.
- Round 024, decision 5: Documentation was left out of the Help menu until it has a
  link that works. The Help menu today: Commands… (F1) · Config in use · Usage
  (Shift+F1) · ─ · About. F1's menu-only rows are `commands::Extra` (Settings, Theme,
  Config in use, About), listed at the end of the General group.
- **Links in the code today**: `commands.rs` `TROUBLESHOOTING` =
  `{repository}/blob/main/docs/troubleshooting.md` (F1's "More" row, read-only) — the
  one that should follow the version. `wrap.rs` has the same URL only as test data (a
  long word to wrap); `crash.rs` `ISSUES` is the issue tracker, not a doc, and stays.
  `cli_docs.rs` (the man page) names the repository. The README's links go to `main`:
  it is GitHub's front page, read there, and is out of this round.
- **About** shows the repository and the releases page as plain text: no OSC 8. The
  clipboard is OSC 52 through `App::copy_text` (refused in restricted mode, and refused
  for text that could act when pasted, `safety.rs`). Opening with the system's program
  is `Cmd::OpenFile` → `launch` (files only, confirmed, refused in restricted mode).
- **`themes/` beside the binary**: `config::theme_dirs` looks in
  `current_exe().parent()/themes`; release.yml copies `themes/` into each archive.
  The archive today: `LICENSE README.md THIRD-PARTY.md themes/ xdevcmd[.exe]`.
- Packaging tests (`tests/packaging.rs`) read release.yml's `package` step for the
  archive name; Scoop, winget and binstall templates are tied to that name.

### 2. Critique: what must be got right

- **The version → the tag.** Tags are `v{version}` (v0.2.0, v0.2.1, v0.3.0;
  `cargo release`'s default). A pre-release `0.4.0-rc.1` → `v0.4.0-rc.1`, as written.
  Build metadata (`0.4.0+abc`) is never in a tag: dropped. **A dev build** carries the
  version in `Cargo.toml`, which `cargo release` bumps only when releasing: the branch
  today says 0.3.0, so a build of `feat/v0.4.0` points at **v0.3.0's** docs, its last
  released tag, which can lack what the build has. Said in the docs, not guessed at in
  the app (no git at run time).
- **A page vs the directory.** `tree/v…/docs` for the directory; a page is
  `blob/v…/docs/<page>` (GitHub's own form for a file). One function,
  `docs_url(page: Option<&str>)`; F1's "More" row is a `&'static str` in a static table,
  so it gets the same text from a `concat!` macro, and a test holds the two equal.
- **No network (hard rule 2).** xdevcmd shows the link; it does not open it. "Open in
  browser" through `launch` would hand a URL to the system's opener, which then fetches
  it: a connection xdevcmd caused, from a program whose promise is that it makes none,
  and `launch` is for files (and asks first). Not done. The link is copied with OSC 52
  on a key, which the user then pastes where they choose.
- **OSC 8 hyperlinks: not used.** About does not use them; ratatui writes a buffer of
  cells, and an escape inside a cell breaks its width accounting and the screenshot
  tests. Terminals that make links clickable (Windows Terminal, iTerm2, kitty, GNOME
  Terminal) detect a plain `https://` URL by themselves, so the plain text already
  clicks where it can.
- **The local copy.** `docs/` beside the binary, found as `themes/` is (from
  `current_exe`, its directory, and the resolved path's when a symlink points there,
  as Homebrew's would). Named first when `docs/README.md` is there; Enter reads it in
  xdevcmd's own viewer (read-only, through the same root check as F3). Absent (cargo
  install, a build from source): the window says the archives carry one.
- **Modes.** Read-only: nothing here writes. Restricted: the window shows both; copying
  is refused by `copy_text` as everywhere ("restricted mode: no clipboard"); viewing a
  copy outside the root is refused as F3 is. Nothing new to refuse.
- **The archive.** `docs/*.md` beside the binary and `themes/`; names unchanged (the
  packaging tests and the Scoop/winget/binstall files are tied to them). About 60 KB.

### 3. Steps

1. This kickoff; ROADMAP: step 3 and item 40 → Round 026. **[skip ci]**.
2. `docs/README.md`: the index in reading order (Guide → Keys → Shell → Config →
   Security → Troubleshooting → Development), one line each, and "for the version you
   have installed, use Help → Documentation".
3. The app: `docs.rs` (`docs_url`, the local copy); Help → Documentation after Usage,
   before About; an F1 row (`Extra::Documentation`, General); the window (local path,
   then the link; Enter reads the local README, C copies the link). F1's "More" row
   versioned.
4. release.yml copies `docs/*.md` into each archive; `docs/development.md` lists it.
5. Tests: unit (normal, pre-release, build metadata, page, the macro equal to the
   function, the local copy found or not); render (the Help menu's order, the window
   without a local copy, the F1 row, a local copy named first with a binary beside a
   `docs/`); packaging (release.yml copies `docs/` into the archive folder).
6. CHANGELOG (Unreleased), `docs/guide.md`.

### 4. Decisions (recommendations; the coder follows them unless the owner objects)

1. **Tag from `CARGO_PKG_VERSION`**, `v` + the version without build metadata; a dev
   build points at its last released tag (documented).
2. **No "open in browser"**, no OSC 8: show and copy (OSC 52) only.
3. **The local copy first**, Enter reads its `README.md` in the viewer; C copies the
   link. Without a local copy, Enter copies the link.
4. **F1's "More" row** follows the version (troubleshooting.md at the tag);
   `crash.rs`'s issues link stays (not a doc).
5. **The archive** gets `docs/` (Markdown only, flat); names unchanged.

### Not doing (this round)

- A docs site (ROADMAP, Later); rewriting the README's links to a tag.
- Opening a URL, ever (hard rule 2).

## Do

## Check

- [ ] `docs_url`: normal, pre-release, build metadata, a page; the F1 row equal
- [ ] Help menu order; the window with and without a local copy; the F1 row
- [ ] release.yml copies `docs/` into every archive; packaging tests pass
- [ ] fmt, clippy `-D warnings`, `cargo test --workspace`; CI on three systems

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → reference/ui.md : Help → Documentation (the Help menu's items)
