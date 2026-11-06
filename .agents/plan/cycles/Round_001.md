# Round 001: Colours on terminals without 24-bit colour

**Status**: Complete
**Part of**: [ROADMAP.md](../ROADMAP.md), item 1
**Date started**: 2026-11-06
**Date completed**: 2026-11-07 (closed by the owner's approval)

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

- [x] 1. *(partly: the fix was confirmed on the Mac, the version numbers were never sent)* **Confirm on the reporter's Mac** before writing code. In the Terminal.app
      window that shows the problem:

      ```bash
      echo "$TERM_PROGRAM $TERM_PROGRAM_VERSION COLORTERM=$COLORTERM TERM=$TERM"; sw_vers -productVersion
      printf '\e[48;2;28;42;58m  rgb  \e[0m  \e[48;5;236m  256  \e[0m\n'
      ```

      Confirmed if `rgb` has a **green** background and `256` a dark grey one. Record
      the version numbers here: they set the cut-off in step 3.
- [x] 2. **Colour depth** as a setting, `[theme] colors = "auto" | "truecolor" | "256" |
      "16"`, and an `RCMD_COLORS` environment variable that overrides it per host (a
      server's config cannot know which terminal is in front of it).
- [x] 3. **`auto` uses 24-bit only on positive evidence**, 256 otherwise:
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
- [x] 3b. **An `ansi` theme** from the 16 ANSI colours, which follows the user's own
      palette (light or dark). Chosen automatically at depth 16; selectable anywhere.
      Whether it becomes the default is a decision for a person (ROADMAP).
- [x] 4. **Convert in one place**: `quiet.rs` already sees every cell before it is
      written. Map `Rgb` to the nearest xterm-256 index there (the 6×6×6 cube or the
      grey ramp, whichever is closer). That covers the theme, `LS_COLORS` and colours a
      program prints into the terminal panel, which reach the backend through tui-term.
- [x] 5. `--print-config`, `docs/config.md`, CHANGELOG.

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

**2026-11-06.** Steps 2–5 done; step 1 (the reporter's Mac) is still open, so the
cause and the build-464 cut-off remain unconfirmed on real hardware.

- `rcmd-core/src/color.rs`: `Depth`, `detect` (environment only, read through a
  closure so tests never touch the real one), `nearest_256` (cube or grey ramp, never
  0–15), `nearest_16`, `palette_rgb`.
- `theme.rs`: `Theme::ansi()` and `fit(colour, depth)`. `config.rs`: `[theme] colors`,
  and a second theme `theme16` built from the same settings over `Theme::ansi()`, so a
  colour the user set applies at every depth and the rest keep the 16-colour palette.
- `quiet.rs`: below 24-bit, the changed cells are copied and fitted before the inner
  backend writes them — foreground, background and underline colour (ratatui's
  `underline-color` feature is on). Every source goes through it: theme, `LS_COLORS`,
  pictures, the terminal panel.
- `main.rs`: `RCMD_COLORS` (a bad value stops start-up, as a bad flag does), then
  `[theme] colors`, then `detect`.

**Deviation.** The plan said the 16 colours would go out "by name" (`ESC[3Xm`).
crossterm sends every named colour as `38;5;N` with N from 0 to 15; those are the user's
own palette entries, which is what was wanted, so nothing changed but the comment and
the test.

**Contrast at 256** (default theme): `bg` and `bg_panel` both become grey 233 (they were
nearly equal anyway); `bg_bar` 234; `row_cursor` 235; `row_cursor_active` 23 (teal);
`fg_dim` 66. No pair that must differ collapsed, so no per-slot fallback was needed.

## Check

- [x] Unit tests: the RGB → 256 and → 16 mappings (cube, grey ramp, the theme's
      colours), `auto` from each environment combination (including `Apple_Terminal`
      with `COLORTERM=truecolor`), and the contrast check above.
      (`color.rs` 10, `theme.rs` 4, `config.rs` 2.)
- [x] A byte test through `Quiet` (as its existing tests do): with 256 colours, no
      `38;2;` or `48;2;` reaches the terminal, including from the terminal panel.
      Fails with the fitting switched off, passes with it. Plus two pty tests on the
      real binary: an old Terminal.app environment gets `48;5;` and no `48;2;` even with
      `COLORTERM=truecolor`; iTerm2 gets `48;2;`; `RCMD_COLORS=16` sends only palette
      entries 0–15.
- [x] The reporter runs the build on the Mac that showed the problem. 2026-11-07, the
      CI build on the owner's Intel Mac in Terminal.app: no green or red backgrounds,
      cursor rows and dimmed text readable (screenshot). Still not sent: the macOS and
      Terminal.app versions, so the build-464 cut-off itself is unconfirmed. **CI cannot see
      colours**: `--screenshot` is text only, and macOS CI has no Terminal.app.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- A terminal without 24-bit colour does not ignore `ESC[48;2;R;G;Bm`: macOS
  Terminal.app before macOS 26 reads each number as its own attribute, so a theme's RGB
  values become green and red backgrounds. ratatui and crossterm never check; the app
  has to.
- Decide colour depth on positive evidence only. `COLORTERM` and `TERM_PROGRAM` do not
  cross SSH, and people export `COLORTERM=truecolor` in dotfiles, so "downgrade only
  where known broken" breaks exactly the remote case.
- One backend wrapper that every cell passes through (`quiet.rs`) made this a single
  fix that also covers `LS_COLORS`, pictures and programs in the terminal panel.
- crossterm sends even the 16 named colours as `38;5;N` (N 0–15); a test asserting
  "no `38;5;`" at 16 colours was wrong, not the code.
- Still unconfirmed: Terminal.app's first 24-bit build (464, from one third-party
  report). If a newer Terminal.app shows muted colours, that number is the suspect.

**Promotions** (proposed; `reference/` is a person's to change):

- [ ] → `reference/platforms.md`: Terminal.app and 24-bit colour, and how `auto` decides
- [ ] → `reference/gotchas.md`: crossterm writes named colours as `38;5;N`; `COLORTERM`
      does not survive SSH
