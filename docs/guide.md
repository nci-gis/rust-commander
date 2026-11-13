# Guide

What xdevcmd can do, one topic at a time. Every key below is a default; press **F1** in
xdevcmd and type what you want to do to find yours, or see [keys.md](keys.md).

- [Panes and tabs](#panes-and-tabs)
- [Moving around](#moving-around)
- [Selecting](#selecting)
- [Files](#files)
- [Find, filter, compare](#find-filter-compare)
- [Archives](#archives)
- [The terminal](#the-terminal)
- [Mouse](#mouse)
- [Keys a terminal does not send](#keys-a-terminal-does-not-send)
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
- **A message gone from the status line**, or cut short there, is in Help → Messages
  (**Alt+M**): the last 50, each whole and with its time.
- **How long start-up took** is in Help → Usage (**Shift+F1**), step by step up to the
  first frame ("start-up 6 ms to the first frame", then config, panes, terminal check,
  shell, first frame). `--timings`
  (or `XDEVCMD_TIMINGS=1`) says it on the status line too, and prints it as one JSON line
  on stderr when xdevcmd ends. A slow "terminal check" is the terminal not answering
  what it can draw: over a slow SSH link, or a terminal that never answers, it waits up
  to 1.5 s; `[viewer] images = "none"` skips it.
- **Names sort as you read them**, in any script: `file2` before `file10`; case,
  accents and width do not split letters apart (`Éclair` with `eclair`, `ｆｉｌｅ２` with
  `file2`); katakana with hiragana and `が` with `か`; and a name copied from a Mac sorts
  where it does here, though macOS stores `é` and `が` in two pieces. Kanji keep the
  order of their code points: only a reading dictionary could do better.

## Moving around

- **Enter** opens a directory, or opens a file with its program (see below).
  **Backspace** or **Alt+←** goes up.
- **~** goes to your home directory. **Ctrl+\\** goes to the root of the drive.
- **/** jumps to a name as you type. **↑ ↓** go to the next match.
- **Ctrl+L** or **Ctrl+G** goes to a path you type. **Tab** completes it.
- **Alt+,** and **Alt+.** go back and forward. **Alt+↓** (or **Alt+J**) lists where you have been,
  and the lists kept (Alt+F7's results, paths from the shell: see [shell.md](shell.md#a-list-from-the-output)).
  **Alt+L** shows the list used last.
- **Ctrl+D** is the hotlist: your bookmarks. You can add the current directory there.
- Panes refresh by themselves when files change on disk: the system tells xdevcmd, and
  on network drives it checks now and then.
- **A slow directory never freezes the screen.** One that takes more than a moment to
  read (a network drive, a disk waking up, a huge folder) is read in the background:
  the pane keeps what it showed, a bar on its bottom edge says "opening photos… 12,400
  · 3 s", and the move happens when the listing arrives. **Esc** stops waiting and
  stays where you were. Keys keep working meanwhile.

## Remote files through a mount

xdevcmd opens no network connection itself: there is no SFTP or cloud panel. For
files on another machine, let the system (or a tool you trust) mount them, and browse
the mount like any directory:

| Linux and macOS | Windows |
|---|---|
| `sshfs host:/srv /mnt/srv` (SSHFS; add `-o reconnect`) | SSHFS-Win with WinFsp: `\\sshfs\user@host` |
| `rclone mount remote: ~/remote` (S3, Google Drive, …) | `rclone mount remote: X:` (with WinFsp) |
| SMB: `mount -t cifs` (Linux), Finder → Connect to Server (macOS) | `net use Z: \\server\share` |
| NFS: `mount -t nfs` | NFS client (Windows features) |

A mount is as fast as its network. When it is slow or the server is gone, the bar
above shows it and Esc gets you out; copies and moves run as jobs with Cancel. A copy
or move that meets a network error that may pass (a timeout, a dropped connection, a
stale handle) tries the file again by itself after 1, 2, 4, 8 and 15 seconds, the job
box counting down ("network error — trying again in 4 s (3 of 5)"), and only then asks
Retry, Skip or Stop; Esc during a wait stops at once. Onto a network mount (NFS, SMB,
sshfs, rclone, a `\\server\share` path) each new file is synced before it gets its
name, since such servers often report a failed write only then. A mount that caches
writes (rclone's `--vfs-cache-mode`) may still be uploading after the job says done:
"done" means written to the mount. A hard NFS mount whose server is gone blocks inside
the system call rather than failing: Esc stops the job once that call returns. Each
tool's own documentation says how to mount and unmount; xdevcmd's user menu (Shift+F9,
`[[menu]]` in the config) can hold those commands, typed into the terminal for you.

## Selecting

- **Space** or **Insert** marks a file. Space on a directory also shows its size.
- **+** and **-** mark or unmark by pattern, like `*.jpg *.png`.
- **Ctrl+A** marks everything, **Alt+U** clears, **\*** inverts.
- **Ctrl+Space** shows the size of every directory in view.

## Files

| Key | Does |
|---|---|
| **F3** | view (text or hex, with search) |
| **F4** | edit in your editor (`[commands] editor`, else `$VISUAL` or `$EDITOR`) |
| **Enter** | open with its program: your `[[open]]` rule, else the system's (as Total Commander does); a type that is not a known document asks first |
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

- **In the viewer** (F3): **/** searches, **n** / **N** next / previous, **h** switches
  text and hex, **w** wraps lines, **y** copies the selected lines (or the top one),
  **Y** the screen, **:** or **Ctrl+G** goes to a line (`40`, `50%`, or in hex an
  offset such as `0x1f00`), **F2** saves a copy under another name (a shell's history
  readable by you only), **q** or **Esc** closes.
- **A big file opens at once.** The viewer draws the first screens straight away and
  counts the rest of the lines in the background; the title says "… lines so far,
  indexing 40%" until it is done, and End goes to the end found so far. A search that
  does not find its text in the part counted yet says so, rather than "no more
  matches". Past 4,000,000 lines the viewer shows the first part, and says so.
- **The viewer colours source** by its language, from the theme's own colours: Rust,
  Python, JavaScript/TypeScript, JSON, TOML, YAML, Markdown, shell (and Dockerfiles,
  Makefiles), C/C++, Go, Java, Kotlin, C#, HTML/XML, CSS, SQL, INI, diffs and logs; the
  title names it. The language comes from the file's name, else its first line
  (`#!/usr/bin/env python3`). Files up to 64 MiB are coloured (quick view: 1 MiB);
  lines longer than 4 KiB are drawn plain. A jump far into a big file may show plain
  lines for a moment while the file is read through in the background. The search term
  is lit over the colours; hex and a shell's history are never coloured.
  `[viewer] highlight = false` (Settings → Files) or `NO_COLOR` turns it off.
- **Enter on a program or script** asks "Run it?" (Cancel first). **Run in a terminal
  tab** shows its output and keeps it after the program ends; Enter or Esc closes the
  tab.
- Where nothing can be opened on a screen — over SSH or without a desktop, unless
  `[commands] opener` is set, and always in read-only mode — Enter views the file
  instead and says why. `enter = "view"` under `[files]` makes Enter always view.
- **Ctrl+Z** undoes the last rename (one file or many), move, copy, Alt+Y
  synchronisation, new directory, new file or delete to the trash (F8 puts things back
  where they were), after showing what it will do. It checks the disk first: a file
  renamed since, a name taken again, or a new directory with something in it now, is
  left alone. **Undoing a copy** removes what the copy made, but only what is still as
  it made it: a copied file you changed since is kept, and so is a copied folder you put
  something into; the job lists what it kept. Files the copy replaced come back from
  the trash (below). A copy of more than 200,000 items, and Shift+F8, cannot be undone,
  and Ctrl+Z says so.
- **A file replaced goes to the trash** where the trash is on the same drive, so Ctrl+Z
  after the copy brings it back (and it can be restored from the trash by hand after a
  move): on Linux the home trash or the drive's own; on macOS any local drive; on
  Windows the system drive only (other drives may have no Recycle Bin). Elsewhere, and
  on network shares, a replace is for good, and the table before the copy marks it ⚠.
  The table says ℹ "a file replaced goes to the trash"; the job's summary says how many
  went there.
- **The job box says when it will end** ("about 4 min left"), from what it has really
  done in the last seconds, counting both bytes and files. It says nothing for the
  first three seconds, while the speed is still changing, or while nothing moves.
- **A kept list follows what you move or rename**: a row moved to another folder (F6,
  from a results tab or elsewhere), renamed (F2, Shift+F2) or moved back by Ctrl+Z
  stays in the list, under its new path, instead of being reported gone.
- xdevcmd always asks before it deletes or overwrites anything. **Shift+F8** deletes
  only on **Y** (or a click on its button), never on Enter.
- **F5** / **F6** look before they start: the dialog opens at once and checks the
  destination while you read it (files already there, newer or older, a file where a
  folder is, names that differ only in case where the drive ignores case, names or
  sizes the drive cannot take, free space, whether you can write there).
  - **Nothing to decide**: the dialog is the plain one, with **U** for what happens to
    a file that turns up during the job: ask about each (the default); replace it only
    when yours is newer and skip the same ones (like robocopy); leave them all; or
    replace them all. Files already there and identical (same size and date) are left,
    and the count line says how many.
  - **Something to decide**: the dialog becomes a short table, one line per group,
    worst first: ⛔ what cannot be done (left out, with the reason), ⚠ what would lose
    data (a newer file replaced, an older one, one of another size, a name that differs
    only in case…), ℹ what is only said (identical files). Each ⚠ group is **skip** by
    default; **↑↓** picks a group and **Space** turns it to **replace** or to **ask
    each** (the old question, file by file). The first line sums it up: create ·
    replace · skip · the space it needs and what is free.
  - **Enter** before the check has finished starts the job as before, asking as it
    goes. **Tab** checks again for the other pane.
  - A file replaced is replaced only while it is still the one the check saw; changed
    since, you are asked. At an overwrite question, **U** applies the "only if newer"
    rule to the rest of the job.
- A file being replaced stays as it was until its new copy is complete: cancelling
  part-way leaves the old one. With **Safe operations** (Settings → General, on by
  default) a new file is written the same way, and if xdevcmd is killed in the middle of
  a job, the next start cleans up and offers to continue it (what it finished is left;
  what you chose in the table still holds, without asking again; what is identical is
  skipped; you are asked about anything else already there). Closing the terminal stops
  jobs tidily and saves the session. Details in [security.md](security.md#safe-operations).
- Long jobs show progress. **Esc** cancels. **B** sends a job to the background.
- **Alt+Q** previews the file under the cursor in the other pane, pictures included.
  Pictures are sharp where the terminal can draw them itself (kitty, Ghostty, iTerm2,
  WezTerm, Windows Terminal, foot), and drawn in half blocks elsewhere (macOS
  Terminal.app, VS Code, inside tmux). `images` under `[viewer]` chooses or turns it
  off.

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
  tab. Enter goes to a file. A search stops at 100,000 results and says so: narrow the
  name or the folder to see the rest.
- **Ctrl+F** shows only the names that match, best matches first. The letters only
  need to come in order: `cfg` finds `config.toml`. `*.rs` style wildcards work too.
  **Esc** shows everything again. `filter = "plain"` under `[files]` matches the text
  as typed instead.
- **Alt+K** compares the two panes and marks what the other side is missing or has an
  older copy of. Then **F5** copies it.
- **Alt+Y** makes two folder trees match. It shows the plan first; nothing happens until
  you say yes. A big tree takes a moment to compare: the status line counts, and
  **Esc** stops it.
- What compare could not decide is shown, never copied: **⚠** a folder that could not be
  read (nothing under it is copied, either way), one that went away while compared, or
  one only partly read; **?** a name whose twin may be among what could not be read;
  **≠** two files with the same time and a different size, or two names that differ only
  in case on a drive that ignores case (macOS, Windows), where copying would replace
  one with the other. The plan's title counts the ⚠ rows; Alt+K names them on the
  status line.
- A file the plan said it would replace is replaced only if it is still that file; if
  it changed, or a file appeared where the plan saw none, you are asked.

## Archives

- **Enter** on a `.zip`, `.tar` or `.tar.gz` opens it like a folder.
- **F3** reads a file inside. **F5** extracts what you marked to the other pane.
- **Alt+F5** packs the selection. **Alt+F9** unpacks everything.
- xdevcmd does not change an archive in place.
- Unpacking refuses an archive that would not fit, or that would grow to far more than
  its own size (over 1 GB at more than 1000 times), as a "zip bomb" would.

## The terminal

- A shell runs under the panes. **F12** goes into it and back.
- Inside the shell, xdevcmd takes only **F12**, **Shift+F12** and **Alt+X**; every other
  key is the program's (readline's Alt+D, emacs' F-keys, mc's F10).
- **Alt+N** opens a new terminal tab. Inside the shell, press **Alt+X** first:
  **Alt+X Alt+S** / **Alt+X Alt+D** split it, **Alt+X Alt+W** closes it, **Alt+X Alt+1**
  goes to tab 1. Anything else after Alt+X, Enter too, goes to the program with it;
  Esc cancels.
- **Shift+F12** opens the shell's history in the viewer, at its end: read it, search it
  with `/`, drag over lines and press `y` to copy them, **Esc** to go back. It stays in
  memory; nothing of it is written to disk unless you turn on a log for that tab (below).
  **Alt+X F12** does the same where Shift+F12
  does not arrive. While a full-screen program (vim, htop) is up, it shows that
  program's screen only, and says so.
- A **right-click on a terminal tab** opens its menu: View history, Save history as…,
  Log to file… (or Stop logging), List from the last command…, Close tab, for that tab
  whichever has focus. Inside the shell a right-click is left to the program.
- **Log to file…** writes everything the tab's shell prints to a file from then on, as
  PuTTY's session log does, including what scrolls past the history Shift+F12 keeps.
  It asks where, offering `xdevcmd-<shell>-<date>-<time>.log` in the shell's directory;
  an existing file is appended to. **Tab** in that prompt switches between **text**
  (colours and other escapes removed, a progress line a CR rewrote kept once, as it
  ended; a full-screen program such as vim is one line saying it ran) and **raw** (the
  bytes as received, for `cat` or `less -R` later). The tab shows **●** while it is
  logged; **Stop logging** in the same menu ends it. Closing the tab, `exit` and
  quitting end it too, and every last line is written first. If the disk fails or
  cannot keep up, the log stops, the status line says why, and the ● becomes a red
  **!** until you stop it; it never carries on with a gap. F1 has both as rows
  ("log tab"), acting on the active tab. Off in the read-only modes. A log keeps
  everything the shell prints: see [security.md](security.md#the-shell-panel).
- **Alt+C** sends the current directory to the shell (`cd`).
- **Alt+T** hides or shows the terminal. `exit` closes one.
- The wheel scrolls back through a shell's history. A full-screen program gets the
  wheel instead: as the mouse if it asked for it (Claude Code, vim, htop), otherwise as
  arrow keys (less, man).
- Typing `xdevcmd` in the panel does not start a second one inside the first: F12 goes
  back to the one running.
- Want your shell to follow you when you quit xdevcmd? See [shell.md](shell.md).

## Mouse

- Click to select; double-click does what Enter does (into a directory, or open the
  file). **Ctrl+click** marks.
- Drag down a pane to mark a range. Drag files onto the other pane to copy or move.
- Click a directory's name in a pane's path, at the top, to go there.
- Drag a border to resize; double-click it to share it evenly between the two panes on
  either side (any others keep their size): two file panes, the file area and the
  terminal, or two shells. Right-click for a menu, or **Shift+F10** (the Menu key too,
  where the terminal reports it) for the same menu at the cursor's row. The wheel
  scrolls.
- **Alt+X, then an F key** does what Shift and that key do (Alt+X F8 is Shift+F8), for
  terminals that do not send Shift+F keys, like macOS Terminal.app. The exception is
  **Alt+X F1**, which opens F1 (Commands) from anywhere, the shell included; Usage
  (Shift+F1) is a row in F1 and in the Help menu.
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
  ‹ or › (or the value itself, for the next one), or Save; the wheel moves between
  rows. Only the lines that changed are written to `config.toml`; your comments stay.
  Rows marked *next start* apply after a restart. The Keys tab says how to rebind keys.
  In read-only and restricted mode it shows the settings but saves nothing.
- **View → Theme…** tries a theme at once.
- **F1, Commands**: every command, found by typing, with its key beside it. Type a few
  letters of a name ("cop" finds Copy, "sbn" Sort by name), a group's name ("sort"), or
  a key itself ("f5", "alt+j"); ↑/↓ choose, **Enter** runs it on the pane you were in,
  and the status line says the key for next time. **Alt+1…9** runs the first nine rows
  (digits alone are typed). With nothing typed, the commands you ran last come first,
  then those for where you are, then the rest under "elsewhere". **F1** again makes it
  the whole window, the commands by group to browse (on a small screen it stays small);
  **Esc** clears what you typed, then closes. Rows that cannot run say why: a mouse
  gesture, the viewer's own key, or a mode that turns the command off. Your
  `[[menu]]` commands are there too, under "Your commands".
- In the shell, F1 is the program's: **Alt+X F1** opens Commands there. A file pane's
  command run from the shell acts on the file pane you came from; the shell's own
  (split, new tab) act on the shell. Over the viewer, F1 lists the viewer's keys first,
  and Esc gives the viewer back.
- **Help → Config in use** shows the config file as it is in effect, read-only: whether
  it was loaded (or IGNORED, and why), the mode, and every setting, `•` on what differs
  from the default. Type to filter; a `[section]`'s name keeps the whole section.
- **Shift+F1** shows what xdevcmd is using: memory, CPU, and whether memory keeps growing.
  It is in F1 and in the Help menu too, for terminals with no Shift+F1.
- **Help → Documentation** says where the documentation for *your* version is: first
  the copy in `docs/` beside xdevcmd, when you installed from a release archive (Enter
  reads its index in the viewer), then the link to these pages at your version's tag
  (C, or Enter when there is no copy, puts it on the clipboard). xdevcmd opens no
  connection itself, so it never starts a browser. A build from source carries the
  version of the last release until the next one, so its link is that release's docs.
- **Shift+F9** runs your own commands. See [config.md](config.md#your-own-commands).
