"""Emit the fluxchat agent protocol from any Matrix agent or harness.

The protocol (docs/agent-protocol.md) is a few fields an agent puts in its
Matrix messages under ``org.fluxchat.agent`` so clients know what it's doing:
run state, current step, progress, runtime and approval choices. This module
builds that content; you send it with whatever Matrix library you use.

    from fluxchat_agent import Run

    run = Run(send, request_event_id="$abc", thread=True)
    await run.start("Reading the repo")
    await run.step("Running the tests", progress=0.5)
    answer = await run.ask("Delete build/?", [("✅", "Approve"), ("❌", "Deny")])
    await run.done("All green.")

``send(content) -> event_id`` is an async callable that sends an
``m.room.message`` with that content (to the room the request is in) and
returns the new event's ID.
"""

from __future__ import annotations

from typing import Any, Awaitable, Callable, Iterable, Optional, Sequence, Tuple, Union

__all__ = ["KEY", "VERSION", "STATES", "fields", "message", "edit", "Run", "choice_from_reaction"]

KEY = "org.fluxchat.agent"
VERSION = 1
STATES = ("working", "waiting", "done", "failed")

Choice = Union[Tuple[str, str], dict]
Send = Callable[[dict], Awaitable[str]]


def _unit(name: str, value: Optional[float]) -> Optional[float]:
    if value is None:
        return None
    value = float(value)
    if not 0.0 <= value <= 1.0:
        raise ValueError(f"{name} must be between 0 and 1, got {value}")
    return value


def _choices(choices: Optional[Iterable[Choice]]) -> Optional[list]:
    if choices is None:
        return None
    out = []
    for c in choices:
        key, label = (c["key"], c["label"]) if isinstance(c, dict) else c
        if not key or not label:
            raise ValueError("a choice needs a key (the reaction to answer with) and a label")
        out.append({"key": str(key), "label": str(label)})
    if not out:
        raise ValueError("choices can't be empty")
    return out


def fields(
    *,
    run: Optional[str] = None,
    state: Optional[str] = None,
    step: Optional[str] = None,
    steps: Optional[int] = None,
    progress: Optional[float] = None,
    model: Optional[str] = None,
    context: Optional[float] = None,
    cwd: Optional[str] = None,
    cost_usd: Optional[float] = None,
    result: Optional[str] = None,
    choices: Optional[Iterable[Choice]] = None,
) -> dict:
    """The ``org.fluxchat.agent`` object (only the fields given)."""
    if state is not None and state not in STATES:
        raise ValueError(f"state must be one of {STATES}, got {state!r}")
    if steps is not None and steps < 0:
        raise ValueError("steps can't be negative")
    out: dict[str, Any] = {"v": VERSION}
    for name, value in (
        ("run", run), ("state", state), ("step", step), ("steps", steps),
        ("progress", _unit("progress", progress)), ("model", model),
        ("context", _unit("context", context)), ("cwd", cwd),
        ("cost_usd", None if cost_usd is None else float(cost_usd)),
        ("result", result),
        ("choices", _choices(choices)),
    ):
        if value is not None:
            out[name] = value
    return out


def message(body: str, *, thread: Optional[str] = None, notice: bool = False, **agent: Any) -> dict:
    """Content for a new message carrying the fields (``thread``: the
    thread root to post in)."""
    content: dict[str, Any] = {"msgtype": "m.notice" if notice else "m.text", "body": body, KEY: fields(**agent)}
    if thread:
        content["m.relates_to"] = {"rel_type": "m.thread", "event_id": thread,
                                   "is_falling_back": True, "m.in_reply_to": {"event_id": thread}}
    return content


def edit(event_id: str, body: str, *, notice: bool = False, **agent: Any) -> dict:
    """Content for an edit (``m.replace``) of ``event_id``. The fields go in
    ``m.new_content``, which is what edit-aware clients read, and at the top
    level for the rest."""
    msgtype = "m.notice" if notice else "m.text"
    meta = fields(**agent)
    return {
        "msgtype": msgtype,
        "body": f"* {body}",
        KEY: meta,
        "m.new_content": {"msgtype": msgtype, "body": body, KEY: meta},
        "m.relates_to": {"rel_type": "m.replace", "event_id": event_id},
    }


