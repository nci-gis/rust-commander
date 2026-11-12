# Round 025: xdevtheme-light (v0.4.0, step 2, item 37 part 2)

**Status**: In Progress
**Part of**: v0.4.0, step 2 of `ROADMAP.md` ("Next"); item 37, part 2 (the owner's go,
2026-11-12)
**Date started**: 2026-11-12
**Date completed**: YYYY-MM-DD

## Goal

A light member of xdevtheme's family, for a terminal with a light background. macOS
Terminal.app's default profile is light, so for many people this is the first look of
xdevcmd (ROADMAP, note on 37). A theme file, not the default.

## Plan (kickoff review)

### 1. Where it starts

- **Dawn** was built the day xdevtheme was (owner's "A + Pulse", 2026-11-11) and dropped
  when the family was trimmed to three: added in `cc1ee63`, deleted in `3b99e08`
  (`git show 3b99e08^:themes/xdevtheme-dawn.toml`). `base = "light"`, xdevtheme's
  `[style]` (rounded, notch, gradient to violet, glow cursor, marker, slant keys, dim
  shadow) with a deeper teal. Memory:
  `memory/2026-11-11-v0.4.0-scope-proposal.md`, "Added later the same day".
- **What already holds it**: `theme::tests::the_example_theme_files_load_and_keep_their_rows_apart`
  loads every `themes/*.toml` and checks that the slots drawn over each other stay
  apart at true colour and 256. View → Theme… and Settings → Theme list every file in
  `themes/` (`config::available_themes`). `--print-theme` prints built-ins only.

### 2. Critique: what must be got right

Measured on Dawn as it was (WCAG contrast; script in the round's scratch, not kept):

- **Contrast on white.** Text 15.5:1 is fine. Below 4.5:1: the accent (titles, keys,
  menu letters) **2.9**, `text_dim` 3.7 (3.0 on the bar), `marked` 3.5, `archive` 4.3,
  the gradient's middle 3.9 and end 4.1. Borders: `border_active` 2.9, `border` **1.4**
  (the inactive pane's frame nearly gone). A light theme's colours must be darker than a
  dark one's, not the same hues.
- **The decor on a light background.** It was tuned on dark:
  - the glow cursor starts at 40 % accent: directory 3.9 and marked **2.3** on it;
  - `shadow = "dim"` darkens toward black by 60 %: a heavy grey slab beside every
    dialog on white;
  - the gradient recolours every accent cell, so its whole run must stay readable, not
    only its ends.
- **256 colours** (Terminal.app has no true colour). The grey ramp is coarse at the
  light end: background, bar and cursor rows can meet; the test checks the pairs, the
  screenshots check the look. `archive` drops to 3.8 at 256.
- **16 colours.** Decor is off; a chosen theme is kept and fitted to the nearest of the
  16, which are the user's palette. Dawn at 16: panel and background → 15 (white),
  bar, cursor and cursor_active → 7: **the focused and unfocused cursor rows are the
  same colour**; directory and symlink both 12. Check what a light terminal makes of
  it; the `ansi` theme stays the advice for 16 colours.
- **Selection, cursor, marks**: the marked colour on the cursor row, on the glow and
  on the plain panel, all three.
- **The terminal panel** is left to the program (`decorate` skips it); a shell's own
  colours are the shell's. Nothing to do, but look that the frame around it reads.
- **F1, dialogs, menus, the viewer**: titles and keys are accent text; the selected row
  of F1 and of menus; refused rows in `text_dim`.

### 3. Steps

1. This kickoff; ROADMAP links the round. **[skip ci]**.
2. **The theme**: Dawn back as `themes/xdevtheme-light.toml`, renamed, tuned to the
   bars below with a contrast script; screenshots at true colour, 256 and 16 of panes
   with marks and cursor, F1 small and large, a dialog, the viewer, quick view off;
   look, fix, look again.
3. **Tests**: the example-theme test covers it already; add that every file in
   `themes/` is in the theme list, that a file theme's text round-trips (a file read
   back as the theme it describes), and a render test: the light theme draws panes and
   F1 without panic, on its background.
4. **Docs**: `themes/README.md`, `docs/config.md` (the theme list), CHANGELOG.

Bars: ≥ 4.5:1 for every text role on the panel, on the bar and on the cursor rows; ≥
3:1 for frames (WCAG 1.4.11: the frame is how one pane is told from the other) and for
the gradient's run as text, ≥ 4.5 where it is reached at its ends (titles, keys) — at
true colour and 256.

### 4. Decisions (recommendations; the coder follows them unless the owner objects)

1. **Name**: `xdevtheme-light`, not Dawn: it says what it is for, beside `light` (the
   plain built-in) as xdevtheme is beside `dark`.
2. **A file theme in `themes/`**, not built in: the family stays one built-in plus
   files; no binary cost; the release archives carry it.
3. **Not the default**, and no detection of the terminal's background (OSC 11) in this
   round: that is ROADMAP's Later ("xdevtheme-light covers it by hand first").
4. **Reached** by View → Theme… and Settings → Theme, as any file, and named in the docs
   for Terminal.app users.
5. **The same `[style]` as xdevtheme** where it reads on white; what does not (the dim
   shadow) changes for this theme only, no engine change unless a screenshot shows a
   decor pass that cannot be tuned by colours.

### Not doing (this round)

- OSC 11 background detection, a light default: Later.
- A light Neon or Glass.

## Do

## Check

- [ ] Every text role ≥ 4.5:1 on panel, bar and cursor rows; frames ≥ 3:1 (true colour
  and 256)
- [ ] Screenshots at true colour, 256 and 16 looked at: panes with marks and cursor, F1
  small and large, a dialog, the viewer, quick view off
- [ ] The file loads, is in the theme list, round-trips; render test
- [ ] Docs and CHANGELOG; fmt, clippy, the full suite
- [ ] The owner: Terminal.app (256 colours, light profile), a light Linux terminal

Record what could **not** be verified as plainly as what could.

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → reference/features.md : the theme family (looks) names xdevtheme-light
