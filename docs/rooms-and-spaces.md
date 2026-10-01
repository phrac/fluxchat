# Rooms and spaces

## Joining rooms

- **Explore rooms**: the room-list **+** menu, the palette, or `/directory [query]`. Search your server's public rooms (or matrixrooms.info); click **Join**.
- **By address**: `/join #room:server` (or a room ID).
- **From a link**: clicking a room link (matrix.to) opens the room if you're in it, jumping to the linked message if there is one. For a room you're not in, fluxchat asks before joining.
- **Ask to join** a room that requires approval: `/knock <room> [reason]`. Pending requests are listed in the room list.
- **Invites** appear under the bell in the room list with **Accept** / **Decline**. You also get a desktop notification.

## Creating rooms

Room list **+** → *Create room* (or `/create [name]`). Choose:
- **Room or space**;
- name and topic;
- which space to add it to;
- whether it's **public**;
- whether it's **end-to-end encrypted** (rooms only; can't be turned off later).

Direct messages: **+** → *Direct message*, `/msg <user> <text>`, `/query <user>`, or **Message** on someone's profile.

## Room settings

Open with the room header's info button, right-click → Room Settings, or the palette. Three tabs:

### General
- **Icon**: upload or remove.
- **Name** and **topic**: the Save button enables when changed.
- **Notify me in this room**: All messages / Mentions / Mute. Only affects this room.
- **Spaces**: the spaces this room is in (with Remove, where you're allowed), and **Add to a space…** for spaces you can edit.
- **Room ID** with a copy button, and **Leave room**.

### Security & Privacy
- **Who can join**:
  - Private (invite only);
  - Ask to join;
  - Space members (anyone in the room's spaces);
  - Public.
- For public rooms: **List in the room directory** and **Allow guests**.
- **Who can read history**:
  - Anyone (public rooms);
  - Members, all history;
  - Members, since they were invited;
  - Members, since they joined.

  Changes apply to future messages.
- **Encryption**: shown as on, or an **Enable** button. Once enabled, it can't be turned off.

### Roles & Permissions
- **People with roles** (admins, moderators, custom levels), with *reset to member* for people below you.
- **Banned** users with **Unban**.
- **Permissions**: the minimum role for each action:
  - default role for new members;
  - send messages, invite, remove, ban;
  - delete others' messages, notify everyone;
  - pin;
  - change name / topic / icon / address / history / permissions;
  - enable encryption, upgrade, and other settings.

  You can set levels up to your own.

Controls you don't have permission for are shown read-only.

## Spaces

Spaces group rooms and people. Select a space in the spaces bar to filter the room list, then click its name (or ⚙) in the room-list header to manage it. Right-click a space → *Manage Space* also works.

The space dialog has three tabs:

- **Rooms**: every room in the space, including ones you haven't joined. You can:
  - filter the list;
  - **Open** rooms you're in, **Join** the others;
  - **Add rooms** you're in;
  - **Create room** directly in the space;
  - **Remove** rooms (with a confirmation).

  Adding and removing need permission in the space.
- **Members**: everyone in the space; click someone for their profile.
- **Settings**: icon, name, topic, and **Leave space** (you stay in its rooms).

A room can belong to several spaces.

## Favourites

Click the star in the room header (or right-click → Favourite). Favourites get their own section at the top of the room list and also stay in their normal section.

## Leaving

Right-click → Leave, `/leave`, or Room settings → Leave room. You're asked to confirm. If leaving fails, the room comes back and the error is shown.

## Room upgrades

If a room has been replaced by a newer version, a banner offers to follow it. Admins can upgrade with `/upgraderoom <version>`.
