# Agent protocol emitters

Small libraries for sending the [fluxchat agent protocol](../docs/agent-protocol.md) from an agent or harness: run state, steps, progress, runtime and approval choices, as fields in ordinary Matrix messages. They don't depend on any Matrix SDK: they build the content, and you pass a `send(content) -> event_id` function from whatever you use (mautrix, matrix-nio, matrix-js-sdk, matrix-bot-sdk…).

| | Path | Tests |
|---|---|---|
| Python ≥ 3.9 | [`python/`](python) (`fluxchat_agent`) | `python3 -m unittest discover -s python/tests` |
| TypeScript (Node ≥ 23.6) | [`typescript/`](typescript) (`fluxchat-agent`) | `cd typescript && npm test` |

Both have the same shape: `fields()`, `message()`, `edit()` for raw content; `Run` for the usual flow (one status message per run, edited as it goes; prompts answered by reaction); `choice_from_reaction` / `choiceFromReaction` for your reaction handler.

```python
from fluxchat_agent import Run

async def send(content):                     # mautrix
    return await client.send_message_event(room_id, EventType.ROOM_MESSAGE, content)

run = Run(send, request_event_id, model="claude-opus-5")
await run.start("Reading the repo")
await run.step("Running the tests", progress=0.5)
prompt = await run.ask("Delete build/?", [("✅", "Approve"), ("❌", "Deny")])
# … in your reaction handler: choice_from_reaction(content, prompt, choices) …
await run.answered(prompt, "Approved")
await run.done("All green.")
```

```ts
import { Run } from "fluxchat-agent";

const send = (content) => client.sendEvent(roomId, "m.room.message", content).then((r) => r.event_id); // matrix-js-sdk
const run = new Run(send, requestEventId, { runtime: { model: "claude-opus-5" } });
await run.start("Reading the repo");
await run.step("Running the tests", { progress: 0.5 });
await run.done("All green.");
```

They're not on PyPI / npm yet; vendor the file or install from the repo path.
