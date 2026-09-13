use std::collections::HashMap;
use fluxchat_core::{MemberInfo, MessageInfo, RoomInfo, VerificationInfo};
use fluxchat_core::auth::LoginCapabilities;
use fluxchat_core::rooms::SpaceNode;

/// Top-level message enum for the Iced Elm architecture.
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
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
    /// Startup found no saved session — show the login form.
    NoSavedSession,
    /// The window was resized (logical width, height).
    WindowResized(f32, f32),
    /// The user clicked a desktop notification for this room.
    NotificationClicked(String),
    /// Diagnostics for undecryptable sessions: (session_id, hint).
    UtdHintsResolved(Vec<(String, String)>),
    Logout,

    // --- Post-login startup ---
    SyncStarted,
    SyncReconnected,
    InitialRoomsLoaded(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<SpaceNode>, HashMap<String, Vec<String>>), // rooms, spaces, invited, knocked, space_tree, space_descendants

    // --- Sync events (from subscription) ---
    SyncEvent(SyncEvent),

    // --- Room navigation ---
    SelectRoom(String),
    SelectSpace(Option<String>),
    SwitchTab(usize), // Alt+1-9: Nth room in sidebar order
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
    SelectionStarted(crate::selectable_text::SelectionWidgetId, usize, usize),
    SelectionMoved(crate::selectable_text::SelectionWidgetId, usize, usize),
    SelectionEnded,
    CopySelection,
    ClearInput,
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
    RefreshRoomList,
    InvitesRefreshed(Vec<fluxchat_core::RoomInfo>),
    ToggleCreateMenu,
    DmSearchChanged(String),
    DmSearchSubmit,
    DmSearchResults(Vec<fluxchat_core::rooms::UserSearchResult>),
    OpenContextMenu(super::state::ContextMenu),
    CloseContextMenu,
    ContextMenuAction(String, String), // action, target_id (room_id or space_id)

    // --- Modals ---
    OpenModal(super::state::Modal),
    CloseModal,

    // --- Settings/prefs ---
    SetTimestampFormat(String),
    ToggleCompactMode,
    ToggleJoinPart,
    SetZoomLevel(u32),
    SetMsgFontSize(f32),
    SetMsgFontFamily(String),
    SetSidebarFontFamily(String),
    SetTheme(String), // theme name

    // --- Notifications ---
    SetNotificationsEnabled(bool),
    SetNotifyMentionsOnly(bool),
    SetNotifyShowPreview(bool),
    SetNotifySound(bool),
    WindowFocusChanged(bool),

    // --- Room commands (async) ---
    JoinRoom(String),                           // room_id or alias
    RoomCreated(String, String, bool, bool, Option<String>), // room_id, name, encrypted, is_space, space_id
    LeaveRoom(String),                          // room_id
    InviteUser(String, String),                 // room_id, user_id
    SpaceInvite(String),                        // space_id — opens invite modal targeting space
    SpaceDetailsLoaded(Vec<fluxchat_core::RoomInfo>, Vec<fluxchat_core::MemberInfo>),
    KickUser(String, String, Option<String>),   // room_id, user_id, reason
    BanUser(String, String, Option<String>),    // room_id, user_id, reason
    UnbanUser(String),                          // user_id or display name
    SetTopic(String, String),                   // room_id, topic
    SetRoomName(String),                        // new room name
    SendHtml(String),                           // raw HTML to send
    SendPlain(String),                          // plain text (no markdown)
    UpgradeRoom(String),                        // room version
    DevToolsLoaded(Vec<(String, String)>),      // state events: (type, json)
    DevToolsTabChanged(String),                 // "state" or "send"
    DevToolsSendTypeChanged(String),
    DevToolsSendStateKeyChanged(String),
    DevToolsSendBodyChanged(String),
    DevToolsToggleIsState,
    DevToolsSend,
    DirectorySearch,
    DirectorySearchChanged(String),
    DirectoryServerChanged(String),
    DirectoryResults(Vec<fluxchat_core::rooms::PublicRoomResult>, Option<String>), // results, next_batch
    DirectoryLoadMore,
    DirectoryJoinRoom(String), // room_id
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
    RotateRoomKeys(String), // room_id — discard outbound megolm session to force reshare
    RotateRoomKeysResult(Result<String, String>), // room_id or error
    SecurityLoadState,
    SecurityStateLoaded(crate::state::SecurityState),
    SecurityEnableBackup,
    SecurityBackupEnabled(Option<String>), // recovery key if full recovery was set up
    SecurityRecoveryKeyInputChanged(String),
    SecurityRecover,
    SecurityRecoverResult(Result<(), String>),
    // Reset-identity flow (throw away existing backup + cross-signing, start fresh)
    SecurityResetIdentityConfirm,                        // first click: arm destructive action
    SecurityResetPasswordChanged(String),
    SecurityResetIdentityCancel,
    SecurityResetIdentitySubmit,                         // user entered password + confirmed
    SecurityResetIdentityResult(Result<Option<String>, String>), // new recovery key or error

    // --- Verification ---
    VerificationAccept(String, String),  // user_id, flow_id
    VerificationConfirm(String, String), // user_id, flow_id
    VerificationCancel(String, String),  // user_id, flow_id

    // --- Avatars ---
    AvatarsResolved(Vec<(String, Option<Vec<u8>>)>), // (mxc_uri, image bytes or None if the fetch failed)

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

    // --- Invites ---
    AcceptInvite(String), // room_id
    RejectInvite(String), // room_id
    KnockRoom(String, Option<String>), // room_id_or_alias, reason
    CancelKnock(String), // room_id
    SendLocation(String), // geo_uri (e.g. "geo:51.5074,-0.1278")
    PlayAudio(String, String), // event_id, room_id
    StopAudio,
    DownloadFile(String, String), // event_id, room_id
    PlayVideo(String, String),    // event_id, room_id
    VideoReady(String, std::path::PathBuf), // event_id, temp file path
    VideoTogglePause,
    VideoSeek(f64),               // position in seconds
    VideoSeekRelease,
    VideoNewFrame,
    VideoEndOfStream,
    VideoClose,
    VideoThumbResolved(String, Vec<u8>, u32, u32), // thumb_key, png_data, width, height
    AudioFinished,
    StartVoiceRecording,
    StopVoiceRecording,
    CancelVoiceRecording,
    VoiceRecordingReady(Vec<u8>, u64), // ogg_bytes, duration_ms
    SendVoiceMessage,
    DiscardVoiceMessage,
    PreviewVoiceMessage,

    // --- Navigation ---
    JumpToEvent(String), // event_id — scroll to and highlight

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

    // --- Room creation ---
    CreateRoomNameChanged(String),
    CreateRoomTopicChanged(String),
    CreateRoomTogglePublic,
    CreateRoomToggleEncrypted,
    CreateRoomToggleSpace,
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
    RoomAvatarUpdated(String, Option<String>), // room_id, new mxc url

    // --- Forward ---
    ForwardSearchChanged(String),
    ForwardToRoom(String), // target room_id

    // --- Profile actions ---
    ToggleIgnoreUser(String), // user_id
    SendDmToUser(String),     // user_id
    DmCreated(Box<fluxchat_core::RoomInfo>),
    StartVerification(String),          // user_id — interactive SAS
    StartDeviceVerification(String),    // device_id — verify specific session
    ManualVerifyUser(String),           // user_id — force-trust master key
    ManualVerifyResult(String, Result<(), String>), // user_id, result
    UserVerifiedStatus(String, bool),  // user_id, is_verified
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

    // --- Diagnostic: drop + rebuild Timeline for current room ---
    RefreshTimeline,

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
    /// Deleting a message failed: (room_id, event_id).
    RedactionFailed(String, String),

    // --- Sidebar resize ---
    SidebarDragStart,
    SidebarDragEnd,
    CursorMoved(f32, f32), // x, y

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
    /// A message the server's push rules say should notify.
    Notify(NotificationInfo),
    RoomListUpdate(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<SpaceNode>, HashMap<String, Vec<String>>), // rooms, spaces, invited, knocked, space_tree, space_descendants
    MessagesUpdated(String, Vec<MessageInfo>), // room_id, messages
    MembersUpdated(String, Vec<MemberInfo>),   // room_id, members
    TypingUpdate(String, Vec<String>),         // room_id, user_ids
    VerificationRequest(VerificationInfo),
    SyncLag(u64), // ms
}

/// A message that matched the user's push rules (notify / highlight).
#[derive(Debug, Clone)]
pub struct NotificationInfo {
    pub room_id: String,
    pub room_name: String,
    pub sender_name: String,
    pub body: String,
    pub is_mention: bool,
}
