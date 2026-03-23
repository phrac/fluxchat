# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI built with Dioxus. Uses matrix-rust-sdk for the Matrix protocol.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. All Matrix operations (auth, messages, rooms, media, sync, etc.)
  - `crates/app/` — Dioxus desktop frontend with IRC-inspired design
- **Storage:** rusqlite for app data (sessions, media cache, settings, read rooms), matrix-sdk's built-in SQLite for crypto/sync
- **Sync:** Background sync loop with `SyncCallbacks` trait, bridged to UI via `std::sync::mpsc` channel
- **State:** Dioxus `Signal`s for reactive UI, `Arc<AppState>` for core state
- **Rendering:** All open tabs render to DOM simultaneously, shown/hidden via CSS `display` for instant tab switching

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js (except inline JS for WebView interop: scroll, drag-drop, paste, mention clicks)
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace fonts throughout, status bar, channel tabs
- Compact messages — more visible than Discord-style
- Channel tabs like Chrome browser tabs — flex to fill, shrink with more tabs
- Status bar at bottom showing gear+avatar+user, channel, nicks, encryption, sync status, device verification
- SVG icons (Lucide-style) for all actions — cross-platform, monochrome, inherits currentColor
- HUD-style dropdowns for search and pinned messages (slide down from tab bar)
- Lightbox overlay for full-size image viewing with native save dialog

## Build & Run
```bash
cargo run -p fluxchat-app
```
- **macOS:** No additional deps (uses WKWebView). Native window decorations enabled via `cfg!(target_os = "macos")`.
- **Linux:** webkit2gtk-4.1, gtk3, libsoup3, xdotool (Arch: `pacman -S webkit2gtk-4.1 gtk3 libsoup3 xdotool`)

## Important Patterns
- Dioxus signals need `mut` to write: `let mut ui = use_context::<UiState>()`
- Don't use `use_effect` inside conditionals — Dioxus hooks must be called unconditionally
- Don't read/write Dioxus Signals from background threads — use channels instead
- RSX format strings can't have inline `if` expressions — pre-compute values before the `rsx!` block
- Use `.peek()` instead of `.read()` for non-subscribing signal reads (avoids re-render cascades)
- Avatar resolution uses sync media cache lookup + background async batch resolve for uncached MXC URIs
- Scroll-to-bottom fires from inside `RoomMessages` when `is_active && !messages.is_empty()` — not from the parent
- Back-pagination preserves scroll position using `scrollHeight` delta: `newScrollHeight - oldScrollHeight`
- JS→Rust communication via `document::eval` + `dioxus.send()` channel (stored globally as `window.__fluxchat_dioxus`)
- File operations (drag-drop, paste, file picker) use JS to read files as base64, decode in Rust
- `read_rooms` HashSet persisted to SQLite settings to survive app restarts

## Performance Notes
- Member list lazy-loads: 30 initially, more on scroll. Avatars resolved in background batch.
- Messages capped at 200 per room, media cache at 500 entries.
- Inactive tabs use `.peek()` to avoid subscribing to message signal changes.
- Tab close frees messages, members, and pagination tokens from memory.
- Initial room load fetches up to 5 batches (20 events each) to find real messages past membership floods.

## Features Implemented
Login, session persistence, rooms grouped by space, spaces sidebar, channel tabs (Chrome-style), messaging with optimistic sends, formatted HTML messages, inline images with lightbox, mention pills with avatars and click-to-profile, member list with lazy loading and roles/presence, IRC-style message layout, status bar with gear/avatar/device status, reactions with emoji picker, replies with context preview, message editing, message deletion, pinned messages (HUD), search (HUD), typing indicators (send + display), URL previews, file upload (picker + drag-drop + paste), read receipts with persistent unread tracking, device verification UI (SAS emoji), profile modal, settings modal with logout, back-pagination on scroll, image lightbox with save dialog, cross-platform window decorations

## Features Pending
Threads UI, room creation/settings, user profile editing, desktop notifications
