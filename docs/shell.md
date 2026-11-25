# Your shell and xdevcmd

xdevcmd has a shell panel under the file panes. It starts a new shell there, of the
kind you use: not the session you started xdevcmd from.

**In short**

- **F12** moves between the file panes and the shell panel. Inside the shell, xdevcmd
  takes only F12, Shift+F12 and Alt+X; every other key is the program's.
- **Alt+C** types `cd <this directory>` into the shell; **Alt+G** brings the pane to the
  shell's directory; **Alt+Enter** types the marked names.
- **Shift+F12** shows the shell's history: open the paths in it, or make a list of files
  from a command's output.
- **To end up in xdevcmd's last directory when you quit**, add the `xc` function below.
- On Linux this all works as it is. On macOS and Windows, a few lines in your shell's
  startup file make it surer: [Telling xdevcmd where the shell is](#telling-xdevcmd-where-the-shell-is).

## Stay in the directory you were in

A program cannot change its parent shell's directory, but xdevcmd can write where you
were, and a small function can `cd` there. Add one of these to your shell's startup
file, then run `xc` instead of `xdevcmd` (the name is yours; `rc` would hide the Plan 9
shell). Quit with **F10** to go to that directory, or **Alt+X q** to stay where you started.

**bash / zsh** (`~/.bashrc` or `~/.zshrc`). Tried by hand (no test runs it):

```bash
xc() { local f; f=$(mktemp) || return; xdevcmd --cwd-file="$f" "$@"; local d; d=$(cat -- "$f"); rm -f -- "$f"; [ -n "$d" ] && [ "$d" != "$PWD" ] && cd -- "$d"; }
```

**fish** (`~/.config/fish/config.fish`). Not run yet:

```fish
function xc; set f (mktemp); xdevcmd --cwd-file=$f $argv; set d (cat -- $f); rm -f -- $f; test -n "$d"; and cd -- $d; end
```

**PowerShell** (`$PROFILE`). Not run yet:

```powershell
function xc { $f = New-TemporaryFile; xdevcmd --cwd-file="$f" @args; $d = Get-Content -Raw $f; Remove-Item $f; if ($d) { Set-Location -LiteralPath $d } }
```

## The shell panel

- It starts a new shell: `[terminal] shell` in the config, else `$SHELL` (Windows:
  `%COMSPEC%`).
- **Keys inside it**: xdevcmd takes only F12, Shift+F12 and Alt+X. **Alt+X, then a
  terminal key** acts on the shell (Alt+S split, Alt+W close, Alt+N new tab, Alt+1…9
  tab N), **q** quits. Anything else after Alt+X, Enter too, goes to the program with
  it; Esc cancels. **Alt+X F1** opens Commands, since F1 alone is the program's (mc, htop and
  emacs use it).
- **The mouse** is the program's when it asked for it (vim with `mouse=a`, htop, Claude
  Code). Otherwise a click gives the shell focus, and the wheel:
  - in a full-screen program (less, man): sends ↑/↓, three a notch;
  - at the prompt: scrolls back through the history.
