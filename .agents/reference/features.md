# Features: finding, selecting, file operations

> **Read this when**: working on quick search, filter, go to, history, hotlist, directory sizes, auto-refresh, Enter on a file, `[[open]]` rules, open-with, undo, cd on exit, find files, bulk rename, quick view, the small file operations, the user menu, background jobs, looks or pictures.

---

## Finding, selecting, staying current

| | |
|---|---|
| `/`, `Ctrl+S` | quick search: jump to the first name starting with the text, else containing it; Up/Down step through matches |
| `Ctrl+F` | filter: hide what does not match; Enter keeps it (shown in the pane's title), Esc takes it off |
| `Ctrl+L` | go to a typed path; a file lands the cursor on it; Tab completes directories |
| `Alt+,` / `Alt+.` / `Alt+↓` | back, forward, the list — per tab |
| `Ctrl+D` | hotlist: `[bookmarks]` from the config, then ones added here |
| `+` / `-` | mark / unmark by pattern; a trailing `/` matches directories only |
| `Space` / `Ctrl+Space` | size of the directory under the cursor / of every one shown |
| `o` | open with the system's application, or `[commands] opener` |
| `Ctrl+U` / `Alt+=` | swap the panes / show this directory in the other one |

**Any key the search line does not use ends it and then does its job**, as in Total
Commander: `/vic` then `F8` deletes `victim.txt`. Alt and Ctrl chords count as "not
used", except the line's own Ctrl+A and Ctrl+U.

**A filter unmarks what it hides.** A mark on a row you cannot see would still be acted
on by F8. `a_mark_the_filter_hides_is_dropped_so_f8_cannot_reach_it` holds that.

**A reload keeps the cursor on its file**, wherever it now sorts. Before, it kept the row
number, which was harmless while only your own keys caused reloads; once a directory
refreshes by itself, a file appearing above the cursor would silently move the next F8
onto its neighbour. The pty test that checks it fails with the old behaviour put back.

**Auto-refresh** (`[files] auto_refresh_ms`, default 1000, 0 off) runs on the active tab of
each visible pane, never under an open dialog or during a drag. Up to 2000 entries it
re-reads the listing and compares names, sizes and times, so a growing log is noticed;
above that it only compares the directory's mtime, which catches files appearing and
going but not growing. Measured: re-reading 2000 entries costs 1.6 ms, so a check a
second is noise. Polling rather than inotify/FSEvents/ReadDirectoryChangesW: no
dependency, identical on all three platforms, and it works on network mounts, where
those APIs are silent.

**Directory sizes** never follow symlinks — a link to `/` inside a directory must not make
it "the size of the disk" — and walks are cancelled when the pane changes directory. A
size with a `+` after it means some subdirectory could not be read.

**`o` never goes through a shell.** A configured opener gets the path as one argument.
Windows uses `explorer.exe "path"`, not `cmd /c start`, which re-parses its command line
and would run `a&calc.exe`. Where `opener::no_screen` finds no screen (below) it refuses
rather than open the file on a remote machine's screen, or let `xdg-open` fall back to a
text browser in this terminal; a configured opener is still used. Programs, scripts and
`.desktop` launchers get a "Run it?" menu whose default is Cancel. Refused in read-only
and restricted modes, like F4.

**cd on exit**: `--cwd-file PATH` writes the last file pane's directory on `q` (raw bytes
on Unix, no newline); `Q` quits without. Written even in read-only mode — the caller
named the file, so it is their output, not xdevcmd's state. Shell functions are in the
README.

## Opening files: Enter, `[[open]]`, Open with

