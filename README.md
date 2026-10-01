# fluxchat

A Matrix chat client built entirely in Rust. Native GPU-rendered desktop app with an IRC-inspired interface.

![fluxchat screenshot](assets/screenshot.png)

## Why fluxchat

- **Pure Rust** -- no JavaScript, no Electron, no WebView. Native GPU rendering via wgpu.
- **IRC meets modern chat** -- timestamp | right-aligned colored nick | message columns in a monospace layout, with inline images, reactions, threads, spaces, encryption, and voice calls.
- **Fast** -- native binary, GPU-accelerated UI, virtualized message list, lazy-loaded member lists.

## Features

- **Messaging** -- real-time via the Matrix Timeline API with local echo and send state (sending / failed + retry), markdown (bold, italic, strike, code, code blocks, headings), `(edited)` markers, day dividers, replies with context, editing, deletion, forwarding, mention pills, `:shortcode` emoji completion, reactions, URL previews, read receipts, per-room drafts, cross-message text selection with copy, right-click menu (copy link / event ID / view source), `//` escape for sending literal `/commands`
- **Readable timeline** -- consecutive messages from one sender are grouped (dimmed nick, no repeated avatar), colored-initial avatars for people without one, columns that scale with the font size, history auto-loads until there's something to show (even in rooms whose recent traffic is all edits or thread replies)
- **Threads** -- summary under each thread root (reply count, latest replier and preview, from the server's thread summary), live thread views on SDK thread timelines, thread tabs nested under their room in the sidebar, scrollable threads panel
- **Spaces** -- space hierarchy in the sidebar, and a space dialog (click the space name in the room list): browse every room in the space including ones you haven't joined, join, open, add existing rooms, create rooms in the space, remove rooms, member list, and edit the space's name, topic and icon -- all permission-aware
- **Rooms** -- grouped room list with unread dot + badge indicators, favourites, Ctrl/Cmd+K quick switcher, Alt+1-9 / Alt+Up/Down room hotkeys (Alt+Shift for unread rooms), room creation, invitations, right-click menus, room settings (icon, name, topic, per-room notifications, space membership, encryption), public room directory
- **People** -- profile cards with presence fetched from the user's server (online / away / last seen), status message, role badges, Mention / Message actions, verification, and -- when you outrank them -- role presets, custom power levels, kick and ban with confirmation; member list with filter and presence dots
- **Encryption** -- E2EE via vodozemac, SAS emoji verification, manual identity trust (for bots), key backup and recovery, identity reset, cross-signing, session management, per-message decryption diagnostics
- **Voice calls** -- MatrixRTC + LiveKit SFU, mic capture via cpal, in-call panel with participant tiles, PIP widget, mute/hangup
- **Voice messages** -- record with the mic button, Opus/OGG encoding, auto-normalization, preview before send, in-process playback with progress bar
- **Video** -- inline player via GStreamer (iced_video_player), generated thumbnails with rotation detection for bridged media
- **Search** -- server-side with client-side fallback, in a command-palette overlay with keyboard navigation and scroll-to-highlight
- **Polls** -- create / vote / end with progress bars (MSC3381)
- **Files & media** -- upload via button, drag-drop or clipboard paste, upload-limit check, image thumbnails, lightbox with save/copy, files panel, GIF search via Giphy, stickers, location sharing, file download cards
- **Notifications** -- desktop notifications driven by your server push rules (mute, mentions-only, keywords), per-room mode (all / mentions / mute), click to open the room, invite notifications, unread count in the window title
- **Settings** -- account (avatar, banner with crop, display name, password), appearance (theme, fonts, text size with live preview), notifications, sessions (verify / sign out), security (backup, recovery, reset)
- **Themes** -- 9 built-in themes (FluxChat, Nord, Nord Light, Dracula, Night Owl, Tokyo Night, Atom One Dark, Catppuccin Mocha/Latte) plus custom TOML themes; every hover and highlight adapts to light and dark themes
- **Code highlighting** -- 100+ languages via syntect, theme-aware, collapsible code blocks
- **Slash commands** -- 57+ commands with Tab-cycling autocomplete, including /devtools (room state inspector + raw event sender), /html, /plain, /unban, /upgraderoom
- **Auth** -- password and SSO/OIDC with provider auto-detection, session persistence
- **Reliability** -- one watcher per timeline (no client-side merging), sync auto-reconnect with a status-bar indicator, virtualized message list, bounded media caches, window size / sidebar width persistence

## Architecture

```
crates/
  core/   -- Pure business logic. Matrix ops, timelines, spaces, media, search, GIF, database.
  app/    -- Iced desktop frontend. Elm architecture: Message -> update() -> view().
```

Both crates are pure Rust with zero JS/TS dependencies. Dialogs are assembled from a small shared component kit (`crates/app/src/ui.rs`).

## Tech Stack

| Component | Library |
|-----------|---------|
| UI | [Iced](https://iced.rs/) 0.14 (wgpu) |
| Matrix | [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) 0.18 |
| Timeline, sync, spaces | matrix-sdk-ui 0.18 (sliding sync) |
| Storage | rusqlite + matrix-sdk SQLite |
| E2EE | vodozemac |
| Voice | [LiveKit](https://livekit.io/) 0.7 (MatrixRTC) |
| Audio | cpal 0.15 |
| Voice codec | opus (libopus) + ogg |
| HTML parsing | html5ever |
| Notifications | notify-rust |
| Video | [iced_video_player](https://github.com/jazzfool/iced_video_player) 0.6 (GStreamer) |
| Syntax | syntect |

## Building

Requires Rust 1.93+ (matrix-sdk 0.18), GPU support (wgpu), GStreamer (for video playback), and libopus + PulseAudio/PipeWire headers on Linux (voice).

```bash
# macOS: install GStreamer
brew install gstreamer

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
