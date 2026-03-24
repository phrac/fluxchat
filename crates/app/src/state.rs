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
    pub files_open: Signal<bool>,
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
    pub highlight_words: Signal<Vec<String>>,

    pub timestamp_format: Signal<String>,  // "short" (HH:MM), "full" (HH:MM:SS), "off" (hidden)
    pub compact_mode: Signal<bool>,
    pub show_joinpart: Signal<bool>,

    pub settings_tab: Signal<String>,  // "account", "sessions"
    pub loading: Signal<bool>,
    pub sync_lag_ms: Signal<u64>,

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

/// Escape a string for safe inclusion in HTML content and attributes.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#39;")
}

/// Syntax-highlight code blocks in HTML using syntect.
/// Finds `<pre><code class="language-xxx">...</code></pre>` and replaces the content with highlighted HTML.
/// Also handles `<pre><code>...</code></pre>` without a language class (auto-detects or uses plain text).
pub fn highlight_code_blocks(html: &str) -> String {
    use syntect::highlighting::ThemeSet;
    use syntect::html::highlighted_html_for_string;
    use syntect::parsing::SyntaxSet;
    use std::sync::LazyLock;

    static SS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
    static TS: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

    let mut result = String::with_capacity(html.len());
    let mut remaining = html;

    while let Some(pre_start) = remaining.find("<pre><code") {
        result.push_str(&remaining[..pre_start]);
        let after_pre = &remaining[pre_start..];

        // Extract language class if present
        let (lang, code_start) = if after_pre.starts_with("<pre><code class=\"language-") {
            let class_start = "<pre><code class=\"language-".len();
            if let Some(quote_end) = after_pre[class_start..].find('"') {
                let lang = &after_pre[class_start..class_start + quote_end];
                // Skip past the closing >
                let tag_end = class_start + quote_end + 2; // ">"
                (Some(lang.to_string()), tag_end)
            } else {
                (None, "<pre><code>".len())
            }
        } else if after_pre.starts_with("<pre><code>") {
            (None, "<pre><code>".len())
        } else {
            // Not a pattern we handle — skip
            result.push_str("<pre><code");
            remaining = &after_pre["<pre><code".len()..];
            continue;
        };

        // Find closing </code></pre>
        let code_content = &after_pre[code_start..];
        if let Some(end_pos) = code_content.find("</code></pre>") {
            let raw_code = &code_content[..end_pos];
            // Unescape HTML entities and convert <br> to newlines
            let code = raw_code
                .replace("<br>", "\n")
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace("\n\n", "\n")
                .trim_matches('\n')
                .to_string();

            // Find syntax by language or auto-detect
            let syntax = lang.as_ref()
                .and_then(|l| SS.find_syntax_by_token(l))
                .unwrap_or_else(|| SS.find_syntax_plain_text());

            let theme = &TS.themes["base16-ocean.dark"];

            match highlighted_html_for_string(&code, &SS, syntax, theme) {
                Ok(highlighted) => {
                    // syntect wraps in <pre style="..."> — extract just the inner content
                    // and wrap in our own <pre><code>
                    let inner = if let Some(start) = highlighted.find('>') {
                        let after = &highlighted[start + 1..];
                        if let Some(end) = after.rfind("</pre>") {
                            after[..end].trim_matches('\n')
                        } else {
                            after.trim_matches('\n')
                        }
                    } else {
                        highlighted.trim_matches('\n')
                    };
                    result.push_str("<pre><code>");
                    result.push_str(inner);
                    result.push_str("</code></pre>");
                }
                Err(_) => {
                    // Fallback: keep original
                    result.push_str(&after_pre[..code_start + end_pos + "</code></pre>".len()]);
                }
            }
            remaining = &code_content[end_pos + "</code></pre>".len()..];
        } else {
            // No closing tag found — keep as-is
            result.push_str(after_pre);
            remaining = "";
        }
    }
    result.push_str(remaining);
    result
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
                    let escaped_uid = html_escape(&user_id);
                    let escaped_name = html_escape(display_name);
                    let avatar_html = if let Some(data_uri) = avatar_cache.get(&user_id) {
                        let escaped_uri = html_escape(data_uri);
                        format!(r#"<img src="{escaped_uri}" style="width:14px;height:14px;border-radius:50%;object-fit:cover;vertical-align:middle;">"#)
                    } else {
                        let initial = display_name.chars().next()
                            .unwrap_or('?').to_uppercase().to_string();
                        let escaped_initial = html_escape(&initial);
                        format!(r#"<span class="mention-avatar nick-{color_idx}">{escaped_initial}</span>"#)
                    };

                    result.push_str(&format!(
                        r#"<span class="mention-pill" data-userid="{escaped_uid}" onclick="if(window.__fluxchat_dioxus)window.__fluxchat_dioxus.send(this.dataset.userid);">{avatar_html}{escaped_name}</span>"#
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
            files_open: Signal::new(false),
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
            highlight_words: Signal::new(Vec::new()),
            timestamp_format: Signal::new("short".into()),
            compact_mode: Signal::new(false),
            show_joinpart: Signal::new(true),
            settings_tab: Signal::new("account".into()),
            loading: Signal::new(true),
            sync_lag_ms: Signal::new(0),
            _avatar_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            _avatar_version: Signal::new(0),
            open_tabs: Signal::new(Vec::new()),
        }
    }
}
