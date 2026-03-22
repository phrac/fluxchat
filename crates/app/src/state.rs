use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use dioxus::prelude::*;
use fluxchat_core::{AppDb, AppState, RoomInfo, MessageInfo, MemberInfo};

/// Initialize the core AppState with SQLite database.
pub fn init_core() -> Arc<AppState> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fluxchat");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");

    let db_path = data_dir.join("fluxchat.db");
    let db = AppDb::open(&db_path).expect("Failed to open database");

    Arc::new(AppState::new(db))
}

/// Global app signals — the reactive UI state.
#[derive(Clone)]
pub struct UiState {
    pub logged_in: Signal<bool>,
    pub user_id: Signal<String>,
    pub display_name: Signal<Option<String>>,

    pub rooms: Signal<Vec<RoomInfo>>,
    pub spaces: Signal<Vec<RoomInfo>>,
    pub selected_room_id: Signal<Option<String>>,
    pub selected_space_id: Signal<Option<String>>,

    pub messages_by_room: Signal<HashMap<String, Vec<MessageInfo>>>,
    pub members_by_room: Signal<HashMap<String, Vec<MemberInfo>>>,
    pub typing_by_room: Signal<HashMap<String, Vec<String>>>,

    pub loading: Signal<bool>,

    // Channel tabs — recently visited rooms
    // (room_id, room_name, space_name, space_avatar_mxc)
    pub open_tabs: Signal<Vec<(String, String, Option<String>, Option<String>)>>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            logged_in: Signal::new(false),
            user_id: Signal::new(String::new()),
            display_name: Signal::new(None),
            rooms: Signal::new(Vec::new()),
            spaces: Signal::new(Vec::new()),
            selected_room_id: Signal::new(None),
            selected_space_id: Signal::new(None),
            messages_by_room: Signal::new(HashMap::new()),
            members_by_room: Signal::new(HashMap::new()),
            typing_by_room: Signal::new(HashMap::new()),
            loading: Signal::new(true),
            open_tabs: Signal::new(Vec::new()),
        }
    }
}
