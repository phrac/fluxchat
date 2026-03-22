# fluxchat

A cross-platform Matrix chat client with a Discord-style UI, built with Tauri v2, Rust, and SolidJS.

> **Note:** This project was vibecoded with [Claude Code](https://claude.ai/claude-code). The entire codebase — Rust backend, SolidJS frontend, and all configuration — was generated through conversational AI pair programming.

## Screenshot

*Coming soon*

## Status

This is an **MVP** (Minimum Viable Product). It works, but expect rough edges.

### Current Features

- **Login & session persistence** — Log in with any Matrix homeserver. Sessions are saved locally via [FluxDB](https://github.com/phrac/fluxdb) and restored on relaunch.
- **Room list** — Rooms organized by space with category headers, avatars, DMs separated from channels. Unread badges and encryption indicators.
- **Spaces** — Browse Matrix spaces with avatar banners. Clicking a room auto-switches to its parent space view.
- **Messaging** — Send and receive text messages in real-time via Matrix sync with optimistic local insertion. Instant room switching via DOM caching (last 5 rooms kept in memory).
- **Message history** — Scroll up to load older messages with backward pagination.
- **File uploads** — Upload images, videos, audio, documents via the + menu. Files are staged with thumbnail previews before sending, supporting multiple files and optional text.
- **Typing indicators** — See when others are typing, send typing notifications while composing.
- **Replies** — Reply to messages with context preview. Reply threads shown inline with colored quote bars.
- **Threads** — Reply in Thread from the context menu opens a dedicated thread panel sidebar with full message rendering and inline reply input.
- **Read receipts** — Automatically sends read receipts and fully-read markers when viewing messages.
- **Reactions** — Quick emoji reactions (😂🔥❤️👍😮😢) via hover action bar. Reaction badges shown below messages with counts.
- **Message editing** — Edit your own messages via context menu with inline edit preview bar.
- **Message actions** — Hover action bar with personalized quick reactions, reply, and a context menu (edit, pin, copy, show source, delete).
- **Pinned messages** — Pin/unpin messages, view pinned messages in a Discord-style modal overlay. Click to jump to message in chat.
- **Message deletion** — Redact messages via the context menu.
- **User avatars** — Avatars fetched through the authenticated Matrix media API with three-tier caching (memory → FluxDB disk → network).
- **Formatted messages** — Renders HTML `formatted_body` from Matrix events (bold, italic, code, links, line breaks). Falls back to markdown parsing for plain text.
- **Image messages** — Displays image attachments inline with click-to-expand.
- **Custom emotes** — Resolves `mxc://` URLs in formatted HTML for inline custom emoji.
- **Member list** — Toggleable right sidebar showing room members grouped by role (Admin/Moderator/Member) with presence indicators and homeserver names.
- **Invite users** — Invite users to rooms via modal dialog.
- **Message search** — Client-side search across loaded messages.
- **End-to-end encryption** — E2EE via matrix-sdk with SQLite crypto store. Supports device verification (SAS emoji matching).
- **Cross-platform** — Targets Linux, macOS, and Android (via Tauri v2).

### Not Yet Implemented

- Polls
- Room creation and settings
- Voice/video calls
- Desktop notifications
- User profile editing
- Server-side message search
- Emoji picker
- Virtual scrolling for large rooms
- Room topic editing
- User status/custom status
- Drag & drop file uploads

## Tech Stack

- **Backend:** [Tauri v2](https://v2.tauri.app/) + [matrix-sdk](https://github.com/matrix-org/matrix-rust-sdk) (Rust)
- **Frontend:** [SolidJS](https://www.solidjs.com/) + [Tailwind CSS](https://tailwindcss.com/) + [Vite](https://vitejs.dev/)
- **Storage:** [FluxDB](https://github.com/phrac/fluxdb) (session + media cache persistence) + SQLite (matrix-sdk crypto/sync state)
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
