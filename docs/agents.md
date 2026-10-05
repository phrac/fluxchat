# AI agents

fluxchat has controls for AI agents you talk to over Matrix, built around [Hermes Agent](https://hermes-agent.nousresearch.com/docs/user-guide/messaging/matrix). Other agents work as far as they use the same conventions.

## Mark someone as an agent

Click the agent's name or avatar to open its profile and turn on **AI agent**. It gets an *AI agent* badge, and it's listed in **Settings → Preferences → AI agents**, where you can remove it again.

A room with an agent in it (a DM with your agent, or a room it's in) gets everything below.

## The agent chip

The room header shows the agent next to the room name: a robot icon, its name, and what it's doing: **idle**, **working · 3m** with a moving progress bar, or **waiting on you**. Hover it for the current step. Click it for quick commands for this room or thread: **Stop the run** (while it works), **Status**, **Usage and cost**, **Context window**, **Switch model**, **New session**, and the agent's profile.

## Commands

Hermes's commands work with `/`, the way you'd type them anywhere else: `/goal`, `/stop`, `/status`, `/model`, `/usage`, `/new`… fluxchat sends them as `!goal`, `!stop` and so on, which is how Hermes takes commands on Matrix.

- Typing `/` lists the agent's commands first (marked with a robot icon), then fluxchat's.
- Where both have a command with the same name (`/status`, `/sessions`, `/help`), the agent's wins in its rooms.
- Commands only fluxchat has (`/me`, `/gif`, `/attach`…) still work as usual.
- Anything else that starts with `/` goes to the agent too, so its skills work (`/arxiv …`).
- `//text` still sends `/text` as a plain message, and you can type `!command` yourself.

## Progress

Hermes reports what it's doing in one message it keeps editing, a line per tool ("📖 Reading notes.md", "🔍 Searching the web for …"). fluxchat uses that message:

- The run bar and the agent chip show the latest step, the step count, the time since your request, and a moving progress bar while it works.
- In the timeline, a progress message with several steps is folded to **N steps** plus the latest one. Click **N steps** to see them all, **Hide steps** to fold it again.

Hermes doesn't say how far along it is, so the bar shows that it's working, not a percentage.

## Runs

Hermes reacts 👀 to your message while it works on it, then ✅ when it's done or ❌ when it failed, and answers in a thread on that message. fluxchat shows this as the run's state:

- the "N replies" row under your message shows **working**, **needs you**, **done** or **failed**;
- the thread's entry in the room list gets a coloured dot while it's working, waiting for you, or failed;
- a room with a run waiting for you gets a yellow dot in the room list.

### Follow new threads

When the agent starts a thread on a message you just sent in the room you're in, the thread opens beside the room by itself (reusing the room's thread pane if one is open). You stay in the room. Turn this off in **Settings → Preferences → AI agents → Open agent threads as they start**.

### The run bar

While the agent works on the thread (or room) you're looking at, a bar above the message box shows it (time so far, current step, progress bar), with:

- **Stop**: interrupts the run (sends `!stop`).
- **Next message**: what a message you send while it's working does.
  - **Send**: a plain message; the agent's own setting decides (Hermes redirects the run by default).
  - **Steer**: sent as `!steer`; the agent reads it after its next step, without stopping.
  - **Queue**: sent as `!queue`; it runs as the next turn, after this one.

  The choice is remembered. Panes that aren't focused show the state and Stop.

## Approvals and choices

When Hermes asks for approval (a command it considers dangerous), or offers a choice (`/model`, `/reasoning`…), it posts a message and reacts to it with the options. fluxchat turns those into buttons on the message: **Approve once**, **For this session**, **Always**, **Deny**, or the picker's options by name. Click one; it stays lit until the agent acts on it and the prompt goes away.

You get a desktop notification when an agent asks, even with *mentions only* on, unless you're looking at the room. Prompts older than an hour (left behind by a restarted agent) show as ordinary reactions.
