# Round 005: Enter opens files the Total Commander way

**Status**: Review
**Part of**: [ROADMAP.md](../ROADMAP.md), item 3
**Replaces**: [Round 002](Round_002.md), the same plan, renumbered so rounds run in
number order (owner, 2026-11-07)
**Date started**: 2026-11-07 (owner: "1->5 làm nhé", target v0.2.0)
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
- **Terminal tabs** start from a string split on spaces (`TermPane::spawn(…, shell)`),
  so a program whose path has a space cannot be given to it as is; and a tab whose
  process exits is removed (`TermTabs::reap`), which would take a program's output with
  it. Step 1 has to change both.
- **Finding (review, 2026-11-07)**: `o` decides "no desktop to open in" from `DISPLAY` /
  `WAYLAND_DISPLAY`, on Linux only. Over SSH into a **Mac or a Windows machine**, `open`
  and `explorer.exe` would open the file on that machine's own screen, or fail, never on
  the screen in front of the user.

## What Enter will do

| Under the cursor | Enter |
|---|---|
| a directory, or an archive rcmd can browse | go in (unchanged) |
| a program, script, `.desktop` or `.lnk` | "Run it?": **Run in a terminal tab** (default after Cancel), Run with the system, Cancel |
| a file with an `[[open]]` rule for its extension | what the rule says |
| any other file | the system's program for it, as `o` |
| anything, when nothing can be opened (over SSH on any system, no desktop, read-only, restricted) | view it (F3), and say why on the status line |

`[files] enter = "open" | "view"`. **Default `open`**, as the owner decided on
2026-11-06 (ROADMAP, Decisions); `view` keeps 0.1.0's behaviour.

## Plan

- [ ] 0. **"Can anything be opened here?"** in one place, used by `o`, Enter and the
      rules: not over SSH (`SSH_CONNECTION` or `SSH_TTY` set) on any system; on Linux
      and the BSDs, not without `DISPLAY` / `WAYLAND_DISPLAY`; never in read-only or
      restricted. Fixes the SSH finding for `o` today, before Enter depends on it.
- [ ] 1. **"Run it?" runs a console program in a terminal tab**: a new tab of the
      terminal panel started on the program's argv directly, never typed into a shell
      (so rule 5 holds without `safety.rs`). "Run with the system" stays as a choice.
      Fixes the finding above and is what step 3's `terminal = true` reuses. Needs:
      `TermPane::spawn_argv` (an argv, no splitting); a tab started for a program stays
      after the program ends, showing its output and "exited (status N) — Enter or Esc
      closes"; its title is the program's name. Refused where the terminal is
      (restricted) and where running programs is (read-only).
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
- [ ] 4. **Open with…** (Shift+O, unbound today; the File menu and the right-click menu): the rules that
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
- **Enter changes for everyone who has 0.1.0.** It viewed; it opens. The CHANGELOG says
  so first under Changed, with `enter = "view"` to keep the old way, and Settings
  (Round 007) will offer it.
- **Over SSH, Enter views.** On a server that is the useful behaviour, and the status
  line says why, so it does not look like Enter is broken.

## Do

**2026-11-07.** Round 004 still waits for the owner's Mac test; this round started on the
owner's go-ahead to work through the roadmap toward v0.2.0.

- Step 0: `opener::no_screen()` — SSH (`SSH_CONNECTION` / `SSH_TTY`) on every system,
  no `DISPLAY` / `WAYLAND_DISPLAY` on Linux and the BSDs. `o`, Enter and
  `needs_display` rules all ask it. Fixes `o` over SSH into a Mac or Windows machine.
- Step 1: `TermPane::spawn_argv` (argv, no splitting), `keep` and `ended` ("exit N");
  `TermPanel::add_program_tab`, `focused_program_ended`; `reap` leaves kept tabs; the
  pane's last row says "<name> ended (exit N) — Enter or Esc closes"; Enter or Esc closes
  it and focus returns to the last file pane; kept tabs are not saved in the session.
  "Run it?" now: Cancel, Run in a terminal tab (console programs: the exec bit on Unix,
  .exe .com .bat .cmd on Windows), Run with the system.
- Step 2: `[files] enter = "open"` (default) | `"view"`; `App::enter_file`. Inside an
  archive Enter still views. Where nothing can be opened the file is viewed and the
  reason is written in the viewer's own bar, which covers the status line (first try
  wrote only the status line, invisible behind the viewer).
- Step 3: `[[open]]` (`OpenRule`: ext, action, command, terminal, needs_display, os),
  validated at load; first match wins; `command` is an argv.
- Step 4: Shift+O, `open_with_menu`: matching rules (needs_display ones greyed out over
  SSH), System default, View, Edit, Run in a terminal tab; in the File and right-click
  menus.
- Step 5: guide, config ("Opening files"), troubleshooting, CHANGELOG, keys.md.
- Tests start as a local desktop session everywhere (render and pty strip the SSH
  variables and set `DISPLAY`), so CI on three systems sees the same thing; this session
  itself runs over SSH, which is how the need showed.

## Check

- [ ] Unit tests: rule matching (case, several extensions, first match, `os`,
      `needs_display`), argv building with hostile names (spaces, `&`, `;`, quotes, a
      leading `-`), config errors. Done: config errors (8 cases), matching (case, os,
      extension). **Not done:** hostile names through a rule's argv — the argv is built
      by pushing the path as one `OsString`, never joined, but no test proves it.
- [x] Render tests: Enter on a file says "would open … with …" under `--screenshot`;
      Enter on a program shows "Run it?"; Enter in read-only views; Open with lists the
      rules. Plus: `enter = "view"`, each rule kind, a rule for another OS, a
      needs_display rule over SSH.
- [x] A pty test: "Run in a terminal tab" shows the program's output in a new tab, and
      the tab is still there after the program ends. (Unix; Enter closes it.)
- [x] Render tests with `SSH_CONNECTION` set: Enter views, `o` refuses with the reason,
      on every system (an environment variable, so CI covers all three).
- [ ] By hand on each system (CI cannot open windows): a PDF opens; Enter over SSH on
      Linux views; a `.cmd` rule with a hostile name is refused, not run.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

-

**Promotions**:

- [ ] → `reference/features.md`: Enter, `[[open]]` rules, Open with
- [ ] → `reference/security.md`: running programs in a tab; the batch-file case
