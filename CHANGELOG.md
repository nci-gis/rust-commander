# Changelog

All notable changes to xdevcmd. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

<!-- Write under the newest heading. `cargo release` puts the version and the date into
that heading and starts a new, empty one here. -->
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- **Example themes in the release files**: `themes/` holds Nord, Gruvbox dark and
  Solarized light. Kept beside `xdevcmd`, they appear in View → Theme… as they are.
- **`xdevcmd --print-theme NAME`** writes a built-in theme out as a file with every
  colour listed, to start your own from.
- **Ctrl+G** also goes to a path you type, beside Ctrl+L.

### Changed
- **Idle, xdevcmd now uses almost no CPU**: about 0.3 % of a core instead of 5 %. The
  screen is drawn when something changed (a key, a click, a shell's output, a job's
  progress, background work reporting in) and once a second otherwise, not 25 times a
  second.

### Security
- **Renaming never replaces a file that appears meanwhile.** F2, renaming many and
  moving into the Linux trash ask the system to refuse a name that is taken, in the
  same step as the rename; before, a file created between xdevcmd's check and the
  rename was overwritten.
- **A program is never found in the directory you browse.** On Linux and macOS a
  program run in a terminal tab by its bare name is looked up only in `PATH`'s absolute
  directories; a `.` or empty entry used to make it look in the browsed directory.

### Fixed
- **The logo in About looked striped and stepped on macOS** in Terminal.app. It is now
  drawn with background colour only, one colour per character cell, so it no longer
  depends on how the terminal's font draws half blocks. In picture previews too, a cell
  whose two halves are one colour is drawn the same way.

## [0.2.0] - 2026-11-07

### Changed
- **rcmd is now xdevcmd**, everywhere: the command, the release files, the config and
  state directories (`~/.config/xdevcmd/`, `~/.local/state/xdevcmd/`;
  `%APPDATA%\xdevcmd\` and `%LOCALAPPDATA%\xdevcmd\` on Windows) and the environment
  variables (`XDEVCMD_CONFIG`, `XDEVCMD_STATE`, `XDEVCMD_COLORS`). Nothing is read from
  the old names. The repository stays `nci-gis/rust-commander`.
- **Enter on a file opens it**, as in Total Commander: with an `[[open]]` rule for its
  extension, else the system's program. Programs, scripts and installers ask "Run it?"
  first, and a type that is not a known document asks "Open it?". Where there is no
  screen to open it on (over SSH, without a desktop) and no `[commands] opener` is set,
  or in read-only mode, it views the file and says why. `[files] enter = "view"` makes
  Enter view. A double-click does what Enter does.
- `o` asks "Run it?" for many more kinds of file that the system runs rather than
  shows: installers and disk images, Java, Python and PowerShell files and office
  files with macros on Windows; AppImages, packages and Windows programs (through Wine)
  on Linux; `.command`, `.pkg` and profiles on macOS.
- Two actions given the same key in `[keys]` are now an error that names both; before,
  the one written later silently took it.
- About shows xdevcmd's logo beside its text, drawn as sharp pixel art; the credit sits
  at the bottom right, beside the close button.

### Added
- **File → Settings…**: tabs for General, Files, Theme and Terminal, and a Keys tab
  that says how to rebind; ←/→ to change, Enter to save, or the mouse: click a tab, a
  check box, a value's ‹ or ›, or a button. Only the changed lines of `config.toml` are
  rewritten, comments kept, and the result is read back before it is written; a
  `config.toml` that is a link is written where it points. Rows say which apply at the
  next start. In read-only and restricted mode it shows the settings but saves nothing.
- Themes: built-in `dark`, `light`, `ansi`, `blue` and `high-contrast`, chosen with
  `[theme] name`, or a file of your own in `themes/` beside the config in use, which
  starts from one of them. **View → Theme…** switches while xdevcmd runs.
- **Ctrl+Z** undoes the last rename, rename-many, move, new directory or new file,
  after asking, and only while the disk still matches: nothing is overwritten or lost.
  After a copy or a delete it says those cannot be undone.
- `[[open]]` rules: by extension, view, edit, or a program (as a list, no shell), in a
  window or a terminal tab, for some systems only, or only where there is a screen.
- **Shift+O**, Open with…: the rules for the file, the system's program, View, Edit.
- "Run it?" can run a program in a terminal tab, where its output stays after it ends;
  Alt+C and the user menu never type into it. On Windows, a batch file is refused there
  when a file name holds a character `cmd.exe` would act on (`&`, `%`, `^`, `,`…), so a
  name like `a&calc.txt` cannot start a second command, or when its path has a space.
- Two-key bindings: `"Alt+r Left"` in `[keys]` is Alt+R, then Left. Bindings that start
  with the same key form a group. Resizing repeats: **Alt+R, then arrows** resizes
  panes until Enter, Esc or another key, on every system (macOS Terminal.app drops the
  modifiers from Alt+Shift+arrows). Any other action runs once and the group closes.
  While a group is open, a badge at the left of the status line says so (`RESIZE`).
- **Alt+1**…**Alt+9** go to tab N. Ctrl+PageUp / PageDown cannot be typed on a laptop
  (Ctrl+Fn+↑ loses Ctrl), and the terminal or desktop takes Ctrl+Tab.
- **Alt+J** also opens the directory history; a Mac cannot type Alt+↓.
- Alt+B and Alt+F also go up and into a directory, because macOS Terminal.app sends
  Option+← and Option+→ as those, even with Option as Meta.
- Click a directory's name in a pane's path, at the top, to go there.
- Double-click a border to share it evenly between the two panes on either side of it;
  with three or more, the others keep their size. The same for the terminal's top edge
  and between shells in a terminal tab.
- `xdevcmd --show-keys` prints each key, click and scroll as xdevcmd receives it, and
  what it would do.
- On macOS, a character typed with Option in a file pane says once how to get Alt.
- [Troubleshooting](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md):
  known problems on each system and what to do; F1 links to it.

### Security
- With `--root`, F4, F7 and Alt+F5 (pack) stay inside the root as Enter and F3 do: F4
  no longer edits a link that points outside it, and F7 and Alt+F5 refuse a name that
  climbs out with `..`. An archive reached through a link from inside the root to
  outside it is not opened or unpacked.
- Quick view of a picture refuses anything that is not a regular file: a FIFO named
  like a picture froze xdevcmd, and a link to `/dev/zero` read until memory ran out.

### Fixed
- Colours on terminals without 24-bit colour. macOS Terminal.app before macOS 26 showed
  green and red backgrounds and hard-to-read text, because it reads 24-bit colour codes
  as other attributes. xdevcmd now sends 24-bit colour only where the terminal is known
  to take it and the 256-colour palette everywhere else; the Linux console gets a
  16-colour theme. `[theme] colors` (`auto`, `truecolor`, `256`, `16`) or
  `XDEVCMD_COLORS` sets it by hand.
- `o` over SSH into a Mac or a Windows machine opened the file on that machine's own
  screen; now it says there is no screen here, as it did on Linux.
- xdevcmd keeps its temporary files in one directory per run, removes it on exit, and
  at start removes what a crashed or killed run left behind, never one still in use.
- Directories named relative to where xdevcmd was started are made absolute, so the
  title and Backspace work from them.
- `xdevcmd --screenshot … | head`, and any long output cut short by its reader, ended
  in a panic; it now ends quietly, as `cat` does.

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
