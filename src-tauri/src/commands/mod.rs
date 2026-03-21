pub mod auth;
pub mod media;
pub mod messages;
pub mod rooms;
pub mod spaces;
pub mod verification;

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
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
}

#[derive(Debug, Serialize, Clone)]
pub struct MessageInfo {
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub formatted_body: Option<String>,
    pub image_mxc: Option<String>,
    pub reactions: Vec<ReactionInfo>,
    pub reply_to_id: Option<String>,
    pub reply_to_sender: Option<String>,
    pub reply_to_body: Option<String>,
    pub timestamp: u64,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemberInfo {
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub power_level: i64,
    pub role: String,
    pub presence: String,
    pub server_name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReactionInfo {
    pub emoji: String,
    pub count: u32,
    pub users: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedMessages {
    pub messages: Vec<MessageInfo>,
    pub end_token: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("Matrix error: {0}")]
    Matrix(String),
    #[error("{0}")]
    Other(String),
}

impl From<matrix_sdk::Error> for AppError {
    fn from(e: matrix_sdk::Error) -> Self {
        AppError::Matrix(e.to_string())
    }
}

impl From<matrix_sdk::ClientBuildError> for AppError {
    fn from(e: matrix_sdk::ClientBuildError) -> Self {
        AppError::Matrix(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
