# Round 025: xdevtheme-light (v0.4.0, step 2, item 37 part 2)

**Status**: Review (set aside for the owner's hand tests, 2026-11-12)
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

- **2026-11-12, the theme** (`a4ecf6f`): Dawn from `3b99e08^` as
  `themes/xdevtheme-light.toml`. Measured with a scratch script (WCAG ratios; 256 by the
  same `nearest_256` as `theme::fit`; not kept, a Rust test holds the bars), then
  screenshots (`XDEVCMD_SCREENSHOT_COLOR=1`, `XDEVCMD_COLORS=truecolor|256|16`, turned
  into PNGs; 16 colours drawn with Terminal.app's Basic palette) of the panes with marks
  and cursor, F1 small and large, the Copy dialog, the viewer, quick view, the File menu.
  - **Colours**, all darker than Dawn's: accent and focused frame `#006e78` (chosen so it
    stays cyan at 16 colours, else the focused frame and the other one are both grey
    there), `text_dim #555c70`, `marked #6e5c00`, `archive #984600`, `directory
    #2a48c2`, `executable #196f2f`, `symlink #792ed3`, `error #bd1e2e`, `border
    #878fa5`. Several were moved for 256 colours: the cube is coarse (an orange and a
    gold both land on 136,95,0), so each was checked on the colour Terminal.app shows.
  - **Two decor changes, light panels only** (`decor::is_light`), since colours alone
    could not fix them: the glow cursor starts at 20 % accent, not 40 % (on white, 40 %
    of a dark teal left directory 3.6:1 and marked 3.1:1 on the cursor row); the `dim`
    shadow darkens by 14 %, not 60 % (a near-black slab beside every dialog). The dark
    themes draw as before. `shadow = "glow"` was tried first: a lavender block, worse.
  - **Deviation**: `--print-theme` took built-in names only; it now prints a theme file
    too (`base` light or dark by the background, every colour, the style), which the
    round-trip test needed and the README's "start your own" suggests.
  - Tests: the example-theme test also prints each file and reads it back equal;
    `xdevtheme_light_reads_on_its_own_background` (the bars above, true colour and 256;
    fails with Dawn's `marked`, checked); render test
    `xdevtheme_light_is_a_theme_that_draws_on_its_white_panel` (panes and F1 small and
    large on `#fbfcff`, the notch, in View → Theme…, the printed copy draws the same
    frame); `decor` unit test for the soft shadow.
- **Docs** (`c83b37a`): `themes/README.md`, `docs/config.md` (nine example themes,
  `--print-theme` of a file, the decor on a light panel), CHANGELOG.
- fmt, clippy `-D warnings` (Linux), `cargo test --workspace` green.

Contrast, true colour / 256 (panel `#fbfcff`): text 15.5 / 15.1; text_dim 6.5 / 6.4
(bar 5.4 / 5.5); directory 7.3 / 6.5; executable 6.1 / 8.0; symlink 6.5 / 7.0; archive
6.4 / 5.7; error 6.0 / 7.4; marked 6.4 / 6.7; accent 5.9 / 7.0 (bar 4.9 / 6.1); border
3.2 / 3.4. On the glow cursor's lit end: directory 5.4 / 4.6, marked 4.8 / 4.8.

## Check

- [x] Every text role ≥ 4.5:1 on panel, bar and cursor rows; frames ≥ 3:1 (true colour
  and 256). Exception: `archive` on the lit end of the cursor row at 256 colours is
  4.1:1 (no orange in the cube does better there); 4.6 at true colour.
- [x] Screenshots at true colour, 256 and 16 looked at: panes with marks and cursor, F1
  small and large, a dialog, the viewer, quick view
- [x] The file loads, is in the theme list, round-trips; render test
- [x] Docs and CHANGELOG; fmt, clippy (Linux), the full suite
- [ ] **16 colours** (decor off, colours fitted to the user's palette): readable, the
  focused frame cyan, but the two cursor rows are the same grey (7), as in the built-in
  `light`, and marked, archive and error are all red (1) — the `•` still marks. The
  README points 16-colour users at `ansi`. Not changed: it needs a per-depth palette in
  theme files, an engine change.
- [ ] Clippy for windows-gnu and darwin: targets not installed here; CI
- [ ] The owner: Terminal.app (256 colours, light profile), a light Linux terminal, by
  hand; the 16-colour look on a real light palette

Record what could **not** be verified as plainly as what could.

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → reference/features.md : the theme family (looks) names xdevtheme-light
