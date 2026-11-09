# Round 016: Between the shell and the panes

**Status**: Review
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
- [x] **1. Shell reports from the byte stream.** `pane/shell_report.rs`: pure parsers
      `osc7(params, host) -> Option<PathBuf>`, `osc9_9(params)`, `osc133(params) -> Mark`, and
      the `Callbacks` impl keeping last directory and mark. Unit tests: host empty/localhost/
      ours/other; percent escapes; `;` in a path; Windows drive form; quoted and bare 9;9; long
      path; garbage ignored without panic.
- [x] **2. The primitive.** `TermPane::shell_dir() -> Result<PathBuf, Why>` and
      `TermPane::state() -> Idle | Busy(name) | Remote(name) | Unknown`. `lsof -Fn` output parsed
      by a pure function (unit test with recorded output). Existing `cwd()` users go through it.
- [x] **3. Alt+G.** `Action::GoShellDir`, a `BINDINGS` row, guards. Pane history records the
      jump. Pty (Linux): `cd sub` in the shell, F12 back, Alt+G → pane shows `sub`; with
      `sleep 30` running, refused, message, pane unchanged; directory gone, refused. Render:
      F1 lists Alt+G.
- [x] **4. Alt+Enter / Ctrl+Enter.** `Action::TypeNames`; `safety::words(shell, names)`.
      Unit: names with space, `'`, `$`, `!`, `%` for sh, fish, pwsh, cmd (cmd refuses `%`); a
      control char refuses the whole list. Pty: mark `it's` and `a b`, Alt+Enter → shell line
      reads `'it'\''s' 'a b' `, no command ran; with `cat` running, refused, cat got nothing.
      Bracketed paste used when bash turns it on.
- [x] **5. `[terminal] follow = false`.** When on, the most recently focused files pane goes to
      the shell's directory **when it changes** (edge-triggered). Only when the shell is idle;
      on Linux folded into the existing 500 ms `/proc` read; elsewhere on OSC arrival. Not
      while showing an archive/find results or a dialog is open. Never the other way. Settings
      → Terminal; on macOS/Windows says it needs the shell to report. Pty: follow on, `cd sub`
      moves the pane; Backspace in the pane, nothing changes until the next `cd`; follow off
      (default), nothing moves.
- [x] **5b. Mouse clicks to a program that asked for the mouse** (owner principle, Round 015
      backlog). If small and safe: clicks and drags inside the shell area go to the program
      when it has turned mouse reporting on, using the wheel forwarding's mode detection
      (SGR / UTF-8 / X10); otherwise xdevcmd keeps today's behaviour. If it turns out large or
      risky, recorded in Do for the owner instead.
