# fluxchat

A cross-platform Matrix chat client with an IRC-inspired UI, built entirely in Rust with Dioxus.

> **Note:** This project was vibecoded with [Claude Code](https://claude.ai/claude-code). The entire codebase — core library, Dioxus frontend, and all configuration — was generated through conversational AI pair programming.

## Screenshot

![fluxchat screenshot](assets/screenshot-v2.png)

## Design

fluxchat blends IRC heritage with modern chat UX — not a Discord clone, not a terminal client, something unique.

- **IRC-style messages** — compact layout with right-aligned colored nicks, vertical separator, and monospace timestamps
- **Channel tabs** — Chrome-style tabs across the top for quick room switching with space context
- **Status bar** — bottom bar showing `⚙ avatar user@server │ #channel │ nicks │ encrypted │ synced │ ✓ verified [device]`
- **Nick colors** — deterministic vibrant colors, the signature IRC visual
- **Monospace throughout** — messages, timestamps, nicks, status bar in JetBrains Mono / system monospace
- **SVG icons** — cross-platform inline SVGs for all actions (Lucide-style)
- **Deep charcoal theme** — dark, warm, easy on the eyes

## Status

This is an **MVP** with active development. Core chat functionality works.

### Current Features

- **Login & session persistence** — Any Matrix homeserver. Sessions saved in SQLite, restored on relaunch.
- **Room list** — Grouped by space with section headers, room avatars, unread badges, DMs separated.
- **Spaces** — Space sidebar with resolved avatars. Clicking filters rooms and shows space overview.
- **Messaging** — Send/receive in real-time via matrix-sdk-ui Timeline API with local echo.
- **Message history** — Automatic back-pagination on scroll-up via Timeline API.
- **Formatted messages** — Renders HTML `formatted_body` with pre-computed mention transforms.
- **Inline images** — Image attachments render inline with async resolution and 3-tier cache.
- **Image lightbox** — Click images to view full-size with native save-to-disk dialog.
- **Reactions** — Full emoji picker (categorized, searchable, frequently-used tracking), reaction tooltips.
- **Reply & edit** — Reply to messages with context preview, edit own messages with inline compose.
- **Message deletion** — Delete own messages via Timeline redact API.
- **Pinned messages** — Pin/unpin messages, view pins in dropdown HUD panel.
- **Mention pills** — Matrix user mentions rendered as styled pills with avatars, click to view profile.
- **File upload** — Attach files via `+` button, drag-and-drop, or clipboard paste (images).
- **Search** — In-room message search via dropdown HUD with result navigation.
- **URL previews** — Automatic link previews fetched from the Matrix server's preview API.
- **Typing indicators** — Sends typing notifications while composing, displays who's typing in each room.
- **Slash commands** — `/join`, `/leave`, `/invite`, `/kick`, `/ban`, `/topic`, `/nick`, `/me`, `/shrug`, `/tableflip`, `/lenny` with autocomplete popup.
- **Emoji picker** — Discord-style picker with 7 categories, search, frequently-used (persisted to SQLite).
- **Channel tabs** — Chrome-style tabs that flex to fill, instant switching via CSS display toggle with memoized components.
- **Nick colors** — Deterministic vibrant colors per user.
- **Member list** — Lazy-loaded (30 at a time), grouped by role with presence dots and avatars.
- **Profile modal** — Click any user mention or member to view profile with avatar, role, presence.
- **Profile editing** — Change display name, upload/remove avatar and banner in settings.
- **Settings modal** — Gear icon opens settings with profile editing, appearance, and logout.
- **Room creation** — Create rooms with name, topic, public/private, encryption toggles. Join rooms by alias or ID.
- **Room settings** — Edit room name/topic, invite users, leave room.
- **Space overview** — View space channels, invite users to space.
- **Device verification** — Incoming verification requests show SAS emoji grid for cross-signing.
- **Read receipts** — Sent via Timeline API, unread state persisted across restarts.
- **Status bar** — User avatar, gear icon, room info, encryption status, sync status, device verification state with session ID.
- **Cross-platform** — Native window decorations on macOS, borderless on Linux.
- **End-to-end encryption** — E2EE via matrix-sdk with SQLite crypto store.
- **Media caching** — 3-tier cache: memory → SQLite → network, capped at 500 entries.
- **Memory management** — Tab close frees messages, members, and Timeline objects in background.

### Not Yet Implemented

- Thread view UI
- Desktop notifications
- Virtual scrolling (for very large rooms)
- Keyboard shortcuts (Ctrl+K room switcher, etc.)
- Notification badges on spaces (aggregate unread counts)
- Right-click context menus
- Multi-account support
- System tray (minimize to tray, badge)
- Room directory (browse public rooms)
- Room avatars in room list

## Architecture

```
fluxchat/
├── Cargo.toml              # Workspace
├── crates/
│   ├── core/               # Business logic — pure Rust, no UI deps
│   │   └── src/
│   │       ├── auth.rs     # Login, logout, session restore, event cache init
│   │       ├── timeline.rs # Timeline API: per-room Timeline management,
│   │       │               # TimelineItem → MessageInfo conversion
│   │       ├── messages.rs # Send, fetch, edit, delete, pin, reactions (legacy)
│   │       ├── rooms.rs    # Room CRUD, spaces, kick/ban, power levels
│   │       ├── media.rs    # Media download with 3-tier cache
│   │       ├── sync.rs     # Sync loop (typing, room list, verification)
│   │       ├── db.rs       # SQLite storage (sessions, media cache, settings)
│   │       ├── state.rs    # AppState with client, db, media cache, timelines
│   │       └── ...         # profile, preview, threads, verification
│   └── app/                # Dioxus desktop frontend
│       └── src/
│           ├── app.rs      # Root component, sync bridge (typing + room list)
│           ├── state.rs    # UiState signals, transform_mentions, emoji data
│           └── components/
│               ├── layout.rs       # MainLayout, modals (settings, profile,
│               │                   # verification, room settings, create room,
│               │                   # lightbox), status bar, device status
│               ├── chat/
│               │   ├── message_area.rs  # MessageArea (shell + JS bridge),
│               │   │                    # TabBar, ActiveRoomContent,
│               │   │                    # RoomMessages, ChannelTab, InlineImage,
│               │   │                    # UrlPreviewCard, HUDs (pins, search),
│               │   │                    # RoomActions, SpaceOverview
│               │   ├── message_input.rs # MessageInput with slash commands,
│               │   │                    # file upload, emoji button, typing
│               │   └── emoji_picker.rs  # EmojiPicker with categories, search,
│               │                        # frequently-used tracking
│               ├── sidebar/
│               │   ├── space_sidebar.rs # Space icons with avatars
│               │   └── room_list.rs     # Room list, select_room with Timeline
│               │                        # subscribe + diff stream watcher
│               └── members/
│                   └── member_list.rs   # Lazy-loaded member list with avatars
└── assets/
    └── style.css           # IRC-inspired dark theme with CSS variables
```

### Timeline API Architecture

fluxchat uses `matrix-sdk-ui`'s Timeline API for all message operations:

```
┌─────────────┐     ┌──────────────┐     ┌────────────────┐
│ select_room │────▶│  Timeline    │────▶│ subscribe()    │
│             │     │  Builder     │     │ → items        │
└─────────────┘     └──────────────┘     │ → diff stream  │
                                         └───────┬────────┘
                                                 │
                    ┌──────────────┐     ┌───────▼────────┐
                    │ messages_by  │◀────│ diff watcher   │
                    │ _room signal │     │ task (spawned) │
                    └──────┬───────┘     └────────────────┘
                           │
                    ┌──────▼───────┐
                    │ RoomMessages │  (memoized by room_id)
                    │ component    │
                    └──────────────┘
```

- **Loading**: `TimelineBuilder::new(&room).build()` → `timeline.subscribe()` → initial items + live stream
- **Live updates**: Diff stream watcher converts `TimelineItem` → `MessageInfo` on every change
- **Pagination**: `timeline.paginate_backwards(20)` — diff stream delivers new items automatically
- **Send/Reply/Edit**: `timeline.send()`, `timeline.send_reply()`, `timeline.edit()` with local echo
- **React/Delete**: `timeline.toggle_reaction()`, `timeline.redact()`
- **Read receipts**: `timeline.mark_as_read()`

### Performance Architecture

- **Component isolation**: `MessageArea` → `TabBar` + `ActiveRoomContent` + `RoomMessages`. Tab bar changes don't cascade into message re-renders.
- **DOM preservation**: All open room components stay mounted with `display: none`. Tab switching toggles CSS, not DOM rebuilds.
- **Dioxus memoization**: `RoomMessages` receives only `room_id` (never changes on tab switch), so Dioxus skips re-rendering.
- **Signal isolation**: `peek()` for non-subscribing reads, `read()` only where re-render is needed.
- **Lazy member list**: Renders 30 members initially, loads more on scroll. Avatars resolved in background batch.
- **Background cleanup**: Tab close frees messages, members, and Timeline objects in a spawned task.

**Pure Rust** — no JavaScript, no TypeScript, no Node.js. The core library has zero UI dependencies and can be reused with any Rust frontend framework. Minimal inline JS is used via Dioxus's `document::eval()` for WebView-specific operations (scroll detection, file drag-drop, clipboard paste).

## Tech Stack

- **UI:** [Dioxus](https://dioxuslabs.com/) 0.7 (desktop via webview)
- **Matrix SDK:** [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) 0.16
- **Timeline:** [matrix-sdk-ui](https://github.com/matrix-org/matrix-rust-sdk) 0.16 (Timeline API)
- **Storage:** [rusqlite](https://github.com/rusqlite/rusqlite) (app data) + SQLite (matrix-sdk crypto/sync/event cache)
- **E2EE:** vodozemac (native Rust, same as Element X)

## Building

### Prerequisites

- Rust 1.77+
- System dependencies for Dioxus desktop:
  - **macOS:** No additional dependencies (uses WKWebView)
  - **Arch:** `sudo pacman -S webkit2gtk-4.1 gtk3 libsoup3 xdotool`
  - **Ubuntu/Debian:** `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libxdo-dev`

### Development

```bash
git clone https://github.com/phrac/fluxchat.git
cd fluxchat
cargo run -p fluxchat-app
```

First build compiles ~500 crates. Subsequent builds are fast.

### Production Build

```bash
cargo build -p fluxchat-app --release
```

## License

MIT
