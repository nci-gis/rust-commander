# Keys

These are the defaults. Press **F1** in rcmd to see your own, after any changes in
`[keys]` in the config. This file is generated: `rcmd --print-keys > docs/keys.md`.

## General

| Keys | Does |
|---|---|
| `Ctrl+q` `F10` `q` | Quit rcmd |
| `F1` | This screen: every key and setting |
| `Ctrl+r` | Re-read the active pane |
| `Ctrl+h` | Show or hide hidden files |
| `Q` | Quit, leaving the shell where it was (with --cwd-file) |
| `Shift+F9` | Your commands from [[menu]], run in the terminal |
| `F9` | The menu bar: File, View, Help |
| `Shift+F1` | What rcmd is using: memory and its trend, CPU, threads, draw time |

## Navigate

| Keys | Does |
|---|---|
| `Up` | Move up |
| `Down` | Move down |
| `PageUp` | Page up |
| `PageDown` | Page down |
| `Home` | First entry |
| `End` | Last entry |
| `Enter` | Enter a directory, or view a file |
| `Alt+Right` | Enter the directory under the cursor |
| `Alt+Left` `Backspace` | Go up one directory |
| `/` `Ctrl+s` | Jump to a name as you type it; Up/Down for the next match |
| `Ctrl+f` | Show only names matching what you type (*.rs works too) |
| `Ctrl+l` | Type a path to go to |
| `Alt+,` | Back to the previous directory |
| `Alt+.` | Forward again |
| `Alt+Down` | Directories visited in this tab |
| `Ctrl+d` | Bookmarks: go to one, or bookmark this directory |

## Select

| Keys | Does |
|---|---|
| `Space` | Mark or unmark the entry |
| `Insert` | Mark, then move down |
| `*` | Invert all marks |
| `Ctrl+a` | Mark everything |
| `Alt+u` | Clear all marks |
| `+` | Mark by pattern, e.g. *.jpg *.png |
| `-` | Unmark by pattern |

## Files

| Keys | Does |
|---|---|
| `F3` | View the file |
| `F4` | Edit the file in your editor |
| `F5` | Copy to the other pane |
| `F6` | Move to the other pane |
| `F2` `Shift+F6` | Rename the entry under the cursor |
| `F7` | Create a directory |
| `F8` | Delete (to the trash, by default) |
| `Shift+F8` | Delete permanently |
| `o` | Open with the system's default application |
| `Ctrl+Space` | Work out the size of every directory shown |
| `Alt+F7` | Find files by name and content; results open in a new tab |
| `Shift+F2` | Rename the selection with a pattern ([N], [C], find and replace), or in $EDITOR |
| `Shift+F4` | Create a file and open it in the editor |
| `Shift+F5` | Copy beside itself under another name |
| `Ctrl+Shift+F5` | Link to this in the other pane's directory |
| `Alt+p` | Permissions: an octal mode, or read-only on Windows |
| `Alt+h` | SHA-256 of the file, checked against a .sha256 beside it |
| `Alt+F5` | Pack the selection into a .zip, .tar or .tar.gz |
| `Alt+F9` | Unpack the archive under the cursor into the other pane |

## Sort

| Keys | Does |
|---|---|
| `Ctrl+F3` | Sort by name (again to reverse) |
| `Ctrl+F4` | Sort by extension |
| `Ctrl+F5` | Sort by size |
| `Ctrl+F6` | Sort by date |

## Panes

| Keys | Does |
|---|---|
| `Tab` | Next file pane |
| `Alt+s` | Split the pane, or the shell, side by side |
| `Alt+d` | Split the pane, or the shell, top and bottom |
| `Alt+w` | Close the pane, or the focused shell |
| `Alt+Shift+Left` | Move the pane's border left |
| `Alt+Shift+Right` | Move the pane's border right |
| `Alt+Shift+Up` | Move the pane's border up |
| `Alt+Shift+Down` | Move the pane's border down |
| `Alt+z` | Fill the screen with this pane, or put it back |
| `Alt+o` | Next split, in the shell or the file panes |
| `Ctrl+u` | Swap this pane with the other one |
| `Alt+=` | Show this directory in the other pane too |
| `Alt+q` | The other pane previews the file under the cursor |
| `Alt+k` | Compare with the other pane: mark what is newer or missing there |
| `Alt+y` | Synchronise both panes' trees: newer wins, shown before it runs |

## Tabs

| Keys | Does |
|---|---|
| `Ctrl+t` | New tab on the same directory |
| `Ctrl+w` | Close the tab |
| `Ctrl+PageDown` | Next tab, of the file pane or the terminal |
| `Ctrl+PageUp` | Previous tab, of the file pane or the terminal |

## Terminal

| Keys | Does |
|---|---|
| `F12` | Jump into or out of the shell |
| `Alt+t` | Show or hide the shell |
| `Alt+c` | cd the shell to this pane's directory |
| `Alt+n` | Open a new terminal tab |
