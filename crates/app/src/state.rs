use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fluxchat_core::{
    AppState, MemberInfo, MessageInfo, RoomInfo, VerificationInfo,
};

/// Channel for timeline diff stream watcher to send updates back to the Iced event loop.
pub const DEFAULT_WINDOW_WIDTH: f32 = 1200.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 800.0;

/// Read the persisted window size ("WxH"), if any.
pub fn saved_window_size(db: &fluxchat_core::AppDb) -> Option<(f32, f32)> {
    let value = db.get_setting("window_size")?;
    let (w, h) = value.split_once('x')?;
    let (w, h) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
    (w >= 400.0 && h >= 300.0).then_some((w, h))
}

pub type TimelineUpdateTx = tokio::sync::mpsc::UnboundedSender<crate::message::Message>;
pub type TimelineUpdateRx = tokio::sync::mpsc::UnboundedReceiver<crate::message::Message>;

/// Channel for call events from LiveKit to the Iced event loop.
pub type CallEventTx = tokio::sync::mpsc::UnboundedSender<crate::message::Message>;
pub type CallEventRx = tokio::sync::mpsc::UnboundedReceiver<crate::message::Message>;

/// Global channel receiver — taken by the Iced subscription on first call.
pub static TIMELINE_RX: std::sync::OnceLock<std::sync::Mutex<Option<TimelineUpdateRx>>> =
    std::sync::OnceLock::new();

/// Global call event channel receiver.
pub static CALL_RX: std::sync::OnceLock<std::sync::Mutex<Option<CallEventRx>>> =
    std::sync::OnceLock::new();

/// Tab entry: (room_id, room_name, space_name, space_avatar_mxc)
pub type TabEntry = (String, String, Option<String>, Option<String>);

#[derive(Debug, Clone)]
pub enum ContextMenu {
    Room { room_id: String, is_dm: bool },
    Space { space_id: String },
    Message { event_id: String },
}

/// Which modal is currently open.
#[derive(Debug, Clone)]
pub enum Modal {
    Settings { tab: String },
    CreateRoom,
    CreateDm,
    RoomSettings,
    Profile { user_id: String },
    Verification { info: VerificationInfo },
    Lightbox { url: String },
    Forward { body: String, formatted_body: String, sender_name: Option<String> },
    Invite,
    Report { event_id: String, room_id: String },
    ConfirmLeave { room_id: String, name: String },
    /// Raw event JSON for "View source".
    EventSource { event_id: String, json: String },
    SpaceDetails { space_id: String },
    PendingInvites,
    DevTools,
    Directory,
}



/// Tabs of the space dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceTab {
    Rooms,
    Members,
    Settings,
}

/// What action selecting an emoji should perform.
#[derive(Debug, Clone, PartialEq)]
pub enum EmojiPickerMode {
    /// Insert the emoji character into the message input.
    Insert,
    /// Send the emoji as a reaction to the given event.
    React(String), // event_id
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
    pub space_tree: Vec<fluxchat_core::rooms::SpaceNode>,
    pub space_descendants: HashMap<String, Vec<String>>,
    pub invited_rooms: Vec<RoomInfo>,
    pub knocked_rooms: Vec<RoomInfo>,
    pub selected_room_id: Option<String>,
    pub selected_space_id: Option<String>,

    // Messages & members
    pub messages_by_room: HashMap<String, Vec<MessageInfo>>,
    pub members_by_room: HashMap<String, Vec<MemberInfo>>,
    pub typing_by_room: HashMap<String, (Vec<String>, std::time::Instant)>, // (user_ids, last_updated)

    // Message actions
    pub reply_to: Option<(String, String, String)>,  // (event_id, sender_name, body)
    pub edit_event: Option<(String, String)>,         // (event_id, original_body)
    pub upload_pending: Option<(String, String, Vec<u8>)>, // (filename, mime, data)

