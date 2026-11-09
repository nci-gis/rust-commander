# Round 014: Sharp image previews

**Status**: In Progress
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

## Do

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

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : —
