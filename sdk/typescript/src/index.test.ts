import { test } from "node:test";
import assert from "node:assert/strict";
import { KEY, Run, choiceFromReaction, edit, fields } from "./index.ts";

test("only given fields, validated", () => {
  assert.deepEqual(fields({ state: "working", step: "x" }), { v: 1, state: "working", step: "x" });
  assert.throws(() => fields({ progress: 1.5 }));
  assert.throws(() => fields({ choices: [] }));
  assert.throws(() => fields({ state: "busy" as any }));
});

test("edits carry the fields in m.new_content", () => {
  const c = edit("$s", "Testing", { state: "working", progress: 0.5 });
  assert.deepEqual(c["m.relates_to"], { rel_type: "m.replace", event_id: "$s" });
  assert.equal((c["m.new_content"][KEY] as any).progress, 0.5);
});

test("reactions answer prompts", () => {
  const r = { "m.relates_to": { rel_type: "m.annotation", event_id: "$p", key: "✅" } };
  assert.equal(choiceFromReaction(r, "$p", [["✅️", "Yes"], ["❌", "No"]]), "✅️");
  assert.equal(choiceFromReaction(r, "$other", [["✅", "Yes"]]), undefined);
});

test("a run edits one status message", async () => {
  const sent: any[] = [];
  const send = async (c: any) => { sent.push(c); return `$e${sent.length}`; };
  const run = new Run(send, "$req", { runtime: { model: "opus-5" } });
  await run.start("Reading");
  await run.step("Testing", { progress: 0.5 });
  const prompt = await run.ask("Ship it?", [["✅", "Yes"], ["❌", "No"]]);
  await run.answered(prompt, "Approved");
  await run.done("Shipped", "Deployed v1.0");
  assert.equal(sent[0]["m.relates_to"].rel_type, "m.thread");
  assert.equal(sent[0][KEY].model, "opus-5");
  const last = sent.at(-1)["m.new_content"][KEY];
  assert.deepEqual([last.state, last.progress, last.steps], ["done", 1, 2]);
  assert.equal(last.result, "Deployed v1.0");
  const ask = sent.find((c) => c[KEY].state === "waiting" && c[KEY].choices && !c["m.new_content"]);
  assert.deepEqual(ask[KEY].choices, [{ key: "✅", label: "Yes" }, { key: "❌", label: "No" }]);
});
