# AI agents

fluxchat is a Matrix client, and it treats the AI agents you talk to over Matrix as first-class: you can see what they're doing, steer or stop them, answer their approvals, and follow every run across your rooms. It works with any agent harness that has a Matrix front end. What each one does is described by a [harness profile](#harness-profiles): Hermes Agent, OpenClaw and Claude Code (through a Matrix bridge) are built in, and a new one is a small TOML file.

## Mark someone as an agent

Click the agent's name or avatar to open its profile and turn on **AI agent**, then pick its **Harness** (Hermes Agent, OpenClaw, Claude Code (Matrix channel), Generic agent, or one you added). It gets an *AI agent* badge, and it's listed in **Settings → Preferences → AI agents**, where you can change its harness or remove it.

A room with an agent in it (a DM with your agent, or a room it's in) gets everything below. fluxchat also keeps those rooms' timelines loaded in the background, so it knows about runs in rooms you aren't looking at.

## The agent chip

The room header shows the agent next to the room name: a robot icon, its name, and what it's doing: **idle**, **working · 3m** with a progress bar, or **waiting on you**, plus its model and context use when it reports them. Hover it for the current step. Click it for:

- **Stop the run** (while it works, if the harness has a stop command);
- **Tell me when it's done**;
- the harness's quick commands (Hermes: Status, Usage and cost, Context window, Switch model, New session);
- **All agent runs**, and the agent's profile.

## Commands

The agent's commands work with `/`, the way you'd type them anywhere else, and go out the way its harness expects: Hermes takes `!goal`, `!stop`…; OpenClaw takes `/stop` as text, starting with its mention in rooms.

- Typing `/` lists the agent's commands first (marked with a robot icon), then fluxchat's.
- Where both have a command with the same name (`/status`, `/sessions`, `/help`), the agent's wins in its rooms.
- Commands only fluxchat has (`/me`, `/gif`, `/snippet`…) still work as usual.
- Anything else that starts with `/` goes to the agent too, so its skills work (`/arxiv …`).
- `//text` still sends `/text` as a plain message, and you can type `!command` yourself.

## Runs

A run is one request and the agent's work on it. fluxchat shows its state, **working**, **needs you**, **done** or **failed**, from whatever the agent gives: the [agent protocol](agent-protocol.md) when it uses it, else its harness's signals (Hermes reacts 👀 / ✅ / ❌ to your message; OpenClaw and the Claude Code channel type while they work).

- The "N replies" row under your request shows the state. The agent's own 👀 / ✅ / ❌ on your message are left out of its reactions there, since the row already says it (anyone else's reactions still show).
- Agents that answer in a thread on your request (Hermes, by default) have the run in that thread from the moment they pick it up: its bar, Stop and Steer / Queue go to the thread, which is where the agent's session for it is. fluxchat learns this per room from how the agent answered your last finished request, and falls back to the harness's habit (`reply_in_thread`).
- The thread's entry in the room list gets a coloured dot while it's working, waiting for you, or failed; a room with a run waiting for you gets a yellow dot.
- The status bar shows **2 running · 1 waiting** across all rooms; click it for the Runs dialog.

### Follow new threads

When the agent starts a thread on a message you just sent in the room you're in, the thread opens beside the room by itself (reusing the room's thread pane if one is open). You stay in the room. For an agent that answers in threads, the thread opens as soon as it picks up your message, so its progress shows there from the start. Turn this off in **Settings → Preferences → AI agents → Open agent threads as they start**.

### The run bar

While the agent works on the thread (or room) you're looking at, a bar above the message box shows it: the time so far, the current step, and a progress bar (filled when the agent says how far along it is, moving otherwise). It has:

- **Stop**: interrupts the run, when the harness has a command for it.
- **🔔**: tell me when it's done (a desktop notification when it finishes, even if it's quick).
- **Next message** (harnesses with steer / queue, like Hermes): what a message you send while it's working does.
  - **Send**: a plain message; the agent's own setting decides (Hermes redirects the run by default).
  - **Steer**: read after its next step, without stopping.
  - **Queue**: runs as the next turn, after this one.

  The choice is remembered. Panes that aren't focused show the state and Stop.

