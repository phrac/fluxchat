# fluxchat-agent

Emit the [fluxchat agent protocol](https://github.com/phrac/fluxchat/blob/main/docs/agent-protocol.md) from a Matrix agent: run state, steps, progress, runtime and approval choices, as fields in ordinary messages. No Matrix SDK dependency: pass a `send(content) -> event_id` coroutine from the one you use.

```python
from fluxchat_agent import Run

run = Run(send, request_event_id, model="claude-opus-5")
await run.start("Reading the repo")
await run.step("Running the tests", progress=0.5)
await run.done("All green.")
```

Tests: `python3 -m unittest discover -s tests`.
