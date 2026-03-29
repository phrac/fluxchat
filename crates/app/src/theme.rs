use iced::widget::{button, container};
use iced::{Border, Color, Theme};

// Color palette from the existing CSS design
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

/// The 15 IRC nick colors.
pub const NICK_COLORS: [Color; 15] = [
    Color::from_rgb(0.925, 0.380, 0.380), // #ec6161 — warm red
    Color::from_rgb(0.925, 0.600, 0.380), // #ec9961 — orange
    Color::from_rgb(0.925, 0.800, 0.380), // #eccc61 — gold
    Color::from_rgb(0.600, 0.850, 0.400), // #99d966 — lime
    Color::from_rgb(0.380, 0.850, 0.500), // #61d980 — green
    Color::from_rgb(0.380, 0.850, 0.750), // #61d9bf — teal
    Color::from_rgb(0.380, 0.750, 0.925), // #61bfec — sky
    Color::from_rgb(0.380, 0.550, 0.925), // #618cec — blue
    Color::from_rgb(0.550, 0.380, 0.925), // #8c61ec — indigo
    Color::from_rgb(0.750, 0.380, 0.925), // #bf61ec — purple
    Color::from_rgb(0.925, 0.380, 0.750), // #ec61bf — pink
    Color::from_rgb(0.925, 0.380, 0.550), // #ec618c — rose
    Color::from_rgb(0.700, 0.820, 0.380), // #b3d161 — yellow-green
    Color::from_rgb(0.380, 0.850, 0.850), // #61d9d9 — cyan
    Color::from_rgb(0.820, 0.700, 0.380), // #d1b361 — tan
];

/// Nick color index from a string hash (same algorithm as Dioxus app).
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
    container::Style {
        background: Some(BG_DARK.into()),
        border: Border { color: BORDER, width: 1.0, radius: 0.0.into() },
        ..Default::default()
    }
}
