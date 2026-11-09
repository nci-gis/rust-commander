# Round 014: Sharp image previews

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), item 20
**Date started**: 2026-11-08 (owner: "Yes, next round", after 0.2.1 was published)
**Date completed**: —

## Goal

Quick view (Alt+Q) shows pictures with real pixels where the terminal can draw them —
the Kitty graphics protocol (kitty, Ghostty), iTerm2 inline images (iTerm2, WezTerm,
VS Code) and sixel (Windows Terminal 1.22+, foot, xterm) — and keeps today's half
blocks everywhere else, macOS Terminal.app included.

## Plan

Decision for the owner first — how to speak the protocols:

| | A. `ratatui-image` 11 (MIT) | B. Our own, about 400 lines |
|---|---|---|
| Measured (2026-11-08, trial add, reverted) | +66 crates across targets, among them `image` 0.25, `windows` 0.58, a second `rustix` (0.38) and `rand` (0.8); licences pass `cargo deny` | +1 crate for sixel: `icy_sixel` 0.7 (MIT OR Apache-2.0; needs `quantette`, `thiserror`); Kitty and iTerm2 are a few escape sequences over the decoder and scaler xdevcmd already has (`png`, `zune-jpeg`) |
| Detection | queries the terminal (font size, protocol) | environment first (`KITTY_WINDOW_ID`, `TERM_PROGRAM`, `WT_SESSION`…), a query where that is not enough, and a setting to choose or turn off |
| Fit with xdevcmd's redraw-on-change loop and the terminal panel | its widget redraws as it likes; to be checked | written for it: drawn once per change, removed under dialogs |

Recommended: **B**, for one small static binary and full control of when pictures are
drawn and removed.

Then, either way:

- [x] Detect what the terminal speaks; `[viewer] images = "auto" | "kitty" | "iterm2" |
      "sixel" | "blocks"` to choose (also in Settings), "blocks" as before.
- [x] Draw the picture once when it changes; remove it when a dialog opens over it, the
      pane closes, the cursor leaves the file, or the window resizes.
- [x] Never inside the terminal panel's area; never in screenshots (the tests).
- [ ] Over SSH: the local terminal's protocol arrives through SSH, so detection must not
      trust only local environment variables; inside tmux, passthrough or blocks.
- [x] Tests: the escape sequences for each protocol (unit), detection from the
      environment, blocks unchanged where nothing is supported; pty tests read the bytes
      sent: a Kitty picture sent once, deleted under a dialog and sent again after;
      iTerm2 and sixel drawn, repainted away under a dialog, drawn again.
- [ ] By hand, by the owner: iTerm2 or Ghostty on macOS, Windows Terminal, and
      Terminal.app (blocks, as now).

Added by the owner before the next release (2026-11-08), not started:

- ~~Quit while the terminal has focus~~ — dropped by the owner: "behavior ko consistent
  thì dễ confuse hơn là hữu ích". F12, then F10, stays the way out.