    // UI panels
    pub show_member_list: bool,
    pub search_open: bool,
    pub pins_open: bool,
    pub threads_open: bool,
    pub files_open: bool,
    pub notices_open: bool,
    pub gifs_open: bool,
    pub audio_playing: Option<String>,      // event_id of currently playing audio
    pub voice_recording: bool,
    pub voice_record_start: Option<std::time::Instant>,
    pub voice_ready: Option<(Vec<u8>, u64)>, // encoded ogg bytes + duration_ms, waiting to send
    pub new_messages_below: bool, // true when msgs arrived while scrolled up
    pub expanded_code_blocks: HashSet<String>, // event_ids with expanded code
    pub report_reason: String,
    pub quick_switcher_open: bool,
    pub quick_switcher_query: String,
    pub quick_switcher_selected: usize,
    pub audio_progress: f32,                // 0.0..1.0 playback progress
    pub audio_start_time: Option<std::time::Instant>, // when playback started
    pub audio_duration_ms: u64,             // duration of current audio
    pub selection: crate::selectable_text::CrossMessageSelection,
    pub video_thumb_sizes: HashMap<String, (u32, u32)>,  // thumb_key -> (width, height)
    pub video_player: Option<iced_video_player::Video>, // inline video player
    pub video_playing_event: Option<String>,            // event_id of video being played
    pub video_temp_path: Option<std::path::PathBuf>,    // temp file to clean up
    pub video_position: f64,                            // current position in seconds
    pub video_dragging: bool,                           // true while seeking via slider
    pub location_open: bool,
    pub location_lat: String,
    pub location_lon: String,
    pub emoji_open: bool,
    pub emoji_mode: EmojiPickerMode,
    pub emoji_query: String,
    pub emoji_category: u8, // 0=frequent, 1-9=emojis::Group
    pub emoji_freq: std::collections::HashMap<String, u32>,
    pub create_menu_open: bool,
    pub context_menu: Option<ContextMenu>,
    pub context_menu_position: (f32, f32),
    /// Rooms we're in the process of leaving (filter from sync updates)
    pub leaving_rooms: HashSet<String>,
    pub hovered_message: Option<String>, // event_id of hovered message
    pub devtools_state_events: Vec<(String, String)>, // (event_type, json)
    pub devtools_tab: String,                         // "state" or "send"
    pub devtools_send_type: String,                   // event type to send
    pub devtools_send_state_key: String,              // state key (empty for message events)
    pub devtools_send_body: String,                   // JSON body to send
    pub devtools_is_state: bool,                      // sending state event vs message event
    pub directory_results: Vec<fluxchat_core::rooms::PublicRoomResult>,
    pub directory_search: String,
    pub directory_server: String,                      // server to query ("" = own homeserver)
    pub directory_next_batch: Option<String>,
    pub directory_loading: bool,
    pub gif_query: String,
    pub gif_searched: bool, // true after a search has completed
    pub help_text: Option<String>,
    pub modal: Option<Modal>,

    // Tabs
    pub open_tabs: Vec<TabEntry>,

    // Unread tracking
    pub read_rooms: HashSet<String>,
    pub last_read_event: HashMap<String, String>,
    pub last_seen_timestamp: HashMap<String, u64>, // room_id → newest message timestamp we've processed
    pub thread_counts: HashMap<String, HashMap<String, u32>>, // room_id → { thread_root → reply_count }

    // User prefs (persisted to SQLite)
    pub ignored_users: HashSet<String>,
    pub highlight_words: Vec<String>,
    pub timestamp_format: String,  // "short", "full", "off"
    pub compact_mode: bool,
    pub show_joinpart: bool,
    pub zoom_level: u32,
    pub msg_font_size: f32,
    pub msg_font_family: String,     // font family name for message area
    pub sidebar_font_family: String, // font family name for sidebar
    pub available_fonts: Vec<String>, // discovered system font families
    pub available_themes: Vec<crate::theme::ThemeColors>,
    pub active_theme_name: String,
    pub giphy_api_key: String,

