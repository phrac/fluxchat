use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RoomInfo {
    pub room_id: String,
    pub name: String,
    pub is_encrypted: bool,
    pub is_space: bool,
    pub is_direct: bool,
    pub unread_count: u64,
    pub highlight_count: u64,
    pub avatar_url: Option<String>,
    pub topic: Option<String>,
    pub space_name: Option<String>,
    pub space_id: Option<String>,
    pub is_favourite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    pub poll: Option<PollInfo>,
    pub geo_uri: Option<String>,
    pub audio_mxc: Option<String>,
    pub duration_ms: Option<u64>,
    pub file_mxc: Option<String>,    // non-image file attachment MXC
    pub file_mimetype: Option<String>,
    pub file_size: Option<u64>,
    pub video_mxc: Option<String>,   // video source MXC (separate from thumbnail in image_mxc)
    #[serde(default)]
    pub is_emote: bool,
    #[serde(default)]
    pub is_video: bool,
    #[serde(default)]
    pub is_voice: bool,
    /// Matrix user IDs whose read marker points at this event, as reported
    /// by the SDK timeline item. Authoritative; replaces polled receipts.
    #[serde(default)]
    pub read_receipt_user_ids: Vec<String>,
    /// Set when the event couldn't be decrypted, for diagnostics in the UI.
    #[serde(default)]
    pub utd: Option<UtdInfo>,
    /// The message has been edited (SDK applies the latest edit to `body`).
    #[serde(default)]
    pub is_edited: bool,
    /// Local echo state; `None` once the server has accepted the event.
    #[serde(default)]
    pub send_state: Option<SendState>,
}

/// Delivery state of a message we sent.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SendState {
    Sending,
    Failed { recoverable: bool },
}

impl MessageInfo {
    /// Synthetic rows we generate (membership/state changes, date dividers).
    pub fn is_system(&self) -> bool {
        self.event_id.starts_with("~state-") || self.event_id.starts_with("~date-")
    }

    /// A day separator; `date_divider_ts()` gives the day's timestamp.
    pub fn is_date_divider(&self) -> bool {
        self.event_id.starts_with("~date-")
    }

    /// A message we sent that has no server event ID yet.
    pub fn is_local_echo(&self) -> bool {
        self.event_id.starts_with("~local-")
    }
}

/// Why an event couldn't be decrypted.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct UtdInfo {
    pub session_id: String,
    /// Curve25519 key the sender claims to have used (from the event; unverified).
    pub sender_key: Option<String>,
    pub device_id: Option<String>,
    /// SDK `UtdCause` as a string, e.g. "Unknown", "WithheldBySender".
    pub cause: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PollInfo {
    pub question: String,
    pub answers: Vec<(String, String)>,  // (answer_id, answer_text)
    pub votes: std::collections::HashMap<String, Vec<String>>,  // answer_id -> list of user_ids who voted
    pub is_ended: bool,
    pub total_votes: u32,
    pub my_vote: Option<String>,  // answer_id the current user voted for
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
