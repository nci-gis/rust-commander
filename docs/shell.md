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
- **Alt+C** types `cd <this directory>` into it, quoted for your shell. It refuses if a
  program is running there (an editor, a password prompt) or if the name has control
  characters in it.
- **F12** moves between the panes and the terminal.
- **Inside the terminal, xdevcmd takes only F12, Shift+F12 and Alt+X**; every other key
  is the program's. Alt+X, then a terminal key (Alt+S split, Alt+W close, Alt+N new tab,
  Alt+1…9 tab N) acts on the shell; anything else after Alt+X goes to the program.

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
