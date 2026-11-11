# Your shell and xdevcmd

## Stay in the directory you were in

A program cannot change the directory of the shell that started it. But xdevcmd can tell
the shell where you were, and a small function can `cd` there.

Add one of these to your shell's startup file. Then run `xc` instead of `xdevcmd`. The
name is yours to choose; `rc` would hide the Plan 9 shell of that name.
Quit with **q** to go to that directory, or **Q** to stay where you started.

**bash / zsh** (`~/.bashrc` or `~/.zshrc`):

```bash
xc() { local f; f=$(mktemp) || return; xdevcmd --cwd-file="$f" "$@"; local d; d=$(cat -- "$f"); rm -f -- "$f"; [ -n "$d" ] && [ "$d" != "$PWD" ] && cd -- "$d"; }
```

**fish** (`~/.config/fish/config.fish`):

```fish
function xc; set f (mktemp); xdevcmd --cwd-file=$f $argv; set d (cat -- $f); rm -f -- $f; test -n "$d"; and cd -- $d; end
```

**PowerShell** (`$PROFILE`):

```powershell
function xc { $f = New-TemporaryFile; xdevcmd --cwd-file="$f" @args; $d = Get-Content -Raw $f; Remove-Item $f; if ($d) { Set-Location -LiteralPath $d } }
```

The bash version is tested. The fish and PowerShell versions have not been run yet.

## The terminal inside xdevcmd

- It runs your shell (`$SHELL`, or `[terminal] shell` in the config).
- **F12** moves between the panes and the terminal.
- **Shift+F12** shows the shell's history in the viewer, to read, search and copy; it is
  never written to disk. The paths in it can be opened: see
  [Paths in the output](#paths-in-the-output).
- **Inside the terminal, xdevcmd takes only F12, Shift+F12 and Alt+X**; every other key
  is the program's. Alt+X, then a terminal key (Alt+S split, Alt+W close, Alt+N new tab,
  Alt+1…9 tab N) acts on the shell; anything else after Alt+X, Enter too, goes to the
  program with it; Esc cancels.
- **The mouse inside the terminal** is the program's when it asked for it (vim with
  `mouse=a`, htop, Claude Code): clicks, drags and the wheel are reported to it, as a
  terminal would. Otherwise a click only gives the shell focus, and the wheel scrolls
  back through the history.

### Between the panes and the shell

These are keys in the file panes, not in the terminal: there, Alt+C and Alt+G are the
program's.

- **Alt+C** types `cd <this directory>` into the shell, quoted for your shell.
- **Alt+Enter** or **Ctrl+Enter** types the marked names (else the one under the cursor)
  into the shell's command line, each quoted, a space after the last, and **no Enter**:
  you finish the command. Bare names when the shell is in their directory, full paths
  otherwise. The files' right-click menu has it as **Type into the shell** (many
  terminals send Ctrl+Enter as plain Enter, and Windows Terminal keeps Alt+Enter for
  full screen).
- **Alt+G** takes the file pane to the shell's directory, the reverse of Alt+C. It types
  nothing. It refuses while a program runs in the shell, when that program is `ssh`
  (the directory that matters is on another machine), and when the directory is not
  known, no longer there, or outside the root xdevcmd is confined to.
- **`[terminal] follow = true`** (Settings → Terminal, off by default): the file pane you
  used last goes where the shell's `cd` goes, at the prompt only. Moving the pane
  yourself changes nothing until the shell's next `cd`; the pane never moves the shell.
  It waits while a dialog is open, and leaves a pane showing an archive or find results
  alone.

### Before anything is typed into the shell

xdevcmd complements the shell; it never types into a program. Alt+C, Alt+Enter and the
user menu (Shift+F9) go through the same check first:

| The shell is… | What happens |
|---|---|
| at its prompt, surely | Typed at once (with `[terminal] send_when_idle = true`, the default). With it off, the question below with three buttons. |
| running a program, or `ssh` | A question: **Open a new shell** (default) or **Cancel**. Nothing is typed into the program. |
| in a state xdevcmd cannot tell | A question: **Open a new shell** (default), **Send to this shell** (S), or **Cancel**, with a pointer to this page. |

**Open a new shell** starts a new terminal tab in the pane's directory. For Alt+C that
is the whole job. Names, or the user menu's command, are typed there once its prompt is
up; if no prompt
shows within a few seconds, the shell stays open and nothing is typed (the status line
says so). A name with a control character, or one your shell cannot be given safely
(`%` for cmd), is refused before any question. Text over 4000 bytes is always asked
about first, even at an idle prompt: a shell without a line editor of its own (dash,
sh, cmd) silently drops a line longer than about 4 KB.

