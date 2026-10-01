# fluxchat

A fast, native [Matrix](https://matrix.org) chat client written entirely in Rust, with an IRC-inspired interface.

![fluxchat screenshot](assets/screenshot.png)

## Highlights

- **Pure Rust, GPU-rendered.** No Electron, no WebView, no JavaScript. Built on [Iced](https://iced.rs) and [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk).
- **IRC feel, modern features.** Timestamp, colored nick and message columns in a monospace layout, with images, reactions, threads, spaces, encryption and voice calls.
- **Fast.** Virtualized timeline, sliding sync, lazy-loaded member lists.

## Features

### Messaging
- Markdown, code blocks with syntax highlighting, inline LaTeX
- Replies, edits, deletions, reactions, forwarding, pins
- Mentions with autocomplete, `:emoji:` shortcodes, full emoji picker
- Images, video, voice messages, files, GIFs, stickers, polls, locations
- Captions: attach a file and type a message, sent together as one
- Link previews, read receipts, typing indicators, per-room drafts

### Threads
- Thread pane beside the room, with its own reply box (or open full size)
- Reply counts and latest-reply previews under each thread root
- Live updates, including bots that answer inside threads

### Rooms & spaces
- Room list grouped by space, favourites, unread and mention badges
- Quick switcher (Ctrl/Cmd+K), Alt+1–9 and Alt+Up/Down room hotkeys
- Space management: browse, join, add, remove and create rooms; edit the space
- Room settings: name, topic, icon, notifications, spaces
- Security & privacy: who can join, history visibility, guests, directory listing, encryption
- Roles & permissions: role per action, privileged users, bans
- Public room directory

### People
- Profile cards with presence, status, roles, verification, and moderation tools
- Member list with filter and presence
- Ignore users, direct messages

### Encryption
- End-to-end encryption, emoji (SAS) verification, cross-signing
- Key backup and recovery, session management
- Explanations for messages that can't be decrypted

### Calls
- Voice calls over MatrixRTC + LiveKit
- Choose microphone and speaker, mic test, echo cancellation, noise suppression

### Notifications
- Desktop notifications driven by your server's push rules
- Account-wide rules: DMs, group rooms, mentions, @room, keywords, invites, calls, bots
- Per-room mode: all messages, mentions only, or mute
- Mentions and keywords highlighted in the timeline

### Customization
- 9 built-in themes plus your own TOML themes (light and dark)
- Message and sidebar fonts, text size
- Preferences for timestamps (12/24h), compact layout, message grouping, big emoji, link previews, GIF autoplay, send shortcut, markdown, privacy (read receipts, typing, presence)
- Resizable room list, thread pane and member list

### Power users
- 57+ slash commands with Tab completion (`/help` lists them)
- `/devtools`: room state inspector and raw event sender
- Password and SSO/OIDC login

## Building

Requirements: Rust 1.93+, a GPU (wgpu), GStreamer (video). On Linux also libopus and PulseAudio/PipeWire headers (voice).

```bash
# macOS
brew install gstreamer
cargo run -p fluxchat
```

**macOS app bundle**

```bash
./build-macos.sh
cp -r target/release/FluxChat.app /Applications/
```

**Linux**

```bash
cargo build -p fluxchat --release   # binary: target/release/fluxchat
```

## Architecture

```
crates/
  core/   Matrix logic: timelines, spaces, room admin, push rules, media, audio
  app/    Iced desktop UI (Elm architecture: Message -> update() -> view())
```

| Component | Library |
|-----------|---------|
| UI | Iced 0.14 (wgpu) |
| Matrix | matrix-rust-sdk 0.18, matrix-sdk-ui 0.18 (sliding sync) |
| Encryption | vodozemac |
| Calls | LiveKit 0.7 (MatrixRTC), cpal 0.15 |
| Voice messages | opus + ogg |
| Video | iced_video_player 0.6 (GStreamer) |
| Storage | rusqlite + matrix-sdk SQLite |
| Syntax highlighting | syntect |

## License

MIT
