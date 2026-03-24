# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI built with Dioxus. Uses matrix-rust-sdk + matrix-sdk-ui for the Matrix protocol and Timeline API.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. Matrix operations, Timeline management, message conversion.
  - `crates/app/` — Dioxus desktop frontend with IRC-inspired design
- **Timeline API**: `matrix-sdk-ui` Timeline per room handles loading, pagination, live updates, local echo, edits, reactions, redactions. Thread timelines via `TimelineFocus::Thread`.
- **Storage:** rusqlite for app data (sessions, media cache, settings, read rooms, emoji frequency), matrix-sdk's built-in SQLite for crypto/sync/event cache
- **Sync:** Background sync loop with `SyncCallbacks` trait for typing + room list updates. Message/edit/reaction handling moved to Timeline diff stream.
- **State:** Dioxus `Signal`s for reactive UI, `Arc<AppState>` for core state including `timelines: Mutex<HashMap<RoomId, Arc<Timeline>>>`

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js (minimal inline JS via `document::eval()` for WebView interop: scroll detection, drag-drop, paste, mention clicks, contenteditable input)
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace fonts throughout, status bar, channel tabs
- Timeline API for all message operations (not manual /messages API)
- Component isolation for performance: TabBar, ActiveRoomContent, RoomMessages are separate components to prevent cascading re-renders
- DOM preservation: all open room components stay mounted with `display: none`, tab switching is a CSS toggle
- Dioxus memoization: RoomMessages only receives `room_id` prop (never changes on tab switch)
- Modular slash command system in `commands.rs` — add new commands by adding entries to `all_commands()`
- Thread tabs use synthetic room IDs (`thread::{room_id}::{event_id}`) stored in `messages_by_room`
- Mention resolution: display names in outgoing messages converted to `matrix.to` HTML links at send time
- Contenteditable input: DOM is source of truth, Rust `text` Signal is plain-text mirror synced via JS interop. Mention pills are non-editable spans with `data-user-id`/`data-display-name`. `__fluxExtractText()` extracts plain text from pills for the signal. `__fluxInsertPill()` inserts avatar pills at cursor.
- Markdown formatting: `build_message_content()` converts `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, ``` ```codeblocks``` ``` to HTML at send time via `simple_markdown_to_html()`

## Build & Run
```bash
cargo run -p fluxchat-app
```
- **macOS:** No additional deps (uses WKWebView). Native window decorations enabled via `cfg!(target_os = "macos")`.
- **Linux:** webkit2gtk-4.1, gtk3, libsoup3, xdotool

## Important Patterns
- Dioxus signals need `mut` to write: `let mut ui = use_context::<UiState>()`
- Don't use `use_effect` inside conditionals — Dioxus hooks must be called unconditionally
- Don't call `use_context` inside conditional RSX blocks — use `consume_context` instead
- Don't read/write Dioxus Signals from background threads — use channels instead
- Use `.peek()` instead of `.read()` for non-subscribing signal reads (avoids re-render cascades)
- Component isolation: separate components for tab bar, message content, room messages to prevent signal cascading
- Timeline diff stream runs in a spawned task, converts TimelineItem → MessageInfo, writes to `messages_by_room` signal
- Diff stream watcher also syncs open thread tabs by scanning `messages_by_room` keys starting with `thread::`
- Avatar resolution uses sync media cache lookup + background async batch resolve for uncached MXC URIs
- CSS `flex-direction: column-reverse` on scroll container for natural bottom-anchored scrolling
- JS→Rust communication via `document::eval` + `dioxus.send()` channel (stored globally as `window.__fluxchat_dioxus`)
- `read_rooms` HashSet persisted to SQLite settings to survive app restarts
- `transform_mentions` and `nick_color_index` live in `state.rs` for shared access
- Thread replies detected via `thread_root` field on `MsgLikeContent`, not `thread_summary` (SDK doesn't populate summary)
- Thread reply counts derived by scanning messages for matching `thread_root` references

## Timeline API Usage
- `get_or_create_timeline(client, timelines, room_id)` creates/caches Timeline per room
- `create_thread_timeline(client, room_id, thread_root_id)` creates thread-focused Timeline
- `timeline.subscribe()` returns initial items + diff stream
- `timeline_item_to_message_info()` converts SDK types to our MessageInfo (extracts thread_root, thread_count)
- Diff stream watcher task (spawned in `select_room`) keeps `messages_by_room` signal updated
- `timeline.paginate_backwards(20)` for scroll-up pagination (triggered by JS scroll listener)
- `timeline.send()`, `send_reply()`, `edit()`, `toggle_reaction()`, `redact()` for message actions
- `timeline.mark_as_read()` for read receipts
- Event cache enabled in auth: `client.event_cache().subscribe()`
- Thread replies sent with `m.thread` relation via `room.send()` (not Timeline send_reply)

## Slash Command System
- Registry in `crates/app/src/components/chat/commands.rs`
- `SlashCommand` struct: name, usage, description, scope (Global/Room/TextRewrite), execute fn
- `dispatch()` looks up and executes, `autocomplete_entries()` feeds the popup
- `CommandAction` enum: Handled, Rewrite(String), ShowHelp(String), Noop
- 30+ commands: /join /leave /part /invite /kick /ban /topic /nick /me /msg /attach /whois /query /ignore /unignore /users /directory /threads /slap /troutslap /shrug /tableflip /unflip /lenny /spoiler /settings /sessions /security /account /away /quit /clear /help

## Settings Modal
- Sidebar-navigated: /account, /sessions, /security tabs
- Account: profile editing (name, avatar, banner), logout
- Sessions: device list with verification status, last seen, remote sign-out with UIAA
- Security: cross-signing status, key backup enable/recover, recovery key display

## Performance Notes
- Component isolation prevents tab bar changes from re-rendering messages
- All open rooms render to DOM with `display: none` — tab switch is CSS toggle, not DOM rebuild
- Dioxus memoizes RoomMessages by `room_id` prop — skips re-render when prop unchanged
- Member list lazy-loads: 30 initially, more on scroll. Avatars resolved in background batch.
- Media cache capped at 500 entries with eviction.
- Tab close frees messages, members, and Timeline objects in background spawn.
- `peek()` used for non-critical signal reads to avoid subscription overhead.

## Features Implemented
Login (password + SSO/OIDC with provider detection), session persistence, rooms grouped by space, spaces sidebar with overview, channel tabs (Chrome-style with memoized DOM preservation), messaging via Timeline API with local echo and mention resolution, formatted HTML messages with pre-computed mention transforms, inline images with lightbox and save dialog, full emoji picker (7 categories, search, frequently-used), reactions via Timeline toggle_reaction, replies with context preview, message editing via Timeline edit (up-arrow to edit last message), message deletion via Timeline redact, pinned messages (HUD), server-side search (HUD with fallback to client-side), threads (indicators, HUD, dedicated tabs with live updates, proper m.thread relation), typing indicators (send + display), URL previews, modular slash commands (30+) with autocomplete and /help, @mention autocomplete with inline avatar pills and Tab completion, contenteditable input with multiline (Shift+Enter), markdown formatting (**bold** *italic* ~~strike~~ `code` ```codeblocks```), /command syntax coloring, file upload (picker + drag-drop + paste + /attach), read receipts via Timeline mark_as_read with persistent unread tracking, unread indicators (sidebar orbs + message divider + floating pill), device verification UI (SAS emoji), key backup and recovery (enable/recover/display), profile modal with ignore button, profile editing (name, avatar, banner), settings modal (account/sessions/security), room creation with public/encrypted toggles, room settings (name/topic/invite/leave), room invitations (accept/reject in sidebar), room upgrade/tombstone detection with follow button, space overview with channel list and invite, back-pagination via Timeline paginate_backwards, ignore/block users (/ignore /unignore with message filtering), user directory search (/users), public room directory (/directory), presence/away status, cross-platform window decorations, SVG icons (Lucide-style), collapsible room list sections with avatars

## Features Pending
- Desktop notifications
- Virtual scrolling (for very large rooms)
- Keyboard shortcuts (Ctrl+K room switcher, etc.)

## Known Issues
- macOS NSBeep sound — WKWebView issue, can't suppress from Rust
- Space active indicator in sidebar — tried multiple approaches, none fully work
- Thread summary (thread_count) not populated by SDK — we derive counts from thread_root references
