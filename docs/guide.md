# Guide

What xdevcmd can do, one topic at a time. Every key here is a default. Lost? Press
**F1** and type what you want to do: it finds the command and shows its key.

**In short**: the keys for a first day

| Key | Does |
|---|---|
| **Tab** | The other pane |
| **Enter** · **Backspace** | Into a directory or open a file · up |
| **Space** | Mark a file |
| **F3** · **F4** | View · edit |
| **F5** · **F6** | Copy · move to the other pane |
| **F7** · **F8** | New directory · delete to the trash |
| **Ctrl+Z** | Undo the last copy, move, rename or delete to the trash |
| **F12** | Into the shell panel and back |
| **F1** | Find any command |
| **F10** | Quit |

**Contents**

- [Panes and tabs](#panes-and-tabs)
- [Moving around](#moving-around)
- [Remote files through a mount](#remote-files-through-a-mount)
- [Selecting](#selecting)
- [Files](#files)
- [Find, filter, compare](#find-filter-compare)
- [Archives](#archives)
- [The terminal](#the-terminal)
- [Mouse](#mouse)
- [Keys a terminal does not send](#keys-a-terminal-does-not-send)
- [Menus and help](#menus-and-help)

Every key: [keys.md](keys.md).

## Panes and tabs

| Key | Does |
|---|---|
| **Tab** | Move between panes (two to start) |
| **Alt+S** · **Alt+D** · **Alt+W** | Split side by side · top and bottom · close a pane |
| **Alt+Shift+arrows**, or **Alt+R** then arrows | Move a border (Enter when done) |
| **Alt+Z** | One pane fills the screen; again puts it back |
| **Ctrl+T** · **Ctrl+W** | Open · close a tab |
| **Ctrl+PgUp/PgDn** · **Alt+1**…**Alt+9** | Switch tabs · go to tab N |
| **Ctrl+U** | Swap two panes |
| **Alt+=** | Show this pane's directory in the other |
| **Alt+M** | Messages: the last 50 from the status line, whole, with their time |

- **Your panes, tabs and directories come back** next time. Start with `--no-restore`,
  or name directories on the command line, to start fresh.
- **Names sort as you read them**, in any script: `file2` before `file10`; case, accents
  and width do not split letters apart (`Éclair` with `eclair`, `ｆｉｌｅ２` with
  `file2`); katakana with hiragana, `が` with `か`; a name copied from a Mac sorts where
  it does here. Kanji keep code-point order: only a reading dictionary could do better.
- **Start-up time** is in Help → Usage (**Shift+F1**), step by step to the first frame
  (config, panes, terminal check, shell). `--timings` (or `XDEVCMD_TIMINGS=1`) also
  shows it on the status line and prints one JSON line on stderr at the end. A slow
  "terminal check" is the terminal not answering what it can draw: it waits up to
  1.5 s (slow SSH, a terminal that never answers); `[viewer] images = "none"` skips it.

## Moving around

| Key | Does |
|---|---|
| **Enter** | Into a directory, or open a file with its program ([below](#opening-a-file)) |
| **Backspace** or **Alt+←** | Up |
| **~** · **Ctrl+\\** | Home · the root of the drive |
| **/** | Jump to a name as you type; **↑ ↓** for the next match |
| **Ctrl+L** or **Ctrl+G** | Go to a path you type; **Tab** completes |
| **Alt+,** · **Alt+.** | Back · forward |
| **Alt+↓** (or **Alt+J**) | Where you have been, and the lists kept (Alt+F7's results, [paths from the shell](shell.md#a-list-from-the-output)) |
| **Alt+L** | The list used last |
| **Ctrl+D** | The hotlist: your bookmarks; add the current directory there |

- **Panes refresh by themselves** when files change: the system tells xdevcmd, and on
  network drives it checks now and then.
- **A slow directory never freezes the screen.** One that takes a moment (a network
  drive, a disk waking up, a huge folder) is read in the background: the pane keeps
  what it showed, a bar on its bottom edge says "opening photos… 12,400 · 3 s", and you
  move when the listing arrives. **Esc** stays where you were. Keys keep working.

## Remote files through a mount

xdevcmd opens no network connection itself: no SFTP or cloud panel. Let the system (or
a tool you trust) mount the files, and browse the mount like any directory:

| Linux and macOS | Windows |
|---|---|
| `sshfs host:/srv /mnt/srv` (SSHFS; add `-o reconnect`) | SSHFS-Win with WinFsp: `\\sshfs\user@host` |
| `rclone mount remote: ~/remote` (S3, Google Drive, …) | `rclone mount remote: X:` (with WinFsp) |
| SMB: `mount -t cifs` (Linux), Finder → Connect to Server (macOS) | `net use Z: \\server\share` |
| NFS: `mount -t nfs` | NFS client (Windows features) |

Your own commands (Shift+F9, `[[menu]]` in the config) can hold the mount commands.

- **Slow or gone**: the bar above shows it and Esc gets you out; copies and moves run
  as jobs with Cancel.
- **A network error that may pass** (timeout, dropped connection, stale handle): the
  file is tried again after 1, 2, 4, 8 and 15 seconds ("network error — trying again in
  4 s (3 of 5)"), then you get Retry, Skip or Stop. Esc during a wait stops at once.
- **Each new file is synced before it gets its name** on a network mount (NFS, SMB,
  sshfs, rclone, `\\server\share`), since such servers often report a failed write only
  then.
- **"Done" means written to the mount**: one that caches writes (rclone's
  `--vfs-cache-mode`) may still be uploading.
- **A hard NFS mount whose server is gone** blocks inside the system call: Esc stops
  the job once that call returns.

## Selecting

| Key | Does |
|---|---|
| **Space** or **Insert** | Mark a file; on a directory, also show its size |
| **+** · **-** | Mark · unmark by pattern, like `*.jpg *.png` |
| **Ctrl+A** · **Alt+U** · **\*** | Mark all · clear · invert |
| **Ctrl+Space** | Size of every directory in view |

## Files

| Key | Does |
|---|---|
| **F3** | View (text or hex, with search) |
| **F4** | Edit in your editor (`[commands] editor`, else `$VISUAL` or `$EDITOR`) |
| **Enter** | Open with its program |
| **o** | Open with the system's application |
| **Shift+O** | Open with…: your rules for this file, the system's program, view, edit |
| **Alt+Q** | Quick view in the other pane, pictures included |
| **F5** / **F6** | Copy / move to the other pane |
| **Shift+F5** | Copy next to itself, under a new name |
| **F2** or **Shift+F6** | Rename |
| **Shift+F2** | Rename many with a pattern |
| **F7** / **Shift+F4** | New directory / new file |
| **F8** / **Shift+F8** | Delete to the trash / delete for good (only on **Y**, never Enter) |
| **Ctrl+Shift+F5** | Make a link in the other pane |
| **Alt+P** / **Alt+H** | Permissions / SHA-256 checksum |
| **Ctrl+Z** | Undo |

xdevcmd always asks before it deletes or overwrites anything.

### The viewer (F3)

| Key | Does |
|---|---|
| **/** · **n** · **N** | Search · next · previous |
| **h** · **w** | Text or hex · wrap lines |
| **y** · **Y** | Copy the selected lines (or the top one) · the screen |
| **:** or **Ctrl+G** | Go to a line: `40`, `50%`, or in hex an offset such as `0x1f00` |
| **F2** | Save a copy under another name (a shell's history: readable by you only) |
| **q** or **Esc** | Close |

- **A big file opens at once.** The rest is counted in the background ("… lines so far,
  indexing 40%"); End goes to the end found so far, and a search that has not reached
  the uncounted part says so. Past 4,000,000 lines the viewer shows the first part, and
  says so.
- **Source is coloured** by language, from the theme's colours: Rust, Python,
  JavaScript/TypeScript, JSON, TOML, YAML, Markdown, shell (and Dockerfiles, Makefiles),
  C/C++, Go, Java, Kotlin, C#, HTML/XML, CSS, SQL, INI, diffs and logs; the title names
  it. The language comes from the name, else the first line (`#!/usr/bin/env python3`).
  Up to 64 MiB (quick view: 1 MiB); lines over 4 KiB are plain; a far jump may show
  plain lines for a moment. The search term is lit over the colours; hex and a shell's
  history are never coloured. Off: `[viewer] highlight = false` (Settings → Files) or
  `NO_COLOR`.
- **Quick view's pictures** (Alt+Q) are sharp where the terminal draws them (kitty,
  Ghostty, iTerm2, WezTerm, Windows Terminal, foot), half blocks elsewhere (macOS
  Terminal.app, VS Code, inside tmux). `[viewer] images` chooses or turns it off.

### Opening a file

- **Enter** uses your `[[open]]` rule, else the system's program, as Total Commander
  does ([config.md](config.md#opening-files)). A type that is not a known document asks
  first.
- **A program or script** asks "Run it?" (Cancel first). **Run in a terminal tab** shows
  its output and keeps it after the program ends; Enter or Esc closes the tab.
- **Nowhere to open it** (over SSH or without a desktop, unless `[commands] opener` is
  set, and always in read-only mode): Enter views the file and says why.
  `[files] enter = "view"` makes Enter always view.

### Copy and move (F5, F6)

**The dialog checks before anything starts.** It opens at once and checks the
destination while you read it: files already there (newer, older), a file where a
folder is (skipped: never a file over a folder, or the reverse), names that differ only
in case where the drive ignores case, names or sizes the drive cannot take, free space,
whether you can write there.

- **Nothing to decide**: the plain dialog. **U** chooses what happens to a file that
  turns up during the job: ask each (default); replace only when yours is newer and
  skip the same (like robocopy); leave all; replace all. Identical files (same size and
  date) are left, and counted.
- **Something to decide**: a short table, one line per group, worst first: ⛔ cannot be
  done (left out, with the reason), ⚠ would lose data (a newer file replaced, an older
  one, another size, a case clash…), ℹ only said (identical files). Each ⚠ group is
  **skip** by default; **↑↓** picks a group, **Space** turns it to **replace** or **ask
  each**. The first line sums up: create · replace · skip · space needed and free.
- **Enter** before the check ends starts the job as before, asking as it goes. **Tab**
  checks again for the other pane.
- A file is replaced only while it is still the one the check saw; changed since, you
  are asked. At an overwrite question, **U** applies "only if newer" to the rest.

**While it runs**: progress, with **Esc** to cancel and **B** to send the job to the
background. The job box says when it will end ("about 4 min left"), from the last
seconds' bytes and files; nothing for the first three seconds or while nothing moves.

**Nothing is half-written.** A file being replaced stays as it was until its new copy
is complete: cancelling leaves the old one. With **Safe operations** (Settings →
General, on by default), if xdevcmd is killed mid-job, the next start cleans up and
offers to continue: what was finished is left, your table choices hold, identical files
are skipped, anything else there is asked about. When a copy may be damaged (a power
cut, a drive pulled out), **C** in that dialog re-reads every copied file against its
source (slow) and copies again what differs; **F** forgets the job.
Closing the terminal stops jobs tidily and saves the session. Details:
[security.md](security.md#safe-operations).

**A file replaced goes to the trash** where the trash is on the same drive, so Ctrl+Z
brings it back (or restore it from the trash by hand after a move): on Linux the home
trash or the drive's own; on macOS any local drive; on Windows the system drive only.
Elsewhere, and on network shares, a replace is for good, and the table marks it ⚠. The
table says ℹ "a file replaced goes to the trash"; the summary says how many went there.

### Undo (Ctrl+Z)

Undoes the last rename (one or many), move, copy, Alt+Y sync, new directory, new file
or delete to the trash (F8 puts things back), after showing what it will do.

- It checks the disk first: a file renamed since, a name taken again, or a new
  directory with something in it now, is left alone.
- **Undoing a copy** removes only what is still as the copy made it: a file you changed
  since, or a folder you put something into, is kept, and the job lists it. Files the
  copy replaced come back from the trash.
- **Not undoable**: a copy of more than 200,000 items, and Shift+F8. Ctrl+Z says so.
- **Kept lists follow your changes**: a row moved (F6), renamed (F2, Shift+F2) or moved
  back by Ctrl+Z stays in the list under its new path.

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
lower, UPPER or Title Case. The counter follows the pane's order.

**F4** opens the names in your editor instead, as the pattern has them so far. Save and
close; xdevcmd asks before it renames. The editor must wait until you close it: on
Windows use `code --wait` or `notepad++ -multiInst -nosession`, not plain `code`.

### What a copy keeps

A copy (F5, and F6 to another drive, which copies then removes) makes new files with
your source's contents. A move on the same drive (F6) is a rename: everything stays.

| | Kept | Not kept |
|---|---|---|
| Contents | always | |
| Modification date | files | folders (they change as files go in) |
| Permissions | Unix mode bits (the executable bit too); the read-only flag on Windows | Windows' hidden, system and archive attributes |
| Owner and group | | the copy is yours, as with `cp` |
| Access and creation dates | | set to now |
| Extended attributes, ACLs, macOS tags and resource forks, NTFS streams | | not copied (a clone on APFS may carry extended attributes with it) |
| Symbolic links | copied as links (Linux, macOS) | on Windows they are not copied; the job says so |
| Hard links | | each name becomes a file of its own |
| Sparse files | | written out to their full size |

### What xdevcmd doesn't do, and what to use

xdevcmd copies what you can see and check in two panes. For the rest, use a tool built
for it; your own commands (Shift+F9, `[[menu]]` in the [config](config.md)) can hold it.

| You need | Use |
|---|---|
| A copy that keeps everything: owner, ACLs, extended attributes, hard links, sparse files | `rsync -aHAXS` (Linux, macOS), `robocopy /COPYALL /SL` (Windows) |
| To copy to or from another machine, or a cloud | `rclone`, `rsync` over ssh |
| To know two trees hold the same contents, not only the same sizes and dates | `rclone check`, `rsync -c --dry-run`, `diff -r` |
| A backup with versions | restic, borg, Time Machine, File History |
| To mirror a tree, deleting what the source lacks | `rsync --delete`, `robocopy /MIR` |

### File names

- **Kept exactly**: as the file system stored it (any bytes on Linux, even not UTF-8;
  UTF-16 on Windows). Copy, rename and delete act on the very file listed.
- **Shown safely**: invalid UTF-8 shows `�`; control characters (possible on Linux) are
  left out when drawn, so a name can never send the terminal a command.
- **Compared as stored**: compare and sync match names by their exact bytes. `Read.me`
  and `read.me` on a drive that ignores case show as a case clash. A name composed on
  one side and decomposed on the other (a folder copied from macOS) may show as two.
- **New names** (rename, new directory, new file, copy as) may not hold `/` or a control
  character; on Windows not `< > : " | ? * \` either, nor end with a dot or a space.

## Find, filter, compare

| Key | Does |
|---|---|
| **Alt+F7** | Find by name, and by text inside if you like. Results open in a new tab; Enter goes to a file. Stops at 100,000 results and says so |
| **Ctrl+F** | Show only matching names, best first. Letters only need to come in order: `cfg` finds `config.toml`; `*.rs` works too. **Esc** shows all. `[files] filter = "plain"` matches as typed |
| **Alt+K** | Compare the two panes: marks what the other side lacks or has older. Then **F5** copies it |
| **Alt+Y** | Make two trees match. Shows the plan first; nothing happens until you say yes. A big tree counts on the status line; **Esc** stops |

What compare could not decide is shown, never copied:

| Mark | Means |
|---|---|
| **⚠** | A folder that could not be read (nothing under it is copied, either way), went away while compared, or was only partly read. The plan's title counts them; Alt+K names them |
| **?** | A name whose twin may be among what could not be read |
| **≠** | Same time, different size; or names that differ only in case on a drive that ignores case (macOS, Windows), where copying would replace one with the other |

A file the plan would replace is replaced only if it is still that file; changed, or a
file where the plan saw none, and you are asked.

## Archives

- **Enter** on a `.zip`, `.tar` or `.tar.gz` opens it like a folder. **F3** reads a file
  inside; **F5** extracts what you marked to the other pane.
- **Alt+F5** packs the selection. **Alt+F9** unpacks everything.
- An archive is never changed in place.
- Unpacking refuses an archive that would not fit, or would grow far beyond its size
  (over 1 GB at more than 1000 times), as a "zip bomb" would.

## The terminal

A shell runs under the panes. Everything about it, including staying in xdevcmd's
directory when you quit: [shell.md](shell.md).

| Key | Does |
|---|---|
| **F12** | Into the shell and back. Inside, xdevcmd takes only **F12**, **Shift+F12** and **Alt+X**; every other key is the program's (readline's Alt+D, emacs' F keys, mc's F10) |
| **Alt+N** | New terminal tab |
| **Alt+X**, then a key | In the shell: **Alt+S** / **Alt+D** split, **Alt+W** close, **Alt+1** tab 1. Anything else after Alt+X, Enter too, goes to the program; Esc cancels |
| **Shift+F12** (or **Alt+X F12**) | The shell's history in the viewer, at its end: search with `/`, drag and `y` to copy, **Esc** back. While a full-screen program is up, its screen only |
| **Alt+C** | `cd` the shell to this directory (a new shell if you typed something not yet run) |
| **Alt+T** | Hide or show the terminal; `exit` closes one |
| **Ctrl+O** | Says the shell is F12 (mc's shell key). In the shell the F-key bar is hidden |

- **A right-click on a terminal tab**: View history, Save history as…, Log to file… (or
  Stop logging), List from the last command…, Close tab. Inside the shell a right-click
  is the program's.
- **Log to file…** writes everything the tab prints to a file, as PuTTY's session log
  does, text or raw; the tab shows **●**. F1 has both as rows ("log tab"). A log keeps
  everything the shell prints, secrets too: [shell.md](shell.md#logging-a-tab-to-a-file),
  [security.md](security.md#the-shell-panel).
- **The wheel** scrolls back through the history; a full-screen program gets it as the
  mouse if it asked (Claude Code, vim, htop), else as arrow keys (less, man).
- **Typing `xdevcmd` in the panel** does not start a second one: F12 goes back to the
  one running.

## Mouse

- **Click** selects; **double-click** does what Enter does. **Ctrl+click** marks.
- **Drag** down a pane to mark a range; drag files onto the other pane to copy or move.
- **Click a directory** in a pane's path, at the top, to go there.
- **Drag a border** to resize; double-click it to share the space evenly between its two
  sides (file panes, the file area and the terminal, or two shells).
- **Right-click** for a menu, or **Shift+F10** (the Menu key too, where the terminal
  reports it) for the same menu at the cursor's row.
- **The wheel** scrolls; so does the bar on a long list's right border (drag it, or
  click where to go).
- **In the viewer**, drag over lines to select; **y** copies, **Esc** lets go. For part
  of a line, use your terminal's own selection (usually **Shift**+drag).

## Keys a terminal does not send

Some terminals keep keys for themselves, or drop Ctrl or Shift. xdevcmd gives those
actions a second key that arrives everywhere:

| Instead of | Press |
|---|---|
| Alt+Shift+arrows (resize) | **Alt+R**, then arrows, as often as you like; Enter when done |
| Alt+↓ (directory history) | **Alt+J** |
| Ctrl+PgUp/PgDn (tabs) | **Alt+1**…**Alt+9** |
| Shift+F keys (macOS Terminal.app) | **Alt+X**, then the F key: Alt+X F8 is Shift+F8. Except **Alt+X F1**, which opens Commands from anywhere, the shell included |

`xdevcmd --show-keys` shows what your terminal sends;
[troubleshooting.md](troubleshooting.md) lists what is known for each system.

## Menus and help

| Where | What |
|---|---|
| **F1**, Commands | Every command, found by typing, with its key |
| **F9** | The menu bar: File, View, Help (clickable) |
| **File → Settings…** | The common settings |
| **View → Theme…** | Try a theme at once |
| **Help → Config in use** | The config as in effect |
| **Shift+F1**, Usage | Memory, CPU, and whether memory keeps growing (also in F1 and Help) |
| **Help → Documentation** | Where the docs for your version are |
| **Shift+F9** | Your own commands ([config.md](config.md#your-own-commands)) |

**F1, Commands.** Type a few letters of a name ("cop" finds Copy, "sbn" Sort by name), a
group ("sort") or a key ("f5", "alt+j"). ↑/↓ choose; **Enter** runs it on the pane you
were in, and the status line says its key for next time. **Alt+1…9** runs the first
nine rows (digits alone are typed).

- With nothing typed: the commands you ran last, then those for where you are, then the
  rest under "elsewhere". Your `[[menu]]` commands are there, under "Your commands".
- **F1** again makes it the whole window, by group, to browse (on a small screen it
  stays small). **Esc** clears what you typed, then closes.
- Rows that cannot run say why: a mouse gesture, the viewer's own key, or a mode that
  turns the command off.
- In the shell F1 is the program's: **Alt+X F1** opens Commands there. A pane's command
  run from the shell acts on the pane you came from; the shell's own (split, new tab)
  act on the shell. Over the viewer, F1 lists the viewer's keys first, and Esc gives the
  viewer back.

**File → Settings…**: ←/→ or Space to change a value, Tab for the next tab, Enter to
save. With the mouse: click a tab, a check box, a value's ‹ or › (or the value, for the
next), or Save; the wheel moves between rows. Only changed lines are written to
`config.toml`; your comments stay. Rows marked *next start* apply after a restart. The
Keys tab says how to rebind keys. In read-only and restricted mode it saves nothing.

**Help → Config in use**, read-only: whether the file was loaded (or IGNORED, and why),
the mode, and every setting, with `•` on what differs from the default. Type to filter;
a `[section]`'s name keeps the whole section.

**Help → Documentation**: first the copy in `docs/` beside xdevcmd, if you installed from
a release archive (Enter reads its index in the viewer), then the link to these pages at
your version's tag (C, or Enter when there is no copy, copies it). xdevcmd opens no
connection, so it never starts a browser. A build from source links to the last
release's docs until the next one.
