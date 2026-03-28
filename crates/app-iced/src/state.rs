use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fluxchat_core::{
    AppState, MemberInfo, MessageInfo, ReadReceipt, RoomInfo, VerificationInfo,
};

/// Channel for timeline diff stream watcher to send updates back to the Iced event loop.
pub type TimelineUpdateTx = tokio::sync::mpsc::UnboundedSender<crate::message::Message>;
pub type TimelineUpdateRx = tokio::sync::mpsc::UnboundedReceiver<crate::message::Message>;

/// Global channel receiver — taken by the Iced subscription on first call.
pub static TIMELINE_RX: std::sync::OnceLock<std::sync::Mutex<Option<TimelineUpdateRx>>> =
    std::sync::OnceLock::new();

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
    pub overflow_menu_open: bool,
    pub hovered_message: Option<String>, // event_id of hovered message
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

    // Media/avatar cache — maps MXC URI to stable image handles (avoids flicker on re-render)
    pub avatar_cache: HashMap<String, iced::widget::image::Handle>,

    // Inline image cache — maps MXC URI to image handle (full size for message images)
    pub image_cache: HashMap<String, iced::widget::image::Handle>,
    pub image_cache_pending: std::collections::HashSet<String>,

    // URL preview cache — maps URL to preview data
    pub url_previews: HashMap<String, fluxchat_core::UrlPreview>,
    pub url_preview_images: HashMap<String, iced::widget::image::Handle>, // og:image mxc/url -> handle
    pub url_previews_pending: std::collections::HashSet<String>, // URLs currently being fetched

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

    // Timeline channel — background diff watchers send updates through this
    pub timeline_tx: TimelineUpdateTx,

    // Input state
    pub input_text: String,
    pub cmd_completions: Vec<(&'static str, &'static str)>, // (name, description)
    pub cmd_completion_idx: usize,
    pub mention_completions: Vec<(String, String)>, // (user_id, display_name)
    pub mention_completion_idx: usize,

    // Settings modal state
    pub settings_devices: Vec<fluxchat_core::devices::DeviceInfo>,
    pub settings_nick_input: String,
    pub settings_status: Option<String>,
    pub settings_signout_password: String,
    pub settings_signout_device: Option<String>, // device_id being signed out

    // Account tab - avatar/banner
    pub settings_avatar_data: Option<Vec<u8>>,   // resolved avatar image bytes
    pub settings_banner_data: Option<Vec<u8>>,   // resolved banner image bytes

    // Security tab state
    pub security_state: Option<SecurityState>,
    pub security_recovery_key_input: String,
    pub security_shown_recovery_key: Option<String>,
}

/// Encryption/backup state fetched from Matrix SDK.
#[derive(Debug, Clone)]
pub struct SecurityState {
    pub is_verified: bool,
    pub backup_exists: bool,
    pub backup_enabled: bool,
    pub recovery_state: String,
}

impl FluxChat {
    pub fn new(core: Arc<AppState>) -> Self {
        let (timeline_tx, timeline_rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = TIMELINE_RX.set(std::sync::Mutex::new(Some(timeline_rx)));
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
            overflow_menu_open: false,
            hovered_message: None,
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
            timeline_tx,
            sync_running: false,
            input_text: String::new(),
            cmd_completions: Vec::new(),
            cmd_completion_idx: 0,
            mention_completions: Vec::new(),
            mention_completion_idx: 0,
            settings_devices: Vec::new(),
            settings_nick_input: String::new(),
            settings_status: None,
            settings_signout_password: String::new(),
            settings_signout_device: None,
            image_cache: HashMap::new(),
            image_cache_pending: std::collections::HashSet::new(),
            url_previews: HashMap::new(),
            url_preview_images: HashMap::new(),
            url_previews_pending: std::collections::HashSet::new(),
            settings_avatar_data: None,
            settings_banner_data: None,
            security_state: None,
            security_recovery_key_input: String::new(),
            security_shown_recovery_key: None,
        }
    }
}
