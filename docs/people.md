# People

## Profiles

Click someone's name or avatar (in a message, the member list, or a space's members) to open their profile card:

- their name, user ID (with copy) and avatar;
- **presence**, fetched from their server: Online, Away, or "Offline · last seen 3h ago". If their server doesn't share presence, nothing is shown;
- their **status message**;
- role badge (Admin / Moderator) for the current room;
- **Mention** (inserts `@name` into your message box) and **Message** (opens a DM);
- **Verification**: Verified, or **Verify** (emoji comparison) and **Trust manually** (for bots that can't do the emoji check);
- **Ignore / Unignore**.

Clicking your own name opens Settings instead.

## Roles and moderation

If you outrank someone in the current room, their profile also shows:

- **Role**: Member / Moderator / Admin, or a custom level (0–100). You can't give a level above your own.
- **Kick** and **Ban**, each with a confirmation.

The same is available as commands: `/op <user> [level]`, `/deop <user>`, `/kick <user> [reason]`, `/ban <user> [reason]`, `/unban <user>`. Names can be display names, not just user IDs.

Room-wide permissions and the ban list live in [Room settings → Roles & Permissions](rooms-and-spaces.md#roles--permissions).

## Member list

The people button in the room header toggles it.

- **Grouping**: admins and moderators are listed first.
- **Presence**: a green dot means online, yellow means away. Presence is fetched from people's servers in rooms of up to 50 members.
- **Status messages** show under names.
- **Filter**: the box at the top filters by name or user ID.
- **Width**: drag its left edge to resize.

## Ignoring

`/ignore <user>` hides someone's messages everywhere; `/unignore <user>` brings them back. You can also ignore from their profile.

## Finding people

`/users <query>` searches your server's user directory. `/whois <user>` shows someone's info.

## Your own presence and status

- `/status <message>` sets your status message.
- `/away [message]` marks you away.
- Turn off *Share presence* in [Preferences](settings.md#preferences) to stop showing when you're online.
