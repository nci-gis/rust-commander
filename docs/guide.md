# Guide

What rcmd can do, one topic at a time. Every key below is a default; press **F1** in
rcmd for yours, or see [keys.md](keys.md).

- [Panes and tabs](#panes-and-tabs)
- [Moving around](#moving-around)
- [Selecting](#selecting)
- [Files](#files)
- [Find, filter, compare](#find-filter-compare)
- [Archives](#archives)
- [The terminal](#the-terminal)
- [Mouse](#mouse)
- [Menus and help](#menus-and-help)

## Panes and tabs

- Two file panes to start. **Tab** moves between them.
- **Alt+S** splits a pane side by side, **Alt+D** top and bottom, **Alt+W** closes one.
- **Alt+Shift+arrows** move a border. **Alt+Z** fills the screen with one pane, and
  again puts it back.
- **Ctrl+T** opens a tab in a pane, **Ctrl+W** closes it, **Ctrl+PgUp/PgDn** switch.
- **Ctrl+U** swaps two panes. **Alt+=** shows this pane's directory in the other one.
- rcmd remembers your panes, tabs and directories, and opens them again next time.
  Start with `--no-restore`, or name directories on the command line, to start fresh.

## Moving around

- **Enter** opens a directory, or views a file. **Backspace** or **Alt+←** goes up.
- **/** jumps to a name as you type. **↑ ↓** go to the next match.
- **Ctrl+L** goes to a path you type. **Tab** completes it.
- **Alt+,** and **Alt+.** go back and forward. **Alt+↓** lists where you have been.
- **Ctrl+D** is the hotlist: your bookmarks. You can add the current directory there.
- Panes refresh by themselves when files change on disk.

## Selecting

- **Space** or **Insert** marks a file. Space on a directory also shows its size.
- **+** and **-** mark or unmark by pattern, like `*.jpg *.png`.
- **Ctrl+A** marks everything, **Alt+U** clears, **\*** inverts.
- **Ctrl+Space** shows the size of every directory in view.

## Files

| Key | Does |
|---|---|
| **F3** | view (text or hex, with search) |
| **F4** | edit in your editor (`$EDITOR`) |
| **o** | open with the system's application |
| **F5** / **F6** | copy / move to the other pane |
| **Shift+F5** | copy next to itself, under a new name |
| **F2** or **Shift+F6** | rename |
| **Shift+F2** | rename many: the names open in your editor |
| **F7** / **Shift+F4** | new directory / new file |
| **F8** / **Shift+F8** | delete to the trash / delete for good |
| **Ctrl+Shift+F5** | make a link in the other pane |
| **Alt+P** / **Alt+H** | permissions / SHA-256 checksum |

- rcmd always asks before it deletes or overwrites anything.
- Long jobs show progress. **Esc** cancels. **B** sends a job to the background.
- **Alt+Q** previews the file under the cursor in the other pane, pictures included.

## Find, filter, compare

- **Alt+F7** finds files by name, and by text inside if you like. Results open in a new
  tab. Enter goes to a file.
- **Ctrl+F** shows only the names that match. **Esc** shows everything again.
- **Alt+K** compares the two panes and marks what the other side is missing or has an
  older copy of. Then **F5** copies it.
- **Alt+Y** makes two folder trees match. It shows the plan first; nothing happens until
  you say yes.

## Archives

- **Enter** on a `.zip`, `.tar` or `.tar.gz` opens it like a folder.
- **F3** reads a file inside. **F5** extracts what you marked to the other pane.
- **Alt+F5** packs the selection. **Alt+F9** unpacks everything.
- rcmd does not change an archive in place.

## The terminal

- A shell runs under the panes. **F12** goes into it and back.
- **Alt+N** opens a new terminal tab. **Alt+S** / **Alt+D** split it.
- **Alt+C** sends the current directory to the shell (`cd`).
- **Alt+T** hides or shows the terminal. `exit` closes one.
- Want your shell to follow you when you quit rcmd? See [shell.md](shell.md).

## Mouse

- Click to select, double-click to open. **Ctrl+click** marks.
- Drag down a pane to mark a range. Drag files onto the other pane to copy or move.
- Drag a border to resize. Right-click for a menu. The wheel scrolls.

## Menus and help

- **F9** opens the menu bar: File, View, Help. You can click it too.
- **F1** lists every key and setting.
- **Shift+F1** shows what rcmd is using: memory, CPU, and whether memory keeps growing.
- **Shift+F9** runs your own commands. See [config.md](config.md#your-own-commands).
