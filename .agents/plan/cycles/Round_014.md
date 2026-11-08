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

- [ ] **Quit while the terminal has focus.** Today F10, q and Ctrl+Q go to the shell
      there (they must: htop quits on F10, nano and others use Ctrl+Q), so the way out is
      F12 back to the panes, then F10. Proposed: F10 quits xdevcmd when the shell sits
      idle at its prompt (the test Alt+C already uses), and goes to the program when one
      is running; File → Quit with the mouse works anywhere.
- [ ] **Save on every way out.** Already: F10, q and Ctrl+Q save the session after the
      loop, and it is autosaved every 5 s (`[general] autosave_secs`) for the ways that
      kill xdevcmd. To add: the signal ways below save too.
- [ ] **A terminal closed under a running job.** Today nothing catches it: SIGHUP (or
      Windows' close event) kills xdevcmd at once. A file being *replaced* is safe (its
      temporary copy is left beside it, hidden); a *new* file being written is left
      truncated **under its real name**, looking complete; the session loses up to 5 s.
      Proposed: (1) every copy writes under a temporary name and renames at the end, so
      even a kill leaves no truncated file under a real name; (2) catch SIGHUP, SIGTERM
      and Windows' close event (`ctrlc` with its termination feature, or `signal-hook`),
      then cancel jobs (workers remove their temporary files), save the session, exit;
      (3) remember unfinished temporary files in the state directory and remove the
      ones still there at the next start.

## Do

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
