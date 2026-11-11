# The interface: keys, terminals, tabs, panes, mouse, menu bar

> **Read this when**: changing keys, bindings or two-key groups, the terminal panel, tabs or sessions, pane layout and zoom, mouse handling, the menu bar, or the usage view.

---

## Keys

The one source of truth is `BINDINGS` in `crates/xdevcmd/src/keys.rs`: each row holds an
action's config name, its help-screen group and description, and its default keys. The
parser, the defaults, the F1 screen, the key bar and `--print-config` all read that table.
To see the live set, press **F1** or run `xdevcmd --print-config`.

**Never write a key into UI text by hand.** The key bar and the startup hint are generated
from the keymap because a user who sets `copy = ["F2"]` and still sees `5 Copy` has been
lied to. Hardcoded strings like that existed until the config landed; a test now checks
the bar follows a rebind.

**One key set for every system.** Measured in each system's default terminal
(`platforms.md`), the keys arrive on Windows and Linux, and on macOS Terminal.app only
history and resize could not be typed. So those actions got second keys that arrive
everywhere — `Alt+J` history, `Alt+R` then arrows resize, `Alt+1`…`Alt+9` tab N
(`go_tab`; inside the shell, after Alt+X, see keymap A below), `Alt+B` / `Alt+F` parent / enter
(what Terminal.app sends for ⌥← / ⌥→) — instead of a keymap per system. Named presets
(`[keys] preset`, `keymaps/*.toml`, `XDEVCMD_KEYS`) are deferred until a second set is
really wanted; Round 004's design §2 is the plan for then.

### Two-key bindings and groups

A binding is one key, or two separated by a space: `"Alt+r Left"` (`Keys::{One, Two}`,
`parse_binding`; three is an error). Two-key bindings that share a first key form a group
(`Keymap::seqs`, `starts_group`, `in_group`). In `App::on_key`:

- The first key opens the group and runs nothing. With a shell focused, only Alt+X opens
  one (keymap A, under Mouse below; `App::in_shell_prefix`); every other first key is
  the program's.
- **No timeout.** The group waits until Enter or Esc (closes, nothing else happens),
  another key (closes, then that key does its own job), or any mouse event (arrows typed
  after a click are arrows). A two-second timeout was built first and dropped: a key
  waiting for its second needs no timer, but must be visible.
- Only actions where `Action::repeats()` is true — the four resizes — keep the group open,
  so ← ← ← resizes three steps. Any other two-key binding runs once and closes.
- **A badge on the status line** shows a waiting group: `RESIZE` when all its actions
  repeat and share a name prefix (`group_name`), otherwise the opening key, `ALT+G …`.
  The hint beside it lists the second keys.

F1 shows two-key bindings as written (`Alt+r Left`) and more than three keys as
`first … last`; its "rebound" mark had to learn two-key bindings (it marked every resize
row until fixed).

### Clashes

Checked in `Keymap::apply` when the config loads:

- An action named under `[keys]` loses all its default keys, single and two-key.
- One key, or one two-key binding, that the user gives two actions: an error naming both.
  It used to be silent — the later action took the key.
- A user key that a default also uses: the user's wins, and the default loses it.
- A key that runs an action and also starts a group: both the user's is an error; the
  user's key beats a default group starting with it (the group is dropped); the user's
  group beats a default key (the key is dropped).
- Two defaults clashing is a bug in `BINDINGS`, and a test says so: no key runs two
  actions, none both runs one and starts a group.

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
shells depending on focus, so there is one set to learn. **Inside the shell these keys
are the program's** (keymap A): press Alt+X first (Alt+X Alt+S splits the shell). From a
file pane they work as listed.

**Shift+F12 is the shell's history in the viewer** (copy mode, Round 015): everything
the shell kept, in memory only; Alt+X F12 where Shift+F12 does not arrive.

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

- Saved on a normal quit to `$XDEVCMD_STATE`, `$XDG_STATE_HOME/xdevcmd/session.toml`,
  `%LOCALAPPDATA%\xdevcmd` on Windows, or `~/.local/state/xdevcmd/session.toml`. Written
  beside the old one and renamed over it, so a crash mid-write cannot corrupt it; mode
  `600`, since it lists the directories you work in.
