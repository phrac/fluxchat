# fluxchat

A Matrix chat client built entirely in Rust. Native GPU-rendered desktop app with an IRC-inspired interface.

![fluxchat screenshot](assets/screenshot-v2.png)

## Why fluxchat

- **Pure Rust** -- no JavaScript, no Electron, no WebView. Native GPU rendering via wgpu.
- **IRC meets modern chat** -- right-aligned colored nicks, monospace layout, but with inline images, reactions, threads, encryption, and voice calls.
- **Fast** -- native binary, GPU-accelerated UI, lazy-loaded member lists, no per-frame overhead.

## Features

- **Messaging** -- real-time via Matrix Timeline API with local echo, markdown formatting, inline images with lightbox, editing, deletion, forwarding, replies with context, mention pills, reactions, URL previews, read receipts, selectable text with copy support
- **Encryption** -- E2EE via vodozemac, SAS emoji verification, key backup/recovery, cross-signing, session management
- **Voice calls** -- MatrixRTC + LiveKit SFU, mic capture via cpal, in-call panel with participant tiles, PIP widget, mute/hangup
- **Voice messages** -- record with mic button, Opus/OGG encoding, auto-normalization, preview before send, playback with progress bar
- **Threads** -- indicators with reply counts, dedicated thread view, thread entries in sidebar
- **Rooms & spaces** -- grouped room list, room creation, settings, invitations, favourites, notifications (all/mentions/mute), aliases, space details modal with room list and members, right-click context menus
- **Search** -- server-side with client-side fallback, overlay with arrow key navigation, scroll-to-highlight
- **Polls** -- create/vote/end with progress bars (MSC3381)
- **Files & media** -- upload via button/drag-drop/paste, thumbnails, GIF search via Giphy, stickers, location sharing
- **Code highlighting** -- 100+ languages via syntect, base16 Ocean Dark theme, collapsible code blocks
- **Notifications** -- desktop notifications via notify-rust, per-room mute, mentions-only mode, invite notifications
- **Profile & settings** -- Discord-style profile cards, power level management, banner/avatar editing, appearance/notification/security settings
- **Slash commands** -- 48+ commands with Tab-cycling autocomplete
- **Auth** -- password and SSO/OIDC with provider auto-detection, session persistence

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
| Timeline | matrix-sdk-ui 0.16 (sliding sync) |
| Storage | rusqlite + matrix-sdk SQLite |
| E2EE | vodozemac |
| Voice | [LiveKit](https://livekit.io/) 0.7 (MatrixRTC) |
| Audio | cpal 0.15 |
| Voice codec | opus (libopus) + ogg |
| Notifications | notify-rust |
| Syntax | syntect |

## Building

Requires Rust 1.77+ and GPU support (wgpu).

```bash
cargo run -p fluxchat
```

### macOS

```bash
./build-macos.sh
cp -r target/release/FluxChat.app /Applications/
```

### Linux

```bash
cargo build -p fluxchat --release
# Binary: target/release/fluxchat
```

## License

MIT
