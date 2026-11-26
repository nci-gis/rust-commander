# Troubleshooting

Find your problem in the table, then the fix below it. Most problems are the terminal,
not xdevcmd: a key it never sends, a colour it cannot show.

| Problem | System | Fix |
|---|---|---|
| A key does nothing | all | [Every system](#every-system): `--show-keys`, then rebind |
| Enter views a file instead of opening it; `o` says there is no screen | all | [Every system](#every-system) |
| Colours look muted over SSH | all | `export XDEVCMD_COLORS=truecolor` |
| File icons are boxes | all | a [Nerd Font](https://www.nerdfonts.com/) as the terminal's font |
| Option+key types `ç`, `ƒ` … | macOS | [macOS](#macos): Option as Meta |
| F1–F12 change brightness or volume | macOS | Fn, or standard function keys |
| Shift+F keys do nothing | macOS Terminal.app | Alt+X, then the F key |
| Green or red backgrounds | macOS before 26 | `XDEVCMD_COLORS=256` |
| The mouse does nothing | macOS | View → Allow Mouse Reporting (⌘R) |
| "cannot be opened … developer cannot be verified" | macOS | [macOS](#macos): only for a download from this repository |
| "Windows protected your PC" | Windows | [Windows](#windows): only for a download from this repository |
| `o` says "no DISPLAY" | Linux | set `[commands] opener`, or F3 |
| After xdevcmd was killed, odd characters appear when the mouse moves | Linux, macOS | `reset`, Enter ([Every system](#every-system)) |
| A copy fails or is slow, or it crashed | all | [Reporting a problem](#reporting-a-problem) |

**First, ask xdevcmd what it receives:** `xdevcmd --show-keys`. It prints your
terminal's variables, how many colours it will use, then a line for every key, click and
scroll, and what it would do (Esc twice quits). A key that prints nothing never reaches
xdevcmd. Paste this output into any bug report.

## Every system

**A key does nothing.** Press it in `xdevcmd --show-keys`. Nothing printed: the terminal
or the desktop keeps it. Something else printed: xdevcmd gets another key. Either way,
give the action another key under `[keys]` ([config.md](config.md#keys)). Known:

- **Ctrl+PageUp / PageDown** (previous / next tab) lose Ctrl on a laptop, where PageUp
  is Fn+↑: use **Alt+1**…**Alt+9**.
- **Ctrl+Tab** belongs to the terminal or the desktop.
- **F11** is the terminal's full screen; xdevcmd does not use it.

**Enter views a file instead of opening it.** Over SSH or without a desktop there is no
screen to open it on, so xdevcmd views it and says why. An `[[open]]` rule with
`terminal = true` opens programs that run in a terminal; `[commands] opener` is used for
the rest ([config.md](config.md#opening-files)). Read-only mode opens nothing in another
program.

**After xdevcmd was killed, odd characters appear when the mouse moves** (`26M35`,
`command not found`). A program killed with `kill -9`, or that crashed, cannot put the
terminal back: it is still sending mouse moves. Type `reset` and Enter, even if you do
not see what you type; or start xdevcmd again in that window and quit it with F10.

**Colours look muted over SSH.** xdevcmd uses 24-bit colour only when it knows the
terminal takes it, usually from `COLORTERM`, which SSH does not pass on. On the server:

```bash
export XDEVCMD_COLORS=truecolor   # or 256, or 16
```

`[theme] colors` in the config does the same on every machine that reads it.

## macOS

**Option+key types a character.** Terminal.app: Settings → Profiles → your profile →
Keyboard → **Use Option as Meta key**. iTerm2: Profiles → Keys → Left Option key →
**Esc+**. xdevcmd says so the first time it happens.

**Some Option keys still do nothing** with Option as Meta on: Terminal.app changes or
drops them. Use these instead:

| To | Instead of | Press |
|---|---|---|
| Go up / into a directory | Alt+← / → | Option+← / → (arrive as Alt+B / Alt+F) |
| Directory history | Alt+↓ | **Alt+J** |
| Resize panes | Alt+Shift+arrows | **Alt+R**, then arrows; Enter when done |
| Previous / next tab | Ctrl+PageUp / PageDown | **Alt+1**…**Alt+9** |

**F1–F12 change the brightness or volume.** Hold **Fn**, or System Settings → Keyboard →
Keyboard Shortcuts → Function Keys → **Use F1, F2, etc. keys as standard function keys**.

**Shift+F keys do nothing in Terminal.app.** It sends Shift+F5–F12 as F13–F20 and
nothing for Shift+F1–F4. Press **Alt+X, then the F key**: Alt+X F8 is Shift+F8, Alt+X F10
the right-click menu (but Alt+X F1 opens F1, Commands). Shift+F12 (the shell's history)
works as it is. iTerm2, Ghostty and kitty send Shift+F keys correctly. To make
Terminal.app send them: Settings → Profiles → Keyboard, **+**, the key with Shift,
"Send Text": `\033[1;2P` `\033[1;2Q` `\033[1;2R` `\033[1;2S` for Shift+F1–F4.

**Green or red backgrounds, hard-to-read text** (Terminal.app before macOS 26). It
cannot show 24-bit colour, so xdevcmd sends 256 colours; if you see this, `[theme]
colors` or `XDEVCMD_COLORS` is set to `truecolor`: set it to `auto` or `256`.

**The mouse does nothing.** View → **Allow Mouse Reporting** (⌘R). The pointer stays an
I-beam inside xdevcmd: that is Terminal.app's pointer, not a sign the mouse is off.

**The window's scrollbar shows the shell above xdevcmd.** Terminal.app keeps your
shell's history above xdevcmd's screen; nothing is wrong. Drag back down, or use the
wheel over xdevcmd. System Settings → Appearance → Show scroll bars → **When scrolling**
hides the bar.

**The terminal panel has another background than the panes.** Where the shell sets no
colour, your terminal profile's background shows through, as outside xdevcmd.

**"xdevcmd cannot be opened because the developer cannot be verified."** The downloads
are not signed. Do this only for an archive from this repository's Releases, after
[checking it](security.md#checking-a-download). In the unpacked
`xdevcmd-…-apple-darwin` folder, remove the download's quarantine mark:

```bash
xattr -d com.apple.quarantine xdevcmd
```

A file from a CI run's Artifacts also needs `chmod +x xdevcmd`: the zip drops the
permission.

## Windows

**"Windows protected your PC" (SmartScreen).** The downloads are not signed. Only for
an archive from this repository's Releases, after
[checking it](security.md#checking-a-download): **More info** → **Run anyway**, once.

**A config written by PowerShell's `>` is UTF-16.** xdevcmd reads it; other tools may
not. `xdevcmd --print-config | Out-File -Encoding utf8 config.toml` writes UTF-8.

## Linux

**`o` says "no desktop session to open it in (no DISPLAY)".** There is no desktop to
open the file in: set `[commands] opener` (a terminal viewer, say), or use F3.

**The Linux console (`TERM=linux`)** shows 16 colours, and xdevcmd uses 16 by itself: the
default theme gives way to a 16-colour one; a theme you chose is fitted to the 16.

## Reporting a problem

[Open an issue](https://github.com/nci-gis/rust-commander/issues) with your system, your
terminal and the `xdevcmd --show-keys` output. For a copy, move or delete that is slow or
fails, attach a debug log; after a crash, the crash log.

### Sending a log with a report

Start xdevcmd with the debug log on, do the same thing again, quit, attach the log:

```bash
XDEVCMD_DEBUG=all xdevcmd          # macOS, Linux
```

```bat
set XDEVCMD_DEBUG=all
xdevcmd
```

At start the status line says `debug log on (jobs, errors): <path>` (Messages, Alt+M,
keeps it).

- **Where**: `debug-<time>-<process>.jsonl` in the state directory
  (`~/.local/state/xdevcmd/` on Linux and macOS, `%LOCALAPPDATA%\xdevcmd\` on Windows;
  beside the `XDEVCMD_STATE` file when that is set), or in `XDEVCMD_DEBUG_DIR`. Each stops
  at 16 MiB; the five newest are kept.
- **What it holds**: per job, its kind and size, the kind of drive at each end (`ntfs`,
  `apfs`, `network` …), how long each part took (checking, opening, creating, writing,
  syncing, finishing), how the bytes were copied, the batches small replaced files
  were synced in (new files' syncs count in "syncing"), and each error's system code,
  retry and your answer.
- **What it never holds**: file or folder names, user or host names. A file is a number,
  the same within one run. You can attach it as it is.
- **Topics**: `jobs` (times), `errors` (errors and questions), `all` or `1` (both).
- **For a week**: `setx XDEVCMD_DEBUG all` on Windows, or the `export` in your shell's
  profile. There is no config setting, on purpose: a shared config would turn it on
  everywhere.

Written on your machine only; nothing is sent anywhere. (It came in 0.4.0-beta.2.)

### xdevcmd crashed

It writes a `crash-….log` in the state directory (above) and says where once the terminal
is back; the five newest are kept. Attach it to an issue, after reading it: it
may hold file names. With the debug log on, it ends with the debug log's last lines.
Nothing is ever sent by xdevcmd itself.
