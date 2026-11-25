# What stays the same from 0.4.0

What you write down or build on keeps working across 0.4.x, and what is not promised.

**In short**

- Your `config.toml`, your themes, your `[keys]` bindings and scripts that start
  xdevcmd with flags keep working from 0.4.0 to every 0.4.x.
- Default keys, the look, message wording and diagnostic output may change.
- The 0.4.0 betas and release candidates are not covered yet.

## The promise

Within 0.4.x (0.4.0, 0.4.1, …), none of the following changes in a way that breaks what
worked before. A later minor version (0.5) may change one; if it does, the CHANGELOG
says so under **Changed**, with what to do.

| What | Covered | Where it is listed |
|---|---|---|
| **Config keys** | Every key in `config.toml`: its name, its section, what its values mean. A new key may be added; its default keeps today's behaviour. | `xdevcmd --print-config`, [config.md](config.md) |
| **Command-line flags** | `[LEFT] [RIGHT]`, `--no-restore`, `--read-only`, `--restricted`, `--root`, `--cwd-file`, `--config`, `--check-config` (and its exit code), `--print-config`, `--print-theme`, `--print-keys`, `--print-completions`, `--print-man`, `--version`, `--help` | `xdevcmd --help`, the man page |
| **Action names in `[keys]`** | The names you bind keys to (`quit`, `help`, `copy`, …), and the old names kept as aliases (`grow_pane`, …). A key's **default** binding is not covered (0.4.0-beta.2 took `q` off Quit). | `xdevcmd --print-config`, section `[keys]` |
| **Theme files** | The format of a theme file (`[theme]` colours and `[style]`), and the names of the built-in themes. | `xdevcmd --print-theme NAME`, [config.md](config.md) |
| **Journals** | A newer 0.4.x tidies up after, and offers to continue, a job that an older 0.4.x was killed in (journal `v 3`). In a journal, a line it does not know is skipped; the lines it knows are acted on. | [security.md](security.md), "Safe operations" |
| **Environment variables** | `XDEVCMD_CONFIG`, `XDEVCMD_STATE`, `XDEVCMD_DEBUG`, `XDEVCMD_DEBUG_DIR`: their names and what they point at. | `xdevcmd --help`, the man page, [config.md](config.md) |

## Not promised

- **What things look like**: the layout, colours of the built-in themes, the wording of
  messages and dialogs, the names shown in F1 (Commands) and the menus.
- **Default key bindings**: a default can change in any version (the CHANGELOG says
  so); a key you bound yourself in `[keys]` keeps working.
- **Diagnostic output**: `--timings`, `--show-keys`, `--screenshot`, the debug log's
  events and fields, crash logs. They are for finding problems and change with the code.
- **The state files' contents**: the session, history and bookmarks files are read by
  the version that wrote them and newer; their format is not an interface. An older
  version may not read what a newer one wrote.
- **`xdevcmd-core`'s API**: the engine crate is published so `xdevcmd` can be built from
  crates.io; it has no stable API of its own and its version always equals the app's.
- **Performance numbers**: the binary's size (at most 7 MiB) is checked in CI and on
  each release archive; start-up and listing 100,000 entries are checked by hand before
  a release. A number measured on one machine is not a promise for another.

## Before 0.4.0

The 0.4.0 betas and release candidates are steps towards this promise, not covered by
it: what changes between them is in the CHANGELOG.
