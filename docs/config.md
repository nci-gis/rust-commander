# Config

xdevcmd works without a config file. Write one only to change something.

**In short**, on Linux and macOS:

```bash
mkdir -p ~/.config/xdevcmd
xdevcmd --print-config > ~/.config/xdevcmd/config.toml   # every setting, with its default
xdevcmd --check-config                                   # check it; exit code 1 if wrong
```

On Windows, in PowerShell:

```powershell
New-Item -ItemType Directory -Force "$env:APPDATA\xdevcmd" | Out-Null
xdevcmd --print-config | Out-File -Encoding utf8 "$env:APPDATA\xdevcmd\config.toml"
xdevcmd --check-config
```

These are the default places. If `XDEVCMD_CONFIG` or `XDG_CONFIG_HOME` is set, the file
is elsewhere: see [Where the file is](#where-the-file-is).

- Keep only the lines you change; the rest keep their defaults.
- **File → Settings…** inside xdevcmd changes the common settings for you and writes
  only the lines that changed. Keys, colours and commands are set in the file.
- **Help → Config in use** shows which file is used, and why one was ignored.

| To change | Go to |
|---|---|
| Trash, hidden files, sort order, the shell panel, the viewer | [Common settings](#common-settings) |
| What Enter does on a file | [Opening files](#opening-files) |
| Theme and colours | [Colours](#colours) |
| A key | [Keys](#keys) |
| Saved directories | [Bookmarks](#bookmarks) |
| A menu of your own commands | [Your own commands](#your-own-commands) |
| Read-only or locked-in mode | [Security](#security) |
| What xdevcmd writes on disk | [Files xdevcmd keeps](#files-xdevcmd-keeps) |

## Where the file is

The file given with `--config PATH`. Otherwise xdevcmd takes the first of these that is
set, and reads only that file:

1. `$XDEVCMD_CONFIG`, if set and not empty
2. `$XDG_CONFIG_HOME/xdevcmd/config.toml`, if `XDG_CONFIG_HOME` is set and not empty
3. `%APPDATA%\xdevcmd\config.toml` (Windows)
4. `~/.config/xdevcmd/config.toml`

If the file it chose is missing, xdevcmd uses the defaults; it does not look further
down the list. A missing file named with `--config` is an error.

**If the file has a mistake**, xdevcmd ignores all of it and asks at start
(**Config not loaded**, with the error):

- **Enter**: go on with the defaults.
- **E**: open the file in your editor; it is checked again when you close it, and a
  mended file is used from the next start.
- **Q**: quit.

Nothing is ever saved over the file.

## Common settings

```toml
[general]
confirm_transfer = true   # ask before copy and move start (delete always asks)
delete_to_trash = true    # F8 uses the trash; Shift+F8 always deletes for good
restore_session = true    # reopen last time's panes and tabs
autosave_secs = 5         # save them this often when they change: 5-3600, or 0 = on quit
safe_operations = true    # copies via a temporary name; a killed run is cleaned up (security.md)
fast_copy = true          # the system's fast copy: a clone, or the kernel's copy (security.md)
menu_bar = true           # the File | View | Help bar
animation = false         # a small agent walks along the status line, a step a second

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
scrollback = 5000         # lines of history each shell keeps (100-9999), for the wheel
                          # and Shift+F12; at most lines × columns × 32 bytes
                          # (Settings shows it); applies to shells started after a change
send_when_idle = true     # Alt+C, typed names and the user menu go to a shell surely
                          # at its prompt, with nothing typed since the last Enter,
                          # without asking; false asks every time (docs/shell.md)
follow = false            # the file pane used last goes where the shell's cd goes, at
                          # the prompt only; never the other way (docs/shell.md)

[viewer]
wrap = true
tab_width = 4             # 1-16
highlight = true          # colour source, config files, diffs and logs by language:
                          # F3 up to 64 MiB, quick view up to 1 MiB; off under NO_COLOR
images = "auto"           # quick view's pictures: auto, kitty, iterm2, sixel or blocks
                          # (sixel only where the terminal gives its cell size)

[commands]
editor = ""               # empty: $VISUAL, then $EDITOR, then vi (notepad on Windows)
opener = ""               # what o and Enter use; empty: the system's own
```

## Opening files

Enter on a file uses the first `[[open]]` rule for its extension, else the system's
program (`open`, `explorer.exe`, `xdg-open`, or `[commands] opener`), as Total Commander
does. Programs, scripts and installers ask "Run it?" first; a type that is not a known
document asks "Open it?".

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

- `command` is a list, never a line a shell reads, so a file name is always one argument.
- **Shift+O** lists the rules that match the file under the cursor, with the system's
  program, View and Edit.

## Colours

```toml
[theme]
name = "xdevtheme"        # xdevtheme, dark, light, ansi, blue, high-contrast, or a file
accent = "#ff8800"        # "#rrggbb", "reset" (the terminal's own), or "0"-"255"
background = "reset"      # let the terminal's background show through
colors = "auto"           # how many colours the terminal shows: auto, truecolor, 256, 16
```

**View → Theme…** switches while xdevcmd runs; `name` keeps the choice. To try one for a
single run: `XDEVCMD_THEME=xdevtheme-neon xdevcmd`. `xdevcmd --print-config` lists every
colour you can set; colours set in `[theme]` go on top of the chosen theme.

### Built-in themes

| Theme | Look |
|---|---|
| `xdevtheme` | The default: teal running to violet across the screen, rounded frames, titles in a notch, a lit cursor row, slanted F keys |
| `dark` | The plain look it replaced: one teal accent |
| `light` | For a light terminal |
| `ansi` | Only your terminal's 16 colours, so it follows your palette |
| `blue` | The blue panels of the DOS-era commanders |
| `high-contrast` | Black and white with bright colours |

The release files bring nine more in `themes/`, which work when kept beside `xdevcmd`:

- **xdevtheme-neon**: the light runs teal, violet and pink; the focused frame glows.
- **xdevtheme-glass**: no lines; panes are cards, the path and cursor row are pills.
- **xdevtheme-light**: xdevtheme for a light terminal (macOS Terminal's default profile).
- Dracula, Tokyo Night, Catppuccin Mocha, Nord, Gruvbox dark, Solarized light.

### Your own theme

A theme is a file `themes/<name>.toml` beside `config.toml` (or beside the binary, for a
portable copy). Start from a printed one, which lists every colour (`--print-theme`
also takes a file's name, such as `xdevtheme-light`):

```bash
mkdir -p ~/.config/xdevcmd/themes
xdevcmd --print-theme xdevtheme > ~/.config/xdevcmd/themes/mine.toml   # then [theme] name = "mine"
```

A theme file starts from a built-in theme and changes what it lists, with the same names
as `[theme]`:

```toml
# themes/sunset.toml — then [theme] name = "sunset"
base = "dark"
accent = "#ff8800"
cursor_active = "#5a3000"
```

It may also say how it draws, in a `[style]` table. Every line is optional; leaving it
out keeps the base's look:

```toml
# themes/glow.toml — then [theme] name = "glow"
base = "dark"

[style]
borders = "rounded"     # plain, rounded, thick, double, or none: each pane a card
titles = "notch"        # line, notch (╭┤title├─), or pill: the focused title on the light
gradient = "#a46bff"    # a light from the accent to this colour, on the frames, tabs, keys
                        # and menus in the accent's colours; two colours make three stops,
                        # gradient = ["#a46bff", "#ff5fb0"]; or "none"
gradient_runs = "across"  # across (left to right) or down (top to bottom)
cursor = "glow"         # block, glow (lit from its left edge) or pill
marker = true           # a ▌ on the frame at the cursor's row
keys = "slant"          # the F keys: plain, caps (the number on the accent), or slant (◢1◤),
                        # which also draws the selected tab and the menu bar's number so
shadow = "dim"          # none, dim, or glow: under dialogs and menus
glow = false            # the focused frame lights the cells around it
sky = false             # the backgrounds take a faint aurora, top to bottom
stars = false           # a few fixed stars in the empty rows of the file panes
```

- The style uses only box-drawing, block and geometric characters, no Nerd Font.
- On a 16-colour terminal it is off (the glyphs may be missing, and there is nothing to
  blend); at 256 colours the gradient is coarser.
- Programs in the shell panel keep their own colours.
- On a light panel the lit cursor row is fainter and the `dim` shadow a soft grey, so
  names stay readable.

**A missing theme** (deleted, renamed, a typo, or a file that does not read) does not
stop the rest of the config: xdevtheme stands in and the status line names it.
`--check-config` still reports it and exits 1.

### How many colours

`colors = "auto"` sends 24-bit colour only where the terminal is known to take it:
`COLORTERM=truecolor` (or `24bit`), iTerm2, WezTerm, kitty, Ghostty, Alacritty, VS Code,
a `TERM` ending in `-direct`, Windows Terminal and the Windows console, and Terminal.app
from macOS 26. Elsewhere it uses 256 colours, which every current terminal shows.

- The Linux console (`TERM=linux`) gets a 16-colour theme in your terminal's palette.
- An older Terminal.app never gets 24-bit colour, whatever `COLORTERM` says: it shows it
  as green and red backgrounds.
- Over SSH, `COLORTERM` usually does not reach the server, so you get 256 colours. To
  get 24-bit back, in the server's shell profile:

```bash
export XDEVCMD_COLORS=truecolor   # in the server's shell profile; overrides [theme] colors
```

`XDEVCMD_COLORS` takes the same values as `colors`; use it to set colours per machine
when several share one config file.

## Keys

```toml
[keys]
copy = ["F5", "Ctrl+c"]   # F5 or Ctrl+C (naming an action replaces all its keys)
```

- **Naming an action replaces all its default keys.** The names are in
  `xdevcmd --print-config`. F1 pressed twice shows the selected command's name on its last
  line (`[keys] copy`); a key you changed has a `•` there.
- **Taking a key xdevcmd uses by default is fine**: that action loses it.
- **Errors**: two actions with the same key, or a key that both runs an action and
  starts a group.

### Two keys in a row

Write them with a space: `"Alt+r Left"` is Alt+R, then Left. Bindings that start with
the same key form a group. There is no time limit on the second key: a badge at the left
of the status line shows the group is waiting, and the hint beside it lists its keys.

- **Resize groups repeat**: press the second keys again and again until Enter, Esc,
  another key or a click. That is how **Alt+R, then arrows** resizes, under a `RESIZE`
  badge. Only resizing repeats; it is the one thing done in many small steps.
- **Any other group runs once**: after its second key the group closes and its badge
  (`ALT+G …` for Alt+G) goes. Esc cancels it.

```toml
[keys]
resize_left = ["Alt+Shift+Left", "Alt+r Left", "Alt+r h"]
```

### Keys with rules

- **`help` always keeps F1.** `help = ["F1", "Ctrl+F1"]` adds a key beside it; a `help`
  without F1, or F1 for any other action, is an error that says so. Inside the shell F1
  is the program's, and **Alt+X F1** opens Commands there (and in the panes).
- **Usage on Alt+X F1, as before 0.4**: `usage = ["Shift+F1", "Alt+x F1"]`. Alt+X F1 is
  then Usage in the panes and goes to the program in the shell; reach Commands from the
  shell with F12, then F1.
- **`focus_terminal` and `terminal_history`** work inside the shell, where xdevcmd takes
  only F12's family. They take only `F12`, `Shift+F12`, `Alt+x F12` and
  `Alt+x Shift+F12`; any other key (`Ctrl+t`, `Ctrl+x h`) is an error, since it would
  take that key from the shell.

## Bookmarks

```toml
[bookmarks]               # shown first in the hotlist (Ctrl+D); ~ is your home
projects = "~/projects"
logs = "/var/log"
```

Bookmarks you add inside xdevcmd are saved separately, never into this file.

## Your own commands

**Shift+F9** shows them. Each one is typed into the shell panel, so you see its output.

```toml
[[menu]]
key = "g"
label = "git log of this file"
command = "git log --oneline -- %f"

[[menu]]
key = "z"
label = "zip the selection into the other pane"
command = "zip -r %D/selection.zip %s"

# A copy that keeps what xdevcmd's does not (owner, ACLs, extended attributes, hard
# links, sparse files): see guide.md, "What xdevcmd doesn't do".
[[menu]]
key = "r"
label = "rsync the selection to the other pane, keeping everything"
command = "rsync -aHAXS --info=progress2 %S %D/"

# The same on Windows, for files: robocopy takes the folder, the target and the file
# names (a folder is copied with `robocopy <folder> <target>\<name> /E /COPYALL`).
[[menu]]
key = "R"
label = "robocopy the selected files to the other pane, keeping everything"
command = "robocopy %d %D %s /COPYALL /SL"
```

| Placeholder | Becomes |
|---|---|
| `%f` / `%F` | file under the cursor: name / full path |
| `%s` / `%S` | selected files: names / full paths |
| `%d` / `%D` | this pane's directory / the other pane's |
| `%%` | a `%` sign |

xdevcmd quotes every file name for your shell, so spaces and quotes in names are safe.
When a command hands its text to another shell (`sh -c`, `bash -c`, `eval`, `ssh`), put
`%f` and the others inside single quotes there (`sh -c 'wc -l %f'`), never double quotes:
the second shell reads the name again.

## Security

```toml
[security]
mode = "normal"           # normal | read_only | restricted
root = ""                 # if set, xdevcmd never goes above this directory
```

See [security.md](security.md).

## Files xdevcmd keeps

No logs unless you ask for one, no cache, nothing sent anywhere. F8 moves files to your
system's own trash. Besides these, xdevcmd writes `config.toml` only when you save in
**File → Settings…** (only the changed lines; a link is written where it points), and the
file named with `--cwd-file`, on quit.

| File | Where | What |
|---|---|---|
| `session.toml` | `~/.local/state/xdevcmd/` (Linux, macOS); `%LOCALAPPDATA%\xdevcmd\` (Windows) | The panes, tabs and directories open when you quit, and each terminal tab's directory. Saved on quit, every `autosave_secs`, and when the terminal closes. Readable by you only. |
| `journal/` | beside `session.toml` | Safe operations: one small file per running job, removed when it ends; what a killed run leaves is cleaned up at the next start. |
| `crash-….log` | beside `session.toml` | Written only if xdevcmd crashes: what happened, for a report. The five newest are kept. |
| `debug-….jsonl` | beside `session.toml`, or `XDEVCMD_DEBUG_DIR` | Only with `XDEVCMD_DEBUG` set: what jobs did and how long each part took, with no names ([Troubleshooting](troubleshooting.md#sending-a-log-with-a-report)). The five newest are kept. |
| `bookmarks.toml` | beside `session.toml` | Bookmarks added inside xdevcmd (Ctrl+D). Bookmarks in this config are not copied there. |
| `xdevcmd-run-<pid>/` | the system's temporary directory | One directory per run, readable by you only, for a file from inside an archive extracted for F3, and the list of names when renaming many files in your editor. Each is removed when you are done with it, the directory when xdevcmd ends; one left by a crash is removed at the next start, an hour on. |

- `$XDG_STATE_HOME/xdevcmd/` replaces the state directory, and `XDEVCMD_STATE` names the
  session file itself, by its full path, such as `/tmp/x/session.toml`: the journal,
  crash logs and bookmarks go beside it. A folder or a relative path is refused at the
  start.
- `restore_session = false` stops the session being saved.
- `--read-only` and `--restricted` save no session, no bookmarks and no settings. They
  still write the file named with `--cwd-file`, and the temporary copy of a file viewed
  from inside an archive, removed as above.