In a room whose run is in a thread that isn't on screen, the room's message box shows that run instead: **fbhermes is working in "now the same scene at dusk"**. Click the name of the thread to open it; Stop goes to the thread.

### Notifications

- When an agent asks for approval or a choice, you're notified even with *mentions only* on, unless you're looking at the room. On Linux and macOS the notification has the choices as buttons (and **Stop**), so you can answer without opening fluxchat.
- When a run of 30 seconds or more finishes (or fails), you're notified unless you're looking at it. Turn this off in **Settings → Preferences → AI agents → Notify when a run finishes**; the 🔔 still works for runs you pick.

## Several agents in a room

A room can have more than one agent: a researcher, a coder and a reviewer, say. fluxchat follows each one's runs separately, and the room header shows a chip per agent with its own state.

- **Who you're talking to** is outlined in the header and picked in the **To** switch above the message box: commands (`/stop`…) and the run controls go to that agent, the way its harness expects. By default it's the agent on the run you're looking at, else the one that spoke last. Click an agent's chip to talk to it (and open its menu).
- **Hand-offs:** when one agent's message addresses another ("@coder can you take the tests?"), the message shows **researcher handed off to coder**. Click it to talk to the one it went to.

## Review what agents change

When an agent posts a diff (a `git diff` in a code block, or a `.diff` / `.patch` file), the message gets a **Review changes · 3 files +40 −12** button. It opens a review: the changed files on the left, each file's diff with old and new line numbers on the right, additions and deletions coloured.

- If the agent is waiting on an approval for it, the review shows that prompt's choices (**Approve once**, **Deny**…), and picking one answers the prompt.
- Otherwise **Approve** and **Request changes** reply in the thread, with your comment if you wrote one.

## The task board

In a room with an agent, the **Threads** panel (header button) becomes **Runs**: every thread is a run, with its state, how long it ran, and its current step. Runs waiting on you and running ones come first. **Active** shows only what's still going; **All** shows everything.

## All your runs

