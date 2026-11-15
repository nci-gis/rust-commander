<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/nci-gis/rust-commander/main/assets/logo-dark.svg">
    <img alt="xdevcmd" src="https://raw.githubusercontent.com/nci-gis/rust-commander/main/assets/logo-light.svg" width="498">
  </picture>
</p>

<p align="center">xDev Commander: a cross-platform, two-pane terminal file manager that extends your shell.</p>

<p align="center"><sub>For developers and devops who live in the terminal.</sub></p>

```
  File   View   Help
┌ /usr/share ────────────────────────────────────┐┌ /usr/include ──────────────────────────────────┐
│ Name ▼                          Ext        Size││ Name ▼                          Ext        Size│
│ ..                                        <DIR>││ ..                                        <DIR>│
│ accountsservice                           <DIR>││ arpa                                      <DIR>│
│ aclocal                                   <DIR>││ asm-generic                               <DIR>│
│ alsa                                      <DIR>││ c++                                       <DIR>│
│ alsa-base                                 <DIR>││ crash                                     <DIR>│
│ alsa-card-profile                         <DIR>││ drm                                       <DIR>│
│ application-registry                      <DIR>││ finclude                                  <DIR>│
│ applications                              <DIR>││ gdb                                       <DIR>│
│ apport                                    <DIR>││ iproute2                                  <DIR>│
│ appstream                                 <DIR>││ linux                                     <DIR>│
│ aspell                                    <DIR>││ misc                                      <DIR>│
│ avahi                                     <DIR>││ mtd                                       <DIR>│
│ backgrounds                               <DIR>││ net                                       <DIR>│
│ base-files                                <DIR>││ netash                                    <DIR>│
│ base-passwd                               <DIR>││ netatalk                                  <DIR>│
└────────────────────────────────────────────────┘└────────────────────────────────────────────────┘
 ⌂ 1 share ×  +
┌ ⌂ sh · share ────────────────────────────────────────────────────────────────────────────────────┐
│$ █                                                                                               │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
 ..  ·  266 items  ·  sort name ▼
 1 Help  2 Ren  3 View  4 Edit  5 Copy  6 Move  7 MkDir  8 Del  9 Menu  10 Quit
```

Two file panes, a terminal underneath, and the familiar keys of two-pane file managers:
F5 copy, F6 move, F8 delete. Built for Linux, macOS and Windows; [Platforms](#platforms)
says how far each is tested.

## Install

With Rust 1.90 or newer, from [crates.io](https://crates.io/crates/xdevcmd) (from 0.3.0):

```bash
cargo install xdevcmd --locked
```

Or the ready-made build, with no compiling, through
[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) (from 0.4.0):

```bash
cargo binstall xdevcmd
```

Or download a ready-made build for Linux (x86_64 and ARM64), macOS or Windows from
[Releases](https://github.com/nci-gis/rust-commander/releases): one program, nothing to
install. They are not signed, so macOS and Windows warn the first time: see
[troubleshooting](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md#macos).

The newest code, from the repository:

```bash
cargo install --git https://github.com/nci-gis/rust-commander xdevcmd --locked
```

Each way installs the `xdevcmd` command. Each release's archives can be checked:
[security.md, "Checking a download"](https://github.com/nci-gis/rust-commander/blob/main/docs/security.md#checking-a-download).

**Planned, not available yet:** a Scoop bucket for Windows.

## Quick start

```bash
xdevcmd                  # open in the current directory
xdevcmd ~/src /tmp       # or with two directories
```

| Key | Does |
|---|---|
| **Tab** | switch pane |
| **Enter** / **Backspace** | open / go up |
| **F5** / **F6** | copy / move to the other pane |
| **F8** | delete (to the trash) |
| **F3** / **F4** | view / edit |
| **/** | jump to a name |
| **F12** | the terminal |
| **F9** | menu · **F1** all keys · **F10** quit |

## Features

- Copy, move, delete, rename with progress, cancel and a question before any overwrite
- A real shell under the panes, with tabs and splits
- Tabs, splits and zoom for the file panes; your layout is reopened next time
- Quick search, filter, bookmarks, history, go to path
- Find files by name and content; rename many files with a pattern or in your editor
- Zip and tar archives open like folders
- Compare two folders, or make them match
- Preview text and pictures; checksums; permissions
- Mouse: click, drag and drop, right-click menu
- Read-only and restricted modes; no network, no telemetry

The [guide](https://github.com/nci-gis/rust-commander/blob/main/docs/guide.md) explains each one.

## Platforms

What has been checked, and how. "CI" is the whole test suite on GitHub's runners at
every pull request; "by hand" is the maintainer using it with
[the hand-test list](https://github.com/nci-gis/rust-commander/blob/main/docs/testing-by-hand.md).

| | Linux | macOS | Windows |
|---|---|---|---|
| Builds (x86_64; ARM64 too on Linux and macOS) | ✓ | ✓ | ✓ (x86_64) |
| Whole test suite in CI | ✓ (x86_64, ARM64) | ✓ | ✓ |
| Tests in a real pseudo-terminal | ✓ | ✓ | ConPTY: start-up only |
| Used by hand by the maintainer | ✓ | not yet | ✓ (Windows 11) |
| Shell reports (directory, prompt marks) | bash tested; zsh, fish not run | zsh not run | PowerShell, cmd not run; whether ConPTY passes them on is not checked |

Known gaps are in [troubleshooting](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md).

## Configure

```bash
mkdir -p ~/.config/xdevcmd
xdevcmd --print-config > ~/.config/xdevcmd/config.toml   # all settings, with defaults
```

Keep only the lines you change. See [docs/config.md](https://github.com/nci-gis/rust-commander/blob/main/docs/config.md).

## Docs

- [Guide](https://github.com/nci-gis/rust-commander/blob/main/docs/guide.md): what xdevcmd does, by topic
- [Keys](https://github.com/nci-gis/rust-commander/blob/main/docs/keys.md): every key
- [Config](https://github.com/nci-gis/rust-commander/blob/main/docs/config.md): settings, bookmarks, your own commands
- [Shell](https://github.com/nci-gis/rust-commander/blob/main/docs/shell.md): stay in the directory you were in when you quit
- [Security](https://github.com/nci-gis/rust-commander/blob/main/docs/security.md): what xdevcmd protects against, and safe modes
- [Compatibility](https://github.com/nci-gis/rust-commander/blob/main/docs/compatibility.md): what stays the same from 0.4.0, and what is not promised
- [Troubleshooting](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md): known problems on each system, and fixes
- [Development](https://github.com/nci-gis/rust-commander/blob/main/docs/development.md): build, test, contribute

## Transparency

xdevcmd is developed with AI coding agents (Claude Code, Copilot); most of the code is
agent-written. A human maintainer sets the scope and the design, reviews the changes,
and tests each release by hand. Safety rests on checks you can run yourself rather
than on trust: about 950 tests, fault injection for copy, move and their journal,
fuzzed parsers, `cargo-deny`, no `unsafe` code in the engine, no network access, and
build provenance on every release. Found a problem? See
[SECURITY.md](https://github.com/nci-gis/rust-commander/blob/main/SECURITY.md).

## Licence

[MIT](https://github.com/nci-gis/rust-commander/blob/main/LICENSE). Developed by XDevTeam.

xdevcmd is written from scratch: a two-pane file manager with a terminal-style look and
feel.
