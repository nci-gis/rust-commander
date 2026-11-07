# Guide

What xdevcmd can do, one topic at a time. Every key below is a default; press **F1** in
xdevcmd for yours, or see [keys.md](keys.md).

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
- **Alt+Shift+arrows** move a border, or **Alt+R** then arrows (Enter when done).
  **Alt+Z** fills the screen with one pane, and again puts it back.
- **Ctrl+T** opens a tab in a pane, **Ctrl+W** closes it, **Ctrl+PgUp/PgDn** switch,
  **Alt+1**…**Alt+9** go to tab N.
- **Ctrl+U** swaps two panes. **Alt+=** shows this pane's directory in the other one.
- xdevcmd remembers your panes, tabs and directories, and opens them again next time.
  Start with `--no-restore`, or name directories on the command line, to start fresh.

## Moving around

- **Enter** opens a directory, or opens a file with its program (see below).
  **Backspace** or **Alt+←** goes up.
- **~** goes to your home directory. **Ctrl+\\** goes to the root of the drive.
- **/** jumps to a name as you type. **↑ ↓** go to the next match.
- **Ctrl+L** goes to a path you type. **Tab** completes it.
- **Alt+,** and **Alt+.** go back and forward. **Alt+↓** (or **Alt+J**) lists where you have been.
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
| **Enter** | open with its program: your `[[open]]` rule, else the system's (as Total Commander) |
| **o** | open with the system's application |
| **Shift+O** | open with…: your rules for this file, the system's program, view, edit |
| **F5** / **F6** | copy / move to the other pane |
| **Shift+F5** | copy next to itself, under a new name |
| **F2** or **Shift+F6** | rename |
| **Shift+F2** | rename many with a pattern; F4 there opens the names in your editor |
| **F7** / **Shift+F4** | new directory / new file |
| **F8** / **Shift+F8** | delete to the trash / delete for good |
| **Ctrl+Shift+F5** | make a link in the other pane |
| **Alt+P** / **Alt+H** | permissions / SHA-256 checksum |

- **Enter on a program or script** asks "Run it?" (Cancel first). **Run in a terminal
  tab** shows its output and keeps it after the program ends; Enter or Esc closes the
  tab.
- Where nothing can be opened on a screen — over SSH, without a desktop, in read-only
  mode — Enter views the file instead and says why. `enter = "view"` under `[files]`
  makes Enter always view, as in xdevcmd 0.1.0.
- **Ctrl+Z** undoes the last rename (one file or many), move, new directory or new
  file, after showing what it will do. It checks the disk first: a file renamed since,
  or a new directory with something in it now, is left alone. Copies and deletes are not
  undone (your system's trash restores what F8 sent there), and Ctrl+Z says so.
- xdevcmd always asks before it deletes or overwrites anything.
- Long jobs show progress. **Esc** cancels. **B** sends a job to the background.
- **Alt+Q** previews the file under the cursor in the other pane, pictures included.

### Renaming many files

**Shift+F2** renames the selection with a pattern. Every new name shows before you press
**Enter**, and nothing is renamed if one of them is wrong.

| In Name or Extension | Becomes |
|---|---|
| `[N]` | the name, without the extension |
| `[N2-5]` · `[N3-]` | its letters 2 to 5 · from the 3rd on |
| `[E]` | the extension |
| `[P]` | the folder the file is in |
| `[C]` | a counter: 1, 2, 3… padded to the widest (`01`…`12`) |
| `[C10+5:3]` | start at 10, step 5, three digits: `010`, `015`… |
| `[[` | a `[` |

Then **Find** / **Replace with** changes text in the whole name, and **Case** makes it
lower, UPPER or Title Case. The counter follows the order the pane shows.

**F4** opens the names in your editor instead, as the pattern has them so far. Save and
close the editor; xdevcmd asks before it renames. The editor must wait until you close
it: on Windows use `code --wait` or `notepad++ -multiInst -nosession`, not plain `code`.

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
- xdevcmd does not change an archive in place.

## The terminal

- A shell runs under the panes. **F12** goes into it and back.
- **Alt+N** opens a new terminal tab. **Alt+S** / **Alt+D** split it.
- **Alt+C** sends the current directory to the shell (`cd`).
- **Alt+T** hides or shows the terminal. `exit` closes one.
- Want your shell to follow you when you quit xdevcmd? See [shell.md](shell.md).

## Mouse

- Click to select, double-click to open. **Ctrl+click** marks.
- Drag down a pane to mark a range. Drag files onto the other pane to copy or move.
- Drag a border to resize. Right-click for a menu. The wheel scrolls.
- The bar on a long list's right border scrolls too: drag it, or click where to go.
- In the viewer, drag over lines to select them; **y** copies them, **Esc** lets go.
  For part of a line, use your terminal's own selection (usually **Shift**+drag).

## Keys a terminal does not send

Some terminals keep keys for themselves, or drop Ctrl or Shift on the way. xdevcmd gives
those actions a second key that arrives everywhere: **Alt+R, then arrows** resizes
panes (as often as you like; Enter when done), **Alt+J** opens the directory history,
**Alt+1**…**Alt+9** go to a tab. `xdevcmd --show-keys` shows what your terminal sends, and
[troubleshooting.md](troubleshooting.md) lists what is known for each system.

## Menus and help

- **F9** opens the menu bar: File, View, Help. You can click it too.
- **File → Settings…** changes the common settings: ←/→ or Space to change one, Tab
  for the next tab, Enter to save. With the mouse, click a tab, a check box, a value's
  ‹ or › (or the value itself, for the next one), or Save; the wheel moves between rows. Only the lines that changed are written to
  `config.toml`; your comments stay. Rows marked *next start* apply after a restart.
- **View → Theme…** tries a theme at once.
- **F1** lists every key and setting.
- **Shift+F1** shows what xdevcmd is using: memory, CPU, and whether memory keeps growing.
- **Shift+F9** runs your own commands. See [config.md](config.md#your-own-commands).
