# Config

rcmd works without a config file. To change something, create one:

```bash
rcmd --print-config > ~/.config/rcmd/config.toml   # every setting, with its default
rcmd --check-config                                # check it; exit code 1 if wrong
```

Keep only the lines you change. Anything you leave out keeps its default.

**Where rcmd looks**, first match wins: `$RCMD_CONFIG`,
`$XDG_CONFIG_HOME/rcmd/config.toml`, `%APPDATA%\rcmd\config.toml` (Windows),
`~/.config/rcmd/config.toml`.

**If the file has a mistake**, rcmd ignores the whole file, starts with the defaults,
and the status line names the problem. **F1 → Settings** shows what is in use.

**File → Settings…** inside rcmd changes the common settings and saves them here,
writing only the lines that changed. Everything else, keys included, is set in this
file.

## Common settings

```toml
[general]
confirm_transfer = true   # ask before copy and move start (delete always asks)
delete_to_trash = true    # F8 uses the trash; Shift+F8 always deletes for good
restore_session = true    # reopen last time's panes and tabs
autosave_secs = 5         # save them this often when they change; 0 = only on quit
menu_bar = true           # the File | View | Help bar

[files]
show_hidden = false
sort = "name"             # name | ext | size | modified
max_panes = 3             # how many file panes you can split into (2-16)
auto_refresh_ms = 1000    # how often panes look for changes; 0 = never
ls_colors = false         # use your $LS_COLORS
icons = false             # file icons; needs a Nerd Font
git_status = false        # a git letter per file; runs git (see security.md)

[terminal]
show = true
height_percent = 30
shell = ""                # empty: $SHELL, or cmd.exe on Windows
max_terminals = 3         # shells across all tabs and splits (1-16)

[viewer]
wrap = true
tab_width = 4

[commands]
editor = ""               # empty: $VISUAL, then $EDITOR, then vi (notepad on Windows)
opener = ""               # what `o` uses; empty: the system's own
```

## Opening files

Enter on a file opens it as Total Commander does: with the first `[[open]]` rule for its
extension, else with the system's program (`open`, `explorer.exe`, `xdg-open`, or
`[commands] opener`). Programs are asked about first.

```toml
[files]
enter = "open"            # or "view": Enter views files, as in rcmd 0.1.0

[[open]]
ext = ["txt", "log", "md"]
action = "view"           # or "edit": rcmd's viewer, or your editor

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
accent = "#ff8800"        # "#rrggbb", "reset" (the terminal's own), or 0-255
background = "reset"      # let the terminal's background show through
colors = "auto"           # how many colours the terminal shows: auto, truecolor, 256, 16
```

**Themes.** `dark` is the default; `light` is for a light terminal; `ansi` uses only
your terminal's 16 colours, so it follows your terminal's palette; `blue` is the
blue-panel look of the DOS-era commanders; `high-contrast` is black and white with
bright colours. **View → Theme…** switches while rcmd runs; `name` keeps the choice.

Your own theme is a file `themes/<name>.toml` beside `config.toml` (or beside the rcmd
binary, for a portable copy). It starts from a built-in theme and changes what it lists,
with the same names as `[theme]`:

```toml
# themes/sunset.toml — then [theme] name = "sunset"
base = "dark"
accent = "#ff8800"
cursor_active = "#5a3000"
```

Colours set in `[theme]` itself go on top of whichever theme is chosen.

**`colors = "auto"`** sends 24-bit colour only where the terminal is known to take it:
`COLORTERM=truecolor` (or `24bit`), iTerm2, WezTerm, kitty, Ghostty, VS Code, Windows
Terminal and the Windows console, and Terminal.app from macOS 26. Everywhere else it uses
the 256-colour palette, which every current terminal shows; the Linux console
(`TERM=linux`) gets a 16-colour theme drawn in your terminal's own palette. An older
Terminal.app is never sent 24-bit colour, whatever `COLORTERM` says, because it shows
it as green and red backgrounds.

Over SSH, `COLORTERM` does not usually reach the server, so rcmd there uses 256 colours.
To get 24-bit colour back, set it for that host:

```bash
export RCMD_COLORS=truecolor   # in the server's shell profile; overrides [theme] colors
```

`RCMD_COLORS` takes the same values, and is the way to set colours per machine when one
config file is shared between several.

`rcmd --print-config` lists every colour you can set.

## Keys

```toml
[keys]
copy = ["F2", "Ctrl+c"]   # copy is now F2 or Ctrl+C, and no longer F5
```

Naming an action replaces all of its default keys. The names are in
`rcmd --print-config` and on the F1 screen.

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
action and starts a group. Taking a key that rcmd uses by default is fine: that action
loses it.

## Bookmarks

```toml
[bookmarks]               # shown first in the hotlist (Ctrl+D); ~ is your home
projects = "~/projects"
logs = "/var/log"
```

Bookmarks you add inside rcmd are saved separately, never into this file.

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

rcmd quotes every file name for your shell, so spaces and quotes in names are safe.

## Security

```toml
[security]
mode = "normal"           # normal | read_only | restricted
root = ""                 # if set, rcmd never goes above this directory
```

See [security.md](security.md).

## Files rcmd keeps

rcmd reads `config.toml` and never writes it. What it does write, and nothing else:

| File | Where | What |
|---|---|---|
| `session.toml` | `~/.local/state/rcmd/` (Linux, macOS); `%LOCALAPPDATA%\rcmd\` (Windows) | The panes, tabs and directories open when you quit, and each terminal tab's directory. Saved on quit and every `autosave_secs`. Readable by you only. |
| `bookmarks.toml` | beside `session.toml` | Bookmarks added inside rcmd (Ctrl+D). Bookmarks in this config are not copied there. |
| `rcmd-view-…/` | the system's temporary directory | A file from inside an archive, extracted for F3. Removed when the viewer closes, or when rcmd ends. |
| `rcmd-rename-….txt` | the system's temporary directory | The list of names when renaming many files in your editor. Removed when the editor closes. |

No logs, no cache, nothing sent anywhere. F8 moves files to your system's own trash.

- `$XDG_STATE_HOME/rcmd/` replaces the state directory, and `RCMD_STATE` names the
  session file itself (bookmarks go beside it).
- `restore_session = false` stops the session being saved.
- `--read-only` and `--restricted` save no session and no bookmarks. The one thing
  they still write is the temporary copy of a file viewed from inside an archive,
  removed as above.
