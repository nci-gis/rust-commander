# Platforms

> **Read this when**: writing platform-specific code (`#[cfg]`), something works on Linux and not on Windows or macOS, deciding colour depth, or a key that some terminal does not deliver (macOS Terminal.app above all).

---

## Platforms

Linux, Windows and macOS. The stack carries most of it: crossterm runs on all three and
`portable-pty` uses **ConPTY** on Windows, so the shell pane works there too.

What is platform-specific, and where:

| | |
|---|---|
| Trash | Linux uses `trash_into` in `ops.rs` — tested here, and it takes an injectable root so tests never touch the real trash. Windows and macOS use the `trash` crate, declared only for `cfg(not(target_os = "linux"))`, because the Recycle Bin and macOS Trash are OS APIs, not a directory you can rename into. |
| Hidden files | A leading dot on Unix; on Windows the `FILE_ATTRIBUTE_HIDDEN` attribute **or** a leading dot, so `.git` still hides on a developer's machine. |
| Executable | The mode bits on Unix, the extension on Windows. |
| Shell name | `$SHELL`, falling back to `%COMSPEC%` on Windows. |
| Editor | `$VISUAL`, `$EDITOR`, then `vi` — or `notepad` on Windows. |
| Symlinks | Copied on Unix. On Windows they are reported as an error rather than silently skipped; creating them needs a privilege we should not assume. |

**Check another platform without leaving this one:**

```bash
rustup target add x86_64-pc-windows-gnu x86_64-apple-darwin
cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy --all-targets --target x86_64-apple-darwin  -- -D warnings
```

`cargo check` and `cargo clippy` do not link, so no cross-linker or SDK is needed. This
catches a broken `#[cfg]` in seconds. It does **not** catch behaviour, which is what the
CI matrix in `.github/workflows/ci.yml` is for; it runs on every push to
`github.com/nci-gis/rust-commander`. Until it has passed there, "works on Windows" means
"compiles for Windows", and nothing stronger.

**No test may depend on a path the host happens to have.** `/etc` and `/usr` do not exist
on Windows; `tests/render.rs` builds its own fixture tree instead.

## Colour depth

**macOS Terminal.app before macOS 26 has no 24-bit colour, and does not ignore it**: it
reads each number after `38;2` / `48;2` as an attribute of its own, so the theme's
(28, 42, 58) cursor row came out as a green background (42) and (101, 119, 139) dim text
as a bright red one (101). ratatui and crossterm never check; the app has to.

`main.rs` decides the depth once at start-up: `RCMD_COLORS` (a bad value stops start-up,
as a bad flag does), then `[theme] colors`, then for `auto` (the default)
`rcmd_core::color::detect`, which reads environment variables only. 24-bit **only on
positive evidence**, in this order:

1. `TERM_PROGRAM=Apple_Terminal`: 24-bit from `TERM_PROGRAM_VERSION` 464
   (`APPLE_TERMINAL_TRUECOLOR`), else 256 — **whatever `COLORTERM` says**, because people
   export `COLORTERM=truecolor` in their dotfiles.
2. `TERM=linux`: 16, and the `ansi` theme stands in for `dark`.
3. `COLORTERM` = `truecolor` / `24bit`: 24-bit.
4. A known terminal (`TERM_PROGRAM` iTerm.app, WezTerm, ghostty, vscode; `TERM`
   containing kitty, ghostty, alacritty, wezterm or `-direct`; `KITTY_WINDOW_ID` or
   `WT_SESSION` set), or running on Windows: 24-bit.
5. Anything else: 256.

Why not "downgrade only where known broken": `COLORTERM` and `TERM_PROGRAM` do not cross
SSH, so a server reached from an old Terminal.app would still be sent 24-bit colour. The
cost is 256 colours over SSH on a capable terminal unless `COLORTERM` is forwarded or
`RCMD_COLORS` is set. No terminal query: it needs an answer, and a cursor query over a
slow link has already killed rcmd once (see gotchas). Below 24-bit, `quiet.rs` fits every
cell — theme, `LS_COLORS`, pictures, the terminal panel — in one place.

Confirmed on a Mac in Terminal.app (no more green or red). **Unconfirmed**: the 464
cut-off, from one third-party report; if a newer Terminal.app shows muted colours, it is
the suspect.

## macOS Terminal.app keys

Measured with `rcmd --show-keys`, "Use Option as Meta key" on (Settings → Profiles →
Keyboard). Without it Option types characters (`ç`, `ƒ`) and no Alt key arrives; on
macOS such a character with no binding, in a file pane, shows a hint once per session
(`keyfix.rs`).

| Pressed | Arrives as |
|---|---|
| ⌥← / ⌥→ | Alt+B / Alt+F (the profile's word left / right), so those are bound to parent / enter, on every system |
| ⌥↑ / ⌥↓ | three keys, `ESC ESC [ A` / `B`, never Alt+↑ / ↓: history is also Alt+J |
| ⌥⇧ + arrows | the same as without ⇧: resize is also Alt+R, then arrows |
| Ctrl+⇧← | a plain Left |
| ⇧← / ⇧→ | themselves |
| ⇧↑ / ⇧↓; F-keys with and without Fn | not itemised |

F-keys are media keys unless Fn is held or the system setting is on, and then rcmd sees
nothing, so that advice lives in F1 and `docs/troubleshooting.md`, not in a hint. The
mouse works with View → Allow Mouse Reporting on.

**Esc as Meta is gone.** Round 003 added Midnight Commander's Esc prefix (Esc then a key
is Alt+key, Esc then a digit an F-key, `[general] esc_as_meta`); Round 004 removed it
before it shipped, because it gave Esc two meanings, and second keys that arrive
everywhere (Alt+J, Alt+R then arrows, Alt+1…9) cover what it was for. A key a terminal
drops gets a second binding, not a prefix.

## Keys each default terminal keeps

Measured with `rcmd --show-keys` in Windows Terminal, a Linux desktop terminal (Ubuntu;
which one not noted) and Terminal.app (above):

- **Windows and Linux deliver every key rcmd binds** — Alt+arrows, Alt+Shift+arrows,
  F1–F10, F12, Shift+F1–F10, Alt+F5 / F7 / F9, Ctrl+Space, Ctrl+\ (as Ctrl+4 on Linux,
  which `normalise` maps back), Alt+digits — **except Ctrl+PageUp / PageDown**, which
  arrive as plain PageUp / PageDown on a laptop, where PageUp is Fn+↑. Hence Alt+1…9
  for tabs.
- The belief that Windows Terminal keeps Alt+arrows and Alt+Shift+arrows for its own
  panes was **wrong**: they arrive.
- **F11** is the terminal's full screen everywhere; rcmd binds nothing to it.
- **Ctrl+Tab** is taken on both (the terminal or desktop on Linux; on Windows it switches
  away), so it is not a tab key.
- **macOS Terminal.app**: only history (Alt+↓) and resize (Alt+Shift+arrows) cannot be
  typed.

This is why there is one key set for every system, not one per system: three sets would
have been two copies of the same one. Check a new default key against these lists;
terminals beyond the default ones (iTerm2, kitty, tmux) keep fewer.

## Toolchain

A user-level `rustup` is all it needs: nothing here requires root, and building must
never install anything system-wide. Facts about a particular development machine belong
in that machine's own notes, not in this repository.
