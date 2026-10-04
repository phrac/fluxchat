# Notifications and mentions

## Desktop notifications (this device)

Settings → Notifications → *This device*:

- **Desktop notifications** on/off.
- **Only for mentions**: only alert when you're mentioned or a keyword matches.
- **Show message preview**: include the message text.
- **Play sound**.

You're not notified about rooms on screen (in any pane of the current window, or one of their threads) while the window is focused. On Linux, clicking a notification opens its room.

## Account rules (all devices)

Settings → Notifications → *Your account*. These are saved on your server as push rules, so they apply on every device:

- **Notify me for**: All messages or Mentions & keywords, separately for **direct messages** and **group rooms**.
- **Mentions of me**: your name, username, or a mention pill.
- **@room**: when someone notifies the whole room.
- **Keywords**: words that notify you (and highlight the message) wherever they appear. Type one and click **Add**; click × on a keyword to remove it.
- **Other**: invites, calls, bot messages, room upgrades.

## Per-room

Room settings → General → *Notify me in this room*: **All messages**, **Mentions**, or **Mute**. Also `/mute` and `/unmute`.

## Highlights in the timeline

Messages that mention you, `@room`, one of your keywords, or a local highlight word get a soft red background. Local highlight words (this device only) are managed with `/highlight <word>` and `/unhighlight <word>`.

## Mentions inbox

The **@** button in the room header (or **Ctrl/⌘+I**) opens the Mentions sidebar. It lists every message that mentioned you or matched a keyword, across all rooms, newest first. Each entry shows the room, the sender, how long ago, and the message.

Click an entry to open the room and jump to the message.

It fills from three places:
- your server's highlight list, which includes rooms you haven't opened;
- mentions that arrive while fluxchat is running;
- rooms that are already loaded.

Your server can't read mentions inside encrypted rooms, so older encrypted mentions show up once the room has been loaded.

## Unread indicators

- **Room list**: a dot and count per room (red = mention, accent = new messages).
- **Window title**: starts with the total unread count.
- **Rooms open at the first unread message**, marked by the **NEW** line.
