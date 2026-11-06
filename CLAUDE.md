# rcmd

An orthodox (two-pane) file manager for the terminal, in Rust. Built because third-party
file managers are not available on the machines this has to run on, so the only way to
have one is to own it.

Modelled on Double Commander / Total Commander **at the level of UI and feature set**.

## Hard rules

1. **Never read Double Commander's source.** Not the Pascal, not their WCX/WDX header
   translations, not their `.po` files, not their icons. doublecmd is GPL-2.0-or-later
   (Copyright Alexander Koblov); reading it and porting makes this a derivative work and
   drags the whole repo under the GPL. Learning from the *running application*, its user
   manual, and Christian Ghisler's published plugin specs is fine and carries nothing —
   copyright does not cover layouts, keyboard conventions or feature lists. That
   distinction is the entire reason this project can be MIT-licensed.
   If you ever need to know "how does doublecmd do X", the answer is: open doublecmd and
   look at it, not at its repository.
2. **No telemetry, no network calls.** A file manager has no business making any.
3. **A destructive operation is never silent.** Delete, overwrite and move-across-device
   each confirm, report what they did, and are cancellable. This is the part users notice
   when it is wrong.
4. **Never panic on a filesystem.** A directory that cannot be read, a broken symlink, a
   file that vanishes mid-listing, a name that is not UTF-8 — each is a row with an error
   marker, never a crash. `unwrap()` on anything that touched the filesystem is a bug.
5. **Never type untrusted text into a terminal, or put it on the clipboard, without
   `safety.rs`.** File and directory names are chosen by whoever made them. A control
   character in one, typed into a shell, is a command: this was exploited, see Security.

## Branches

Work happens on **`dev`**; larger pieces on `feat/…` branches merged into `dev` by pull
request. **`main` is what people install**: it moves only by a pull request from `dev`,
with CI green on Linux, Windows and macOS, and is protected on GitHub so nothing else
can. Commit as the repository's identity — see the git notes for this machine.

## Name, description, licence: one place

`[workspace.package]` in the root `Cargo.toml` holds the version, the one-line
description, the authors (XDevTeam), the licence (MIT), the repository and the keywords.
Help > About, `--help` and `--version` read them with `env!("CARGO_PKG_…")`;
`scripts/sync-about.sh` copies the description and keywords (as topics) to the GitHub
repository's About box. The README's tagline is the description, and a test fails if they
drift. Change the text in `Cargo.toml`, run the script, done.

## Layout

```
crates/rcmd-core/   engine. No terminal code; everything testable lives here.
  listing/sort/     reading a directory, ordering it, formatting sizes and dates.
  format.rs
  rename.rs         rename in place: validate, never replace, allow case-only changes.
  glob.rs           `*` and `?` patterns, several at once, case-insensitive.
  size.rs           a directory's size, walked on its own thread, cancellable.
  find.rs           Alt+F7: names and content, streamed from a thread, no symlinks followed.
  batch_rename.rs   Shift+F2: a plan from edited names, checked whole, applied in two steps.
  sha256.rs         FIPS 180-4, checked against its test vectors. No dependency.
  attrs.rs          permissions (octal on Unix, read-only on Windows) and symlinks.
  archive.rs        zip / tar / tar.gz: list, extract, pack. The hostile-archive rules.
  compare.rs        two directories by size and time; a plan to make them agree.
  image.rs          PNG / JPEG decoded and shrunk for half-block drawing, bounded first.
  policy.rs         read-only and root, installed once at startup; the engine's own wall.
  ops.rs            copy, move, delete on a worker thread. Sends events, blocks on
                    questions, honours cancel. Nothing destructive decides for itself.
  document.rs       a file opened for viewing. Indexes line starts once, then seeks;
                    the file itself never comes into memory.
crates/rcmd-tui/    binary `rcmd`.
  layout.rs         binary split tree (tmux/VS Code style). Pure geometry, unit-tested.
  theme.rs          the palette, in one place.
  pane/files.rs     a file list: state, columns, mouse geometry.
  pane/tabs.rs      a file pane's tabs; `Pane::as_files()` returns the one on show.
  session.rs        what was open at the last quit: save atomically, load or ignore.
  pane/term.rs      one shell in a pty, drawn with tui-term; scrollback on the wheel.
  pane/terms.rs     the terminal panel: tabs of shells, each tab its own split tree.
  keys.rs           Action, the BINDINGS table, key parsing, the keymap.
  config.rs         config.toml: location, schema, validation, --print-config.
  help.rs           F1: the Keys and Settings tabs, built from the live config.
  modal.rs          confirm, text input, progress, and the question box a job raises.
  line_edit.rs      one editable line: cursor, selection, counted in chars not bytes.
  menu.rs           popup menu: right-click, the copy-or-move question after a drop,
                    history and hotlist. A one-character hint is also its key.
  search.rs         the quick search / filter line at the bottom of a pane.
  opener.rs         `o`: the system's application for a file, detached, no shell.
  bookmarks.rs      the hotlist's saved half; `~` expansion and tidying for display.
  find_dialog.rs    the two-field Alt+F7 box.
  usermenu.rs       Shift+F9: `[[menu]]` placeholders, each quoted for the shell it goes to.
  usage.rs          Shift+F1: memory, CPU, threads, draw time, a trend line. /proc, ps.
  look.rs           optional looks: $LS_COLORS, Nerd Font icons. Off by default.
  git.rs            optional git status column, from a thread. Off by default.
  viewer.rs         F3: text and hex, wrapping, search with highlighting.
  app.rs            event loop, focus, key and mouse dispatch.
```

