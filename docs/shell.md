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
