/**
 * Emit the fluxchat agent protocol from any Matrix agent or harness.
 *
 * The protocol (docs/agent-protocol.md) is a few fields an agent puts in its
 * Matrix messages under `org.fluxchat.agent` so clients know what it's doing:
 * run state, current step, progress, runtime and approval choices. This
 * module builds that content; you send it with whatever Matrix library you use.
 *
 *   const run = new Run(send, "$request");
 *   await run.start("Reading the repo");
 *   await run.step("Running the tests", { progress: 0.5 });
 *   const prompt = await run.ask("Delete build/?", [["✅", "Approve"], ["❌", "Deny"]]);
 *   await run.done("All green.");
 *
 * `send(content)` sends an `m.room.message` with that content (to the room the
 * request is in) and resolves to the new event's ID.
 */

export const KEY = "org.fluxchat.agent";
export const VERSION = 1;
export const STATES = ["working", "waiting", "done", "failed"] as const;
export type State = (typeof STATES)[number];

export type Choice = [key: string, label: string] | { key: string; label: string };
export type Send = (content: Record<string, unknown>) => Promise<string>;

export interface Fields {
  run?: string;
  state?: State;
  step?: string;
  steps?: number;
  progress?: number;
  model?: string;
  context?: number;
  cwd?: string;
  cost_usd?: number;
  choices?: Choice[];
}

function unit(name: string, value: number | undefined): number | undefined {
  if (value === undefined) return undefined;
  if (!(value >= 0 && value <= 1)) throw new RangeError(`${name} must be between 0 and 1, got ${value}`);
  return value;
}

function normalizeChoices(choices: Choice[] | undefined): { key: string; label: string }[] | undefined {
  if (choices === undefined) return undefined;
  if (choices.length === 0) throw new RangeError("choices can't be empty");
  return choices.map((c) => {
    const [key, label] = Array.isArray(c) ? c : [c.key, c.label];
    if (!key || !label) throw new RangeError("a choice needs a key (the reaction to answer with) and a label");
    return { key, label };
  });
}

/** The `org.fluxchat.agent` object (only the fields given). */
export function fields(f: Fields = {}): Record<string, unknown> {
  if (f.state !== undefined && !STATES.includes(f.state)) {
    throw new RangeError(`state must be one of ${STATES.join(", ")}, got ${String(f.state)}`);
  }
  if (f.steps !== undefined && f.steps < 0) throw new RangeError("steps can't be negative");
  const out: Record<string, unknown> = { v: VERSION };
  const entries: [string, unknown][] = [
    ["run", f.run], ["state", f.state], ["step", f.step], ["steps", f.steps],
    ["progress", unit("progress", f.progress)], ["model", f.model],
    ["context", unit("context", f.context)], ["cwd", f.cwd], ["cost_usd", f.cost_usd],
    ["choices", normalizeChoices(f.choices)],
  ];
  for (const [k, v] of entries) if (v !== undefined) out[k] = v;
  return out;
}

/** Content for a new message carrying the fields (`thread`: root to post in). */
export function message(body: string, f: Fields = {}, opts: { thread?: string; notice?: boolean } = {}) {
  const content: Record<string, unknown> = {
    msgtype: opts.notice ? "m.notice" : "m.text",
    body,
    [KEY]: fields(f),
  };
  if (opts.thread) {
    content["m.relates_to"] = {
      rel_type: "m.thread", event_id: opts.thread, is_falling_back: true,
      "m.in_reply_to": { event_id: opts.thread },
    };
  }
  return content;
}

/** Content for an edit of `eventId`; the fields go in `m.new_content` (what
 * edit-aware clients read) and at the top level. */
export function edit(eventId: string, body: string, f: Fields = {}, opts: { notice?: boolean } = {}) {
  const msgtype = opts.notice ? "m.notice" : "m.text";
  const meta = fields(f);
  return {
    msgtype,
    body: `* ${body}`,
    [KEY]: meta,
    "m.new_content": { msgtype, body, [KEY]: meta },
    "m.relates_to": { rel_type: "m.replace", event_id: eventId },
  };
}

