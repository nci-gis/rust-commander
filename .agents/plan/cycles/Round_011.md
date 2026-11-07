# Round 011: One name — rcmd becomes xdevcmd

**Status**: In Progress
**Part of**: [ROADMAP.md](../ROADMAP.md), item 8
**Date started**: 2026-11-07 (owner: "okay, xdevcmd. đổi tên trước nhé rồi mới release")
**Date completed**:

## Goal

One name for the command, the package, the release files, the config and state
directories and the environment variables, before 0.2.0 and before any package manager
or launch. The repository keeps its name, `nci-gis/rust-commander` (owner: keep it
unless renaming is needed; it is not — nothing installs from the repository's name).

## Why this name

Checked on 2026-11-07: `rust-commander` is taken on crates.io by another TUI file
manager inspired by Midnight Commander, and at least five GitHub repositories of that
name are file commanders too; `rcmd` is taken on crates.io (a 2020 crate) and is a
popular Homebrew cask (a macOS app switcher), so `brew install rcmd` would install that.
Of the owner's candidates, `xdevcmd` was free everywhere checked — crates.io, Homebrew
formulae and casks, AUR, scoop, Debian and Ubuntu files, npm, PyPI, GitHub — and is
tied to nothing else; `twincmd` sits next to an iOS "Twin Commander" and a
`twin-commander` crate; `crackcmd` reads as software cracking; `robotcmd` as robotics.

## Plan

Owner's decisions, 2026-11-07:

- **Only `xdevcmd`**: no second `rcmd` command; whoever wants a short one makes an alias.
- **A clean break**: nothing is read from `~/.config/rcmd`, `RCMD_*` or the old state
  directory, and no migration notes — 0.1.0 had no users but the owner, who will remove
  its release.
- **`.agents` follows**: `context/`, `reference/`, `skills/` and the stubs say xdevcmd
  (explicit permission, logged in `promotions.md`); rounds, memory and logs stay as
  written, since they are history.

Steps:

- [x] Crates `rcmd-core` / `rcmd-tui` → `xdevcmd-core` / `xdevcmd-tui` (directories too),
      binary `xdevcmd`, `assets/xdevcmd.ico`.
- [x] Every `rcmd` / `RCMD` in code, tests, docs, CI, the release workflow,
      `release.toml`, `deny.toml` and scripts.
- [x] The README wordmark redrawn as strokes for `xdevcmd`, light and dark.
- [x] `tempfiles.rs`: the sweep of the pre-`tempfiles` names dropped; with a new
      prefix there is nothing old to sweep.
- [x] CHANGELOG: one entry under Changed; the 0.1.0 section left as released.
- [ ] Tests, clippy, a pull request into `dev`, CI on all three systems.

## Do

Round 010 waits only for its pull request (#7) to be merged; this round's branch,
`feat/rename-xdevcmd`, is cut from it so the rename covers its files too.

- The replacement was mechanical (`rcmd` → `xdevcmd`, `RCMD` → `XDEVCMD`) over every
  tracked text file outside the history files; `git grep -i rcmd` afterwards finds only
  those, the CHANGELOG's 0.1.0 section and its rename entry, and the roadmap's
  decision record.

## Check

- [ ] `cargo test`, `cargo clippy --all-targets -D warnings`, `cargo fmt --check`.
- [ ] CI green on Linux, macOS (universal artifact named `xdevcmd`) and Windows (the
      icon embedded in `xdevcmd.exe`).
- [ ] The release workflow's names (`xdevcmd-v0.2.0-<target>`) are only exercised by the
      tag; read, not run, until then.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- ...

**Promotions**:

- (none yet)
