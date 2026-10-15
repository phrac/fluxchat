# fluxchat-agent

Emit the [fluxchat agent protocol](https://github.com/phrac/fluxchat/blob/main/docs/agent-protocol.md) from a Matrix agent: run state, steps, progress, runtime and approval choices, as fields in ordinary messages. No Matrix SDK dependency: pass a `send(content) => Promise<eventId>` from the one you use.

```ts
import { Run } from "fluxchat-agent";

const run = new Run(send, requestEventId, { runtime: { model: "claude-opus-5" } });
await run.start("Reading the repo");
await run.step("Running the tests", { progress: 0.5 });
await run.done("All green.");
```

Approval prompts: `run.ask(question, choices)` posts the question and moves the status to `waiting`; the user answers by reacting with a choice's key — map it back with `choiceFromReaction(reactionContent, promptEventId, choices)`, then settle the bubble with `run.answered(promptEventId)`. `run.failed(error)` closes a run that didn't make it; `run.updateRuntime({ cost_usd })` keeps cost/model current on every edit.

The raw event builders are exported too (`fields`, `message`, `edit`) if you'd rather place the protocol events yourself.

Tests: `npm test` (node --test, ESM, Node ≥ 23.6).