    // Notifications
    pub notifications_enabled: bool,
    pub notify_mentions_only: bool,
    pub notify_show_preview: bool,
    pub notify_sound: bool,
    pub app_focused: bool,

    // Status
    pub sync_lag_ms: u64,
    pub sidebar_width: f32,
    pub sidebar_dragging: bool,
    /// Layout values changed since they were last written to SQLite;
    /// flushed on the next Tick so resize drags don't hammer the database.
    pub layout_dirty: bool,
    pub window_size: (f32, f32),
    pub device_id: Option<String>,

    // Media/avatar cache — maps MXC URI to stable image handles (avoids flicker on re-render)
    pub avatar_cache: HashMap<String, iced::widget::image::Handle>,
    /// Avatar MXC URIs with a fetch in flight.
    pub avatar_pending: std::collections::HashSet<String>,
    /// Avatar MXC URIs whose last fetch failed, and when.
    pub avatar_failed: HashMap<String, std::time::Instant>,

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
    pub search_selected: Option<usize>,
    /// The current query has been searched (results, possibly none, arrived).
    pub search_done: bool,
    pub highlight_event_id: Option<String>,

    // Sync state
    pub sync_running: bool,

    // Timeline channel — background diff watchers send updates through this
    pub timeline_tx: TimelineUpdateTx,

    // Input state
    pub input_content: iced::widget::text_editor::Content,
    pub input_text: String, // plain text mirror of input_content
    pub cmd_completions: Vec<(&'static str, &'static str)>, // (name, description)
    pub cmd_completion_idx: usize,
    pub mention_completions: Vec<(String, String)>, // (user_id, display_name)
    pub mention_completion_idx: usize,
    pub emoji_completions: Vec<(String, String)>, // (emoji, shortcode)
    pub emoji_completion_idx: usize,
    /// Sync problem shown in the status bar (None = healthy).
    pub sync_status: Option<String>,
    /// Homeserver upload limit in bytes, if it reported one.
    pub max_upload_size: Option<u64>,
    /// Unsent input text per view, restored when switching back.
    pub drafts: HashMap<String, String>,
    /// Member list filter (name / user ID substring).
    pub member_filter: String,

    // Settings modal state
    pub settings_devices: Vec<fluxchat_core::devices::DeviceInfo>,
    pub settings_nick_input: String,
    pub settings_status: Option<String>,
    pub settings_signout_password: String,
    pub settings_signout_device: Option<String>, // device_id being signed out

    // Account tab - avatar/banner (cached handles to avoid flicker)
    pub settings_avatar_data: Option<Vec<u8>>,
    pub settings_avatar_handle: Option<iced::widget::image::Handle>,
    pub settings_banner_data: Option<Vec<u8>>,
    pub settings_banner_handle: Option<iced::widget::image::Handle>,
    pub settings_banner_offset: f32, // vertical crop offset 0.0 (top) to 1.0 (bottom)
    pub settings_banner_dimensions: Option<(u32, u32)>, // original width, height
    // Password change
    pub settings_old_password: String,
    pub settings_new_password: String,
    pub settings_confirm_password: String,

    // Room creation modal state
    pub create_room_name: String,
    pub create_room_topic: String,
    pub create_room_public: bool,
    pub create_room_encrypted: bool,
    pub create_room_space: Option<String>, // space room_id to create in
    pub create_room_is_space: bool,

    // Room settings modal state
    pub room_settings_name: String,
    pub room_settings_topic: String,
    /// Notification mode of the room in Room Settings ("all"/"mentions"/"mute").
    pub room_notify_mode: Option<String>,
    /// Spaces the room in Room Settings belongs to (`None` while loading).
    pub room_parent_spaces: Option<Vec<String>>,
    /// Spaces we may add rooms to (for Room Settings' "Add to space").
    pub manageable_spaces: Vec<String>,
    /// Space picked in Room Settings' "Add to space" picker.
    pub room_settings_add_space: Option<String>,

