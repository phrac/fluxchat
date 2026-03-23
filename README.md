# fluxchat

A cross-platform Matrix chat client with an IRC-inspired UI, built entirely in Rust with Dioxus.

> **Note:** This project was vibecoded with [Claude Code](https://claude.ai/claude-code). The entire codebase — core library, Dioxus frontend, and all configuration — was generated through conversational AI pair programming.

## Screenshot

![fluxchat screenshot](assets/screenshot-v2.png)

## Design

fluxchat blends IRC heritage with modern chat UX — not a Discord clone, not a terminal client, something unique.

- **IRC-style messages** — compact layout with right-aligned colored nicks, vertical separator, and monospace timestamps
- **Channel tabs** — Chrome-style tabs across the top for quick room switching with space context
- **Status bar** — bottom bar showing `user@server │ #channel │ nicks │ encrypted │ synced`
- **Nick colors** — deterministic vibrant colors, the signature IRC visual
- **Monospace throughout** — messages, timestamps, nicks, status bar in JetBrains Mono / system monospace
- **SVG icons** — cross-platform inline SVGs for all actions (Lucide-style)
- **Deep charcoal theme** — dark, warm, easy on the eyes

## Status

This is an **MVP** with active development. Core chat functionality works, with more features being ported.

### Current Features

- **Login & session persistence** — Any Matrix homeserver. Sessions saved in SQLite, restored on relaunch.
- **Room list** — Grouped by space with section headers, room avatars, unread badges, DMs separated.
- **Spaces** — Space sidebar with resolved avatars. Clicking filters rooms.
- **Messaging** — Send/receive in real-time via Matrix sync with optimistic local insertion.
- **Message history** — Lazy loading with automatic back-pagination on scroll-up.
- **Formatted messages** — Renders HTML `formatted_body` from Matrix events.
- **Inline images** — Image attachments render inline with async resolution and 3-tier cache.
- **Image lightbox** — Click images to view full-size with save-to-disk via native dialog.
- **Reactions** — Emoji picker on hover, reaction tooltips showing who reacted, reactions below messages.
- **Reply & edit** — Reply to messages with context preview, edit own messages with inline compose.
- **Message deletion** — Delete own messages with optimistic local removal.
- **Pinned messages** — Pin/unpin messages, view pins in a dropdown HUD panel.
- **Mention pills** — Matrix user mentions rendered as styled pills with avatars, click to view profile.
- **File upload** — Attach files via `+` button, drag-and-drop, or clipboard paste (images).
- **Search** — In-room message search via dropdown HUD with result navigation.
- **Channel tabs** — Chrome-style tabs that flex to fill, instant switching via CSS show/hide.
- **Nick colors** — Deterministic vibrant colors per user.
- **Member list** — Lazy-loaded, grouped by role with presence dots and avatars.
- **Profile modal** — Click any user mention or member to view profile with avatar, role, presence.
- **Settings modal** — Gear icon opens settings with account info and logout.
- **Device verification** — Incoming verification requests show SAS emoji grid for cross-signing.
- **Read receipts** — Sent on room enter and incoming messages, persisted across restarts.
- **Status bar** — User avatar, gear icon, room info, encryption status, sync status, device verification state with session ID.
- **Cross-platform** — Native window decorations on macOS, borderless on Linux.
- **End-to-end encryption** — E2EE via matrix-sdk with SQLite crypto store.
- **Media caching** — 3-tier cache: memory → SQLite → network, capped at 500 entries.
- **Memory management** — 200 message cap per room, lazy member list, tab close frees data.

### Not Yet Implemented

- Thread view UI
- Room creation and settings
- User profile editing
- URL previews
- Desktop notifications
- Typing indicator display (backend ready)

## Architecture

```
fluxchat/
├── Cargo.toml              # Workspace
├── crates/
│   ├── core/               # Business logic — pure Rust, no UI deps
│   │   └── src/
│   │       ├── auth.rs     # Login, logout, session restore
│   │       ├── messages.rs # Send, fetch, edit, delete, pin, reactions
│   │       ├── rooms.rs    # Room CRUD, spaces, kick/ban, power levels
│   │       ├── media.rs    # Media download with 3-tier cache
│   │       ├── sync.rs     # Sync loop with SyncCallbacks trait
│   │       ├── db.rs       # SQLite storage (rusqlite)
│   │       └── ...         # profile, preview, threads, verification
│   └── app/                # Dioxus desktop frontend
│       └── src/
│           ├── app.rs      # Root component, sync bridge
│           ├── state.rs    # Reactive signals wrapping core
│           └── components/ # IRC-inspired UI components
└── assets/
```

**Pure Rust** — no JavaScript, no TypeScript, no Node.js. The core library has zero UI dependencies and can be reused with any Rust frontend framework.

## Tech Stack

- **UI:** [Dioxus](https://dioxuslabs.com/) 0.7 (desktop via webview)
- **Matrix SDK:** [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk)
- **Storage:** [rusqlite](https://github.com/rusqlite/rusqlite) (app data) + SQLite (matrix-sdk crypto/sync)
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

First build compiles ~400 crates. Subsequent builds are fast.

### Production Build

```bash
cargo build -p fluxchat-app --release
```

## License

MIT