**Enter on a file opens it**, as in Total Commander (`[files] enter = "open"`, the
default; `"view"` keeps 0.1.0's F3). `App::enter_file`, in order:

1. Inside an archive, or outside `--root`: view.
2. The first `[[open]]` rule for the extension (case ignored; `os` scopes a rule;
   `needs_display` rules are skipped where there is no screen): `action = "view"` /
   `"edit"`, or `command`, an argv with the path pushed as one more argument — never a
   string a shell splits. `terminal = true` runs it in a terminal tab (`security.md`).
3. Read-only or restricted: view.
4. A program, script, `.desktop` or `.lnk`: "Run it?" — Cancel first, Run in a terminal
   tab (console programs only), Run with the system. Nothing runs from one keypress.
5. Else the system's program, or `[commands] opener`, as `o`.

Where nothing can be opened the file is viewed and the reason goes in the viewer's own
bar: the first try wrote it to the status line, hidden behind the viewer.

**One "can anything be opened here?" check**, `opener::no_screen`, for `o`, Enter and
`needs_display`: not over SSH (`SSH_CONNECTION` / `SSH_TTY`) on any system, and on Linux
and the BSDs not without `DISPLAY` / `WAYLAND_DISPLAY`. It replaced guesses in several
places and found a bug: `o` over SSH into a Mac or Windows machine opened the file on
that machine's own screen. Render and pty tests strip the SSH variables and set
`DISPLAY`, so CI on three systems, and a developer working over SSH, see the same thing.

**Shift+O, Open with…** (also in the File and right-click menus): the matching rules
(`needs_display` ones greyed out without a screen), System default, View, Edit, and Run in
a terminal tab for a console program. Windows' own chooser is left out:
`SHOpenWithDialog` needs FFI, which `forbid(unsafe_code)` rules out.

Not proved by a test: a hostile file name through a rule's argv (the path is one
`OsString`, never joined, but nothing checks it).

## Find, rename many, quick view, small operations

**Find (Alt+F7) opens its results as a tab**, so the directory you searched from is one
`Ctrl+PgUp` away and `Ctrl+W` throws the results away. Each row carries its full path,
so marks, F3, F4, F5, F6 and F8 need no special case. Enter on a result goes to its
directory with the cursor on it; Esc stops a search still running. The walk never
follows symlinks and opens only regular files, so a link to `/` or a FIFO cannot hang
it; content is matched ignoring ASCII case, in 1 MiB chunks that overlap by the
needle's length (a match across a chunk boundary is tested).

**Shift+F2 renames by editing text**, the way yazi does rather than Total Commander's
`[N]` masks: one name per line in a temporary owner-only file, `$EDITOR`, then a plan
shown as `old → new` before anything moves. The plan is refused whole for a missing
line, two files given one name (compared without case, since that is one file on
Windows and macOS), or a name that would replace a file not being renamed. Renames go
through temporary names first, so swaps and chains work; a failure in the first step
puts back what moved.

**Quick view (Alt+Q)** draws the viewer in the other pane, following the cursor. Files
over 64 MiB are not opened for it — moving the cursor across a directory of large logs
must not index each one. It ends when its pane gets the focus.

**Shift+F5 copies beside itself** through the copy engine (`Request::rename_to`), so it
has progress, cancel, and the overwrite question like F5. **Shift+F4** creates the file
with `create_new` — an existing name is opened, never emptied. **Ctrl+Shift+F5** makes a
symlink in the other pane and never replaces anything; on Windows it needs Developer
Mode, and the error says so. **Alt+P** takes an octal mode on Unix, never through a
symlink (chmod on a link changes its target); on Windows only the read-only attribute
(`ro`/`rw`), because a text box cannot honestly edit an ACL. **Alt+H** hashes on a
thread and checks `name.sha256`, `SHA256SUMS` or `sha256sums.txt` beside the file; a
mismatch turns the box red.

**Shift+F9 types `[[menu]]` commands into the terminal**, so their output is where you can
read it. The command is the user's own (from a config xdevcmd refuses if others can write
it), but the placeholders are file names somebody else chose: each goes through
`safety::quote` for the shell in the panel, and one that cannot be quoted safely stops
the command. `f9_types_a_menu_command_into_the_shell_with_names_quoted` uses a name
with an apostrophe, which unquoted would leave the shell waiting for the closing one.

## Rename

