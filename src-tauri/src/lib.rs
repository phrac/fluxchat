mod commands;
mod db;
mod events;
mod state;

use state::AppState;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fluxchat")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    // Open FluxDB with batch_size=1 so every write flushes immediately
    let db_path = data_dir().join("fluxdb_data");
    std::fs::create_dir_all(&db_path).expect("Failed to create data directory");
    let db = fluxdb::database::Database::open_with_wal(
        &db_path,
        1,     // batch_size: flush after every write
        1024,  // batch_bytes
        fluxdb::wal::SyncMode::EveryFlush,
    )
    .expect("Failed to open FluxDB");
    db::init_collections(&db);

    let app_state = AppState::new(db);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::logout,
            commands::auth::restore_session,
            commands::rooms::get_rooms,
            commands::rooms::join_room,
            commands::rooms::leave_room,
            commands::rooms::invite_user,
            commands::messages::get_room_messages,
            commands::messages::send_message,
            commands::messages::send_reply,
            commands::messages::send_typing,
            commands::messages::send_reaction,
            commands::messages::delete_message,
            commands::messages::get_pinned_messages,
            commands::messages::pin_message,
            commands::messages::get_room_members,
            commands::spaces::get_spaces,
            commands::spaces::get_space_rooms,
            commands::media::get_media_url,
            commands::verification::accept_verification,
            commands::verification::confirm_verification,
            commands::verification::cancel_verification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
