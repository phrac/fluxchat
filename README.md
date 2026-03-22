# fluxchat

A cross-platform Matrix chat client with an IRC-inspired UI, built entirely in Rust with Dioxus.

> **Note:** This project was vibecoded with [Claude Code](https://claude.ai/claude-code). The entire codebase — core library, Dioxus frontend, and all configuration — was generated through conversational AI pair programming.

## Screenshot

![fluxchat screenshot](assets/screenshot.png)

## Design

fluxchat blends IRC heritage with modern chat UX — not a Discord clone, not a terminal client, something unique.

- **IRC-style messages** — compact layout with right-aligned colored nicks, vertical separator, and monospace timestamps
- **Channel tabs** — Chrome-style tabs across the top for quick room switching with space context
- **Status bar** — bottom bar showing `user@server │ #channel │ nicks │ encrypted │ synced`
- **Nick colors** — deterministic vibrant colors, the signature IRC visual
- **Monospace accents** — timestamps, nicks, status bar in JetBrains Mono; body text in Inter
- **Deep charcoal theme** — dark, warm, easy on the eyes

## Status

This is an **MVP** with active development. Core chat functionality works, with more features being ported.

### Current Features

- **Login & session persistence** — Any Matrix homeserver. Sessions saved in SQLite, restored on relaunch.
- **Room list** — Grouped by space with section headers, room avatars, DMs separated.
- **Spaces** — Space sidebar with resolved avatars. Clicking filters rooms.
- **Messaging** — Send/receive in real-time via Matrix sync with optimistic local insertion.
- **Message history** — Backward pagination on scroll.
- **Formatted messages** — Renders HTML `formatted_body` from Matrix events.
- **Channel tabs** — Recently visited rooms as Chrome-style tabs with space context and close buttons.
- **Nick colors** — Consistent, vibrant colors per user.
- **Member list** — Grouped by role (Admin/Mod/Member) with presence dots and avatars.
- **Status bar** — IRC-style connection info at the bottom.
- **End-to-end encryption** — E2EE via matrix-sdk with SQLite crypto store.
- **Media caching** — 3-tier cache: memory → SQLite → network.

### Not Yet Implemented (porting from v1)

- Reactions and emoji picker
- Reply and thread support
- Message editing and deletion
- File uploads and image display
- Pinned messages
- Typing indicators (backend ready, UI pending)
- Room creation and settings
- User profile editing
- Device verification UI
- URL previews
- Search
- Desktop notifications

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
