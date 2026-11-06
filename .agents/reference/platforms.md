# Platforms

> **Read this when**: writing platform-specific code (`#[cfg]`), or something works on Linux and not on Windows or macOS.

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

## Toolchain

A user-level `rustup` is all it needs: nothing here requires root, and building must
never install anything system-wide. Facts about a particular development machine belong
in that machine's own notes, not in this repository.