The split is deliberate: if a GUI is ever wanted, it replaces `rcmd-tui` and keeps
`rcmd-core` untouched. Nothing in core may depend on ratatui or crossterm.

## How to run

```bash
cargo run -p rcmd-tui              # start in the current directory
cargo run -p rcmd-tui -- /etc /var # start with a left and right path
cargo build --release              # target/release/rcmd
./target/release/rcmd --screenshot=118x34 /etc /usr
./target/release/rcmd --print-config            # every setting at its default
./target/release/rcmd --check-config            # exit 1 if the config is wrong
./target/release/rcmd --config other.toml
```

`--screenshot` renders one frame into a `TestBackend` and prints it as text. It is the
way to look at the layout without a terminal attached — use it when a pane is the wrong
size or a column is off by one, rather than describing the problem from memory.

## How to test

```bash
cargo test                         # 292 tests, about 15 s (the soak test is opt-in)
cargo deny check                   # advisories, licences, sources
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Four layers:

| | |
|---|---|
| `rcmd-core` unit tests | sorting, formatting, listing, `ops.rs`, `document.rs` |
| `rcmd-tui` unit tests | layout, keymap, config, menu, base64 |
| `tests/render.rs` | runs `--screenshot --keys=…` and asserts on the drawn frame; every platform |
| `tests/pty.rs` | the real binary in a real pty, real key and mouse bytes in, the screen read back through vt100, assertions on the disk; Unix only |

**`tests/pty.rs` is the layer that catches what a frame cannot**: behaviour over time,
and the terminal talking back. Two of its tests were proved by putting the bug back:

- remove `marked.clear()` from `finish_job` and
  `copying_over_a_file_asks_first_then_overwrites` times out waiting for the marks
  to clear;
- resume from `$EDITOR` with `ratatui::init()` + `Terminal::clear()` and
  `f4_hands_the_terminal_to_the_editor_and_takes_it_back` fails with *"The cursor
  position could not be read within a normal duration"* — the original failure, because
  the harness, like a slow SSH link, never answers that query.

It waits on the screen rather than sleeping, so the whole suite runs in about half a
second; 20 consecutive runs and 5 runs under full parallel load were all clean.

Unix only because on Windows the same bytes pass through ConPTY's translation into
console events, and whether SGR mouse input survives that is unverified. `render.rs`
covers the same behaviour on all three platforms.

**Every test runs rcmd isolated** from whoever runs it: `RCMD_CONFIG` points at a file
that does not exist, `XDG_DATA_HOME` at a scratch directory, and the pty tests use
`/bin/sh` with a plain prompt. Without that, a developer who had rebound F5 would see
every copy test fail, and a stray F8 in a test would land in their real trash.

`--keys` types into the app before the frame is drawn, so a dialog can be put on screen
and checked without a terminal: `--screenshot=110x24 --keys=Down,Insert,S-F8` renders the
permanent-delete warning. It takes mouse input too — `click:X:Y`, `rclick:X:Y`,
`ctrl-click:X:Y`, `drag:X1:Y1:X2:Y2`, `wheel:X:Y:N` — and redraws after each event, so
every click lands on what is actually on screen at that moment.

Mouse tests find their coordinates by locating text in a first screenshot (`locate` in
`tests/render.rs`), never by hardcoding them, so a layout change moves the clicks with
it instead of breaking every test.

`ops.rs` is tested against a real filesystem in a temp directory, including the cases
that matter: an existing destination is never overwritten without a question, cancelling
mid-file removes the half-written copy rather than leaving it looking complete, two
files of the same name both survive going to the trash, and copying a directory into
itself is refused instead of recursing until the disk fills.

Logic worth testing lives in `rcmd-core` precisely so it can be tested without a terminal.
Sorting, size/date formatting and name splitting all have unit tests — these are the
things that look trivial and are quietly wrong (`file2` vs `file10`, `archive.tar.gz`,
leading zeros). Each of those tests was written before the code was right, and three of
them caught a real bug on the first run.

## Keys

The one source of truth is `BINDINGS` in `crates/rcmd-tui/src/keys.rs`: each row holds an
action's config name, its help-screen group and description, and its default keys. The
parser, the defaults, the F1 screen, the key bar and `--print-config` all read that table.
To see the live set, press **F1** or run `rcmd --print-config`.

**Never write a key into UI text by hand.** The key bar and the startup hint are generated
from the keymap because a user who sets `copy = ["F2"]` and still sees `5 Copy` has been
lied to. Hardcoded strings like that existed until the config landed; a test now checks
the bar follows a rebind.

## Terminals

One panel at the bottom, like an editor's: **tabs**, and each tab **split** into as many
shells as wanted. A tab is a `Node` tree — the same type the file panes use — so
splitting both ways, closing, and dragging a boundary all work inside a tab with no
second implementation.

| | |
|---|---|
| `Alt+N` / `+` on the tab bar | new terminal tab |
| `Ctrl+PgDn` / `Ctrl+PgUp` | next / previous tab |
| `Alt+S` / `Alt+D` | split the focused shell (when a shell has focus) |
| `Alt+O` | next split |
| `Alt+W` / `×` | close the shell / the tab; the last one removes the panel |
| `Alt+T` | hide or show the panel, shells kept running |
| `exit` | closes that terminal, as in an editor |

Split, close and resize are **context-sensitive**: the same keys act on file panes or on
shells depending on focus, so there is one set to learn.

**Tab names say what the shell is doing**: the program in the foreground (`vim`,
`cargo`, `sleep`) or, at a prompt, its working directory (`~` for home). Linux reads
both from `/proc` — the terminal's foreground process group in the shell's `stat`, and
the `cwd` link — so it works whatever the prompt is configured to do. Re-read at most
twice a second. Elsewhere tabs show the shell's name.

## Tabs and sessions

**File panes have tabs**, Total Commander style: `Ctrl+T` opens one on the same
directory, `Ctrl+W` closes it, a click on the bar selects or closes. The bar only
appears from two tabs, so a pane that never uses them keeps its full height — at the
cost of its top border sitting one row below its neighbours' when it does.

**`Ctrl+PgUp/PgDn` switch tabs of whatever has focus**: file tabs in a file pane,
terminal tabs in the terminal. `next_terminal` / `prev_terminal` are aliases for
`next_tab` / `prev_tab` so older printed configs still load.

Every tab bar's buttons win over the resize strip under them. A file pane split below
another has its tab bar on the strip's second row — the same trap the terminal's tab
bar fell into, avoided here before it shipped.

**The last session reopens on start**: the panes and their splits, every tab with its
directory, sort and hidden-file setting, which pane had focus, and the terminal — shown
or hidden, its height, and where each of its tabs' shells was (Linux, from `/proc`;
elsewhere they open in the focused pane's directory).

- Saved on a normal quit to `$RCMD_STATE`, `$XDG_STATE_HOME/rcmd/session.toml`,
  `%LOCALAPPDATA%\rcmd` on Windows, or `~/.local/state/rcmd/session.toml`. Written
  beside the old one and renamed over it, so a crash mid-write cannot corrupt it; mode
  `600`, since it lists the directories you work in.
- **Directories on the command line win**; `--no-restore` starts fresh; and
  `[general] restore_session = false` turns it off.
- **A session never stops rcmd opening.** Unreadable, from another version, or with
  more panes than `max_panes` now allows: a fresh start instead. A directory that no
  longer exists opens at its nearest existing parent.
- Every test sets `RCMD_STATE` to a scratch file. Without it, the pty tests — which quit
  with `q`, which saves — would overwrite the developer's own session.

## Pane size and zoom

**Alt+Shift+arrows move the nearest border in the arrow's direction**, the way dragging
it would: Left/Right the nearest vertical border, Up/Down the nearest horizontal one.
So Alt+Shift+Up on the terminal raises its top edge and it grows; on a file pane at the
top it raises that pane's bottom edge and it shrinks. Inside a terminal tab with
splits, the border between shells moves first. "Nearest" is the deepest split of that
axis on the way to the pane (`Node::move_boundary`).

This replaced grow/shrink, which resized whichever split happened to own the pane — so
Alt+Shift+Right on the terminal changed its *height*. `grow_pane` and `shrink_pane` are
kept as aliases in `keys.rs`: `--print-config` wrote them into people's files, and
an unknown name rejects the whole config.

**Alt+Z** fills the screen with the focused pane, file pane or terminal panel, and puts
it back. Moving focus anywhere else ends the zoom, as in tmux: a hidden pane must never
have the focus. Borders cannot be dragged while zoomed, since they are not on screen.

The status line says `terminal 2/3 · split 1/2`, because which split has focus is
otherwise only a border colour, and that is not something a test — or a person on a
16-colour terminal — can rely on.

## Navigating

`Alt+←` goes up one level and `Alt+→` goes into the directory under the cursor, as in a
browser or Explorer; `Backspace` and `Enter` still work. `Alt+→` only ever enters a
directory — on a file it says so — because `Enter` already views files, and a key that
sometimes navigates and sometimes opens a viewer is one you stop trusting.

Pane resizing moved from `Alt+←/→` to `Alt+Shift+arrows` to make room. Neither navigation
key is global, so with a shell focused, `Alt+←/→` still reach the shell for word motion.

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
and would run `a&calc.exe`. On Linux with no `DISPLAY`/`WAYLAND_DISPLAY` — over SSH — it
refuses instead of letting `xdg-open` fall back to a text browser in this terminal.
Programs, scripts and `.desktop` launchers get a "Run it?" menu whose default is Cancel.
Refused in read-only and restricted modes, like F4.

**cd on exit**: `--cwd-file PATH` writes the last file pane's directory on `q` (raw bytes
on Unix, no newline); `Q` quits without. Written even in read-only mode — the caller
named the file, so it is their output, not rcmd's state. Shell functions are in the
README.

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
read it. The command is the user's own (from a config rcmd refuses if others can write
it), but the placeholders are file names somebody else chose: each goes through
`safety::quote` for the shell in the panel, and one that cannot be quoted safely stops
the command. `f9_types_a_menu_command_into_the_shell_with_names_quoted` uses a name
with an apostrophe, which unquoted would leave the shell waiting for the closing one.

## Archives

**Browsed as directories.** Enter on a `.zip` (and `.jar`, `.docx`… — they are zips),
`.tar`, `.tar.gz` or `.tgz` lists it; `cwd` stays the directory holding it, and each row's
path is `archive/inside/name`, so marks and the selection need nothing special.
`ArchiveView::inner` turns those back into names inside. Only reading and extracting
happen there: F8, F6, F2, F7, F4 and the other writes say so, an archive pane is never a
copy destination, and drags into or out of one are refused — rewriting an archive to add
one file is not something rcmd does.

**F5 inside extracts** through the copy engine (`Kind::Extract`), so it has the same
progress, overwrite question and cancel. The directory being browsed is taken off the
front: F5 on `proj/src/main.rs` while inside `proj/src` writes `main.rs`, not
`proj/src/main.rs`. **F3 inside** extracts that one entry (up to 256 MiB) into a fresh
owner-only temporary directory, removed when the viewer closes. **Alt+F5** packs
(`Kind::Pack`) into a name whose extension picks the format, written as `.part` and
renamed at the end, so a cancelled pack leaves nothing; **Alt+F9** unpacks everything.

**Hostile archives** — every rule here is a test in `archive.rs`, and the zip-slip one
also end to end in `render.rs` with a zip written byte by byte, independently of the
code under test:

| | |
|---|---|
| `../x`, `/etc/x`, `C:x`, `a\..\..\x` | hidden from the listing, counted in a notice, never written |
| a symlink already in the destination | not followed: the entry fails, nothing lands outside |
| symlinks and hard links in the archive | listed, skipped on extraction, reported |
| SUID, SGID, sticky | dropped; `rwx` kept |
| an entry larger than its header says | stopped one byte past, the partial file removed |

**`zip` 9 is the floor**, and why the workspace's `rust-version` is 1.88: `cargo deny`
rejected zip 7 for RUSTSEC-2026-0367, -0368, -0369 (set-id bits and symlinks escaping on
extraction) and -0429 (abort on a crafted symlink). rcmd does not call the crate's
`extract()` that those are about, but a known-vulnerable version is not shipped either way.

## Compare and synchronise

**Alt+K** compares this pane with the other one level deep and marks, on each side, what
is missing on the other or newer than the other's copy — so F5 from either pane copies
exactly that, with the usual questions. **Alt+Y** compares the whole trees and shows a
plan, `→` this pane to the other, `←` back, each overwrite named; Enter runs it as one
copy job (`Request::sync`, pairs of source and destination directory). Because every
overwrite is in the plan that was confirmed, the job does not ask again — the one place
`overwrite_confirmed` is set.

Size and modification time, with two seconds of slack (FAT and many shares round to
two); content is not read. **Same time, different size is a conflict** and is never
copied either way: there is no telling which is right. A directory on one side only is
one entry, copied whole.

## Background jobs, looks, pictures

**`B` in a progress box sends the job to the background** (`App::jobs`). The status line
shows `⟳ N jobs P%`; a background job that asks a question comes back to the front as
soon as no other dialog is up, since it is blocked until answered, and one that ends in
errors comes forward so they can be read. Marks are cleared when a job goes to the
background, as when one finishes. Quitting with jobs running asks; yes cancels them and
waits (up to five seconds) for the workers to remove their half-written files.

**Looks are opt-in** (`[files] ls_colors`, `icons`, `git_status`) because each depends on
something rcmd cannot see: `LS_COLORS` is written for some background, icons need a Nerd
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

## Menu bar, usage, soak

**F9 is the menu bar**, as in Midnight Commander and Total Commander; the user menu moved
to Shift+F9 to make room. The bar is one row at the top (`[general] menu_bar`, default
on); F9 opens it even when it is hidden. Items are Actions, so a menu entry does exactly
what its key does, shows that key from the live keymap, and is greyed out by the same
`App::refused` that hides keys from the key bar. Left/Right move between menus
(`Outcome::Side`), a click on a title switches, and menus taller than the screen scroll.
Turning the bar on moved every row down by one: tests that read "the first line" now
read the second, and mouse tests were unaffected because they locate what they click.

**Usage (Shift+F1)** samples resident memory every two seconds for twenty minutes and
fits a least-squares line. It says *growing* only with a minute of samples and a slope
above 64 KiB a minute — below that is the allocator settling after a listing or a
viewer. Draw time is measured around `terminal.draw` in the real loop.

**The soak test** (`soak_memory_settles_instead_of_growing`, `#[ignore]`, Linux) drives
listings, tabs, viewer, search, filter, quick view, sizes, find, the usage view and
3000 lines of shell output per cycle, and compares median memory between the halves of
the run after a fifth of warm-up. **It was proved by putting a leak in**: 64 KiB per frame
took it from 81 to 139 MiB in a minute and failed it. The first attempt at that leak,
`mem::forget(vec![..])`, changed nothing — LLVM removed an allocation nobody observed —
so the mutation needs `black_box`, or it proves nothing. Measured clean: 180 s, 84
cycles, 23 MiB from start to end.

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

