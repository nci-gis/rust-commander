# Hunt: what xdevcmd types into the shell (0.4.0-beta.1 prebuilt, Linux, bash 5.2, tmux 3.4)

Sandbox: /tmp/qa/hunt-shell. Launcher `run.sh` sets SHELL=/bin/bash, XDEVCMD_CONFIG, XDEVCMD_STATE,
XDG_DATA_HOME, HOME=$SB/home (empty .bashrc, so no prompt marks), runs
`/tmp/qa/bin/xdevcmd --no-restore "$@"` from $SB/root. Test tree in $SB/root holds directories
`$(touch PWNED)`, `;touch PWNED`, `` `touch PWNED2` ``, `q'uo"te`, `sp ace`, `pc%s%f`, `tab<TAB>name`,
`nl<LF>name`, and the files `$(touch PWNEDf).txt` and `f<LF>l.txt`.

```
SB=/tmp/qa/hunt-shell
tmux -L qash new-session -d -s x -x 120 -y 36 "$SB/run.sh $SB/root $SB/root/b"          # config.toml
tmux -L qash new-session -d -s x -x 120 -y 36 "CFG=$SB/config2.toml $SB/run.sh --root $SB/root $SB/root $SB/root/b"
```
config.toml: `max_terminals = 3`, with [[menu]] `e`=`echo F=%f`, `s`=`echo S=%s D=%d`, `t`=`touch MENURAN`.
config2.toml: `max_terminals = 1`, `follow = true`, with [[menu]] `d`=`echo "F=%f"`, `q`=`echo 'F=%f'`, `t`=`touch MENURAN`.

## Findings

### F1. A menu command with `%f` inside the user's own quotes runs a file name as code: REPRODUCED, P0
config.md:330 says "xdevcmd quotes every file name for your shell, so spaces and quotes in names are safe."
No doc warns against writing `"%f"` or `'%f'`, which is the habit from mc and from scripts.
```
# config2.toml session, cursor on the directory "$(touch PWNED)"
tmux -L qash send-keys -t x S-F9 ; tmux -L qash send-keys -t x d
# shell shows:  echo "F='$(touch PWNED)'"   ->  F=''      and $SB/root/PWNED now exists
tmux -L qash send-keys -t x F12 S-F9 ; tmux -L qash send-keys -t x q
# shell shows:  echo 'F='$(touch PWNED)''   ->  F=        and PWNED is created again
```
The quoted name is pasted inside the user's quotes. That cancels the quoting, and the command
substitution runs with Enter. xdevcmd gives no warning when it loads the config or when it sends.

