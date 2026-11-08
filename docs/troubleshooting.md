# Troubleshooting

Known problems, by system, and what to do about them. Most of them are the terminal,
not xdevcmd: a key the terminal never sends, a colour it cannot show.

**First, ask xdevcmd what it receives:**

```bash
xdevcmd --show-keys
```

It prints your terminal's variables and how many colours xdevcmd will use, then one line
for every key, click and scroll it gets, and what that key would do. Esc twice quits.
If a key prints nothing, your terminal is not sending it. Please paste this output into
any bug report.

## Every system

**A key does nothing.** Run `xdevcmd --show-keys` and press it. If nothing prints, the
terminal or the desktop keeps that key; if something else prints, xdevcmd gets a different
key. Either way, give the action another key under `[keys]` (see
[config.md](config.md#keys)). Measured so far, on every system:

- **Ctrl+PageUp / PageDown** (previous / next tab) lose Ctrl on a laptop, where PageUp
  is Fn+↑. Use **Alt+1**…**Alt+9** for tab N.
- **Ctrl+Tab** belongs to the terminal or the desktop.
- **F11** is the terminal's full screen. xdevcmd does not use it.

**Enter views a file instead of opening it.** Over SSH or without a desktop there is no
screen here to open it on, so xdevcmd views it and says why at the bottom of the viewer.
An `[[open]]` rule with `terminal = true` opens programs that work in a terminal, and
`[commands] opener` is used for everything else (see
[config.md](config.md#opening-files)). In read-only mode nothing is opened in another
program.

**`o` says "over SSH there is no screen here to open it on".** Set `[commands] opener`,
or use F3.

**Colours look muted over SSH.** xdevcmd uses 24-bit colour only when it knows the
terminal takes it, usually from `COLORTERM`, which SSH does not pass on. On the server:

```bash
export XDEVCMD_COLORS=truecolor   # or 256, or 16
```

`[theme] colors` in the config does the same for every machine that reads it.

**File icons show as boxes.** `[files] icons = true` needs a
[Nerd Font](https://www.nerdfonts.com/) set as your terminal's font.

## macOS

**Option+key does nothing** (Option types `ç`, `ƒ`, … instead). Terminal.app:
Settings → Profiles → your profile → Keyboard → **Use Option as Meta key**. iTerm2:
Profiles → Keys → Left Option key → **Esc+**. xdevcmd says so the first time Option types
a character in a file pane.

**Some Option keys still do nothing with Option as Meta on.** Terminal.app sends
Option+← / → as word left / right, Option+↑ / ↓ as three separate keys, and drops Shift
or Ctrl from Option+Shift and Ctrl+Shift arrows. xdevcmd has a key for each that does
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

**Shift+F1, Shift+F2, Shift+F4 do nothing in Terminal.app.** Terminal.app sends nothing
for Shift+F1–F4 (and sends Shift+F5–F12 as F13–F20, which xdevcmd understands as
meant). To give them back: Terminal → Settings → Profiles → Keyboard, **+**, pick the
key with Shift, Action "Send Text", and type the sequence below (press Esc for the
`\033`). iTerm2, Ghostty and kitty send them as they are.

| Key | Send Text | In xdevcmd |
|---|---|---|
| Shift+F1 | `\033[1;2P` | what xdevcmd is using |
| Shift+F2 | `\033[1;2Q` | rename many |
| Shift+F3 | `\033[1;2R` | — |
| Shift+F4 | `\033[1;2S` | new file |

**Green or red backgrounds, hard-to-read text** in Terminal.app before macOS 26.
Terminal.app there cannot show 24-bit colour and misreads it, so xdevcmd sends it 256
colours. If you see this, `[theme] colors` or `XDEVCMD_COLORS` is probably set to
`truecolor`: set it back to `auto`, or to `256`.

**The mouse does nothing.** Check View → **Allow Mouse Reporting** (⌘R) is ticked; with
it, clicks, the wheel, drags and double-clicks all reach xdevcmd. The pointer stays the
text cursor (I-beam) inside xdevcmd: that is Terminal.app's pointer everywhere, and says
nothing about whether the mouse works.

**Dragging the window's scrollbar up shows the shell, with empty space, above xdevcmd.**
xdevcmd draws on the terminal's second screen, and Terminal.app keeps the first one, with
your shell's history, reachable above it. xdevcmd cannot hide it, and nothing is wrong:
drag back down, or use the wheel over xdevcmd, which scrolls xdevcmd. System Settings →
Appearance → Show scroll bars → **When scrolling** hides the bar.

**The terminal panel has a different background from the panes.** The panel shows
what the shell prints, and where the shell sets no colour, your terminal profile's own
background shows through, as it would outside xdevcmd.

**"xdevcmd cannot be opened because the developer cannot be verified."** The downloads
are not signed. After unpacking, in the `xdevcmd-…-apple-darwin` folder:

```bash
xattr -d com.apple.quarantine xdevcmd
```

A file from a CI run's Artifacts also needs `chmod +x xdevcmd`: the zip drops the
permission.

## Windows

**"Windows protected your PC" (SmartScreen).** The downloads are not signed. Choose
**More info** → **Run anyway**, once.

**A config saved by PowerShell (`xdevcmd --print-config > config.toml`) is UTF-16.** xdevcmd
reads it; other tools may not. `xdevcmd --print-config | Out-File -Encoding utf8
config.toml` writes UTF-8.

## Linux

**`o` says "no desktop session to open it in (no DISPLAY)".** Without `DISPLAY` or
`WAYLAND_DISPLAY` there is no desktop to open the file in. Set `[commands] opener` (for
example a terminal viewer), or use F3.

**The Linux console (`TERM=linux`) shows only 16 colours.** xdevcmd switches to a
16-colour theme there by itself.

## Still stuck

[Open an issue](https://github.com/nci-gis/rust-commander/issues) with your system,
terminal and the `xdevcmd --show-keys` output.

## xdevcmd crashed

It writes what happened to a `crash-….log` file in its state directory
(`~/.local/state/xdevcmd/` on Linux and macOS, `%LOCALAPPDATA%\xdevcmd\` on Windows) and
says where, after the terminal is restored; it keeps the five newest. Please
[open an issue](https://github.com/nci-gis/rust-commander/issues) and attach it — read it
first: it may hold file names. Nothing is ever sent by xdevcmd itself.
