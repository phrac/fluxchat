#![allow(dead_code, mismatched_lifetime_syntaxes)]

mod commands;
mod html;
mod message;
mod state;
mod theme;
mod update;
mod view;

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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("warn,fluxchat_iced=info,matrix_sdk_crypto=error"),
        )
        .init();

    let core = init_core();

    let core2 = core.clone();
    iced::application(
        move || FluxChat::new_with_init(core2.clone()),
        FluxChat::update,
        FluxChat::view,
    )
    .subscription(FluxChat::subscription)
    .theme(FluxChat::theme)
    .window(window::Settings {
        size: Size::new(1200.0, 800.0),
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
