# Round 008: The logo in the app

**Status**: Complete
**Part of**: [ROADMAP.md](../ROADMAP.md), item 7
**Date started**: 2026-11-07
**Date completed**: 2026-11-07 (closed by the owner: "các round đã okay nhé, bro flip to Complete")

## Goal

The owner liked the icon and asked for it inside rcmd: About shows it, drawn in the
terminal.

## Plan

- [x] 1. A 24-pixel PNG of `assets/icon.svg`, built into the binary
      (`include_bytes!`), made by `scripts/icons.sh` with the `.ico`.
- [x] 2. `rcmd_core::image::png_bytes`: the PNG decoder reads from memory as well as a
      file; the same shrinking and transparency over the panel's colour.
- [x] 3. One half-block painter (`modal::paint_picture`) for quick view and About.
- [x] 4. About: the logo on the left, 24 × 12 cells, the text beside it.
- [ ] 5. "The empty state" from the roadmap note: not done. No screen in rcmd is empty
      enough to want it today; left for when one is.

## Do

**2026-11-07**, on `feat/v0.2.0`. `Ask` gained `picture`; `render_ask` widens the box by
the picture and two columns, and the buttons and the credit use the whole width. At 256
or 16 colours the logo is fitted like every other cell (Round 001).

## Check

- [x] Render: About has 12 rows of 24 half blocks beside its text; the credit is still on
      the button row.
- [ ] How it looks: `--screenshot` is text, so the colours were not seen by anyone yet.

## Act

**Unticked checks, and why the round closes anyway:** the colours were not seen by an
agent; the owner saw the logo, found 24 px blocky, and `feat/polish` now draws the
largest of 24, 32, 48 and 64 px the window has room for.

**Learnings**:

- **In half blocks a pixel is half a cell**: a sharper logo is a larger one, so the
  size follows the window rather than being fixed.

**Promotions**:

- (none)
