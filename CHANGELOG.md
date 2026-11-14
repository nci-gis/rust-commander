# Changelog

All notable changes to xdevcmd. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

<!-- Write under the newest heading. `cargo release` puts the version and the date into
that heading and starts a new, empty one here. -->
<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added
- **`o` and Open with… say what happened**: "opening …" on the status line if the
  system's opener has not answered after 300 ms (a quick open shows nothing in between),
  then "opened …" when it hands the file over, or its error. An opener that waits for the
  program it started (some desktops' xdg-open) counts as handed over after 5 s; a failure
  after that is still said.
- **Messages (Alt+M, Help → Messages, and in F1)**: the status line's last 50 messages,
  each whole and with its time, in the viewer. The status line stays one row.

- **Copy relative path** (the right-click menu, and Copy path too, now in F1): the
  selected paths from the repository's root when the folder is in one (a `.git` above
  it), else from the folder itself; the status line says which ("copied src/main.rs,
  relative to the repository proj").
- **A debug log to attach to a report**: `XDEVCMD_DEBUG=all` writes what each job did
  and how long each part took, and each error's system code and retry, to a
  `debug-….jsonl` file in the state directory (or `XDEVCMD_DEBUG_DIR`). It holds no file,
  folder, user or host name; the status line says where it is. A crash log ends with its
  last lines. See Troubleshooting.
- **A config file with a mistake is asked about at start** instead of only named on
  the status line: continue with the defaults (Enter), edit it (E; checked again when
  the editor closes) or quit (Q).

### Changed
- **F1's own row, chosen inside F1, switches between the small and the large window**
  instead of closing and opening F1 again.
- **Replacing many small files is many times faster**: replaced files of up to 1 MiB
  on a local drive are synced together, in batches, on several threads, with one
  journal sync a batch. Each is still renamed only once its data and its journal line
  are on disk. 5,000 files: 2.9 s instead of 59.7 s (Linux). A job past 1,000 files
  tries 2, 8, 16 and 32 threads on its next batches and keeps the fastest.
  `cargo xtask sync-sweep` measures a drive.
- **Copies use the system's fast copy where the drive allows it** (Linux and macOS): a
  clone on Btrfs, XFS and APFS (instant, no space used until a file changes), else on
  Linux the kernel's `copy_file_range` (server-side on NFS 4.2 and SMB; on ext4 about
  10–15% faster). Anything refused falls back to xdevcmd's own read and write. The
  status line says which ran ("5 copied (by the kernel)"). `[general] fast_copy = false`
  turns it off.
- **F3 opens a big file at once**: the first 4 MiB are indexed before the first frame
  and the rest in the background, the title counting ("… lines so far, indexing 40%");
  Esc closes it at any point. A 1 GiB log: 1.5 ms to open instead of the 120 ms its
  whole index takes (warm cache; from a cold disk or a share the wait was far longer).
- **Find stops at 100,000 results** and says so, and takes them in a bounded share per
  frame: a search of a whole disk no longer grows memory without end or slows every key.

### Fixed
- **F5 and F6 check faster on Windows the first time**: inside a folder, what a file is
  (size, times) is taken from the folder's listing instead of opening each file, which
  Windows Defender scanned on first open (1,500 documents of 500 MB took 1–1.5 s; as
  many small files a few ms). Links and other reparse points are still opened.
- **`~` on Windows when `HOME` is not a Windows path** (left by Cygwin or an old MSYS,
  `/home/ann`): it goes to `USERPROFILE` instead of saying there is no home directory.

## [0.4.0-beta.1] - 2026-11-13

### Added
- **How long start-up took**, step by step up to the first frame, in Help → Usage
  (Shift+F1);
  `--timings` (or `XDEVCMD_TIMINGS=1`) also says it on the status line and prints it as
  one JSON line on stderr at the end.
- **Ctrl+Z undoes a copy** (Round 031): it removes what the copy (F5, Alt+Y) made, only
  while each file is still as the copy made it; a file changed since, or a folder holding
  something else, is kept and listed. Up to 200,000 items, run as a job.
