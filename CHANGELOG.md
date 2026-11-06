# Changelog

All notable changes to rcmd. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

<!-- Write under the newest heading. `cargo release` puts the version and the date into
that heading and starts a new, empty one here. -->
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- Esc as a prefix in the file panes, for keyboards and terminals without Alt or
  F-keys (a Mac's Option key types characters; its F-keys are media keys): Esc then a
  key is Alt+key, Esc then 1…9, 0 is F1…F10. `[general] esc_as_meta = false` turns
  it off.
- [Troubleshooting](docs/troubleshooting.md): known problems on each system and what
  to do; F1 links to it.
- `rcmd --show-keys` prints each key, click and scroll as rcmd receives it, and what
  it would do.
- On macOS, a character typed with Option in a file pane says once how to get Alt.

### Changed
- About: the credit sits at the bottom right, beside the close button.

### Fixed
- Colours on terminals without 24-bit colour. macOS Terminal.app before macOS 26 showed
  green and red backgrounds and hard-to-read text, because it reads 24-bit colour codes
  as other attributes. rcmd now sends 24-bit colour only where the terminal is known to
  take it and the 256-colour palette everywhere else; the Linux console gets a
  16-colour theme. `[theme] colors` (`auto`, `truecolor`, `256`, `16`) or `RCMD_COLORS`
  sets it by hand.

## [0.1.0] - 2026-11-06

The first release.

### Files
- Copy, move, delete (to the trash or for good), rename, with progress, cancel, and a
  question before anything is overwritten. Jobs can run in the background.
- Rename many files at once with a pattern (name, extension, counter, find and replace,
  case) and a live preview, or by editing the names in your editor.
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
- An icon, in `rcmd.exe` on Windows, and a logo.
- Scrollbars on file panes, the viewer and the terminal's history, shown only when there is
  more than fits; drag them or click the track.
- The viewer: drag over lines to select them, `y` copies the selection.
- `~` goes home, `Ctrl+\` to the root of the drive.
- Nothing is written to the terminal while nothing on screen changes.
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
