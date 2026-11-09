# Round 017: Getting 0.3.0 ready to release

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), items 8 and 28
**Date started**: 2026-11-10
**Date completed**: —

## Goal

Everything 0.3.0 needs except pressing the button. The package is renamed `xdevcmd`
(owner, 2026-11-09), both crates publishable so `cargo package --workspace` passes, Scoop
and winget manifests ready to fill, README says how to install. **Not done here, by the
owner's word: publishing, tagging, changing the version.** The owner runs `cargo release minor`.

## Plan

Branch `feat/release-0.3.0`, from `feat/shell-bridge` at Round 016's Review commit.

**Decision: move the directory too**, `crates/xdevcmd-tui` → `crates/xdevcmd` with `git mv`.
Mentions to update: Cargo.toml members, release.yml (`-p`, the jq `select`), release.toml,
deny.toml, scripts/sync-about.sh, docs/development.md, README. Binary name and
`CARGO_BIN_EXE_xdevcmd` unchanged. Mentions in `.agents/context/project.md` and
`.agents/reference/{testing,ui}.md` are read-only for agents; **the owner authorised editing
those name/path mentions only** (2026-11-10), each logged in `plan/promotions.md`.

- [x] **1. Rename the package and its directory.** `name = "xdevcmd"`; `[[bin]]` stays `xdevcmd`.
      Every mention, Cargo.lock regenerated. Test: suite; `cargo run -p xdevcmd -- --version`.
- [x] **2. Files the build reads live inside the crate.** `assets/about-logo.png` →
      `crates/xdevcmd/assets/about-logo.png` (`include_bytes!("../assets/about-logo.png")`);
      `assets/xdevcmd.ico` → `crates/xdevcmd/assets/xdevcmd.ico` (build.rs). SVGs stay in top-level
      `assets/`. scripts/icons.sh and docs/development.md point at new places. Also any test that
      reads `../../themes` etc. outside the crate must not break `cargo package` verification
      (tests are not built by package verify, but check). Test: build Linux and
      `--target x86_64-pc-windows-gnu` if available; `cargo package --list -p xdevcmd` has both.
- [x] **3. Manifests ready for crates.io.** Both crates: `publish = true`, `readme`, `homepage`,
      `categories = ["command-line-utilities", "filesystem"]`, keywords
      `["file-manager", "tui", "terminal", "commander", "two-pane"]`. `xdevcmd` uses
      `readme = "../../README.md"`. `xdevcmd-core` gets a short README (engine of xdevcmd, no
      terminal code, **no stable API**, use `xdevcmd`). Dependency `xdevcmd-core = { path =
      "../xdevcmd-core", version = "=0.2.1" }`. A `LICENSE` copy in each crate + a test (skipped
      where the top-level file is absent) that both equal the top-level one. sync-about.sh reads
      the `xdevcmd` package and adds `rust` as a GitHub topic of its own.
- [x] **4. cargo-release and cargo-deny.** release.toml `publish = true`. Core's
      `[package.metadata.release]` `release = false` → `tag = false` (one tag `v{{version}}` from
      `xdevcmd`). Dry run `cargo release minor` (no `--execute`) if cargo-release is installable;
      record output. deny.toml: `allow-wildcard-paths = false`; rewrite the `[licenses.private]`
      comment; `cargo deny check` passes (if installable).
- [x] **5. `cargo package --workspace` passes, and stays passing.** Run locally; check `--list`
      for things that should not ship; `exclude` as needed. Add the command to ci.yml on Linux.