- **A replaced file goes to the trash** where the trash is on the same drive (Linux: the
  home trash or the drive's own; macOS: local drives; Windows: the system drive), so
  undoing the copy brings it back. A kill between the two steps is finished at the next
  start. Elsewhere a replace is for good, and the table marks it ⚠.
- **Network errors are tried again before asking**: a timeout, a dropped connection or a
  stale handle retries the file after 1, 2, 4, 8 and 15 s, counting down in the job box;
  then Retry, Skip or Stop. Esc during a wait stops at once. New files on a network mount
  (NFS, SMB, sshfs, rclone, UNC) are synced before their rename, so a late write error
  is not lost.
- **The job box says how long is left** ("about 4 min left"), from what it has done,
  counting bytes and files, once the speed has settled.
- **Kept lists follow a move or a rename**: rows moved elsewhere, renamed, or moved back
  by Ctrl+Z keep their place in the list under their new path.
- **F5 and F6 look before they start** (Round 030, the Smart Planner's first part). The
  dialog opens at once and checks the destination meanwhile: files already there (newer,
  older, another size, identical), a file where a folder is and the reverse, names
  that differ only in case where the drive ignores case, names a FAT, exFAT or NTFS
  drive (or Windows) cannot hold, files of 4 GiB or more onto FAT, free space, and
  whether you can write there. With nothing to decide the dialog is the one it always
  was (identical files are left, and it says how many). Otherwise it becomes a table,
  one line per group, worst first: ⛔ what cannot be done, with why; ⚠ what would lose
  data, each **skip** by default, **↑↓** and **Space** to make it **replace** or **ask
  each**; ℹ what is only said. A summary line counts what is created, replaced and
  skipped and the space needed. Enter before the check ends starts as before; Tab
  checks again for the other pane. The job walks its files once, not twice.
- **Continuing an interrupted copy or move keeps what you chose**: the plan's decisions
  and each finished source are in its journal, so Resume leaves what you said to leave
  and replaces what you said to replace, without asking again.
- **Data safety, the first part of 0.4.0's** (Round 029). A file replaced is synced to
  the disk before it is renamed over the old one; a move across drives removes each
  source only once its copy, the copy's folder and the journal's line are on disk (a
  batch at a time); safe operations' journal is synced at the start of a job and before
  each step that cannot be undone. With safe operations off too. See
  [docs/security.md](docs/security.md#safe-operations) for what survives a kill and a
  power cut.
- **A journal that cannot be written is asked about**: before the job starts, or before
  the next step that cannot be undone. Stop (the default) or continue without it. Rename
  many renames nothing without its journal, and offers "rename anyway".
- **A file is replaced only if it is still the one you agreed to.** Right before the
  rename, xdevcmd looks again; a file another program changed or replaced meanwhile is
  asked about again. Alt+Y's plan remembers what it saw at each file it replaces, and
  asks about a file that appeared where it saw none.
- **Compare shows what it could not read.** A folder that cannot be read is a ⚠ row,
  never "only on the other side", and nothing under it is copied; names that differ only
  in case on a drive that ignores case are a clash, left alone. Names that are not UTF-8
  are compared as they are.
- **The viewer colours source code.** F3 and quick view colour Rust, Python,
  JavaScript/TypeScript, JSON, TOML, YAML, Markdown, shell, C/C++, Go, Java, Kotlin, C#,
  HTML/XML, CSS, SQL, INI, diffs and logs, chosen by the file's name or its first line,
  in the theme's own colours (every theme, yours included, with no new keys). A big
  file opens as fast as before: only the lines on screen are coloured, from states a
  background pass keeps every 128 lines, so a jump to line 1,000,000 never reads the
  file on the spot. Up to 64 MiB in F3 and 1 MiB in quick view; lines over 4 KiB plain.
  `[viewer] highlight = false` or `NO_COLOR` turns it off. Our own small lexer, not a
  library: about 90 KB, where syntect would have taken the binary past its 7 MB budget.
- **Log a terminal tab to a file.** A terminal tab's right-click menu has **Log to
  file…**: everything that tab's shell prints goes to a file from then on, including
  what scrolls past the history. **text** (the default) removes escapes and keeps a
  line a CR rewrote once, as it ended; **raw** (Tab in the prompt) keeps the bytes. The
  file is appended to and, when new, readable by you only. The tab shows ● while it is
  logged; **Stop logging**, closing the tab, `exit` and quitting end it after a last
  flush. A disk that fails or cannot keep up stops the log, says so on the status line
  and turns the ● into a red !: never a silent gap. F1 has both ("log tab"). Off in
  the read-only modes; nothing is logged unless you ask.
- **F1 is Commands: every command, found by typing, with its key.** Type a few letters
  of a name, a group's name or a key ("f5"), and Enter runs it on the pane you were in;
  the status line then says the key for next time. Alt+1…9 run the first nine rows.
  With nothing typed, the commands you ran last (kept across restarts, 8 of them) come
  first, then those for where you are. F1 again makes it the whole window, the
  commands by group as F1 was before. Your `[[menu]]` commands, Settings…, Theme… and
  About are rows too; mouse gestures and the viewer's keys are listed to read. Over the
  viewer, F1 lists the viewer's keys and Esc gives it back.
- **Help → Config in use**: the config in effect in its own read-only window, with a
  filter; what F1's Settings tab showed, `[keys]` included.
- **Help → Documentation** (and a row in F1): the docs for the version you have. The
  release archives now carry `docs/` beside the program, and the window names that copy
  first (Enter reads it); then the link to `docs/` at your version's tag, not `main`,
  which C copies. Nothing is opened: no network. `docs/README.md` gives the reading
  order. F1's link to troubleshooting follows the version too.
- **A slow directory no longer freezes the screen.** A directory that takes more than
  150 ms to read (a network mount, a sleeping disk, a huge folder) is read in the
  background: the pane keeps what it showed, a bar on its bottom edge names what is
  being opened and counts the entries, the move happens when the listing arrives, and
  Esc stops waiting. The periodic check for changes reads a directory once, not up to
  three times, and never puts the bar up. Alt+F7 and a list being checked show the bar
  too.
- **docs/guide.md: remote files through a mount** (sshfs, rclone, SMB, NFS), since
  xdevcmd itself opens no network connection.
- **A list from the shell's output.** In the history (Shift+F12), `p` makes a list of
  the paths in the lines dragged over, or in the last command's output when the shell
  sends prompt marks; it opens in a file pane, where F3, F4, F5, F6, F8, marking and
  renaming work on the real files. `P` adds the paths to a list instead (a menu asks
  which). A terminal tab's menu has "List from the last command…".
- **Lists are kept for the session** (the last 10, in memory only): Back returns to a
  list after Enter on a row, Alt+L shows the list used last from anywhere (Alt+X Alt+L
  in the shell), and Alt+Down/Alt+J lists them above the directories. Alt+F7's results
  are kept the same way. Coming back checks every row again and names what was deleted
  or moved meanwhile.
- **Paths in the shell's output can be opened.** In the history (Shift+F12), paths
  that exist are underlined: compiler and grep locations (`src/main.rs:42:9`), Python
  tracebacks, MSBuild's `x.cs(12,5)`, `ls` listings. Tab and Shift+Tab choose one, and
  the line under the viewer gives its full path; Enter (or a click) takes the file pane
  there, F3 views the file at its line, F4 edits it at its line (vim, nano, emacs,
  helix, VS Code, Sublime, Zed, Notepad++ and others told the line their own way). A
  relative path is taken from the directory its command ran in when the shell sends
  prompt marks, else from the shell's directory now, and it says which.
- **Two more looks in xdevtheme's family**, in `themes/`: **xdevtheme-neon** (the light
  runs teal, violet, pink, and the focused frame glows into the cells around it) and
  **xdevtheme-glass** (no lines: panes are cards, the focused path and the cursor row
  are pills).
- **xdevtheme-light**, in `themes/`: xdevtheme for a terminal with a light background,
  such as macOS Terminal's default profile. Every name, title and key reads at 4.5:1 or
  more on white, on the bars and on the cursor row, at true colour and at 256 colours;
  the frames at 3:1. Pick it in View → Theme…; it is not the default.
- **`--print-theme` prints a theme file too** (`xdevcmd --print-theme xdevtheme-light`),
  every colour and its style written out, to start your own from it.
- **A theme file can say how it draws**, in a `[style]` table: `borders` (plain,
  rounded, thick, double, none), `titles` (line, notch, pill), `gradient` (one or two
  colours after the accent), `gradient_runs` (across, down), `cursor` (block, glow,
  pill), `marker`, `keys` (plain, caps, slant), `shadow` (none, dim, glow), `glow`, `sky`
  and `stars`. Off unless a theme sets it, and off on a 16-colour terminal. No Nerd Font
  needed, and the programs in the terminal panel keep their own colours and lines.
  `--print-theme` writes it too.
- **`XDEVCMD_THEME`** overrides `[theme] name` for one run, to try a theme without
  editing the config.
- **`cargo binstall xdevcmd`** installs the ready-made build from the GitHub release,
  with no compiling: the crate says where each target's archive is.
- **A long name is shown whole on the cursor's row.** When the Name column cuts a name,
  the row under the cursor shows all of it, extension included, over Ext, Size and
  Modified (the status line still gives that file's size). The other rows keep their
  columns.
- **The menu bar shows which xdevcmd this is**, at its right in the dim colour:
  `xdevcmd 18144`, its process number, which Help → Usage also shows. Hidden where the
  menus need the room.

### Changed
- **F1 opens Commands, not a screen of keys and settings**; the settings are in Help →
  Config in use, and the Help menu is Commands… · Config in use · Usage · About.
- **Alt+X F1 opens F1**, from the shell too (where F1 alone stays the program's). It was
  Usage's second key; Usage keeps Shift+F1 and is a row in F1 and the Help menu.
  `[keys] usage = ["Shift+F1", "Alt+x F1"]` brings the old key back in the file panes;
  in the shell Alt+X F1 then goes to the program, and Commands is F12, then F1.
- **F1 always opens Commands**: a config whose `[keys] help` leaves F1 out, or that gives
  F1 to another action, is refused, saying so.
- **Names sort as people read them, in any script.** Case, accents and width no longer
  split letters (`Éclair` with `eclair`, `Ölfeld` with `öl`, full-width `ｆｉｌｅ２` with
  `file2`, its `２` a number); katakana sort with hiragana and voiced kana with plain
  ones; Vietnamese letters with marks sort with their letter, `đ` after `d`; a name a
  Mac stored decomposed sorts with its composed form. ASCII names sort as before.
- **The default theme is xdevtheme, xdevcmd's own look**: its teal turns into a light
  that runs across the screen to violet, with rounded frames and the titles in a notch,
  a lit cursor row with a bar on the frame, the F keys and the selected tab as slanted
  segments, and a shadow under dialogs. The plain look is still there as `dark`:
  `[theme] name = "dark"` keeps it, and a config that already names a theme keeps that
  one. On a 16-colour terminal the default is the 16-colour theme, as before.
- **On Windows, xdevcmd started from its terminal panel asks first**: "Start another
  xdevcmd here? [y/N]", naming the xdevcmd whose panel it is; Enter is no and ends it.
  Windows cannot tell that panel from a new window opened from it (`start cmd`), so it
  asks in both; before, it started and only said so on the status line. Without a
  terminal to ask on, it starts. Linux and macOS are unchanged: refused in the panel
  itself, started in any other window.

### Fixed
- **Alt+Y no longer copies into a folder it could not read** (as if it were empty), nor
  plans two non-UTF-8 names that read alike as one path that does not exist, nor copies
  a file onto its twin on the other side when that twin could not be looked at.
- **Alt+Y no longer replaces a file that appeared after the compare** at a place the
  plan saw empty (it took every file there when the job started for one it named).
- **A link copied over a file no longer loses the file** when making the link fails: it
  is made under a temporary name and renamed over.
- **A journal write that fails is no longer silent** (it was dropped, and a kill then
  left nothing to tidy up with).
- **Enter on a search result in a slow folder lands on the file**, not on `..`, once the
  folder has been read; so do a typed path, a link's Go and "both panes here".
- **Panelize takes a line whole when it names a file**: `report:2026.txt`, `a b.txt` and
  `2026` are those files, not `report` at line 2026, `a` and `b.txt`, or nothing.
- A path in the shell's history was shown as joined, not resolved (`src/../lib/a.rs`);
  it is shown, and acted on, as the file it is.
- **A theme that is not there no longer throws away the whole config.** A `[theme] name`
  whose file was deleted or does not read made xdevcmd ignore every setting; now the
  default theme stands in, the rest of the config holds, and the status line says which
  theme was missing. `--check-config` still reports it.
- **Names in Chinese, Japanese or Korean keep the columns in line**: such a character
  takes two columns, and names were fitted by counting characters, so a row with one ran
  past the others, pushed Ext, Size and Modified to the right and cut the date. Names are
  now fitted, padded and shortened in columns as the terminal draws them; the same goes
  for pane titles and the status line.
- **A status message no longer runs into the terminal's keys**: with the terminal in
  focus and a long message, the status line read "…Shift+F12 historytoo small to split";
  the two keep their gap now.
- **A second tab on one pane no longer puts its list a row below the other pane's.** The
  pane beside it shows its tab bar too, with its one tab, so both lists start on the
  same row. A lone tab has no × now, since the last tab cannot be closed.
- **A dialog over a Chinese, Japanese or Korean name keeps its left border.** When the
  dialog's edge fell on the second half of a wide character, the terminal printed the
  character over the border; that character is now blanked.

### Removed
- **The Night Owl example theme**: it was nearly the built-in `dark` (the closest pair of
  all the themes), and close to Nord and Tokyo Night. A copy you keep in your own
  `themes/` folder still works; without it, xdevtheme stands in (see Fixed).

## [0.3.0] - 2026-11-10

### Added
- **Between the panes and the shell** (docs/shell.md). In the file panes: **Alt+G** takes
  the pane to the shell's directory (the reverse of Alt+C); **Alt+Enter** or
  **Ctrl+Enter** (and **Type into the shell** in the files' right-click menu) types the
  marked names into the shell's command line, quoted for that shell, without Enter.
  `[terminal] follow` (off by default) lets the file pane used last follow the shell's
  `cd`, at the prompt only, never the other way.
- **One check before anything is typed into a shell**, for Alt+C, typed names and the
  user menu (Shift+F9): a
  shell surely at its prompt gets it at once (`[terminal] send_when_idle`, on by
  default); a program running there, or `ssh`, gets nothing and xdevcmd offers **Open a
  new shell** in the pane's directory instead; when it cannot tell (Windows without
  prompt marks) it asks, with **Send to this shell** as well.
- **xdevcmd reads what the shell says about itself**: OSC 7 and OSC 9;9 (its directory)
  and OSC 133 prompt marks, from snippets for bash, zsh, fish, PowerShell and cmd in
  docs/shell.md (only bash's has been run). Linux reads the directory from `/proc` and
  ignores OSC 7; macOS asks `/usr/sbin/lsof` when a key needs it; Windows has only
  what the shell reports. What is trusted, and for what: docs/security.md.
- **Clicks and drags inside the terminal go to a program that asked for the mouse**
  (vim with `mouse=a`, htop, Claude Code), in the encoding it asked for, as the wheel
  already did. Otherwise a click there is as before.
- **Shift+F12: the shell's history in the viewer** (copy mode). Everything the focused
  shell kept, and its screen, opens in the F3 viewer at its end: read, `/` search, drag
  and `y` to copy (OSC 52). It is held in memory only; nothing of a shell's output is
  written to disk. Works from inside the shell, and from the files on the shell last in
  focus. Alt+X F12 where Shift+F12 does not arrive; macOS Terminal.app's Shift+F12 is
  understood as it is. Esc goes back where you were.
- **A right-click on a terminal tab** opens a small menu for that tab: View history,
  Save history as…, Close tab, each with its key. Inside the shell a right-click stays
  the program's.
- **The viewer saves and goes to a line.** F2 is Save as…: a shell's history is offered
  as `xdevcmd-history-<date>-<time>.txt` in the shell's directory and written readable
  by you only (`0600`, Unix); a file is copied under the name given. A name that exists
  is asked about first; read-only and restricted modes refuse. `:` or Ctrl+G goes to a
  line (`40`) or a share (`50%`), and in hex to an offset (`0x1f00`); past the end goes
  to the end and says so. A message on the viewer's last row now always shows, keys
  making room for it.
- **`[terminal] scrollback`**: how many lines of history each shell keeps, 100 to 9999
  (5000 as before), for the wheel and Shift+F12. In Settings → Terminal with what a full
  history costs at the window's width (vt100 keeps 32 bytes a cell: 9999 lines at 120
  columns is about 37 MB a shell). Applies to shells started after the change.
- **Sharp pictures in quick view** where the terminal draws them itself: the Kitty
  graphics protocol (kitty, Ghostty), iTerm2's inline images (iTerm2, WezTerm) and sixel
  (Windows Terminal, foot). Half blocks stay everywhere else, macOS Terminal.app and
  tmux included. `[viewer] images` (also in Settings) chooses: auto, kitty, iterm2,
  sixel or blocks. Sixel is drawn pixel for pixel, so only where the terminal gives its
  cell size (Windows Terminal's is fixed); elsewhere half blocks, and Usage says so.
- **Safe operations** (`[general] safe_operations`, on by default): every copy writes
  under a hidden temporary name and renames when complete, so no file is ever left cut
  short under its real name, whatever ends xdevcmd; a journal lets the next start clean
  up after a killed run, put rename many's files back, and offer to continue the job:
  what is already there and identical is skipped, anything else there is asked about.

- **Shift+F10** (and the Menu key, where the terminal reports it) opens the right-click
  menu for the row under the cursor, at that row.
- **Alt+X, then an F key**, does what Shift and that key do (Alt+X F8 is Shift+F8,
  Alt+X F10 the right-click menu): for terminals that send no Shift+F keys, such as
  macOS Terminal.app.
- **xdevcmd says which terminal it runs in** (from the environment, or by asking the
  terminal its name and kind, which works over SSH) and how it draws pictures: on the
  status line at start, in Usage (Shift+F1) and in About.
- **A crash log**: if xdevcmd crashes it writes what happened to its state directory and
  says where, to attach to an issue; nothing is sent. A failure in a background task
  is logged the same way and xdevcmd goes on, saying where the details are.
- **`--print-completions bash|zsh|fish|powershell` and `--print-man`**: tab completion
  and a manual page, for packages to install.
- About links the Releases page (xdevcmd never looks for updates itself).
- **The wheel reaches full-screen programs in the terminal panel**: to a program that
  asked for the mouse (Claude Code, vim, htop) as the mouse; as arrow keys to one that
  did not (less, man). In a plain shell it still scrolls the history.
- **Four more example themes**: Dracula, Night Owl, Tokyo Night and Catppuccin Mocha, in
  `themes/` beside Nord, Gruvbox dark and Solarized light.

### Changed
- **The name, spelled out: xDev Commander.** The one-line description is now "xDev
  Commander: a cross-platform, two-pane terminal file manager that extends your shell."
  (`--help`, About, the README, GitHub, crates.io, Scoop and winget). On Windows, Task
  Manager and the taskbar show "xDev Commander". The program is still `xdevcmd`.
- **Installing**: `cargo install xdevcmd --locked`, from crates.io, from 0.3.0. The
  package is now called `xdevcmd` (it was `xdevcmd-tui`, never published), and its engine
  `xdevcmd-core` is published beside it, with no stable API of its own. The command is
  still `xdevcmd`. From the repository: `cargo install --git
  https://github.com/nci-gis/rust-commander xdevcmd --locked` (the last word was
  `xdevcmd-tui`). Scoop and winget manifests are ready in `packaging/`, for after the
  release.
- **Alt+C and the user menu with a program running in the shell** no longer only say so
  on the status line: they ask, offering to open a new shell in the pane's directory
  instead. On Windows, where xdevcmd cannot tell without prompt marks, they ask too.
- **Inside the terminal panel, xdevcmd takes only F12, Shift+F12 and Alt+X** (keymap
  A). Every other key is the program's: Alt+N, Alt+S, Alt+D, Alt+W, Alt+O, Alt+T, Alt+C,
  Alt+Z, the Alt+R group, Alt+Shift+arrows, Alt+1…9 and Ctrl+PgUp/PgDn no longer act
  when typed into the shell, so readline's Alt+D, Alt+C, Alt+T, Alt+N and Alt+1…9 work
  there. From the files they do what they did. To use one on the shell, press Alt+X
  first (Alt+X Alt+S splits the shell); anything else after Alt+X goes to the program.
  F1…F11 now reach the program too (mc, htop); before, no F-key did.
- **xdevcmd will not start inside its own terminal panel**: it says it is already
  running and that F12 goes back to it. Only that terminal counts: a new window started
  from the panel (`code .`, `kitty &`) starts xdevcmd as usual. On Windows, where the
  terminal cannot be told apart, xdevcmd starts and says on its status line that it was
  started from a panel.
- **Closing the terminal (or losing SSH) ends xdevcmd tidily**: jobs are cancelled and
  clean up, and the session is saved. Before, xdevcmd was killed on the spot.
- **F10 with jobs running waits until they have cleaned up**, and asks again if one is
  stuck after five seconds, instead of quitting under it. While it waits, the screen goes
  on drawing and says how many jobs it is waiting for.

### Security
- **A few bytes of output could freeze xdevcmd.** The terminal panel's emulator, vt100
  0.16, spent most of a second on a single `ESC [ 65535 @` (insert that many blanks),
  and the whole interface waited on it: a file of them shown with `cat`, or a server's
  banner over `ssh`, stopped xdevcmd for minutes. vt100 is now 0.18, where it takes
  microseconds; the panel is drawn by xdevcmd's own code, and `tui-term` is no longer a
  dependency.
- **Crash logs and the safe-operations journal were readable by other users** on a shared
  machine; both name files by their full paths. They are now the user's alone (`0600`,
  and the journal's directory `0700`), and a crash log writes the home directory as `~`.
- **Tidying up after a killed run went around read-only and `--root`** in the engine:
  only the interface checked read-only, and nothing checked the root. The engine now
  leaves every journal alone in read-only mode, and one that names a place outside the
  root for a run that may go there.

### Fixed
- **Long lines in dialogs and F1 were cut off.** About's description lost its middle to
  "…" in a window under about 115 columns, and F1's descriptions ran off the right edge.
  A sentence now wraps at spaces (dialogs at 72 columns at most); F1's continue under
  their own column. Paths are still shortened in the middle, as before.
- **Settings → Save said where it saved in Windows' internal form**, `\\?\C:\…`, and F1's
  Settings page showed the config file the same way. Both show `C:\…` now.
- **Inside GNU screen, xdevcmd's start-up question about pictures became screen's status
  line.** It is no longer asked inside screen (as inside tmux), also under `sudo`, where
  `TMUX` and `STY` are dropped but `TERM` says it. A reply arriving after the start-up
  wait is now read and kept, instead of reaching the shell as typed keys.
- **The user menu's `%f`/`%s` could pass a file named like an option** (`-rf`) to the
  command as an option. Such a name now gets `./` in front, as typed names do, and a
  name that is not UTF-8 stops the command instead of being typed changed.
- **A pipeline whose first program had ended** (`git log | less` once git is done) was
  taken for a shell at its prompt on Linux, so Alt+C typed into it. It is a running
  program now, and a `/proc` answer that cannot be read counts as "cannot tell".
- **Ctrl+Space (and Ctrl+@) reached a program in the terminal panel as a space**, not
  the NUL a terminal sends: emacs' and zsh's set-mark did nothing.
- **A click on a directory in the title of a pane split off below (Alt+D) did nothing**:
  that title sits on the boundary between the panes, and the click started a drag of
  the boundary instead. The names now take the click; the rest of the row still drags.
- **The scrollbar looked dashed in macOS Terminal.app**, which leaves space between
  lines; the bar is now solid there too.

## [0.2.1] - 2026-11-07

### Added
- **Example themes in the release files**: `themes/` holds Nord, Gruvbox dark and
  Solarized light. Kept beside `xdevcmd`, they appear in View → Theme… as they are.
- **`xdevcmd --print-theme NAME`** writes a built-in theme out as a file with every
  colour listed, to start your own from.
- **Ctrl+G** also goes to a path you type, beside Ctrl+L.
- **A build for 64-bit ARM Linux** (`aarch64-unknown-linux-musl`, static): ARM servers
  and a Raspberry Pi running a 64-bit system. The test suite runs on ARM too.
- **Ctrl+Z undoes a delete to the trash**: what F8 moved goes back where it was, if its
  name is still free and it is still in the trash. On Linux from xdevcmd's own trash
  records, on macOS from where the Trash said it put it, on Windows by finding it in
  the Recycle Bin.
- **Copy and move can replace only older files**, as robocopy does: F5/F6's
  confirmation has a line for files already there, which **U** changes (ask; replace
  only if older and skip the same; leave all; replace all), and the overwrite question
  has **U**, "update all".
- **A fuzzy Ctrl+F filter**, the default: the letters in order, best matches first.
  `[files] filter = "plain"` keeps the old behaviour.
- **On Linux, a file on another drive goes to that drive's trash** (`.Trash-$uid`, as
  the freedesktop spec says) instead of being offered a permanent delete.

### Changed
- **Idle, xdevcmd now uses almost no CPU**: about 0.3 % of a core instead of 5 %. The
  screen is drawn when something changed (a key, a click, a shell's output, a job's
  progress, background work reporting in) and once a second otherwise, not 25 times a
  second.
- **Panes learn of changes from the system** (inotify, FSEvents, ReadDirectoryChangesW)
  for the directories on screen, instead of re-reading every second; polling stays,
  less often, for network drives.
- **Nothing heavy runs on the drawing thread any more**: Alt+Y's compare runs on its
  own, counts as it goes, and Esc stops it; quick view decodes pictures on a thread.
- **Directory sizes** use a few threads (one per core, at most four) instead of one per
  directory, count a hard-linked file once, and stay on the directory's own drive.
- **Shift+F8 deletes only on Y** (or a click on its button), never on Enter, and lists
  the first names when several items go.

### Security
- **Renaming never replaces a file that appears meanwhile.** F2, renaming many and
  moving into the Linux trash ask the system to refuse a name that is taken, in the
  same step as the rename; before, a file created between xdevcmd's check and the
  rename was overwritten.
- **A program is never found in the directory you browse.** On Linux and macOS a
  program run in a terminal tab by its bare name is looked up only in `PATH`'s absolute
  directories; a `.` or empty entry used to make it look in the browsed directory.
- **Copying and moving never overwrite a file that appears meanwhile.** A name found
  free is written only if still free (a new-file-only open for a copy, the rename
  above for a move); a file created in between gets the usual overwrite question.
- Updated libraries to their latest patch releases, among them `zip` and `flate2`,
  which read archives.
- **Replacing a file never loses the old one part-way.** A copy or an extraction that
  replaces a file writes the new one under a temporary name and swaps it in only when
  complete; a cancel or an error used to leave neither.
- **"All" answers stay with their question.** "Overwrite all" no longer also agrees to
  copy-then-delete across drives or to permanent deletes when the trash is unavailable.
  A synchronisation replaces only the files its plan listed.
- **Unpacking never writes through a link** already at a file's name (the link is
  replaced), and refuses an archive that would not fit, or that would unpack to over
  1 GB at more than 1000 times its size, or to more than a million files.
- **The Linux trash follows the spec**: the `.trashinfo` is written before the file is
  moved, so a crash cannot leave a file nobody can restore, and a trashed link records
  its own path, not its target's.

### Fixed
- **A move to another drive deleted files the user had chosen to skip.** Across drives
  a move is a copy then a delete, and the whole source was deleted when nothing had
  *failed*; a file skipped at the overwrite question was deleted without being copied.
  Now each file is removed only once its own copy is complete. A single file moved onto
  another across drives is asked about once, not twice. (0.2.0 has this bug.)
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