- [x] **6. Docs and hand-over.** docs/shell.md "Telling xdevcmd where the shell is": bash
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
- Steps 1–4 (`aa307c3`, `4e97930`, `5ae6756`, `c24df94`, `36c7541`), committed by a coder
  run cut off by a session restart; verified by the next run: build clean, every test
  green, nothing in them found broken.
  - 1: `pane/shell_report.rs`. vt100 0.16.2 (Cargo.lock) has `Callbacks::unhandled_osc`;
    the parser is `vt100::Parser<ShellReports>` (`term::Parser`). OSC 7 host checked
    against `journal::hostname()`; percent-decoded; `;`-split params joined back; Windows
    drive form; 9;9 quoted or bare; 133 A/B/D = prompt, C = running.
  - 2: `TermPane::shell_dir()` (Linux `/proc`; macOS `/usr/sbin/lsof -Fn` with a 2 s limit,
    then OSC 7; Windows the reported directory) and `TermPane::state()` → `state_of`, pure:
    a program the system sees always wins, a C mark is busy everywhere, a prompt mark
    makes a shell idle only where the system cannot see (Windows). Deviation from the
    plan's signature: `shell_dir` returns `Option`, the caller says why. The Shift+F12
    history's Save as… goes through `shell_dir`.
  - 3: Alt+G, `Action::GoShellDir`; refused while Busy/Remote, when the directory is
    unknown, gone or outside the root; `set_cwd` records the jump in the pane's history.
  - 4 + the owner's one decision: `App::send_to_shell(ShellSend)` with
    `term::decide(state, send_when_idle)` → Send / AskWithSend / AskNewShell. **Alt+C goes
    through it** (verified: `cd_terminal_here` builds the line, then `send_to_shell`).
    Busy/Remote → Open a new shell (default) / Cancel; Unknown, or Idle with
    `send_when_idle = false` → Open a new shell / Send to this shell (S) / Cancel, with the
    docs/shell.md pointer for Unknown. `modal::Ask` gained `also` for the middle button.
    Names: `safety::words`; bracketed paste when the shell turned it on; a new shell gets
    the names once `prompt_ready()` (idle, printed, 250 ms quiet), else after 4 s nothing
    typed and said. Found in verification: the user menu (Shift+F9) also types into the
    shell but only refused Busy/Remote on the status line and sent blindly when Unknown
    (Windows) — against "one decision for every action that types". Now through
    `send_to_shell` too (`b097d18`; pty test fails without it; `TermPanel::send`, now
    unused, removed). Plan deviation: "with `cat` running, refused" became the owner's
    two-button dialog; the pty test checks cat received nothing.
- Step 5, `[terminal] follow = false` (`3ee45f5`). `App::follow_shell` in `background()`:
  edge-triggered on the focused shell's directory (the first sighting of a shell, or a
  tab switch, only learns it); idle only; Linux reads it from the same 500 ms `/proc`
  read as the tab label (`TermPane::idle_dir`, the activity cache now also keeps the
  directory when no program has the terminal, and is not stamped while the shell has no
  pid yet); elsewhere only when a new OSC report arrived (then `state()` once — no `ps`
  every tick on macOS). Waits while a dialog, a quick search or a drag is open; a pane
  showing an archive or find results lets the change pass (not kept for later: a jump
  out of results after closing them would surprise). Never the other way. Settings →
  Terminal "Files follow the shell's cd", help line names the OSC need off Linux. Fixed
  on the way: `last_files_id` had no answer when focus went from the starting pane
  straight to the shell (`mru` empty) — follow found no pane; it now falls back to the
  first file pane. Known limit: the first directory seen is the baseline, so a `cd`
  within half a second of a shell starting (before its first `/proc` read) is not
  followed. The two Settings labels were too long for their column (seen in a
  screenshot); shortened, and a render test checks they line up.
- Step 5b, mouse (`285d49c`): small, so done. `term::mouse_report` is the wheel's encoder
  made shared (SGR `M`/`m`; UTF-8 and X10 with release as button 3); `TermPane::mouse`
  honours the mode (X10 presses only, 1000 press+release, 1002/1003 drags too). Left,
  middle and right presses inside a shell's border go to the program when it asked; the
  drag and release of a press it took follow it (`App::mouse_to_shell`), so they never
  start a file drag. The tab bar and right-click tab menu are unchanged; with no mouse
  mode a click only focuses, as before. Pty test fails without the change (checked).
- Step 6, docs (`2f8f748`): docs/shell.md "Between the panes and the shell", "Before
  anything is typed into the shell" (the table, `send_when_idle`), "Telling xdevcmd where
  the shell is" with snippets for bash (run: bash 5.2, the bytes recorded into a unit
  test), zsh, fish, PowerShell, cmd (not run, marked so), and the rule "add the marks only
  if the shell can send C" (cmd gets 9;9 only: a prompt mark without C would make a busy
  shell look idle on Windows). docs/security.md "The shell panel". docs/config.md,
  CHANGELOG, F1's mouse rows; docs/keys.md regenerated (`--print-keys`, unchanged since
  steps 3–4 had already written it). ROADMAP row 27 → review.
