# Config

xdevcmd works without a config file. To change something, create one:

```bash
mkdir -p ~/.config/xdevcmd
xdevcmd --print-config > ~/.config/xdevcmd/config.toml   # every setting, with its default
xdevcmd --check-config                                   # check it; exit code 1 if wrong
```

Keep only the lines you change. Anything you leave out keeps its default.

**Where xdevcmd looks**: the file given with `--config PATH`; otherwise, first match
wins: `$XDEVCMD_CONFIG`,
`$XDG_CONFIG_HOME/xdevcmd/config.toml`, `%APPDATA%\xdevcmd\config.toml` (Windows),
`~/.config/xdevcmd/config.toml`.

**If the file has a mistake**, xdevcmd ignores the whole file, starts with the defaults,
and the status line names the problem. **F1 → Settings** shows what is in use.

**File → Settings…** inside xdevcmd changes the common settings and saves them here,
writing only the lines that changed. Everything else, keys included, is set in this
file.

## Common settings

```toml
[general]
confirm_transfer = true   # ask before copy and move start (delete always asks)
delete_to_trash = true    # F8 uses the trash; Shift+F8 always deletes for good
restore_session = true    # reopen last time's panes and tabs
autosave_secs = 5         # save them this often when they change: 5-3600, or 0 = on quit
safe_operations = true    # copies via a temporary name; a killed run is cleaned up (security.md)
menu_bar = true           # the File | View | Help bar

[files]
show_hidden = false
sort = "name"             # name | ext | size | modified
reverse = false           # the other way round
dirs_first = true         # directories before files
max_panes = 3             # how many file panes you can split into (2-16)
auto_refresh_ms = 1000    # how often panes look for changes where the system cannot tell
                          # (network drives; 10 times less often where it can); 0 = never
filter = "fuzzy"          # Ctrl+F: fuzzy (letters in order, best first) or plain
ls_colors = false         # use your $LS_COLORS
icons = false             # file icons; needs a Nerd Font
git_status = false        # a git letter per file; runs git (see security.md)

[terminal]
show = true
height_percent = 30       # 10-90
shell = ""                # empty: $SHELL, or %COMSPEC% (cmd.exe) on Windows
max_terminals = 3         # shells across all tabs and splits (1-16)

[viewer]
wrap = true
tab_width = 4             # 1-16
images = "auto"           # quick view's pictures: auto, kitty, iterm2, sixel or blocks

[commands]
editor = ""               # empty: $VISUAL, then $EDITOR, then vi (notepad on Windows)
opener = ""               # what o and Enter use; empty: the system's own
```

## Opening files

Enter on a file opens it as Total Commander does: with the first `[[open]]` rule for its
extension, else with the system's program (`open`, `explorer.exe`, `xdg-open`, or
`[commands] opener`). Programs, scripts and installers ask "Run it?" first; a type that
is not a known document asks "Open it?".

```toml
[files]
enter = "open"            # or "view": Enter views files

[[open]]
ext = ["txt", "log", "md"]
action = "view"           # or "edit": xdevcmd's viewer, or your editor

[[open]]
ext = ["pdf"]
command = ["zathura"]     # a program and its arguments; the file is added at the end
needs_display = true      # skipped over SSH and without a desktop

[[open]]
ext = ["rs", "toml"]
command = ["nvim"]
terminal = true           # in a terminal tab, not a window of its own
os = ["linux", "macos"]   # only on these systems
```

`command` is a list, never a line a shell reads, so a file name is always one argument.
Shift+O lists the rules that match the file under the cursor, with the system's program,
View and Edit.

## Colours

```toml
[theme]
name = "dark"             # dark, light, ansi, blue, high-contrast, or a file of yours
accent = "#ff8800"        # "#rrggbb", "reset" (the terminal's own), or "0"-"255"
background = "reset"      # let the terminal's background show through
colors = "auto"           # how many colours the terminal shows: auto, truecolor, 256, 16
```

**Themes.** `dark` is the default; `light` is for a light terminal; `ansi` uses only
your terminal's 16 colours, so it follows your terminal's palette; `blue` is the
blue-panel look of the DOS-era commanders; `high-contrast` is black and white with
bright colours. **View → Theme…** switches while xdevcmd runs; `name` keeps the choice.

Your own theme is a file `themes/<name>.toml` beside `config.toml` (or beside the xdevcmd
binary, for a portable copy). The release files come with three examples in `themes/`
(Nord, Gruvbox dark, Solarized light), which work as they are when kept beside
`xdevcmd`. To start your own, print a built-in theme with every colour listed:

```bash
mkdir -p ~/.config/xdevcmd/themes
xdevcmd --print-theme dark > ~/.config/xdevcmd/themes/mine.toml   # then [theme] name = "mine"
```

A theme file starts from a built-in theme and changes what it lists, with the same
names as `[theme]`:

```toml
# themes/sunset.toml — then [theme] name = "sunset"
base = "dark"
accent = "#ff8800"
cursor_active = "#5a3000"
```

