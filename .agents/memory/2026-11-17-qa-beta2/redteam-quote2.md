# Red-team, final pass: POSIX escaping + backtick refusal (f41e35f, d194953; branch feat/a10-qa-p0-ps2)

Clone /tmp/rt-q2 at d194953, binary /tmp/rt-q2-target/debug/xdevcmd (built after the commit, verified
up to date). Sandbox /tmp/rt-q2-sb. Shells: bash 5.2 and dash only (zsh, fish, pwsh not installed:
anything about them is READ/SUSPECT). Harness: /tmp/rt-q2-sb/h/fuzz.py (Python copy of `escaped`,
safety.rs:149-161, plus the backtick/control refusals and the `./-` rule of `word`, safety.rs:192-199;
matches the real binary's typed text in tmux byte for byte), /tmp/rt-q2-sb/h/lvl2.py.

## P0 / P1
None.

## Evidence

### Level-1 contexts: no name runs (REPRODUCED, bash + dash)
fuzz.py: 63 contexts x 3 delivery modes x {Alt+C `cd <q>\n`, Alt+Enter `<q> ` no Enter,
[[menu]] `echo %f` / `echo "%f"` / `echo '%f'` + Enter} x 47 hand-written attack names
(+120 random names from 41 fragments: `$( $(( ${ ' " \ ; & | < > ! # ) ] }`, `$'`, `esac`, `]]`,
`EOF`, `touch$IFS.PWNED`, ...). Each script = opener + typed + closer, run with bash -c / dash -c in an
empty dir; a hit = a file `PWNED` or `.PWNED`.
  - seed 1, base names: 23,230 runs, 0 hits.  seed 7, +120 random: 83,830 runs, 0 hits.
  - contexts: prompt; open " ' $'…' $"…"; $( ; "$( ; $(" ; "$(" ; $(' ; "$(' ; ${u:- ; "${u:- ;
    ${u:-" ; ${u:-' ; "${u:-' ; ${u# ; ${u/a/ (bash) ; "${u/a/ ; $(( ; "$(( ; (( ; $[ ;
    here-doc plain / 'EOF' / <<- / with $( ${ $(( inside / inside "$( ; <( ; <("; >( ; [[ == ;
    [[ =~ ; [[ == " ; case pattern ; case word ; $(case ; f() { … } (same line and next line) ;
    ( … ) ; { … } ; \ continuation (bare and inside ") ; for … in ; a=( ) ; v= ; export v= ;
    if/then ; while/do ; > redirect ; <<< (bare and ") ; '$(" ; "it's ; # comment ;
    "$("${u:- .
  - Refused combos (backtick, control char): 505 — nothing typed.

### Real binary in tmux (REPRODUCED, SHELL=/bin/bash, empty .bashrc, isolated HOME/XDG)
1. Alt+C, open double quote: `echo "abc` Enter; pane inside dir `x"$(touch$IFS.PWNED1)"`; Alt+C typed
   `cd /tmp/rt-q2-sb/w/left/x\"\$\(touch\$IFS.PWNED1\)\"`; `"` Enter -> echo printed the text,
   no .PWNED1.
2. Alt+Enter, open single quote: `echo 'abc` Enter; file `y'$(touch$IFS.PWNED2)'`; Alt+Enter typed
   `y\'\$\(touch\$IFS.PWNED2\)\' `; closed with `'` twice -> printed `y\$(touch$IFS.PWNED2)'`,
   no .PWNED2.
3. Alt+Enter on `a`touch$IFS.PWNED3`` -> nothing typed, status
   `This shell cannot be given a name holding "`" safely — not sent`.
4. Shift+F9 menu `sh -c "echo %f"` on `a\;touch$IFS.PWNED4` -> .PWNED4 CREATED (see P2-1).

### Tests (REPRODUCED)
tests/pty.rs:2630 `what_is_typed_into_an_open_quote_never_runs_a_name` passes (5 cases incl. the
backtick case added in 786d502, detects `.PWNED`); `safety` unit tests 10/10 pass.

### False refusals (REPRODUCED)
`find / -xdev` (minus /proc /sys): 434,841 paths. Holding a backtick: 2, both red-team test
artifacts (/tmp/qa/hunt-shell, /tmp/rt-q-sb) — 0 real names. For comparison, holding `\`: 3, two of
them real systemd units (`/usr/lib/systemd/system/system-systemd\x2dcryptsetup.slice`) — so a
backslash refusal would cost real names; the backtick refusal costs none here.
fish/pwsh refusals on this tree: `$(` 5, `"` 3 — all QA artifacts.

## P2 (non-blocking)

P2-1  A [[menu]] command that re-reads its text as shell code inside the user's own "…" runs a name
holding a backslash (REPRODUCED, real binary case 4 + lvl2.py, bash and dash).
  `sh -c "echo %f"`, `bash -c "…"`, `eval "…"`, `find … -exec sh -c "…" \;` with name
  `a\;touch$IFS.PWNED` -> typed `a\\;touch\$IFS.PWNED`; the outer "…" turns `\\`->`\` and `\$`->`$`,
  the inner shell sees `a\;touch$IFS.PWNED` -> runs `touch .PWNED`.
  Same templates with the user's own '…' (`sh -c 'echo %f'`, `eval 'echo %f'`): 0 hits over 167 names.
  Not a regression in kind: the pre-f41e35f single-quote form ran `$(…)` names already at level 1
  in any "…". It does make the earlier red-team's note "refusing ` fixes both" (redteam-quote.md,
  P2 list) wrong. Refusing `\` would close it but refuses real systemd unit names (above). Suggest a
  docs line at docs/config.md:330: put %f/%s bare, or inside '…' when the command hands its text to
  another shell (sh -c, ssh, eval); never inside "…". Same root as `ssh host "ls %f"`.
P2-2  The backtick refusal names no next safe action (safety.rs:167-169; docs/shell.md:97-99), against
  the red-team rule "a refusal must name the next safe action". Suggest: "… — not sent; rename it,
  or copy the path (Ctrl+…) and quote it yourself".
P2-3  `[[ %f -eq 1 ]]`, `let %f`, `test -v %f`, `declare -i`: bash evaluates the (already literal)
  word as arithmetic and runs `a[$(cmd)]` — the command interprets its argument as code; same as
  the old form (prior report, P3). Not a quoting defect.

## Carried over, unchanged (READ/SUSPECT, not re-tested: shells absent)
- fish / PowerShell: open SINGLE quote not covered (prior P1-1) — now documented at
  docs/shell.md:91-92 ("press Ctrl+C there first"); safety.rs:124-127, 93-110 unchanged.
- PowerShell `"` now refused (safety.rs:98) — prior P1-2 closed (READ + unit test safety.rs:217).
- zsh, non-UTF-8 locales: SUSPECT as before.

## Verdict
SIGN — no P0/P1 in bash or dash across 107k context runs and 3 real-binary cases; P2-1 (doc line)
and P2-2 (refusal message) recommended but not blocking.
