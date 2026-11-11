# Example themes

Theme files for xdevcmd. The five built-in themes (dark, light, ansi, blue,
high-contrast) need no file; these are extra, and a start for your own.

- **xdevtheme** is xdevcmd's own look, and **xdevtheme-dawn** its light pair. Their
  `[style]` table draws rounded frames, a teal-to-violet light, slanted F keys and a
  lit cursor row (see [docs/config.md](../docs/config.md#colours)).
- Three more in the same family: **xdevtheme-neon** (teal, violet and pink, the focused
  frame glowing), **xdevtheme-glass** (no lines, panes as cards, pills) and
  **xdevtheme-borealis** (an aurora down the screen, stars in the empty rows).
- The others are well-known palettes: Dracula, Tokyo Night, Catppuccin Mocha, Nord,
  Gruvbox dark and Solarized light.

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
