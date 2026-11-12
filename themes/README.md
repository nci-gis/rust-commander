# Example themes

Theme files for xdevcmd. The built-in themes (xdevtheme, the default, and dark, light,
ansi, blue, high-contrast) need no file; these are extra, and a start for your own.

- Two more looks of xdevcmd's own, beside the built-in xdevtheme: **xdevtheme-neon**,
  the loud one (teal, violet and pink, the focused frame glowing), and
  **xdevtheme-glass**, the different one (no lines: panes are cards, pills). Their
  `[style]` table is in [docs/config.md](../docs/config.md#colours).
- **xdevtheme-light**: xdevtheme for a terminal with a light background (macOS
  Terminal's default profile, for one). The same style, with colours dark enough that
  every name reads at 4.5:1 or more on white, also at 256 colours. On a 16-colour
  terminal the built-in `ansi` theme, which follows your terminal's own palette, reads
  better.
- The others are well-known palettes: Dracula, Tokyo Night, Catppuccin Mocha, Nord,
  Gruvbox dark and Solarized light.

To use one, put it in a `themes` folder in either place:

- beside your `config.toml` (`~/.config/xdevcmd/themes/` on Linux and macOS,
  `%APPDATA%\xdevcmd\themes\` on Windows), or
- beside the `xdevcmd` program, for a copy you carry around. The release archives
  already put this folder there.

Then pick it in **View > Theme…**, or set `[theme] name = "nord"` in `config.toml`.

To make your own, print a theme as a file (a built-in one, or one of these) and change
what you like:

```bash
xdevcmd --print-theme xdevtheme > ~/.config/xdevcmd/themes/mine.toml
```

Every name a theme file takes is in [docs/config.md](../docs/config.md#colours).