def choice_from_reaction(content: dict, prompt_event_id: str, choices: Sequence[Choice]) -> Optional[str]:
    """The choice key a reaction answers ``prompt_event_id`` with, if it's one
    of ``choices`` (for wiring up your reaction handler)."""
    rel = content.get("m.relates_to") or {}
    if rel.get("rel_type") != "m.annotation" or rel.get("event_id") != prompt_event_id:
        return None
    key = rel.get("key")
    keys = {c["key"] for c in _choices(choices) or []}
    bare = lambda k: (k or "").replace("️", "")  # noqa: E731 - ✅ with or without VS16
    return next((k for k in keys if bare(k) == bare(key)), None)


class Run:
    """One request and the agent's work on it, reported through a single
    status message that's edited as the run moves.

    ``request_event_id``: the user's message this run answers. ``thread``:
    post in the request's thread (``True``), in another thread (an event ID),
    or in the room (``False``). Runtime info given here (``model``,
    ``context``, ``cwd``, ``cost_usd``) rides along on every update.
    """

    def __init__(self, send: Send, request_event_id: str, *, thread: Union[bool, str] = True, **runtime: Any):
        self._send = send
        self.run = request_event_id
        self.thread = request_event_id if thread is True else (thread or None)
        self.runtime = runtime
        self.status_id: Optional[str] = None
        self.steps = 0
        self.step_text: Optional[str] = None

    def _meta(self, **agent: Any) -> dict:
        return {"run": self.run, **self.runtime, **agent}

    async def _status(self, body: str, **agent: Any) -> str:
        meta = self._meta(**agent)
        if self.status_id is None:
            self.status_id = await self._send(message(body, thread=self.thread, notice=True, **meta))
        else:
            await self._send(edit(self.status_id, body, notice=True, **meta))
        return self.status_id

    def update_runtime(self, **runtime: Any) -> None:
        """Model / context / cwd / cost for the next updates."""
        self.runtime.update({k: v for k, v in runtime.items() if v is not None})

    async def start(self, step: Optional[str] = None) -> str:
        if step:
            self.steps, self.step_text = 1, step
        return await self._status(step or "Working…", state="working", step=step, steps=self.steps or None)

    async def step(self, text: str, *, progress: Optional[float] = None) -> str:
        self.steps += 1
        self.step_text = text
        return await self._status(f"{text} ({self.steps})", state="working", step=text,
                                  steps=self.steps, progress=progress)

    async def ask(self, question: str, choices: Sequence[Choice]) -> str:
        """Post a prompt; the user answers by reacting with a choice's key.
        Call ``answered`` once you've acted on it, so clients drop the buttons.
        Returns the prompt's event ID."""
        self._prompt = (question, choices)
        await self._status(question, state="waiting", step=self.step_text, steps=self.steps or None)
        return await self._send(message(question, thread=self.thread, state="waiting", choices=choices,
                                        **{k: v for k, v in self._meta().items() if k != "choices"}))

    async def answered(self, prompt_event_id: str, note: str = "Answered") -> None:
        """The prompt is resolved: edit it out of "waiting", resume working."""
        question, choices = getattr(self, "_prompt", ("", ()))
        await self._send(edit(prompt_event_id, f"{question}\n\n{note}".strip(), **self._meta(state="working", choices=choices)))
        await self._status(self.step_text or "Working…", state="working", step=self.step_text, steps=self.steps or None)

    async def done(self, summary: str = "Done", *, result: Optional[str] = None) -> str:
        """End the run. ``result``: the one-line outcome (protocol ``result``);
        ``summary`` stays the status message's body."""
        return await self._status(summary, state="done", steps=self.steps or None, progress=1.0,
                                  result=result or summary)

    async def failed(self, error: str = "Failed", *, result: Optional[str] = None) -> str:
        return await self._status(error, state="failed", steps=self.steps or None,
                                  result=result or error)
