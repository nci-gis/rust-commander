# Round 003: macOS keys and mouse

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), item 2
**Date started**: 2026-11-06
**Date completed**:

> This number first held a spike for SFTP and plugins. The owner dropped SFTP on
> 2026-11-06 before it started; plugins moved to the end of the roadmap.

## Goal

On macOS, every shortcut rcmd shows works, and the mouse wheel scrolls rcmd rather
than the terminal window's history — in Terminal.app with its default settings, or,
where that is impossible, with one clear line telling the user which setting to change.

## What was reported

1. **Alt shortcuts do nothing** in Terminal.app. rcmd has 30 bindings with Alt
   (Alt+F7 find, Alt+C, Alt+N, Alt+Left/Right history, …).
2. **Some other keys** do nothing either. Which ones is not known yet.
3. **A scrollbar appears, and scrolling up shows the terminal window's history** instead
   of scrolling rcmd.

## Likely causes (to confirm in step 1, not facts yet)

1. **Option is not Alt by default on a Mac.** Terminal.app (and iTerm2) make Option type
   characters: Option+C is `ç`, Option+N starts a dead key. rcmd never sees an Alt key.
   Terminal.app has "Use Option as Meta key" (Settings → Profiles → Keyboard); iTerm2
   has "Left Option key: Esc+".
2. **F-keys are media keys** on a Mac keyboard unless Fn is held or "Use F1, F2, etc.
   keys as standard function keys" is on, and macOS takes some for itself (Mission
   Control, Show Desktop). The F-key bar is rcmd's main interface, so this alone could
   be the "other keys". F-keys not arriving in a terminal (Ghostty) is among mc's
   most-reacted open issues: an orthodox commander lives or dies by them.
3. **The wheel reaches the window, not rcmd.** rcmd switches to the alternate screen and
   asks for mouse reports. If Terminal.app's mouse reporting is off (View → Allow Mouse
   Reporting, ⌘R), or Terminal.app does not accept the reporting modes crossterm turns
   on, the wheel scrolls Terminal.app's own scrollback, which stays reachable above the
   alternate screen.

## Plan

- [ ] 1. **Measure on the reporter's Mac**, before any code. In Terminal.app:
      - macOS version, `echo $TERM_PROGRAM $TERM_PROGRAM_VERSION`;
      - which keys fail: each F-key, Alt+letter, Alt+arrows, Shift+F-keys, Ctrl+\\;
        with and without Fn; with and without "Use Option as Meta key";
      - in rcmd: does a click move the cursor? does the wheel scroll the pane? with ⌘R
        on and off.

      rcmd cannot yet show which key it received, so this step is easier after step 2.
- [ ] 2. **A key tester**: `rcmd --show-keys` prints each key as crossterm decodes it,
      and the binding it would trigger, until Esc Esc. Useful on every platform and for
      every future "key X does nothing" report.
- [ ] 3. **Esc as a prefix**, as Midnight Commander does, which works in any terminal
      with no setting: Esc then a key within a second is Alt+key; Esc then 1…0 is
      F1…F10. Only where Esc alone does nothing (file panes, not dialogs, not the
      terminal panel), so closing a dialog is never delayed.
