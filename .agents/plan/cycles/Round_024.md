# Round 024: F1 Commands, the command palette (v0.4.0, step 1, item 38)

**Status**: In Progress (the owner's go, 2026-11-12: "can go")
**Part of**: v0.4.0, step 1 of `ROADMAP.md` ("Next"); item 38, design C as decided by
the owner on 2026-11-12 (ROADMAP, "Notes on the items → 38 F1 Commands")
**Date started**: 2026-11-12
**Date completed**: YYYY-MM-DD

## Goal

Nothing to memorise: every command can be found by typing, from anywhere, with its key
shown beside it. F1 opens a small palette (a filter, the list below); F1 again makes it
the large window, which is today's Help without its tabs. Enter runs the command on
what had focus before. The loaded config moves to its own read-only window, Config in
use, and the Help menu is reshaped around both.

This round decides a newcomer's first five minutes, which is why it stays when v0.4.0
runs late (ROADMAP, "If it runs late").

## Plan (kickoff review)

### 1. What changed, and where things stand

- **Rounds 018–023 are at Review** on `feat/v0.4.0`. The owner's hand tests: Linux and
  Windows done (2026-11-12), macOS to come. CI green on all three (run 43181663296).
  This round starts with them still at Review, as the ROADMAP's step 0 allows; nothing
  here touches what they changed except the F1 screen tests.
- **The ROADMAP was rewritten around v0.4.0** the same day. The owner chose design C of
  four for F1 (note on item 38) and separated Config in use from it.
- **Today's F1** (`help.rs`, 571 lines) is a scrolling screen with two tabs:
  - **Keys**: keymap A's rule in a sentence, then `keys::grouped()` (87 bindings in 8
    groups), each row `keys · name · about`, a `•` on a rebound key, and "works in the
    shell" / "in the shell: Alt+x first"; then four fixed sections typed into the file:
    the viewer's keys, the mouse, safe operations, two-key bindings.
  - **Settings**: the config file's status (loaded / none / IGNORED and why), mode and
    root in effect, then the config as TOML with `•` on changed values (`[keys]` left
    out: it is on the other tab).
  - It cannot run anything. Tab, 1/2 and ←/→ switch tabs; F1, Esc and q close.
- **Item 19's fuzzy filter** exists and fits: `xdevcmd_core::fuzzy::score` (126 lines,
  fzf-style: word starts, runs and gaps scored, smart case), used by Ctrl+F in a pane
  with `[files] filter = "fuzzy"`.

### 2. Map: the code it touches

- **`keys.rs`** (1,659 lines), the one place for each key:
  - `Binding { action, name, group, about, keys }` and `BINDINGS` (87 rows). There is
    no display name: `name` is the `[keys]` action name (`copy_as`), which the
    compatibility promise will cover. F1 needs a **`title`** ("Copy as…") that may
    change freely (the note on item 38).
  - `GROUP_ORDER` and `grouped()`: F1 and `--print-keys` both read them; keep them.
  - `Action::is_global()` (F12, Shift+F12: keymap A) and `acts_on_shell()` (what Alt+X
    then a key does in the shell). **`Alt+X F1` does not open help from the shell
    today**: `in_shell_prefix` runs only those two kinds, and `help` is neither. And
    `usage` has `Alt+x F1` as a default (for terminals without Shift+F1), so in a pane
    Alt+X F1 is Usage. See decision 1.
  - `Keymap::apply`: the `[keys]` checks. Keymap A's rule for `is_global` is here; the
    new "`help` keeps F1" check goes beside it.
  - `shell_rule()` and `markdown()` (`--print-keys`, `docs/keys.md`).
  - Help's row `about`: "This screen: every key and setting", which becomes untrue.
- **`help.rs`**: `Help` (tabs, scroll, wrap), `keys_page`, `settings_page`,
  `sectioned`. It splits in two:
  - **`commands.rs`** (new): the palette. Rows, ranking, context, the two sizes, its
    keys and mouse. `keys_page`'s logic (rebound `•`, "in the shell: …" hints, the `…`
    for Alt+1 … Alt+9) moves into the row builder; the fixed sections become read-only
    rows.
  - **`config_view.rs`** (new, or what is left of `help.rs`): Config in use, from
    `settings_page`, plus a filter.
