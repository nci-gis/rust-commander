# rcmd

A two-pane (orthodox) file manager for the terminal, written in Rust.

> **Status: usable.** Two file panes, a live shell below them, mouse, splittable layout,
> copy / move / delete with conflict handling and an undoable trash, and a viewer that
> opens a 337 MB log in under a tenth of a second.

```
  File   View   Help                                                                                          F9 menu
┌ /usr/share ─────────────────────────────────────────────┐┌ /usr/include ───────────────────────────────────────────┐
│ Name ▼                  Ext        Size Modified        ││ Name ▼                  Ext        Size Modified        │
│ ..                                <DIR> -               ││ ..                                <DIR> -               │
│ accountsservice                   <DIR> 2025-08-05 23:50││ arpa                              <DIR> 2026-09-12 06:26│
│ aclocal                           <DIR> 2026-09-25 06:21││ asm-generic                       <DIR> 2026-10-10 06:13│
│ alsa                              <DIR> 2026-07-16 06:09││ c++                               <DIR> 2025-12-17 18:29│
│ alsa-base                         <DIR> 2025-08-05 23:50││ crash                             <DIR> 2026-10-19 08:50│
│ alsa-card-profile                 <DIR> 2025-08-05 23:50││ drm                               <DIR> 2026-10-10 06:13│
│ application-registry              <DIR> 2026-09-24 06:48││ finclude                          <DIR> 2025-12-16 10:13│
│ applications                      <DIR> 2026-09-24 06:48││ gdb                               <DIR> 2026-08-24 11:10│
│ apport                            <DIR> 2026-08-24 11:09││ iproute2                          <DIR> 2026-08-24 11:09│
│ appstream                         <DIR> 2025-08-05 23:50││ linux                             <DIR> 2026-10-10 06:13│
│ aspell                            <DIR> 2025-08-05 23:50││ misc                              <DIR> 2026-10-10 06:13│
│ avahi                             <DIR> 2026-05-14 06:01││ mtd                               <DIR> 2026-10-10 06:13│
│ backgrounds                       <DIR> 2025-08-05 23:50││ net                               <DIR> 2026-09-12 06:26│
│ base-files                        <DIR> 2026-08-24 11:09││ netash                            <DIR> 2026-09-12 06:26│
│ base-passwd                       <DIR> 2025-08-05 23:48││ netatalk                          <DIR> 2026-09-12 06:26│
│ bash-completion                   <DIR> 2025-08-05 23:48││ netax25                           <DIR> 2026-09-12 06:26│
│ binfmts                           <DIR> 2026-09-12 06:27││ neteconet                         <DIR> 2026-09-12 06:26│
│ brltty                            <DIR> 2025-08-05 23:51││ netinet                           <DIR> 2026-09-12 06:26│
│ bug                               <DIR> 2026-10-19 08:51││ netipx                            <DIR> 2026-09-12 06:26│
└─────────────────────────────────────────────────────────┘└─────────────────────────────────────────────────────────┘
 ⌂ 1 share ×  +
┌ ⌂ sh · share ──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│$ █                                                                                                                 │
│                                                                                                                    │
│                                                                                                                    │
│                                                                                                                    │
│                                                                                                                    │
│                                                                                                                    │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ..  ·  265 items  ·  sort name ▼                                                 F1 help · F12 terminal · F10 quit
 1 Help  3 View  4 Edit  5 Copy  6 Move  7 MkDir  8 Del  12 Term  10 Quit
```

## What works

- **Two panes**, orthodox keys, natural-order sorting, marks, hidden-file toggle
- **Real shells underneath**, in ptys, like an editor's terminal panel: **tabs**
  (`Alt+N`, `Ctrl+PgUp/PgDn`) and **splits** (`Alt+S`/`Alt+D` while a shell has focus).
  `F12` jumps in and out, `Alt+C` `cd`s the shell to the directory you are looking at,
  `exit` closes a terminal, and the wheel scrolls back through its history
- **Mouse**: Ctrl+click to mark, drag down a pane to mark a range, **drag files onto the
  other pane** to copy or move them, drag a border to resize panes or the terminal,
  right-click for a context menu. Dialog buttons, the key bar and help tabs are all
  clickable. *Copy path* puts paths on your clipboard via OSC 52 — across SSH too.