**Agent runs** (the status bar's run count, the agent chip menu, or the palette) lists every run across your rooms: waiting on you, working, then the last few hours' finished ones, with the room and current step. Click one to open it.

Type in the search box to find runs by title, room or agent. Under **History** are the runs fluxchat saw happen, newest first, with how long they took, and the model and cost at the end when the agent reports them. The ⬇ button on any run exports its thread as a Markdown transcript (with the run's details at the top). The history is kept encrypted on this computer.

A run that shows no sign of life for two hours while "working" (no new message or thread reply from the agent, and it isn't typing) is no longer counted as running: the agent most likely restarted without taking back its 👀. It moves to History as **stalled**. Lone commands like `!usage` or `!status` aren't runs and don't go into the history; a command with a task (`!goal ship the release`) does.

**Watch in a window** opens a new window tiling the active runs, one pane each. It follows: runs that start later get a pane in it too (up to six). Use it as a dashboard on a second screen, or switch to it with **Ctrl+B n**.

## Progress, answers and runtime

- Progress posted as a message of steps, a line per tool ("📖 Reading notes.md", "🔍 Searching the web for …"), is folded to **N steps** plus the latest one. Click to see them all.
- Long answers (over 30 lines) are folded to their first screenful, with **Show full answer**.
- Code blocks have a **copy** button; diffs (`git diff` output, tagged or not) are highlighted.
- PDFs show their first page.
- Model, context use, cost and working directory show in the agent chip and the status bar, from the [agent protocol](agent-protocol.md) or Hermes's runtime footer (`/footer on`), which is taken off the answers.

## Approvals and choices

When an agent asks for approval (a command it considers dangerous) or offers a choice (`/model`, `/reasoning`…), fluxchat shows the options as buttons on the message: **Approve once**, **For this session**, **Always**, **Deny**, or the options by name. Clicking one sends the reaction the agent waits for; it stays lit until the agent acts on it. This works for agents that offer choices as reactions on their own message (Hermes, the Claude Code channel) and for the [agent protocol](agent-protocol.md)'s `choices`. Prompts older than an hour show as ordinary reactions, and so do prompts that are over: the agent has posted after it (a status line like Hermes's "⏳ Working — 3 min" doesn't count), or it was edited to say so (Hermes's "⌛ Approval timed out…").

Some agents ask questions as plain text and take your reply as the answer: Hermes's "❓ Which docs should I index?" with numbered options. While one is waiting (nobody has posted after it), the run shows **waiting on you**, you're notified, and the options appear as buttons under the question; a button sends its number, which is what the agent expects. You can also just type your answer. A question without options asks you to reply below.

## Your prompts

- **Ctrl+R** searches what you've sent (the palette in `<` mode): pick one to put it back in the message box.
- `/snippet <name> <text>` saves a prompt you use often; it shows first in Ctrl+R. `/snippet` lists them, `/unsnippet <name>` removes one.

Your history and snippets are kept encrypted on this computer, like the media cache.

## Spotting agents

If someone in a room acts like an agent you haven't marked (they react 👀 / ✅ to several of your messages, post prompts they react to themselves, post step logs or agent-protocol fields), a bar above the message box says **… looks like an AI agent (Hermes Agent)** with the harness it matches. **Mark as agent** marks it with that harness; **It isn't** stops the suggestion for that person.

## Voice

Hold the 🎤 to talk: the voice message is sent when you let go, into the thread you're in. A quick tap records until you tap again, then lets you listen before sending. Agents that take voice notes (Hermes does) get it as a prompt.

## Harness profiles

A harness profile tells fluxchat how one agent framework behaves on Matrix. Profiles are TOML files in fluxchat's data folder, under `agents/` (`~/.local/share/fluxchat/agents/` on Linux, `~/Library/Application Support/fluxchat/agents/` on macOS, `%APPDATA%\fluxchat\agents\` on Windows). The built-in ones are written there on first start; edit them, or copy one to describe another harness. Files are read at start, and a file with a built-in's `id` replaces it.

| Built in | Commands | Working | Done / failed | Approvals |
|---|---|---|---|---|
| Hermes Agent | `!name` | 👀 on your message | ✅ / ❌ on it | reactions on its prompt: ✅ 🌀 ♾️ ❌ |
| OpenClaw | `/name`, with its mention in rooms | typing (and its ack reaction) | — | ✅ / ❌ |
| Claude Code (Matrix channel) | — | typing | — | ✅ / ❌ on the permission prompt |
| Generic agent | `/name` | typing | — | the agent protocol |

Every field is optional; leave out what doesn't apply. A file with a built-in's `id` (like the copies fluxchat puts in the folder) only needs what you change: anything it leaves out comes from the built-in, so newer settings reach older copies.

```toml
id = "my-harness"            # how agents refer to it
name = "My harness"          # shown in the picker
about = "One line about it"
url = "https://…"

command_prefix = "/"         # "!" → !stop, "/" → /stop as text
mention_commands = false     # outside DMs, start commands with the agent's ID
stop = "stop"                # command names, without prefix; "" = none
steer = ""
queue = ""

working_reaction = ""        # reactions on your request; "" = not used
done_reaction = ""
failed_reaction = ""
typing_means_working = true  # the agent typing = working
reply_in_thread = false      # answers in a thread on your request (outside DMs)
question_prefix = ""         # text questions start with this ("❓"); answered by replying
prompt_over_markers = []     # a prompt edited to start with one of these is over (["⌛"])

progress_steps = true        # progress as "emoji step" lines
runtime_footer = false       # "model · 42% · ~/dir" under answers

approvals = [ { emoji = "✅", label = "Approve" }, { emoji = "❌", label = "Deny" } ]
menu = [ { label = "Status", command = "status" } ]
commands = [ { name = "stop", description = "Stop the run" } ]
```

For an agent or harness you write yourself, the cleanest route is the [agent protocol](agent-protocol.md): a few fields in its messages, and fluxchat needs no guessing at all.

**pi** and other harnesses without a Matrix front end yet: give them one (a small bridge or bot, or ask your agent to write it), then describe it with a profile or have it speak the agent protocol.

## Matrix first

fluxchat speaks Matrix and only Matrix: one open, encrypted, self-hostable protocol, with bridges to most other chat networks. If you want fluxchat on another platform, have your agent build it and send us a pull request.
