# rcmd

A terminal-style file commander, written in Rust.

```
  File   View   Help                                                                        F9 menu
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
 ..  ·  266 items  ·  sort name ▼                               F1 help · F12 terminal · F10 quit
 1 Help  2 Ren  3 View  4 Edit  5 Copy  6 Move  7 MkDir  8 Del  9 Menu  10 Quit  12 Term
```

Two file panes, a terminal underneath, and the familiar keys of two-pane file managers:
F5 copy, F6 move, F8 delete. Runs on Linux, macOS and Windows.

## Install

```bash
cargo install --git https://github.com/nci-gis/rust-commander rcmd-tui --locked
```

Needs Rust 1.88 or newer. This installs the `rcmd` command.

## Quick start

```bash
rcmd                  # open in the current directory
rcmd ~/src /tmp       # or with two directories
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
- Find files by name and content; rename many files in your editor
- Zip and tar archives open like folders
- Compare two folders, or make them match
- Preview text and pictures; checksums; permissions
- Mouse: click, drag and drop, right-click menu
- Read-only and restricted modes; no network, no telemetry

The [guide](docs/guide.md) explains each one.

## Configure

```bash
rcmd --print-config > ~/.config/rcmd/config.toml   # all settings, with defaults
```

Keep only the lines you change. See [docs/config.md](docs/config.md).

## Docs

- [Guide](docs/guide.md): what rcmd does, by topic
- [Keys](docs/keys.md): every key
- [Config](docs/config.md): settings, bookmarks, your own commands
- [Shell](docs/shell.md): stay in the directory you were in when you quit
- [Security](docs/security.md): what rcmd protects against, and safe modes
- [Development](docs/development.md): build, test, contribute

## Licence

[MIT](LICENSE). Developed by XDevTeam.

rcmd is written from scratch. It follows the way two-pane file managers work, without
using code from any of them.