## Mouse

| | |
|---|---|
| click / double-click | select / enter a directory or view a file |
| Ctrl+click | mark or unmark |
| drag within a pane | mark a range; scrolls when it reaches an edge |
| drag onto another pane | drop: a menu asks **Copy here / Move here** |
| drag a border | resize panes side by side, or the terminal's height |
| right-click | context menu; its key hints come from the keymap |
| click a header / the key bar / a dialog button / a help tab | what you would expect |
| wheel | scroll panes, the viewer and help |

**A drag decides what it is as it moves.** It starts undecided; staying in its own pane
makes it a range selection, crossing into another file pane makes it a drag-and-drop.
Grabbing a *marked* row carries the whole selection; grabbing an unmarked one carries
just that file — and if it had started drawing a range first, those marks are put back.

**A drop always asks, copy or move.** Never decided by a modifier: terminals disagree
about which modifiers reach the app, and a move guessed wrong is the expensive mistake.
The menu answer is the confirmation; overwrites still ask mid-job.

**Clickable things record where they were drawn.** Dialog buttons, key-bar entries, help
tabs and menu rows store their `Rect` while rendering, and a click is matched against
that — never against geometry recomputed in the click handler, which drifts.

**Shift+click usually never arrives.** Most terminals keep it for their own text
selection when an app has the mouse. It is handled when it does arrive; nothing depends
on it.

