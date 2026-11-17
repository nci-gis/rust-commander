# Red team: PS2 fix (feat/a10-qa-p0-ps2, e5f32c8 + a81621a)

Clone /tmp/rt-ps2, binary built at /tmp/rt-ps2-target/debug/xdevcmd, sandbox /tmp/rt-ps2-sb
(HOME, XDEVCMD_CONFIG, XDEVCMD_STATE, XDG_DATA_HOME under it, SHELL=/bin/bash, empty .bashrc),
driven with `tmux -L rtps2` (120x40). Folder `/tmp/rt-ps2-sb/a/$(touch PWNED)`.

Verdict: REFUSE as a fix for the P0. The check reads the screen through a view that the
user can scroll, and the original exploit still runs end to end.

## P0-1 Terminal scrolled back: the check reads the wrong row, the cd is typed into the open quote and PWNED is created. REPRODUCED

Cause (READ): `cursor_row` (crates/xdevcmd/src/pane/term.rs:1059-1068) takes
`screen.cursor_position()` (the live screen) but indexes `screen.rows(..)`, which in vt100
0.18.1 is `grid().visible_rows()`. That view is shifted by the scrollback offset
(vt100 grid.rs:197-205). The mouse wheel over the shell scrolls it (term.rs:935), and nothing
scrolls it back down before the check. `type_text` does scroll back down
(term.rs:810), but only after the decision has been made. Both `dirty()` (term.rs:1074) and
`unfinished_target` (shell.rs:805-809) are affected, so Alt+C, the user menu and Alt+Enter
all get through.

Steps: F12, `seq 40` Enter, `echo "` Enter, F12, Down, one wheel notch up over the shell
(`\e[<64;20;35M`, which shows "↑ 3 lines back"), Enter (into the folder), Alt+C:
```
│root@vm:/tmp/rt-ps2-sb/a# echo "
│> cd '/tmp/rt-ps2-sb/a/$(touch PWNED)'
│> █
 status: shell: cd /tmp/rt-ps2-sb/a/$(touch PWNED)
```
F12, `"` Enter, then `find /tmp/rt-ps2-sb -name PWNED` prints `/tmp/rt-ps2-sb/a/PWNED`.
Alt+Enter at the same point (after `seq 60`; `echo "`; one wheel notch) typed
`> '$(touch PWNED)' ` into the quote (status `shell: '$(touch PWNED)'`).
Fix: read the row at the live position, without the scrollback offset (scroll to 0 under the
write lock first, or index `drawing_rows`). Also add a pty test that scrolls before Alt+C.
The `\`-continuation check at term.rs:1095 uses the same function and so has the same flaw,
which predates this fix.

## P1-1 Output printed after the `> ` moves the cursor off the prompt row. REPRODUCED

`(sleep 2; echo done) &`, `echo "` Enter, wait 3 s, Alt+C. Bash does not redraw PS2 after
async output, so the cursor row is empty, `continuation_prompt` is false and the dirty flag is
false (Enter cleared it):
```
│root@vm:/tmp/rt-ps2-sb/a# echo "
│> done
│cd '/tmp/rt-ps2-sb/a/$(touch PWNED)'
```
The cd ended up inside the open quote. A background job's output, a `wall` message or a
`notify` would all do this. The check is a single-row screen heuristic. A sturdier way to know
"the line is not finished" is the state since the last Enter, for example the prompt marks:
no new OSC 133 A/B after the Enter means the line is not finished.

## P1-2 PowerShell (`>> `) and fish (no prompt) are not covered. READ / SUSPECT

- `continuation_prompt` (term.rs:1539-1547): on PowerShell's PSReadLine continuation prompt
  `>> `, `find("> ")` gives 1 and words=">" is not lowercase, so the result is false (READ, from
  the code). In PowerShell, `$( )` inside an open `"` is a subexpression, so
  `Set-Location -LiteralPath '...$(calc)...'` (safety.rs:48) typed into it runs on close:
  the same P0 on Windows (SUSPECT: no Windows here). docs/shell.md says only "`> `" and zsh,
  so it neither covers nor warns about this case.
- fish (a supported kind, safety.rs:78; docs/shell.md:282) keeps an unclosed `"` in its
  editor with no continuation prompt. fish 3.4+ expands `$(...)` inside double quotes, so
  the check misses it (SUSPECT: fish is not installed here).
- zsh: the lowercase-words rule covers `dquote> `, `quote> `, `bquote> `, `cmdsubst> `,
  `heredoc> `, `cursh> `, `then> ` (READ, not run: zsh is not installed).
- Bash `$(`, backtick, `{`, `if`, a single quote and a function body all use PS2 `> `,
  so they are covered while the view is not scrolled (READ). A custom PS2 is documented as
  a known gap (docs/shell.md diff).

## P2 False refusal: a primary prompt such as `\W> ` (`a> `, `src> `) matches. REPRODUCED

`PS1='\W> '` in dir `a` shows `a> `, and Alt+Enter then always says "shell 1's line is not
finished (> ) — nothing typed; Ctrl+C clears it". Ctrl+C does not help, so the refusal is a
dead end (red-team rule: "a refusal must name the next safe action"). Alt+C opens a new
shell each time until the tabs run out. Any lowercase-only directory with a `> `-terminated
PS1 triggers this. Not documented (only the code comment at term.rs:1537-1538 mentions it).
PowerShell `PS C:\> ` does not match (uppercase/`:`/`\`), and cmd `C:\>` has no space
after `>`, so it does not match either (READ term.rs:1539-1547).

## Senders (READ)

Every path that types goes through `send_to_shell`/`send_here` (shell.rs:700-869):
Alt+C (`cd_terminal_here`, shell.rs:613), Alt+Enter, Ctrl+Enter and the right-click "Type into
the shell" (`Action::TypeNames`, input.rs:547 → shell.rs:649), the user menu, Shift+F9 and
the F1 rows (open.rs:459), and "S send to this shell" (`Cmd::ShellSendHere`, input.rs:749 →
`send_here`, which checks again). The only other writer is `pump_pending_type`
(shell.rs:933), which types into a new shell at its first prompt. That is ungated, but a new
shell is not normally at PS2. All of them inherit P0-1 and P1-1, because the check underneath
is the same.

## Tests

- pty.rs `alt_c_at_a_continuation_prompt_opens_a_new_shell` is real: it passes on the
  branch and fails on 8f9e72e (no fix) with "typed into the open quote" at
  pty.rs:2645 (REPRODUCED, `cargo test -p xdevcmd --test pty alt_c_at_a_continuation`).
  It checks the screen, not the disk: it never closes the quote, so it does not look for
  PWNED. The Alt+Enter half was not shown to fail on its own, because the run fails earlier.
- The unit test `continuation_prompts_are_recognised` (term.rs) only confirms. It does not
  include `>> `, a scrolled view or `a> ` (which it would accept).
- No test scrolls the view or prints output after the `> `.
