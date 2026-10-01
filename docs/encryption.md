# Encryption and security

Encrypted rooms are end-to-end encrypted: only members' devices can read them. Encrypted rooms show **encrypted** in the status bar.

## Verify this session

A new session should be verified so your other devices (and other people) trust it:

- Click **unverified** in the status bar → Security, or
- From another of your devices, accept the verification request and compare the emoji. fluxchat shows the emoji grid; click **They match** if they're identical.

## Verify other people

Open someone's profile → **Verify** and compare emoji with them. For bots that can't do the emoji check, **Trust manually** marks their identity as trusted. Verified people get a green shield on their profile.

## Key backup and recovery

Settings → Security:

- **Enable backup** stores your message keys on the server, encrypted with a **recovery key**. Save the key when it's shown.
- On a new device, **Recover from backup** with that key (or your passphrase) to read old encrypted messages.
- "key backup not connected" in the status bar means a backup exists but this device isn't using it; recover to fix that.

## Messages that can't be decrypted

When a message can't be decrypted, fluxchat explains why under it when it can tell, for example:
- the sender's device is unknown;
- the device's keys changed;
- the key was never shared with you;
- the message was sent before you joined;
- key backup isn't set up.

## Encryption controls

- Enable encryption for a room: Room settings → Security & Privacy → **Enable**. This can't be undone.
- `/rotatekeys` starts a new encryption session for the current room.
- Sessions: review and sign out devices in Settings → Sessions.
