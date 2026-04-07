use iced::widget::{button, container};
use iced::{Border, Color, Theme};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::sync::RwLock;

/// Serializable color as [r, g, b] or [r, g, b, a] in 0-255.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ColorDef {
    Rgb([u8; 3]),
    Rgba([u8; 4]),
}

impl ColorDef {
    fn to_color(&self) -> Color {
        match self {
            ColorDef::Rgb([r, g, b]) => Color::from_rgb(
                *r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0,
            ),
            ColorDef::Rgba([r, g, b, a]) => Color::from_rgba(
                *r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0, *a as f32 / 255.0,
            ),
        }
    }
}

/// Theme definition loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThemeDef {
    name: String,
    #[serde(default = "default_true")]
    dark: bool,
    #[serde(default)]
    bg_deep: Option<ColorDef>,
    #[serde(default)]
    bg_dark: Option<ColorDef>,
    #[serde(default)]
    bg_mid: Option<ColorDef>,
    #[serde(default)]
    bg_light: Option<ColorDef>,
    #[serde(default)]
    bg_hover: Option<ColorDef>,
    #[serde(default)]
    text_bright: Option<ColorDef>,
    #[serde(default)]
    text_normal: Option<ColorDef>,
    #[serde(default)]
    text_muted: Option<ColorDef>,
    #[serde(default)]
    text_dim: Option<ColorDef>,
    #[serde(default)]
    accent: Option<ColorDef>,
    #[serde(default)]
    green: Option<ColorDef>,
    #[serde(default)]
    yellow: Option<ColorDef>,
    #[serde(default)]
    red: Option<ColorDef>,
    #[serde(default)]
    purple: Option<ColorDef>,
    #[serde(default)]
    border: Option<ColorDef>,
    #[serde(default)]
    code_bg: Option<ColorDef>,
    #[serde(default)]
    code_text: Option<ColorDef>,
}

/// Runtime theme colors.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub name: String,
    pub dark: bool,
    pub bg_deep: Color,
    pub bg_dark: Color,
    pub bg_mid: Color,
    pub bg_light: Color,
    pub bg_hover: Color,
    pub text_bright: Color,
    pub text_normal: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub purple: Color,
    pub border: Color,
    pub code_bg: Color,
    pub code_text: Color,
}

fn default_true() -> bool { true }

impl Default for ThemeColors {
    fn default() -> Self {
        fluxchat_theme()
    }
}

/// Built-in FluxChat theme (default).
pub fn fluxchat_theme() -> ThemeColors {
    ThemeColors {
        name: "FluxChat".into(),
        dark: true,
        bg_deep: Color::from_rgb(0x1a as f32 / 255.0, 0x1b as f32 / 255.0, 0x1e as f32 / 255.0),
        bg_dark: Color::from_rgb(0x22 as f32 / 255.0, 0x23 as f32 / 255.0, 0x27 as f32 / 255.0),
        bg_mid: Color::from_rgb(0x2a as f32 / 255.0, 0x2c as f32 / 255.0, 0x31 as f32 / 255.0),
        bg_light: Color::from_rgb(0x32 as f32 / 255.0, 0x35 as f32 / 255.0, 0x3b as f32 / 255.0),
        bg_hover: Color::from_rgb(0x3a as f32 / 255.0, 0x3d as f32 / 255.0, 0x44 as f32 / 255.0),
        text_bright: Color::from_rgb(0xe4 as f32 / 255.0, 0xe4 as f32 / 255.0, 0xe7 as f32 / 255.0),
        text_normal: Color::from_rgb(0xc8 as f32 / 255.0, 0xc9 as f32 / 255.0, 0xcd as f32 / 255.0),
        text_muted: Color::from_rgb(0x8b as f32 / 255.0, 0x8d as f32 / 255.0, 0x93 as f32 / 255.0),
        text_dim: Color::from_rgb(0x5c as f32 / 255.0, 0x5e as f32 / 255.0, 0x66 as f32 / 255.0),
        accent: Color::from_rgb(0x3b as f32 / 255.0, 0x82 as f32 / 255.0, 0xf6 as f32 / 255.0),
        green: Color::from_rgb(0x22 as f32 / 255.0, 0xc5 as f32 / 255.0, 0x5e as f32 / 255.0),
        yellow: Color::from_rgb(0xea as f32 / 255.0, 0xb3 as f32 / 255.0, 0x08 as f32 / 255.0),
        red: Color::from_rgb(0xef as f32 / 255.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0),
        purple: Color::from_rgb(0xa8 as f32 / 255.0, 0x55 as f32 / 255.0, 0xf7 as f32 / 255.0),
        border: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        code_bg: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
        code_text: Color::from_rgb(0xc8 as f32 / 255.0, 0xc9 as f32 / 255.0, 0xcd as f32 / 255.0),
    }
}

