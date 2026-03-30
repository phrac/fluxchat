# fluxchat

A Matrix chat client built entirely in Rust. Native GPU-rendered desktop app with an IRC-inspired interface.

![fluxchat screenshot](assets/screenshot-iced.png)

## Why fluxchat

- **Pure Rust** -- no JavaScript, no Electron, no WebView. Native GPU rendering via wgpu.
- **IRC meets modern chat** -- right-aligned colored nicks, monospace layout, channel tabs, but with inline images, reactions, threads, and encryption.
- **Fast** -- native binary, GPU-accelerated UI, 3-tier media cache, lazy-loaded member lists.

## Features

### Messaging
Send and receive messages in real-time via the Matrix Timeline API. Local echo, back-pagination on scroll-up, markdown formatting (`**bold**`, `*italic*`, `` `code` ``, `~~~codeblocks~~~`), inline images with lightbox, message editing (up-arrow), deletion, and forwarding. Multiline input with Shift+Enter. Read receipt avatars displayed per message.

### Reactions & Emoji
Full emoji picker with 9 categories, search, and frequently-used tracking. React to messages with any emoji. Pill-shaped reaction badges.

### Threads
Thread indicators with reply counts. Open threads as dedicated tabs with live updates. Thread replies use proper `m.thread` relation for cross-client compatibility.

### Encryption
End-to-end encryption via matrix-sdk (vodozemac). SAS emoji verification, key backup and recovery, session management with remote sign-out, cross-signing status.

### Rooms & Spaces
Room list grouped by space with collapsible sections. Room creation, settings, invitations, favourites. Room notifications (all/mentions/mute). Aliases, topic bar, upgrade detection.

### Search
Server-side search with client-side fallback for encrypted rooms. Clickable results with scroll-to-highlight. User directory and public room directory.

### Polls
Create interactive polls with `/poll`, vote with progress bars, end with `/pollend`. MSC3381 compatible.

### Files & Media
Upload via button, drag-drop, paste, or `/attach`. Thumbnail generation for images. GIF search via Giphy API. Sticker display. URL previews with og:image.

### Code Highlighting
Syntax highlighting for 100+ languages via syntect. Base16 Ocean Dark theme.

### Slash Commands
48+ commands with Tab-cycling autocomplete. Room management, messaging, user ops, discovery, polls, display toggles, text macros.

### Profile & Settings
Discord-style profile cards with colored banner, avatar, role badges, and power level management (Member/Mod/Admin presets + custom levels). Settings modal with sidebar navigation for account, sessions, and security. Password change, banner crop positioning, avatar management.

### Authentication
Username/password and SSO/OIDC with auto-detection and provider-specific buttons. Session persistence.

## Architecture

```
crates/
  core/   -- Pure business logic. Matrix ops, timeline, media, search, GIF, database.
  app/    -- Iced desktop frontend. Elm architecture: Message -> update() -> view().
```

Both crates are pure Rust with zero JS/TS dependencies.

## Tech Stack

| Component | Library |
|-----------|---------|
| UI | [Iced](https://iced.rs/) 0.14 (wgpu) |
| Matrix | [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) 0.16 |
| Timeline | matrix-sdk-ui 0.16 |
| Storage | rusqlite + matrix-sdk SQLite |
| E2EE | vodozemac |
| Syntax | syntect |

## Building

Requires Rust 1.77+ and GPU support (wgpu).

```bash
# Development
cargo run -p fluxchat

# Release
cargo build -p fluxchat --release
```

### macOS

```bash
./build-macos.sh
# Creates FluxChat.app bundle in target/release/
# Install: cp -r target/release/FluxChat.app /Applications/
```

### Linux

```bash
cargo build -p fluxchat --release
# Binary: target/release/fluxchat
```

## License

MIT
