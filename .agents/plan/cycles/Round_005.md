# Round 005: Enter opens files the Total Commander way

**Status**: Planning
**Part of**: [ROADMAP.md](../ROADMAP.md), item 3
**Replaces**: [Round 002](Round_002.md), the same plan, renumbered so rounds run in
number order (owner, 2026-11-07)
**Date started**: (when its branch is opened, after 0.2.0)
**Date completed**:

## Goal

Enter on a file does what it does in Total Commander: the file opens in the program its
extension belongs to, chosen by the system unless the user says otherwise. No file name
ever passes through a shell, and running a program is still asked first.

## What exists

- **`o`** opens the file under the cursor with the system's program (`open` on macOS,
  `explorer.exe` on Windows, `xdg-open` on Linux), or `[commands] opener`. No shell; the
  name is one argument. Refused over SSH on Linux (no `DISPLAY`), and in read-only and
  restricted modes. Listed in the File menu and the right-click menu.
- **Programs, scripts, `.desktop` files and `.lnk`s** get "Run it?" with Cancel as the
  default.
- **Enter** on a file views it (F3). On a directory or archive, goes in.
- **Finding**: "Run it?" hands a console program to the system opener with stdin,
  stdout and stderr closed. Its output is lost, and what happens differs by platform:
  `xdg-open` may open a script in an editor, `open` starts a new Terminal.app window,
  Windows opens a console window.

## What Enter will do

| Under the cursor | Enter |
|---|---|
| a directory, or an archive rcmd can browse | go in (unchanged) |
| a program, script, `.desktop` or `.lnk` | "Run it?": **Run in a terminal tab** (default after Cancel), Run with the system, Cancel |
| a file with an `[[open]]` rule for its extension | what the rule says |
| any other file | the system's program for it, as `o` |
| anything, when nothing can be opened (no desktop over SSH, read-only, restricted) | view it (F3), and say why on the status line |

`[files] enter = "open" | "view"`. **Default `open`** if the owner confirms (ROADMAP
decisions); `view` keeps 0.1.0's behaviour.

## Plan

- [ ] 1. **"Run it?" runs a console program in a terminal tab**: a new tab of the
      terminal panel started on the program's argv directly, never typed into a shell
      (so rule 5 holds without `safety.rs`). "Run with the system" stays as a choice.
      Fixes the finding above and is what step 3's `terminal = true` reuses.
- [ ] 2. **Enter** as in the table; `[files] enter`.
- [ ] 3. **`[[open]]` rules** for extensions the user wants handled differently, first
      match wins, before the system's choice:

      ```toml
      [[open]]
      ext = ["txt", "log", "md"]
      action = "view"                # or "edit": rcmd's own viewer, or $EDITOR

      [[open]]
      ext = ["pdf"]
      command = ["zathura"]          # argv; the path is appended as one argument
      needs_display = true           # skipped over SSH, so the next rule or view applies

      [[open]]
      ext = ["rs", "toml"]
      command = ["nvim"]
      terminal = true                # in a terminal tab, not a window
      ```

      `command` is a list, never a string split by a shell. `os = ["windows"]` scopes a
      rule to some systems.
- [ ] 4. **Open with…** (Shift+O, the File menu and the right-click menu): the rules that
      match this file, "System default", "View", "Edit". Windows' own chooser is left
      out: the supported call (`SHOpenWithDialog`) needs FFI, which
      `forbid(unsafe_code)` rules out. Listing installed applications on Linux and
      macOS is left out too.
- [ ] 5. Config validation and `--print-config`; `docs/guide.md`, `docs/config.md`,
      keys, CHANGELOG ("Enter now opens files" is a visible change).

## Risks

- **Enter becomes an execute key.** Programs keep "Run it?" with Cancel first; nothing
  runs from one keypress. A file manager lost users on launch day over a single
  destructive key without a confirmation.
- **Text files leave the terminal.** With the system deciding, a `.txt` opens in
  TextEdit or Notepad, as in Total Commander. The first `[[open]]` rule example is
  there for people who want them viewed in rcmd; consider shipping it in the default
  config comments.
- **Windows batch files.** A `.bat`/`.cmd` re-parses its arguments through `cmd.exe`
  (CVE-2024-24576). Rust 1.77.2 and later escape them or return `InvalidInput`, and
  the MSRV is 1.88. rcmd reports that error plainly and **never retries with
  `raw_arg`**.
- **Secure modes.** Every new path that starts a program goes through `App::refused` and
  the `policy.rs` root check, like `o` today.

## Do

## Check

- [ ] Unit tests: rule matching (case, several extensions, first match, `os`,
      `needs_display`), argv building with hostile names (spaces, `&`, `;`, quotes, a
      leading `-`), config errors.
- [ ] Render tests: Enter on a file says "would open … with …" under `--screenshot`;
      Enter on a program shows "Run it?"; Enter in read-only views; Open with lists the
      rules.
- [ ] A pty test: "Run in a terminal tab" shows the program's output in a new tab.
- [ ] By hand on each system (CI cannot open windows): a PDF opens; Enter over SSH on
      Linux views; a `.cmd` rule with a hostile name is refused, not run.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/features.md`: Enter, `[[open]]` rules, Open with
- [ ] → `reference/security.md`: running programs in a tab; the batch-file case
