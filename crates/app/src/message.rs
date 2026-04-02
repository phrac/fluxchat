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
    InitialRoomsLoaded(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>), // rooms, spaces, invited, knocked
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
    InputAction(iced::widget::text_editor::Action),
    SendMessage,
    StartReply(String, String, String), // event_id, sender_name, body
    CancelReply,
    StartEdit(String, String), // event_id, original_body
    CancelEdit,
    CompleteCommand(usize),  // index in cmd_completions
    CompleteMention(usize),  // index in mention_completions

    // --- Message actions ---
    CopyMessageText(String),       // body text to copy
    ToggleReaction(String, String), // event_id, emoji
    DeleteMessage(String),          // event_id
    PinMessage(String),             // event_id
    ForwardMessage(String, String, Option<String>), // body, formatted, sender

    // --- Search ---
    SearchQueryChanged(String),
    SearchSubmit,
    SearchResults(Vec<fluxchat_core::messages::SearchResult>),
    SearchSelectResult(usize),
    SearchArrowDown,
    SearchArrowUp,

    // --- Threads ---
    OpenThread(String, String), // room_id, thread_root_event_id

    // --- UI panels ---
    ToggleMemberList,
    ToggleSearch,
    TogglePins,
    ToggleThreads,
    ToggleFiles,
    ToggleNotices,
    ToggleGifs,
    ToggleEmojiPicker,
    OpenEmojiReact(String),        // event_id
    EmojiQueryChanged(String),
    EmojiCategoryChanged(u8),
    EmojiSelect(String),           // emoji character
    ToggleOverflowMenu,
    SetRightPanel(Option<super::state::RightPanel>),

    // --- Modals ---
    OpenModal(super::state::Modal),
    CloseModal,

    // --- Settings/prefs ---
    SetTimestampFormat(String),
    ToggleCompactMode,
    ToggleJoinPart,
    SetZoomLevel(u32),
    SetMsgFontSize(f32),

    // --- Notifications ---
    SetNotificationsEnabled(bool),
    SetNotifyMentionsOnly(bool),
    SetNotifyShowPreview(bool),
    SetNotifySound(bool),
    WindowFocusChanged(bool),

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
    FileDropped(std::path::PathBuf),
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
    SettingsPickBanner,           // open file dialog, load into preview
    SettingsBannerPicked(Vec<u8>), // raw image bytes from file picker
    SettingsSaveBanner,            // crop + upload the banner
    SettingsRemoveBanner,
    SettingsBannerOffsetChanged(f32),
    SettingsOldPasswordChanged(String),
    SettingsNewPasswordChanged(String),
    SettingsConfirmPasswordChanged(String),
    SettingsChangePassword,

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
    GifFramesResolved(String, std::sync::Arc<iced_gif::Frames>), // (mxc_uri, frames)

    // --- URL Previews ---
    UrlPreviewFetched(String, Option<fluxchat_core::UrlPreview>), // url, preview
    UrlPreviewImageResolved(String, Vec<u8>), // image_url, bytes

    // --- Message hover ---
    HoverMessage(Option<String>), // event_id or None
    TogglePreviewExpand(String),  // event_id

    // --- Formatting ---
    InsertFormat(&'static str, &'static str), // (prefix, suffix) e.g. ("**", "**")

    // --- Overflow menu actions ---
    ToggleFavourite,
    CopyRoomLink,

    // --- Invites ---
    AcceptInvite(String), // room_id
    RejectInvite(String), // room_id
    KnockRoom(String, Option<String>), // room_id_or_alias, reason
    CancelKnock(String), // room_id
    SendLocation(String), // geo_uri (e.g. "geo:51.5074,-0.1278")
    PlayAudio(String, String), // event_id, room_id
    StopAudio,
    AudioFinished,
    StartVoiceRecording,
    StopVoiceRecording,
    CancelVoiceRecording,
    VoiceRecordingReady(Vec<u8>, u64), // ogg_bytes, duration_ms
    SendVoiceMessage,
    DiscardVoiceMessage,
    PreviewVoiceMessage,

    // --- Code blocks ---
    ToggleCodeBlock(String), // event_id

    // --- Reporting ---
    ReportReasonChanged(String),
    SubmitReport,

    // --- Quick switcher ---
    ToggleQuickSwitcher,
    QuickSwitcherQuery(String),
    QuickSwitcherSelect,
    QuickSwitcherUp,
    QuickSwitcherDown,
    ToggleLocationPicker,
    LocationLatChanged(String),
    LocationLonChanged(String),
    LocationSubmit,

    // --- Panels data ---
    PinsLoaded(Vec<fluxchat_core::MessageInfo>),
    ThreadsLoaded(Vec<fluxchat_core::MessageInfo>),

    // --- Room creation ---
    CreateRoomNameChanged(String),
    CreateRoomTopicChanged(String),
    CreateRoomTogglePublic,
    CreateRoomToggleEncrypted,
    CreateRoomSelectSpace(Option<String>),
    CreateRoomSubmit,

    // --- Room settings ---
    RoomSettingsNameChanged(String),
    RoomSettingsTopicChanged(String),
    RoomSettingsSaveName,
    RoomSettingsSaveTopic,
    EnableEncryption(String), // room_id
    RoomSettingsUploadAvatar,
    RoomSettingsRemoveAvatar,

    // --- Forward ---
    ForwardSearchChanged(String),
    ForwardToRoom(String), // target room_id

    // --- Profile actions ---
    ToggleIgnoreUser(String), // user_id
    SendDmToUser(String),     // user_id
    ProfilePowerInputChanged(String),
    ProfileSetPowerLevel,     // apply power level from profile_power_input

    // --- Power levels ---
    SetPowerLevel(String, String, i64), // room_id, user_id, level

    // --- Invite ---
    InviteSearchChanged(String),
    InviteSearchSubmit,
    InviteSearchResults(Vec<fluxchat_core::rooms::UserSearchResult>),
    InviteUserFromModal(String), // user_id

    // --- Space management ---
    AddRoomToSpace(String),    // space_id (current room)
    RemoveRoomFromSpace(String), // space_id (current room)

    // --- Highlight words ---
    AddHighlight(String),
    RemoveHighlight(String),

    // --- Room notifications ---
    MuteRoom,
    UnmuteRoom,

    // --- Giphy ---
    SetGiphyKey(String),
    GifSearch(String),                                   // query
    GifQueryChanged(String),                             // search input
    GifSearchSubmit,                                     // trigger search
    GifResultsLoaded(Vec<fluxchat_core::gif::GifResult>),
    GifPreviewLoaded(String, std::sync::Arc<iced_gif::Frames>), // preview_url, animated frames
    GifSelect(String, String),                           // gif_url, description

    // --- Read receipts ---
    SendReadReceipt,
    ReadReceiptsLoaded(String, Vec<fluxchat_core::ReadReceipt>), // room_id, receipts

    // --- Sidebar resize ---
    SidebarDragStart,
    SidebarDragEnd,
    SidebarDrag(f32), // new x position

    // --- Scroll ---
    MessagesScrolled(iced::widget::scrollable::Viewport),
    ScrollToBottom,

    // --- Calls/RTC ---
    CallJoin(String),                                    // room_id
    CallLeave,
    CallJoinResult(Result<fluxchat_core::rtc::CallJoinInfo, String>),
    CallConnected,
    CallDisconnected(String),
    CallError(String),
    CallParticipantJoined(fluxchat_core::rtc::CallParticipant),
    CallParticipantLeft(String),                         // identity
    CallParticipantsUpdated(String, Vec<fluxchat_core::rtc::CallParticipant>),
    CallToggleMic,
    CallRoomsWithCallsUpdated(std::collections::HashSet<String>),

    // --- Misc ---
    PaginateBack(String),     // room_id
    PaginateBackDone,
    PaginateTimeout,
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
    RoomListUpdate(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>), // rooms, spaces, invited, knocked
    MessagesUpdated(String, Vec<MessageInfo>), // room_id, messages
    MembersUpdated(String, Vec<MemberInfo>),   // room_id, members
    TypingUpdate(String, Vec<String>),         // room_id, user_ids
    VerificationRequest(VerificationInfo),
    SyncLag(u64), // ms
}