- [ ] ~~3b.~~ **Dropped 2026-11-07 by the owner** ("Use Option as Meta key" works; Esc-prefix is enough). Was: **Read Option's characters as Alt**, on macOS only, in file panes only (never
      in a text field or the terminal panel), with a setting to turn it off. With the
      default US / ABC layout Option+letter types a fixed character: å ∫ ç ∂ ƒ © ˙ ∆ ˚ ¬
      µ ø π œ ® ß † √ ∑ ≈ ¥ Ω for a b c d f g h j k l m o p q r s t v w x y z, and ≤ ≥ ≠
      for `,` `.` `=`. A table maps them back to Alt+letter, so Option works with no
      Terminal.app setting. Two limits, said in the docs: Option+E, I, N, U and `
      are dead keys that type nothing until the next key (Alt+N needs Esc N or the
      setting), and another keyboard layout types other characters (the table is only
      for US / ABC; the setting and Esc-prefix cover the rest).
- [ ] ~~3c.~~ **Dropped with 3b.** Was: **Option+← / →**: Terminal.app sends `ESC b` / `ESC f` for them by default (to
      confirm), which crossterm reads as Alt+B / Alt+F. Bind those to back and forward
      as well, so history works on a Mac out of the box.
- [ ] 4. **Say what to change.** When a character that Option produces arrives where
      typing means nothing (a file pane), say once per session: "Option is typing
      characters: turn on Use Option as Meta key in Terminal → Settings → Profiles →
      Keyboard, or press Esc then the key". The same for F-keys if step 1 shows they
      arrive as media keys or not at all (rcmd sees nothing then; the hint goes on F1
      and in the docs).
- [ ] 5. **The wheel**: from step 1's result. If reporting was off, the hint and docs.
      If Terminal.app ignores crossterm's modes, also turn on alternate-scroll mode
      (`ESC[?1007h`, where the terminal turns the wheel into arrow keys on the
      alternate screen) and handle the arrows as scrolling.
- [ ] 6. `docs/guide.md`: a "rcmd on macOS" section with the three settings. Keys F1 page
      mentions Esc-prefix. CHANGELOG.

## Risks

- **Esc-prefix delays nothing only if scoped.** Applied globally, every Esc that closes
  a dialog waits for a possible second key. Scope it to file panes, and test that Esc
  still closes every dialog at once.
- **Reading characters as keys is layout-dependent.** `ç` is Option+C on US / ABC
  only. Step 3b acts on the table only on macOS and only in file panes, where typing
  `ç` would otherwise start a quick search for a name beginning with it; a setting
  turns it off for anyone whose file names start with those characters. Anything not
  in the table only triggers step 4's hint.
- **CI cannot press keys in Terminal.app.** The macOS runner has no Terminal.app
  session. Everything in step 1 and the end check is by hand on a Mac.

## Do

**2026-11-07, first report from the owner's Intel Mac** (Terminal.app; version not yet
sent):

- Colours right after Round 001: no green or red backgrounds.
- Clicking in rcmd does nothing in rcmd, and the wheel scrolls Terminal.app's window,
  so **mouse reports are not reaching rcmd**: either View → Allow Mouse Reporting (⌘R)
  is off, or Terminal.app does not accept the modes crossterm turns on. Asked which.
  ~~The pointer is the I-beam, which Terminal.app shows only when no program has mouse
  reporting on.~~ **Wrong** (owner, 2026-11-07): Terminal.app shows the I-beam in a
  plain zsh too, so the pointer's shape says nothing about mouse reporting; the clicks
  and the wheel are the evidence. The blue "selection" was the profile's background,
  as the second report found.
- Option does not act as Alt, as expected. "Esc + Option + key" did nothing, also as
  expected: Esc-prefix is step 3, not built yet.

**2026-11-07, second report.** "Use Option as Meta key" on: Option works as Alt. The
owner asks only for Esc-prefix on top ("nhấn Esc rồi nhả"), so 3b and 3c are dropped.
View → Allow Mouse Reporting was already on, and the mouse still does not reach rcmd;
the blue area in the terminal panel was the owner's Terminal.app profile background
showing through cells with the default colour, not a selection as first thought.

**Built (2026-11-07):**

- Step 2, `rcmd --show-keys`: prints the terminal's variables and the colour depth,
  then each key with the action it runs, each click and wheel event, and once that
  movement is reported. Esc Esc or Ctrl+C quits. Checked in a pty on Linux: F7, Alt+C,
  an SGR click and a wheel all print.
- Step 3, Esc-prefix (`keyfix.rs`, `App::translate_key`): only in file panes, and only
  when Esc has nothing to stop (search, extraction, archive listing, filter), so no
  Esc is ever delayed. A status line says it is waiting. Esc Esc cancels.
- Step 4, partly: on macOS a non-ASCII character with no binding, typed in a file pane,
  shows the Option hint once per session. The F-key advice is in F1 and the guide, not
  a hint: rcmd sees nothing when a media key is pressed.
- Step 6: guide section "No Alt key, or no F-keys", an F1 section, CHANGELOG.

**Owner's follow-up (2026-11-07):** a setting for Esc as Meta (`[general] esc_as_meta`,
on by default); the how-to on GitHub, per system with known issues
(`docs/troubleshooting.md`), and only a few lines plus the link in F1. The macOS
settings moved there from the guide.

**Closed: the mouse (step 5), no change needed (2026-11-07).** `rcmd --show-keys` on
the owner's Mac printed movement, clicks with their column and row, and ScrollUp /
ScrollDown; and inside rcmd itself clicks, the wheel, selection and double-click all
work. There was no mouse bug. What looked like one: the I-beam pointer (Terminal.app's
pointer everywhere, a plain zsh included), and dragging the *window's* scrollbar, which
shows the first screen (the shell, with blank rows) above rcmd's second one. rcmd cannot
hide that; `docs/troubleshooting.md` says so. Alternate-scroll mode (`?1007h`) is not
needed.

**2026-11-07, third report.** Esc-prefix works on the Mac. With "Use Option as Meta
key" on, still not working: going up and into directories with Alt+← / → (and probably
Alt+↓, history), and resizing with Alt+Shift+arrows. Likely, to measure with
`--show-keys`: Terminal.app's profile maps ⌥← / ⌥→ to `ESC b` / `ESC f` (word left /
right) whatever the Meta setting, so rcmd gets Alt+B / Alt+F; what ⌥↑ / ⌥↓ and
⌥⇧-arrows send is unknown. Also suspect: Fn+↑ / ↓ (PageUp / PageDown) scroll
Terminal.app's own window rather than reaching rcmd, which would break PageUp /
PageDown and Ctrl+PageUp / PageDown (tabs). The owner prefers a macOS keymap (ROADMAP
item 5) to working around each key; its keys come from this measurement, not guesses.

## Check

- [ ] Step 1's table recorded here: which keys arrived, with which settings. Partly:
      Option works as Alt once "Use Option as Meta key" is on; the mouse arrives. Not
      recorded: which F-keys arrive with and without Fn.
- [ ] Unit tests: the Esc-prefix state machine (timeout, Esc Esc, Esc + digit, Esc in a
      dialog closes at once); the hint fires once and only in a file pane. Partly:
      `keyfix` tests Esc + key, Esc + digit, Esc Esc, and what looks like Option.
      **Not tested:** the two-second timeout, the hint firing only once (macOS-only
      code, no Mac test), and Esc in a dialog — that one holds by construction, since
      dialogs take keys before `translate_key` runs.
- [x] Render and pty tests for `--show-keys` and Esc-prefix on Linux. Esc-prefix: three
      render tests (Esc 7 is F7, Esc F7 is Alt+F7, Esc Esc cancels, a filter is taken
      off first, `esc_as_meta = false`). `--show-keys`: run by hand in a pty (F7,
      Alt+C, a click, a wheel); **no automated test**.
- [ ] The reporter: every key on the F1 page works in Terminal.app with default
      settings via Esc-prefix, and directly once Option is Meta; the wheel scrolls rcmd.
      Partly: Option as Meta works, the wheel scrolls rcmd. Esc-prefix on the Mac not
      reported yet.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/platforms.md`: macOS keyboard and mouse, and what rcmd does about it
- [ ] → `reference/gotchas.md`: anything step 1 turns up that is not obvious