- **`app/input.rs`**: `on_key` (modal first, then groups, then keymap A),
  `in_shell_prefix`, `group_hint` (lists the keys after Alt+X in the shell: F1 must
  appear), `refused()` (the one place that says why a mode forbids an action),
  `perform()` (archive refusals after `refused`; `Action::Help` builds `Help::new`),
  `run_bound` (GoTab needs the key's digit), `footer_key`.
- **`app/modals.rs`** `on_modal_key` and **`app/mouse.rs`** (`Modal::Help` at ~777):
  the new modal's keys and mouse. **`modal.rs`**: `Modal::Help(Box<Help>)` becomes
  `Modal::Commands(Box<Commands>)` and `Modal::Config(Box<ConfigView>)`;
  `frame_box`, `put`, `decor::bordered`/`notch` for the frame.
- **The viewer is a modal** (`Modal::View`) that takes every key, and F1 does nothing
  in it today. For F1 there to work, the palette must open **over** it and give it back
  on Esc: the palette keeps what it covered (`under: Option<Box<Modal>>`).
- **`app/draw.rs`**: the Help menu (`help: &[…]` at ~72: "Keys and settings", "Usage",
  then About added at `i == 2`); the key bar's `(Action::Help, "Help")`; `draw_status`
  for "Next time: F5".
- **`app/open.rs`**: `open_user_menu` (Shift+F9; `[[menu]]` as `Cmd::UserCommand(i)`)
  and `run_user_command`; its "no commands yet" message points at "F1, Settings".
- **`usermenu.rs`**: `[[menu]]` entries (label, key, template) and placeholders; read
  only for the rows.
- **Menu-only commands**: Settings…, Theme…, About, and the new Config in use have no
  `Action`, only a `Cmd`. They must be rows too, or "every command by typing" is false.
- **`config.rs`**: `[keys]` reaches `Keymap::apply`; `--check-config` reports its error.
- **`app/settings.rs`**: two messages name "F1 › Settings" (lines ~15, ~55).
- **`session.rs` / state**: where recent commands would be kept (decision 4).
- **`keyfix.rs`**: macOS Option typing characters; the hint it gives.
- **Tests that snapshot F1**:
  - `tests/render.rs`: `f1_lists_keys_and_tab_switches_to_settings` (~1120),
    `help_tabs_are_clickable` (~1291), the keymap A tests reading F1 at 150×150
    (~1351, ~1403), the `[keys] help = ["F1", "Alt+g k"]` test (~580),
    `docs_keys_md_is_what_print_keys_prints` (~4075).
  - `tests/pty.rs`: two Kitty-picture tests use F1 as "a dialog over it" (~499, ~538).
  - `keys.rs` unit tests on `is_global` / F12's family (~1548).
- **Docs**: `docs/guide.md` ("F1 lists every key and setting", ~237), `docs/config.md`
  ("F1 → Settings shows what is in use", ~19; ~213), `docs/keys.md` (regenerated),
  README if it names F1, CHANGELOG.

### 3. Outside check (2026-11-12)

- **VS Code's command palette**: one flat list, the key shown at the right of each
  row, "recently used" on top with an empty filter, then the rest. A command
  unavailable in the context is hidden, not greyed. Its UX guidelines ask for a
  category prefix on each command ("Git: Commit"). Taken: the key at the right,
  recent first, the group tag. Not taken: hiding; here the rest goes under
  "elsewhere", since a hidden command is one nobody learns exists.
- **Zed's command palette**: sorts by use count, then name (#2954), and keeps its
  history across restarts (#26948). Taken: recent use as the tie-break after the
  match's score, kept across restarts (decision 4).
- **Sublime Text's palette and Helix's pickers** (`Space ?` lists commands with keys):
  type to narrow, a fuzzy score with word starts weighted; Helix has no recents. Both
  open at the top of the screen, a fixed width, not over the whole window. Taken for
  the small size.
- **Total Commander's F1** opens the manual; its command list is `cm_` names in a
  separate dialog. **mc's F1** is an info-style manual with links. Neither runs a
  command from help. The large size keeps mc's "browse by group" reading, which the
  current F1 already does.
- **Ranking**: reuse `xdevcmd_core::fuzzy::score` (item 19). It is the same feel as
  Ctrl+F, smart case included, and needs nothing new (the 7 MB budget). On top:
  - a key typed as text (`f5`, `alt+j`) matches the Key column exactly and ranks first;
  - the title is scored; the group is a word of it (`Files: Copy`), so "files" finds
    the group;
  - the description only when the title does not match, ranked below every title
    match;
  - ties: recent first, then the context's rows, then table order.
- Sources:
  - [VS Code: Command Palette UX guidelines](https://code.visualstudio.com/api/ux-guidelines/command-palette)
  - [Zed: sort command palette entries by name and use count (#2954)](https://git.secluded.site/zed/commit/f66dd43a847fb52297045fd14487dd5f58b05fa4)
  - [Zed: persistence of command palette history (#26948)](https://git.secluded.site/zed/commit/16f625bd07eacd071ce21f669b6baf4847e20ab6)

### 4. Critique: what must be got right

- **Keymap A does not regress.** Inside the shell xdevcmd still takes only F12,
  Shift+F12 and the Alt+X prefix; F1 typed in the shell stays the program's (mc, htop
  and emacs use it). From the shell F1 is **Alt+X F1**: `in_shell_prefix` learns one
  more kind, "opens from the shell" (Help only), kept apart from `is_global` so the F12
  family rule and its tests do not move. The keymap A render tests must pass unchanged
  apart from the added "Alt+x F1" in the hint.
- **F1 is always kept.** `[keys] help = ["Alt+g k"]` (F1 gone) is refused by `apply`
  with a reason, as keymap A's rule is: "[keys] help: F1 is always kept; add keys
  beside it". Keys added beside it act in the panes only (they are not global).
- **Context is decided when F1 is pressed**, from the focus: a file pane (a directory,
  a list or results tab, an archive), the shell, the viewer. Each `Action` gets a
  `where` (file pane, terminal, everywhere) beside `is_global`, in an exhaustive
  `match`, so a new action cannot be forgotten. An archive takes the file-pane rows,
  with the writes refused for the reason `perform` gives today.
- **"On what had focus before"**: the palette is a modal and focus does not move under
  it, so Enter closes it and calls `run_bound` as if the key had been pressed. Three
  cases need care:
  - **from the shell**, a terminal action (split, new tab) acts on the shell; a
    file-pane action (Copy) acts on the file pane the shell was entered from (the
    `mru`, as F12 back does), and focus moves there, so the user sees it;
  - **from the viewer**, a viewer row is read-only (its keys are the viewer's own);
    any other command closes the viewer first, as its Esc would, then runs;
  - **Go to tab** needs a digit: the row says "Alt+1 … Alt+9" and is read-only; the
    per-tab rows are not listed nine times.
- **Read-only rows** (mouse gestures, the viewer's own keys, the terminal tab bar, the
  two-key notes, safe operations) are shown, dimmed in the Key column's style, and
  Enter on one says why it cannot run ("a mouse gesture" / "the viewer's own key: press
  it there"). They never close the palette.
- **Refused commands** (read-only or restricted mode, inside an archive) are shown with
  the reason in their description column, in the theme's dim colour, and Enter says the
  reason on the palette's last line rather than closing. The reason comes from
  `refused()` and the archive check, not a copy of them: one place decides.
- **Alt+1 … Alt+9 run the first nine rows**; the row shows its number in the margin.
  Digits alone go into the filter. On macOS Terminal.app without "Use Option as Meta"
  Option+1 types `¡`: that goes into the filter (no match), and `keyfix`'s hint is
  shown once, as in the panes. Not mapped by layout: Option's characters differ per
  keyboard. ↑/↓ and Enter work everywhere.
- **Two sizes, one component.** One `Commands` with a `size: Small | Large`; the
  filter, selection and scroll are the same state, so F1 expanding keeps the text
  typed and the selected row.
  - Small: top-centre, width `min(72, screen − 4)`, up to 12 rows; empty filter shows
    recent, then this context's rows.
  - Large: today's Help area; empty filter shows the groups (browse), then the
    read-only sections.
  - **A small screen stays small**: when the large size would not be larger (the
    screen under about 90×30), F1 does not change the size; the palette is already as
    big as it can be. The order of F1 presses: small → large → closed; on a small
    screen, small → closed.
- **Narrow widths**: Name · Key · Description at 70 columns and more; under that
  Name · Key, with the selected row's description on the last line. Under 30 columns,
  Name only. Keys that do not fit are cut with `…`, never wrapped into the next row.
- **Esc**: clears the filter if it has text, then closes. **Tab** and digits are no
  longer tab switches (there are no tabs).
- **"Next time: F5"**: after a command that has a key is run from the palette, the
  status line says "Next time: F5" (the key that works where it ran: "Alt+x Alt+s" from
  the shell). Not said for a command with no key, nor when the palette was opened with
  the mouse-only Help menu (it says the menu's own key instead). Shown once per run,
  replaced by the command's own message if it has one.
- **`[[menu]]` entries are rows**, group "Your commands", their `key` from the config
  in the Key column; Enter runs `run_user_command(i)`, the same quoting path as
  Shift+F9. Restricted mode refuses them, as `UserMenu` is refused today.
- **Performance**: about 87 bindings + 40 read-only + menu-only + `[[menu]]` rows,
  ~150–200. Scoring them on each key is microseconds; the list is built once per open
  (keymap, mode and config cannot change while it is open) and re-ranked per key. A
  unit test ranks 2,000 rows under 5 ms in debug, to keep it honest.
- **Theme and decor**: frame by `decor::bordered` and `notch`, the theme's `accent`
  for matched letters (bold where 16 colours), `fg_dim` for read-only and "elsewhere",
  `mark` for the rebound `•`. Nothing new in theme files.
- **Mouse**: click selects a row, double-click runs it, the wheel scrolls; a click
  outside the palette closes it (as menus do); a click on the filter line does nothing
  special. Pictures (Kitty) are hidden under it as under any modal.
- **Config in use** is the old Settings tab as its own window, with a filter that keeps
  section headers of matching lines. Read-only; Enter does nothing yet (0.5). Opened
  from Help → Config in use and from its palette row; no key.
- **The Help menu**: **Commands…** (F1, opens the large size) · **Config in use** (no
  key) · **Usage** (Shift+F1) · ─ · **About xdevcmd**. Documentation is not added now:
  it comes with item 40 (Round 026), and a dead item for two rounds is worse than none.
- **Display names are not the compatibility promise.** `title` is separate from `name`;
  `--print-keys` keeps the `about` column and adds nothing that would make `title` look
  stable.

### 5. Steps

Each step is one commit with its tests. Sizes: S a day or less, M two to three days.

1. **Rows and ranking** (`keys.rs`, new `commands.rs`, no UI): a `title` on every
   `Binding` (87, by hand, unique, a test says so); `Action::place()` (file pane,
   terminal, everywhere, viewer) by exhaustive match; `opens_from_shell()` for Help;
   the row builder (bindings with live keys and the rebound `•`, menu-only `Cmd`s,
   `[[menu]]`, read-only sections moved from `help.rs`); ranking on
   `fuzzy::score` as in §3. Unit tests: ranking (exact key first, title over
   description, group word, smart case, ties by recent then context), context per
   focus, every action has a place, read-only rows never run, 2,000 rows under 5 ms.
   **M.**
2. **The palette, small and large** (`commands.rs` render and keys, `Modal::Commands`
   replacing `Modal::Help`, `on_modal_key`, `mouse.rs`): filter (`LineEdit`), ↑/↓,
   PageUp/PageDown, Esc twice, F1 to expand keeping the text, small-screen rule,
   narrow layouts, groups when empty (large), recent + context (small), "elsewhere".
   Render tests: both sizes at 118×34, the small one at 80×24 and 60×20 (stays small),
   narrow 50 columns (description on the last line), F1 F1 keeps the text, a refused
   row with its reason in read-only mode, a read-only row, a click and a double-click
   located from a first screenshot, the wheel. The old F1 render tests move to the
   large size. **M.**
3. **Running** (`app/input.rs`): Enter / double-click / Alt+1…9 through `run_bound`;
   the shell's mru file pane for file-pane actions; over the viewer (`under`, Esc gives
   it back); refused and read-only on the last line; "Next time: …" on the status line;
   `[[menu]]` rows through `run_user_command`; recent commands kept (decision 4). Tests:
   render for "Next time: F5" and a refused row; **pty**: F1, type "copy", Enter copies
   the file under the cursor of the pane that had focus; from the shell **Alt+X F1**
   opens it and F1 alone reaches the program (a `read -n` echo); **Alt+1** runs the
   first row; a `[[menu]]` row types its command into the terminal. **M.**
4. **Keys and config** (`keys.rs` `apply`, `in_shell_prefix`, `group_hint`): `help`
   always keeps F1 (refused otherwise, `--check-config` says why); Alt+X F1 opens F1
   from the shell and from the panes (decision 1); `usage` keeps Shift+F1. Tests: unit
   on `apply` refusing `help = ["Alt+g k"]` and accepting `["F1", "Alt+g k"]`; render:
   the existing `[keys] help` test, keymap A's tests with "Alt+x F1" in the hint; pty
   from step 3 covers the shell. **S.**
5. **Config in use and the Help menu** (`config_view.rs` from `settings_page`,
   `Modal::Config`, `app/draw.rs`, `app/open.rs` and `app/settings.rs` messages): the
   window with its filter; the menu reshaped as in §4; About unchanged. Render tests:
   the menu's items and keys, Config in use with a changed value marked and filtered to
   one section, the "IGNORED" status. **S.**
6. **Docs**: `docs/guide.md` (F1 Commands, Config in use, Alt+X F1), `docs/config.md`
   (F1 kept; Config in use), `help`'s `about` and `docs/keys.md` regenerated
   (`--print-keys`), README if it names F1, CHANGELOG; the two Kitty pty tests checked
   (any modal hides the picture; if the small palette changes that, they press F1
   twice). **S.**

About five to six days, the ROADMAP's M. Clippy for Linux, windows-gnu and darwin, and
the full suite, at each step.

### 6. The owner's decisions

Settled (2026-11-12): design C as in the ROADMAP note; Config in use separate; "can
go". Open points below; **the coder follows each recommendation unless the owner
objects**, and nothing waits on them.

1. **Alt+X F1.** Today it is Usage's second key (for terminals without Shift+F1). The
   design gives it to F1 Commands from the shell. *Recommended:* Alt+X F1 opens F1 in
   the panes and the shell alike; Usage keeps Shift+F1 and is a row in F1 and in the
   Help menu. One meaning for one key, and Usage is still findable by typing. A user's
   own `usage = ["Alt+x F1"]` keeps working (theirs wins).
2. **F1's presses on a small screen.** *Recommended:* small → large → closed; on a
   screen where large is no larger (under about 90×30), small → closed.
3. **File-pane commands from the shell.** *Recommended:* they act on the file pane the
   shell was entered from, and focus moves there; terminal commands act on the shell.
   Alternative: refuse them from the shell ("F12 to the panes first"), which is safer
   but makes the palette half-useful there.
4. **Recent commands kept across restarts.** *Recommended:* yes, the last 8 by
   `[keys]` action name (or `[[menu]]` label) in the state file beside the session;
   unknown names dropped on load. Nothing private: commands, not files.
5. **Documentation in the Help menu.** *Recommended:* not added now; Round 026 (item
   40) adds it with the link that works.
6. **Display names.** *Recommended:* a hand-written `title` on each binding, written in
   step 1, shown to the owner in the step's commit; not part of the compatibility
   promise.

### Not doing (this round)

- Enter on a row of Config in use opening Settings at that setting: 0.5, after F1 has
  met users (ROADMAP, Later).
- which-key for the prefix groups: item 39, v0.4.0 step 7 if a day is spare.
- Useful commands (examples and your own, readiness): item 41, 0.5; its rows would join
  F1's "Your commands".
- Help → Documentation: item 40, Round 026.
- Keymap presets, a palette for files (Ctrl+P-style "go to file"): not planned here.

## Do

- **2026-11-12, step 1: rows and ranking** (`keys.rs`, new `commands.rs`, core `fuzzy.rs`).
  - A hand-written `title` on all 87 bindings, unique (a test). Help's is "Commands",
    `cd_terminal_here` "cd the shell here"; most others are the menu's own label
    ("Copy as…", "Rename many…"), so F1 and F9 call a command the same.
  - `Action::place()` (Files, Terminal, Viewer, Everywhere) by exhaustive match;
    `opens_from_shell()` (Help only), kept apart from `is_global`.
  - `commands::rows`: bindings by group with live keys, the rebound `•`, the "in the
    shell: …" note; Settings…, Theme… and About at the end of General; `[[menu]]` as
    "Your commands" with `Shift+F9 e` in the Key column; the viewer, mouse, safe
    operations and two-key sections moved from `help.rs` as read-only rows. Go to tab
    N is read-only with `Alt+1 … Alt+9`.
  - Ranking, one deviation from §3: fuzzy is loose ("trash" matches "Terminal: Show
    or hide terminal" letter by letter), so a name match counts above a description
    only when it reads as words typed (each letter a word start or after the last
    one, `fuzzy::is_words`). Tiers: key typed as text (two characters or more, so
    "o" is not the key o) · name as words · description as words · name scattered.
    A description matched only scattered is dropped. Ties as planned.
  - `fuzzy`: `Query` prepared once, `score_positions` (the hits to highlight) with no
    allocation for a name that does not match; the quick filter's scores unchanged
    (its four tests pass as they were).
  - **Measured**: ranking 2,000 rows, debug, best of 5 over five queries: 4.0 ms
    first, 2.2 ms after `Query` and an ASCII fold; release 0.34 ms (before those).
    F1's real list is ~180 rows.
  - The module is unused until step 2: `allow(dead_code)` outside tests, removed then.
  - Not verified: clippy for windows-gnu and darwin — `rustup target add` cannot
    download here (proxy refuses the tunnel); CI checks them.
- **2026-11-12, step 2: the palette, small and large** (`commands.rs` `Commands`,
  `Modal::Commands`, `app/palette.rs`, `modals.rs`, `mouse.rs`).
  - F1 opens the small size: top-centre under the menu bar, `min(72, width − 4)` wide,
    up to 12 rows, the filter on top, a last line (bar colour) below. F1 again: the
    whole window, keymap A's two sentences above the groups. Below 90×30 F1 stays
    small and the second F1 closes; "F1 larger" is said only where it would grow.
  - Small, empty filter: recent (step 3), then this place's rows (the Files group
    first in a file pane), then the rest under a dim "elsewhere". F1 itself is left
    out of that list. Large, empty: every group under its name, as the old F1.
  - Rows: number (first nine) · rebound `•` · Name · Key · Description at 70 inner
    columns and more; Name · Key under 70, the selected description on the last line;
    Name alone under 30. Keys and descriptions are cut with `…`, never wrapped; a
    description cut on its row is said whole on the last line. A refused row shows its
    reason in the description column, dimmed; read-only and "elsewhere" rows dimmed.
    The large size's last line names the selected row's `[keys]` name, since the
    name column is gone and rebinding needs it.
  - Keys: ↑/↓, PageUp/PageDown; Home/End go to the list's ends while the filter is
    empty (with text they move along it); Esc clears, then closes; Tab does nothing.
    Mouse: click selects, double-click runs, the wheel scrolls (the selection follows
    into view), a click outside closes.
  - The filter also looks at the Key column, so "double-click" finds the mouse rows.
  - **Deviation**: Help menu already has Commands… (large) and Config in use, the
    latter opening the old Help on its Settings page until step 5 replaces it, so the
    settings stay reachable in every commit.
  - Tests updated honestly: the old "F1 then Tab" settings test now goes through
    Help → Config in use and finds the rebound `• … Copy … F2` by typing; the help-tab
    click test became F1's click/double-click/outside test; the 80-column wrap test
    became "cut on the row, whole on the last line"; keymap A's tests read the large
    size (F1 F1) and find rows by title. The sixel pty test waited for "Keys".

## Check

- [ ] Keymap A unchanged inside the shell: F1 typed there reaches the program; Alt+X F1
  opens F1 (pty)
- [ ] `help` cannot lose F1 (`apply`, `--check-config`)
- [ ] Context: the rows for where F1 was pressed come first, from a file pane, a list,
  an archive, the shell and the viewer
- [ ] Enter runs on what had focus before, including from the shell and over the viewer
  (pty)
- [ ] Read-only rows never run; refused rows say why
- [ ] Alt+1…9 runs the first nine rows; digits go to the filter
- [ ] Both sizes, narrow widths, a small screen staying small (render)
- [ ] F1 expanding keeps the filter's text; Esc clears, then closes
- [ ] "Next time: F5" after a command with a key
- [ ] `[[menu]]` rows run through the same quoting as Shift+F9
- [ ] Config in use shows what Settings showed, with a filter; the Help menu reshaped
- [ ] Ranking 2,000 rows under 5 ms (debug)
- [ ] Docs and `docs/keys.md` regenerated; clippy on three targets; the full suite
- [ ] The owner: F1 on macOS Terminal.app (Alt+1 without Option as Meta), Windows
  Terminal and a Linux terminal, by hand

Record what could **not** be verified as plainly as what could.

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → context/ : `project.md`'s "One place for each fact" names F1 among the readers
  of `BINDINGS`; check it still reads right with `title`
- [ ] → reference/ui.md : F1 Commands and Config in use
