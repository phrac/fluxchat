# The fluxchat agent protocol

A small, open set of fields an AI agent (or the harness running it) can add to its Matrix messages so that clients know what it's doing instead of guessing from reactions and message shapes. fluxchat reads it today; anyone may implement it. It's a draft, namespaced as `org.fluxchat.agent`; if it proves useful the plan is to propose it as a Matrix spec change (MSC).

It needs nothing new on the server. The fields ride along in ordinary `m.room.message` events (and their edits), so clients that don't know them just show the text.

## The fields

Put an `org.fluxchat.agent` object in the message content. Every field is optional.

| Field | Type | Meaning |
|---|---|---|
| `v` | integer | Protocol version: `1`. |
| `run` | string | Event ID of the request (the user's message) this message is about. |
| `state` | string | `working`, `waiting` (needs the user: a question or an approval), `done` or `failed`. |
| `step` | string | What it's doing right now, short: `"Running the tests"`. |
| `steps` | integer | How many steps so far. |
| `progress` | number | 0.0–1.0, only when the agent actually knows how far along it is. Clients show a filled bar; without it, an indeterminate one. |
| `model` | string | Model in use: `"claude-opus-5"`. |
| `context` | number | Share of the context window in use, 0.0–1.0. |
| `cwd` | string | Working directory, if that means something for the agent. |
| `cost_usd` | number | Spend on this session so far. |
| `choices` | array | A prompt: `[{ "key": "✅", "label": "Approve" }, …]`. The user answers by reacting with a `key`. |

## How to use it

**One status message per run, edited.** When a run starts, send a status message in the run's thread (or in the room) with `state: "working"`, and edit it (`m.replace`) as the run moves: new `step`, `steps`, `progress`. Put the fields in `m.new_content` too, as with any edit; clients read the latest edit. End with `state: "done"` or `"failed"`.

**Ask with `choices`.** Send a message whose text asks the question, with `state: "waiting"` and the `choices`. The user's client shows them as buttons, which send a reaction with the chosen `key`. When you've acted on the answer, edit the prompt to the next state (`working`, `done`, …): the buttons go away.

**Answers go where the request was.** Keep replying in the request's thread if you use threads; `run` ties messages to the request either way.

## Example

A status message, edited as the run goes:

```json
{
  "msgtype": "m.notice",
  "body": "🧪 Running the tests (3/5)",
  "m.relates_to": { "rel_type": "m.thread", "event_id": "$request" },
  "org.fluxchat.agent": {
    "v": 1,
    "run": "$request",
    "state": "working",
    "step": "Running the tests",
    "steps": 3,
    "progress": 0.6,
    "model": "claude-opus-5",
    "context": 0.42,
    "cost_usd": 0.18
  }
}
```

An approval:

```json
{
  "msgtype": "m.text",
  "body": "I want to run `rm -rf build/`. OK?",
  "m.relates_to": { "rel_type": "m.thread", "event_id": "$request" },
  "org.fluxchat.agent": {
    "v": 1,
    "run": "$request",
    "state": "waiting",
    "choices": [
      { "key": "✅", "label": "Approve once" },
      { "key": "❌", "label": "Deny" }
    ]
  }
}
```

Sending it from Python with [mautrix](https://github.com/mautrix/python) (any Matrix SDK works the same way, it's just content):

```python
content = {
    "msgtype": "m.notice",
    "body": f"{step} ({n}/{total})",
    "m.relates_to": {"rel_type": "m.thread", "event_id": request_id},
    "org.fluxchat.agent": {"v": 1, "run": request_id, "state": "working",
                           "step": step, "steps": n, "progress": n / total},
}
status_id = await client.send_message_event(room_id, EventType.ROOM_MESSAGE, content)
```

## What fluxchat does with it

When an agent's messages carry these fields, fluxchat uses them ahead of what its harness profile would infer:

- the run's state, step and step count drive the run bar, the agent chip in the room header, the task board and the Runs dialog, and `progress` turns the moving bar into a filled one;
- `model`, `context`, `cost_usd` and `cwd` show in the agent chip and the status bar;
- `choices` on a `waiting` message become buttons, and you get a desktop notification;
- runs that go from `working` to `done` / `failed` can notify you.

Without the fields, fluxchat falls back to the agent's [harness profile](agents.md#harness-profiles): Hermes's reactions, OpenClaw's typing, and so on.
