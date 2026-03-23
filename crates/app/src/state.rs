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

#[derive(Clone, PartialEq)]
pub enum RightPanel {
    Pins,
    Search,
    Threads,
    Invites,
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
    pub pagination_tokens: Signal<HashMap<String, String>>,
    pub members_by_room: Signal<HashMap<String, Vec<MemberInfo>>>,
    pub typing_by_room: Signal<HashMap<String, Vec<String>>>,
    pub profile_user_id: Signal<Option<String>>,

    // Message action state
    pub reply_to: Signal<Option<(String, String, String)>>, // (event_id, sender_name, body)
    pub edit_event: Signal<Option<(String, String)>>,       // (event_id, original_body)
    pub settings_open: Signal<bool>,
    pub right_panel: Signal<Option<RightPanel>>,  // which panel is open on the right
    pub show_member_list: Signal<bool>,
    pub search_open: Signal<bool>,
    pub pins_open: Signal<bool>,

    pub loading: Signal<bool>,

    // Resolved avatar data URIs — shared between render and background tasks
    pub avatar_cache: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>>,
    pub avatar_version: Signal<u32>, // bump to trigger re-render

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
            pagination_tokens: Signal::new(HashMap::new()),
            members_by_room: Signal::new(HashMap::new()),
            typing_by_room: Signal::new(HashMap::new()),
            profile_user_id: Signal::new(None),
            reply_to: Signal::new(None),
            edit_event: Signal::new(None),
            settings_open: Signal::new(false),
            right_panel: Signal::new(None),
            show_member_list: Signal::new(true),
            search_open: Signal::new(false),
            pins_open: Signal::new(false),
            loading: Signal::new(true),
            avatar_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            avatar_version: Signal::new(0),
            open_tabs: Signal::new(Vec::new()),
        }
    }
}