**Copy path uses OSC 52**: the terminal puts it on the clipboard of the machine you are
sitting at, across SSH too. Windows Terminal, iTerm2, kitty and WezTerm allow it; tmux
needs `set -g set-clipboard on`; some terminals refuse silently.

Layout and terminal actions (`Action::is_global`) work from inside the shell pane too.
Every other key typed there goes to the shell, which is why `Ctrl+q` quits from a file
pane but not from the shell — in the shell it is XON and belongs to the shell.

`F12` was chosen as the escape from the shell pane because every other candidate collides
with something a shell uses: `Ctrl+\`` is not reported reliably by older terminals,
`Ctrl+J` is newline, `Ctrl+W` is word-erase. An escape hatch that sometimes types into
your shell instead is worse than an unfashionable key.

## Config

`config.toml`, looked for at `$RCMD_CONFIG`, `$XDG_CONFIG_HOME/rcmd/config.toml`,
`%APPDATA%\rcmd\config.toml` on Windows, then `~/.config/rcmd/config.toml` (macOS too —
terminal tools live there, not in `~/Library`).

- **Every field is optional.** A one-line file is valid; the rest stays default.
- **A wrong file is rejected whole**, never half-applied: bad TOML, a misspelt key
  (`deny_unknown_fields`), a colour that is not a colour, an unknown action under
  `[keys]`. rcmd starts on the defaults and the status line says `config ignored — …`
  with the field named. A config applied halfway is harder to reason about than one not
  applied at all.
