# Changelog

All notable changes to rcmd. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - unreleased

The first release.

### Files
- Copy, move, delete (to the trash or for good), rename, with progress, cancel, and a
  question before anything is overwritten. Jobs can run in the background.
- Rename many files at once by editing their names in your editor.
- Copy beside itself, new file, symlink, permissions, SHA-256 checksums.
- Zip, tar and tar.gz archives open like folders; extract, pack and unpack.
- Compare two folders, and make two folder trees match from a plan you approve.

### Finding your way
- Quick search, filter, go to path, back and forward, history, bookmarks.
- Find files by name and content; results open in a tab.
- Quick view of text and pictures in the other pane.

### Interface
- Two or more file panes with tabs, splits and zoom; your layout reopens next time.
- A real shell underneath, with tabs and splits.
- Menu bar (F9), help with every key and setting (F1), and a usage view (Shift+F1).
- Mouse: click, drag and drop, right-click menus, drag borders to resize.
- Optional: colours from `$LS_COLORS`, Nerd Font icons, a git status column.

### Configuration
- One TOML file, every setting optional; `--print-config` and `--check-config`.
- Your own commands (`[[menu]]`), typed into the terminal with names safely quoted.
- `--cwd-file`, so your shell can follow you out of rcmd.

### Security
- Read-only and restricted modes, and a root directory to stay inside.
- Hostile file names, archives and images are handled; no network, no telemetry.

### Known limitations
- On Windows: the terminal cannot tell whether a program is running in it, the config
  file's permissions are not checked, and the usage view does not measure memory.
- The fish and PowerShell versions of the cd-on-exit function have not been tested.

[Unreleased]: https://github.com/nci-gis/rust-commander/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nci-gis/rust-commander/releases/tag/v0.1.0
