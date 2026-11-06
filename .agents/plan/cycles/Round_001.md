# Round 001: Colours on terminals without 24-bit colour

**Status**: Planning
**Part of**: [ROADMAP.md](../ROADMAP.md), item 1
**Date started**: 2026-11-06
**Date completed**:

## Goal

On macOS Terminal.app, rows show green and red backgrounds and text is hard to read.
Make rcmd look right on a terminal that cannot show 24-bit colour, without changing how
it looks on one that can.

## What is known

Every colour in `theme.rs` is `Color::Rgb`, sent as `ESC[38;2;R;G;Bm` /
`ESC[48;2;R;G;Bm`. rcmd never asks what the terminal supports. Terminal.app before
macOS 26 has no 24-bit colour and, by every report, reads the numbers after `38;2` /
`48;2` as separate attributes. The theme's own numbers then become:

| Theme colour | RGB | Read as |
|---|---|---|
| `row_cursor` | 28, **42**, 58 | 42 = **green background** |
| `fg_dim` | 101, 119, 139 | **101** = **bright red background** |
| `mark` | 255, 214, **102** | 102 = **bright green background** |
| `bg_bar` | 17, 23, **32** | 32 = green text |
| `border` | **37**, **49**, 63 | white text, default background |
| `accent`, `border_active`, `row_cursor_active` | **0**, … | 0 = reset everything |

That matches the report exactly: the cursor row green, dimmed text on red, marks on
green. The terminal profile only changes *which* green and red. This is a hypothesis
until step 1 confirms it on the reporter's Mac.

Reports (not Apple documentation) say Terminal.app on macOS 26 (Tahoe) supports
24-bit colour; whether it sets `COLORTERM` is unknown. One third-party change puts the
cut-off at `TERM_PROGRAM_VERSION` 464 and forces 256 colours below it **even when
`COLORTERM=truecolor`**, because people export that in their dotfiles. ratatui and
crossterm do no detection or downgrading; every ratatui app that fixed this wrote its
own.

**Over SSH neither variable arrives** (`COLORTERM` and `TERM_PROGRAM` are not forwarded
without `SendEnv`/`AcceptEnv`). rcmd on a server reached from an old Terminal.app would
still send 24-bit colour under a rule that only downgrades `Apple_Terminal`. Hence the
reversed rule in step 3.

## Plan

- [ ] 1. **Confirm on the reporter's Mac** before writing code. In the Terminal.app
      window that shows the problem:

      ```bash
      echo "$TERM_PROGRAM $TERM_PROGRAM_VERSION COLORTERM=$COLORTERM TERM=$TERM"; sw_vers -productVersion
      printf '\e[48;2;28;42;58m  rgb  \e[0m  \e[48;5;236m  256  \e[0m\n'
      ```

      Confirmed if `rgb` has a **green** background and `256` a dark grey one. Record
      the version numbers here: they set the cut-off in step 3.
- [ ] 2. **Colour depth** as a setting, `[theme] colors = "auto" | "truecolor" | "256" |
      "16"`, and an `RCMD_COLORS` environment variable that overrides it per host (a
      server's config cannot know which terminal is in front of it).
- [ ] 3. **`auto` uses 24-bit only on positive evidence**, 256 otherwise:
      - `TERM_PROGRAM=Apple_Terminal` below the version from step 1 → 256, whatever
        `COLORTERM` says;
      - `COLORTERM` = `truecolor`/`24bit` → 24-bit;
      - a terminal known to support it (iTerm2, WezTerm, kitty, Ghostty; Windows
        Terminal and conhost when running locally on Windows) → 24-bit;
      - `TERM=linux` → 16;
      - anything else → 256.

      No terminal query by default: rcmd has already had a cursor query time out over a
      slow SSH link, and Tahoe's Terminal.app reportedly does not answer colour
      queries. The cost is 256 colours over SSH on a capable terminal unless
      `COLORTERM` is forwarded or set.
- [ ] 3b. **An `ansi` theme** from the 16 ANSI colours, which follows the user's own
      palette (light or dark). Chosen automatically at depth 16; selectable anywhere.
      Whether it becomes the default is a decision for a person (ROADMAP).
- [ ] 4. **Convert in one place**: `quiet.rs` already sees every cell before it is
      written. Map `Rgb` to the nearest xterm-256 index there (the 6×6×6 cube or the
      grey ramp, whichever is closer). That covers the theme, `LS_COLORS` and colours a
      program prints into the terminal panel, which reach the backend through tui-term.
- [ ] 5. `--print-config`, `docs/config.md`, CHANGELOG.

## Risks

- **Contrast can collapse at 256.** The palette is coarse at the dark end: `bg`
  (10, 14, 20), `bg_panel` and `row_cursor` (28, 42, 58) may land on the same one or two
  greys (233–236) and the cursor row disappears. A test checks that the slots that must
  differ still differ after quantising; where they do not, the theme names an explicit
  256-colour fallback for that slot.
- **The reversed `auto` changes Linux-over-SSH users** from 24-bit to 256 unless their
  terminal forwards `COLORTERM`. Accepted: the difference is slight and the text stays
  readable; `colors = "truecolor"` or `RCMD_COLORS` restores it.

## Do

## Check

- [ ] Unit tests: the RGB → 256 and → 16 mappings (cube, grey ramp, the theme's
      colours), `auto` from each environment combination (including `Apple_Terminal`
      with `COLORTERM=truecolor`), and the contrast check above.
- [ ] A byte test through `Quiet` (as its existing tests do): with 256 colours, no
      `38;2;` or `48;2;` reaches the terminal, including from the terminal panel.
- [ ] The reporter runs the build on the Mac that showed the problem. **CI cannot see
      colours**: `--screenshot` is text only, and macOS CI has no Terminal.app.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/platforms.md`: Terminal.app and 24-bit colour, and how `auto` decides