- **Directories on the command line win**; `--no-restore` starts fresh; and
  `[general] restore_session = false` turns it off.
- **A session never stops xdevcmd opening.** Unreadable, from another version, or with
  more panes than `max_panes` now allows: a fresh start instead. A directory that no
  longer exists opens at its nearest existing parent.
- Every test sets `XDEVCMD_STATE` to a scratch file. Without it, the pty tests — which quit
  with `q`, which saves — would overwrite the developer's own session.

## Pane size and zoom

**Alt+Shift+arrows (or Alt+R, then arrows) move the nearest border in the arrow's
direction**, the way dragging it would: Left/Right the nearest vertical border, Up/Down the nearest horizontal one.
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
browser or Explorer; `Backspace` and `Enter` still work, and so do `Alt+B` / `Alt+F`,
which is what Terminal.app sends for ⌥← / ⌥→. `Alt+→` only ever enters a directory — on
a file it says so — because `Enter` already opens files, and a key that sometimes
navigates and sometimes opens a file is one you stop trusting.

Pane resizing moved from `Alt+←/→` to `Alt+Shift+arrows` to make room. Neither navigation
key is global, so with a shell focused, `Alt+←/→` still reach the shell for word motion.

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

**Keymap A** (owner, 2026-11-09): inside the shell xdevcmd takes only **F12, Shift+F12
and Alt+X** (`Action::is_global`); every other key is the program's, so readline, emacs,
mc, htop or Claude Code get every key they expect, F1…F11 included. Actions that act on
the shell (`Action::acts_on_shell`: split, close, tabs, Alt+1…9) are reached with Alt+X,
then their key; any other key after Alt+X goes to the program, with the Alt+X, as if
xdevcmd had not looked (`App::in_shell_prefix`). From a file pane every key does what it
always did. This is why `Ctrl+q` quits from a file pane but not from the shell, where it
is XON.

**Alt+X, then an F key, is Shift and that key** (Alt+X F8 is Shift+F8, Alt+X F10 the
right-click menu): for terminals that send no Shift+F keys, such as macOS Terminal.app.
**Shift+F10** (and the Menu key, where the terminal reports it) opens the right-click
menu for the row under the cursor, at that row.

**Between the panes and the shell** (Round 016): Alt+C sends `cd` to the shell, Alt+G
takes the pane to the shell's directory, Alt+Enter or Ctrl+Enter types the marked names
into the shell's command line, quoted for that shell. Nothing is typed into a shell that
is not surely at its prompt: xdevcmd offers a new shell instead, or asks (docs/shell.md).

`F12` was chosen as the escape from the shell pane because every other candidate collides
with something a shell uses: `Ctrl+\`` is not reported reliably by older terminals,
`Ctrl+J` is newline, `Ctrl+W` is word-erase. An escape hatch that sometimes types into
your shell instead is worse than an unfashionable key.

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

## Menu bar, usage, soak

**F9 is the menu bar**, as in Midnight Commander and Total Commander; the user menu moved
to Shift+F9 to make room. The bar is one row at the top (`[general] menu_bar`, default
on); F9 opens it even when it is hidden. Items are Actions, so a menu entry does exactly
what its key does, shows that key from the live keymap, and is greyed out by the same
`App::refused` that hides keys from the key bar. Left/Right move between menus
(`Outcome::Side`), a click on a title switches, and menus taller than the screen scroll.
Turning the bar on moved every row down by one: tests that read "the first line" now
read the second, and mouse tests were unaffected because they locate what they click.

**The bar's right end says which xdevcmd this is**: `xdevcmd <pid>`, dim, where the
menus leave room (Round 018). A shell started from a terminal panel inherits
`XDEVCMD_PANEL=<pid>`, and the question Windows asks before starting there names the same
number, so the two can be matched on screen. Usage (Shift+F1) shows it too.

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