- **Listing an action under `[keys]` replaces all its default keys**, so the old key
  stops working. Otherwise a rebind leaves two keys live and the config is lying.
- **Deleting always confirms.** There is deliberately no setting for it (hard rule 3).
  `confirm_transfer` only skips the question *before* a copy or move starts; overwrites
  still ask mid-job.
- `--print-config` output must pass `--check-config`; a test holds that, so the printed
  template can never drift from the schema.

## Many panes, many terminals

`[files] max_panes` (default 3, 2-16) and `[terminal] max_terminals` (default 3, 1-16)
cap what Alt+S / Alt+D / Alt+N will create. The terminal limit counts every shell, splits
included. Hitting a limit says which setting raises it.

With more than two file panes "the other pane" means nothing, so copy and move pick a
destination by **recency**: the other pane you were in last, preferring one that shows a
different directory. **Tab** in the confirm dialog steps through the rest, and a target
that is the source's own directory is called out before Enter. Dropping onto a pane that
shows the source's directory says so instead of offering a no-op menu. Closing a pane
returns focus to the one used last. All of this rides on `App::mru`, updated on every
draw.

Before this, Alt+S cloned the left pane's directory into a new middle pane, F5 from the
left pane targeted that clone, and a copy ended as "0 copied, 1 skipped".

