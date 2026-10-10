# Hermes Agent: agent protocol support

Two patches for [Hermes Agent](https://github.com/NousResearch/hermes-agent) that make it send the [fluxchat agent protocol](../../docs/agent-protocol.md) on Matrix. They're ready to propose upstream; they haven't been sent.

## What they do

**0001: approvals.** Exec-approval prompts carry `org.fluxchat.agent` with `state: "waiting"` and their choices, labelled with the prompt's own (localized) action labels: `{"key": "✅", "label": "Allow once"}`. When the prompt is resolved by a reaction, or expires, the adapter edits it to `state: "working"`, so clients drop the buttons. Without this, clients have to notice the bot taking its reactions back.

**0002: run state, progress and spend.**
- Hermes's tool-progress message (the one it edits a line per tool) carries `state: "working"`, the request it belongs to (`run`), the latest step (the line without its emoji) and the step count.
- The final reply carries `state: "done"` or `"failed"`, plus the model, the share of the context window in use, the session's estimated spend (`cost_usd`; left out when Hermes can't price the model) and the run's one-line outcome (`result`, the reply's first line).
- When the run ends, the adapter edits the progress message to the same final state, so it never stays "working". That includes runs whose answer was streamed, and runs that were stopped.
- Approval prompts name the run they hold up.

Only adapters that declare `AGENT_STATUS = True` get the metadata, and only Matrix does. Other platforms see no change, and Matrix clients that don't know the key see what they see today. The 👀 / ✅ / ❌ reactions are unchanged.

With 0002 applied, fluxchat shows the run's steps from the protocol rather than parsing them out, and [budgets](../../docs/agents.md#budgets) see what Hermes spends.

## Apply and test

Made against `NousResearch/hermes-agent` at `38880bd2f1` (regenerated there on 2026-10-09 with the run result added; the earlier revision was `76534243`).

```bash
cd hermes-agent
git am path/to/fluxchat/integrations/hermes/000*.patch
pytest -q tests/gateway/test_agent_status.py tests/gateway/test_matrix_agent_protocol.py
```

Results here, with the `matrix`, `messaging`, `slack` and `telegram` extras and the `dev` and `test` groups installed (`uv sync --frozen`):
- The 14 new tests pass.
- Hermes's gateway tests for Matrix, approvals, progress, final delivery, streaming and turns (111 files): 999 pass.
- One fails, `test_update_streaming.py::…::test_gateway_flag_enables_gateway_prompt_for_stash`. It fails without the patches too.

## Pull request text

> **Matrix: agent protocol run state, progress and approvals**
>
> Matrix clients only see a Hermes run as reactions, a progress message and text, so they guess at its state, can't tell an approval prompt from a message, and never learn its spend. This adds an optional, namespaced `org.fluxchat.agent` object to the messages Hermes already sends:
>
> - the tool-progress message: `state: "working"`, `run` (the request's event ID), `step`, `steps`;
> - the final reply: `state: "done"` / `"failed"`, `model`, `context`, `cost_usd` (session total, only when priced), `result` (the reply's first line, the run's one-line outcome);
> - when the run ends, the progress message is edited to the final state;
> - approval prompts: `state: "waiting"`, the choices (reaction key + the prompt's own label) and the run, edited to `working` once resolved or expired.
>
> - Opt-in plumbing: adapters declare `AGENT_STATUS = True` (Matrix only) and read `metadata["fluxchat_agent"]` in `send()` / `edit_message()`. `gateway/agent_status.py` builds the fields.
> - No behaviour change for other platforms or for clients that ignore the key; reactions are untouched.
> - Tests: `tests/gateway/test_agent_status.py`, `tests/gateway/test_matrix_agent_protocol.py`.
>
> Spec: https://github.com/phrac/fluxchat/blob/main/docs/agent-protocol.md (draft, aiming for an MSC).