- [ ] **Running jobs at the end, two cases kept apart** (owner: "Phân biệt … thoát app
      properly … tắt terminal"). The debate, and the proposal:

  **What can be done with a file being written when xdevcmd stops**

  | | Way | A kill (`kill -9`, power cut) leaves | Litter | Used by |
  |---|---|---|---|---|
  | A | write under the real name (today, for a new file) | a truncated file **under its real name**, looking complete | none | Total Commander, `cp` |
  | B | write under a temporary name beside it, rename at the end (today, for a replacement and for extraction) | a hidden `.name.xdevcmd-part-PID-N`; the real name absent or untouched | a hidden file per interrupted copy | rsync, FreeFileSync (`.ffs_tmp`) |
  | C | write under the real name with an impossible date until done | a truncated file under its real name, with a 1980 date | none | robocopy (and `/Z` to resume) |
  | D | catch the signal and clean up | — only for what can be caught: SIGHUP (terminal closed, SSH lost), SIGTERM, Windows' close event (5 s to finish); never `kill -9` or a power cut | — | most daemons |

  Proposed: **B for every copy, plus a journal, plus D**. B is the only way a wrong file
  never sits under a real name, whatever kills xdevcmd; its litter is handled by a
  journal: each temporary name (and rename-many's temporary names, which are the
  user's own files under another name) is noted in the state directory before it is
  made and crossed out once renamed or removed; at the next start, entries whose
  process is gone are cleaned — a part file removed, a rename-many name put back. D
  makes the usual cases tidy at once.

  **Quitting properly (F10, q, Ctrl+Q) with jobs running** — today: "Quit? … cancel
  them and quit"; then up to 5 s for the workers to remove their part files, and xdevcmd
  quits even if one is still stuck (a hung network drive). Proposed: keep the question;
  then "stopping…" until every worker has really finished cleaning; after 5 s, ask
  again ("still stopping: quit anyway?"), and what is left goes to the journal. The
  session is saved.

  **The terminal closed under a job** — nobody to ask, the screen gone. Proposed: cancel
  every job, let workers remove their part files (at most 2 s; Windows allows 5), save
  the session, exit without touching the terminal. Not "finish in the background"
  (nobody could answer an overwrite question, and nobody would know it is still
  running); perhaps later as a setting for long copies over SSH.

- [ ] **Safe operations, the owner's shape** (2026-11-08: "thêm 1 settings … Safe
      Operation … nếu bật thì mới dùng đề xuất … ngược lại dùng như hiện tại"; Help says
      which functions it covers; the docs say how it works, its limits, and force kill;
      and perhaps "session state … lần khởi động tiếp theo có thể báo, hỏi người dùng
      tiếp tục"). Proposed refinement, waiting on the owner:
  - `[general] safe_operations = true` (Settings → General). **On**: every copy writes a
    temporary name and renames at the end; a journal in the state directory notes each
    temporary name and each interrupted job; the next start cleans what a dead run left
    and offers to resume. **Off**: as 0.2.1.
  - Kept **always**, setting or not, because turning them off would bring a data loss
    back, at no gain: a replacement written under a temporary name (0.2.1's fix: a
    cancel used to lose old and new), extraction the same, the Linux trash's record
    first, renames that never replace. And catching SIGHUP, SIGTERM and Windows' close
    event to cancel, clean and save the session: it only tidies up.
  - Covered, said in Help (F1) and the docs: F5 copy, F6 move (across drives: copy then
    delete per file), Shift+F5 copy as, Alt+Y sync, F5/Alt+F9 extract, Alt+F5 pack,
    Shift+F2 rename many (its temporary names are the user's files: put back, never
    removed). Not coverable: Shift+F8 (nothing to keep), F4 (the editor writes).
  - **Resume** looks feasible thanks to item 18: the journal keeps the job (kind,
    sources, destination); at the next start "A copy of 120 files to … was interrupted.
    Continue / Discard"; Continue runs the same job with "replace only if older", so
    what was done is skipped in a moment and the file cut short starts again. A move
    resumes on the sources still there; a delete on what is left. Not resumed: a file
    from its middle (whole files only), extraction and packing (started again).
  - Docs (a "Safe operations" section in docs/security.md, a line in the guide): how it
    works; what force kill (`kill -9`, power cut) leaves and how the next start cleans
    it; limits — space for a replacement until it is renamed, file systems where a
    rename into place is not allowed or not atomic (some network and FUSE ones), part
    files not hidden on Windows (a leading dot hides nothing there), a run that never
    starts again on that machine leaves its part files.

Added for 0.3.0 by the owner ("đồng ý với đề xuất", 2026-11-08), kept on this local
branch until the owner has used 0.2.1 for a week or two; not started:

- [x] **A crash log**: on a panic, write what happened (message, place, version, system)
      to the state directory and say where it is and how to report it. No network.
- [x] **Shell completions and a man page**: `xdevcmd --print-completions
      bash|zsh|fish|powershell` and `--print-man`, ready for the packages to install.
- [x] **Windows' console close event** — not done: no safe way (see Do) for safe operations' tidy end, if a safe way
      exists (the crates forbid `unsafe`); otherwise left as documented (the next start
      cleans up).
- [x] **About: version and the releases page link** (no update check: no network;
      package managers update).
- Deferred, as decided: pinning to the taskbar or Dock (item 14), checking for updates
  (needs the network, which xdevcmd never uses).
- [x] **The context menu from the keyboard** (owner: "yes, all of them") — Shift+F10
      (Windows' key for it, free here) and the Menu key where the terminal reports it;
      opened at the cursor's row in the focused pane, beside the name, as Explorer
      does for the selected item, above the row when there is no room below. Files
      panes only: in the terminal panel the key belongs to the program.

The owner's notes on the terminal panel (2026-11-09), to weigh; not started:

- Scrolling: the wheel over a shell scrolls its history (tested,
  `the_wheel_scrolls_back_through_shell_history`), 3 lines a notch; there is no key for
  it (PgUp goes to the shell). **Done 2026-11-09** for full-screen programs, after the
  owner found Claude Code in the panel would not scroll: the wheel goes to the program
  as a mouse report when it asked for the mouse (SGR, UTF-8 or X10, as it asked), as
  three arrow keys on the other screen without the mouse (less, man; application-cursor
  aware), otherwise history. Tests: `the_wheel_goes_to_a_program_that_asked_for_the_mouse`,
  `the_wheel_is_arrow_keys_to_a_full_screen_program_without_the_mouse`. Still proposed:
  Shift+PgUp/PgDn/Home/End for the history.
- Copying: no selection in the panel at all, and xdevcmd holding the mouse keeps the
  outer terminal's own selection away (Shift-drag gets it in Windows Terminal, GNOME,
  kitty; Option-drag in iTerm2 — but across both panes, not just the shell). Proposed:
  drag to select in the shell, copy on release through OSC 52, as the viewer does.
  OSC 52 depends on the outer terminal: Windows Terminal, kitty, WezTerm, iTerm2 (with
  its clipboard setting) take it; Terminal.app and VTE terminals do not — saving to a
  file is the way there.
- History length: 5000 lines, fixed in code. Proposed: `[terminal] scrollback`, in
  Settings. vt100 keeps 32 bytes a cell: 10,000 lines of 120 columns is ~38 MB per shell
  when full, 200 columns ~64 MB.
- Copy all / save the history to a file from a menu on the panel; a PuTTY-like log of
  everything a shell prints (low priority, the owner said).

### Plan for the terminal panel (2026-11-09, agreed in outline; not started)

The owner's frame: these are the limits of a terminal inside a terminal, as over SSH —
xdevcmd has only what the outer terminal gives it. tmux meets the same limits with the
same answers (`history-limit`, copy mode, `pipe-pane` logging), which is a check on the
direction. Owner's decisions: history at most **9999** lines; the menu **on the tab
only**, never in the shell's area (the user should not have to know when a right-click
belongs to the program and when to xdevcmd); the menu holds **only what matters**.

**Step 1 — History view (copy mode).** The biggest gain; reuses the F3 viewer.
- **Alt+V** in the panel (free: not bound, and readline leaves M-v alone) opens the
  focused shell's history and screen as plain text in the viewer, at the bottom (where
  you were). There: drag selects lines, `y` copies, `Y` the screen, `/` `n` `N` search,
  Esc back to the shell. The program keeps the mouse the rest of the time.
