# Languages

The words on screen (menus, key bar, dialogs, status line, F1) can be in another
language. A release has Vietnamese and Japanese; you can add your own. A
language is one text file.

Stays English: these docs, `--help`, config keys and their errors, and some messages of
the copy engine. A text with no translation shows in English.

## Choose a language

1. **File → Settings… → Languages**.
2. Pick a row (Space, ←/→ or a click), then Enter:
   - `auto - detect by locale - (vi)`: the system's (`LC_ALL`, `LC_MESSAGES`, `LANG`).
     Windows rarely sets these, so pick by name there.
   - `English - en - built in`.
   - `<name> - <code> - <file>` for each language file found.
3. Start xdevcmd again.

Or in `config.toml`:

```toml
[general]
language = "vi"   # "auto" (default), "en", or a language's code
```

Help → Config in use shows the language in use and those found.

## Where the files are

Read in this order, the later winning:

1. `lang/` beside the program: what a release ships (`vi.toml`, `ja.toml`,
   `template.txt`). Replaced by each update.
2. `lang/` in your config folder: yours, never touched.
   - Linux, macOS: `~/.config/xdevcmd/lang/`
   - Windows: `%APPDATA%\xdevcmd\lang\`
   - Any system, `XDG_CONFIG_HOME` set: `$XDG_CONFIG_HOME/xdevcmd/lang/`

## Make your own

1. Copy `lang/template.txt` (beside the program) to your `lang/` folder as
   `<code>.toml`: `de.toml`, `vi-hue.toml`, `pirate.toml`, any name.
2. Translate the texts in quotes. Keep the keys and every `{name}` in braces. Delete
   lines you skip: they show in English.
3. Choose it (above).

```toml
# lang/vi.toml
"$__name" = "Tiếng Việt"   # optional: the name Settings shows
"$__code" = "vi"           # optional: the code to choose it by (else the file name)

[keybar]
copy = "Chép"   # the same key as "keybar.copy" = "Chép"
```

To change a few words of a shipped language, make a file with its code in your folder,
holding only those keys.

## A pseudo-language

Every text wrapped in marks: what still shows plain is English, and longer texts show
where the screen cuts.

1. In your `lang/` folder, make `xx.toml` from the template:
   - Linux, macOS:

     ```sh
     { echo '"$__name" = "Pseudo"'
       sed -E 's/^([^#[].* = ")(.*)"$/\1⟦\2⟧"/' /path/to/xdevcmd/lang/template.txt
     } > xx.toml
     ```

   - Windows: open `template.txt` in an editor with regular expressions (VS Code:
     Ctrl+H, `.*` on), replace `^([^#\[].* = ")(.*)"$` with `$1⟦$2⟧"`, save as
     `xx.toml` (UTF-8).
2. Choose `xx`.

## Rules

- `$__code` and `$__name`: at most 32 characters (longer ends in `...`).
- `{n}`, `{name}` …: filled in by xdevcmd. Any order; leaving one out is fine; one the
  English does not have makes that text English. `{{` `}}` is a brace.
- **Escapes**: TOML's (`\n`, `\u001b` …) work, but a text holding a control character
  (ESC, BEL …) or a direction override (U+202E …) that its English does not hold is
  not used: it shows in English. A language file is text from anyone, and what it says
  is drawn on your terminal. The same characters are left out of `$__name`.
- A file that is not valid TOML is skipped and named on the status line at start.
- A language chosen by name and not found: English, and the status line says where it
  looked.
- From the source tree: `cargo xtask lang check my.toml` lists all of the above;
  `cargo xtask lang template` and `pseudo` print for the code you have
  ([Development](development.md)).
