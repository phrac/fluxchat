# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI. Uses matrix-rust-sdk + matrix-sdk-ui for the Matrix protocol and Timeline API.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. Matrix operations, Timeline management, message conversion, GIF search (Giphy API).
  - `crates/app/` — Iced desktop frontend (native GPU-rendered via wgpu, Elm architecture)
- **Timeline API**: `matrix-sdk-ui` Timeline per room handles loading, pagination, live updates, local echo, edits, reactions, redactions. Thread timelines via `TimelineFocus::Thread`.
- **Storage:** rusqlite for app data (sessions, media cache, settings, emoji frequency), matrix-sdk's built-in SQLite for crypto/sync/event cache
- **Sync:** Pure sliding sync via `SyncService` from matrix-sdk-ui. Room list updates via reactive `RoomListService` stream watcher. Typing + verification handlers registered directly on client. No legacy `/sync` loop.
- **State:** Plain `FluxChat` struct with all fields as owned data. Elm architecture: `Message` enum → `update()` returns `Task<Message>` → `view()` renders UI. No signals, no JS interop.

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js. Fully native GPU-rendered via wgpu.
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace fonts throughout, status bar with mode badges (@/+), room header bar with space/room name
- Timeline API for all message operations (not manual /messages API)
- Modular slash command system in `commands.rs` — add new commands by adding entries to `all_commands()`
- **Panes & windows (tmux-style, `panes.rs`)**: each `Window` is an iced `pane_grid` of `PaneSlot { key }` (room, thread, or empty); `windows`/`active_window`. The focused pane of the active window *is* the main view (`selected_room_id`; `SelectRoom` → `focus_key_in_pane`) and renders the full `view_content()`; other panes render `view_pane_body` (non-virtualized, last `SIDE_PANE_ROWS`, own composer in `pane_inputs`, `PaneSend`). Title bars (drag to rearrange, split/zoom/close) when a window has >1 pane. Pane focus: pane_grid's `on_click` fires on mouse *press*, but focusing swaps the pane's widget tree (compact body → full view), which would swallow the click — so `PaneClicked` only records `pending_pane_focus` and the left-release (`SidebarDragEnd`) commits it (`commit_pane_focus`, also focuses the composer); title-bar controls use the immediate `PaneFocus`. Keys: Ctrl+B prefix (`PREFIX_ARMED` static, flipped synchronously in the global `listen_with` closure, so composers swallow the command key via `tmux_binding`; commands read from `modified_key` because iced's `key` ignores Shift — `%` would arrive as `5`; key repeat ignored) then `%` `"` `x` `z` `o` arrows/hjkl `c` `n` `p` 0–9 `&` `?`. `OpenThread`/`OpenInPane` → `show_beside` (split right, reuse a same-room thread pane). Layout of all windows persisted as JSON (`pane_layout`, `save_layout` on Tick via `layout_dirty`, `restore_layout` after the room list loads). Status bar shows `0:name*` window list. Visible pane rooms get read receipts while the window is focused
- **Command palette** (Ctrl/Cmd+K, `palette_items`): rooms first, then matching actions (`palette_actions`: settings tabs, create room/DM, explore, mentions, compact, clock, room settings/invite/search/sidebars/mark read/leave, themes); `>` prefix = actions only; rows are `PaletteItem::{Room, Action}` run via `PaletteActivate`
- **Mentions inbox** (header @ button, Ctrl/Cmd+I, `ToggleMentions`): resizable sidebar of messages that mentioned you / matched keywords across rooms — server `/notifications?only=highlight` (`messages::fetch_highlights`, decrypts where keys exist) + live `Notify` events with `is_mention` (covers encrypted rooms) + loaded timelines (`is_highlight`). `OpenMention` selects the room and jumps (`pending_jump` waits for the timeline to load it)
- **Open at first unread**: the SDK read marker is kept as a hidden `~readmarker` row (`READ_MARKER_ID`); on room switch `capture_read_anchor` records the message before it (`read_anchor`) and `ScrollToReadAnchor` puts it near the top; the NEW divider sits there until the room changes
- **Restore on launch**: last room (`last_room`), side pane (`last_thread_pane`), and sidebar toggles (`show_member_list`, `threads_open`, `mentions_open`) persist; `restore_last_view` runs after the initial room list
- (superseded) **Thread pane**: `OpenThread` shows the thread in a resizable pane beside the room (`thread_pane`, `view_thread_pane.rs`) with its own composer (`thread_input`, `ThreadSend` via the thread Timeline). Expand (`ExpandThreadPane`) shows it full-size as a thread view (sidebar entry under its room); switching rooms or Esc closes the pane. The client is built with `ThreadingSupport::Enabled` — without it the SDK event cache never routes live sync events into thread caches, so thread timelines (and thread summaries) stop updating after they open
- **Resizable panels**: room list, threads sidebar, mentions sidebar and member list each have a divider (panes resize via the pane grid) (`resize_handle(ResizeTarget)`, `resizing` state; cursor streamed only while dragging); widths persist (`sidebar_width`, `thread_list_width`, `mentions_width`, `member_list_width`)
- Thread tabs use view keys `thread::{room_id}::{event_id}` (`timeline::thread_key`/`parse_thread_key`/`real_room_id`) as `messages_by_room` keys; they are backed by SDK `TimelineFocus::Thread` timelines with their own watcher, so they update live and sends get the `m.thread` relation from `Timeline::send`
- Mention resolution: `@displayname` in outgoing messages resolved to `matrix.to` HTML links at send time via room member lookup (3s timeout). MSC3952 intentional mentions (`m.mentions`) set with resolved user IDs + `@room` detection. Incoming mention links rendered as colored pills with `@` prefix, clickable to profile modal. Both `@` and `%40` URL-encoded forms detected.
- Lightbox: dialog-style header (file name, sender · date, Copy / Save, close) over an inset image stage; clicking the image (zoom-out cursor), the backdrop or Esc closes it. Timeline images show a zoom-in cursor
- Message deletion: hides locally (`pending_redactions`) and sends the redaction; the message reappears if the request fails, and the entry clears once the timeline shows the redaction
- Room encryption: enable via room settings (one-way toggle, sends `RoomEncryptionEventContent` state event)
- Reaction tooltips: hover shows display names of who reacted (resolved from room members)
- URL previews: max 2 shown per message with expandable "Show N more" / "Show less" toggle. Thumbnails on left. Up to 5 URLs extracted per message.
- GIF search: "No results" only shown after search completes (not while typing). Help text auto-clears on typing.
- Markdown formatting: `build_message_content()` HTML-escapes the body, resolves mentions, then converts `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, ``` ```codeblocks``` ```, `# headings` (h1-h6) to HTML via `simple_markdown()` + `apply_headings()`. Edits go through the same builder. Headings render at scaled font sizes (h1=1.7×, h2=1.45×, h3=1.25×, h4=1.15×, h5=1.05×, h6=1×) relative to message font size. Markdown processed before `\n` → `<br>` conversion; br conversion skips `<pre>` blocks.
- Command escaping: `//foo` sends literal `/foo` as a message (strips leading `/`). Suppresses command autocomplete when input starts with `//`.
- File upload: native file dialog via `rfd` crate (also drag-drop / paste). The pending file shows as a card above the composer; Enter or Send goes through `timeline::send_attachment` (Timeline send queue: local echo, thread relation, reply) with the composer text as the Matrix v1.10 **caption** — one event, not file + separate message. Incoming captions: `MessageInfo.caption` / `caption_html` (then `body` = file name), rendered under the media. GIFs use the same path.
- Image thumbnails: `generate_thumbnail()` creates 320x240 PNG thumbnails on upload via `image` crate, passed in `AttachmentConfig`. Display always uses `get_event_media()` (handles encrypted media decryption), not server-side thumbnails.
- Read receipts: authoritative per-event `read_receipt_user_ids` from SDK timeline items (`ev.read_receipts()`). Rendered as right-aligned stacked avatars (14px, max 5 shown) below message row via `view_read_receipts()`. Own user excluded. Outgoing receipts via `timeline::mark_as_read()` (Read + FullyRead, SDK-deduplicated; threaded in thread views) only when the view is active, scrolled to bottom, and the window is focused; also from the "Mark as Read" context menu item.
- Power levels: badges in member list (yellow=admin, blue=mod). Profile modal shows role badge + power level, with full-width role preset buttons (Member/Moderator/Admin with descriptions) and custom level input when user has sufficient permissions. `/op` and `/deop` commands with display name resolution.
- Profile card (`view_profile.rs`): gradient banner (nick colour → accent) with overlapping avatar, Mention / Message actions, name + copyable user ID, presence + status message, role badges, verification card (Verify / Trust manually), moderation card when we outrank them in the current room (role `segmented` control, custom level, Kick / Ban with inline confirm via `profile_confirm`), Ignore. `find_member()` falls back to space/other rooms' member lists. Clicking own profile opens Settings instead.
- **Preferences** (`prefs.rs`, `Prefs` persisted as JSON under `preferences`; `SetPrefs`): 12h clock, grouping, profile/room-change/deleted-message rows (`MessageInfo.row_kind`: Message/JoinLeave/Membership/Profile/RoomState/Redacted — redacted messages are kept as placeholder rows and filtered in `visible_rows`), read receipts / typing display, big emoji (`is_emoji_only`), link previews, GIF autoplay (off = still frame from `image_cache`), code blocks expanded, Ctrl/Cmd+Enter to send, markdown on/off (`send_text(.., markdown)`), emoji suggestions, emoticon replacement (`replace_emoticons`), private read receipts (`mark_as_read(.., private)` → ReadPrivate), typing notices, presence sharing (`set_own_presence`). Timestamps / compact / join-leave keep their own settings keys (and `/timestamp`, `/compact`, `/joinpart` now actually take effect)
- **Voice & Video** (`core::audio_devices`): input/output device by name (fallback to default), echo cancellation / noise suppression / AGC (fed to LiveKit `AudioSourceOptions`), `MicTest` level meter; set once via `set_audio_prefs` from `Prefs::audio()`
- **Notification rules** (`core::notify_rules`, Settings → Notifications "Your account"): server push rules — DM / group defaults, mentions (new + legacy rules toggled together), @room, keywords, invites, calls, bot notices, room upgrades. Loaded at startup: keywords + mentions drive the red message highlight (`is_highlight`, whole-word `contains_word`), along with local `/highlight` words
- **Room settings tabs** (`RoomSettingsTab`): General / Security & Privacy / Roles & Permissions (`core::room_admin`: join rule incl. space-restricted, history visibility, guest access, directory listing, encryption; power levels per action incl. per-event-type, privileged users, ban list with unban)
- Read receipts: the SDK timeline only tracks them with `track_read_marker_and_receipts(MessageLikeEvents)` (set in `get_or_create_for_key`); avatars under each message with a "Read by …" tooltip
- Window title: `(unread) #room › thread · Space — @me · N members — FluxChat`
- Settings modal: Sidebar tab navigation (Account/Preferences/Appearance/Notifications/Voice & Video/Sessions/Security) with vertical layout. Account tab: banner at top (150px, crop position slider for non-4:1 images, pick-then-save flow), avatar, display name, password change. Avatar/banner use cached `image::Handle` to prevent flicker.
- Session restore: failures never delete the saved session or crypto store (another instance may hold the lock); the error is shown on the login screen. Only an explicit new login replaces the store.
- Password change: `change_password()` in core with UIAA password auth. UI in settings account tab with current/new/confirm fields.
- Account deactivation: `deactivate_account()` in core (UI not yet exposed).
- Banner crop: Pick image -> preview with offset slider -> Save uploads cropped 4:1 version. Crops original bytes via `image` crate before upload.
- Presence: sliding sync delivers no presence, so the store is usually empty. `messages::fetch_presence()` asks the user's server (`GET /presence/{user}/status`); the profile card fetches fresh on open (`profile_presence`), and `get_room_members` fills presence for rooms of ≤ `PRESENCE_FETCH_LIMIT` members (8 concurrent, 5s timeout). Labels: "online" / "idle" / "offline" (+ `last_active_secs`)
- Custom status: `status_msg` from presence, displayed in member list and profile modal, `/status` command
- Stickers: `m.sticker` events handled in timeline conversion + `get_event_media()`, rendered via InlineImage
- Syntax highlighting: `syntect` crate with base16-ocean.dark theme, processes `<pre><code class="language-xxx">` blocks in `highlight_code_blocks()`, language tag detection in `simple_markdown_to_html()`
- Theming: `ThemeColors` struct with 15 color slots + dark/light flag. Built-in FluxChat default hardcoded; all other themes are TOML files seeded to `data_dir/fluxchat/themes/` on first run and loaded from disk. Users can edit/delete/add themes without recompiling. `theme::colors()` returns active theme via global `RwLock`. `dark` flag controls `Theme::Dark` vs `Theme::Light` for Iced widget defaults. 9 bundled themes: FluxChat, Nord, Nord Light, Dracula, Night Owl, Tokyo Night, Atom One Dark, Catppuccin Mocha, Catppuccin Latte.
- Font selection: separate message and sidebar font families via `fontdb` system font discovery. Persisted to SQLite. `font_by_name()` / `font_bold_by_name()` leak font names to `'static` for Iced's `Family::Name` requirement.
- Inline LaTeX: `$...$` converted to Unicode via `unicodeit` crate. Strips `\text{}`, `\mathrm{}` etc. before conversion.
- Security: `html_escape()` for user content in HTML, URL-encoding for matrix.to links
- UTD diagnostics: `MessageInfo.utd` carries session/device/cause; `check_utd_reasons()` compares the sender device's cached identity key with the event's `sender_key` and shows a hint under the message (unknown device / device keys changed / key never shared)
- DRY: `parse_room_id()`/`parse_user_id()` helpers in rooms.rs, `extract_message_content()` shared helper, `HudPanel` reusable wrapper, index-based room list grouping
- GIF search: Giphy API via `crates/core/src/gif.rs`, `/gif` and `/giphy` commands open GifsHud above input bar, click sends GIF as image upload. API key persisted to SQLite via `/giphykey`, falls back to `GIPHY_API_KEY` env var. GifsHud renders above MessageInput (not top like other HUDs) with 50vh max-height, 3-column grid with 4:3 aspect ratio thumbnails.
- Failed room operations (join/invite/kick/ban/rename/topic/power level) surface via `Message::ShowError` in the notice line above the input
- Per-view input drafts (`drafts`): unsent text is saved/restored when switching rooms; PageUp/PageDown scroll the message list from the input bar, Ctrl+End jumps to the bottom
- Member list has a filter box (`member_filter`, name or user ID substring)
- Runtime: iced tasks run on `BigStackExecutor` (tokio, 16 MB worker stacks) because matrix-sdk crypto futures overflowed the default 2 MB stacks
- Header action bar: room avatar + name, Search, Pins (red icon when pins exist), Threads, Members, Settings, Favourite (star) — all with tooltips. No overflow menu.
- Tooltips: always delayed (`ui::TOOLTIP_DELAY`, 600ms) — instant ones flashed over neighbouring buttons. Use `ui::with_tooltip` (above), `ui::tooltip_at(.., Position::Bottom)` for anything near the window top (a tooltip that doesn't fit is snapped back into the window, covering its own button), or `ui::styled_tooltip` for custom content. Raw `iced::widget::tooltip` calls must add `.delay(crate::ui::TOOLTIP_DELAY)`.
- User manual lives in `docs/` (index `docs/README.md`); update it, README.md and this file with every user-visible change before committing.
- Room right-click menu starts with the room's 3 most recently active threads (server `/threads` via `messages::recent_threads`, `context_threads`) and "All threads" (`ShowRoomThreads` → room + threads panel). The header's Threads button toggles a **threads sidebar** (`view_threads_sidebar`, between the thread pane and the member list; resizable via `ResizeTarget::ThreadList`, width persisted) listing the server's thread list (`thread_list`, newest activity first; loaded threads shown until it arrives, live activity merged in by `merge_loaded_threads`, reloaded on room switch). Clicking a thread opens it in the thread pane
- Threads use one icon everywhere: `SVG_THREAD` (a spool of thread)
- Sidebar: each room lists its `SIDEBAR_THREADS` most recently active threads indented below it (`room_threads`, fetched for all non-DM rooms at startup via `load_all_room_threads`, kept live by `merge_room_threads`), collapsible per room (caret + count, `collapsed_room_threads` persisted). Thread rows in the room menu, threads sidebar and room sidebar show a hover preview (`thread_hover_preview`: full root message + latest reply)
- Right-click context menus on rooms (mark read/favourite/copy link/invite/settings/leave — leave asks for confirmation via `Modal::ConfirmLeave`), spaces, and messages (copy message link / copy event ID / view source JSON via `get_event_source`) and spaces (details/invite/leave)
- Space dialog (`view_space.rs` + `update_spaces.rs`, opened from the room-list header when a space is selected, or right-click a space → Manage Space): Rooms / Members / Settings tabs. Children come from `/hierarchy` via SDK `SpaceRoomList` (`core::spaces::space_children`, so unjoined rooms show with names + Join through `via`); add existing / remove (inline confirm) / create room in space, gated by `space_permissions()` (m.space.child / m.room.name / invite power). Add/remove go through SDK `SpaceService` (also maintains `m.space.parent`). A `m.space.child` with empty `via` is a removed child — `spaces::active_child_id()` filters those everywhere
- Dialog kit (`ui.rs`): `header`, `btn`/`icon_btn`/`icon_only` with `Tone` (Primary/Secondary/Danger/Success/Ghost), `input`, `field`, `card`, `tabs`, `segmented`, `badge`, `toggle_row`, `empty_state`, `status_line`, `dialog_style`. All modals are built from these; Profile and Settings draw edge to edge (no frame padding)
- `Message::Batch(Vec<Message>)` runs several messages in order (task callbacks needing more than one follow-up)
- Hover/selection overlays use `theme::tint(alpha)` (theme text colour at alpha, boosted on light themes) — never hardcoded white-alpha, which vanishes on light themes. Black alphas are only for shadows/backdrops/image scrims
- Search and Ctrl/Cmd+K switcher share the command-palette chrome (`palette()` + `palette_layer()`): search icon + borderless input, inset `ui::row_style` rows (keyboard selection = accent), empty states, keyboard-hint footer. Context menus use `ui::dialog_style` + inset rounded rows
- No room selected shows a welcome view (Jump to room / New message / Explore rooms)
- Message row geometry: `msg_columns()` (time / nick / avatar widths scaled to font size) and `msg_content_indent()` — use these for anything aligned to the message text (reactions, previews, thread summary), never hardcoded offsets. Consecutive messages from one sender within `MSG_GROUP_SECS` (`is_continuation`) dim the nick and drop the avatar; a new speaker gets extra top padding. Members without an avatar get a tinted initial
- Sidebar: room icons sit in a fixed 18px slot so names align; thread entries line up under their parent room's name (`THREAD_ENTRY_OFFSET` + the space indent)
- Room invitations: red bell icon header with badge, desktop notification on new invite, accept navigates to room, instant UI removal on accept/reject
- Typing indicators: auto-expire after 10s, clear immediately on empty list
- Room avatar upload updates sidebar immediately
- New rooms appear in sidebar instantly after creation
- Leave room / server-side prune removes from sidebar immediately
- Room notifications: per-room mute/mentions/all via `client.notification_settings()`, `/mute` and `/unmute` commands, segmented control in room settings
- Room settings modal: icon, name/topic (Save enabled when changed), notification mode `segmented` (all/mentions/mute, `room_notify_mode`), membership in every space (`spaces::parent_spaces`), encryption, copyable room ID, leave
- Message forwarding: Forward button in the message hover bar opens ForwardModal (searchable room picker); `forward_message()` sends content to the target room
- Favourites: `room.set_is_favourite()` / `room.is_favourite()` via Matrix tags, star toggle in overflow menu, dedicated sidebar section, rooms also appear in their original section
- Files HUD: scans messages for `image_mxc`, shows thumbnails from media cache, click to scroll+highlight
- Topic bar: IRC-style slim bar below tabs showing room topic
- Header-bar panels (Pins/Files/Notices): `hud_panel()` (title + count + close, scrollable body capped at `HUD_PANEL_MAX_HEIGHT`) with `hud_row()` entries; pins/files jump to the message, threads open the thread
- Search: overlay dropdown with click-to-scroll + blue highlight on target message, server-side with client-side fallback, arrow key navigation, Enter to select, auto-focus on toggle, results clear on select
- Desktop notifications: `notify-rust`, fired off-thread by `show_notification()`. A client event handler on `OriginalSyncRoomMessageEvent` uses the SDK-computed push `Vec<Action>` (room mute / mentions-only / keyword rules) for every room, skipping own messages, startup history, and the active focused room. Clicking a notification (freedesktop backends) sends `NotificationClicked` → selects the room and focuses the window. Settings: enable, mentions-only (uses highlight action), preview, sound.
- Polls: MSC3381 unstable events (`org.matrix.msc3381.poll.start/response/end`). `PollState::results()` from matrix-sdk-ui provides aggregated votes. `PollDisplay` component renders interactive vote UI with progress bars. Voting sends `UnstablePollResponseEventContent` via Timeline.
- Date dividers: SDK `VirtualTimelineItem::DateDivider` → `~date-{ms}` rows rendered as "Today / Yesterday / weekday, day month" separators (`view::format_day`)
- Message status: `(edited)` marker from `Message::is_edited()`; local echoes render as normal rows with "sending…" / "failed to send [retry]" from `EventSendState` (`RetrySend` re-enables the send queue)
- Emoji shortcodes: `:smi` (≥2 chars after a colon at a word start) pops a completion list from the `emojis` crate; Tab/Enter/click inserts
- Status bar shows "reconnecting…" (red) while the sync service is restarting; verified/unverified and "key backup not connected" open the Security tab; thread views show "#room › thread"; window title is "(unread) Room — FluxChat"
- IRC features: join/part messages with colored arrows (green -->/red <--), mode badges in status bar (@/+), lag meter (whoami round-trip every 30s), Alt+1-9 selects the Nth room in sidebar order (`sidebar_room_order()`), Alt+Up/Down previous/next room, Alt+Shift+Up/Down previous/next unread room, Tab-cycling command completion, server notices panel
- Display toggles: `/timestamp [short|full|off]`, `/compact` mode, `/joinpart` visibility — all persisted to SQLite
- Highlight words: `/highlight`/`/unhighlight` with persisted keyword list, matching messages get red-tinted background
- Activity indicators: red badges for mentions (highlight_count from SDK), blue for regular unread
- Space avatars: lookup by space_id (not name) to handle rooms in multiple spaces correctly
- All SVG icons (Lucide-style) — zero emoji characters in the UI chrome
- Clickable nicks/avatars in messages open profile modal
- State events (power level changes, room name/topic/avatar) rendered as system messages
- Profile modal prioritizes current room for member lookup (correct power levels per room)

## Build & Run
```bash
cargo run -p fluxchat
```
- Requires GPU (wgpu). Works on Linux (Wayland/X11), macOS, Windows.
- No WebView deps needed.
- Fonts bundled in `crates/app/fonts/` (relative paths via `include_bytes!`).
- macOS: `./build-macos.sh` creates `.app` bundle with icon. `cp -r target/release/FluxChat.app /Applications/` to install.

## App Architecture (`crates/app/`)
- **Elm architecture:** `FluxChat` state struct → `Message` enum → `update(&mut self, msg) -> Task<Message>` → `view(&self) -> Element<Message>`
- **Files:** main.rs (entry), state.rs (FluxChat struct + enums), message.rs (Message/SyncEvent), update.rs (core state transitions), update_settings.rs (settings/profile/security), update_spaces.rs (space dialog), update_call.rs (call/RTC), view.rs (layout/room list/content), view_messages.rs (message rendering), view_modals.rs (modals), view_profile.rs (profile card), view_space.rs (space dialog), ui.rs (dialog kit), view_members.rs (member list + call widget), view_input.rs (input bar/autocomplete), html.rs (Matrix HTML→Spans), selectable_text.rs (selectable rich text widget), commands.rs (slash commands), theme.rs (colors)
- **HTML rendering:** `html5ever`-based parser with arena DOM converts Matrix HTML to `Vec<Span>` for `selectable_rich_text` widget. Handles bold/italic/strike/code/links/mentions/blockquotes/lists (ul/ol). Syntax highlighting via syntect. Markdown-style bullets in formatted bodies auto-converted to HTML lists.
- **Selectable text:** Custom `SelectableRichText` widget (`selectable_text.rs`) replaces Iced's built-in `rich_text` for message rendering. Uses cosmic-text's buffer (already underlying Iced's text engine) for hit-testing, mouse click/drag selection, and Ctrl+C/Cmd+C copy. Preserves all existing span styling (colors, backgrounds, links, underlines, strikethrough). Link clicks still work on click without drag. Ctrl+A selects all text in hovered widget.
- **Modals:** `stack` + `opaque` + `mouse_area` overlay pattern. Modal enum dispatches to Settings/CreateRoom/Profile/Verification/Lightbox/Forward/DevTools.
- **Commands:** 48+ slash commands producing `CommandResult` (Messages/Rewrite/Help/Noop). Decoupled from UI — returns Message variants. Display name resolution for /kick, /ban, /op, /deop.
- **Subscriptions:** `time::every(5s)` Tick for invites/typing expiry + call presence (member presence and all-room call scan every 6th tick), `keyboard::listen()` for Alt+1-9 + Escape, window focus events for notification suppression. Room list updates via reactive `RoomListService` stream (debounced 300ms). Cursor movement is recorded in atomics (`view::cursor_position()`) and only emitted as `Message::CursorMoved` while dragging the sidebar — every Message rebuilds the view.
- **Async:** Room commands (join/leave/invite/kick/ban/topic), file upload, search, member loading all via `Task::perform()`.
- **Status:** Fully functional — login, rooms, messaging, reactions, emoji picker, search, threads, back-pagination, media, modals, settings all implemented.

## Important Patterns
- Elm architecture: all state changes go through `Message` enum → `update()` → returns `Task<Message>`
- `Task::perform()` for async operations, `Task::batch()` to combine multiple tasks
- One watcher task per room Timeline (`core::timeline::RoomTimelines`), created together with the Timeline and aborted when it is removed. It mirrors the SDK item list by applying `VectorDiff`s (only changed items re-converted), coalesces bursts, and sends the full ordered list through the `TimelineSink` → `SyncEvent::MessagesUpdated`, which replaces `messages_by_room[room]` outright. Nothing else writes room messages — no Tick re-read, no merge, no drift detection. `request_snapshot()` asks a watcher to re-send.
- Locally deleted messages are hidden via `pending_redactions` until the timeline reflects the redaction (or `RedactionFailed` restores them)
- Avatar resolution uses sync media cache lookup + background async batch resolve for uncached MXC URIs
- Message list is virtualized: `visible_rows()` → `virtual_window()` picks rows within viewport ± `VIRTUAL_OVERSCAN` using measured heights (`row_heights`, filled by the transparent `Measured` wrapper in `measured.rs`; unmeasured rows use `ESTIMATED_ROW_HEIGHT`), with spacers above/below. `virt_anchor` records the first visible row + its offset each render; `MessagesScrolled` (also fired on content-size changes) compensates any change in that row's position (rows getting measured, history prepended) via `scroll_to`. Back-pagination triggers at offset < 100px; `JumpToEvent` uses `row_top()`.
- Emoji picker: `emojis` crate for data, `EmojiPickerMode::Insert`/`React(event_id)` for two trigger modes, frequency tracked in `HashMap<String, u32>` persisted as JSON to SQLite, 400px right-aligned panel; `EMOJI_CATEGORIES` gives each of the 10 tabs (frequent + 9 `emojis::Group`s) a Lucide icon + tooltip, equal-width so they always fit; category heading above a 9-column grid
- `read_rooms` HashSet is session-only (not persisted) — SDK's `notification_count` is authoritative on startup
- `nick_color_index` lives in `theme.rs` for shared access
- Thread replies detected via SDK `MsgLikeContent::thread_root` (populated since SDK 0.18)
- Thread roots show a summary row (`view_thread_summary`): reply count = max(SDK `thread_summary.num_replies` via `thread_count`, loaded replies in `thread_counts`) + latest reply's sender/preview (`MessageInfo.thread_latest`)
- Timeline watchers keep back-paginating (`Backfill`, max 20 pages) until the view has 30 *visible* rows — thread replies and edits don't count — so rooms whose recent history is all thread/edit traffic don't open blank
- Thread views are thread-focused SDK Timelines opened through `get_or_create_for_key`; the parent room's member list is stored under the thread key
- Thread replies hidden from main room timeline, shown in dedicated thread view
- Open thread tabs tracked in `open_tabs`, shown indented in sidebar below parent room, removed on navigate away
- Named constants: `MEDIA_CACHE_LIMIT`, `MEDIA_CACHE_EVICT`, `MESSAGE_PAGE_SIZE`, `RECEIPT_EVENT_LIMIT`, `NICK_COLOR_COUNT`
- Zero clippy warnings (`cargo clippy --workspace --all-targets`); no crate-level `dead_code` allow. Unit tests (`cargo test --workspace`) cover markdown/escaping, thread keys, HTML span parsing, body cache.

## Timeline API Usage
- `get_or_create_timeline(client, &core.timelines, room_id)` creates/caches Timeline per room and spawns its watcher (the only way Timelines are created); `core.timelines.get()` for lookup without creating
- `create_thread_timeline(client, room_id, thread_root_id)` creates thread-focused Timeline
- `timeline.subscribe()` is called only inside the watcher
- `timeline_item_to_message_info()` converts SDK types to our MessageInfo (extracts thread_root, thread_count, polls, stickers)
- Initial back-pagination (to 30 items) runs from the watcher after its first snapshot
- `timeline.paginate_backwards(20)` for scroll-up pagination
- `timeline.send()`, `send_reply()`, `edit()`, `toggle_reaction()`, `redact()` for message actions
- `timeline::mark_as_read()` for read receipts
- Event cache enabled in auth: `client.event_cache().subscribe()`
- Thread replies sent with `m.thread` relation via `room.send()` (not Timeline send_reply)
- Poll votes sent as `AnyMessageLikeEventContent::UnstablePollResponse` via `timeline.send()`

## Slash Command System
- Registry in `crates/app/src/commands.rs`
- `SlashCommand` struct: name, usage, description, scope (Global/Room/TextRewrite), execute fn
- `dispatch()` looks up and executes, `autocomplete_entries()` feeds the popup
- `CommandAction` enum: Handled, Rewrite(String), ShowHelp(String), Noop
- Tab-cycling autocomplete: repeated Tab presses cycle through matches, typing resets
- 57+ commands: /join /leave /part /invite /kick /ban /unban /topic /op /deop /nick /me /msg /attach /forward /whois /query /ignore /unignore /users /directory /threads /notices /mute /unmute /status /highlight /unhighlight /timestamp /compact /joinpart /poll /pollend /gif /giphy /giphykey /slap /troutslap /shrug /tableflip /unflip /lenny /spoiler /settings /sessions /security /account /away /quit /clear /help /roomname /html /plain /upgraderoom /devtools

## Settings Modal
- Sidebar tab navigation (vertical left sidebar, 120px) with Account/Appearance/Notifications/Sessions/Security tabs
- Appearance: message font size slider (10-24px, persisted to SQLite) with live preview
- Notifications: toggle switches (Iced Toggler) for enable/disable, mentions-only, show preview, sound — all persisted to SQLite
- Account: banner at top (150px, crop slider for tall images, pick-then-save upload flow), avatar with cached handles (no flicker), display name, password change (old/new/confirm), logout
- Sessions: device list with SVG verification icons, last seen, remote sign-out with UIAA
- Security: cross-signing status with SVG icons, key backup enable/recover, recovery key display
- Status bar "verified"/"unverified" clickable → opens security tab
- Clicking own profile anywhere opens Settings modal (Account tab) instead of profile modal

## Voice Calls (MatrixRTC + LiveKit)
- **Architecture:** MatrixRTC signaling (m.call.member state events) + LiveKit SFU for audio transport
- **Token flow:** `.well-known` → `org.matrix.msc4143.rtc_foci` → `livekit_service_url` → OpenID token exchange → `POST /sfu/get` for LiveKit JWT
- **Core modules:** `rtc.rs` (MatrixRTC signaling: join/leave, state events, JWT exchange), `rtc_audio.rs` (LiveKit connection, cpal audio capture/playback)
- **Audio capture:** cpal input device → mpsc channel → tokio task → `NativeAudioSource` → LiveKit publish. Channel bridges sync cpal callback to async LiveKit API.
- **Audio playback:** `NativeAudioStream` from remote tracks → tokio task → ring buffer → cpal output stream. Output stream leaked into tokio task, cleaned up when track ends.
- **Call lifecycle:** `CallJoin` → get JWT → `CallSession::connect()` → publish mic (muted by default) → event loop drains `RoomEvent` → `CallEvent` → bridged to `Message` variants. `CallLeave` → `watch::channel` signal → `session.disconnect()` → `leave_call()` state event.
- **Participant tracking:** LiveKit events are source of truth during connected calls. Tick handler only updates room indicators, not participants, when connected.
- **UI:** In-call panel (participant tiles + controls) above messages, PIP in member list when viewing other rooms. Phone icon in tab bar action buttons. Join banner for rooms with active calls.
- **State events:** `m.call.member` (MSC3401) sent with `room.send_state_event_raw()`. Custom event type `org.matrix.msc3401.call.member` with `memberships` array. Empty memberships = left call. Suppressed from timeline rendering.
- **macOS:** Requires `-ObjC` linker flag for libwebrtc Objective-C categories (`.cargo/config.toml`).
- **TLS:** `livekit` crate requires `native-tls` feature for WSS connections.

## Performance Notes
- Member list lazy-loads: 30 initially, more on scroll. Avatars resolved in background batch.
- Media caches: in-memory data-URI cache capped at `MEDIA_CACHE_LIMIT` (500); SQLite `media_cache` pruned to `MEDIA_DB_CACHE_LIMIT` (1500) entries + VACUUM at login (`prune_media_cache`). `cache_media` is INSERT OR REPLACE.
- Avatar cache stores `image::Handle` (cheap Arc clone) to avoid re-decoding on every render.
- Inline image cache persisted to SQLite (base64) for instant display on restart.
- syntect `SyntaxSet`/`ThemeSet` loaded once via `LazyLock` statics.
- SDK timeline output used directly — no client-side message merge. Edits applied by SDK. Parsed message bodies (HTML → spans, syntect) memoized in `html::BodyCache`; cleared on theme change or room-name changes.
- Empty messages filtered at load time, not per-render. New message detection via timestamp tracking.
- Thread counts computed at update time, not per-render.

## Code Quality
- Zero clippy warnings
- Named constants for all magic numbers
- `html_escape()` for XSS prevention
- `extract_message_content()` shared helper eliminates message parsing duplication
- `parse_room_id()`/`parse_user_id()` helpers eliminate error mapping duplication
- `tracing::warn!` on critical silent failures (encryption, session deletion)

## Features Implemented
Login (password + SSO/OIDC with provider detection), session persistence, rooms grouped by space, spaces sidebar with overview, channel tabs (Chrome-style with memoized DOM preservation, DM tabs without # prefix, Alt+1-9 switching), messaging via Timeline API with local echo and word-bounded mention resolution, formatted HTML messages with pre-computed mention transforms, inline images with lightbox and save dialog, full emoji picker (7 categories, search, frequently-used), reactions via Timeline toggle_reaction, replies with context preview, message editing via Timeline edit (up-arrow to edit last message), message deletion via Timeline redact, message forwarding with room picker modal, pinned messages (HUD), server-side search (HUD with clickable result list, scroll-to-highlight, arrow keys, client-side fallback), threads (indicators, HUD, dedicated tabs with live updates, proper m.thread relation), typing indicators (send + display), URL previews with og:image thumbnails, sticker display (m.sticker events), polls (create/vote/end with progress bars via MSC3381), modular slash commands (45+) with Tab-cycling autocomplete and /help, @mention autocomplete with inline avatar pills and Tab completion, contenteditable input with multiline (Shift+Enter, recursive div/p extraction), markdown formatting (**bold** *italic* ~~strike~~ `code` ```codeblocks``` with language tags), syntax highlighting via syntect (100+ languages, base16-ocean.dark), file upload (picker + drag-drop + paste + /attach) with thumbnail generation for images, files HUD with image previews, read receipt display (stacked avatars per message via view_read_receipts, loaded every 5s via Tick, 14px avatars max 5 shown), read receipts sent via Timeline mark_as_read with persistent unread tracking, unread indicators (numeric sidebar badges, red for mentions / blue for regular, message divider, floating pill), device verification UI (SAS emoji), interactive verification request from profile modal, manual user verification for bots (signs master key via UserIdentity::verify()), key backup and recovery (enable/recover/display), Discord-style profile modal (colored banner strip, avatar, role badge, power level controls with Member/Mod/Admin presets + custom level, permission-gated, Message/Ignore actions), clicking own profile opens Settings, clickable nicks/avatars in messages, power level badges in member list, /op and /deop commands, custom user status (/status, display in member list + profile modal), profile editing (name, avatar, banner with crop positioning), settings modal (sidebar tab navigation, account/sessions/security), password change in settings, banner pick-crop-save flow (4:1 aspect crop with offset slider), cached avatar/banner handles (no flicker), room creation with public/encrypted toggles, room settings (name/topic/aliases/notifications/invite/leave), room notification modes (all/mentions/mute), room alias management (add/remove), room invitations (accept/reject in sidebar), room upgrade/tombstone detection with follow button, space overview with channel list and invite, back-pagination via Timeline paginate_backwards, ignore/block users (/ignore /unignore with message filtering), user directory search (/users), public room directory (/directory), presence/away status, favourite rooms (star toggle, dedicated sidebar section + original section), IRC-style topic bar, IRC join/part with colored arrows, mode badges in status bar, lag meter (30s ping), copy room link to clipboard, highlight words with visual indicators, display toggles (timestamp/compact/joinpart), server notices panel, state event rendering (power levels, room name/topic/avatar changes), cross-platform window decorations (macOS native, .app bundle via build-macos.sh), bundled fonts (cross-platform relative paths), all SVG icons (zero emojis), collapsible room list sections with space avatars by ID, DM person icon in sidebar, invite icon in member list header, reusable HudPanel/ForwardModal/PollDisplay components, 3-dot overflow menu for secondary room actions, GIF search via Giphy API (/gif /giphy commands + overflow menu, GifsHud with grid picker, click to download+upload as image, /giphykey for API key persistence), emoji picker (9 categories, search, frequently-used with SQLite persistence, insert + react modes, 380px right-aligned panel), pill-shaped reaction badges with accent tint, scroll-up back-pagination with loading indicator and scroll correction, desktop notifications via notify-rust (settings: enable/mentions-only/preview/sound, window focus tracking, per-room mute), MSC3952 intentional mentions (m.mentions with user_ids + @room), sliding sync (pure SyncService, reactive RoomListService, no legacy /sync), pin/unpin from message hover bar, header bar with tooltips (search/pins/threads/members/settings/favourite), thread indicators with reply counts + sidebar entries + sub-header, voice messages (record via cpal + Opus/OGG encoding, in-process Opus decode + cpal playback, MSC3245 voice marker, preview before send, auto-normalization), audio message display (play/stop with progress bar, duration), room knocking (MSC2403 /knock command + sidebar pending section), location sharing (MSC3488 /location command + picker HUD + coordinate card display), selectable message text (custom SelectableRichText widget using cosmic-text buffer hit-testing, mouse drag selection, Ctrl+C copy, Ctrl+A select all), right-click context menus on rooms and spaces, space details modal (rooms/members/homeserver), invite notifications (red bell badge, desktop notification, instant accept/reject), room avatar in header bar, /kick and /ban with display name resolution, /rotatekeys for encryption key rotation, TOML theme system (9 built-in themes + custom user themes, dark/light mode, seeded to data dir), font selection (separate message/sidebar fonts via fontdb system font discovery), inline LaTeX-to-Unicode conversion via unicodeit, inline video player (iced_video_player + GStreamer with rotation handling via videoflip + videorate for 0fps bridged media), video thumbnail generation (GStreamer frame extraction + GstDiscoverer rotation metadata + image crate resize, cached to SQLite with dimensions), file attachment download cards, sync auto-reconnect on network loss (clears cached timelines, re-subscribes active room), persistent collapsed sidebar sections, indented room list under space headers, Ctrl+K quick room switcher, spaces hierarchy (recursive SpaceNode tree), theme-aware code blocks (code_bg + code_text per theme, light theme support), gold favourite star with instant local toggle, cross-message text selection (controlled SelectableRichText with lifted state, drag across messages, Ctrl+C copies concatenated text), /devtools modal (room state viewer + raw event sender), /me emote fix (sends as m.emote msgtype), /unban /roomname /html /plain /upgraderoom commands, cache eviction limits (avatar 1000, image 500, gif 100, preview 200), theme-aware hardcoded colors replaced with theme::colors() + with_alpha(), GIF download 50MB size limit, video temp file cleanup on close, Ctrl+U clear input bar, unread dot+badge indicators on left side of room entries, stable widget tree for text selection persistence across hover, command escaping (// sends literal /command as message), markdown heading rendering (h1-h6 with scaled font sizes), DM creation instant sidebar update

## Features Pending
- Video calls (camera capture + video tiles)
- Location map tiles (currently shows coordinate card, needs embedded OSM tile rendering or static map image fetch)
- In-process decoding for non-Opus audio messages (mp3/m4a still use a system player: ffplay/afplay/mpv)
- Voice message waveform visualization (MSC3246)
- Push-to-talk for voice calls (hold key to unmute, release to mute; toggle between PTT and open mic modes)

## Known Issues
- Rows never rendered contribute `ESTIMATED_ROW_HEIGHT` to spacers, so the scrollbar length is approximate until they are measured
- Window size / sidebar width persist via `layout_dirty` flushed on Tick