    // Profile modal state
    pub profile_power_input: String,
    /// Whether the user shown in profile modal is verified (None = not yet loaded)
    pub profile_user_verified: Option<bool>,
    /// Moderation action awaiting confirmation on the profile card.
    pub profile_confirm: Option<String>,
    /// Presence fetched from the server when the profile opened
    /// (`Some(None)` = server wouldn't say).
    pub profile_presence: Option<Option<fluxchat_core::Presence>>,

    // Forward modal state
    pub forward_search: String,

    // Invite modal state
    pub invite_search: String,
    pub invite_results: Vec<fluxchat_core::rooms::UserSearchResult>,
    pub invite_target_id: Option<String>, // override room_id for space invites

    // DM modal state
    pub dm_search: String,
    pub dm_results: Vec<fluxchat_core::rooms::UserSearchResult>,

    // Space details modal state
    /// Space dialog: children (`None` while loading), members, permissions.
    pub space_detail_rooms: Option<Vec<fluxchat_core::spaces::SpaceChild>>,
    pub space_detail_members: Vec<fluxchat_core::MemberInfo>,
    pub space_perms: fluxchat_core::spaces::SpacePermissions,
    pub space_tab: SpaceTab,
    pub space_filter: String,
    /// Showing the "add existing rooms" picker instead of the children list.
    pub space_adding: bool,
    /// Child awaiting remove confirmation.
    pub space_confirm_remove: Option<String>,
    /// Rooms with an add/remove/join in flight.
    pub space_busy: HashSet<String>,
    pub space_name_input: String,
    pub space_topic_input: String,
    pub space_status: Option<String>,

    /// Measured height of each message row (by event ID), filled in by the
    /// `Measured` wrapper during layout. Unmeasured rows use an estimate.
    pub row_heights: crate::measured::RowHeights,
    /// Height of the message scrollable's viewport (from the last scroll event).
    pub messages_viewport_h: f32,
    /// The first row intersecting the viewport at the last render and its
    /// offset from the top of the list, so height changes above it (rows
    /// getting measured, history prepended) can be compensated.
    pub virt_anchor: std::cell::RefCell<Option<(String, f32)>>,
    /// Diagnostic hints for undecryptable sessions, keyed by session ID.
    pub utd_hints: HashMap<String, String>,
    /// Session IDs whose diagnostics have been (or are being) computed.
    pub utd_checked: HashSet<String>,
    /// Number of 5s Ticks since startup, for slower periodic work.
    pub tick_count: u64,
    /// Messages deleted locally whose redaction the timeline hasn't shown yet, per room.
    pub pending_redactions: HashMap<String, std::collections::HashSet<String>>,
    /// Parsed message bodies, so view() doesn't re-parse HTML every frame.
    pub body_cache: crate::html::BodyCache,

    // Scroll tracking — true when messages scrollable is at/near bottom
    pub messages_at_bottom: bool,
    // Previous scroll offset — used to distinguish user scroll-up from content growth
    pub last_scroll_offset: f32,
    // Whether a back-pagination request is currently in flight
    pub paginating: bool,
    // Message count when pagination was triggered — to detect when new messages arrive
    pub paginate_msg_count: usize,

    // Animated GIF cache for inline messages
    pub gif_frames_cache: HashMap<String, std::sync::Arc<iced_gif::Frames>>,

    // GIF panel state
    pub gif_results: Vec<fluxchat_core::gif::GifResult>,
    pub gif_searching: bool,
    pub gif_preview_cache: HashMap<String, std::sync::Arc<iced_gif::Frames>>, // preview_url -> animated frames

    // Panels data
    pub pinned_messages: Vec<fluxchat_core::MessageInfo>,
    pub thread_messages: Vec<fluxchat_core::MessageInfo>,

    // Security tab state
    pub security_state: Option<SecurityState>,
    pub security_recovery_key_input: String,
    pub security_shown_recovery_key: Option<String>,
    // Reset-identity flow (when recovery key is lost)
    pub security_reset_confirm: bool,          // one-click confirmation gate
    pub security_reset_password: String,       // UIAA password
    pub security_resetting: bool,              // in-flight flag