/// Load a theme from TOML string.
fn load_theme_str(toml_str: &str) -> Option<ThemeColors> {
    let def: ThemeDef = toml::from_str(toml_str).ok()?;
    let base = fluxchat_theme();
    Some(ThemeColors {
        name: def.name,
        dark: def.dark,
        bg_deep: def.bg_deep.map(|c| c.to_color()).unwrap_or(base.bg_deep),
        bg_dark: def.bg_dark.map(|c| c.to_color()).unwrap_or(base.bg_dark),
        bg_mid: def.bg_mid.map(|c| c.to_color()).unwrap_or(base.bg_mid),
        bg_light: def.bg_light.map(|c| c.to_color()).unwrap_or(base.bg_light),
        bg_hover: def.bg_hover.map(|c| c.to_color()).unwrap_or(base.bg_hover),
        text_bright: def.text_bright.map(|c| c.to_color()).unwrap_or(base.text_bright),
        text_normal: def.text_normal.map(|c| c.to_color()).unwrap_or(base.text_normal),
        text_muted: def.text_muted.map(|c| c.to_color()).unwrap_or(base.text_muted),
        text_dim: def.text_dim.map(|c| c.to_color()).unwrap_or(base.text_dim),
        accent: def.accent.map(|c| c.to_color()).unwrap_or(base.accent),
        green: def.green.map(|c| c.to_color()).unwrap_or(base.green),
        yellow: def.yellow.map(|c| c.to_color()).unwrap_or(base.yellow),
        red: def.red.map(|c| c.to_color()).unwrap_or(base.red),
        purple: def.purple.map(|c| c.to_color()).unwrap_or(base.purple),
        border: def.border.map(|c| c.to_color()).unwrap_or(base.border),
        code_bg: def.code_bg.map(|c| c.to_color()).unwrap_or(
            if def.dark { Color::from_rgba(0.0, 0.0, 0.0, 0.3) }
            else { Color::from_rgba(0.0, 0.0, 0.0, 0.07) }
        ),
        code_text: def.code_text.map(|c| c.to_color()).unwrap_or(
            if def.dark { Color::from_rgb(0xc8 as f32 / 255.0, 0xc9 as f32 / 255.0, 0xcd as f32 / 255.0) }
            else { Color::from_rgb(0x1a as f32 / 255.0, 0x1b as f32 / 255.0, 0x1e as f32 / 255.0) }
        ),
    })
}

/// Load a theme from a TOML file.
pub fn load_theme_file(path: &std::path::Path) -> Option<ThemeColors> {
    let content = std::fs::read_to_string(path).ok()?;
    load_theme_str(&content)
}