### F2. Alt+C into an open double quote or a here-doc runs the directory name: REPRODUCED, P0
shell.md:89-90 says: "Not seen: an open quote or an unfinished `for … end` (the command lands inside it, harmlessly)."
The command is not harmless when the pane's directory name holds `$(...)`.
```
# config.toml session; at the shell prompt (F12):
tmux -L qash send-keys -t x -l 'echo "' ; tmux -L qash send-keys -t x Enter F12
# in the files pane, Enter on the directory "$(touch PWNED)", then
tmux -L qash send-keys -t x M-c          # types:  > cd '/tmp/qa/hunt-shell/root/$(touch PWNED)'  + Enter
tmux -L qash send-keys -t x F12 '"' Enter   # close the quote -> $SB/root/PWNED created
```
A here-doc gives the same result. `cat <<EOF` Enter, F12, Alt+C (pane in `$(touch PWNED)`), F12, `EOF` Enter
creates $SB/root/PWNED. Bash expands `$(...)` in the body of an unquoted here-doc. xdevcmd typed
at once without asking. The PS2 prompt `> ` is the same one that it does detect for a trailing `\`.

### F3. Alt+C is typed and submitted into a shell builtin `read -s` password prompt: REPRODUCED, P0/P1
shell.md:74 says "xdevcmd never types into a running program". The shell is not at its prompt.
It waits in `read`, with no child process, so /proc shows it as idle.
```
tmux -L qash send-keys -t x -l "clear; read -s -p 'pw: ' PW; echo \"got[\$PW]\"" ; tmux -L qash send-keys -t x Enter F12
tmux -L qash send-keys -t x M-c
# shell: pw: got[cd '/tmp/qa/hunt-shell/root/$(touch PWNED)']
```
The `cd` line, Enter included, became the answer. Any `read`, `select` or `while read` loop
written at the prompt works the same way, for example `read -p "Delete? [y/N]"`. No doc lists
this case. The docs list only an open quote and an unfinished `for`.

### F4. Cancelling a user-menu command's "A program is running" dialog leaves the focus in the busy shell: REPRODUCED, P1
Alt+C's dialog keeps the focus in the files pane. The Shift+F9 dialog moves the focus to the
terminal: the status line reads "terminal 1/1" while the dialog is open, and it stays there after
Esc. The user's next keys, meant for the panes, then go to the running program. Alt+Enter after
Esc put `^[` and a CR into `sleep`'s tty. When sleep ended, bash ran two empty lines.
```
tmux -L qash send-keys -t x -l "clear; sleep 40"; tmux -L qash send-keys -t x Enter F12
tmux -L qash send-keys -t x S-F9; tmux -L qash send-keys -t x t      # dialog; status: "terminal 1/1"
tmux -L qash send-keys -t x Escape                                   # still "terminal 1/1", tab ◢⌂ 1 sleep◤
tmux -L qash send-keys -t x M-Enter                                  # ^[ echoed into sleep's terminal
```
The focus also moves to the shell when a menu command is refused ("all shell tabs in use").
In that case the next keystroke joins the typed line.

### F5. max_terminals = 1 with a busy shell: the dialog offers "Open a new shell", and accepting it is a dead end: REPRODUCED, P1
shell.md:91-92 says "With every shell tab in use, nothing is typed and the status line says to press Ctrl+C there."
Instead, a dialog asks "Open a new shell … instead? [Enter]". Enter then gives
"at most 1 terminals — [terminal] max_terminals in config.toml". Nothing is typed (that held).
The grammar is also wrong: "1 terminals".
```
# config2.toml session
tmux -L qash send-keys -t x -l "clear; sleep 25"; tmux -L qash send-keys -t x Enter F12 M-c ; tmux -L qash send-keys -t x Enter
```

### F6. "all shell tabs in use" appears while other tabs sit idle at a prompt: REPRODUCED, P2
With max_terminals = 3 and tabs 1 and 3 idle at a prompt, a typed-but-not-run line in tab 2
(here `echo \` + Enter), followed by Alt+C, gives "Ctrl+C clears shell 2's line — all shell tabs in use".
The tabs are not in use. The real cause is the max_terminals limit, and xdevcmd does not offer an idle tab.

### F7. Menu dialog text shows the menu label, not the command: REPRODUCED, P2
For [[menu]] `t` (`touch MENURAN`), the "A program is running" dialog says "To type: ran touch menuran".
That text is the lowercased label with "ran" in front of it, not the command that will be typed.
The status line after a send also shows "shell: ran echo f".

### F8. Log dialog wording and a truncated reason: REPRODUCED, P2
- The dialog says "appended to:" for a name that does not exist, and also for a symlink that it will refuse.
- The refusal reason is cut in the middle: "cannot log to /tmp/qa/hunt-shel…he file itself or another name"
  (symlink) and "…lar file — choose another name" (FIFO). The word that says why (link, FIFO) is lost.
  Repro: open Alt+X F1, "Log tab to file…", Ctrl+U, type the path to a symlink or FIFO, Enter.
- On a full disk the log stops correctly (red **!**, status "No space left on device"). But the text log ends mid-line ("…12760\n127"),
  with no closing line. shell.md:202 says "a text log ends with a line saying so". On ENOSPC that cannot be
  written. Either the doc should note this, or the closing line needs reserved space. (tmpfs 64k at $SB/root/full.)

## What held (REPRODUCED)
- Quoting of hostile names: Alt+C on `$(touch PWNED)` typed `cd '/…/$(touch PWNED)'`. Alt+Enter on 6 marked names
  (`$(…)`, `;…`, backticks, `q'uo"te` → `'q'\''uo"te'`, space) typed correct single quoting, and `printf '<%s>\n'` printed
  them literally. Menu `%s`, `%d` and `%f` were also quoted, and `pc%s%f` was not expanded twice. No PWNED file from any of these.
- Control characters: names with a TAB or LF were refused whole ("control character (U+0009/U+000A) — not sent") for
  Alt+Enter and for menu `%f`.
- Busy detection: sleep, cat, `seq|less`, nested `bash --norc`, a fake `ssh` script that execs bash, and `exec sleep 20`
  all asked before typing. Choosing "Open a new shell" for menu `t` ran `touch MENURAN` in the new tab, in the pane's directory.
- A typed-but-not-run line (`echo typed`): Alt+C went to a new shell, and the line stayed. Alt+Enter names joined the line.
  A line ending in `\` was detected.
- OSC 7: `printf '\e]7;file://localhost/etc\e\\'` and a spoofed `root/b` did not move the pane (Alt+G: "already in the
  shell's directory"; follow ignored them). A real `cd /etc` gave Alt+G "outside the root". `follow` moved the pane on a real `cd` inside the root.
- Log refusals: symlink (victim file unchanged), FIFO, a file owned by `nobody`, an absolute path outside --root, `../../x`,
  and a path through a symlinked directory leading outside the root. An existing 0644 file of mine was appended to, with the
  warning "note: others can read that file". A new log was 0600.

Not tested: vim, Ctrl+U erase (documented as "press Enter once"), the over-4000-byte question, macOS and Windows.
Side effects left on disk: none. PWNED was removed after each P0 repro, the tmpfs was unmounted, and $SB/victim still holds "orig".
