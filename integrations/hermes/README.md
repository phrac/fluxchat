# Hermes Agent: agent protocol support

A patch for [Hermes Agent](https://github.com/NousResearch/hermes-agent)'s Matrix adapter that sends the [fluxchat agent protocol](../../docs/agent-protocol.md) with its exec-approval prompts. It's ready to propose upstream; it hasn't been sent.

## What it does

- Approval prompts carry `org.fluxchat.agent` with `state: "waiting"` and their choices, labelled with the prompt's own (localized) action labels: `{"key": "✅", "label": "Allow once"}`.
- When the prompt is resolved by a reaction, or expires, the adapter edits it to `state: "working"`, so clients drop the buttons. Today clients have to notice the bot taking its reactions back.
- `send()` puts `metadata["fluxchat_agent"]` into the content, and `edit_message()` takes `agent_status=`, so other gateway code can report run state and progress the same way (see Next).

Clients that don't know the key see exactly what they see today.

## Apply and test

Made against `NousResearch/hermes-agent` at `76534243`.

```bash
cd hermes-agent
git am path/to/fluxchat/integrations/hermes/0001-Matrix-agent-protocol-status-on-exec-approvals-org.f.patch
pytest -q tests/gateway/test_matrix_agent_protocol.py tests/gateway/test_matrix_exec_approval.py
```

Results here: the 3 new tests pass. The 20 Matrix test files give the same results with and without the patch (193 passed). The 17 that fail did so before the patch too: they need the `matrix` extra's dependencies, which weren't installed.

## Pull request text

> **Matrix: agent protocol status on exec approvals**
>
> Matrix clients only see an approval prompt as text plus the bot's own reactions, so they can't tell a prompt from a message and can't show it as buttons. This adds an optional, namespaced `org.fluxchat.agent` object to approval prompts: `state: "waiting"` and the choices (reaction key + the prompt's own label), and edits the prompt to `state: "working"` once it's resolved or expires.
>
> - Opt-in plumbing: `send()` reads `metadata["fluxchat_agent"]`; `edit_message()` takes `agent_status=`.
> - No behaviour change for clients that ignore the key; the reactions flow is untouched.
> - Tests: `tests/gateway/test_matrix_agent_protocol.py`.
>
> Spec: https://github.com/phrac/fluxchat/blob/main/docs/agent-protocol.md (draft, aiming for an MSC).

## Next

Run state and progress come from the gateway, not the adapter: the 👀 / ✅ / ❌ processing reactions and the tool-progress bubble (`gateway/run_turn_runner.py`). A follow-up would pass `fluxchat_agent` metadata from there: `working` + `step` + `steps` on each progress edit, and `done` / `failed` with the runtime footer's model and context. Until then, fluxchat's Hermes harness profile reads those from the reactions and progress lines.
