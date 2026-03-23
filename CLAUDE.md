# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI built with Dioxus. Uses matrix-rust-sdk + matrix-sdk-ui for the Matrix protocol and Timeline API.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. Matrix operations, Timeline management, message conversion.
  - `crates/app/` — Dioxus desktop frontend with IRC-inspired design
- **Timeline API**: `matrix-sdk-ui` Timeline per room handles loading, pagination, live updates, local echo, edits, reactions, redactions.
- **Storage:** rusqlite for app data (sessions, media cache, settings, read rooms, emoji frequency), matrix-sdk's built-in SQLite for crypto/sync/event cache
- **Sync:** Background sync loop with `SyncCallbacks` trait for typing + room list updates. Message/edit/reaction handling moved to Timeline diff stream.
- **State:** Dioxus `Signal`s for reactive UI, `Arc<AppState>` for core state including `timelines: Mutex<HashMap<RoomId, Arc<Timeline>>>`

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js (minimal inline JS via `document::eval()` for WebView interop: scroll detection, drag-drop, paste, mention clicks)
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace fonts throughout, status bar, channel tabs
- Timeline API for all message operations (not manual /messages API)
- Component isolation for performance: TabBar, ActiveRoomContent, RoomMessages are separate components to prevent cascading re-renders
- DOM preservation: all open room components stay mounted with `display: none`, tab switching is a CSS toggle
- Dioxus memoization: RoomMessages only receives `room_id` prop (never changes on tab switch)

## Build & Run
```bash
cargo run -p fluxchat-app
```
- **macOS:** No additional deps (uses WKWebView). Native window decorations enabled via `cfg!(target_os = "macos")`.
- **Linux:** webkit2gtk-4.1, gtk3, libsoup3, xdotool

## Important Patterns
- Dioxus signals need `mut` to write: `let mut ui = use_context::<UiState>()`
- Don't use `use_effect` inside conditionals — Dioxus hooks must be called unconditionally
- Don't read/write Dioxus Signals from background threads — use channels instead
- Use `.peek()` instead of `.read()` for non-subscribing signal reads (avoids re-render cascades)
- Component isolation: separate components for tab bar, message content, room messages to prevent signal cascading
- Timeline diff stream runs in a spawned task, converts TimelineItem → MessageInfo, writes to `messages_by_room` signal
- Avatar resolution uses sync media cache lookup + background async batch resolve for uncached MXC URIs
- CSS `flex-direction: column-reverse` on scroll container for natural bottom-anchored scrolling
- JS→Rust communication via `document::eval` + `dioxus.send()` channel (stored globally as `window.__fluxchat_dioxus`)
- `read_rooms` HashSet persisted to SQLite settings to survive app restarts
- `transform_mentions` and `nick_color_index` live in `state.rs` for shared access

## Timeline API Usage
- `get_or_create_timeline(client, timelines, room_id)` creates/caches Timeline per room
- `timeline.subscribe()` returns initial items + diff stream
- `timeline_item_to_message_info()` converts SDK types to our MessageInfo
- Diff stream watcher task (spawned in `select_room`) keeps `messages_by_room` signal updated
- `timeline.paginate_backwards(20)` for scroll-up pagination (triggered by JS scroll listener)
- `timeline.send()`, `send_reply()`, `edit()`, `toggle_reaction()`, `redact()` for message actions
- `timeline.mark_as_read()` for read receipts
- Event cache enabled in auth: `client.event_cache().subscribe()`

## Performance Notes
- Component isolation prevents tab bar changes from re-rendering messages
- All open rooms render to DOM with `display: none` — tab switch is CSS toggle, not DOM rebuild
- Dioxus memoizes RoomMessages by `room_id` prop — skips re-render when prop unchanged
- Member list lazy-loads: 30 initially, more on scroll. Avatars resolved in background batch.
- Media cache capped at 500 entries with eviction.
- Tab close frees messages, members, and Timeline objects in background spawn.
- `peek()` used for non-critical signal reads to avoid subscription overhead.

## Features Implemented
Login, session persistence, rooms grouped by space, spaces sidebar with overview, channel tabs (Chrome-style with memoized DOM preservation), messaging via Timeline API with local echo, formatted HTML messages with pre-computed mention transforms, inline images with lightbox and save dialog, full emoji picker (7 categories, search, frequently-used), reactions via Timeline toggle_reaction, replies with context preview, message editing via Timeline edit, message deletion via Timeline redact, pinned messages (HUD), search (HUD), typing indicators (send + display), URL previews, slash commands with autocomplete (/join /leave /invite /kick /ban /topic /nick /me /shrug /tableflip /lenny), file upload (picker + drag-drop + paste), read receipts via Timeline mark_as_read with persistent unread tracking, device verification UI (SAS emoji), profile modal, profile editing (name, avatar, banner), settings modal with logout, room creation with public/encrypted toggles, room settings (name/topic/invite/leave), space overview with channel list and invite, back-pagination via Timeline paginate_backwards, cross-platform window decorations, SVG icons (Lucide-style)

## Features Pending
Threads UI, desktop notifications, virtual scrolling
