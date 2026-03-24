use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReadReceipt {
    pub user_id: String,
    pub event_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RoomInfo {
    pub room_id: String,
    pub name: String,
    pub is_encrypted: bool,
    pub is_space: bool,
    pub is_direct: bool,
    pub unread_count: u64,
    pub avatar_url: Option<String>,
    pub topic: Option<String>,
    pub space_name: Option<String>,
    pub is_favourite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageInfo {
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub formatted_body: Option<String>,
    pub transformed_body: Option<String>, // Pre-computed HTML (e.g. mention pills)
    pub image_mxc: Option<String>,
    pub reactions: Vec<ReactionInfo>,
    pub reply_to_id: Option<String>,
    pub reply_to_sender: Option<String>,
    pub reply_to_body: Option<String>,
    pub timestamp: u64,
    pub avatar_url: Option<String>,
    pub thread_root: Option<String>,      // event is in a thread (root event_id)
    pub thread_count: Option<u32>,        // event is a thread root (reply count)
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReactionInfo {
    pub emoji: String,
    pub count: u32,
    pub users: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MemberInfo {
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub power_level: i64,
    pub role: String,
    pub presence: String,
    pub status_msg: Option<String>,
    pub server_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedMessages {
    pub messages: Vec<MessageInfo>,
    pub end_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginResult {
    pub user_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UrlPreview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub site_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VerificationInfo {
    pub flow_id: String,
    pub other_user_id: String,
    pub is_self_verification: bool,
    pub state: String,
    pub emojis: Option<Vec<EmojiInfo>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EmojiInfo {
    pub symbol: String,
    pub description: String,
}