/// Discover custom theme files from the themes directory.
pub fn discover_themes() -> Vec<ThemeColors> {
    let mut themes = vec![fluxchat_theme()];
    // Bundled themes (compiled into binary)
    // Write bundled themes to data dir on first run
    if let Some(data_dir) = dirs::data_dir() {
        let themes_dir = data_dir.join("fluxchat").join("themes");
        let _ = std::fs::create_dir_all(&themes_dir);
        let bundled: &[(&str, &str)] = &[
            ("nord.toml", include_str!("../../../assets/themes/nord.toml")),
            ("nord-light.toml", include_str!("../../../assets/themes/nord-light.toml")),
            ("dracula.toml", include_str!("../../../assets/themes/dracula.toml")),
            ("night-owl.toml", include_str!("../../../assets/themes/night-owl.toml")),
            ("tokyo-night.toml", include_str!("../../../assets/themes/tokyo-night.toml")),
            ("atom-one-dark.toml", include_str!("../../../assets/themes/atom-one-dark.toml")),
            ("catppuccin-mocha.toml", include_str!("../../../assets/themes/catppuccin-mocha.toml")),
            ("catppuccin-latte.toml", include_str!("../../../assets/themes/catppuccin-latte.toml")),
        ];
        for (filename, content) in bundled {
            let path = themes_dir.join(filename);
            if !path.exists() {
                let _ = std::fs::write(&path, content);
            }
        }
    }
    if let Some(data_dir) = dirs::data_dir() {
        let themes_dir = data_dir.join("fluxchat").join("themes");
        if themes_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&themes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "toml") {
                        if let Some(theme) = load_theme_file(&path) {
                            themes.push(theme);
                        }
                    }
                }
            }
        }
    }
    // Sort alphabetically, but keep FluxChat first
    themes[1..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    themes
}

// --- Active theme (global, set once at startup / on theme change) ---

static ACTIVE: LazyLock<RwLock<ThemeColors>> = LazyLock::new(|| RwLock::new(fluxchat_theme()));

/// Set the active theme.
pub fn set_active_theme(colors: ThemeColors) {
    *ACTIVE.write().unwrap() = colors;
}

/// Read a color from the active theme.
fn active() -> std::sync::RwLockReadGuard<'static, ThemeColors> {
    ACTIVE.read().unwrap()
}

// --- Public color accessors (drop-in replacements for the old constants) ---

// Keep the old constants for backward compatibility in const contexts (closures, style fns).
// Non-const code can use `theme::colors()` to get the active theme.
pub const BG_DEEP: Color = Color::from_rgb(0x1a as f32 / 255.0, 0x1b as f32 / 255.0, 0x1e as f32 / 255.0);
pub const BG_DARK: Color = Color::from_rgb(0x22 as f32 / 255.0, 0x23 as f32 / 255.0, 0x27 as f32 / 255.0);
pub const BG_MID: Color = Color::from_rgb(0x2a as f32 / 255.0, 0x2c as f32 / 255.0, 0x31 as f32 / 255.0);
pub const BG_LIGHT: Color = Color::from_rgb(0x32 as f32 / 255.0, 0x35 as f32 / 255.0, 0x3b as f32 / 255.0);
pub const BG_HOVER: Color = Color::from_rgb(0x3a as f32 / 255.0, 0x3d as f32 / 255.0, 0x44 as f32 / 255.0);
pub const TEXT_BRIGHT: Color = Color::from_rgb(0xe4 as f32 / 255.0, 0xe4 as f32 / 255.0, 0xe7 as f32 / 255.0);
pub const TEXT_NORMAL: Color = Color::from_rgb(0xc8 as f32 / 255.0, 0xc9 as f32 / 255.0, 0xcd as f32 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(0x8b as f32 / 255.0, 0x8d as f32 / 255.0, 0x93 as f32 / 255.0);
pub const TEXT_DIM: Color = Color::from_rgb(0x5c as f32 / 255.0, 0x5e as f32 / 255.0, 0x66 as f32 / 255.0);
pub const ACCENT: Color = Color::from_rgb(0x3b as f32 / 255.0, 0x82 as f32 / 255.0, 0xf6 as f32 / 255.0);
pub const GREEN: Color = Color::from_rgb(0x22 as f32 / 255.0, 0xc5 as f32 / 255.0, 0x5e as f32 / 255.0);
pub const YELLOW: Color = Color::from_rgb(0xea as f32 / 255.0, 0xb3 as f32 / 255.0, 0x08 as f32 / 255.0);
pub const RED: Color = Color::from_rgb(0xef as f32 / 255.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0);
pub const PURPLE: Color = Color::from_rgb(0xa8 as f32 / 255.0, 0x55 as f32 / 255.0, 0xf7 as f32 / 255.0);
pub const BORDER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);