"Surely at its prompt" comes from the system on Linux (`/proc`) and macOS (`ps`). On
Windows the system does not say; only the prompt marks below can tell, and without them
every send asks.

## Paths in the output

In the history (Shift+F12), the paths that exist on disk are underlined: compiler and
grep locations (`src/main.rs:42:9`, `main.c:7:`), Python's `File "x.py", line 12`,
MSBuild's `Program.cs(12,5)`, `ls` listings, `~/…`, `C:\…`, `.\…`, and a quoted name
with spaces. URLs, version numbers and paths on another machine (`\\server\…`,
`//host/…`) are left alone.

| Key | Does |
|---|---|
| Tab · Shift+Tab | The next / previous path; the line under the viewer says its full path, line, and the directory it was taken from |
| Enter, or a click on the chosen path | The file pane goes there, the cursor on the file |
| F3 | View the file at its line; Esc comes back to the history |
| F4 | Edit the file at its line |
| Esc | Let go of the path (a second Esc closes the history) |

**A relative path** is taken from the directory its command ran in, when the shell
sends prompt marks (OSC 133, below): `cd a; make` then `cd b` still finds `make`'s
`src/x.c` in `a`. Without prompt marks, or for lines before the first one, it is taken
from the shell's directory now, and the line under the viewer says so; a path that is
not there from that directory is not offered.

**The line goes to the editor** for those whose way of saying it is known: vi, vim,
nvim, nano, emacs, micro, kak, mg, joe (`+N file`); hx, subl, zed (`file:N:C`); code,
codium, cursor, windsurf (`-g file:N:C`); notepad++ (`-nN file`). Any other editor gets
the file alone, and the status line says the line was not passed.

What a line of output says is not trusted: any program can print any path. So a path
is only offered, never opened by itself; each key shows its full path first, and it is
checked as a file pane's would be (the root xdevcmd is confined to; F4 is off in the
read-only modes).

## A list from the output

A command's output can become a list in a file pane, to work on with the panes' keys:
`rg -l TODO`, `git status --porcelain`, `find . -name '*.orig'`, `grep -rn`, a build's
errors. In the history (Shift+F12):

| Key | Does |
|---|---|
| `p` | A new list of the paths in the lines dragged over; with nothing selected, in the output of the command the chosen path is in, else of the last command (this needs prompt marks, below) |
| `P` | Add those paths to a list: a menu, the list used last first (Enter), the others by number, `n` for a new one. A path already there is not added twice |

A terminal tab's right-click menu has **List from the last command…** too.

In the list, every row is a real path: F3, F4, F5, F6, F8, marking and renaming work as
in a directory, and Enter on a file goes to its directory with the cursor on it.
Without prompt marks and with nothing dragged over, nothing is guessed: drag over the
lines first.

**Lists are kept** for the session, the last 10, in memory only (never written to
disk, as the history is not):

- **Back** (Alt+, or Backspace) after Enter on a row comes back to the list, the cursor
  on that row. Alt+F7's results are kept the same way.
- **Alt+L** shows the list used last, from anywhere; from inside the shell, Alt+X then
  Alt+L.
- **Alt+Down** (or **Alt+J**) lists them above the directories visited; the menu bar
  has View → History and lists…, and View → Last list.
- Coming back to a list checks each row again: a path deleted or moved away since is
  named on the status line ("1 gone since: y.txt") and dropped.

A list holds at most 10,000 rows, and none outside the root xdevcmd is confined to; the
title says when either left rows out. While `ssh` (or mosh, telnet) runs in the shell,
its output names the other machine's files, so no list is made of it, and a path
chosen there says it is looked up on this machine.

On macOS every Alt key needs "Use Option as Meta key" (Terminal → Settings → Profiles
→ Keyboard; iTerm2: Option key "Esc+"). `p`, `P` and the menu bar need nothing.

## Telling xdevcmd where the shell is

| System | The shell's directory | Whether a program runs |
|---|---|---|
| Linux | read from `/proc`; nothing to set up | read from `/proc` |
| macOS | asked of `/usr/sbin/lsof` when a key needs it; `follow` needs OSC 7 | `ps` |
| Windows | only what the shell reports: OSC 9;9 or OSC 7 | only the prompt marks, OSC 133 |

A shell reports by printing short escape sequences at each prompt, the same ones
Windows Terminal, iTerm2, WezTerm and VS Code read:

