# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI. Uses matrix-rust-sdk + matrix-sdk-ui for the Matrix protocol and Timeline API.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. Matrix operations, Timeline management, message conversion, GIF search (Giphy API).
  - `crates/app/` — Iced desktop frontend (native GPU-rendered via wgpu, Elm architecture)
- **Timeline API**: `matrix-sdk-ui` Timeline per room handles loading, pagination, live updates, local echo, edits, reactions, redactions. Thread timelines via `TimelineFocus::Thread`.
- **Storage:** rusqlite for app data (sessions, media cache, settings, read rooms, emoji frequency), matrix-sdk's built-in SQLite for crypto/sync/event cache
- **Sync:** Background sync loop with `SyncCallbacks` trait for typing + room list updates. Message/edit/reaction handling moved to Timeline diff stream.
- **State:** Plain `FluxChat` struct with all fields as owned data. Elm architecture: `Message` enum → `update()` returns `Task<Message>` → `view()` renders UI. No signals, no JS interop.

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js. Fully native GPU-rendered via wgpu.
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace fonts throughout, status bar with mode badges (@/+), channel tabs with Alt+1-9 switching
- Timeline API for all message operations (not manual /messages API)
- Modular slash command system in `commands.rs` — add new commands by adding entries to `all_commands()`
- Thread tabs use synthetic room IDs (`thread::{room_id}::{event_id}`) stored in `messages_by_room`
- Mention resolution: display names in outgoing messages converted to `matrix.to` HTML links at send time, with word-boundary checking to avoid mangling URLs
- Markdown formatting: `build_message_content()` converts `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, ``` ```codeblocks``` ``` to HTML at send time via `simple_markdown_to_html()`. Markdown processed before `\n` → `<br>` conversion; br conversion skips `<pre>` blocks.
- File upload: native file dialog via `rfd` crate, with thumbnail generation for images.
- Image thumbnails: `generate_thumbnail()` creates 320x240 PNG thumbnails on upload via `image` crate, passed in `AttachmentConfig`. Display always uses `get_event_media()` (handles encrypted media decryption), not server-side thumbnails.
- Read receipts: `load_event_receipts()` per recent event_id (not per-user) with both `ReceiptThread::Main` and `Unthreaded`. Debounced refresh every 5s in diff stream watcher. Rendered as right-aligned stacked avatars below message row.
- Power levels: badges in member list (yellow=admin, blue=mod), numeric display + promote/demote in profile modal, `/op` and `/deop` commands with display name resolution
- Custom status: `status_msg` extracted from presence events, displayed in member list and profile modal, `/status` command
- Stickers: `m.sticker` events handled in timeline conversion + `get_event_media()`, rendered via InlineImage
- Syntax highlighting: `syntect` crate with base16-ocean.dark theme, processes `<pre><code class="language-xxx">` blocks in `highlight_code_blocks()`, language tag detection in `simple_markdown_to_html()`
- Security: `html_escape()` for user content in HTML, URL-encoding for matrix.to links
- DRY: `parse_room_id()`/`parse_user_id()` helpers in rooms.rs, `extract_message_content()` shared helper, `HudPanel` reusable wrapper, index-based room list grouping, `TabEntry` type alias
- GIF search: Giphy API via `crates/core/src/gif.rs`, `/gif` and `/giphy` commands open GifsHud above input bar, click sends GIF as image upload. API key persisted to SQLite via `/giphykey`, falls back to `GIPHY_API_KEY` env var. GifsHud renders above MessageInput (not top like other HUDs) with 50vh max-height, 3-column grid with 4:3 aspect ratio thumbnails.
- Overflow menu: 3-dot menu in room actions for secondary actions (favourite, copy link, pins, threads, files, GIFs, notices), primary actions (search, members, info) stay visible
- Room notifications: per-room mute/mentions/all via `client.notification_settings()`, `/mute` and `/unmute` commands, toggle in room settings
- Room aliases: display, add, remove in room settings modal via `create_alias`/`delete_alias` API
- Message forwarding: `forward_message()` sends content to target room, ForwardModal with searchable room picker, `/forward` command
- Favourites: `room.set_is_favourite()` / `room.is_favourite()` via Matrix tags, star toggle in overflow menu, dedicated sidebar section, rooms also appear in their original section
- Files HUD: scans messages for `image_mxc`, shows thumbnails from media cache, click to scroll+highlight
- Topic bar: IRC-style slim bar below tabs showing room topic
- HudPanel: reusable wrapper component for Pins/Threads/Files/Notices HUDs with consistent chrome, Escape key handling, auto-focus, item cap (50)
- Search: result dropdown with click-to-scroll, `data-event-id` attributes on messages, `search-highlight` CSS class, client-side fallback for encrypted rooms, arrow key navigation
- Polls: MSC3381 unstable events (`org.matrix.msc3381.poll.start/response/end`). `PollState::results()` from matrix-sdk-ui provides aggregated votes. `PollDisplay` component renders interactive vote UI with progress bars. Voting sends `UnstablePollResponseEventContent` via Timeline.
- IRC features: join/part messages with colored arrows (green -->/red <--), mode badges in status bar (@/+), lag meter (30s server ping), Alt+1-9 tab switching, Tab-cycling command completion, server notices panel
- Display toggles: `/timestamp [short|full|off]`, `/compact` mode, `/joinpart` visibility — all persisted to SQLite
- Highlight words: `/highlight`/`/unhighlight` with persisted keyword list, matching messages get red-tinted background
- Activity indicators: red badges for mentions (highlight_count from SDK), blue for regular unread
- Space avatars: lookup by space_id (not name) to handle rooms in multiple spaces correctly
- All SVG icons (Lucide-style) — zero emoji characters in the UI
- Clickable nicks/avatars in messages open profile modal
- State events (power level changes, room name/topic/avatar) rendered as system messages
- Profile modal prioritizes current room for member lookup (correct power levels per room)

## Build & Run
```bash
cargo run -p fluxchat
```
- Requires GPU (wgpu). Works on Linux (Wayland/X11), macOS, Windows.
- No WebView deps needed.

## App Architecture (`crates/app/`)
- **Elm architecture:** `FluxChat` state struct → `Message` enum → `update(&mut self, msg) -> Task<Message>` → `view(&self) -> Element<Message>`
- **Files:** main.rs (entry), state.rs (FluxChat struct + enums), message.rs (Message/SyncEvent), update.rs (all state transitions), view.rs (all rendering), html.rs (Matrix HTML→Spans), commands.rs (slash commands), theme.rs (colors)
- **HTML rendering:** Custom parser converts Matrix HTML to `Vec<Span>` for `rich_text` widget. Handles bold/italic/strike/code/links/mentions/blockquotes. Syntax highlighting via syntect.
- **Modals:** `stack` + `opaque` + `mouse_area` overlay pattern. Modal enum dispatches to Settings/CreateRoom/Profile/Verification/Lightbox/Forward.
- **Commands:** 37 slash commands producing `CommandResult` (Messages/Rewrite/Help/Noop). Decoupled from UI — returns Message variants.
- **Subscriptions:** `time::every(5s)` for room list refresh, `keyboard::listen()` for Alt+1-9 + Escape.
- **Async:** Room commands (join/leave/invite/kick/ban/topic), file upload, search, member loading all via `Task::perform()`.
- **Status:** Login, room list, messages, tabs, reactions, member list, search, modals all implemented. Needs: timeline wiring (messages don't load yet), media cache for images, testing.

## Important Patterns
- Elm architecture: all state changes go through `Message` enum → `update()` → returns `Task<Message>`
- `Task::perform()` for async operations, `Task::batch()` to combine multiple tasks
- Timeline diff stream runs via channel + subscription, converts TimelineItem → MessageInfo
- Diff stream watcher also syncs open thread tabs by scanning `messages_by_room` keys starting with `thread::`
- Avatar resolution uses sync media cache lookup + background async batch resolve for uncached MXC URIs
- Scrollable with `anchor_bottom()` for natural bottom-anchored scrolling
- `read_rooms` HashSet persisted to SQLite settings to survive app restarts
- `nick_color_index` lives in `theme.rs` for shared access
- Thread replies detected via `thread_root` field on `MsgLikeContent`, not `thread_summary` (SDK doesn't populate summary)
- Thread reply counts derived by scanning messages for matching `thread_root` references
- Named constants: `MEDIA_CACHE_LIMIT`, `MEDIA_CACHE_EVICT`, `MESSAGE_PAGE_SIZE`, `RECEIPT_EVENT_LIMIT`, `NICK_COLOR_COUNT`
- Zero clippy warnings — all code passes `cargo clippy` clean

## Timeline API Usage
- `get_or_create_timeline(client, timelines, room_id)` creates/caches Timeline per room
- `create_thread_timeline(client, room_id, thread_root_id)` creates thread-focused Timeline
- `timeline.subscribe()` returns initial items + diff stream
- `timeline_item_to_message_info()` converts SDK types to our MessageInfo (extracts thread_root, thread_count, polls, stickers)
- Diff stream watcher task keeps `messages_by_room` updated via channel
- `timeline.paginate_backwards(20)` for scroll-up pagination
- `timeline.send()`, `send_reply()`, `edit()`, `toggle_reaction()`, `redact()` for message actions
- `timeline.mark_as_read()` for read receipts
- Event cache enabled in auth: `client.event_cache().subscribe()`
- Thread replies sent with `m.thread` relation via `room.send()` (not Timeline send_reply)
- Poll votes sent as `AnyMessageLikeEventContent::UnstablePollResponse` via `timeline.send()`

## Slash Command System
- Registry in `crates/app/src/commands.rs`
- `SlashCommand` struct: name, usage, description, scope (Global/Room/TextRewrite), execute fn
- `dispatch()` looks up and executes, `autocomplete_entries()` feeds the popup
- `CommandAction` enum: Handled, Rewrite(String), ShowHelp(String), Noop
- Tab-cycling autocomplete: repeated Tab presses cycle through matches, typing resets
- 48+ commands: /join /leave /part /invite /kick /ban /topic /op /deop /nick /me /msg /attach /forward /whois /query /ignore /unignore /users /directory /threads /notices /mute /unmute /status /highlight /unhighlight /timestamp /compact /joinpart /poll /pollend /gif /giphy /giphykey /slap /troutslap /shrug /tableflip /unflip /lenny /spoiler /settings /sessions /security /account /away /quit /clear /help

## Settings Modal
- Sidebar-navigated: /account, /sessions, /security tabs
- Account: profile editing (name, avatar, banner), logout
- Sessions: device list with SVG verification icons, last seen, remote sign-out with UIAA
- Security: cross-signing status with SVG icons, key backup enable/recover, recovery key display
- Status bar "verified"/"unverified" clickable → opens security tab

## Performance Notes
- Member list lazy-loads: 30 initially, more on scroll. Avatars resolved in background batch.
- Media cache capped at `MEDIA_CACHE_LIMIT` (500) entries with `MEDIA_CACHE_EVICT` (100) eviction.
- Avatar cache stores `image::Handle` (cheap Arc clone) to avoid re-decoding on every render.
- Inline image cache persisted to SQLite (base64) for instant display on restart.
- syntect `SyntaxSet`/`ThemeSet` loaded once via `LazyLock` statics.
- Message rendering limited to last 50 messages per room (before virtual scrolling).

## Code Quality
- Zero clippy warnings
- Named constants for all magic numbers
- `html_escape()` for XSS prevention
- `extract_message_content()` shared helper eliminates message parsing duplication
- `parse_room_id()`/`parse_user_id()` helpers eliminate error mapping duplication
- `tracing::warn!` on critical silent failures (encryption, session deletion)

## Features Implemented
Login (password + SSO/OIDC with provider detection), session persistence, rooms grouped by space, spaces sidebar with overview, channel tabs (Chrome-style with memoized DOM preservation, DM tabs without # prefix, Alt+1-9 switching), messaging via Timeline API with local echo and word-bounded mention resolution, formatted HTML messages with pre-computed mention transforms, inline images with lightbox and save dialog, full emoji picker (7 categories, search, frequently-used), reactions via Timeline toggle_reaction, replies with context preview, message editing via Timeline edit (up-arrow to edit last message), message deletion via Timeline redact, message forwarding with room picker modal, pinned messages (HUD), server-side search (HUD with clickable result list, scroll-to-highlight, arrow keys, client-side fallback), threads (indicators, HUD, dedicated tabs with live updates, proper m.thread relation), typing indicators (send + display), URL previews with og:image thumbnails, sticker display (m.sticker events), polls (create/vote/end with progress bars via MSC3381), modular slash commands (45+) with Tab-cycling autocomplete and /help, @mention autocomplete with inline avatar pills and Tab completion, contenteditable input with multiline (Shift+Enter, recursive div/p extraction), markdown formatting (**bold** *italic* ~~strike~~ `code` ```codeblocks``` with language tags), syntax highlighting via syntect (100+ languages, base16-ocean.dark), file upload (picker + drag-drop + paste + /attach) with thumbnail generation for images, files HUD with image previews, read receipt display (stacked avatars per message via load_event_receipts), read receipts sent via Timeline mark_as_read with persistent unread tracking, unread indicators (numeric sidebar badges, red for mentions / blue for regular, message divider, floating pill), device verification UI (SAS emoji), key backup and recovery (enable/recover/display), profile modal with ignore button and power level management (promote/demote), clickable nicks/avatars in messages, power level badges in member list, /op and /deop commands, custom user status (/status, display in member list + profile modal), profile editing (name, avatar, banner), settings modal (account/sessions/security with SVG icons), room creation with public/encrypted toggles, room settings (name/topic/aliases/notifications/invite/leave), room notification modes (all/mentions/mute), room alias management (add/remove), room invitations (accept/reject in sidebar), room upgrade/tombstone detection with follow button, space overview with channel list and invite, back-pagination via Timeline paginate_backwards, ignore/block users (/ignore /unignore with message filtering), user directory search (/users), public room directory (/directory), presence/away status, favourite rooms (star toggle, dedicated sidebar section + original section), IRC-style topic bar, IRC join/part with colored arrows, mode badges in status bar, lag meter (30s ping), copy room link to clipboard, highlight words with visual indicators, display toggles (timestamp/compact/joinpart), server notices panel, state event rendering (power levels, room name/topic/avatar changes), cross-platform window decorations, all SVG icons (zero emojis), collapsible room list sections with space avatars by ID, DM person icon in sidebar, invite icon in member list header, reusable HudPanel/ForwardModal/PollDisplay components, 3-dot overflow menu for secondary room actions, GIF search via Giphy API (/gif /giphy commands + overflow menu, GifsHud with grid picker, click to download+upload as image, /giphykey for API key persistence)

## Features Pending
- Emoji picker (full grid with categories, search, frequently-used)
- GIF search UI (Giphy grid picker)
- Room creation form
- Multiline input (switch from text_input to TextEditor widget)
- Desktop notifications
- Virtual scrolling (for very large rooms)
- Keyboard shortcuts (Ctrl+K room switcher)
- Voice/Video calls (VoIP)
- Voice messages
- Location sharing (MSC3488)
- Spaces hierarchy (deep recursive browsing)
- Video playback (system player for now)

## Known Issues
- Thread summary (thread_count) not populated by SDK — we derive counts from thread_root references
- Input is single-line (Iced text_input limitation) — multiline editing not yet supported
