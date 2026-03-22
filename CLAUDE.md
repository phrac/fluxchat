# fluxchat — Claude Code Context

## What is this
A cross-platform Matrix chat client written entirely in Rust. IRC-inspired UI built with Dioxus. Uses matrix-rust-sdk for the Matrix protocol.

## Architecture
- **Cargo workspace** with two crates:
  - `crates/core/` — pure business logic, no UI deps. All Matrix operations (auth, messages, rooms, media, sync, etc.)
  - `crates/app/` — Dioxus desktop frontend with IRC-inspired design
- **Storage:** rusqlite for app data (sessions, media cache), matrix-sdk's built-in SQLite for crypto/sync
- **Sync:** Background sync loop with `SyncCallbacks` trait, bridged to UI via `std::sync::mpsc` channel
- **State:** Dioxus `Signal`s for reactive UI, `Arc<AppState>` for core state

## Key Design Decisions
- Pure Rust — no JavaScript, TypeScript, or Node.js
- IRC-inspired: right-aligned nicks with vertical separator, colored nicks, monospace timestamps, status bar, channel tabs
- Compact messages — more visible than Discord-style
- Channel tabs like browser tabs for quick room switching
- Status bar at bottom showing user@server, channel, nicks, encryption status

## Build & Run
```bash
cargo run -p fluxchat-app
```
System deps: webkit2gtk-4.1, gtk3, libsoup3, xdotool (Arch: `pacman -S webkit2gtk-4.1 gtk3 libsoup3 xdotool`)

## Important Patterns
- Dioxus signals need `mut` to write: `let mut ui = use_context::<UiState>()`
- Don't use `use_effect` inside conditionals — Dioxus hooks must be called unconditionally
- Don't read/write Dioxus Signals from background threads — use channels instead
- RSX format strings can't have inline `if` expressions — pre-compute values before the `rsx!` block
- Variable name `core` conflicts with Rust's `core` crate in nested scopes — use `app_state` instead
- Avatar resolution crashes if done reactively — currently using colored initials in messages

## Features Implemented
Login, session persistence, rooms grouped by space, spaces sidebar, channel tabs, messaging with optimistic sends, formatted HTML messages, member list with roles/presence, IRC-style message layout, status bar

## Features Pending (from v1 Tauri version, backend ready in core/)
Reactions, emoji picker, replies, threads, message editing/deletion, file uploads, image display, pinned messages, typing indicators, room creation, user profile editing, verification UI, URL previews, search, notifications
