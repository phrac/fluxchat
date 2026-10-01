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
    /// Login types for a homeserver; carries the homeserver string the lookup
    /// was for so stale results (user kept typing) can be ignored.
    LoginCapsLoaded(String, Option<LoginCapabilities>),
    SessionRestored(LoginSuccess),
    /// Startup found no saved session — show the login form.
    NoSavedSession,
    /// The window was resized (logical width, height).
    WindowResized(f32, f32),
    /// The user clicked a desktop notification for this room (only
    /// freedesktop backends report clicks).
    #[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
    NotificationClicked(String),
    /// Re-enable the send queue and retry failed sends in a room.
    RetrySend(String),
    /// Sync connection status for the status bar (None = healthy).
    SyncStatus(Option<String>),
    /// Alt+Up/Down: move to the previous/next room in sidebar order.
    SwitchRoomRelative(i32),
    /// Alt+Shift+Up/Down: previous/next room with unread messages.
    SwitchUnread(i32),
    /// Accept the Nth `:shortcode` emoji completion.
    CompleteEmoji(usize),
    /// Ctrl+V: if the clipboard holds an image, stage it as an upload.
    PasteClipboardImage,
    /// Show a failure to the user (in the help/notice line above the input).
    ShowError(String),
    /// Open the "leave room?" confirmation for a room.
    AskLeaveRoom(String),
    /// Raw event JSON fetched for "View source": (event_id, json).
    EventSourceLoaded(String, String),
    /// Scroll the message list by a fraction of the viewport (+down / -up).
    ScrollMessagesBy(f32),
    /// Member list filter text changed.
    MemberFilterChanged(String),
    /// Lightbox: save the image (by MXC URI) via a file dialog.
    SaveImage(String),
    /// Lightbox: copy the image (by MXC URI) to the clipboard.
    CopyImage(String),
    /// Server's max upload size in bytes (from /media/v1/config), if known.
    MaxUploadSizeLoaded(Option<u64>),
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
    OpenThread(String, String), // room_id, thread_root_event_id — opens the thread pane
    /// Show a room in a pane beside the focused one (split view).
    OpenInPane(String),
    // --- Panes & windows (tmux-style, panes.rs) ---
    /// Mouse pressed in a pane. Focus moves on release (see `pending_pane_focus`).
    PaneClicked(iced::widget::pane_grid::Pane),
    /// Focus a pane right away (title-bar controls).
    PaneFocus(iced::widget::pane_grid::Pane),
    /// An action from a side pane's message row: focus the pane showing
    /// the view key, then run the action there.
    InView(String, Box<Message>),
    /// Show a room in a given pane (an empty pane's picker).
    FillPane(iced::widget::pane_grid::Pane, String),
    PaneResized(iced::widget::pane_grid::ResizeEvent),
    PaneDragged(iced::widget::pane_grid::DragEvent),
    PaneSplit(iced::widget::pane_grid::Axis),
    /// Close a pane (`None` = the focused one).
    PaneClose(Option<iced::widget::pane_grid::Pane>),
    PaneZoom,
    PaneFocusDirection(iced::widget::pane_grid::Direction),
    PaneCycle,
    WindowNew,
    WindowSelect(usize),
    WindowNext,
    WindowPrev,
    WindowClose,
    /// Composer of a non-focused pane (view key).
    PaneInputAction(String, iced::widget::text_editor::Action),
    PaneSend(String),
    /// Ctrl+B pressed; the next key is a pane command.
    TmuxPrefix,
    /// The window's close button / shortcut: tidy up, then exit.
    CloseRequested,
    TmuxKey(String),
    ShowTmuxHelp,
    /// Server thread list for a room: (room_id, for the context menu?, threads).
    ThreadListLoaded(String, bool, Vec<fluxchat_core::ThreadPreview>),
    /// "All threads" from a room's menu: open the room with its threads panel.
    ShowRoomThreads(String),
    /// Sidebar thread lists fetched at startup (room → top threads).
    RoomThreadsLoaded(Vec<(String, Vec<fluxchat_core::ThreadPreview>)>),
    /// Collapse / expand a room's threads in the sidebar.
    ToggleRoomThreads(String),

    // --- UI panels ---
    ToggleMemberList,
    ToggleSearch,
    TogglePins,
    ToggleThreads,
    /// Open the media gallery for the current view on a tab (header button,
    /// Ctrl/Cmd+Shift+G, `/gallery`, `/files`).
    OpenGallery(crate::state::GalleryTab),
    GalleryTabChanged(crate::state::GalleryTab),
    /// A page of server media arrived: (view key, page or error).
    GalleryPageLoaded(String, Result<fluxchat_core::gallery::MediaPage, String>),
    /// The grid scrolled: (offset, viewport height).
    GalleryScrolled(f32, f32),
    /// "Load older" (after automatic loading stopped at its cap).
    GalleryLoadMore,
    /// Open a gallery item (image → lightbox, video → its message).
    GalleryOpen(String), // event_id
    /// Lightbox opened from the gallery: previous (-1) / next (+1) image.
    GalleryStep(i32),
    /// Close the gallery and go to the item's message.
    GalleryJump(String), // event_id
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
    /// Collapse / expand the room list (Ctrl/Cmd+[).
    ToggleRoomList,
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

    /// Replace the preferences (Settings → Preferences) and persist them.
    SetPrefs(Box<crate::prefs::Prefs>),
    AudioDevicesLoaded(Vec<String>, Vec<String>), // inputs, outputs
    ToggleMicTest,
    MicLevelTick,
    // --- Notification rules (server push rules) ---
    NotifyRulesLoaded(Result<fluxchat_core::notify_rules::NotifyRules, String>),
    SetDefaultNotify(bool, fluxchat_core::notify_rules::DefaultLevel), // dm?, level
    SetNotifyToggle(fluxchat_core::notify_rules::NotifyToggle, bool),
    KeywordInputChanged(String),
    AddKeyword,
    RemoveKeyword(String),
    /// A rule change finished (error).
    NotifyRuleDone(Option<String>),
    SetCompactMode(bool),
    SetShowJoinPart(bool),
    /// Handle several messages in order (for task callbacks that need more
    /// than one follow-up).
    Batch(Vec<Message>),
    RoomNotifyModeLoaded(String, String),       // room_id, mode
    RoomParentSpacesLoaded(String, Vec<String>, Vec<String>), // room_id, parent space_ids, manageable space_ids
    RoomSettingsPickSpace(String),              // space_id chosen in "Add to space"
    ReloadRoomSettingsExtras,
    // --- Room settings: security & roles ---
    RoomSettingsTabChanged(crate::state::RoomSettingsTab),
    RoomSecurityLoaded(String, Result<fluxchat_core::room_admin::RoomSecurity, String>),
    RoomPermsLoaded(String, Result<(fluxchat_core::room_admin::RoomPermissions, Vec<fluxchat_core::room_admin::BannedUser>), String>),
    SetJoinMode(fluxchat_core::room_admin::JoinMode),
    SetHistoryMode(fluxchat_core::room_admin::HistoryMode),
    SetGuestAccess(bool),
    SetInDirectory(bool),
    SetPermission(fluxchat_core::room_admin::Permission, i64),
    /// A security/permission change finished (room_id, error).
    RoomAdminDone(String, Option<String>),
    // --- Room settings: advanced ---
    RoomAdvancedLoaded(String, Result<Box<fluxchat_core::room_admin::RoomAdvanced>, String>),
    RoomAliasInputChanged(String),
    RoomBridgesLoaded(String, Vec<fluxchat_core::room_admin::BridgeInfo>),
    AddRoomAlias,
    RemoveRoomAlias(String),
    /// Make an address the main one (None: no main address).
    SetMainAlias(Option<String>),
    /// Publish or unpublish an address as an alternative.
    SetAliasPublished(String, bool),
    /// Ask for (true) or cancel (false) the Upgrade confirmation.
    ConfirmRoomUpgrade(bool),
    /// An upgrade finished: (old room, new room or error).
    RoomUpgraded(String, Result<String, String>),
    SetRoomNotifyMode(String),                  // mode for the active room
    // --- Notifications ---
    SetNotificationsEnabled(bool),
    SetNotifyMentionsOnly(bool),
    SetNotifyShowPreview(bool),
    SetNotifySound(bool),
    WindowFocusChanged(bool),

    // --- Room commands (async) ---
    JoinRoom(String),                           // room_id or alias
    /// Join through specific servers (room link `via=`).
    JoinRoomVia(String, Vec<String>),
    /// A matrix.to link was clicked: (room id / alias / user id, event id, via servers).
    OpenMatrixLink(String, Option<String>, Vec<String>),
    RoomCreated(String, String, bool, bool, Option<String>), // room_id, name, encrypted, is_space, space_id
    LeaveRoom(String),                          // room_id
    InviteUser(String, String),                 // room_id, user_id
    SpaceInvite(String),                        // space_id — opens invite modal targeting space
    // --- Space dialog ---
    OpenSpaceDetails(String),                   // space_id
    /// Children, members and our permissions for the open space dialog.
    SpaceDetailsLoaded(
        String,
        Result<Vec<fluxchat_core::spaces::SpaceChild>, String>,
        Vec<fluxchat_core::MemberInfo>,
        fluxchat_core::spaces::SpacePermissions,
    ),
    SpaceTabChanged(crate::state::SpaceTab),
    SpaceFilterChanged(String),
    SpaceToggleAdding,
    SpaceAddChild(String, String),              // space_id, room_id
    SpaceAskRemove(Option<String>),             // child room_id awaiting confirm
    SpaceRemoveChild(String, String),           // space_id, room_id
    SpaceJoinChild(String, String),             // space_id, room_id
    /// An add/remove/join finished: (space_id, room_id, error).
    SpaceChildOpDone(String, String, Option<String>),
    SpaceNameChanged(String),
    SpaceTopicChanged(String),
    SpaceSaveDetails(String),                   // space_id
    SpaceUploadAvatar(String),                  // space_id
    SpaceRemoveAvatar(String),                  // space_id
    SpaceDetailsSaved(String, Option<String>),  // space_id, error
    SpaceCreateRoom(String),                    // space_id
    SpaceOpenChild(String),                     // room_id — close dialog, select room
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
    UploadFailed(String),
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
    GifFramesResolved(String, crate::anim::Anim), // (mxc_uri, frames)

    // --- URL Previews ---
    UrlPreviewFetched(String, Option<fluxchat_core::UrlPreview>), // url, preview
    UrlPreviewImageResolved(String, Vec<u8>), // image_url, bytes

    // --- Message hover ---
    HoverMessage(Option<String>), // event_id or None
    /// The cursor left a message row (`Some`) or the message list (`None`).
    /// Ignored while it's on that message's action bar.
    HoverLeft(Option<String>),
    /// The cursor entered (`Some`) or left (`None`) a message's action bar.
    HoverBar(Option<String>),
    TogglePreviewExpand(String),  // event_id

    // --- Formatting ---
    InsertFormat(&'static str, &'static str), // (prefix, suffix) e.g. ("**", "**")

    // --- Overflow menu actions ---
    ToggleFavourite,

    // --- Invites ---
    AcceptInvite(String), // room_id
    RejectInvite(String), // room_id
    AcceptAllInvites,
    /// We put a room in a space: (room_id, space_id). Shown there at once.
    SpaceLinked(String, String),
    /// Ask for (true) or cancel (false) the "Decline all" confirmation.
    ArmDeclineAllInvites(bool),
    DeclineAllInvites,
    /// A bulk accept/decline finished: rooms that failed, with the error.
    InvitesBulkDone(Vec<(String, String)>),
    /// Live join requests of a room (from its watcher).
    JoinRequestsUpdated(String, fluxchat_core::room_admin::JoinRequests),
    /// A room's join-request watcher is running (dropping the guard stops it).
    JoinRequestsWatching(String, crate::state::TaskGuard),
    ToggleJoinRequests,
    RespondJoinRequest(String, fluxchat_core::room_admin::JoinResponse), // user_id
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
    /// Scroll the open room to its read anchor (first unread near the top).
    ScrollToReadAnchor,

    // --- Code blocks ---
    ToggleCodeBlock(String, usize), // event_id, code block index in the message

    // --- Reporting ---
    ReportReasonChanged(String),
    SubmitReport,

    // --- Quick switcher ---
    ToggleQuickSwitcher,
    /// Run a command-palette row (room or action).
    PaletteActivate(Box<crate::state::PaletteItem>),
    /// Mentions inbox (all mentions / keyword hits across rooms).
    ToggleMentions,
    MentionsLoaded(Vec<fluxchat_core::MentionItem>),
    /// Open a mention: select its room and jump to the message.
    OpenMention(String, String), // room_id, event_id
    QuickSwitcherQuery(String),
    QuickSwitcherSelect,
    QuickSwitcherUp,
    QuickSwitcherDown,
    ToggleLocationPicker,
    LocationLatChanged(String),
    LocationLonChanged(String),
    LocationSubmit,

    // --- Panels data ---
    PinsLoaded(String, Vec<fluxchat_core::MessageInfo>), // room_id, pins

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
    /// Room icon file chosen: (room_id, mime, bytes).
    RoomAvatarPicked(String, &'static str, Vec<u8>),
    /// Room icon upload finished: (room_id, Ok((mxc, bytes)) or error).
    RoomAvatarUploaded(String, Result<(String, Vec<u8>), String>),
    RoomSettingsRemoveAvatar,
    RoomAvatarUpdated(String, Option<String>), // room_id, new mxc url

    // --- Forward ---
    ForwardSearchChanged(String),
    ForwardToRoom(String), // target room_id

    // --- Profile actions ---
    ToggleIgnoreUser(String), // user_id
    IgnoredUsersLoaded(Vec<String>),
    MediaPreviewsLoaded(fluxchat_core::profile::MediaPreviewConfig),
    SetMediaPreviews(fluxchat_core::profile::MediaPreviewConfig),
    /// Show one hidden image/video: (view key, event id).
    RevealMedia(String, String),
    /// An ignore-list change finished: (user, now ignored?, error).
    IgnoreDone(String, bool, Option<String>),
    SettingsDeactivatePasswordChanged(String),
    /// Ask for (true) or cancel (false) the deactivation confirmation.
    SettingsDeactivateArm(bool),
    SettingsDeactivate,
    DeactivateDone(Option<String>), // error
    SendDmToUser(String),     // user_id
    MentionUser(String),      // display name — insert "@name " into the input
    ProfileConfirm(Option<String>), // pending "kick" / "ban" on the profile card
    ProfilePresenceLoaded(String, Option<fluxchat_core::Presence>), // user_id, presence
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
    GifPreviewLoaded(String, crate::anim::Anim), // preview_url, animated frames
    GifSelect(String, String),                           // gif_url, description

    // --- Read receipts ---
    SendReadReceipt,
    /// Deleting a message failed: (room_id, event_id).
    RedactionFailed(String, String),

    // --- Sidebar resize ---
    /// Start dragging a panel divider.
    ResizeStart(crate::state::ResizeTarget),
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
    /// A back-pagination request finished: (view key, Some(reached the
    /// start of history) or None on error).
    PaginateBackDone(String, Option<bool>),
    /// Evaluated shortly after PaginateBackDone, once the new rows landed.
    PaginateSettled(String, Option<bool>),
    /// Scroll-up gesture over the timeline while it's already at the top.
    TimelineWheelUpAtTop,
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
    pub protection: fluxchat_core::vault::Protection,
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
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub is_mention: bool,
    pub event_id: String,
    /// Seconds since the epoch.
    pub timestamp: u64,
}