## Security

rcmd shows data other people chose (file names, file contents), runs programs (a shell,
an editor), and changes files with your permissions. Each of these was attacked on
2026-11-06 before being called safe. What held, what did not, and what changed:

| Attack | Before | Now |
|---|---|---|
| Escape sequences in a file name, to retitle the window or write the clipboard | held — ratatui drops control characters when it writes text | unchanged |
| A directory named `evil<Ctrl+U>touch PWNED<Enter>`, then Alt+C | **ran `touch PWNED`** — readline acts on Ctrl+U and Enter before the shell sees the quotes | refused: `safety::unsafe_to_type` |
| Alt+C while a program owns the terminal (an editor, a `sudo` password prompt) | typed the `cd` into it | refused, Linux and macOS (`/proc`, `ps`); Windows cannot tell yet |
| Copy path on a name with control characters, then paste into a terminal | would run, like typing it | refused |
| F3 on a FIFO | **froze the whole UI**: `open` on a FIFO blocks | refused from `stat`, before opening |
| F5 on a FIFO | **hung, and Esc could not cancel** a blocked read | refused: `entry::special_kind` |
| F3 on `/dev/zero`, or a link to it | indexing never ends | refused |
| A config file anyone can write (it names the shell and editor rcmd runs) | loaded | refused when world-writable (Unix) |
| `o` on a file named `a&calc.exe`, or a script | — | no shell anywhere in the path; programs ask first, Cancel by default |
| A filter hiding marked files, then F8 | — | hidden rows are unmarked |
| F9 with a file named `it's.txt` or one holding Ctrl+U | — | quoted per shell; control characters stop the command |
| Alt+F7 into a tree with a link to `/` or a FIFO | — | links listed, never followed; only regular files opened |
| A zip whose names climb out, carry links or set-id bits, or lie about their size | — | see Archives: hidden, skipped, dropped, stopped |
| A PNG header claiming 60000×60000 pixels | — | refused before decoding: 64 MiB file, 50 M pixel caps |
| git status in a repository with hostile config | — | off by default; fsmonitor disabled; filters cannot be, so documented |

Each row that was an attack is now a test in `tests/pty.rs`, and the command injection
test was proved by removing the guard: it reports `a command ran: [.../PWNED]`.

**Alt+C quotes per shell.** POSIX shells get `cd '…'` with `'\''`, fish escapes `\` and
`'`, PowerShell gets `Set-Location -LiteralPath '…'` with `''`, and cmd.exe gets
`cd /d "…"` — refusing a path with `%`, `!`, `^` or `"`, which cmd.exe cannot quote.
All of it ends with `\r`, the Enter key, which ConPTY expects too.

**Scanning.** `cargo deny check` (installed user-level with `cargo install --locked
cargo-deny`) checks dependencies against the RustSec advisory database, rejects any
licence not in `deny.toml` — a GPL or LGPL crate fails it, proved by adding
`sequoia-openpgp` — and allows crates.io only. CI runs it on every push and weekly,
because advisories are published after code is written. `#![forbid(unsafe_code)]` is
set in both crates: any unsafe code rcmd runs is in a dependency.

### Secure modes

| | `normal` | `read_only` | `restricted` |
|---|---|---|---|
| copy, move, delete, rename, mkdir, F4 edit, drop | yes | **no** | **no** |
| session file written on quit | yes | **no** — rcmd writes nothing | **no** |
| view (F3), navigate, copy path | yes | yes | view and navigate only |
| shell (F12, Alt+N, Alt+C), clipboard | yes | yes | **no** — no shell is ever started |

