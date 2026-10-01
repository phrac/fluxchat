# Themes

Choose a theme in Settings → Appearance, or from the palette ("Theme: …").

## Built-in themes

FluxChat (default), Nord, Nord Light, Dracula, Night Owl, Tokyo Night, Atom One Dark, Catppuccin Mocha, Catppuccin Latte.

## Your own themes

Themes are TOML files in your data folder:

| OS | Folder |
|----|--------|
| macOS | `~/Library/Application Support/fluxchat/themes/` |
| Linux | `~/.local/share/fluxchat/themes/` |
| Windows | `%APPDATA%\fluxchat\themes\` |

The built-in themes are copied there on first run, so you can edit them, delete them, or add new files. Restart fluxchat to pick up changes.

### Format

Colours are `[r, g, b]` or `[r, g, b, alpha]` (0–255). Any colour you leave out uses the default theme's colour.

```toml
name = "My Theme"
dark = true            # false for light themes (affects default widget styling)

# Backgrounds, darkest to lightest
bg_deep = [33, 38, 46]     # window, sidebars
bg_dark = [46, 52, 64]     # panels, dialogs, message box
bg_mid = [59, 66, 82]      # timeline, cards
bg_light = [67, 76, 94]    # selected rows, buttons
bg_hover = [76, 86, 106]   # hovered buttons

# Text, brightest to dimmest
text_bright = [236, 239, 244]
text_normal = [216, 222, 233]
text_muted = [123, 136, 157]
text_dim = [86, 98, 118]

# Accents
accent = [136, 192, 208]   # links, focus, selection
green = [163, 190, 140]    # success, online, verified
yellow = [235, 203, 139]   # warnings, away, admins
red = [191, 97, 106]       # errors, mentions, destructive actions
purple = [180, 142, 173]

border = [255, 255, 255, 20]

# Code blocks
code_bg = [0, 0, 0, 76]
code_text = [216, 222, 233]
```

Hover and selection highlights are derived from your text colour, so they work on both light and dark themes. Name colours are picked automatically from a fixed palette.
