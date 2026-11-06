# Troubleshooting

Known problems, by system, and what to do about them. Most of them are the terminal,
not rcmd: a key the terminal never sends, a colour it cannot show.

**First, ask rcmd what it receives:**

```bash
rcmd --show-keys
```

It prints your terminal's variables and how many colours rcmd will use, then one line
for every key, click and scroll it gets, and what that key would do. Esc twice quits.
If a key prints nothing, your terminal is not sending it. Please paste this output into
any bug report.

## Every system

**A key does nothing.** Run `rcmd --show-keys` and press it. If nothing prints, the
terminal or the desktop keeps that key; if something else prints, rcmd gets a different
key. Either way, give the action another key under `[keys]` (see
[config.md](config.md#keys)). Measured so far, on every system:

- **Ctrl+PageUp / PageDown** (previous / next tab) lose Ctrl on a laptop, where PageUp
  is Fn+↑. Use **Alt+1**…**Alt+9** for tab N.
- **Ctrl+Tab** belongs to the terminal or the desktop.
- **F11** is the terminal's full screen. rcmd does not use it.

**Colours look muted over SSH.** rcmd uses 24-bit colour only when it knows the
terminal takes it, usually from `COLORTERM`, which SSH does not pass on. On the server:

```bash
export RCMD_COLORS=truecolor   # or 256, or 16
```

`[theme] colors` in the config does the same for every machine that reads it.

**File icons show as boxes.** `[files] icons = true` needs a
[Nerd Font](https://www.nerdfonts.com/) set as your terminal's font.

## macOS

**Option+key does nothing** (Option types `ç`, `ƒ`, … instead). Terminal.app:
Settings → Profiles → your profile → Keyboard → **Use Option as Meta key**. iTerm2:
Profiles → Keys → Left Option key → **Esc+**. rcmd says so the first time Option types
a character in a file pane.

**Some Option keys still do nothing with Option as Meta on.** Terminal.app sends
Option+← / → as word left / right, Option+↑ / ↓ as three separate keys, and drops Shift
or Ctrl from Option+Shift and Ctrl+Shift arrows. rcmd has a key for each that does
arrive:

| To | Instead of | Press |
|---|---|---|
| Go up / into a directory | Alt+← / → | Option+← / → (they arrive as Alt+B / Alt+F) |
| Directory history | Alt+↓ | **Alt+J** |
| Resize panes | Alt+Shift+arrows | **Alt+R**, then arrows; Enter when done |
| Previous / next tab | Ctrl+PageUp / PageDown | **Alt+1**…**Alt+9** |

**F1–F12 change the brightness or volume.** Hold **Fn** with them, or System Settings
→ Keyboard → Keyboard Shortcuts → Function Keys → **Use F1, F2, etc. keys as standard
function keys**.

**Green or red backgrounds, hard-to-read text** (rcmd 0.1.0 in Terminal.app before
macOS 26). Terminal.app there cannot show 24-bit colour and misreads it. Fixed after
0.1.0: rcmd now sends it 256 colours, so upgrading is the fix. With 0.1.0 itself,
setting every `[theme]` colour to a palette number (`"0"`–`"255"`) avoids it.

**The mouse does nothing.** Check View → **Allow Mouse Reporting** (⌘R) is ticked; with
it, clicks, the wheel, drags and double-clicks all reach rcmd. The pointer stays the
text cursor (I-beam) inside rcmd: that is Terminal.app's pointer everywhere, and says
nothing about whether the mouse works.

**Dragging the window's scrollbar up shows the shell, with empty space, above rcmd.**
rcmd draws on the terminal's second screen, and Terminal.app keeps the first one, with
your shell's history, reachable above it. rcmd cannot hide it, and nothing is wrong:
drag back down, or use the wheel over rcmd, which scrolls rcmd. System Settings →
Appearance → Show scroll bars → **When scrolling** hides the bar.

**The terminal panel has a different background from the panes.** The panel shows
what the shell prints, and where the shell sets no colour, your terminal profile's own
background shows through, as it would outside rcmd.

**"rcmd cannot be opened because the developer cannot be verified."** The downloads
are not signed. After unpacking:

```bash
xattr -d com.apple.quarantine rcmd
```

A file from a CI run's Artifacts also needs `chmod +x rcmd`: the zip drops the
permission.

## Windows

**"Windows protected your PC" (SmartScreen).** The downloads are not signed. Choose
**More info** → **Run anyway**, once.

**A config saved by PowerShell (`rcmd --print-config > config.toml`) is UTF-16.** rcmd
reads it; other tools may not. `rcmd --print-config | Out-File -Encoding utf8
config.toml` writes UTF-8.

## Linux

**`o` says "no desktop session to open it in".** Over SSH there is no `DISPLAY`, so
there is no program to open the file with. Set `[commands] opener` (for example a
terminal viewer), or use F3.

**The Linux console (`TERM=linux`) shows only 16 colours.** rcmd switches to a
16-colour theme there by itself.

## Still stuck

[Open an issue](https://github.com/nci-gis/rust-commander/issues) with your system,
terminal and the `rcmd --show-keys` output.