Set by `[security] mode` in the config or `--read-only` / `--restricted`. **A flag can
only tighten the config**: the effective mode is the stricter of the two, so a wrapper
that sets `restricted` cannot be escaped by adding `--read-only`. `[security] root` /
`--root` confine navigation to a directory, resolved through symlinks, so a link
inside the root pointing out of it is refused for both entering and F3; `--root` must
be inside a configured root. The root confines rcmd, not a shell — `restricted` is
what removes the shell.

**Two walls, each tested alone.** `App::refused` is the one place the UI decides: the
key bar hides what is off, the context menu greys it out, every input path — keys,
mouse, drag-and-drop, menus — asks it and says why. Behind it, `rcmd-core/policy.rs`
is installed once (`OnceLock`, so nothing can loosen it at runtime) and the
copy/move/delete engine, rename and mkdir refuse on their own. With the UI gate removed
on purpose, the UI tests fail and the pty test that presses F8, Shift+F8, F5, F6, F2
and F7 in read-only mode still finds the disk untouched.

The modes guard against mistakes and hostile content, not against the person running
rcmd: anyone can start it without the flag, or point `RCMD_CONFIG` elsewhere. A real
boundary is the operating system's — Landlock on Linux, the App Sandbox on macOS,
AppContainer on Windows.

**Not covered.** rcmd runs with your privileges and has no sandbox; a file you choose
to open in `$EDITOR` is that editor's problem. On Windows the busy-shell check and the
config permission check do nothing yet. Nothing here fuzzes the parsers.

## Platforms

Linux, Windows and macOS. The stack carries most of it: crossterm runs on all three and
`portable-pty` uses **ConPTY** on Windows, so the shell pane works there too.

What is platform-specific, and where:

| | |
|---|---|
| Trash | Linux uses `trash_into` in `ops.rs` — tested here, and it takes an injectable root so tests never touch the real trash. Windows and macOS use the `trash` crate, declared only for `cfg(not(target_os = "linux"))`, because the Recycle Bin and macOS Trash are OS APIs, not a directory you can rename into. |
| Hidden files | A leading dot on Unix; on Windows the `FILE_ATTRIBUTE_HIDDEN` attribute **or** a leading dot, so `.git` still hides on a developer's machine. |
| Executable | The mode bits on Unix, the extension on Windows. |
| Shell name | `$SHELL`, falling back to `%COMSPEC%` on Windows. |
| Editor | `$VISUAL`, `$EDITOR`, then `vi` — or `notepad` on Windows. |
| Symlinks | Copied on Unix. On Windows they are reported as an error rather than silently skipped; creating them needs a privilege we should not assume. |

**Check another platform without leaving this one:**

```bash
rustup target add x86_64-pc-windows-gnu x86_64-apple-darwin
cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy --all-targets --target x86_64-apple-darwin  -- -D warnings
```

`cargo check` and `cargo clippy` do not link, so no cross-linker or SDK is needed. This
catches a broken `#[cfg]` in seconds. It does **not** catch behaviour, which is what the
CI matrix in `.github/workflows/ci.yml` is for; it runs on every push to
`github.com/nci-gis/rust-commander`. Until it has passed there, "works on Windows" means
"compiles for Windows", and nothing stronger.

**No test may depend on a path the host happens to have.** `/etc` and `/usr` do not exist
on Windows; `tests/render.rs` builds its own fixture tree instead.

## Toolchain

A user-level `rustup` is all it needs: nothing here requires root, and building must
never install anything system-wide. Facts about a particular development machine belong
in that machine's own notes, not in this repository.

## Gotchas that cost time

- **vt100 panics on a one-row screen.** When a line wraps, `grid.rs` does
  `prev_pos.row -= scrolled` with row 0 and underflows; the `unwrap()` after it panics on
  the pty reader thread. The process survives, so the symptom is a shell that silently
  stops updating. Reproduced at 80x24 by splitting the terminal top and bottom and
  running anything with a long line of output (bash's own prompt does not trigger it:
  readline scrolls horizontally on a one-row screen). Two fixes: vt100 is never given
  fewer than 2 rows (`MIN_ROWS` in `pane/term.rs`), and a terminal split that would leave
  a half too small to use is refused. `no_layout_at_any_size_panics_or_overflows` renders
  14 layouts at 6 sizes from 1x1 to 200x60 and keeps it from coming back.
- **`--screenshot` ticks once before the first frame.** Without it, a frame with no
  `--keys` never saw anything a thread delivered (git letters, sizes), because only
  input drove `tick()`; the git column looked broken when it was not.
