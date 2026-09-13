#![allow(mismatched_lifetime_syntaxes)]

mod commands;
mod html;
mod icons;
mod message;
mod primary_clipboard;
mod selectable_text;
mod state;
mod theme;
mod update;
mod update_call;
mod update_settings;
mod view;
mod view_input;
mod view_members;
mod view_messages;
mod view_modals;

use std::path::PathBuf;
use std::sync::Arc;

use fluxchat_core::{AppDb, AppState};
use iced::window;
use iced::Size;

use state::FluxChat;

/// Initialize the core AppState with SQLite database.
fn init_core() -> Arc<AppState> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fluxchat");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");

    let db_path = data_dir.join("fluxchat.db");
    let db = AppDb::open(&db_path).expect("Failed to open database");

    Arc::new(AppState::new(db))
}

fn main() -> iced::Result {
    // RUST_LOG overrides the default filter (e.g. RUST_LOG=matrix_sdk_crypto=debug
    // to diagnose decryption failures).
    let default_filter = "warn,fluxchat=info,matrix_sdk_crypto=warn,html5ever=error";
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    // Set macOS notification app identity to avoid "Where is use_default?" dialog
    #[cfg(target_os = "macos")]
    { let _ = notify_rust::set_application("com.apple.Terminal"); }

    let core = init_core();
    let (win_w, win_h) = state::saved_window_size(&core.db)
        .unwrap_or((state::DEFAULT_WINDOW_WIDTH, state::DEFAULT_WINDOW_HEIGHT));

    let core2 = core.clone();
    iced::application(
        move || FluxChat::new_with_init(core2.clone()),
        FluxChat::update,
        FluxChat::view,
    )
    .subscription(FluxChat::subscription)
    .theme(FluxChat::theme)
    .font(include_bytes!("../fonts/JetBrainsMonoNerdFont-Regular.ttf").as_slice())
    .font(include_bytes!("../fonts/JetBrainsMonoNerdFont-Bold.ttf").as_slice())
    .default_font(iced::Font {
        family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
        ..iced::Font::MONOSPACE
    })
    .window(window::Settings {
        size: Size::new(win_w, win_h),
        min_size: Some(Size::new(800.0, 400.0)),
        decorations: cfg!(target_os = "macos"),
        icon: load_icon(),
        ..Default::default()
    })
    .run()
}

fn load_icon() -> Option<window::icon::Icon> {
    let bytes = include_bytes!("../../../assets/icons/icon_256x256.png");
    window::icon::from_file_data(bytes, None).ok()
}
