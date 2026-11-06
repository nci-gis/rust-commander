# Changelog

All notable changes to rcmd. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

<!-- Write under the newest heading. `cargo release` puts the version and the date into
that heading and starts a new, empty one here. -->
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- Two-key bindings: `"Alt+r Left"` in `[keys]` is Alt+R, then Left. Bindings that
  start with the same key form a group, and its second keys can be pressed again and
  again until Enter, Esc or another key: **Alt+R, then arrows** resizes panes, on every system (macOS Terminal.app
  drops the modifiers from Alt+Shift+arrows).
- **Alt+1**…**Alt+9** go to tab N. Ctrl+PageUp / PageDown cannot be typed on a laptop
  (Ctrl+Fn+↑ loses Ctrl), and the terminal or desktop takes Ctrl+Tab.
- **Alt+J** also opens the directory history; a Mac cannot type Alt+↓.
- `[[open]]` rules: by extension, view, edit, or a program (as a list, no shell), in a
  window or a terminal tab, for some systems only, or only where there is a screen.
- **File → Settings…**: tabs for General, Files, Theme and Terminal; ←/→ to change,
  Enter to save. Only the changed lines of `config.toml` are rewritten, comments kept;
  the result is read back before it is written. Rows say which apply at the next start.
  Not in read-only or restricted mode.
- Themes: built-in `dark`, `light`, `ansi`, `blue` and `high-contrast`, chosen with
  `[theme] name`, or a file of your own in `themes/` that starts from one of them.
  **View → Theme…** switches while rcmd runs.
- **Ctrl+Z** undoes the last rename, rename-many, move, new directory or new file,
  after asking, and only while the disk still matches: nothing is overwritten or lost.
  After a copy or a delete it says those cannot be undone.
- **Shift+O**, Open with…: the rules for the file, the system's program, View, Edit.
- "Run it?" can run a program in a terminal tab, where its output stays after it ends.
- [Troubleshooting](docs/troubleshooting.md): known problems on each system and what
  to do; F1 links to it.
- Alt+B and Alt+F also go up and into a directory, because macOS Terminal.app sends
  Option+← and Option+→ as those, even with Option as Meta.
- `rcmd --show-keys` prints each key, click and scroll as rcmd receives it, and what
  it would do.
- On macOS, a character typed with Option in a file pane says once how to get Alt.

### Changed
- About shows rcmd's logo, drawn in the terminal.
- **Enter on a file opens it**, as in Total Commander: with an `[[open]]` rule for its
  extension, else the system's program; programs still ask "Run it?" first. Over SSH,
  without a desktop or in read-only mode it views the file and says why.
  `[files] enter = "view"` keeps the old Enter.
- About: the credit sits at the bottom right, beside the close button.
- Two actions given the same key in `[keys]` are now an error that names both; before,
  the one written later silently took it.
- The config documentation lists every file rcmd keeps, and where.

### Fixed
- `rcmd --screenshot … | head`, and any long output cut short by its reader, ended in
  a panic; it now ends quietly, as `cat` does.
- rcmd keeps its temporary files in one directory per run, removes it on exit, and at
  start removes what a crashed or killed run left behind.
- Theme files are looked for beside the config in use, including one given with
  `--config`.
- `o` over SSH into a Mac or a Windows machine opened the file on that machine's own
  screen; now it says there is no screen here, as it did on Linux.
- A file viewed from inside an archive is no longer left in the temporary directory
  when rcmd ends with the viewer still open.
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