- Text from vt100 page by page: scrollback offset from the top down to 0, the visible
  rows of each, joined where `row_wrapped` says the line went on. Trailing blanks cut.
- **In memory, not a temporary file**: a shell's output can hold tokens and customer
  data, and nothing of it should touch the disk unless the user saves it. So
  `Document` (core) gets a second source, bytes in memory, beside the file; its line
  index and search work on either.
- One viewer, not a second one (owner agreed): selection, search, scrollbar and keys
  are already there and tested.
- Tests: pty — `seq 1 300`, Alt+V, the viewer shows `1` at the top after Home and
  `300` at the bottom; `y` after a drag sends OSC 52 with the lines; Esc returns with
  the shell still typing.

**Step 2 — Save as… and Go to line in the viewer.** For every document, not only history.
- **Save as…** (F2 in the viewer; free there): asks for a path, prefilled with
  `<shell's directory>/xdevcmd-history-<date>-<time>.txt` for history and the file's own
  name for a file. Refuses to overwrite without asking (the same question as copy);
  written `0600` for history. Read-only and restricted mode: refused, as other writes.
  This also covers the owner's "export / copy all", and is the way out where OSC 52 is
  missing (macOS Terminal.app over SSH).
- **Go to line**: `:` (as less and vim) and Ctrl+G (as Ctrl+G goes to a path in the
  panes). Takes a line number or a percentage (`50%`); out of range goes to the end and
  says so. In hex view, an offset.
- Tests: render — `:` 40 Enter puts line 40 at the top; `50%`; pty — F2 writes the file,
  a second F2 to the same name asks first.

**Step 3 — `[terminal] scrollback`.** Default stays 5000; 100 to **9999** (owner). In
Settings with the memory it can take when full, worked out from the panel's width
(vt100 keeps 32 bytes a cell: 9999 lines × 120 columns ≈ 38 MB per shell). Applies to
shells started after the change (vt100 cannot resize its history in place); Settings
says so. Test: config parse bounds; a shell started with 200 keeps 200 lines.

**Step 4 — The menu on a terminal tab.** Right-click a tab (the bar is xdevcmd's, so no
guessing who gets the click). Only what matters:
- View history (Alt+V)
- Save history as…
- Close tab
Not in it: New tab and split (the + button and Alt+N/S/D already); Paste (xdevcmd
cannot read the clipboard over SSH, and the terminal's own paste, Ctrl+Shift+V or Cmd+V,
already works); Clear history (destructive, rarely wanted; `clear` in the shell). Each
item shows its key, so the menu also teaches the keys. Test: render — right-click a tab
shows the three items; Close tab closes that tab, not the focused one.

**Step 5 — Logging (later, off by default; weighed, not decided).**
- "Write only what overflows" cannot be done reliably: vt100 does not say when a line
  leaves its history, so it would be guesswork, with lines lost or doubled. The sound
  version writes **everything the shell prints**, escapes stripped, as PuTTY's log
  does; the file then holds the whole history, which is what the owner wants from it.
  Full-screen programs (Claude Code, htop) redraw constantly and log as noise.
- Where: not the current directory (logs scattered through projects, committed by
  mistake, and they can hold secrets): xdevcmd's state directory, `logs/`, files `0600`,
  a setting to choose another directory. Name: `<date>-<time>-tab<N>-<pid>.log`, never
  the same twice. A size cap per file and deletion after N days.
- Its own decision when the owner is ready; a menu item "Log this tab" would join step 4.

Order: 1, 2, 3, 4 (each its own commit with its tests), 5 only on the owner's word.

## Do

