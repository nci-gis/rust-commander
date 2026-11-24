# Languages

The words on screen can be in another language: menus, the key bar, dialogs, the
status line, F1. A release has **Vietnamese** and a **Japanese preview**. You can add
your own, change a few words of one, or make up a language just to see how the screen
reads. A language is one text file; nothing needs building.

What stays English: these docs, `--help` and other command-line output, config keys,
mistakes reported in `config.toml`, and some messages from the copy engine (what a job
did, why a file was skipped). A text with no translation shows in English, never blank.

## Choose a language

**File → Settings… → Languages**. One row a language:

- `auto - detect by locale - (vi)`: the system's language (the default).
- `English - en - built in`.
- One row for each language file found: `<name> - <code> - <file>`, such as
  `Tiếng Việt - vi - vi.toml`. A file that does not read is not listed (see
  [Mistakes](#mistakes)).

Space, ←/→ or a click picks a row; Enter saves. The language changes **at the next
start**.

Or in `config.toml`:

```toml
[general]
language = "vi"      # "auto" (the default), "en", or any language's code
```

`auto` reads `LC_ALL`, then `LC_MESSAGES`, then `LANG`: `vi_VN.UTF-8` is `vi`, `ja_JP`
is `ja`. `C`, `POSIX`, `en_…` or none set is English. On Windows these are usually unset,
so `auto` is English there: pick the language by name.

**Help → Config in use** shows the language in use and every one found.

## Where language files are

xdevcmd reads `*.toml` files from two `lang/` folders, the second winning:

1. `lang/` beside the program: what a release ships (`vi.toml`, `ja.toml`, and
   `template.txt`, every text in English).
2. `lang/` in xdevcmd's config folder, which is yours:

   | System | Folder |
   |---|---|
   | Linux, macOS | `~/.config/xdevcmd/lang/` |
   | Windows | `%APPDATA%\xdevcmd\lang\` |

   `$XDG_CONFIG_HOME/xdevcmd/lang/` instead, on any system, when `XDG_CONFIG_HOME` is
   set to a full path. It is where `config.toml` is by default
   ([Config](config.md#where-the-file-is)); a config file given with `--config` or
   `XDEVCMD_CONFIG` does not move it.

Files with the same code are read together, yours last, so a file of yours changes
only the texts it has. An update of xdevcmd replaces the files beside the program, never
yours.

## Make your own language

1. Copy `lang/template.txt` (beside the program) to your `lang/` folder, named for your
   language: `lang/de.toml`, `lang/vi-hue.toml`, `lang/pirate.toml`. Any name will do.
2. Change the texts in quotes; keep the keys. Delete the lines you do not translate:
   they show in English.
3. Pick it in Settings → Languages, or `language = "<name>"`, and start xdevcmd again.

A file looks like this:

```toml
# lang/vi.toml
"$__name" = "Tiếng Việt"   # optional: the name Settings shows
"$__code" = "vi"           # optional: the code `language` picks it by

[keybar]
copy = "Chép"
quit = "Thoát"

[menu]
file = "Tệp"
```

- **Code and name.** The code is the file's name (`de.toml` is `de`) unless
  `"$__code"` says another. The name shown is `"$__name"`, else the language's own name
  for a common ISO code (`de` is Deutsch), else the code. Each is cut to 32 characters
  (longer ends in `...`).
- **Keys.** `[keybar]` with `copy = …` is the same key as `"keybar.copy" = …`: write
  them either way. A key xdevcmd does not use is ignored.
- **`{n}`, `{name}` …** are filled in by xdevcmd. Keep the ones the English has, in any
  order; leaving one out is allowed. A text that names one the English does not have
  shows in English. `{{` and `}}` are a brace.
- **Only some words.** To change a few words of Vietnamese, make `lang/vi.toml` in your
  folder with just those keys: the rest comes from the shipped file.
- **Width.** Translations longer than the English may be cut where room is short (the
  key bar, menus at 80 columns). Try your language at the size you use.

## A pseudo-language

A made-up language shows at a glance what is translated and what a longer text does
to the screen. Wrap every text in marks, so anything still plain on screen is English:

```sh
# Linux, macOS: from the template, in your lang/ folder
{ echo '"$__name" = "Pseudo"'
  sed -E 's/^([^#[].* = ")(.*)"$/\1⟦\2⟧"/' /path/to/xdevcmd/lang/template.txt
} > xx.toml
```

On Windows, open `template.txt` in an editor with regular expressions (VS Code: Ctrl+H,
`.*` on), replace `^([^#\[].* = ")(.*)"$` with `$1⟦$2⟧"`, and save it as `xx.toml`
(UTF-8, no BOM). Then `language = "xx"`.

Any change works the same way: upper case, twice as long, or the words in reverse. It
is your file.

## Mistakes

- **A file that is not valid TOML** is skipped, not listed in Settings, and the status
  line says which file and why at start. The other files still load.
- **A language asked for by name and not found** (`language = "de"`, no `de.toml`):
  English, and the status line says where it looked.
- **A text that is not a string**, or a `{name}` the English does not have: that key
  shows in English.

From the source tree, `cargo xtask lang check my.toml` lists those, and keys the code no
longer has; `cargo xtask lang template` and `cargo xtask lang pseudo` print the
template and the pseudo-language for the code you have
([Development](development.md)).