Colours set in `[theme]` itself go on top of whichever theme is chosen.

**`colors = "auto"`** sends 24-bit colour only where the terminal is known to take it:
`COLORTERM=truecolor` (or `24bit`), iTerm2, WezTerm, kitty, Ghostty, Alacritty, VS Code,
a `TERM` ending in `-direct`, Windows Terminal and the Windows console, and
Terminal.app from macOS 26. Everywhere else it uses
the 256-colour palette, which every current terminal shows; the Linux console
(`TERM=linux`) gets a 16-colour theme drawn in your terminal's own palette. An older
Terminal.app is never sent 24-bit colour, whatever `COLORTERM` says, because it shows
it as green and red backgrounds.

Over SSH, `COLORTERM` does not usually reach the server, so xdevcmd there uses 256 colours.
To get 24-bit colour back, set it for that host:

```bash
export XDEVCMD_COLORS=truecolor   # in the server's shell profile; overrides [theme] colors
```

`XDEVCMD_COLORS` takes the same values, and is the way to set colours per machine when one
config file is shared between several.

`xdevcmd --print-config` lists every colour you can set.

## Keys

```toml
[keys]
copy = ["F5", "Ctrl+c"]   # F5 or Ctrl+C (naming an action replaces all its keys)
```

Naming an action replaces all of its default keys. The names are in
`xdevcmd --print-config` and on the F1 screen.

A binding can be two keys in a row, written with a space: `"Alt+r Left"` is Alt+R, then
Left. Bindings that start with the same key form a group, and there is no time limit
on the second key: a badge at the left of the status line shows the group is waiting,
and the hint beside it lists its keys.

- **A group of resize actions is a mode**: the second keys can be pressed again and
  again, until Enter, Esc, another key or a click. That is how **Alt+R, then arrows**
  resizes, under a `RESIZE` badge.
- **Any other action runs once**: after its second key the group closes, and the badge,
  `ALT+G …` for a group opened by Alt+G, goes. Esc cancels it.

Only resizing repeats; it is the one thing done in many small steps.

```toml
[keys]
resize_left = ["Alt+Shift+Left", "Alt+r Left", "Alt+r h"]
```

Two actions given the same key here is an error, and so is a key that both runs an
action and starts a group. Taking a key that xdevcmd uses by default is fine: that action
loses it.

## Bookmarks

```toml
[bookmarks]               # shown first in the hotlist (Ctrl+D); ~ is your home
projects = "~/projects"
logs = "/var/log"
```

Bookmarks you add inside xdevcmd are saved separately, never into this file.

## Your own commands

**Shift+F9** shows them. Each one is typed into the terminal, so you see its output.

```toml
[[menu]]
key = "g"
label = "git log of this file"
command = "git log --oneline -- %f"

[[menu]]
key = "z"
label = "zip the selection into the other pane"
command = "zip -r %D/selection.zip %s"
```

| | |
|---|---|
| `%f` / `%F` | file under the cursor: name / full path |
| `%s` / `%S` | selected files: names / full paths |
| `%d` / `%D` | this pane's directory / the other pane's |
| `%%` | a `%` sign |

xdevcmd quotes every file name for your shell, so spaces and quotes in names are safe.

## Security

```toml
[security]
mode = "normal"           # normal | read_only | restricted
root = ""                 # if set, xdevcmd never goes above this directory
```

See [security.md](security.md).

## Files xdevcmd keeps

Besides `config.toml`, which xdevcmd writes only when you save in **File → Settings…**
(and then only the lines you changed; a link is written where it points), and the file
you name with `--cwd-file`, written on quit, it writes these and nothing more:

| File | Where | What |
|---|---|---|
| `session.toml` | `~/.local/state/xdevcmd/` (Linux, macOS); `%LOCALAPPDATA%\xdevcmd\` (Windows) | The panes, tabs and directories open when you quit, and each terminal tab's directory. Saved on quit, every `autosave_secs`, and when the terminal closes. Readable by you only. |
| `journal/` | beside `session.toml` | Safe operations: one small file per running job, removed when it ends; what a killed run leaves is cleaned up at the next start. |
| `bookmarks.toml` | beside `session.toml` | Bookmarks added inside xdevcmd (Ctrl+D). Bookmarks in this config are not copied there. |
| `xdevcmd-run-<pid>/` | the system's temporary directory | One directory per run, readable by you only, for a file from inside an archive extracted for F3, and the list of names when renaming many files in your editor. Each is removed when you are done with it, the directory when xdevcmd ends; one left by a crash is removed at the next start, an hour on. |

No logs, no cache, nothing sent anywhere. F8 moves files to your system's own trash.

- `$XDG_STATE_HOME/xdevcmd/` replaces the state directory, and `XDEVCMD_STATE` names the
  session file itself (bookmarks go beside it).
- `restore_session = false` stops the session being saved.
- `--read-only` and `--restricted` save no session, no bookmarks and no settings. They
  still write the file named with `--cwd-file`, and the temporary copy of a file viewed
  from inside an archive, removed as above.
