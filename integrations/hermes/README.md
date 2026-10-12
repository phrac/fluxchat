# Hermes Agent: agent protocol support

Four patches for [Hermes Agent](https://github.com/NousResearch/hermes-agent) that make it send the [fluxchat agent protocol](../../docs/agent-protocol.md) on Matrix. 0001 and 0002 are ready to propose upstream; they haven't been sent. 0003 and 0004 fix bugs they expose and are ours alone until the other two land.

## What they do

**0001: approvals.** Exec-approval prompts carry `org.fluxchat.agent` with `state: "waiting"` and their choices, labelled with the prompt's own (localized) action labels: `{"key": "✅", "label": "Allow once"}`. When the prompt is resolved by a reaction, or expires, the adapter edits it to `state: "working"`, so clients drop the buttons. Without this, clients have to notice the bot taking its reactions back.

**0002: run state, progress and spend.**
- Hermes's tool-progress message (the one it edits a line per tool) carries `state: "working"`, the request it belongs to (`run`), the latest step (the line without its emoji) and the step count.
- The final reply carries `state: "done"` or `"failed"`, plus the model, the share of the context window in use, the session's estimated spend (`cost_usd`; left out when Hermes can't price the model) and the run's one-line outcome (`result`). The Matrix platform hint tells the agent to open its final reply with an outcome headline ("Deployed v0.2.0", "2 tests still failing"); `result` is that first line, stripped of heading/bold markup and capped at 240 chars — the headline is the agent's, the emitter only trims it.
- When the run ends, the adapter edits the progress message to the same final state, so it never stays "working". That includes runs whose answer was streamed, and runs that were stopped.
- Approval prompts name the run they hold up.

**0003: settle runs the gateway died on.** A gateway killed mid-run never reaches `on_processing_complete`, so its 👀 stays on the request and its working bubble stays `state: "working"` on the wire forever — fluxchat shows the run as running until its own stall sweep. The adapter now persists in-flight runs (👀 event ids + bubbles) to `in_flight_runs.json` in its store dir and, on the next connect, redacts the leftover 👀s and edits the leftover bubbles to `failed` ("interrupted by a gateway restart"). Crash-safe: re-settling after a crash between write and clear is a no-op on both signals.

**0004: settle the first working bubble, not the last tracked send.** 0002 tracked the run's settle target as "the most recent message sent with `state: working`", but later working sends (a step log, a heartbeat) are progress, not the bubble. When the queued-follow-up path ("final stream delivery not confirmed; sending first response before continuing") sent one, the run-end edit chased it and settled the wrong message; the real bubble stayed `state: "working"` on the wire forever (The Battlefront, 2026-10-12). The tracker now remembers only the FIRST working message per (room, run); edits of the bubble itself still refresh its remembered text, so 0003's restart-settle still lands the latest progress.

Only adapters that declare `AGENT_STATUS = True` get the metadata, and only Matrix does. Other platforms see no change, and Matrix clients that don't know the key see what they see today. The 👀 / ✅ / ❌ reactions are unchanged.

With 0002 applied, fluxchat shows the run's steps from the protocol rather than parsing them out, and [budgets](../../docs/agents.md#budgets) see what Hermes spends.

## Apply and test

Made against `NousResearch/hermes-agent` at `38880bd2f1` (regenerated there on 2026-10-10 so 0002 also carries the Matrix platform-hint guidance for writing the result headline; first regenerated 2026-10-09 with the run result added, earlier revision `76534243`).

```bash
cd hermes-agent
git am path/to/fluxchat/integrations/hermes/000*.patch
pytest -q tests/gateway/test_agent_status.py tests/gateway/test_matrix_agent_protocol.py tests/gateway/test_matrix_settle_orphaned_runs.py
```

Results here, with the `matrix`, `messaging`, `slack` and `telegram` extras and the `dev` and `test` groups installed (`uv sync --frozen`):
- The 22 new tests pass (0003 adds `tests/gateway/test_matrix_settle_orphaned_runs.py`, 3 tests; 0004 adds 2 more there).
- Hermes's gateway tests for Matrix, approvals, progress, final delivery, streaming and turns (111 files): 999 pass.
- One fails, `test_update_streaming.py::…::test_gateway_flag_enables_gateway_prompt_for_stash`. It fails without the patches too.

## Pull request text

> **Matrix: agent protocol run state, progress and approvals**
>
> Matrix clients only see a Hermes run as reactions, a progress message and text, so they guess at its state, can't tell an approval prompt from a message, and never learn its spend. This adds an optional, namespaced `org.fluxchat.agent` object to the messages Hermes already sends:
>
> - the tool-progress message: `state: "working"`, `run` (the request's event ID), `step`, `steps`;
> - the final reply: `state: "done"` / `"failed"`, `model`, `context`, `cost_usd` (session total, only when priced), `result` (the reply's first line — the Matrix prompt hint asks the agent to open with a one-line outcome, so it reads as a headline, not a transcript fragment);
> - when the run ends, the progress message is edited to the final state;
> - approval prompts: `state: "waiting"`, the choices (reaction key + the prompt's own label) and the run, edited to `working` once resolved or expired.
>
> - Opt-in plumbing: adapters declare `AGENT_STATUS = True` (Matrix only) and read `metadata["fluxchat_agent"]` in `send()` / `edit_message()`. `gateway/agent_status.py` builds the fields.
> - No behaviour change for other platforms or for clients that ignore the key; reactions are untouched.
> - Tests: `tests/gateway/test_agent_status.py`, `tests/gateway/test_matrix_agent_protocol.py`.
>
> Spec: https://github.com/phrac/fluxchat/blob/main/docs/agent-protocol.md (draft, aiming for an MSC).
