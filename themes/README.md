# Example themes

Theme files for xdevcmd. The five built-in themes (dark, light, ansi, blue,
high-contrast) need no file; these are extra, and a start for your own.

To use one, put it in a `themes` folder in either place:

- beside your `config.toml` (`~/.config/xdevcmd/themes/` on Linux and macOS,
  `%APPDATA%\xdevcmd\themes\` on Windows), or
- beside the `xdevcmd` program, for a copy you carry around. The release archives
  already put this folder there.

Then pick it in **View > Theme…**, or set `[theme] name = "nord"` in `config.toml`.

To make your own, print a built-in theme as a file and change what you like:

```bash
xdevcmd --print-theme dark > ~/.config/xdevcmd/themes/mine.toml
```

Every name a theme file takes is in [docs/config.md](../docs/config.md#colours).
