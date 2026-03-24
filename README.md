# fluxchat

A cross-platform Matrix chat client with an IRC-inspired UI, built entirely in Rust with Dioxus.

## Screenshot

![fluxchat screenshot](assets/screenshot-v2.png)

## Design

fluxchat blends IRC heritage with modern chat UX — not a Discord clone, not a terminal client, something unique.

- **IRC-style messages** — compact layout with right-aligned colored nicks, vertical separator, monospace timestamps
- **Channel tabs** — Chrome-style tabs for instant room switching with memoized DOM preservation
- **Command bar** — contenteditable input with multiline support, inline mention pills with avatars, markdown formatting, emoji picker, slash command autocomplete with syntax coloring
- **Status bar** — user avatar, room info, encryption status, sync state, device verification
- **Deep charcoal theme** — monospace throughout, SVG icons, deterministic nick colors

## Features

**Messaging** — Real-time send/receive via matrix-sdk-ui Timeline API with local echo, back-pagination on scroll, formatted HTML with mention pills, inline images with lightbox, reactions with emoji picker, replies with context, message editing and deletion (up-arrow to edit last message). Multiline input with Shift+Enter. Markdown formatting (**bold**, *italic*, ~~strike~~, `code`, ```codeblocks```). Read receipt avatars displayed per message. Unread indicators with numeric badge counts and floating "new messages" pill. Message forwarding with room picker modal.

**Threads** — Thread indicators on messages with reply counts. Click to open thread as a dedicated tab. Threads HUD dropdown lists all threads in a room. Thread replies sent with proper `m.thread` relation. Live updates via diff stream sync.

**Rooms & Spaces** — Room list grouped by space with collapsible sections and avatars, space sidebar, room creation with encryption toggles, room settings (name, topic, aliases, notifications, invite, leave), space overview. Room invitations with accept/reject. Room upgrade/tombstone detection with follow button. Favourite rooms with dedicated sidebar section. IRC-style topic bar below tabs. Copy room link to clipboard.

**Slash Commands** — Modular command registry with autocomplete and `/help`. Room commands (`/join`, `/leave`, `/part`, `/invite`, `/kick`, `/ban`, `/topic`, `/op`, `/deop`, `/mute`, `/unmute`), messaging (`/me`, `/msg`, `/attach`, `/forward`), user management (`/nick`, `/whois`, `/query`, `/ignore`, `/unignore`, `/status`), discovery (`/users`, `/directory`, `/threads`), text macros (`/shrug`, `/tableflip`, `/lenny`), settings (`/settings`, `/sessions`, `/security`, `/account`), and system (`/away`, `/quit`, `/clear`).

**Files & Media** — File upload via button, drag-drop, clipboard paste, or `/attach` with thumbnail generation for images. 3-tier media cache (memory, SQLite, network). URL previews with og:image thumbnails. Files HUD with image previews. Sticker display (`m.sticker` events).

**Security** — End-to-end encryption via matrix-sdk. SAS emoji verification. Key backup and recovery (enable, recover with key/passphrase). Session management with device listing, verification status, and remote sign-out. Cross-signing status display.

**Search** — Server-side message search via Matrix search API with fallback to client-side filtering. Search HUD with clickable result list, scroll-to-message highlighting, and arrow key navigation. User directory search (`/users`). Public room directory (`/directory`).

**Authentication** — Username/password login. SSO/OIDC support with auto-detection, provider-specific buttons, and browser-based auth flow. Session persistence and restore.

**User Management** — Ignore/block users (`/ignore`, `/unignore`) with message filtering. Profile modals with ignore button, power level display, and promote/demote actions. Power level badges in member list. Presence display, `/away` status, and custom `/status` messages shown in member list and profile modal.

**Settings** — Sidebar-navigated settings: `/account` (profile, avatar, banner), `/sessions` (device management, remote sign-out), `/security` (key backup, recovery, cross-signing status). Room settings with notification modes (all/mentions/mute), alias management, and room info panel. All accessible via slash commands.

**Code Display** — Syntax highlighting for code blocks via `syntect` with language detection (```rust, ```python, etc.) and 100+ supported languages. Base16 Ocean Dark theme. Plain text messages preserve newlines.

## Architecture

```
fluxchat/
├── crates/
│   ├── core/               # Business logic — pure Rust, no UI deps
│   │   └── src/
│   │       ├── auth.rs     # Login (password + SSO), logout, session restore
│   │       ├── timeline.rs # Timeline API: per-room + thread timeline management
│   │       ├── devices.rs  # Device listing, rename, remote sign-out
│   │       ├── messages.rs # Send, edit, delete, pin, reactions, server search
│   │       ├── rooms.rs    # Room CRUD, spaces, DMs, invites, user/room search
│   │       ├── media.rs    # Media download with 3-tier cache
│   │       ├── sync.rs     # Sync loop (typing, room list, verification)
│   │       ├── db.rs       # SQLite storage (sessions, media cache, settings)
│   │       └── state.rs    # AppState with client, db, media cache, timelines
│   └── app/                # Dioxus desktop frontend
│       └── src/
│           ├── app.rs      # Root component, sync bridge
│           ├── state.rs    # UiState signals, mention transforms
│           └── components/
│               ├── layout.rs          # MainLayout, modals, status bar, threads
│               ├── chat/
│               │   ├── commands.rs    # Modular slash command registry (38+ commands)
│               │   ├── message_area.rs # Tabs, messages, HUDs (pins/search/threads/files)
│               │   ├── message_input.rs # Contenteditable input, mention pills, formatting
│               │   └── emoji_picker.rs  # Categorized emoji with search
│               ├── sidebar/
│               │   ├── space_sidebar.rs # Space icons with avatars
│               │   └── room_list.rs     # Collapsible room list with invitations
│               └── members/
│                   └── member_list.rs   # Lazy-loaded with avatars
└── assets/
    └── style.css           # IRC-inspired dark theme with CSS variables
```

**Pure Rust** — no JavaScript, no TypeScript, no Node.js. The core library has zero UI dependencies. Minimal inline JS via `document::eval()` for WebView interop (scroll detection, drag-drop, clipboard paste, contenteditable input management).

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

### Production Build

```bash
cargo build -p fluxchat-app --release
```

## License

MIT