/** The choice key a reaction answers `promptEventId` with, if it's one of
 * `choices` (for your reaction handler). */
export function choiceFromReaction(content: Record<string, any>, promptEventId: string, choices: Choice[]): string | undefined {
  const rel = content["m.relates_to"] ?? {};
  if (rel.rel_type !== "m.annotation" || rel.event_id !== promptEventId) return undefined;
  const bare = (k: string) => (k ?? "").replace(/️/g, "");
  return normalizeChoices(choices)?.map((c) => c.key).find((k) => bare(k) === bare(rel.key));
}

/** One request and the agent's work on it, reported through a single status
 * message that's edited as the run moves. */
export class Run {
  readonly run: string;
  readonly thread: string | undefined;
  statusId: string | undefined;
  steps = 0;
  stepText: string | undefined;
  private runtime: Fields;
  private send: Send;
  private prompt: { question: string; choices: Choice[] } | undefined;

  constructor(send: Send, requestEventId: string, opts: { thread?: boolean | string; runtime?: Pick<Fields, "model" | "context" | "cwd" | "cost_usd"> } = {}) {
    this.send = send;
    this.run = requestEventId;
    const thread = opts.thread ?? true;
    this.thread = thread === true ? requestEventId : thread || undefined;
    this.runtime = { ...(opts.runtime ?? {}) };
  }

  private meta(f: Fields): Fields {
    return { run: this.run, ...this.runtime, ...f };
  }

  private async status(body: string, f: Fields): Promise<string> {
    if (this.statusId === undefined) {
      this.statusId = await this.send(message(body, this.meta(f), { thread: this.thread, notice: true }));
    } else {
      await this.send(edit(this.statusId, body, this.meta(f), { notice: true }));
    }
    return this.statusId;
  }

  /** Model / context / cwd / cost for the next updates. */
  updateRuntime(runtime: Pick<Fields, "model" | "context" | "cwd" | "cost_usd">): void {
    Object.assign(this.runtime, runtime);
  }

  start(step?: string): Promise<string> {
    if (step) { this.steps = 1; this.stepText = step; }
    return this.status(step ?? "Working…", { state: "working", step, steps: this.steps || undefined });
  }

  step(text: string, opts: { progress?: number } = {}): Promise<string> {
    this.steps += 1;
    this.stepText = text;
    return this.status(`${text} (${this.steps})`, { state: "working", step: text, steps: this.steps, progress: opts.progress });
  }

  /** Post a prompt; the user answers by reacting with a choice's key. Call
   * `answered` once you've acted on it. Resolves to the prompt's event ID. */
  async ask(question: string, choices: Choice[]): Promise<string> {
    this.prompt = { question, choices };
    await this.status(question, { state: "waiting", step: this.stepText, steps: this.steps || undefined });
    return this.send(message(question, this.meta({ state: "waiting", choices }), { thread: this.thread }));
  }

  /** The prompt is resolved: edit it out of "waiting", resume working. */
  async answered(promptEventId: string, note = "Answered"): Promise<void> {
    const { question, choices } = this.prompt ?? { question: "", choices: [] as Choice[] };
    await this.send(edit(promptEventId, `${question}\n\n${note}`.trim(), this.meta({ state: "working", choices: choices.length ? choices : undefined })));
    await this.status(this.stepText ?? "Working…", { state: "working", step: this.stepText, steps: this.steps || undefined });
  }

  done(summary = "Done"): Promise<string> {
    return this.status(summary, { state: "done", steps: this.steps || undefined, progress: 1 });
  }

  failed(error = "Failed"): Promise<string> {
    return this.status(error, { state: "failed", steps: this.steps || undefined });
  }
}