- 2026-11-09, **close-out done; set aside at Review for the owner's hand tests. Round
  015 starts from it** (on its own `feat/…` branch from this one). Commits on
  `feat/image-preview`, local, not pushed: `0fff462` pty tests' `$` prompt as root,
  `fa62a95` (1), `efdf77e` (2), `703b7d2` (3), `834adef` (4), `1dc7009` (5), `84fc8b8`
  (6), `695ce94` (7), `3b491ad` (8), `943d6b9` (9), `792a4e9` (10), then two found in
  step 11: "Read the terminal's input on its own thread" and "Cancel tests cancel a copy
  still running". Each fix proved by putting the bug back: the test failed, then passed.

  **Found in step 11.**
  - Step 6's hang-up test failed 5 of 15 runs alone: crossterm 0.29's reader, on a
    hung-up pty, loops on the failing read and never returns the error, so the loop
    never saw it. The terminal's input is now read on a thread of its own (`KeyReader`);
    the loop waits on a channel for one TICK and, on a timeout, polls stdin for
    HUP/ERR (Unix). The thread reads only when asked and the loop waits for it to be
    idle before `$EDITOR` takes the terminal, so it never takes the editor's keys. New
    pty test `keys_typed_while_the_editor_runs_reach_the_editor` (a line typed while
    the editor is still starting): 3/3 fail with a thread that reads all the time, 3/3
    pass as written. Hang-up test after: 15/15 alone, and in every full run below.
  - `with_safe_operations_a_cancelled_new_copy_leaves_nothing_under_any_name` (core)
    failed once in a full `cargo test --workspace`: 8 MiB copied completely before the
    test thread cancelled, and a complete copy is rightly kept. Under CPU load, 6/20
    lib runs failed before, 0/30 after: the source is now a sparse 1 GiB file (also in
    `cancelling_mid_file_removes_the_half_written_copy`, same window). Test only.

  **Measured.**
  - Resume flake, before: 2 failures in 20 alone, 14 in 20 within the full parallel pty
    run. A 50 ms sleep after the rename made it fail 5/5: the plan's cause is real. A
    second one showed too, the more frequent: `q` sent while the job box (which ignores
    q) was still up, "did not exit on q". Waiting for the summary ("1 copied") covers
    both. After: 0/20 alone, 20/20 full parallel runs clean (and 5 more after step 10).
  - Pictures, a 2048x2048 photo-like picture, release build, this machine: Kitty (PNG,
    base64) 6.75 MB, encode 60 ms; iTerm2 6.74 MB, 50 ms; sixel 2.24 MB, 239 ms (with
    heavier noise: 14.7 / 14.7 / 4.5 MB, 84 / 65 / 220 ms). That encode was on the
    drawing thread, per dialog closed and per file passed. Now on the decode thread;
    the frame writes cached bytes: 0.3–2.6 ms for the copy. Kitty after a dialog:
    `a=p`, about 40 bytes, instead of the whole picture again.

  **Deviations from the plan, and why.**
  - Before step 0: `a_directory_name_cannot_run…` and `the_terminal_answers_where…`
    failed here because dash, run as root, ignores an inherited `PS1` and prompts `# `.
    Fixed in the harness (`ENV` pointing at a file that sets `PS1`, which an interactive
    sh reads last), its own commit.
  - Step 4: the run tag is `<pid>x<start time in ns, hex>`. A journal with no `host`
    line (an older one) counts as this machine's. A held lock is also checked against
    the file's link count, so a journal removed by a job that just ended is never taken
    for a dead run's. `Journal::gone` and `touch` removed with `TOUCH`/`STALE`.
  - Step 6: `XDEVCMD_TEST_NO_SIGNALS` cannot skip the handlers outright: closing the
    pty sends SIGHUP, whose default would kill xdevcmd before any EIO. It registers
    them onto a flag nobody reads, so only the error can end the loop (the race the fix
    is for). The harness's reader stops at its next read so the last master copy closes.
    Without the fix the process did not end at all: the copy went on with no terminal.
    `edit_externally`'s error goes the same way as the others (the loop body is now
    `App::turn`).
  - Step 7: `on_panic(main) -> Steps { restore, tell, note }` (calling the previous hook
    is part of `restore`).
  - Step 8: applied to a protocol chosen by hand as well as `auto`. Windows Terminal
    counts as knowing its cell size (`WT_SESSION`): it maps sixel through a fixed
    virtual 10x20 cell, which is the guess; without this, sixel would never be used on
    Windows, where nothing gives the cell size. The iTerm2/sixel pty test now answers
    the cell size.
  - Step 9: the picture's key holds the box and the protocol as well as file, pixels and
    date. Kitty transmissions and placements carry `p=1`, so a placement replaces
    rather than adds. A resize or the editor's return sends the picture whole again.
    The quick-run test sends Downs 60 ms apart (a key repeat); sent all at once it
    passed even without the wait, since only the last decode was ever kept.

  **Not verified here.** Windows and macOS were not compiled: `rustup target add` was
  refused by this sandbox's proxy (static.rust-lang.org), so the Windows code paths
  (share-mode journal lock, `COMPUTERNAME`, the `Lock` enum's `cfg_attr`) were checked by
  reading only; CI or the owner's machines must build them. Also by hand: journal
  locking on Windows (kill from Task Manager), NFS, pictures on real terminals (a Kitty
  `a=p` after a dialog in kitty and Ghostty; sixel in Windows Terminal, which rests on its
  10x20 virtual cell), Ctrl+Space in a real shell; the input thread on Windows and
  macOS (Windows relies on crossterm's console reader reporting a closed console as an
  error; not tried). `cargo deny` is not installed here (no new crate; rustix gains its
  `system` feature, already in the tree).

  Recorded for the owner, not changed: global Alt keys (`is_global`) that readline also
  uses (Alt+D, Alt+C, Alt+T, Alt+N, Alt+1…9, the Alt+R group).

- 2026-11-09, **close-out planned** (owner: run the rounds one after another, each left at
  Review for the owner to close after testing by hand; each on its own `feat/…` branch,
  chained). An independent review found nine issues; each was checked against the code
  before planning; all nine are real. A tenth: **Ctrl+Space reaches a program in the
  terminal panel as a space**, not NUL (`send_key` maps Ctrl+letter to a control byte, `' '`
  is not a letter; Ctrl+@ the same). The owner's principle, the same day: xdevcmd
  complements the shell and never competes with it. Ctrl+R history search and Tab
  completion are the shell's job (bash, zsh, fish, PSReadLine, fzf, atuin). When a program
  runs in the panel, the program gets the keys and the mouse.

  Findings, as checked:
  1. `resume()` (app.rs ~5352) sets `Existing::Update`: a file the user chose to keep in the
     first run is replaced in the resumed run if its source is newer. Update does not even
     help the case it was chosen for (a new file written in place gets its date only at the
     end, so a file cut short carries *now* and Update skips it). Journals are only written
     with safe operations on. A new file reaches its real name only when complete, so Skip
     would be sound for new files, but not for a replacement cut short (old file still under
     its real name; Skip would keep it although the user agreed to replace it). Rule that
     never decides for the user: **skip what is identical (same size and date, within FAT's
     two seconds), ask about anything else already there** = new `Existing::Resume`. A
     cross-device move killed between copy and source removal leaves an identical copy and
     the source: Resume skips the copy, leaves the source, the summary says so.
     Also: `SAFE` is read per file, so toggling safe ops mid-job split a job — fixed by item 8.
  2. Liveness. Windows: alive by journal date (`STALE` = 120 s), not refreshed during long
     blocking steps. Unix: pid only. NFS-shared home → another machine's live job looks
     dead here; reused pid → dead run looks alive, and `tidy` finds part files by
     `.xdevcmd-part-<pid>-`, so a live run with that pid loses its part files.
     `File::lock` is stable from 1.89; `rust-version` is 1.88 — not raised. Instead, no unsafe:
     - Unix: `rustix::fs::flock(fd, NonBlockingLockExclusive)` on the journal file for the
       life of the job (rustix `fs` already a core dependency). Kernel drops it on death.
     - Windows: std `OpenOptionsExt::share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)` on the
       journal handle; another start's probe (open read+write, `share_mode(0)`) fails with a
       sharing violation while the job runs, succeeds once the process is gone.
       `FILE_SHARE_DELETE` lets the job remove its own journal with the handle open.
     - Journal gets a `host` line; a journal from another host is left alone. Temporary
       names get a **run tag** `<pid>x<start time, hex>` (journal file name, part files,
       rename-many names), so a reused pid never matches another run's files.
     - Where flock is unsupported (ENOLCK, EOPNOTSUPP): fall back to the pid check, only for
       journals from this host.
     - Unix creates `.<run>-<n>.journal.new`, locks it, renames to `<run>-<n>.journal`; a
       `.new` left unlocked is removed.
     - `TOUCH`, `STALE` and every `touch()` go; `ask()` waits with a plain `recv`.
  3. journal.rs:233 `line[5..]` panics on a bare `kind` line → panic at every start.
     Fix: `strip_prefix("kind ")`.
  4. No cell size from the terminal → `cell_px` defaults 10×20; sixel drawn pixel for pixel
     overflows on 8×16 cells. Kitty/iTerm2 fit to cells, safe.
  5. `sync_graphics` encodes (PNG / sixel) and writes up to 2048² on the drawing thread, again
     on every dialog close and for every file passed in a photo directory.
  6. `event::poll(TICK)?` / `event::read()?` (~695/698) return on EIO before the SIGHUP flag is
     seen: `end_on_signal` and `save_session` skipped. `terminal.draw(...)?`,
     `sync_graphics(...)?` the same.
  7. Panic hook (main.rs) restores the terminal and prints "xdevcmd crashed" from any thread;
     a background panic leaves the main loop running in a broken terminal; std's message
     printed over the screen.
  8. `ops::SAFE` process-global; a test flips it while other ops tests run in parallel.
  9. `an_interrupted_copy_is_tidied_up_and_resumed_at_the_next_start` fails 2/5. Likely: the
     resumed job keeps its journal until its worker is dropped, after the rename that makes
     `b.txt` appear; the test checks "journal gone" the moment `b.txt` is there.

  Close-out steps (each one commit with its tests, in this order), all done 2026-11-09:

  - [x] **0. Measure the flake first** (no commit). Run that pty test 20× alone, 20× within a
        full parallel `cargo test -p xdevcmd-tui --test pty`; record counts. Confirm cause with
        a scratch 50 ms sleep after the rename.
  - [x] **1. A truncated journal never crashes a start** (3). Test: every byte-prefix of a valid
        journal and garbage lines go through `tidy` without panic; fails with `line[5..]`.
  - [x] **2. Safe operations taken per job** (8). `Request.safe: bool`, filled from `ops::safe()`
        by constructors at job start, kept by the worker for journal and in-place decisions.
        `batch_rename::apply(moves, safe)`. No test writes the global. Test: two requests at once,
        one safe one not, each writes as asked.
  - [x] **3. Resume asks rather than decides** (1). `Existing::Resume`: skip a destination with the
        same size and date (`compare::newer_than == Equal` + equal length), ask about others;
        directories merge as before. `App::resume` uses it. Dialog line: "What was done is
        skipped; a file cut short starts again; you are asked about any other file already
        there." Core tests: identical skipped silently; a kept older different file is asked
        about and stays on Skip (fails with Update); cross-device move with identical copy leaves
        the source and says so. pty fixture gives `dst/a.txt` the source's date. docs/security.md
        and guide updated; CHANGELOG.
  - [x] **4. Liveness is a lock, not a date or bare pid** (2). As above. `hostname()` public in core
        (rustix `system` feature on Unix; `COMPUTERNAME` on Windows). Tests: journal held by a live
        `Journal` left alone even if pid not ours; unheld one recovered; other-host left alone
        (fails without fix); same-pid different-tag part file not removed (fails without fix);
        unlocked `.new` removed, held one not. pty fixtures gain `host`. docs/security.md: how a dead
        run is told from a live one, NFS limit. Not verifiable here: Windows, NFS.
  - [x] **5. The resume pty test waits for what it checks** (9). Wait for "copied", then wait
        (bounded by `PATIENCE`) until the journal dir is empty. Proof: 20/20 alone and in parallel.
  - [x] **6. Losing the terminal always cancels, cleans and saves** (6). Any error from poll, read,
        draw or sync_graphics in the loop → `end_on_signal()`, break, save session after the loop,
        return the error after the save. Hidden `XDEVCMD_TEST_NO_SIGNALS=1` skips registering
        handlers; harness `Session::hang_up()` drops writer and master. Pty test: autosave off, a
        large copy running, hang_up → process ends, session.toml holds the dir, no part file left;
        fails without the fix.
  - [x] **7. A background panic is logged, not the end** (7). Main thread id in a `OnceLock`. Main
        thread: as today. Other threads: write crash log (thread name, "xdevcmd went on"), no
        restore, no previous hook; put log path in a global that `background()` shows as "a
        background task failed — details in <log>". Decision as pure `on_panic(main: bool) -> Steps`,
        unit tested. Pty test via hidden `XDEVCMD_TEST_PANIC_THREAD=1`: screen still drawn, no
        "crashed", a crash-*.log exists, q quits.
  - [x] **8. Sixel only where the cell size is known** (4). `cell_known: bool`;
        `graphics::effective(protocol, cell_known)` → half blocks for sixel without it. Usage /
        start-up note says so. Unit test; pty test: terminal answering `CSI c` with 4 but not
        `CSI 16 t` gets no `ESC P…q`; existing sixel test still draws sixel.
  - [x] **9. Pictures prepared off the drawing thread, sent once the cursor settles** (5). Decode
        thread also encodes (knows box size), cached in `quick_image`; `sync_graphics` writes cached
        bytes only. Decoding starts after the cursor stays 120 ms; loop stays awake at TICK while
        pending. Kitty: transmit once (`a=T`); under a dialog delete placement only (`d=i`); on
        close place again (`a=p,i=<id>`); free (`d=I`) when the picture changes or pane closes.
        iTerm2/sixel resend cached bytes. Pty tests: Kitty open, dialog, close → exactly one `a=T`
        then `a=p`; five quick Downs over five pictures → one picture sent. Record bytes per
        protocol for a 2048² photo and draw time before/after in Do.
  - [x] **10. Ctrl+R, Tab, Ctrl+Space reach the program in the terminal panel.** Ctrl+Space and
        Ctrl+@ send NUL. `send_key` mapping → pure `key_bytes(KeyEvent) -> Option<Vec<u8>>`, unit
        tested (Ctrl+R → 0x12, Tab → 0x09, Ctrl+Space → 0x00, Alt+x → ESC x). Pty: in the panel
        `stty raw -echo; dd bs=1 count=3 2>/dev/null | od -An -c; stty sane`, then Ctrl+R, Tab,
        Ctrl+Space → screen shows `022  \t  \0` and xdevcmd's own Ctrl+R/Tab/Ctrl+Space didn't fire.
  - [x] **11. Check and hand over.** fmt; clippy Linux + `x86_64-pc-windows-gnu` +
        `aarch64-apple-darwin` (if targets install); `cargo test --workspace`; `cargo deny check`
        (if available). ROADMAP rows in. Status → **Review**, a Do line: set aside for the owner's
        hand tests; Round 015 starts from it.

- 2026-11-09, three of the owner's finds in the terminal panel. (1) The wheel did nothing
  in Claude Code running in the panel: it now goes to the program (see the scrolling
  note above). (2) On macOS the scrollbar's thumb was dashed: Terminal.app leaves space
  between lines, and a █ glyph does not fill it. The thumb's background is now its
  colour too, so the cell is filled either way; still █, so text screenshots and their
  tests see it. (3) xdevcmd started from its own panel ran inside itself. Its shells now
  carry `XDEVCMD_PANEL=<pid>`; a start that finds it, with that process alive (Unix asks
  the system; Windows takes the variable at its word), says "already running … F12 goes
  back to it" and exits 1. A shell that outlived its xdevcmd (tmux started in the panel)
  is not refused; `unset XDEVCMD_PANEL` overrides (not said in the message, at the
  owner's word). `--screenshot` and the print options
  are not refused. Test: `xdevcmd_will_not_start_inside_its_own_terminal_panel`.

- 2026-11-08, a bug the owner found in 0.2.1 while using it: in a pane split off below
  (Alt+D), clicking a directory in the title's path did nothing. That title sits on the
  boundary's row, and dividers are hit-tested before panes ("both border rows grab
  it"), so the click started a drag. A click on a crumb of a shown pane now wins, as the
  tab bar's buttons already did; the rest of the row still drags. Test:
  `a_click_on_the_title_of_a_pane_split_off_below_goes_there_too` (fails without the
  fix).

- 2026-11-08, **paused** at the owner's word: "hoạt động tốt trên macOS rồi nhé … tạm dừng
  develop ở đây, mình trải nghiệm v0.2.1 1-2 tuần sau đó get back". The owner confirmed
  on macOS the Alt+X keys and the terminal's name. Where it stands: everything is on
  `feat/image-preview`, pushed, no pull request, CI never run on it. When work resumes:
  run CI once (macOS, Windows, ARM), the owner tests (pictures on Windows Terminal and
  iTerm2 or WezTerm; a killed copy resumed; Safe operations), close the round, then the
  usual way to 0.3.0 (PR into dev, `cargo release minor`, PR into main, tag, draft).

- 2026-11-08, the owner on the F13–F20 conversion ("đây có phải cách xử lý đúng???"):
  it was not — +8 is Terminal.app's alone; urxvt and the Linux console add 10, so
  Shift+F6 there would have run Shift+F8's action. Reverted. Instead (owner's idea):
  an **Alt+X group**, Alt+X then F1…F10 doing what Shift+F1…F10 do, on every terminal,
  Terminal.app's missing Shift+F1–F4 included; and **xdevcmd names its terminal**:
  `TERM_PROGRAM`, `WT_SESSION` and the like, else XTVERSION (`CSI > 0 q`: name and
  version) or DA2 (`CSI > c`) asked with the start-up probe, else "an unknown terminal";
  ", over SSH" when it is. Shown on the status line at start (cleared by the first key
  or click — left there, it hid the pane's own line, and a pty test caught it), in
  Usage and in About.

- 2026-11-08, 0.3.0's small additions (owner: "yes, all of them … push nhánh lên github",
  no pull request): Shift+F10 and the Menu key open the right-click menu at the cursor's
  row (`Menu.row`: below it, or above when the bottom has no room); `crash.rs` writes
  `crash-<secs>-<pid>.log` to the state directory from the panic hook, keeps five, and
  says where once the terminal is restored; `cli_docs.rs` holds the option list behind
  `--print-completions` (bash checked with `bash -n` and a completion run; zsh, fish and
  PowerShell not run: no such shells here) and `--print-man` (rendered with `man -l`);
  About links the releases page. **Windows' close event: not done.** Its 5 seconds are
  only there while the handler has not returned, so it must block until clean-up ends;
  `ctrlc` returns at once, and a blocking `SetConsoleCtrlHandler` needs `unsafe`, which
  both crates forbid. The journal's clean-up at the next start stays the answer there.

- 2026-11-08, a review of the whole branch (owner: "review code kỹ để tránh gây ra
  issue"), nine findings, all fixed: a panic (`clamp(1, 0)`) when quick view's box is
  0 cells wide or high; the start-up probe now waits up to 1.5 s (ending on the device
  attributes reply), since a slow SSH link's late replies would reach crossterm as keys;
  an interrupted *permanent* delete is cleaned up after but never offered again (one
  Enter would have gone on); recovery runs whatever the setting is now, so rename many's
  files are put back even after Safe operations is turned off; a worker waiting on a
  question keeps its journal fresh (Windows judges by age); taking an iTerm2 or sixel
  picture away writes back only the cells it covered, as the frame has them, instead of
  repainting the screen; the probe also runs when a protocol is chosen by hand (for the
  cell size); one base64 (app.rs's) for both uses; a misplaced doc comment.

- 2026-11-08, the owner over SSH: pictures shown, but smaller and blurry ("size ảnh và
  resolution bị giảm, nên ảnh rất mờ"). With no answer to the cell-size question the
  guess was 8×16, smaller than real cells; Kitty and iTerm2 then scaled a small picture
  up. Now Kitty and iTerm2 get the picture at twice the cell density (at most 2048 px a
  side) and scale it down themselves; Kitty is sent as PNG (`f=100`), a fraction of raw
  RGB's bytes over SSH; sixel, drawn pixel for pixel, guesses VT340's 10×20 cell, which
  Windows Terminal and xterm use. Not seen on a real screen yet.

- 2026-11-08, safe operations built (owner: "On by default nếu ko ảnh hưởng nặng … ngược
  lại off"). SIGHUP/SIGTERM through `signal-hook` (in the tree via crossterm): cancel,
  two seconds to clean up, save, end; a second signal ends at once (Unix; Windows' close
  event not yet). `ops::set_safe`: a new file through `part_name` and
  `rename_noreplace`. `journal.rs` in the core: one file per job, written before acting,
  removed on drop; first a line per temporary file, then — measured — a line per
  directory, since the names carry the pid and a later start finds them there. Rename
  many journals its temporary names and the next start renames them back. Start-up
  recovery (Unix asks whether the pid lives; elsewhere a journal untouched for two
  minutes is dead), a summary dialog, and "continue" runs the job again with "replace
  only if older". F10 with jobs: after five seconds still stopping, asks again.
  **Measured** (release, Linux, ext4, page cache): 10,000 × 1 KiB, 175 → 265 ms (the
  rename, ~9 µs a file; the journal at first added another ~4 µs, gone with a line per
  directory); 300 MiB, no difference; memory nothing measurable. Not heavy: on by
  default.

- 2026-11-08, the owner's terms for this round: "nhánh này chỉ làm test local, ko ci
  và chưa có plan release next version" and "dùng 1-2 weeks đã" — work and tests stay
  on this machine, no CI runs, nothing pushed until the owner asks; no next release is
  planned while 0.2.1 is used for a week or two.
- 2026-11-08, the owner chose **B** ("để ít phụ thuộc nhiều nhất vào thư viện khi ko
  cần"): our own Kitty and iTerm2 sequences, `icy_sixel` for sixel only.

- 2026-11-08, built: `graphics.rs` (detection from the environment; Kitty raw RGB in
  4096-byte base64 chunks with `c`/`r` cells and an id; iTerm2 as an inline PNG made by
  the core's new `image::to_png`; sixel through `icy_sixel`; a dozen-line base64).
  Quick view decodes at the cell size the terminal reports (`window_size`; 8×16 where
  it says nothing, as on Windows) instead of two pixels a cell, leaves its box empty,
  and the loop sends the picture after the frame (`sync_graphics`) only when it changed:
  a Kitty picture is removed by id; an iTerm2 or sixel one, being cells, by repainting
  the screen. Under any dialog nothing is drawn. A resize or an editor's return forgets
  what was shown. `Quiet::raw` writes the bytes past ratatui's cell diff.
- 2026-11-08, the owner's first try (a screenshot over SSH): the picture came out large
  and blocky — the half blocks, because `auto` found nothing in the environment; over
  SSH `WT_SESSION`, `TERM_PROGRAM` and the like do not arrive. Now xdevcmd also **asks
  the terminal** at start-up (Unix; not inside tmux): a Kitty graphics query, the cell
  size (`CSI 16 t`) and the device attributes (`CSI c`, `4` = sixel), read straight from
  the descriptor with a 300 ms limit before crossterm reads anything; the answers cross
  SSH. The pty test terminal now answers that question when told to, as a real one
  would (`Session::start_answering`), which the first version of the test missed: it
  answered only after the first frame, which waits on the answer.
- 2026-11-08: round opened on `feat/image-preview` from `dev` after 0.2.1. `ratatui-image`
  tried in a scratch build to measure what it brings (table above), then reverted.

## Check

- [x] fmt, clippy (Linux, and the macOS and Windows targets from Linux), `cargo test
      --workspace`: core 146, tui 136 + 147 render + 35 pty, all pass; `cargo deny` passes
      with `icy_sixel` (2026-11-08). No CI, as the owner asked.
- [ ] Not verified: how any picture looks — no terminal here draws them. Over SSH from
      iTerm2 or Ghostty to this machine, `auto` should find iTerm2 by `LC_TERMINAL` or
      Ghostty by `TERM`; Windows Terminal's sixel and the 8×16 cell guess there.

- [x] Close-out (2026-11-10): fmt; clippy `--workspace --all-targets -D warnings` on
      Linux; `cargo test --workspace` three times after the step-11 fixes: core 158 +
      1, tui 143 unit + 150 render + 48 pty (1 ignored, the soak), all pass each time;
      the pty suite 5 times in all, 48/48 each. Before the core test fix, one full run
      failed on it (in Do). Windows and macOS targets could not be installed here
      (proxy), so neither was compiled.
- [x] Input thread: hang-up test 15/15 with it, 10/15 without; the editor-keys test
      fails with a reader that never stops.
- [x] No `kind`-line panic: prefix test fails with `line[5..]` put back.
- [x] Resume never replaces a kept file: core test fails with `Update` put back.
- [x] Liveness: other-host and same-pid tests fail with the pid-only check put back (and
      the held-journal test with liveness by pid alone).
- [x] Resume pty test 20/20 alone and in full parallel run (before: 2/20 and 14/20
      failing; in Do).
- [x] Hang-up: session saved on EIO with signals off; fails with `?` put back (the
      process does not even end).
- [x] Background panic leaves xdevcmd running, log written; fails with the old hook.
- [x] Sixel without cell size: no sixel bytes; fails with the check off.
- [x] Pictures: one `a=T` per picture, `a=p` after a dialog; five pictures passed, one
      sent (fails without the 120 ms wait); bytes and draw times recorded in Do.
- [x] Ctrl+R, Tab, Ctrl+Space arrive as `022 \t \0`; fails with Ctrl+Space as a space.
- [ ] Not verified here, owner by hand: Windows journal locking (kill from Task Manager),
      NFS, pictures on real terminals, Ctrl+Space in a real shell; Windows and macOS
      builds (CI).
- [x] Recorded for the owner, not changed: global Alt keys (`is_global`) that readline uses
      (Alt+D, Alt+C, Alt+T, Alt+N, Alt+1…9, Alt+R group).

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : —
