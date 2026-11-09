# Round 017: Getting 0.3.0 ready to release

**Status**: In Progress
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

- [ ] **1. Rename the package and its directory.** `name = "xdevcmd"`; `[[bin]]` stays `xdevcmd`.
      Every mention, Cargo.lock regenerated. Test: suite; `cargo run -p xdevcmd -- --version`.
- [ ] **2. Files the build reads live inside the crate.** `assets/about-logo.png` →
      `crates/xdevcmd/assets/about-logo.png` (`include_bytes!("../assets/about-logo.png")`);
      `assets/xdevcmd.ico` → `crates/xdevcmd/assets/xdevcmd.ico` (build.rs). SVGs stay in top-level
      `assets/`. scripts/icons.sh and docs/development.md point at new places. Also any test that
      reads `../../themes` etc. outside the crate must not break `cargo package` verification
      (tests are not built by package verify, but check). Test: build Linux and
      `--target x86_64-pc-windows-gnu` if available; `cargo package --list -p xdevcmd` has both.
- [ ] **3. Manifests ready for crates.io.** Both crates: `publish = true`, `readme`, `homepage`,
      `categories = ["command-line-utilities", "filesystem"]`, keywords
      `["file-manager", "tui", "terminal", "commander", "two-pane"]`. `xdevcmd` uses
      `readme = "../../README.md"`. `xdevcmd-core` gets a short README (engine of xdevcmd, no
      terminal code, **no stable API**, use `xdevcmd`). Dependency `xdevcmd-core = { path =
      "../xdevcmd-core", version = "=0.2.1" }`. A `LICENSE` copy in each crate + a test (skipped
      where the top-level file is absent) that both equal the top-level one. sync-about.sh reads
      the `xdevcmd` package and adds `rust` as a GitHub topic of its own.
- [ ] **4. cargo-release and cargo-deny.** release.toml `publish = true`. Core's
      `[package.metadata.release]` `release = false` → `tag = false` (one tag `v{{version}}` from
      `xdevcmd`). Dry run `cargo release minor` (no `--execute`) if cargo-release is installable;
      record output. deny.toml: `allow-wildcard-paths = false`; rewrite the `[licenses.private]`
      comment; `cargo deny check` passes (if installable).
- [ ] **5. `cargo package --workspace` passes, and stays passing.** Run locally; check `--list`
      for things that should not ship; `exclude` as needed. Add the command to ci.yml on Linux.
- [ ] **6. Scoop and winget templates.** `packaging/scoop/xdevcmd.json`: URL
      `https://github.com/nci-gis/rust-commander/releases/download/v$version/xdevcmd-v$version-x86_64-pc-windows-msvc.zip`
      (match release.yml's names exactly), `extract_dir`, `bin` `xdevcmd.exe`, `checkver` from
      GitHub releases, `autoupdate` hash from `SHA256SUMS`. `packaging/winget/` three manifests
      (version, defaultLocale, installer) for `XDevTeam.xdevcmd`: `zip`,
      `NestedInstallerType: portable`, `NestedInstallerFiles` → `xdevcmd-v<ver>-x86_64-pc-windows-msvc\xdevcmd.exe`,
      `PortableCommandAlias: xdevcmd`; version/URL/hash placeholders. `packaging/README.md`: fill
      from `SHA256SUMS`, `wingetcreate new` first, then `wingetcreate update … --submit`; bucket
      `nci-gis/scoop-xdevcmd` created by the owner. Test: JSON parses and URL pattern matches
      release.yml's name line.
- [ ] **7. README and CHANGELOG.** Install: `cargo install xdevcmd --locked` first, then Scoop,
      winget, downloads; git form `cargo install --git … xdevcmd --locked`. README tagline render
      test still passes. CHANGELOG Unreleased/Changed. docs/development.md: `-p xdevcmd`, release
      steps include crates.io publishing by the owner.
- [ ] **8. Check and hand-over.** fmt, clippy, `cargo test --workspace`, `cargo deny check`,
      `cargo package --workspace`. Status → Review. Owner: CI on three systems; PR into dev,
      `cargo release minor`, PR into main, tag, draft; bucket repo and winget submission.

**Risks**: README relative links on crates.io; README names Scoop/winget before they exist
(owner trims at release); name availability rechecked by the owner before publishing.

**Not doing**: publishing, tagging, version change; creating the bucket or submitting to
winget; binstall/Homebrew/AUR; signing; workflows holding tokens; raising `rust-version`.

## Do

—

## Check

- [ ] `cargo package --workspace` passes locally and in CI's new step.
- [ ] The `cargo release minor` dry run (if run) shows core first, one tag, pin rewritten.
- [ ] `cargo deny check` passes with wildcard paths refused.
- [ ] LICENSE copies and Scoop URL pattern each checked by a test.
- [ ] By the owner: CI on three systems; README on crates.io; Scoop install; winget submission.

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : the new crate layout in `project.md` (owner's edit)