- **OSC 7** `ESC ] 7 ; file://HOST/PATH ESC \` — the directory, percent-encoded. HOST
  must be empty, `localhost` or this machine's name; any other host is ignored.
- **OSC 9;9** `ESC ] 9 ; 9 ; "C:\path" ESC \` — Windows Terminal's form.
- **OSC 133** prompt marks — `A` a prompt starts, `C` a command starts running, `D` it
  finished. A `C` with no `D` after it means busy, on every system, once the shell has
  sent a prompt mark (`A`, `B` or `D`); before that a `C` is ignored, so a file that
  holds one cannot make a shell without the snippet look busy. A reported directory
  with a control character in it is ignored.

**Only add the marks if your shell can send `C`** when a command starts. A shell that
prints `A` at its prompt but never `C` looks idle while a program runs, and on Windows
xdevcmd would then type into that program. cmd cannot send `C`: give it only OSC 9;9.

These snippets go in your shell's startup file. Only bash's has been run (bash 5.2, on
Linux); the others are written from each shell's manual and **have not been run**. The
rest is up to you: your prompt, your framework (starship, oh-my-zsh, oh-my-posh) may
already send some of these, and xdevcmd reads them whoever sends them.

**bash** (`~/.bashrc`; `PS0` needs bash 4.4 or later). Tested:

```bash
__xdevcmd_osc7() {
  local LC_ALL=C p=$PWD out= c i
  for ((i = 0; i < ${#p}; i++)); do
    c=${p:i:1}
    case $c in
      [-/._~A-Za-z0-9]) out+=$c ;;
      *) printf -v c '%%%02X' "'$c"; out+=$c ;;
    esac
  done
  printf '\e]7;file://%s%s\e\\' "$HOSTNAME" "$out"
}
PS0=$'\e]133;C\a'
PROMPT_COMMAND='printf "\e]133;D\a"; __xdevcmd_osc7; printf "\e]133;A\a"'"${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
```

**zsh** (`~/.zshrc`). Not run:

```zsh
__xdevcmd_prompt() {
  emulate -L zsh; setopt extendedglob; local LC_ALL=C
  print -n '\e]133;D\a'
  printf '\e]7;file://%s%s\e\\' "$HOST" "${PWD//(#m)[^A-Za-z0-9\/._~-]/%${(l:2::0:)$(([##16]#MATCH))}}"
  print -n '\e]133;A\a'
}
__xdevcmd_exec() { print -n '\e]133;C\a'; }
precmd_functions+=(__xdevcmd_prompt)
preexec_functions+=(__xdevcmd_exec)
```

**fish** (`~/.config/fish/config.fish`). Not run:

```fish
function __xdevcmd_prompt --on-event fish_prompt
    printf '\e]133;D\a\e]7;file://%s%s\e\\\e]133;A\a' (hostname) (string escape --style=url -- $PWD)
end
function __xdevcmd_exec --on-event fish_preexec
    printf '\e]133;C\a'
end
```

**PowerShell** (`$PROFILE`; PSReadLine, as PowerShell 7 and Windows PowerShell 5.1 ship).
Not run:

```powershell
$__xdevcmdPrompt = $function:prompt
function prompt {
    $e = [char]27; $a = [char]7
    $loc = $executionContext.SessionState.Path.CurrentLocation
    $out = "$e]133;D$a"
    if ($loc.Provider.Name -eq 'FileSystem') { $out += "$e]9;9;`"$($loc.ProviderPath)`"$e\" }
    "$out$e]133;A$a" + (& $__xdevcmdPrompt)
}
Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    [Console]::Write("$([char]27)]133;C$([char]7)")
}
```

**cmd** (set it with `setx PROMPT` or in the `PROMPT` variable; the directory only,
no marks, as said above). Not run:

```bat
PROMPT $e]9;9;$P$e\$P$G
```

Whether Windows' console (ConPTY) passes these sequences through to xdevcmd has not
been checked yet. If it does not, Alt+G says the directory is not known, and every send
asks.

What a shell reports is text in its output, and any program printing into the terminal
can print the same. [security.md](security.md#the-shell-panel) says what xdevcmd does
and does not trust it for.

## Tab completion and the manual page

xdevcmd prints its own completions and manual page; packages install them for you.

```bash
xdevcmd --print-completions bash > ~/.local/share/bash-completion/completions/xdevcmd
xdevcmd --print-completions zsh  > ~/.zfunc/_xdevcmd        # a directory on $fpath
xdevcmd --print-completions fish > ~/.config/fish/completions/xdevcmd.fish
xdevcmd --print-man | man -l -                               # read it once
```

PowerShell: `xdevcmd --print-completions powershell | Out-String | Invoke-Expression`, or
dot-source its output from your `$PROFILE`. The bash script is tested; zsh, fish and
PowerShell are generated from the same list of options but not yet run.
