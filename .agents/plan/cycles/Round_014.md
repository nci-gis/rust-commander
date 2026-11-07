# Round 014: Sharp image previews

**Status**: Planning
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

- [ ] Detect what the terminal speaks; `[view] images = "auto" | "kitty" | "iterm2" |
      "sixel" | "blocks"` to choose, with "blocks" as today.
- [ ] Draw the picture once when it changes; remove it when a dialog opens over it, the
      pane closes, the cursor leaves the file, or the window resizes.
- [ ] Never inside the terminal panel's area; never in screenshots (the tests).
- [ ] Over SSH: the local terminal's protocol arrives through SSH, so detection must not
      trust only local environment variables; inside tmux, passthrough or blocks.
- [ ] Tests: the escape sequences for each protocol (unit), detection from the
      environment, blocks unchanged where nothing is supported.
- [ ] By hand, by the owner: iTerm2 or Ghostty on macOS, Windows Terminal, and
      Terminal.app (blocks, as now).

## Do

- 2026-11-08: round opened on `feat/image-preview` from `dev` after 0.2.1. `ratatui-image`
  tried in a scratch build to measure what it brings (table above), then reverted.

## Check

- [ ] —

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : —
