# Getting started

## Install

fluxchat is built from source with Rust (1.93 or newer). You also need a GPU (it renders with wgpu) and GStreamer for video playback. On Linux, install the libopus and PulseAudio/PipeWire development headers for voice.

**macOS**

```bash
brew install gstreamer
./build-macos.sh
cp -r target/release/FluxChat.app /Applications/
```

**Linux**

```bash
cargo build -p fluxchat --release
./target/release/fluxchat
```

**Run from source (any platform)**

```bash
cargo run -p fluxchat
```

## Sign in

1. Enter your homeserver (for example `matrix.org`, or your own server's name).
2. fluxchat detects which sign-in methods the server offers:
   - **Password**: enter your username and password.
   - **Single sign-on (SSO / OIDC)**: click your provider's button. Your browser opens to complete sign-in, then you're returned to fluxchat.
3. Your session is saved, so you stay signed in next time.

If a saved session can't be restored, the error is shown on the sign-in screen. Your saved session and encryption keys are never deleted automatically; only signing in again replaces them.

## First steps

- **Verify this session** so your encrypted messages are trusted on your other devices: click **unverified** in the status bar, or see [Encryption](encryption.md).
- **Pick a room** in the left room list, or press **Ctrl/⌘+K** and type a room name.
- **Say hello**: type in the box at the bottom and press **Enter**.

fluxchat reopens where you left off: the same room, the same split panes, and the same open sidebars.

## Finding your way around

| Area | What it's for |
|------|---------------|
| Spaces bar (far left) | Home and your spaces. Click one to show only its rooms |
| Room list | Rooms, DMs and favourites, with unread badges and recent threads |
| Timeline (centre) | Messages of the room you're in. Can be split into panes |
| Sidebars (right) | Threads, Mentions and Users. Open any combination; drag to resize |
| Status bar (bottom) | Your account, the room, encryption, connection, windows |

See [The interface](interface.md) for the details.
