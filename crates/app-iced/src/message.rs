use fluxchat_core::{MemberInfo, MessageInfo, RoomInfo, VerificationInfo};
use fluxchat_core::auth::LoginCapabilities;

/// Top-level message enum for the Iced Elm architecture.
#[derive(Debug, Clone)]
pub enum Message {
    // --- Auth ---
    LoginHomeserverChanged(String),
    LoginUsernameChanged(String),
    LoginPasswordChanged(String),
    LoginSubmit,
    LoginSso,
    LoginResult(Result<LoginSuccess, String>),
    LoginCapsLoaded(Option<LoginCapabilities>),
    SessionRestored(LoginSuccess),
    Logout,

    // --- Post-login startup ---
    SyncStarted,
    InitialRoomsLoaded(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>), // rooms, spaces, invited
    TimelineLoaded {
        room_id: String,
        messages: Vec<MessageInfo>,
        members: Option<Vec<MemberInfo>>,
    },

    // --- Sync events (from subscription) ---
    SyncEvent(SyncEvent),

    // --- Room navigation ---
    SelectRoom(String),
    SelectSpace(Option<String>),
    CloseTab(String),
    SwitchTab(usize), // Alt+1-9
    ToggleSection(String), // collapse/expand room list section

    // --- Message input ---
    InputChanged(String),
    SendMessage,
    StartReply(String, String, String), // event_id, sender_name, body
    CancelReply,
    StartEdit(String, String), // event_id, original_body
    CancelEdit,
    CompleteCommand(usize),  // index in cmd_completions
    CompleteMention(usize),  // index in mention_completions

    // --- Message actions ---
    ToggleReaction(String, String), // event_id, emoji
    DeleteMessage(String),          // event_id
    PinMessage(String),             // event_id
    ForwardMessage(String, String, Option<String>), // body, formatted, sender

    // --- Search ---
    SearchQueryChanged(String),
    SearchSubmit,
    SearchResults(Vec<fluxchat_core::messages::SearchResult>),

    // --- UI panels ---
    ToggleMemberList,
    ToggleSearch,
    TogglePins,
    ToggleThreads,
    ToggleFiles,
    ToggleNotices,
    ToggleGifs,
    SetRightPanel(Option<super::state::RightPanel>),

    // --- Modals ---
    OpenModal(super::state::Modal),
    CloseModal,

    // --- Settings/prefs ---
    SetTimestampFormat(String),
    ToggleCompactMode,
    ToggleJoinPart,
    SetZoomLevel(u32),

    // --- Room commands (async) ---
    JoinRoom(String),                           // room_id or alias
    LeaveRoom(String),                          // room_id
    InviteUser(String, String),                 // room_id, user_id
    KickUser(String, String, Option<String>),   // room_id, user_id, reason
    BanUser(String, String, Option<String>),    // room_id, user_id, reason
    SetTopic(String, String),                   // room_id, topic
    SetNick(String),                            // new display name
    SendDm(String, String),                     // user_id, body

    // --- File upload ---
    PickFile,
    FileSelected(String, String, Vec<u8>), // filename, mime, data
    CancelUpload,
    UploadFile,                            // trigger actual upload

    // --- Settings ---
    SettingsNickChanged(String),
    SettingsSaveNick,
    SettingsLoadDevices,
    SettingsDevicesLoaded(Vec<fluxchat_core::devices::DeviceInfo>),
    SettingsSignOutDevice(String), // device_id
    SettingsSignOutPasswordChanged(String),
    SettingsSignOutConfirm,
    SettingsStatus(String),
    SettingsLoadProfile,
    SettingsProfileLoaded {
        avatar_data: Option<Vec<u8>>,
        banner_data: Option<Vec<u8>>,
    },
    SettingsUploadAvatar,
    SettingsRemoveAvatar,
    SettingsUploadBanner,
    SettingsRemoveBanner,

    // --- Security ---
    SecurityLoadState,
    SecurityStateLoaded(crate::state::SecurityState),
    SecurityEnableBackup,
    SecurityBackupEnabled(Option<String>), // recovery key if full recovery was set up
    SecurityRecoveryKeyInputChanged(String),
    SecurityRecover,
    SecurityRecoverResult(Result<(), String>),

    // --- Avatars ---
    AvatarsResolved(Vec<(String, Vec<u8>)>), // vec of (mxc_uri, image_bytes)

    // --- Inline images ---
    ImageResolved(String, Vec<u8>), // (mxc_uri, image_bytes)

    // --- URL Previews ---
    UrlPreviewFetched(String, Option<fluxchat_core::UrlPreview>), // url, preview
    UrlPreviewImageResolved(String, Vec<u8>), // image_url, bytes

    // --- Formatting ---
    InsertFormat(&'static str, &'static str), // (prefix, suffix) e.g. ("**", "**")

    // --- Misc ---
    PaginateBack(String), // room_id
    OpenUrl(String),
    Escape, // close modals/panels/completions
    Tick, // periodic refresh
    Noop,
}

/// Successful login info.
#[derive(Debug, Clone)]
pub struct LoginSuccess {
    pub user_id: String,
    pub display_name: Option<String>,
}

/// Events coming from the Matrix sync subscription.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    RoomListUpdate(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>), // rooms, spaces, invited
    MessagesUpdated(String, Vec<MessageInfo>), // room_id, messages
    MembersUpdated(String, Vec<MemberInfo>),   // room_id, members
    TypingUpdate(String, Vec<String>),         // room_id, user_ids
    VerificationRequest(VerificationInfo),
    SyncLag(u64), // ms
}
