<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img alt="xdevcmd" src="assets/logo-light.svg" width="498">
  </picture>
</p>

<p align="center">A terminal-style file commander, written in Rust.</p>

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
F5 copy, F6 move, F8 delete. Runs on Linux, macOS and Windows.

## Install

```bash
cargo install --git https://github.com/nci-gis/rust-commander xdevcmd-tui --locked
```

Needs Rust 1.88 or newer. This installs the `xdevcmd` command.

Or download a ready-made build for Linux, macOS or Windows from
[Releases](https://github.com/nci-gis/rust-commander/releases). They are not signed, so
macOS and Windows warn the first time: see
[troubleshooting](docs/troubleshooting.md#macos).

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

The [guide](docs/guide.md) explains each one.

## Configure

```bash
mkdir -p ~/.config/xdevcmd
xdevcmd --print-config > ~/.config/xdevcmd/config.toml   # all settings, with defaults
```

Keep only the lines you change. See [docs/config.md](docs/config.md).

## Docs

- [Guide](docs/guide.md): what xdevcmd does, by topic
- [Keys](docs/keys.md): every key
- [Config](docs/config.md): settings, bookmarks, your own commands
- [Shell](docs/shell.md): stay in the directory you were in when you quit
- [Security](docs/security.md): what xdevcmd protects against, and safe modes
- [Troubleshooting](docs/troubleshooting.md): known problems on each system, and fixes
- [Development](docs/development.md): build, test, contribute

## Licence

[MIT](LICENSE). Developed by XDevTeam.

xdevcmd is written from scratch: a two-pane file manager with a terminal-style look and
feel.