- **Splittable layout**: `Alt+S` / `Alt+D` to split a pane, `Alt+W` to close it
- **Tabs in every file pane** (`Ctrl+T`, `Ctrl+W`, `Ctrl+PgUp/PgDn`), and the **last
  session reopens** on start: panes, splits, tabs, directories and terminal tabs.
  Name directories on the command line, or pass `--no-restore`, to start fresh.
- **Viewer** (`F3`): text or hex, wrapping, case-insensitive search with highlighting.
  The file stays on disk — only line offsets are held, so a huge log opens instantly.
- **Editor** (`F4`): hands the terminal to `$EDITOR` and takes it back.
- **File operations**: copy, move, delete — recursive, with a progress bar, cancellable
  part-way, and a question whenever something already exists or goes wrong. `F8` moves to
  the XDG trash and writes the `.trashinfo` that makes it restorable; `Shift+F8` deletes
  for real and says so.
- **Finding your way**: `/` jumps to a name as you type, `Ctrl+F` filters the pane down
  to what matches (`*.rs` works), `Ctrl+L` goes to a typed path with Tab completion,
  `Alt+,` / `Alt+.` go back and forward, `Alt+↓` lists where this tab has been, and
  `Ctrl+D` is a hotlist of bookmarks.
- **Selecting**: `+` / `-` mark and unmark by pattern (`*.jpg *.png`), `Space` on a
  directory works out its size, `Ctrl+Space` does every directory in view.
- **Panes stay current**: a directory that changes on disk — a file appears, a log grows —
  is redrawn within a second, with the cursor still on the file it was on.
- **`o` opens a file** with the system's application (`xdg-open`, `open`, Explorer) or
  the program in `[commands] opener`; a program or script is asked about first.
- **`Ctrl+U` swaps** the panes, `Alt+=` shows this directory in the other one too.
- **cd on exit**: with `--cwd-file`, quitting with `q` leaves your shell in the directory
  you were looking at — see below.
- **Find files** (`Alt+F7`): by name pattern and, optionally, text inside; results stream
  into a new tab where F3, F4, F5, F6, F8 work as anywhere else, and Enter goes to the file.
- **Rename many at once** (`Shift+F2`): the names open in `$EDITOR`; change what you
  like, save, and rcmd shows the plan before touching anything. Swaps work.
- **Quick view** (`Alt+Q`): the other pane previews whatever the cursor is on.
- **Small things**: `Shift+F4` new file, `Shift+F5` copy beside itself under another name,
  `Ctrl+Shift+F5` symlink into the other pane, `Alt+P` permissions, `Alt+H` SHA-256
  checked against a `SHA256SUMS` or `.sha256` beside the file.
- **Archives as directories**: Enter on a `.zip`, `.tar` or `.tar.gz` goes inside it; F3
  reads a file in it, F5 extracts what is marked into the other pane. `Alt+F5` packs the
  selection, `Alt+F9` unpacks an archive. Hostile archives are handled: names that climb
  out (`../`, absolute paths) are hidden and never written, links inside are never
  created, set-id bits are dropped, and an entry larger than it claims is stopped.
- **Compare and synchronise** (`Alt+K`, `Alt+Y`): mark what each pane has that the other
  lacks or holds an older copy of — then F5 — or plan a two-way sync of both trees,
  read it, and run it. Files that differ at the same time are listed and left alone.
- **Background jobs**: `B` in a progress box sends the copy, move, delete, extract or
  pack to the background; the status line shows `⟳ 1 job 40%`, and a job that needs an
  answer comes back to the front. Quitting with jobs running asks first.
- **Pictures in quick view**: PNG and JPEG drawn with half blocks — no graphics protocol,
  so it works over SSH in any true-colour terminal.
- **Optional looks** (all off by default): `ls_colors` (your `$LS_COLORS`), `icons` (needs
  a Nerd Font), `git_status` (a letter per file: `M ? A D R U`).
- **Menu bar** (`F9`, or click): File | View | Help, every item showing its key from your
  keymap and greyed out when the mode in effect turns it off. `[general] menu_bar = false`
  hides the bar; F9 still opens it.
- **Usage** (`Shift+F1`, Help → Usage): rcmd's own memory, peak, CPU, threads, open
  files and draw time, with a twenty-minute memory graph and a trend line that says
  *flat* or *growing*. Linux reads `/proc`, macOS asks `ps`; Windows is not measured yet.