`Shift+F6` (orthodox) or `F2` (what Windows and editor users reach for), or Rename in the
context menu. The dialog opens with **only the name part selected** — `report` in
`report.docx` — so typing replaces it and keeps the extension, as Explorer and VS Code
do. Directories and dotfiles are selected whole.

- **Never replaces an existing file.** The one exception is a change of case only on a
  filesystem that ignores case, where the "existing" target is the file itself; that
  is decided by inode, so a hard link to the same file is still refused.
- **A path separator is refused**, not treated as a move. F6 moves.
- **A refusal keeps the dialog open** with the reason and the text as typed.
- The cursor **follows the renamed file** to wherever it now sorts.
- The check and the rename are two steps, so a file created between them could still be
  replaced. std has no portable no-replace rename; for one interactive rename the window
  is negligible, and `rename.rs` says so rather than hiding it.

`LineEdit` counts positions in `char`s. In bytes, a cursor in `báo cáo` lands inside a
letter and the next insert panics; `positions_are_characters_not_bytes` holds that.

## Undo

**Ctrl+Z undoes the last operation where that loses nothing** (`xdevcmd_core::undo`). Every
place a write starts records it (`App::remember_job` for jobs; rename, rename many, F7
and Shift+F4 directly), so "the last" really is the last. `Undo::plan` decides against
the disk **as it is now**, not as recorded, and Ctrl+Z asks first, listing what will
happen. One step: an undo is not undone. Refused in read-only and restricted;
`undo::remove` checks again and refuses outside `--root`.

| Last operation | Ctrl+Z |
|---|---|
| rename, one or many | turned round, while each new name is still there and each old one free |
| move | moved back by the same job engine as F6 (progress, overwrite questions, across drives), for what is still where it went; from several source directories, refused |
| F7 | removes the first level that did not exist (`first_missing`), while the tree holds only directories |
| Shift+F4 | removes the file while it is empty |
| copy, delete, extract, pack, synchronise, permissions, link | nothing, and says so |

**Why not copies or deletes**: undoing a copy means deleting the copies, which loses
whatever was done to them since. Undoing a delete means restoring from the trash: the
Linux trash xdevcmd writes could be read back, but Windows and macOS keep their own, and a
restore that works on one system only would surprise, so the message points at the
system's trash. Recording these as `Undo::Cannot` matters as much as the rest: without
it, Ctrl+Z after a copy would undo an older rename behind the user's back.

## Background jobs, looks, pictures

**`B` in a progress box sends the job to the background** (`App::jobs`). The status line
shows `⟳ N jobs P%`; a background job that asks a question comes back to the front as
soon as no other dialog is up, since it is blocked until answered, and one that ends in
errors comes forward so they can be read. Marks are cleared when a job goes to the
background, as when one finishes. Quitting with jobs running asks; yes cancels them and
waits (up to five seconds) for the workers to remove their half-written files.

**Looks are opt-in** (`[files] ls_colors`, `icons`, `git_status`) because each depends on
something xdevcmd cannot see: `LS_COLORS` is written for some background, icons need a Nerd
Font, and git status runs a program. Backgrounds in `LS_COLORS` are ignored — the row's
background belongs to the cursor. The git letter shares the mark gutter, so turning it
on changes no layout; a marked row shows its mark.

**git status runs git, and git obeys the repository's config.** `core.fsmonitor` is
switched off on the command line and `GIT_OPTIONAL_LOCKS=0` keeps it from taking the
index lock, but content filters (`filter.*.clean`) cannot be turned off that way: in a
repository someone else prepared, `git status` can run their program. Hence off by
default, documented in the config template, and never in restricted mode.

**Pictures**: quick view draws PNG and JPEG with `▀`, two pixels a cell, box-filtered
down. The file is capped at 64 MiB and the header's pixel count at 50 million before
anything is allocated; `a_header_claiming_billions_of_pixels_is_refused_before_decoding`
uses a 60000×60000 PNG header. Decoders are `png` and `zune-jpeg`, pure Rust. The
render test writes its PNG byte by byte, so it does not lean on the encoder.
