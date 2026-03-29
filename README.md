# fluxchat

A cross-platform Matrix chat client with an IRC-inspired UI, built entirely in Rust with Iced (native GPU-rendered via wgpu).

## Screenshot

![fluxchat screenshot](assets/screenshot-iced.png)

## Design

fluxchat blends IRC heritage with modern chat UX — not a Discord clone, not a terminal client, something unique.

- **IRC-style messages** — compact layout with right-aligned colored nicks, vertical separator, monospace timestamps
- **Channel tabs** — Chrome-style shrinking tabs for instant room switching
- **Command bar** — floating input bar with SVG formatting toolbar, slash command autocomplete, file upload
- **Status bar** — user avatar, room info, encryption status, sync state, device verification
- **Deep charcoal theme** — monospace throughout, SVG icons, deterministic nick colors

## Features

**Messaging** — Real-time send/receive via matrix-sdk-ui Timeline API with local echo, back-pagination on scroll-up with Loading indicator, formatted HTML with mention pills, inline images with lightbox, reactions with emoji picker (9 categories, search, frequently-used), replies with context, message editing and deletion (up-arrow to edit last message). Multiline input with Shift+Enter. Markdown formatting (**bold**, *italic*, ~~strike~~, `code`, ```codeblocks```). Read receipt avatars displayed per message. Unread indicators with numeric badge counts (red for mentions, blue for regular) and floating "new messages" pill. Message forwarding with room picker modal. Custom highlight words (`/highlight`) with visual indicators on matching messages.

**Threads** — Thread indicators on messages with reply counts. Click to open thread as a dedicated tab. Threads HUD dropdown lists all threads in a room. Thread replies sent with proper `m.thread` relation. Live updates via diff stream sync.

**Rooms & Spaces** — Room list grouped by space with collapsible sections and avatars, space sidebar, room creation with encryption toggles, room settings (name, topic, aliases, notifications, invite, leave), space overview. Room invitations with accept/reject. Room upgrade/tombstone detection with follow button. Favourite rooms with dedicated sidebar section. IRC-style topic bar below tabs. Copy room link to clipboard.

**Emoji & Reactions** — Full emoji picker with 9 categories (Smileys, People, Animals, Food, Travel, Activities, Objects, Symbols, Flags), search by name/shortcode, frequently-used tracking persisted to SQLite. Picker opens from message input bar (insert mode) or message hover react button (reaction mode). Pill-shaped reaction badges with accent tint for own reactions.

**Polls** — Create polls with `/poll "Question" "Option1" "Option2"`, vote by clicking options, end with `/pollend`. Interactive UI with progress bars, vote counts, percentages, and ended state. Uses MSC3381 unstable poll events for Element compatibility.

**Slash Commands** — Modular command registry with Tab-cycling autocomplete and `/help`. 48+ commands covering room management, messaging, user management, discovery, polls, display toggles, text macros, settings, and system.

**Files & Media** — File upload via button, drag-drop, clipboard paste, or `/attach` with thumbnail generation for images. 3-tier media cache (memory, SQLite, network). URL previews with og:image thumbnails. Files HUD with image previews. Sticker display (`m.sticker` events). GIF search and send via Giphy API (`/gif`, `/giphy`) with animated preview grid.

**Security** — End-to-end encryption via matrix-sdk. SAS emoji verification. Key backup and recovery (enable, recover with key/passphrase). Session management with device listing, verification status, and remote sign-out. Cross-signing status display.

**Search** — Server-side message search via Matrix search API with fallback to client-side filtering. Search HUD with clickable result list, scroll-to-message highlighting, and arrow key navigation. User directory search (`/users`). Public room directory (`/directory`).

**Authentication** — Username/password login. SSO/OIDC support with auto-detection, provider-specific buttons, and browser-based auth flow. Session persistence and restore.

**Code Display** — Syntax highlighting for code blocks via `syntect` with language detection and 100+ supported languages. Base16 Ocean Dark theme.

## Architecture

```
fluxchat/
├── crates/
│   ├── core/               # Business logic — pure Rust, no UI deps
│   │   └── src/
│   │       ├── auth.rs     # Login (password + SSO), logout, session restore
│   │       ├── timeline.rs # Timeline API: per-room + thread timeline management
│   │       ├── devices.rs  # Device listing, rename, remote sign-out
│   │       ├── gif.rs      # Giphy API search and download
│   │       ├── messages.rs # Send, edit, delete, pin, reactions, server search
│   │       ├── rooms.rs    # Room CRUD, spaces, DMs, invites, user/room search
│   │       ├── media.rs    # Media download with 3-tier cache
│   │       ├── sync.rs     # Sync loop (typing, room list, verification)
│   │       ├── db.rs       # SQLite storage (sessions, media cache, settings)
│   │       └── state.rs    # AppState with client, db, media cache, timelines
│   └── app/                # Iced desktop frontend (native GPU-rendered)
│       └── src/
│           ├── main.rs     # Entry point, window config
│           ├── state.rs    # FluxChat struct, enums
│           ├── message.rs  # Message enum, SyncEvent
│           ├── update.rs   # All state transitions
│           ├── view.rs     # All rendering
│           ├── html.rs     # Matrix HTML → Spans
│           ├── commands.rs # Slash command registry
│           └── theme.rs    # Colors
```

**Pure Rust** — no JavaScript, no TypeScript, no Node.js, no WebView. Fully native GPU-rendered via wgpu.

## Tech Stack

- **UI:** [Iced](https://iced.rs/) 0.14 (native GPU-rendered via wgpu, Elm architecture)
- **Matrix SDK:** [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) 0.16
- **Timeline:** [matrix-sdk-ui](https://github.com/matrix-org/matrix-rust-sdk) 0.16 (Timeline API)
- **Storage:** [rusqlite](https://github.com/rusqlite/rusqlite) (app data) + SQLite (matrix-sdk crypto/sync/event cache)
- **E2EE:** vodozemac (native Rust, same as Element X)

## Building

### Prerequisites

- Rust 1.77+
- GPU support (wgpu)

### Development

```bash
git clone https://github.com/phrac/fluxchat.git
cd fluxchat
cargo run -p fluxchat
```

### Production Build

```bash
cargo build -p fluxchat --release
```

## License

MIT
