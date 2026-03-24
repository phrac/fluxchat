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

#[allow(dead_code)]
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
    pub invited_rooms: Signal<Vec<RoomInfo>>,
    pub selected_room_id: Signal<Option<String>>,
    pub selected_space_id: Signal<Option<String>>,

    pub messages_by_room: Signal<HashMap<String, Vec<MessageInfo>>>,
    pub members_by_room: Signal<HashMap<String, Vec<MemberInfo>>>,
    pub typing_by_room: Signal<HashMap<String, Vec<String>>>,
    pub receipts_by_room: Signal<HashMap<String, Vec<fluxchat_core::ReadReceipt>>>,
    pub profile_user_id: Signal<Option<String>>,

    // Message action state
    pub reply_to: Signal<Option<(String, String, String)>>, // (event_id, sender_name, body)
    pub edit_event: Signal<Option<(String, String)>>,       // (event_id, original_body)
    pub settings_open: Signal<bool>,
    pub right_panel: Signal<Option<RightPanel>>,  // which panel is open on the right
    pub show_member_list: Signal<bool>,
    pub search_open: Signal<bool>,
    pub pins_open: Signal<bool>,
    pub threads_open: Signal<bool>,
    pub verification_request: Signal<Option<fluxchat_core::VerificationInfo>>,
    pub read_rooms: Signal<std::collections::HashSet<String>>,  // rooms we've marked as read this session
    pub last_read_event: Signal<HashMap<String, String>>,  // room_id -> last event_id when room was read
    pub ignored_users: Signal<std::collections::HashSet<String>>,  // user IDs we're ignoring
    pub help_text: Signal<Option<String>>,  // shared help text for command results
    pub upload_pending: Signal<Option<(String, String, Vec<u8>)>>,  // (filename, mime_type, data)
    pub lightbox_url: Signal<Option<String>>,  // data URI for full-size image viewer
    pub forward_event: Signal<Option<(String, String, Option<String>)>>,  // (body, formatted_body, sender_name)
    pub create_room_open: Signal<bool>,
    pub room_settings_open: Signal<bool>,

    pub settings_tab: Signal<String>,  // "account", "sessions"
    pub loading: Signal<bool>,

    // Resolved avatar data URIs — shared between render and background tasks
    pub _avatar_cache: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>>,
    pub _avatar_version: Signal<u32>, // bump to trigger re-render

    // Channel tabs — recently visited rooms
    // (room_id, room_name, space_name, space_avatar_mxc)
    pub open_tabs: Signal<Vec<(String, String, Option<String>, Option<String>)>>,
}

/// Nick color index from a string hash.
pub fn nick_color_index(name: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in name.bytes() { hash = b as u32 + ((hash << 5).wrapping_sub(hash)); }
    hash % 15
}

/// Transform Matrix mention links into styled mention pills.
/// `avatar_cache` maps user_id -> resolved data URI for avatars.
pub fn transform_mentions(html: &str, avatar_cache: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(html.len());
    let mut remaining = html;
    let prefix = r#"<a href="https://matrix.to/#/"#;

    while let Some(start) = remaining.find(prefix) {
        result.push_str(&remaining[..start]);
        let after_prefix = &remaining[start + prefix.len()..];

        if let Some(quote_end) = after_prefix.find("\">") {
            let user_id_raw = &after_prefix[..quote_end];
            let user_id = user_id_raw.replace("%40", "@");
            let after_quote = &after_prefix[quote_end + 2..];

            if user_id.starts_with('@') {
                if let Some(end_tag) = after_quote.find("</a>") {
                    let display_name = &after_quote[..end_tag];
                    remaining = &after_quote[end_tag + 4..];

                    let color_idx = nick_color_index(display_name);

                    // Build avatar HTML — real image or colored initial
                    let avatar_html = if let Some(data_uri) = avatar_cache.get(&user_id) {
                        format!(r#"<img src="{data_uri}" style="width:14px;height:14px;border-radius:50%;object-fit:cover;vertical-align:middle;">"#)
                    } else {
                        let initial = display_name.chars().next()
                            .unwrap_or('?').to_uppercase().to_string();
                        format!(r#"<span class="mention-avatar nick-{color_idx}">{initial}</span>"#)
                    };

                    result.push_str(&format!(
                        r#"<span class="mention-pill" data-userid="{user_id}" onclick="if(window.__fluxchat_dioxus)window.__fluxchat_dioxus.send(this.dataset.userid);">{avatar_html}{display_name}</span>"#
                    ));
                    continue;
                }
            }
            // Not a user mention — keep original link
            result.push_str(prefix);
            remaining = after_prefix;
        } else {
            result.push_str(prefix);
            remaining = after_prefix;
        }
    }
    result.push_str(remaining);
    result
}

impl UiState {
    pub fn new() -> Self {
        Self {
            logged_in: Signal::new(false),
            user_id: Signal::new(String::new()),
            display_name: Signal::new(None),
            rooms: Signal::new(Vec::new()),
            spaces: Signal::new(Vec::new()),
            invited_rooms: Signal::new(Vec::new()),
            selected_room_id: Signal::new(None),
            selected_space_id: Signal::new(None),
            messages_by_room: Signal::new(HashMap::new()),
            members_by_room: Signal::new(HashMap::new()),
            typing_by_room: Signal::new(HashMap::new()),
            receipts_by_room: Signal::new(HashMap::new()),
            profile_user_id: Signal::new(None),
            reply_to: Signal::new(None),
            edit_event: Signal::new(None),
            settings_open: Signal::new(false),
            right_panel: Signal::new(None),
            show_member_list: Signal::new(true),
            search_open: Signal::new(false),
            pins_open: Signal::new(false),
            threads_open: Signal::new(false),
            verification_request: Signal::new(None),
            read_rooms: Signal::new(std::collections::HashSet::new()),
            last_read_event: Signal::new(HashMap::new()),
            ignored_users: Signal::new(std::collections::HashSet::new()),
            help_text: Signal::new(None),
            upload_pending: Signal::new(None),
            lightbox_url: Signal::new(None),
            forward_event: Signal::new(None),
            create_room_open: Signal::new(false),
            room_settings_open: Signal::new(false),
            settings_tab: Signal::new("account".into()),
            loading: Signal::new(true),
            _avatar_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            _avatar_version: Signal::new(0),
            open_tabs: Signal::new(Vec::new()),
        }
    }
}