- **`RCMD_SCREENSHOT_WAIT_MS`** (default 900) is how long `--screenshot` waits for a shell
  prompt. The sweep sets it to 0; anything that asserts on shell output must not.

- **The tab bar shares a row with the resize strip.** The strip that resizes the
  terminal panel is two rows — the file panes' bottom border and the panel's first row —
  and the panel's first row is its tab bar. Dividers were hit-tested first, so clicking
  `+` or a tab started a resize instead. Tab-bar buttons now win; the bar's empty space
  still drags the panel. Found by `the_tab_bar_buttons_are_clickable`.
- **F12 after closing the last terminal** started a new one but left focus on the file
  panes, so the next keystrokes went to the files. `toggle_term` deliberately leaves
  focus alone because startup uses it; `toggle_term_focus` now takes it.

- **`Ctrl+letter` has no case.** A legacy terminal sends Ctrl+R as a control byte, and
  crossterm reports it as lowercase `r`. `keys::normalise` folds case under Ctrl, so a
  binding written `Ctrl+R` still fires. Found by a keymap test, before any user did.
- **`--keys` splits on commas**, so `Alt+,` cannot be written there: use `Alt+comma`
  and `Alt+period`, which the config accepts too.
- **Tests that share a scratch directory race.** `policy.rs` had three tests creating
  and deleting one directory; it failed one run in six once more tests ran beside it.
  Every test gets its own directory, named after it.
- **`--keys` and `[keys]` share one parser** (`keys::parse_key`). `--screenshot` once had
  its own small parser that silently dropped `PageDown`; a help-screen test that should
  have scrolled did not, and the bug looked like it was in the help screen.

- **`Terminal::clear()` reads the cursor position.** In ratatui 0.30 it snapshots the
  cursor so it can restore it, and that query times out as *"The cursor position could
  not be read within a normal duration"* whenever the terminal is slow to answer — a
  laggy SSH link, a multiplexer, anything that is not a local xterm. It killed the app
  on the way back from `F4`. Clear through `backend_mut().clear_region(ClearType::All)`
  and reset the diff state with **two** `swap_buffers()` calls; one resets only one of
  the two buffers, and the diff against the stale frame comes out empty.
- **Do not re-run `ratatui::init()` to resume after suspending.** Same cursor query,
  same failure. Leave and re-enter the alternate screen by hand and keep the same
  `Terminal`.
- **A capped line index must record where it stopped.** `document.rs` stops indexing at
  four million lines; without an `indexed_len`, the last line's end fell through to EOF
  and the viewer drew 300 MB of the file as one row with every newline shown as `·`.

- **Never assert on a pty's raw byte stream.** ratatui only emits the cells that
  changed, so the stream arrives without the spaces between words:
  `Deletedpermanently.Thiscannotbeundone.` The first, Python, pty harness matched against
  that and needed whitespace-squashing hacks. `tests/pty.rs` feeds the bytes through
  vt100 — the parser the shell pane already uses — and asserts on the reconstructed
  screen, where `"This cannot be undone."` simply matches.
- **Clear marks when an operation finishes.** Leaving them set means the next `F8`
  silently includes the files selected for the last `F5`. Found by an end-to-end test
  that deleted more than it selected; `finish_job` clears them now.
- **`vt100::Parser` has no `set_size`** — it lives on the screen:
  `parser.screen_mut().set_size(rows, cols)`.

- **Never add `crossterm` or `vt100` as direct dependencies.** `ratatui` pins its own
  crossterm (via `ratatui-crossterm`) and `tui-term` pins its own vt100. Adding either
  directly resolves a second copy, and you get two `Event` enums with identical names in
  an error message that does not mention versions. Use `ratatui::crossterm`, and take the
  vt100 version from `cargo tree -i vt100`. Adding `unicode-width` directly broke
  resolution outright, because ratatui pins it exactly.
- **Enabling mouse capture is on us.** `ratatui::init()` does not do it, and if the
  process dies without `DisableMouseCapture` the user's shell prints escape codes on
  every click. `main.rs` chains a panic hook for exactly this.

## What matters here

- **The two-pane shell is a weekend; the tail is the product.** File operations with
  proper overwrite/skip/retry semantics, archives and remote filesystems browsed as
  directories, multi-rename, directory compare. Build the shell quickly and spend the time
  on the engine.
- **Measure before optimising, then do optimise.** Searching a 337 MB log took 1.2 s
  with the UI frozen, because `find` read the file line by line — four million seeks.
  Scanning sequentially in 1 MiB chunks and mapping the hit back to a line index took it
  to 0.2 s. Opening that file and jumping to the end costs 0.09 s and 34 MB of RSS, which
  is the line index and nothing else.
- Directory listing on a big tree is the remaining hot path and has not been profiled.
