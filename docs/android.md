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

Rooms waiting on you come first, then rooms where an agent is working, then the rest by their latest activity (replies in session threads count).

The chips at the top filter to **Needs you** or **Running**. Pull down to refresh.

## Start a session from anywhere

A small **New session** sheet opens over whatever you're doing: pick the agent room (it starts on the one you last asked something in), type or speak the request, **Send**. It closes once sent, and the live run notification takes over. The ↗ button opens the room instead. Open the sheet from:
- **the home-screen widget** (*fluxchat → New session*): a text field (*Ask fbhermes…*, its room under it) and a mic. When you add it, pick its room: one of your agent rooms, or *the room I last used*; add one per room if you like, and long-press it to change its room. Widgets can't take typing themselves, so a tap opens the sheet with the keyboard up, and the mic opens it listening;
- **the Quick Settings tile** (edit your Quick Settings and drag *New session* in);
- **a long-press on the app icon**: a *New session in …* shortcut for each of your top agent rooms.

## Speak instead of typing

The **mic** in the message box (and in the New session sheet) uses your phone's speech input; what you say lands in the box to check and edit before you send. fluxchat doesn't record anything itself, so it needs no microphone permission.

## A room as its sessions

In a room where the agent answers each request in its own thread (Hermes outside DMs), every message you send starts a new session. So the room shows its **sessions as cards** instead of a chat, newest at the bottom by the message box:
- **Each card:** the session's title (your request), when it was last active, the run's state (*Working · 3m · 🔧 Running tests*, with a progress bar), and a summary: the first lines of the agent's latest answer. Finished sessions show Done / Failed, the reply count and the cost.
- **A request you just sent** shows straight away as *Waiting for the agent* until the agent picks it up.
- **Every session in the room** is there, from your server, not only the loaded history; older ones load as you scroll up. Messages in the room that aren't requests get a card too, so nothing is hidden.
- **Tap a card** to open the session. The message box starts a new one.
- **Long-press a card** to **pin** the session (long-press again to unpin). Pinned sessions move to a **Pinned** block at the top of the room, above everything else, and the project keeps its place at the top of the Projects list no matter how much newer work arrives.
- **The chat icon** in the top bar switches to the room's plain timeline; the cards icon switches back.

DMs, and rooms where the agent answers in the conversation itself, stay a chat.

## A room or thread

- **Sessions read as a transcript**, like Hermes's desktop app: in a thread, or a DM with an agent, your prompts are blocks on the right and the agent's answers run the full width as plain text, under its name once per turn. Its tool work is an activity block (*4 tool calls* and the latest step); tap it for every step. The one the agent is working on now spins and starts open; each terminal command is its own step. Hermes's timer heartbeat (*⏳ Working — 3 min*) is a single muted line. Once a turn is finished it becomes a card, faintly green when it succeeded and red when it failed (from the agent's ✅ / ❌ on your request): its work folds into one line (*Worked for 6m · 31 tool calls · 2 updates*; tap it for every step and note), so the answer stands on its own, with a line under it: *✓ Done · 6m · 31 tool calls · $0.34* and a copy button. The turn in progress, and any turn with an approval or question still waiting on you, stay open. Answers are set as a document: headings, lists, quotes, tables, and code blocks with their language and a copy button. An approval you've answered folds to one line (*✅ Approve once* and what it asked); tap it for the whole request. The agent's own 👀 / ✅ / ❌ reactions are left out, since the run card says the same.
- **Messages** elsewhere appear as bubbles. Agents' step logs fold into one line ("6 steps · 🔧 Running tests"); tap to see every step. Long answers fold too. Images show inline; tap one for full screen, and pinch to zoom.
- **Approvals and questions** appear as buttons under the message: Approve once / For this session / Always / Deny, or a question's numbered choices. A question can also be answered in your own words.
- **The run card** above the message box shows the state, time so far, the latest step, and a progress bar.
  - **Stop** sends the harness's stop command.
  - In a room whose run is happening in a thread, the card names the thread; tap it to open the thread.
- **While the agent works**, you choose what a new message does, as on the desktop: **Send**, **Steer** (read after its next step) or **Queue** (the next task).
- **Attachments:** the paperclip beside the message box sends a **photo or video** (Android's photo picker, no storage permission) or **any file**. The file waits above the box with a preview; what you type goes with it as its caption, in one message. In a thread it goes to the thread; from a room's sessions it starts a new session.
- **Files the agent (or anyone) sends** show as a card: name, size, type (and a video's thumbnail). Tap it to open it in an app that handles that type; the download button saves it where you choose. Files are downloaded (and decrypted) the first time you open or save them.
- **Share from other apps:** pick fluxchat in Android's share sheet (a screenshot, a PDF, a link, some text), then the agent room to send it to. The room opens with the file attached and the text in the message box, for you to add to and send. In a room shown as sessions, that starts a new session.
- **`/` commands**: type `/` for the agent's commands as you type. They go out the way its harness takes them (`!stop` for Hermes).
- **The ⋮ menu** has the agent's quick commands: Status, Usage, Context, Model, New session.
- **Threads follow you.** Send a request from a room. When the agent starts on it in a thread, the thread opens.

## Sessions

Hermes keeps one session per thread. In a thread (and in a room shown as its timeline, or a DM), tap the **clock icon** in the top bar for the room's **Sessions**: every thread in the room, from your server (not just the loaded history), grouped by last activity (Today, Yesterday, Previous 7 days, Previous 30 days, Older).
- **Each row:** the session's title (your request), its latest reply, when it was last active, and the run's state. A live run shows its step instead of the latest reply.
- **Search** filters by the request or the latest reply.
- **Tap a session** to open it. From one session to another, Back still goes to the room.
- **Long-press a session** to **pin** it (long-press again to unpin). Pinned sessions sit at the top of the list in their own block; a pin also keeps the project at the top of the Projects list. Pins live on your account, so the desktop sees them too.
- **A session's own screen** has the same pin as a 📌 button in its top bar.
- **New session:** in a room, it takes you to the room to send a fresh request, which opens a new thread. In a DM, where the agent doesn't use threads, it sends the agent's new-session command (`!new` for Hermes).

On a wide screen (840 dp or more: a tablet, an unfolded foldable, a phone in landscape) the sessions stay open in a sidebar beside the chat. On a phone the drawer opens from its button, not a swipe from the left edge, since that's Android's back gesture.

## Notifications

**While an agent works**, a quiet ongoing notification shows who's working, on which session, the current step, a running timer and a progress bar, with **Stop**. With several runs it lists them all. It keeps fluxchat syncing in the background so it stays current, and goes away when the last run ends. (You can turn it off in Android's notification settings: *Agents at work*.)

**Answering from the notification:** Android fits three buttons, so an approval gets **Approve once** and **Deny**, a question its first two choices, and **More** opens a reply box with every choice as a chip, or type your own answer.

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

## App lock

**Settings → Security → Require unlock** asks for your fingerprint, face, or screen lock when you open the app. Leaving the app locks it again — the screen you left, draft and half-sent photo included, waits right behind the lock. Detours you start yourself — the camera, the photo and file pickers — come back without asking again. A cold start comes up locked; while it's on, the content also stays out of screenshots and the app switcher. Notification buttons (Approve, Deny, Reply) still work over the lock screen — answering an agent doesn't need the app open. The quick entry points are deliberately *not* locked: the home-screen widget, the Quick Settings tile, the app shortcuts and share-into-fluxchat only ever send what you type, they never read your history back.

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