- **Shift+F12** shows the history in the viewer, to read, search and copy. It is written
  to disk only when you ask: F2 there, **Save history as…** in the tab's right-click menu,
  or [a log](#logging-a-tab-to-a-file).

### Between the panes and the shell

Pressed in the file panes (in the shell, Alt+C and Alt+G are the program's):

| Key | Does |
|---|---|
| **Alt+C** | Types the shell's own `cd` with this directory, quoted, and Enter: `cd` (sh, bash, zsh, fish), `cd /d` (cmd), `Set-Location -LiteralPath` (PowerShell) |
| **Alt+Enter** or **Ctrl+Enter** | Types the marked names (else the one under the cursor), each quoted, with a space after the last and **no Enter**: you finish the command. Bare names when the shell is in their directory, full paths otherwise. Also **Type into the shell** in the files' right-click menu, since many terminals send Ctrl+Enter as Enter and Windows Terminal keeps Alt+Enter for full screen |
| **Alt+G** | Takes the pane to the shell's directory; types nothing. Refused while a program runs in the shell, when it is `ssh` (the directory is on another machine), and when the directory is unknown, gone, or outside xdevcmd's root |

**`[terminal] follow = true`** (Settings → Terminal; off by default): the file pane used
last goes where the shell's `cd` goes, at the prompt only. Moving the pane yourself
changes nothing until the next `cd`; the pane never moves the shell. It waits while a
dialog is open, and leaves a pane showing an archive or find results alone.

### Before anything is typed into the shell

xdevcmd never types into a running program. Alt+C, Alt+Enter and your own commands
(Shift+F9) are checked first:

| The shell is… | What happens |
|---|---|
| surely at its prompt | Typed at once (`[terminal] send_when_idle = true`, the default). With it off, the three-button question below. |
| running a program, or `ssh` | Asks: **Open a new shell** (default) or **Cancel**. Nothing is typed into the program. |
| in a state xdevcmd cannot tell | Asks: **Open a new shell** (default), **Send to this shell** (S), or **Cancel**. |
| at its prompt with a line you typed and did not run | Alt+C and your commands go to a new shell; your line stays. Alt+Enter's names still join it. |

- **Open a new shell** starts a terminal tab in the pane's directory. For Alt+C that is
  all. Names or a command are typed once its prompt is up; if none shows within a few
  seconds, the shell stays open and nothing is typed (the status line says so).
- **"Typed and not run"** means any key since your last Enter or Ctrl+C (Linux, macOS);
  a line ending in `\` counts. After `q` in less, or erased text, press Enter once.
  Not seen: an open quote, a here-doc or an unfinished `for … end`: the command lands
  inside it as text, and a name in it never runs (each character a shell acts on is
  escaped). In fish and PowerShell, an open single quote is not covered yet: press
  Ctrl+C there first. Nor is a program built into the shell that reads a line, such as bash's
  `read -s`: the shell looks idle and Alt+C is typed as its answer; the prompt marks
  ([below](#telling-xdevcmd-where-the-shell-is)) show it as busy.
- **With every shell tab in use**, nothing is typed and the status line says what to do:
  Ctrl+C in that shell (Linux, macOS), or close a shell tab (Windows).
- **Refused before any question**: a name with a control character, or one your shell
  cannot be given safely: a backtick for sh, bash and zsh; `%`, `"`, `!` or `^` for cmd;
  `$(` or `"` for fish; `$(` or a double quote (plain or typographic) for PowerShell.
- **Over 4000 bytes** always asks first, even at an idle prompt: dash, sh and cmd drop a
  line longer than about 4 KB without a word.
- **How xdevcmd knows**: from the system on Linux (`/proc`) and macOS (`ps`); a program
  the shell `exec`ed is seen on Linux only. On Windows only the prompt marks
  ([below](#telling-xdevcmd-where-the-shell-is)) can tell; without them every send asks.

## Paths in the output

In the history (Shift+F12), paths that exist on disk are underlined: compiler and grep
locations (`src/main.rs:42:9`, `main.c:7:`), Python's `File "x.py", line 12`, MSBuild's
`Program.cs(12,5)`, `ls` listings, `~/…`, `C:\…`, `.\…`, and a quoted name with spaces.
URLs, version numbers and paths on another machine (`\\server\…`, `//host/…`) are left
alone.

| Key | Does |
|---|---|
| Tab · Shift+Tab | The next / previous path; the line under the viewer shows its full path, line, and the directory it was taken from |
| Enter, or a click on the chosen path | The file pane goes there, the cursor on the file |
| F3 | View the file at its line; Esc comes back to the history |
| F4 | Edit the file at its line |
| Esc | Let go of the path (a second Esc closes the history) |

- **A relative path** is taken from the directory its command ran in, when the shell
  sends prompt marks: `cd a; make`, then `cd b`, still finds `make`'s `src/x.c` in `a`.
  Without them it is taken from the shell's directory now (the line under the viewer
  says so), and a path not found from there is not offered.
- **The line number reaches the editor** for: vi, vim, nvim, nano, emacs, micro, kak,
  mg, joe (`+N file`); hx, subl, zed (`file:N:C`); code, codium, cursor, windsurf
  (`-g file:N:C`); notepad++ (`-nN file`). Others get the file alone, and the status
  line says so.
- **Output is not trusted**: any program can print any path. A path is only offered,
  never opened by itself; each key shows the full path first, and it is checked as a
  file pane's would be (xdevcmd's root; F4 is off in the read-only modes).

## A list from the output

Turn a command's output into a list in a file pane and work on it with the panes' keys:
`rg -l TODO`, `git status --porcelain`, `find . -name '*.orig'`, `grep -rn`, a build's
errors. In the history (Shift+F12):

| Key | Does |
|---|---|
| `p` | A new list of the paths in the lines dragged over; with nothing selected, of the output of the command the chosen path is in, else of the last command (needs prompt marks) |
| `P` | Add those paths to a list: a menu, the list used last first (Enter), the others by number, `n` for a new one. A path already there is not added twice |

A terminal tab's right-click menu also has **List from the last command…**. Without
prompt marks and with nothing dragged over, nothing is guessed: drag over the lines
first.

**In the list** every row is a real path: F3, F4, F5, F6, F8, marking and renaming work
as in a directory, and Enter on a file goes to its directory with the cursor on it. A
list holds at most 10,000 rows and nothing outside xdevcmd's root; the title says when
rows were left out.

**Lists are kept** for the session, the last 10, in memory only:

- **Back** (Alt+,) after Enter on a row returns to the list, on that row. Backspace
  is Parent, as anywhere.
  Alt+F7's results are kept the same way.
- **Alt+L** shows the list used last, from anywhere (from the shell: Alt+X, then Alt+L).
- **Alt+Down** (or **Alt+J**) lists them above the directories visited; also View →
  History and lists…, and View → Last list.
- Coming back checks each row again: a path deleted or moved since is named on the
  status line ("1 gone since: y.txt") and dropped.

**What a list cannot read**, since paths are found in text:

- A line that is a whole existing path is taken whole, so `a b.txt` and
  `report:2026.txt` work. Inside longer lines a name with spaces needs quotes
  (`'a b.txt'`, as GNU `ls` prints it).
- A name with a newline, or NUL-separated output (`find -print0`): use `find -print` or
  `rg -l`.
- A line holding a control character (other than a tab), or a quoted name with a tab.
- Paths that do not exist when the list is made.
- While `ssh` (or mosh, telnet) runs, the output names another machine's files: no list
  is made, and a chosen path says it is looked up on this machine.

On macOS every Alt key needs "Use Option as Meta key" (Terminal → Settings → Profiles →
Keyboard; iTerm2: Option key "Esc+"). `p`, `P` and the menu bar need nothing.

## Logging a tab to a file

A terminal tab's right-click menu has **Log to file…**. From then on, everything that
tab's shell prints also goes to a file, read before the screen sees it, so nothing is
lost when it scrolls past `[terminal] scrollback`. The tab shows **●**; its menu then
offers **Stop logging**.

- **Where**: `xdevcmd-<shell>-<date>-<time>.log`, in the shell's directory when the
  system gives it (Linux, macOS), else the file pane's. An existing file is appended to;
  a new one is readable by you only (`0600`; on Windows, its folder's permissions).
- **Refused**: a link, a FIFO or a device at that name (checked by the open itself, so
  one swapped in at the last moment is refused too), a file another user owns (Unix),
  anything outside `--root`, and read-only or restricted mode.
- **text or raw** (Tab in the prompt):
  - *text* removes colours, titles and other escapes; keeps each line as the terminal
    left it (a progress bar rewritten with CR is written once, as it ended); writes one
    line for a full-screen program instead of its redraws; puts a header and footer
    around each run.
  - *raw* is the bytes as received: `cat` or `less -R` replays it.
- **One log per tab**: it follows the shell that had focus when it started; a split
  opened later is not in it.
- **It ends** when you stop it, the shell exits, the tab closes or xdevcmd quits, after
  writing what was queued. On a disk that stopped answering (a hung network mount),
  each of these waits up to 2 seconds per log, then goes on and says the log may be
  incomplete. If xdevcmd crashes, the footer and what was queued (up to about 2 MB) are
  lost; what was written stays.
- **Never a silent gap**: the shell is never held up by the disk for more than half a
  second. If the disk cannot keep up or a write fails (full disk, removed drive), the
  log stops, the status line says why, a text log ends with a line saying so, and ●
  becomes a red **!** until you stop it.

Logging every shell, by a setting, is planned for 0.5.

## Telling xdevcmd where the shell is

What xdevcmd can learn by itself:

| System | The shell's directory | Whether a program runs |
|---|---|---|
| Linux | read from `/proc`; nothing to set up | read from `/proc` |
| macOS | asked of `/usr/sbin/lsof` when a key needs it; `follow` needs OSC 7 | `ps` |
| Windows | only what the shell reports: OSC 9;9 or OSC 7 | only the prompt marks, OSC 133 |

Everything else comes from short escape sequences the shell prints at each prompt, the
same ones Windows Terminal, iTerm2, WezTerm and VS Code read. Your prompt or framework
(starship, oh-my-zsh, oh-my-posh) may already send them; xdevcmd reads them whoever
sends them.

- **OSC 7** `ESC ] 7 ; file://HOST/PATH ESC \`: the directory, percent-encoded. HOST must
  be empty, `localhost` or this machine's name; other hosts are ignored.
- **OSC 9;9** `ESC ] 9 ; 9 ; "C:\path" ESC \`: Windows Terminal's form.
- **OSC 133** prompt marks: `A` a prompt starts, `C` a command starts, `D` it finished.
  A `C` with no `D` after it means busy, once the shell has sent a prompt mark (`A`,
  `B` or `D`); before that a `C` is ignored, so a file that holds one cannot make a
  shell look busy. A reported directory with a control character is ignored.

**Add the marks only if your shell can send `C`** when a command starts. A shell that
prints `A` but never `C` looks idle while a program runs, and on Windows xdevcmd would
then type into that program. cmd cannot send `C`: give it only OSC 9;9.

Put one of these in your shell's startup file. Only bash's has been run (bash 5.2, on
Linux); the others are written from each shell's manual and **have not been run**.

**bash** (`~/.bashrc`; `PS0` needs bash 4.4 or later). Tried by hand (no test runs it):

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

**cmd** (with `setx PROMPT`, or in the `PROMPT` variable; the directory only, no marks).
Not run:

```bat
PROMPT $e]9;9;$P$e\$P$G
```

Whether Windows' console (ConPTY) passes these through has not been checked yet. If it
does not, Alt+G says the directory is not known, and every send asks.

Any program printing into the terminal can print the same sequences:
[security.md](security.md#the-shell-panel) says what xdevcmd trusts them for.

## Tab completion and the manual page

Packages install these for you. Otherwise:

```bash
xdevcmd --print-completions bash > ~/.local/share/bash-completion/completions/xdevcmd
xdevcmd --print-completions zsh  > ~/.zfunc/_xdevcmd        # a directory on $fpath
xdevcmd --print-completions fish > ~/.config/fish/completions/xdevcmd.fish
xdevcmd --print-man | man -l -                               # read it once
```

PowerShell: `xdevcmd --print-completions powershell | Out-String | Invoke-Expression`, or
dot-source its output from your `$PROFILE`. The bash script is tested; zsh, fish and
PowerShell are generated from the same list of options but not yet run.