- **Your own commands** (`Shift+F9`): `[[menu]]` entries in the config, typed into the
  terminal with `%f %s %d %D` filled in and quoted for your shell.

## Config

```bash
rcmd --print-config > ~/.config/rcmd/config.toml   # every setting, at its default
rcmd --check-config                                # validate; exit 1 if wrong
```

Keep only what you change — a missing key keeps its default:

```toml
[theme]
accent = "#ff8800"        # "#rrggbb", "reset" for the terminal's own colour, or 0-255
background = "reset"      # let the terminal's background (and transparency) through

[files]
max_panes = 4             # how many file panes Alt+S / Alt+D may create (default 3)

[terminal]
height_percent = 40
shell = "bash -l"
max_terminals = 6         # shells across all tabs and splits (default 3)

[keys]
copy = ["F2", "Ctrl+c"]   # replaces F5 for copy

[files]
auto_refresh_ms = 1000    # how often panes look for changes on disk; 0 = off

ls_colors = false         # colour names from $LS_COLORS instead of the theme
icons = false             # Nerd Font icons; needs a Nerd Font
git_status = false        # M ? A D R U per file; runs git, see Security

[commands]
opener = ""               # what `o` opens files with; empty = the system's own

[bookmarks]               # shown first in the hotlist (Ctrl+D); ~ is home
projects = "~/projects"
logs = "/var/log"

[[menu]]                  # Shift+F9; %f file, %s selection, %d this dir, %D the other pane's
key = "g"
label = "git log of this file"
command = "git log --oneline -- %f"

[[menu]]
key = "z"
label = "zip the selection into the other pane"
command = "zip -r %D/selection.zip %s"
```

Bookmarks added with `Ctrl+D` inside rcmd are saved beside the session
(`~/.local/state/rcmd/bookmarks.toml`, owner-only), never into your config file.

### cd on exit

rcmd cannot change the directory of the shell that started it — no program can — but it
can tell the shell where you were. Put one of these in your shell's startup file and run
`rc` instead of `rcmd`; `q` leaves you in the directory you were looking at, `Q` does
not:

```bash
# bash / zsh
rc() { local f; f=$(mktemp) || return; rcmd --cwd-file="$f" "$@"; local d; d=$(cat -- "$f"); rm -f -- "$f"; [ -n "$d" ] && [ "$d" != "$PWD" ] && cd -- "$d"; }
```

```fish
# fish
function rc; set f (mktemp); rcmd --cwd-file=$f $argv; set d (cat -- $f); rm -f -- $f; test -n "$d"; and cd -- $d; end
```

```powershell
# PowerShell
function rc { $f = New-TemporaryFile; rcmd --cwd-file="$f" @args; $d = Get-Content -Raw $f; Remove-Item $f; if ($d) { Set-Location -LiteralPath $d } }
```

The bash version was tested; the fish and PowerShell ones have not been run yet.

A broken file is never applied halfway: rcmd starts on the defaults and the status line
says which field is wrong. **F1** shows every key and setting as currently loaded, and
marks what you changed.

## Keys

Defaults below. **F1** inside rcmd shows the live set, including your rebinds.