- [x] **6. Scoop and winget templates.** `packaging/scoop/xdevcmd.json`: URL
      `https://github.com/nci-gis/rust-commander/releases/download/v$version/xdevcmd-v$version-x86_64-pc-windows-msvc.zip`
      (match release.yml's names exactly), `extract_dir`, `bin` `xdevcmd.exe`, `checkver` from
      GitHub releases, `autoupdate` hash from `SHA256SUMS`. `packaging/winget/` three manifests
      (version, defaultLocale, installer) for `XDevTeam.xdevcmd`: `zip`,
      `NestedInstallerType: portable`, `NestedInstallerFiles` → `xdevcmd-v<ver>-x86_64-pc-windows-msvc\xdevcmd.exe`,
      `PortableCommandAlias: xdevcmd`; version/URL/hash placeholders. `packaging/README.md`: fill
      from `SHA256SUMS`, `wingetcreate new` first, then `wingetcreate update … --submit`; bucket
      `nci-gis/scoop-xdevcmd` created by the owner. Test: JSON parses and URL pattern matches
      release.yml's name line.
- [x] **7. README and CHANGELOG.** Install: `cargo install xdevcmd --locked` first, then Scoop,
      winget, downloads; git form `cargo install --git … xdevcmd --locked`. README tagline render
      test still passes. CHANGELOG Unreleased/Changed. docs/development.md: `-p xdevcmd`, release
      steps include crates.io publishing by the owner.
- [x] **8. Check and hand-over.** fmt, clippy, `cargo test --workspace`, `cargo deny check`,
      `cargo package --workspace`. Status → Review. Owner: CI on three systems; PR into dev,
      `cargo release minor`, PR into main, tag, draft; bucket repo and winget submission.

**Risks**: README relative links on crates.io; README names Scoop/winget before they exist
(owner trims at release); name availability rechecked by the owner before publishing.

**Not doing**: publishing, tagging, version change; creating the bucket or submitting to
winget; binstall/Homebrew/AUR; signing; workflows holding tokens; raising `rust-version`.

## Do

- 2026-11-10, round opened on `feat/release-0.3.0` from `feat/shell-bridge` at Round 016's
  Review commit (`74b3441`). Owner decisions binding: package `xdevcmd`, directory moved;
  the owner authorised editing the crate name/path mentions in `context/project.md` and
  `reference/{testing,ui}.md` (done in step 1, logged in `plan/promotions.md`; no other
  `.agents/` canon named the crate; rounds and memory keep the old name, as history).
- **crates.io names**, `GET https://crates.io/api/v1/crates/<name>` through the proxy,
  2026-11-10: `xdevcmd` 404, `xdevcmd-core` 404, `xdevcmd-tui` 404 (free); control
  `ratatui` 200. Recheck before publishing.
- 1 (`ca48a81`): `git mv crates/xdevcmd-tui crates/xdevcmd`; `name = "xdevcmd"`, bin
  unchanged. Mentions: Cargo.toml members, release.yml (jq `select`, `-p`), release.toml,
  core's release comment, deny.toml, sync-about.sh, docs/development.md, README, the three
  canon files. Cargo.lock regenerated (rename only). `cargo run -p xdevcmd -- --version`
  → `xdevcmd 0.2.1 · MIT · XDevTeam`; suite green.
- 2 (`ca1de25`): `about-logo.png` and `xdevcmd.ico` → `crates/xdevcmd/assets/`
  (`include_bytes!("../assets/about-logo.png")`, build.rs `assets/xdevcmd.ico`); SVGs
  stay; scripts/icons.sh writes to the new place; docs/development.md says why.
  `cargo package --list -p xdevcmd` has both. **Not verified**: the Windows build with
  the icon — `rustup target add x86_64-pc-windows-gnu` fails here (static.rust-lang.org
  unreachable: "unsuccessful tunnel"), so no Windows check or clippy from this machine;
  CI's Windows job (`XDEVCMD_REQUIRE_ICON=1`) is the check. Test-only reads outside the
  crate (`render.rs` README, `theme.rs` `../../themes`) do not affect `cargo package`.
- 3 (`2ceb4e3`): `homepage`, `categories` and the five crates.io keywords (`two-pane`
  replaces `rust`) in `[workspace.package]`, inherited by both; `readme` per crate;
  `publish = true`; `xdevcmd-core = { path, version = "=0.2.1" }`. `crates/xdevcmd-core/README.md`
  (engine, no terminal code, no stable API, install `xdevcmd`). LICENSE copied into each
  crate; `tests/packaging.rs` checks both equal the top-level one (skipped without it);
  proved by appending a line to core's copy → fails. sync-about.sh reads `xdevcmd` and
  adds `rust` as a topic of its own.
- 4 (`b0353a2`): release.toml `publish = true`; core `release = false` → `tag = false`.
  deny.toml `allow-wildcard-paths = false`, `[licenses.private]` comment rewritten.
  `cargo install cargo-release cargo-deny --locked` worked (cargo-release 1.1.7,
  cargo-deny 0.20.2). **`cargo deny check`: advisories ok, bans ok, licenses ok, sources
  ok** (duplicate-version warnings as before; a warning that `BSD-3-Clause` is allowed but
  unused). Wildcard refusal proved: the pin removed → `error[wildcard]`, bans FAILED.
  **The full `cargo release minor` dry run could not run here**: cargo-release fetches the
  crates.io index with its own HTTP client (ureq, bundled roots, no proxy), and this
  sandbox intercepts the direct connection → "invalid peer certificate: UnknownIssuer";
  `SSL_CERT_FILE`/proxy variables do not help, `--no-publish` still fetches. The step-wise
  dry runs that need no network: `cargo release version minor` → workspace 0.3.0, "Updating
  xdevcmd's dependency from =0.2.1 to =0.3.0"; `cargo release tag` → only `xdevcmd` is
  considered (core has no tag). Both also warn that `feat/…` is not an allowed branch, as
  expected. The owner's dry run on `dev` is the real check.
- 5 (`689dea0`): `cargo package --workspace` passes. `--list`: core ships src, tests,
  LICENSE, README; `xdevcmd` shipped `tests/` (260 KB; `render.rs` cannot build outside the
  repository, it includes `../../README.md`) → `exclude = ["tests/"]`; cargo then warns
  it ignores those three tests, which is the intent. Now 33 files / 96 KB and 47 files /
  269 KB compressed. ci.yml: `cargo package --workspace --locked` after the tests, on
  `ubuntu-latest` only (`--locked` checked locally; passes on a clean tree without
  `--allow-dirty`). YAML parses; the other jobs are untouched.
- 6 (`b6cec46`): `packaging/scoop/xdevcmd.json` (version 0.3.0 filled, `hash` a
  placeholder, `checkver` github, `autoupdate` URL/`extract_dir`/hash from `SHA256SUMS`
  by `$basename`), `packaging/winget/` three 1.9.0 manifests for `XDevTeam.xdevcmd` (zip,
  portable, `NestedInstallerFiles`, alias `xdevcmd`, `<VERSION>`/`<SHA256>`),
  `packaging/README.md`. Two tests read release.yml's `name="xdevcmd-${GITHUB_REF_NAME}-…"`
  line, check the msvc target is built and zipped as `$name.zip`, and require the Scoop
  URLs/extract_dir and the winget URL/nested path to match; serde_json added as a
  dev-dependency to parse the JSON. Proved: renaming the archive in release.yml → both
  fail. **Not verified**: `scoop install` and `winget validate` (no Windows here).
- 7 (`e0933e0`): README install: crates.io first ("from 0.3.0"), Scoop and winget marked
  "coming with 0.3.0; not available yet" for the owner to trim at release, downloads,
  then the git form with `xdevcmd`. Tagline test passes. CHANGELOG Unreleased/Changed.
  docs/development.md: **every `cargo release` before publishing now says
  `--no-publish`** — with `publish = true`, the documented step-1 command on `dev` would
  otherwise have published from `dev`; publishing added as step 5 (`cargo release publish`,
  owner, from `main` at the tag), Scoop/winget as step 6.
- 8: see Check.
- **CI on Windows and macOS, 2026-11-10** (known red from run 42712465398 on
  `feat/shell-bridge`). Iterated on a scratch branch `ci/scratch` whose only change was
  `cargo test --all --no-fail-fast` (so one failing test binary hid none behind it),
  ci.yml on this branch is unchanged. **The remote `ci/scratch` is still there**: this
  sandbox's proxy refuses deleting a branch (git push --delete and the API both 403);
  the owner deletes it (`git push origin --delete ci/scratch`).
  - **Windows, journal** (`a_dead_runs_parts_are_removed_and_renames_put_back`,
    `a_journal_held_by_a_running_job_is_left_alone_whatever_pid_it_names`). Cause: the
    probe opened the journal for writing with `share_mode(0)`, and Round 014 rightly holds
    it until after `remove_file`; `remove_file` must itself open the file for DELETE, which
    a handle sharing nothing refuses — so a dead run's journal was never removed (the
    sharing rules are per handle, the same within one process as across processes). Fix
    (`2bbb6f0`): the probe shares `FILE_SHARE_DELETE` only (0x4). It still asks for
    write, so it is refused while the job holds the journal (`FILE_SHARE_READ |
    FILE_SHARE_DELETE`, no write sharing) and while another start's probe holds it (no
    write sharing either): two starts still offer a job once. Removed while held, the name
    is gone (POSIX delete) or delete-pending (no open is granted), so nobody gets it in
    between. `FILE_FLAG_DELETE_ON_CLOSE` on the held journal was rejected: the system
    would delete a *killed* run's journal too, which is exactly the one that must survive.
    No `unsafe`, no `File::lock`.
  - **macOS, `a_terminal_is_gone_only_once_its_other_side_closes`** ("its other side
    closed"). Cause: on macOS `tcgetattr` on a pty slave whose master closed still
    succeeds. Fix (`2bbb6f0`): `tty_gone` = `tcgetattr` EIO/ENXIO **or** a zero-timeout
    `rustix::event::poll` answering **POLLHUP**. Only POLLHUP counts: POLLNVAL (macOS's
    answer for devices its poll does not support, e.g. `/dev/tty`, live or not) is never
    taken for a hang-up; there the next draw's failed write still ends xdevcmd. Linux
    behaviour unchanged (tcgetattr already said EIO). The test is unchanged and passes on
    macOS, Linux x86_64 and ARM.
  - **macOS, pty**: `alt_enter_types_the_marked_names_quoted_without_running_them`,
    `alt_enter_types_nothing_into_a_running_program`,
    `typed_names_go_in_bracketed_paste_when_the_shell_asks_for_it` (found once
    `--no-fail-fast` ran past the first binary, run 42718674366). Cause: a **product bug**,
    not a runner quirk. The shell's directory is the system's (links resolved,
    `/private/var/folders/…`), the pane's keeps the user's links (`/var/folders/…`; on
    macOS `/var` → `/private/var`); `name_words` compared the text, so it typed full
    paths at a shell already in that directory. Fix (`e672c90`): `all_in` compares the
    paths' one parent with the shell's directory, then their canonical forms (one
    canonicalize per side, not per name). Reproduced on Linux with `TMPDIR` pointing at a
    symlink (both alt_enter tests fail without the fix, pass with it); unit test
    `typed_names_are_bare_in_the_same_directory_by_another_name`. Untrusted (OSC-reported)
    directories still get full paths.
  - **Windows, `the_winget_installer_points_at_the_archive_release_yml_makes`**: the test
    was wrong for Windows — git on the runner checks out with CRLF (`core.autocrlf`), and
    it looked for `…\n`. Fix (`e672c90`): compare with CRLF made LF (winget reads either).
  - **Windows, `save_as_refuses_a_name_with_a_control_character`**: its fixture
    `bad\u{15}x.txt` cannot be created on Windows (error 123, invalid name), so the
    scenario cannot arise there: `#[cfg(unix)]`, with the reason in a comment. The refusal
    code itself is untouched.
  - Runs: 42718674366 (scratch, after `2bbb6f0`: journal and tty fixed; the five above
    found), 42719848686 (scratch, after `e672c90`: **all green**, ubuntu x86_64 + ARM,
    windows, macos, cross-check, cargo-deny), 42721141031 (this branch at `e672c90`, real CI flags: **all green**).

## Check

- [x] `cargo package --workspace` passes locally (clean tree, `--locked`). CI's new step:
      not run yet (feat branches run CI only on request or by PR).
- [ ] The `cargo release minor` dry run: **not run whole** (network, see Do 4). Step-wise:
      pin rewritten `=0.2.1` → `=0.3.0`, one tag from `xdevcmd`. Core-first order not seen;
      by cargo-release's dependency order it should be. Owner's dry run on `dev`.
- [x] `cargo deny check` passes with wildcard paths refused (proved by removing the pin).
- [x] LICENSE copies and Scoop (and winget) URL pattern each checked by a test, each shown
      to fail when broken.
- [x] `cargo fmt --all --check`; `cargo clippy --all-targets --all-features -- -D warnings`
      (Linux only; Windows target not installable here); `cargo test --workspace` twice:
      595 passed, 2 ignored, 0 failed each time.
- [ ] By the owner: CI on three systems; README on crates.io; Scoop install; winget submission.

Set aside for the owner's hand tests; publishing is the owner's.

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : the new crate layout in `project.md` (owner's edit)