    // --- Call/RTC state ---
    pub expanded_previews: HashSet<String>, // event_ids with all URL previews shown
    pub calls_supported: bool, // true if homeserver has LiveKit configured
    pub call_state: fluxchat_core::rtc::CallConnectionState,
    pub call_room_id: Option<String>,
    pub call_participants: Vec<fluxchat_core::rtc::CallParticipant>,
    pub rooms_with_calls: HashSet<String>,
    pub call_mic_muted: bool,
    pub call_disconnect_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub call_event_tx: CallEventTx,
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
        {
            let tx: TimelineUpdateTx = timeline_tx.clone();
            core.timelines.set_sink(Arc::new(move |room_id, messages| {
                let _ = tx.send(crate::message::Message::SyncEvent(
                    crate::message::SyncEvent::MessagesUpdated(room_id, messages),
                ));
            }));
        }
        let _ = TIMELINE_RX.set(std::sync::Mutex::new(Some(timeline_rx)));
        let (call_event_tx, call_event_rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = CALL_RX.set(std::sync::Mutex::new(Some(call_event_rx)));
        Self {
            core,
            screen: Screen::Login,
            loading: true,
            user_id: String::new(),
            display_name: None,
            rooms: Vec::new(),
            spaces: Vec::new(),
            space_tree: Vec::new(),
            space_descendants: HashMap::new(),
            invited_rooms: Vec::new(),
            knocked_rooms: Vec::new(),
            selected_room_id: None,
            selected_space_id: None,
            messages_by_room: HashMap::new(),
            members_by_room: HashMap::new(),
            typing_by_room: HashMap::new(),
            reply_to: None,
            edit_event: None,
            upload_pending: None,
            show_member_list: true,
            search_open: false,
            pins_open: false,
            threads_open: false,
            files_open: false,
            notices_open: false,
            create_menu_open: false,
            context_menu: None,
            context_menu_position: (0.0, 0.0),
            leaving_rooms: HashSet::new(),
            hovered_message: None,
            devtools_state_events: Vec::new(),
            devtools_tab: "state".into(),
            devtools_send_type: String::new(),
            devtools_send_state_key: String::new(),
            devtools_send_body: "{}".into(),
            devtools_is_state: true,
            directory_results: Vec::new(),
            directory_search: String::new(),
            directory_server: String::new(),
            directory_next_batch: None,
            directory_loading: false,
            gifs_open: false,
            audio_playing: None,
            voice_recording: false,
            voice_record_start: None,
            voice_ready: None,
            new_messages_below: false,
            expanded_code_blocks: HashSet::new(),
            report_reason: String::new(),
            quick_switcher_open: false,
            quick_switcher_query: String::new(),
            quick_switcher_selected: 0,
            audio_progress: 0.0,
            audio_start_time: None,
            audio_duration_ms: 0,
            selection: Default::default(),
            video_thumb_sizes: HashMap::new(),
            video_player: None,
            video_playing_event: None,
            video_temp_path: None,
            video_position: 0.0,
            video_dragging: false,
            location_open: false,
            location_lat: String::new(),
            location_lon: String::new(),
            emoji_open: false,
            emoji_mode: EmojiPickerMode::Insert,
            emoji_query: String::new(),
            emoji_category: 0,
            emoji_freq: std::collections::HashMap::new(),
            gif_query: String::new(),
            gif_searched: false,
            help_text: None,
            modal: None,
            open_tabs: Vec::new(),
            read_rooms: HashSet::new(),
            last_read_event: HashMap::new(),
            last_seen_timestamp: HashMap::new(),
            thread_counts: HashMap::new(),
            ignored_users: HashSet::new(),
            highlight_words: Vec::new(),
            timestamp_format: "short".into(),
            compact_mode: false,
            show_joinpart: true,
            zoom_level: 100,
            msg_font_size: 14.0,
            msg_font_family: "JetBrainsMono Nerd Font".to_string(),
            sidebar_font_family: "JetBrainsMono Nerd Font".to_string(),
            available_fonts: Vec::new(),
            available_themes: Vec::new(),
            active_theme_name: "FluxChat".to_string(),
            giphy_api_key: String::new(),
            notifications_enabled: true,
            notify_mentions_only: false,
            notify_show_preview: true,
            notify_sound: true,
            app_focused: true,
            sync_lag_ms: 0,
            sidebar_width: 220.0,
            sidebar_dragging: false,
            layout_dirty: false,
            window_size: (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
            device_id: None,
            avatar_cache: HashMap::new(),
            avatar_pending: Default::default(),
            avatar_failed: HashMap::new(),
            login_homeserver: "matrix.org".into(),
            login_username: String::new(),
            login_password: String::new(),
            login_error: None,
            login_submitting: false,
            login_caps: None,
            collapsed_sections: HashSet::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: None,
            search_done: false,
            highlight_event_id: None,
            timeline_tx,
            sync_running: false,
            input_content: iced::widget::text_editor::Content::new(),
            input_text: String::new(),
            cmd_completions: Vec::new(),
            cmd_completion_idx: 0,
            mention_completions: Vec::new(),
            mention_completion_idx: 0,
            emoji_completions: Vec::new(),
            emoji_completion_idx: 0,
            sync_status: None,
            max_upload_size: None,
            drafts: HashMap::new(),
            member_filter: String::new(),
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
            create_room_name: String::new(),
            create_room_topic: String::new(),
            create_room_public: false,
            create_room_encrypted: false,
            create_room_space: None,
            create_room_is_space: false,
            room_settings_name: String::new(),
            room_settings_topic: String::new(),
            room_notify_mode: None,
            room_parent_spaces: None,
            manageable_spaces: Vec::new(),
            room_settings_add_space: None,
            profile_power_input: String::new(),
            profile_user_verified: None,
            profile_confirm: None,
            profile_presence: None,
            forward_search: String::new(),
            invite_search: String::new(),
            dm_search: String::new(),
            dm_results: Vec::new(),
            invite_results: Vec::new(),
            invite_target_id: None,
            space_detail_rooms: None,
            space_detail_members: Vec::new(),
            space_perms: Default::default(),
            space_tab: SpaceTab::Rooms,
            space_filter: String::new(),
            space_adding: false,
            space_confirm_remove: None,
            space_busy: HashSet::new(),
            space_name_input: String::new(),
            space_topic_input: String::new(),
            space_status: None,
            tick_count: 0,
            row_heights: Default::default(),
            messages_viewport_h: 700.0,
            virt_anchor: Default::default(),
            utd_hints: HashMap::new(),
            utd_checked: HashSet::new(),
            body_cache: Default::default(),
            pending_redactions: HashMap::new(),
            messages_at_bottom: true,
            last_scroll_offset: 0.0,
            paginating: false,
            paginate_msg_count: 0,
            gif_frames_cache: HashMap::new(),
            gif_results: Vec::new(),
            gif_searching: false,
            gif_preview_cache: HashMap::new(),
            pinned_messages: Vec::new(),
            thread_messages: Vec::new(),
            settings_avatar_data: None,
            settings_avatar_handle: None,
            settings_banner_data: None,
            settings_banner_handle: None,
            settings_banner_offset: 0.5,
            settings_banner_dimensions: None,
            settings_old_password: String::new(),
            settings_new_password: String::new(),
            settings_confirm_password: String::new(),
            security_state: None,
            security_recovery_key_input: String::new(),
            security_shown_recovery_key: None,
            security_reset_confirm: false,
            security_reset_password: String::new(),
            security_resetting: false,
            expanded_previews: HashSet::new(),
            calls_supported: false,
            call_state: fluxchat_core::rtc::CallConnectionState::Disconnected,
            call_room_id: None,
            call_participants: Vec::new(),
            rooms_with_calls: HashSet::new(),
            call_mic_muted: true,
            call_disconnect_tx: None,
            call_event_tx,
        }
    }
}