| | | | |
|---|---|---|---|
| `Tab` | switch pane | `Enter` | enter directory |
| `↑ ↓ PgUp PgDn` | move | `Backspace` | go up |
| `Insert` / `Space` | mark (Space also sizes a directory) | `*` | invert marks |
| `+` / `-` | mark / unmark by pattern | `Ctrl+A` / `Alt+U` | mark all / none |
| `/` or `Ctrl+S` | jump to a name | `Ctrl+F` | filter (`Esc` clears) |
| `Ctrl+L` | go to a path | `Ctrl+D` | hotlist (bookmarks) |
| `Alt+,` / `Alt+.` | back / forward | `Alt+↓` | history |
| `o` | open with system app | `Ctrl+Space` | size of every directory |
| `Ctrl+U` | swap panes | `Alt+=` | same directory in the other pane |
| `Alt+F7` | find files | `Alt+Q` | quick view |
| `Shift+F2` | rename many in `$EDITOR` | `Shift+F4` | new file |
| `Shift+F5` | copy as | `Ctrl+Shift+F5` | symlink into the other pane |
| `Alt+P` | permissions | `Alt+H` | SHA-256 |
| `F9` | menu bar | `Shift+F9` | your commands (`[[menu]]`) |
| `Shift+F1` | usage: memory, CPU, trend | `Enter` on an archive | browse it |
| `Alt+F5` | pack | `Alt+F9` | unpack |
| `Alt+K` | compare with the other pane | `Alt+Y` | synchronise both trees |
| `Ctrl+R` | reload | `Ctrl+H` | hidden files |
| `F12` | focus the shell | `Alt+T` | show/hide the shell |
| `Alt+C` | `cd` the shell here | `Alt+S` / `Alt+D` | split pane |
| `Alt+W` | close pane | `Alt+Z` | zoom the pane / put it back |
| `Alt+Shift+arrows` | move the pane's border that way | | |
| `Ctrl+T` / `Ctrl+W` | new / close tab | `Ctrl+PgUp/PgDn` | previous / next tab |
| `Alt+←` | up one level | `Alt+→` | into the directory |
| `F3` | view | `F4` | edit in `$EDITOR` |
| `Shift+F6` / `F2` | rename | | |
| `F5` / `F6` | copy / move | `F7` | create directory |
| `F8` | delete to trash | `Shift+F8` | delete permanently |
| `F1` | help: keys and settings | `F10` / `q` | quit (`Q`: without cd) |

## Why

Third-party file managers are not installable on the machines this targets. An in-house
tool sidesteps that entirely.

It follows the Total Commander / Double Commander key layout because that muscle memory is
the whole value of the format. **No code is taken from either** — see the provenance note
below.

## Platforms

Linux, Windows and macOS — crossterm for the UI, ConPTY for the shell pane on Windows,
and the `trash` crate for the Recycle Bin and macOS Trash.

Honest status: every platform **compiles and passes clippy**, checked from Linux with
`cargo clippy --target x86_64-pc-windows-gnu` and the macOS equivalent. Behaviour on
Windows and macOS is covered by the CI matrix in `.github/workflows/ci.yml`, which runs
on every push to [nci-gis/rust-commander](https://github.com/nci-gis/rust-commander/actions).
Until it has passed there, treat Windows and macOS as "should work" rather than
"verified".

## Security

rcmd displays names other people chose and runs a shell, so it was attacked before
this section was written. A crafted directory name could run a command through Alt+C,
and a FIFO could freeze the UI; both are fixed and covered by tests, along with the
attacks that did not work. `cargo deny check` scans dependencies for known
vulnerabilities and non-permissive licences, in CI on every push and weekly. Details,
including what is not covered: the Security section of [CLAUDE.md](CLAUDE.md).

Two secure modes, from the command line or the config — a flag can only tighten the
config, never loosen it:

```bash
rcmd --read-only           # rcmd writes nothing: no copy, move, delete, rename, edit
rcmd --restricted          # read-only, and no shell, no editor, no clipboard
rcmd --root /srv/data      # never navigate above /srv/data, through symlinks included
```

```toml
[security]
mode = "read_only"         # normal | read_only | restricted
root = "/srv/data"
```

## Checking it does not leak

```bash
RCMD_SOAK_SECS=600 cargo test --release --test pty -- --ignored soak --nocapture
```

Drives rcmd through everything that allocates for ten minutes, samples its memory once a
second into `target/tmp/soak-rss.csv`, and fails if the second half sits above the
first. Linux only. Three minutes on this machine: 23 MiB throughout.

## Build

```bash
cargo build --release               # target/release/rcmd
cargo run -p rcmd-tui -- /etc /var  # start with two directories
cargo test                          # 200 tests, about 15 s
cargo deny check                    # dependency advisories and licences
cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
./target/release/rcmd --screenshot  # draw one frame as plain text
```

## Provenance and licence

`rcmd` is an independent implementation. Its design follows the orthodox file-manager
conventions established by Total Commander (proprietary) and Double Commander (GPL-2.0+),
learned by **using those programs and reading their user documentation** — never by
reading their source.

Copyright does not cover UI layouts, keyboard conventions or feature lists; Double
Commander is itself a clean reimplementation of Total Commander's design. That is why this
repository carries no GPL obligation and is released under `MIT OR Apache-2.0`.

See [CLAUDE.md](CLAUDE.md) for the working rules that keep it that way.
