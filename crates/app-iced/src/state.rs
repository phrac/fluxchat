use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fluxchat_core::{
    AppState, MemberInfo, MessageInfo, ReadReceipt, RoomInfo, VerificationInfo,
};

/// Tab entry: (room_id, room_name, space_name, space_avatar_mxc)
pub type TabEntry = (String, String, Option<String>, Option<String>);

/// Which panel is open on the right side.
#[derive(Debug, Clone, PartialEq)]
pub enum RightPanel {
    Pins,
    Search,
    Threads,
    Invites,
}

/// Which modal is currently open.
#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    Settings { tab: String },
    CreateRoom,
    RoomSettings,
    Profile { user_id: String },
    Verification { info: VerificationInfo },
    Lightbox { url: String },
    Forward { body: String, formatted_body: String, sender_name: Option<String> },
}

/// The screen the app is on.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Login,
    Main,
}

/// All application state — replaces Dioxus signals with plain fields.
pub struct FluxChat {
    pub core: Arc<AppState>,
    pub screen: Screen,
    pub loading: bool,

    // Auth
    pub user_id: String,
    pub display_name: Option<String>,

    // Rooms
    pub rooms: Vec<RoomInfo>,
    pub spaces: Vec<RoomInfo>,
    pub invited_rooms: Vec<RoomInfo>,
    pub selected_room_id: Option<String>,
    pub selected_space_id: Option<String>,

    // Messages & members
    pub messages_by_room: HashMap<String, Vec<MessageInfo>>,
    pub members_by_room: HashMap<String, Vec<MemberInfo>>,
    pub typing_by_room: HashMap<String, Vec<String>>,
    pub receipts_by_room: HashMap<String, Vec<ReadReceipt>>,

    // Message actions
    pub reply_to: Option<(String, String, String)>,  // (event_id, sender_name, body)
    pub edit_event: Option<(String, String)>,         // (event_id, original_body)
    pub upload_pending: Option<(String, String, Vec<u8>)>, // (filename, mime, data)

    // UI panels
    pub right_panel: Option<RightPanel>,
    pub show_member_list: bool,
    pub search_open: bool,
    pub pins_open: bool,
    pub threads_open: bool,
    pub files_open: bool,
    pub notices_open: bool,
    pub gifs_open: bool,
    pub gif_query: String,
    pub help_text: Option<String>,
    pub modal: Option<Modal>,

    // Tabs
    pub open_tabs: Vec<TabEntry>,

    // Unread tracking
    pub read_rooms: HashSet<String>,
    pub last_read_event: HashMap<String, String>,

    // User prefs (persisted to SQLite)
    pub ignored_users: HashSet<String>,
    pub highlight_words: Vec<String>,
    pub timestamp_format: String,  // "short", "full", "off"
    pub compact_mode: bool,
    pub show_joinpart: bool,
    pub zoom_level: u32,
    pub giphy_api_key: String,

    // Status
    pub sync_lag_ms: u64,

    // Media/avatar cache
    pub avatar_cache: HashMap<String, String>,

    // Login form state
    pub login_homeserver: String,
    pub login_username: String,
    pub login_password: String,
    pub login_error: Option<String>,
    pub login_submitting: bool,
    pub login_caps: Option<fluxchat_core::auth::LoginCapabilities>,

    // Room list UI
    pub collapsed_sections: HashSet<String>,

    // Search
    pub search_query: String,
    pub search_results: Vec<fluxchat_core::messages::SearchResult>,

    // Sync state
    pub sync_running: bool,

    // Input state
    pub input_text: String,
    pub cmd_completions: Vec<(&'static str, &'static str)>, // (name, description)
    pub cmd_completion_idx: usize,
    pub mention_completions: Vec<(String, String)>, // (user_id, display_name)
    pub mention_completion_idx: usize,
}

impl FluxChat {
    pub fn new(core: Arc<AppState>) -> Self {
        Self {
            core,
            screen: Screen::Login,
            loading: true,
            user_id: String::new(),
            display_name: None,
            rooms: Vec::new(),
            spaces: Vec::new(),
            invited_rooms: Vec::new(),
            selected_room_id: None,
            selected_space_id: None,
            messages_by_room: HashMap::new(),
            members_by_room: HashMap::new(),
            typing_by_room: HashMap::new(),
            receipts_by_room: HashMap::new(),
            reply_to: None,
            edit_event: None,
            upload_pending: None,
            right_panel: None,
            show_member_list: true,
            search_open: false,
            pins_open: false,
            threads_open: false,
            files_open: false,
            notices_open: false,
            gifs_open: false,
            gif_query: String::new(),
            help_text: None,
            modal: None,
            open_tabs: Vec::new(),
            read_rooms: HashSet::new(),
            last_read_event: HashMap::new(),
            ignored_users: HashSet::new(),
            highlight_words: Vec::new(),
            timestamp_format: "short".into(),
            compact_mode: false,
            show_joinpart: true,
            zoom_level: 100,
            giphy_api_key: String::new(),
            sync_lag_ms: 0,
            avatar_cache: HashMap::new(),
            login_homeserver: "matrix.org".into(),
            login_username: String::new(),
            login_password: String::new(),
            login_error: None,
            login_submitting: false,
            login_caps: None,
            collapsed_sections: HashSet::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            sync_running: false,
            input_text: String::new(),
            cmd_completions: Vec::new(),
            cmd_completion_idx: 0,
            mention_completions: Vec::new(),
            mention_completion_idx: 0,
        }
    }
}
