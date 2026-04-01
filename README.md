# fluxchat

A Matrix chat client built entirely in Rust. Native GPU-rendered desktop app with an IRC-inspired interface.

![fluxchat screenshot](assets/screenshot-iced.png)

## Why fluxchat

- **Pure Rust** -- no JavaScript, no Electron, no WebView. Native GPU rendering via wgpu.
- **IRC meets modern chat** -- right-aligned colored nicks, monospace layout, but with inline images, reactions, threads, encryption, and voice calls.
- **Fast** -- native binary, GPU-accelerated UI, 3-tier media cache, lazy-loaded member lists.

## Features

### Messaging
Send and receive messages in real-time via the Matrix Timeline API. Local echo, back-pagination on scroll-up, markdown formatting (`**bold**`, `*italic*`, `` `code` ``, `~~~codeblocks~~~`), inline images with lightbox, message editing (up-arrow), deletion, and forwarding. Multiline input with Shift+Enter. Read receipt avatars displayed per message. Reply context with colored bar. Mention pills (@name) with profile popup on click. Reaction tooltips showing who reacted. URL preview cards with thumbnails (max 2 shown, expandable).

### Encryption
Enable encryption on any room from room settings (one-way toggle). Full E2EE support for messages, images, and media in encrypted rooms.

### Reactions & Emoji
Full emoji picker with 9 categories, search, and frequently-used tracking. React to messages with any emoji. Pill-shaped reaction badges.

### Threads
Thread indicators with reply counts on root messages. Threads HUD lists all thread roots. Open threads from hover bar, HUD, or reply count — loads via relations API with sub-header showing root message. Thread entries in sidebar below parent room. Thread replies use proper `m.thread` relation. Replies hidden from main timeline.

### Encryption
End-to-end encryption via matrix-sdk (vodozemac). SAS emoji verification, key backup and recovery, session management with remote sign-out, cross-signing status.

### Rooms & Spaces
Room list grouped by space with collapsible sections. Room creation, settings, invitations, favourites. Room notifications (all/mentions/mute). Aliases, topic bar, upgrade detection.

### Voice Calls (MatrixRTC)
Voice calls via MatrixRTC + LiveKit. Join or start calls from any room. Microphone capture via cpal, audio playback of remote participants. In-call panel with participant avatars and mute/hangup controls. PIP widget in member list when viewing other rooms during a call. Defaults to muted mic on join. Requires a LiveKit SFU (discovered via `.well-known`).

### Notifications
Desktop notifications via `notify-rust`. Fires when messages arrive in non-active rooms or when the app is unfocused. Settings tab with toggles: enable/disable, mentions only, show message preview, notification sound. Per-room mute via `/mute` command.

### Search
Server-side search with client-side fallback for encrypted rooms. Overlay dropdown with clickable results, arrow key navigation, scroll-to-highlight with blue tint. User directory and public room directory.

### Polls
Create interactive polls with `/poll`, vote with progress bars, end with `/pollend`. MSC3381 compatible.

### Files & Media
Upload via button, drag-drop, paste, or `/attach`. Thumbnail generation for images. GIF search via Giphy API. Sticker display. URL previews with og:image.

### Code Highlighting
Syntax highlighting for 100+ languages via syntect. Base16 Ocean Dark theme.

### Slash Commands
48+ commands with Tab-cycling autocomplete. Room management, messaging, user ops, discovery, polls, display toggles, text macros.

### Profile & Settings
Discord-style profile cards with colored banner, avatar, role badges, and power level management (Member/Mod/Admin presets + custom levels). Settings modal with sidebar navigation for Account, Appearance, Notifications, Sessions, and Security. Password change, banner crop positioning, avatar management, font size control, notification preferences.

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
| Voice | [LiveKit](https://livekit.io/) 0.7 (MatrixRTC) |
| Audio | cpal 0.15 |
| Notifications | notify-rust |
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
