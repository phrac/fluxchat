# fluxchat

A cross-platform Matrix chat client with a Discord-style UI, built with Tauri v2, Rust, and SolidJS.

> **Note:** This project was vibecoded with [Claude Code](https://claude.ai/claude-code). The entire codebase — Rust backend, SolidJS frontend, and all configuration — was generated through conversational AI pair programming.

## Screenshot

*Coming soon*

## Status

This is an **MVP** (Minimum Viable Product). It works, but expect rough edges.

### Current Features

- **Login & session persistence** — Log in with any Matrix homeserver. Sessions are saved locally via [FluxDB](https://github.com/phrac/fluxdb) and restored on relaunch.
- **Room list** — View all joined rooms, sorted alphabetically with unread badges and encryption indicators.
- **Spaces** — Browse Matrix spaces (Discord server equivalent) and their child rooms.
- **Messaging** — Send and receive text messages in real-time via Matrix sync.
- **Message history** — Scroll up to load older messages with backward pagination.
- **User avatars** — Avatars are fetched through the authenticated Matrix media API and cached in memory.
- **Formatted messages** — Renders HTML `formatted_body` from Matrix events (bold, italic, code, links, line breaks). Falls back to markdown parsing for plain text.
- **Image messages** — Displays image attachments inline with click-to-expand.
- **Custom emotes** — Resolves `mxc://` URLs in formatted HTML for inline custom emoji.
- **Member list** — Toggleable right sidebar showing room members with avatars.
- **End-to-end encryption** — E2EE via matrix-sdk with SQLite crypto store. Supports device verification (SAS emoji matching).
- **Cross-platform** — Targets Linux, macOS, and Android (via Tauri v2).

### Not Yet Implemented

- Message editing and deletion
- Reply/thread support
- Typing indicators
- Read receipts
- Room creation and settings
- File uploads
- Voice/video calls
- Notification system
- User profile editing
- Room search
- Message search
- Emoji picker
- Virtual scrolling for large rooms

## Tech Stack

- **Backend:** [Tauri v2](https://v2.tauri.app/) + [matrix-sdk](https://github.com/matrix-org/matrix-rust-sdk) (Rust)
- **Frontend:** [SolidJS](https://www.solidjs.com/) + [Tailwind CSS](https://tailwindcss.com/) + [Vite](https://vitejs.dev/)
- **Storage:** [FluxDB](https://github.com/phrac/fluxdb) (session persistence) + SQLite (matrix-sdk crypto/sync state)
- **Markdown:** [marked](https://github.com/markedjs/marked)

## Building

### Prerequisites

- Rust 1.77+
- Node.js 18+
- npm
- System dependencies for Tauri v2 ([see docs](https://v2.tauri.app/start/prerequisites/))

### Development

```bash
git clone https://github.com/phrac/fluxchat.git
cd fluxchat
npm install
npx tauri dev
```

First build takes several minutes (684 crates to compile). Subsequent builds are fast.

### Production Build

```bash
npx tauri build
```

## License

MIT