- Review and QA findings (2026-11-10), one commit each, each test checked to fail with
  the fix reverted:
  - Blocker, `2c91270`: Linux `foreground_of` gave up when the foreground group's
    leader had no `/proc/<pid>/comm` (`git log | less` after git ends; `true | sleep 5`)
    and that read as idle — Alt+C typed into the pipeline. Now `Fg::{Shell, Program,
    Unknown}`: a foreign group is a program ("a program" when unnamed), an unreadable
    `stat` is Unknown, which never counts as idle where the system should say (a prompt
    mark does not change that). macOS `ps` by `/bin/ps`. Pty + unit (`fg_from_stat`).
  - `329de3a`: bare names only when the shell's directory came from the system
    (`ShellDir { path, trusted }`); a reported one (OSC 7/9;9) gets full paths, and the
    history's Save as… no longer offers it. Alt+G and follow still use it. Unit test.
  - `f342bde`: a mouse gesture goes to the shell that took its press (its key and
    rectangle kept), the point clamped inside it, so a release over the panes or another
    split still reaches the program; Shift/Alt/Ctrl carried (+4/+8/+16). Pty test.
  - `bdea51b`: a new shell is ready once it printed and went quiet when the system cannot
    see the foreground (Windows without marks), so "Open a new shell" types the names
    there; where the system should see, Unknown is still not ready. Unit test.
  - `956195d`: the user menu's names go through `safety::word` (now shared with
    `words`): `-rf` becomes `./-rf`; non-UTF-8 names (any placeholder) refuse the
    command. Unit test.
  - `1242e86`: over 4000 bytes, `send_to_shell` asks even at an idle prompt
    (`decide_for`), saying some shells drop longer lines. Chosen over "bracketed paste
    only": simpler, and holds for shells that do enable paste but read through a
    canonical tty. Unit test.
  - `9e6db93`, tests for surviving mutants: a forged OSC 7 printed in the shell, Alt+G
    still goes to the `/proc` directory; follow waits while the last mark is C with the
    shell itself in the foreground, and follows after D.
  - Nits: `be5c242` — a C mark counts only after the shell has sent a prompt mark (a
    `cat` of a file cannot make a shell without snippets look busy); control characters
    in an OSC 7 path refuse it (the comment now says so). `99a79bd` — a second "Open a
    new shell" says the earlier names were dropped (status text only; no test: the
    timing in a pty is not reliable). Docs (shell.md, security.md, CHANGELOG Fixed)
    follow.
- Set aside for the owner's hand tests; Round 017 starts from it.

## Check

- [x] fmt, clippy `-D warnings`, `cargo test --workspace` (twice), pty suite 3×, all green
      on Linux (core 168 + 1, tui unit 178 + 1 ignored, render 163, pty 82 + 1 ignored,
      after the review fixes); no new crate (Cargo.toml/Cargo.lock untouched since Round 015).
      `cargo deny check` not run: cargo-deny is not installed here — CI runs it.
      Windows and macOS targets cannot be installed here (proxy): their `cfg` branches
      (`lsof_cwd_of`, macOS `foreground_of` with `/bin/ps`, the non-Linux `idle_dir` and
      `shell_dir`, `FOLLOW_HELP`) are unbuilt — CI will say.
- [x] Every guard has a test that sends nothing and shows its message (Alt+G busy/gone;
      Alt+C and Alt+Enter into `sleep`/`cat` → dialog, the program got nothing).
- [x] Quoting tests per shell; a control character refuses the whole list.
- [x] Follow edge-triggered and off by default (pty: follow on, Backspace, busy; off).
- [ ] By hand, owner: macOS (lsof, OSC 7), Windows Terminal with PowerShell and cmd snippets,
      ConPTY pass-through (step 0), fish.

## Act

**Learnings**:

- …

**Promotions**:

- [ ] → context/ : "never type into a program; refuse when its state is unknown", if it held up
