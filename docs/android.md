# fluxchat for Android

The Android app is a companion to desktop fluxchat for your AI agents. It doesn't try to be a full Matrix client. It shows the rooms your agents are in and what every run is doing. You can approve or answer an agent from a notification, stop a run, or send a quick follow-up, from anywhere, through your homeserver. You don't need a VPN or to reach the agent's machine.

The app is new and early. Build it from source (below); there's no store release yet.

## Signing in

Sign in with your Matrix account (a username, `@name`, or a full `@name:server`); the phone becomes a new session. If your account has key backup, the app asks for your **recovery key** next. That unlocks encrypted agent rooms and verifies the phone. You can skip it, but encrypted rooms then stay unreadable on the phone.

## Your agents

The app uses the same agent list as desktop fluxchat. It's saved to your account, so agents you marked on the desktop are already there. Manage them under **Agents** (the robot icon):
- **Add agent**: by Matrix ID.
- Tap an agent to choose its harness (Hermes Agent, OpenClaw, Claude Code, Generic), or to remove it.

Changes apply on every device. The phone reads agents the same way the desktop does: same [harness profiles](agents.md#harness-profiles), same [agent protocol](agent-protocol.md).

## The agents screen

One card per room with an agent in it. Rooms waiting on you come first, then rooms with a run going, then the rest by latest message.

Each card shows:
- the room and its latest message, or who's typing;
- the run's state (*Working · 3m · 🔧 Running tests · $0.42*);
- the room's live thread runs, each one tap away;
- unread counts.

The chips at the top filter to **Needs you** or **Running**. Pull down to refresh.

## A room or thread

- **Messages** appear as bubbles. Agents' step logs fold into one line ("6 steps · 🔧 Running tests"); tap to see every step. Long answers fold too. Images show inline; tap one for full screen, and pinch to zoom.
- **Approvals and questions** appear as buttons under the message: Approve once / For this session / Always / Deny, or a question's numbered choices. A question can also be answered in your own words.
- **The run card** above the message box shows the state, time so far, the latest step, and a progress bar.
  - **Stop** sends the harness's stop command.
  - In a room whose run is happening in a thread, the card names the thread; tap it to open the thread.
- **While the agent works**, you choose what a new message does, as on the desktop: **Send**, **Steer** (read after its next step) or **Queue** (the next task).
- **`/` commands**: type `/` for the agent's commands as you type. They go out the way its harness takes them (`!stop` for Hermes).
- **The ⋮ menu** has the agent's quick commands: Status, Usage, Context, Model, New session.
- **Threads follow you.** Send a request from a room. When the agent starts on it in a thread, the thread opens.

## Sessions

Hermes keeps one session per thread. Instead of scrolling back through a room for an older session, tap the **clock icon** in a room's top bar for its **Sessions**: every thread in the room, from your server (not just the loaded history), grouped by last activity (Today, Yesterday, Previous 7 days, Previous 30 days, Older).
- **Each row:** the session's title (your request), its latest reply, when it was last active, and the run's state. A live run shows its step instead of the latest reply.
- **Search** filters by the request or the latest reply.
- **Tap a session** to open it. From one session to another, Back still goes to the room.
- **New session:** in a room, it takes you to the room to send a fresh request, which opens a new thread. In a DM, where the agent doesn't use threads, it sends the agent's new-session command (`!new` for Hermes).

On a wide screen (840 dp or more: a tablet, an unfolded foldable, a phone in landscape) the sessions stay open in a sidebar beside the chat. On a phone the drawer opens from its button, not a swipe from the left edge, since that's Android's back gesture.

## Notifications

Push goes through [UnifiedPush](https://unifiedpush.org), with no Google services:
1. Install a distributor app, such as [ntfy](https://ntfy.sh). It works with ntfy.sh or your own ntfy server.
2. In **Settings → Push notifications**, turn push on and pick the distributor if asked.

Your homeserver then pushes to the distributor through a Matrix push gateway: ntfy's own if it has one, else `matrix.gateway.unifiedpush.org`. The app fetches and decrypts each event itself. What notifies:
- **An agent waiting on you**, with the answers as buttons. Tap one to answer without opening the app.
- **An agent's question**, with its numbered choices as buttons.
- **Other agent messages** (answers, finished runs). Step logs don't notify.
- **Messages that mention you.**

Every agent notification has **Reply**, which answers in the thread the message is in. While the app is running (a push wakes it), a prompt's notification also goes away once it's answered anywhere: on the desktop, in the app, or from the notification.

Hermes approvals sent without the [agent protocol](agent-protocol.md) only become prompts when Hermes adds its reactions, a moment after the message arrives. The buttons appear when the app sees those reactions. With the [Hermes patches](../integrations/hermes) applied, the prompt carries its choices and the buttons are there at once.

Without push, you only hear from agents while the app is open.

## Diagnostics

**Settings → Diagnostics** shows what the app knows and has done:
- your session, sync state and encryption status;
- the agent rooms it found, with how many messages are loaded and how many runs are live;
- push: distributor, gateway, whether the pusher is set, pushes received and notifications shown;
- recent events and errors.

**Copy** puts it on the clipboard for a bug report. It never includes tokens or keys. Push endpoints show only their host, since the path is a secret topic.

## Building

From the repository's `android/` directory, with Android Studio installed (or a JDK 21 and the Android SDK):

```bash
JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew :app:assembleRelease
adb install -r app/build/outputs/apk/release/app-arm64-v8a-release.apk
```

The release build is signed with the debug key unless `KEYSTORE_PATH`, `STORE_PASSWORD`, `KEY_ALIAS` and `KEY_PASSWORD` are set. That's fine for installing on your own phone. Android 9 or newer is needed.