/// Get the active theme's colors for use in non-const contexts.
pub fn colors() -> ThemeColors {
    active().clone()
}

/// The 15 IRC nick colors.
pub const NICK_COLORS: [Color; 15] = [
    Color::from_rgb(0.925, 0.380, 0.380),
    Color::from_rgb(0.925, 0.600, 0.380),
    Color::from_rgb(0.925, 0.800, 0.380),
    Color::from_rgb(0.600, 0.850, 0.400),
    Color::from_rgb(0.380, 0.850, 0.500),
    Color::from_rgb(0.380, 0.850, 0.750),
    Color::from_rgb(0.380, 0.750, 0.925),
    Color::from_rgb(0.380, 0.550, 0.925),
    Color::from_rgb(0.550, 0.380, 0.925),
    Color::from_rgb(0.750, 0.380, 0.925),
    Color::from_rgb(0.925, 0.380, 0.750),
    Color::from_rgb(0.925, 0.380, 0.550),
    Color::from_rgb(0.700, 0.820, 0.380),
    Color::from_rgb(0.380, 0.850, 0.850),
    Color::from_rgb(0.820, 0.700, 0.380),
];

/// Nick color index from a string hash.
pub fn nick_color_index(name: &str) -> usize {
    const NICK_COLOR_COUNT: u32 = 15;
    let mut hash: u32 = 0;
    for b in name.bytes() {
        hash = (b as u32).wrapping_add((hash << 5).wrapping_sub(hash));
    }
    (hash % NICK_COLOR_COUNT) as usize
}

/// Get a nick color from a username hash.
pub fn nick_color(name: &str) -> Color {
    NICK_COLORS[nick_color_index(name)]
}

/// Transparent button with no background.
pub fn button_transparent(_: &Theme, _: button::Status) -> button::Style {
    button::Style { background: None, ..Default::default() }
}

/// Button that shows a subtle highlight on hover.
pub fn button_subtle(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: if matches!(status, button::Status::Hovered) {
            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
        } else {
            None
        },
        border: Border { radius: 4.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// Panel container with BG_DARK background and BORDER.
pub fn container_panel(_: &Theme) -> container::Style {
    let t = active();
    container::Style {
        background: Some(t.bg_dark.into()),
        border: Border { color: t.border, width: 1.0, radius: 0.0.into() },
        ..Default::default()
    }
}

/// Theme-aware text_input style.
pub fn text_input_style(_: &Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    let t = active();
    let is_focused = matches!(status, iced::widget::text_input::Status::Focused { .. });
    iced::widget::text_input::Style {
        background: iced::Background::Color(t.bg_mid),
        border: Border {
            color: if is_focused { t.accent } else { t.border },
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: t.text_dim,
        placeholder: t.text_dim,
        value: t.text_bright,
        selection: Color::from_rgba(0.23, 0.51, 0.99, 0.3),
    }
}

/// Theme-aware pick_list style.
pub fn pick_list_style(_: &Theme, status: iced::widget::pick_list::Status) -> iced::widget::pick_list::Style {
    let t = active();
    let is_hovered = matches!(status, iced::widget::pick_list::Status::Hovered);
    iced::widget::pick_list::Style {
        background: iced::Background::Color(if is_hovered { t.bg_light } else { t.bg_mid }),
        text_color: t.text_bright,
        placeholder_color: t.text_dim,
        handle_color: t.text_muted,
        border: Border {
            color: t.border,
            width: 1.0,
            radius: 6.0.into(),
        },
    }
}

