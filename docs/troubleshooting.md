# Troubleshooting

## A room or thread looks empty

fluxchat keeps loading older history until there's something to show. A room whose recent activity is all edits, reactions or thread replies may take a moment. If it stays empty, `/refresh` rebuilds the room's timeline.

## A bot's answer isn't in the room

Many bots reply **in a thread** on your message. Look for the "N replies" summary under your message, the thread under the room in the room list, or the Threads sidebar.

## Someone shows as offline but isn't

Presence comes from their homeserver. If their server doesn't share presence, or a bot never sets it, nothing (or "Offline") is shown.

## "Unable to decrypt" messages

The explanation under the message says why: unknown device, changed keys, key never shared, sent before you joined, or no key backup.
- Verify this session, and recover from your key backup (Settings → Security).
- Messages encrypted to keys you never received can't be recovered.

## "key backup not connected"

A backup exists on the server but this device isn't using it. Settings → Security → *Recover from backup* with your recovery key.

## "reconnecting…" in the status bar

The connection to your homeserver dropped; fluxchat reconnects automatically, retrying less often the longer it stays offline (up to once a minute). The message clears once syncing has actually resumed. If your server rejects the session (for example after signing out elsewhere), you're returned to the sign-in screen.

## Couldn't restore the session

The error is shown on the sign-in screen. fluxchat never deletes your saved session or encryption keys on its own. A common cause is another fluxchat instance still running and holding the database; quit it and try again.

## GIF search says it needs a key

Set a Giphy API key with `/giphykey <key>` or the `GIPHY_API_KEY` environment variable.

## The microphone test, voice messages or calls are silent

macOS only lets apps use the microphone after you allow it. Rebuild the app with `./build-macos.sh` (older builds never asked), open FluxChat, start the mic test and allow access when asked, or turn FluxChat on in System Settings → Privacy & Security → Microphone. Rebuilding may make macOS ask again.

## Video won't play

Install GStreamer (`brew install gstreamer` on macOS, your distribution's packages on Linux).

## Calls aren't available

Your homeserver needs a LiveKit (MatrixRTC) server configured in its `.well-known`. Without one the call button is hidden.

## Logs

Run fluxchat from a terminal to see logs. `RUST_LOG` changes the detail level, for example:

```bash
RUST_LOG=fluxchat=debug cargo run -p fluxchat
```

## Where things are stored

| OS | Data folder |
|----|-------------|
| macOS | `~/Library/Application Support/fluxchat/` |
| Linux | `~/.local/share/fluxchat/` |
| Windows | `%APPDATA%\fluxchat\` |

It contains `fluxchat.db` (settings, media cache), `matrix_store/` (encryption keys, sync state, message cache) and `themes/`.
