# Red-team: POSIX backslash quoting fix (f41e35f, branch feat/a10-qa-p0-ps2)

Clone /tmp/rt-q, build /tmp/rt-q-target/debug/xdevcmd, sandbox /tmp/rt-q-sb. Shells on the
machine: bash 5.2, dash (/bin/sh). zsh, fish, pwsh: NOT installed (claims about them are READ/SUSPECT).
Harness: /tmp/rt-q-sb/h/esc.py (a copy of `escaped`, safety.rs:143-155), run.py (22 contexts x 16
names, bash+dash, looks for files created), rt.py (round-trip at a prompt), old.py (old vs new).

## P0-1  Open backtick: a name holding a backtick runs (REPRODUCED, regression)
safety.rs:149-151 turns `` ` `` into `` \` ``. Inside backticks one level of backslash is removed
before the body is parsed (`` \` `` -> `` ` ``, `\$` -> `$`), so the escaped backtick becomes a
live nested command substitution. Same threat model as the original P0; docs/shell.md:89-91 now
promise "a name in it never runs".

Real binary, bash, tmux (SHELL=/bin/bash, empty .bashrc):
    touch 'a`touch$IFS.PWNED`'         # in the left pane
    F12; type: echo `echo  <Enter>      # shell shows "> "
    F12; Down; Alt+Enter                # typed:  a\`touch\$IFS.PWNED\`
    F12; type: ` <Enter>
    ls -a  ->  .PWNED  a`touch$IFS.PWNED`
Harness: same hit in bash and dash for contexts `echo \`echo …\`` and `echo "\`echo …\`"`
(also applies to a backtick opened inside an unquoted here-doc, and to Alt+C's `cd …`).
Old single-quote form (old.py, same names, same contexts): no hit -> introduced by this fix.
Fix idea: no backslash form is inert both at a prompt and inside backticks; refuse names
holding `` ` `` for POSIX shells (as fish/pwsh refuse `$(`), with a message naming Ctrl+X/copy-path
or rename as the next safe action. Add the backtick case to pty.rs:2632.

## P1-1  fish and PowerShell: an open SINGLE quote is not covered (READ; no fish/pwsh here)
safety.rs:124-126 (fish) and 93-110 (pwsh) still wrap in '...'. With the user's line inside an
open `'`, the typed opening `'` closes it and the name is bare code until the next `'`:
    fish:  echo 'abc + Alt+C on dir ";touch PWNED;"  ->  echo 'abccd ';touch PWNED;''  (runs)
    pwsh:  same; '' pairs only when the name's FIRST char is ' — any other first char closes.
`refuse_breakouts` only lists double-quote breakouts (safety.rs:98, 125). Pre-existing, but
docs/shell.md:89-91 now state for every shell that a name in an open quote never runs.

## P1-2  PowerShell on Linux/macOS: `"` is not refused (READ)
safety.rs:94-100 omits ASCII `"` because "Windows names cannot hold it", but shell_kind
(safety.rs:66-80) picks Pwsh for `pwsh` on any OS. A Unix name `x";touch PWNED;"` typed into an
open "…" closes it and runs. Refuse `"` for pwsh too (cheap; the cmd branch already does).

## Checked, held (no finding)
- REPRODUCED no hit (bash+dash): prompt, open ", open ', $'…' (bash), $( , " inside $( , here-doc
  unquoted / 'EOF' / <<-, [[ ]], case, for, $(( )), (( )), ${u:-…}, \-continuation, a=( ),
  declare -A h[…]. Names: $(…), backticks, quotes, ${…}, a[$(…)], !!, \, ', EOF.
- REPRODUCED bash `set -H` inside an open ": names `!-2 !! !echo !?touch?` typed escaped, nothing
  re-run (\! stops history expansion; backslash stays as text).
- REPRODUCED round-trip at a prompt (bash, dash; printf '%s'): all 32 ASCII punctuation chars,
  doubled, `~ ~root -x =ls #a a\ \ ' {a,b} * ? [a] é e+U+0301 emoji U+00A0 ＄(x) a#b x=1 a;b` —
  0 differences. ~, {}, *, =, # stay literal.
- `let NAME` / `test -v NAME` with a name `a[$(touch PWNED)]` runs (REPRODUCED) — bash arithmetic
  re-evaluation; the old quoting did the same. Not this fix (P3 note).
- Tests real: pty.rs:2630 passes on f41e35f and FAILS at pty.rs:2660 with the old safety.rs
  swapped in (REPRODUCED). It runs /bin/sh only (pty.rs:126) and has no backtick case.
  safety.rs:203 unit test CONFIRMS only.

## P2 / SUSPECT
- Non-UTF-8 locales (SJIS/Big5): a UTF-8 last byte 0x81-0x9F read as a lead byte could swallow the
  following escape backslash (e.g. "Á" = C3 81 before `\;`). No such locale installed: SUSPECT.
- zsh (not installed): `\=`, `\~`, `\(`, `\#`, `\^` cover =cmd, glob qualifiers, extendedglob at a
  prompt (READ). RC_QUOTES in an open ': escaped text never holds two adjacent quotes. SUSPECT ok.
- fish in an open ": `$VAR[(cmd)]` (cmd-subst in an index) is not refused; whether fish runs it
  inside double quotes is unverified: SUSPECT.
- One more evaluation level (`ssh host "ls <name>"`, `sh -c "…"`, `eval "…"`): a backtick name runs
  on the far side — same root as P0-1; the old form ran it locally. Refusing ` fixes both.
- %f inside the user's own "…" in a [[menu]] command now yields `a\ b` (wrong name, not unsafe);
  the old form gave `'a b'`, equally wrong. docs/config.md:301-327 only show unquoted %f.
