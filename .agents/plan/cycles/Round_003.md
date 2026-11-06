# Round 003: macOS keys and mouse

**Status**: Planning
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
- **Guessing from characters is layout-dependent.** `ç` is Option+C on a US layout only.
  The hint fires on "a non-ASCII character in a file pane on macOS", which is enough
  to suggest the setting, never to act on it as a key.
- **CI cannot press keys in Terminal.app.** The macOS runner has no Terminal.app
  session. Everything in step 1 and the end check is by hand on a Mac.

## Do

## Check

- [ ] Step 1's table recorded here: which keys arrived, with which settings.
- [ ] Unit tests: the Esc-prefix state machine (timeout, Esc Esc, Esc + digit, Esc in a
      dialog closes at once); the hint fires once and only in a file pane.
- [ ] Render and pty tests for `--show-keys` and Esc-prefix on Linux.
- [ ] The reporter: every key on the F1 page works in Terminal.app with default
      settings via Esc-prefix, and directly once Option is Meta; the wheel scrolls rcmd.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/platforms.md`: macOS keyboard and mouse, and what rcmd does about it
- [ ] → `reference/gotchas.md`: anything step 1 turns up that is not obvious
