# Round 016: Between the shell and the panes

**Status**: In Progress
**Part of**: [ROADMAP.md](../ROADMAP.md), item 27
**Date started**: 2026-11-10
**Date completed**: —

## Goal

The smallest careful set of bridges between the terminal panel's shell and the file
panes. xdevcmd learns the shell's directory on Linux, macOS and Windows; one key takes the
pane there (the reverse of Alt+C); one key types the selected names into the shell's
command line without pressing Enter; and `[terminal] follow` lets the active pane follow the
shell's `cd`. The owner's principle (2026-11-09) governs every step: xdevcmd complements
the shell and never competes. It never types into a program, and it refuses with a
reason rather than guessing.

## Plan

Branch `feat/shell-bridge`, from `feat/terminal-history` at Round 015's Review commit.

**Decision: where the shell's directory comes from.**

| System | First | Then | Never |
|---|---|---|---|
| Linux | `/proc/<pid>/cwd` (exists: `TermPane::cwd`) | — (OSC 7 ignored: any program's output could forge it) | — |
| macOS | `/usr/sbin/lsof -a -p PID -d cwd -Fn`, **only when a key asks** (full path, never `PATH`) | OSC 7 from the shell, for `follow` | polling lsof |
| Windows | OSC 9;9 (Windows Terminal's) or OSC 7, from prompt snippets in docs/shell.md | — | guessing |

- OSC 7 `ESC ] 7 ; file://HOST/PATH ST|BEL`, accepted only when HOST is empty, `localhost`
  or this machine's name (`hostname()` from core, Round 014). Percent-decoded; `/C:/…` →
  `C:\…` on Windows; vt100's `;`-split params joined back. OSC 9;9 takes `"C:\path"` with or
  without quotes.
- Received through vt100's `Callbacks::unhandled_osc` (verify the vt100 version in
  Cargo.lock exposes it). Parser becomes `vt100::Parser<ShellReports>`. tui-term draws from
  `Screen`, unchanged.
- Same callback keeps **prompt marks** OSC 133 A/B/C/D: C = command running, A/B/D = at the
  prompt. On Windows `foreground_program` is always `None`, so marks are the only busy signal.
  Everywhere, a C with no D after counts as busy.
- A directory from any source must exist locally as a directory and be inside the policy
  root (restricted mode) before a pane goes there.

**Decision: keys.**
- **Alt+G — go to the shell's directory.** Free in panes. **Not** global, so with the shell
  focused Alt+G still goes to the shell.
- **Alt+Enter and Ctrl+Enter — type the names into the shell's command line.** mc's and
  Total Commander's keys. Legacy terminals deliver Ctrl+Enter as Enter (the file opens; seen).
  Windows Terminal keeps Alt+Enter for full screen. Files pane's right-click menu also gets
  **Type into the shell**. Files-pane only, never global.
- What is typed: marked names, else the cursor's, each quoted for the shell by
  `safety::quote`, joined by spaces, trailing space, **no Enter**. Bare names when the shell's
  directory is known and equals the pane's; otherwise full paths. Wrapped in bracketed paste
  (`ESC[200~ … ESC[201~`) when the shell turned it on (`Screen::bracketed_paste()`).

**Typing into the shell: one decision for every action that types** (owner, 2026-11-09).
Alt+C (`cd <pane dir>`) and Alt+Enter/Ctrl+Enter (names) go through one function,
`App::send_to_shell(text, purpose)`, using `TermPane::state()`:
- **Busy** (a program is known to run, or the last OSC 133 mark is C): a dialog "<program>
  is running in this shell" with **Open a new shell** (default) / **Cancel**. A new shell =
  a new tab started in the pane's directory; for Alt+C that is the whole job (no `cd` typed);
  for names, the names are typed into the new shell once its prompt is up (or, if a prompt
  cannot be detected in time, it is opened and the names are not typed — said on the status line).
- **Unknown** (Windows without OSC 133 marks, or any system that cannot tell): a dialog "xdevcmd
  cannot tell whether a program is running in this shell" with **Open a new shell** (default),
  **Send to this shell**, **Cancel**, and a line pointing to docs/shell.md for the snippet.
- **Idle** (known for sure): send directly when `[terminal] send_when_idle = true`
  (**default true**: today's behaviour on Linux and macOS, no question at every Alt+C);
  when false, the Unknown dialog is shown instead.
- **Remote** (ssh etc.): as Busy — the shell there is on another machine.
- Control characters or unquotable names: refused before any dialog (existing `safety`).
Setting in Settings → Terminal: "Send to the shell without asking when it is surely idle".
Tests: unit on the decision table `decide(state, send_when_idle) -> Send | Ask3 | Ask2`;
pty: Alt+C with `sleep 30` running shows the two-button dialog, Open a new shell opens a tab
in the pane's directory, sleep received nothing; with send_when_idle=false at an idle prompt
the three-button dialog appears; Send types the `cd`.

**Other guards, each a status-line message, nothing sent:**
- a program runs in the shell (`foreground_program` Some, or last mark is C);
- shell state unknowable (Windows without marks): "cannot tell whether a program is running
  here — see docs/shell.md" (typing only; reading the directory types nothing);
- foreground is `ssh`, `mosh-client`, `mosh`, `autossh`, `et`, `telnet`: "ssh is running — the
  shell's directory there is on another machine"; OSC 7 naming another host ignored;
- directory unknown: "the shell's directory is not known — see docs/shell.md";
- a name holds a control character or cannot be quoted for that shell (cmd and `%`): the
  existing `safety` refusal.

Steps:

- [ ] **0. Windows check, by the owner, before relying on it.** Does ConPTY through
      portable-pty pass OSC 7, 9;9 and 133? Recorded as an owner hand test; if not, Windows
      refuses with the messages above, documented.
- [ ] **1. Shell reports from the byte stream.** `pane/shell_report.rs`: pure parsers
      `osc7(params, host) -> Option<PathBuf>`, `osc9_9(params)`, `osc133(params) -> Mark`, and
      the `Callbacks` impl keeping last directory and mark. Unit tests: host empty/localhost/
      ours/other; percent escapes; `;` in a path; Windows drive form; quoted and bare 9;9; long
      path; garbage ignored without panic.
- [ ] **2. The primitive.** `TermPane::shell_dir() -> Result<PathBuf, Why>` and
      `TermPane::state() -> Idle | Busy(name) | Remote(name) | Unknown`. `lsof -Fn` output parsed
      by a pure function (unit test with recorded output). Existing `cwd()` users go through it.
- [ ] **3. Alt+G.** `Action::GoShellDir`, a `BINDINGS` row, guards. Pane history records the
      jump. Pty (Linux): `cd sub` in the shell, F12 back, Alt+G → pane shows `sub`; with
      `sleep 30` running, refused, message, pane unchanged; directory gone, refused. Render:
      F1 lists Alt+G.
- [ ] **4. Alt+Enter / Ctrl+Enter.** `Action::TypeNames`; `safety::words(shell, names)`.
      Unit: names with space, `'`, `$`, `!`, `%` for sh, fish, pwsh, cmd (cmd refuses `%`); a
      control char refuses the whole list. Pty: mark `it's` and `a b`, Alt+Enter → shell line
      reads `'it'\''s' 'a b' `, no command ran; with `cat` running, refused, cat got nothing.
      Bracketed paste used when bash turns it on.
- [ ] **5. `[terminal] follow = false`.** When on, the most recently focused files pane goes to
      the shell's directory **when it changes** (edge-triggered). Only when the shell is idle;
      on Linux folded into the existing 500 ms `/proc` read; elsewhere on OSC arrival. Not
      while showing an archive/find results or a dialog is open. Never the other way. Settings
      → Terminal; on macOS/Windows says it needs the shell to report. Pty: follow on, `cd sub`
      moves the pane; Backspace in the pane, nothing changes until the next `cd`; follow off
      (default), nothing moves.
- [ ] **5b. Mouse clicks to a program that asked for the mouse** (owner principle, Round 015
      backlog). If small and safe: clicks and drags inside the shell area go to the program
      when it has turned mouse reporting on, using the wheel forwarding's mode detection
      (SGR / UTF-8 / X10); otherwise xdevcmd keeps today's behaviour. If it turns out large or
      risky, recorded in Do for the owner instead.
- [ ] **6. Docs and hand-over.** docs/shell.md "Telling xdevcmd where the shell is": bash
      (`PROMPT_COMMAND` OSC 7), zsh (`precmd`/`chpwd`), fish, PowerShell (OSC 9;9 + 133 marks),
      cmd (`PROMPT $e]9;9;$P$e\$P$G`); only bash tested. docs/security.md: what is trusted and
      the guards. docs/keys.md regenerated, CHANGELOG. Status → Review; Round 017 starts from it.

**Risks**: ConPTY may swallow OSC; OSC 7/9;9 forgeable by program output (only ever moves a
pane, documented); Ctrl+Enter reach varies (menu fallback); parser type change touches every
`vt100::Parser` mention.

**Not doing**: own Ctrl+R/Tab completion; typing into anything not a shell at its prompt;
following inside ssh; pane `cd`-ing the shell on its own; unified directory history (v0.4);
Ctrl+Shift+Enter variants; polling lsof. (Alt+C now goes through `send_to_shell`, above.)
Note: after Round 015's keymap A, Alt+C/Alt+G/Alt+Enter are pane keys only; none is global.

## Do

- 2026-11-10, round opened on `feat/shell-bridge` from `feat/terminal-history` at
  Round 015's Review commit (`b9ddb48`). Owner decisions binding for this round: keymap A
  holds (Alt+C, Alt+G, Alt+Enter/Ctrl+Enter are pane keys); every action that types into
  the shell goes through `send_to_shell` with the decision table above; `send_when_idle`
  defaults to true; unified directory history stays in v0.4; step 5b added.

## Check

- [ ] fmt, clippy, `cargo test --workspace`, `cargo deny check`; no new crate.
- [ ] Every guard has a test that sends nothing and shows its message.
- [ ] Quoting tests per shell; a control character refuses the whole list.
- [ ] Follow edge-triggered and off by default.
- [ ] By hand, owner: macOS (lsof, OSC 7), Windows Terminal with PowerShell and cmd snippets,
      ConPTY pass-through (step 0), fish.

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : "never type into a program; refuse when its state is unknown", if it held up
