import asyncio
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import fluxchat_agent as fa  # noqa: E402


class Recorder:
    def __init__(self):
        self.sent = []

    async def __call__(self, content):
        self.sent.append(content)
        return f"$e{len(self.sent)}"


class FieldsTest(unittest.TestCase):
    def test_only_given_fields_and_validation(self):
        self.assertEqual(fa.fields(state="working", step="x"), {"v": 1, "state": "working", "step": "x"})
        with self.assertRaises(ValueError):
            fa.fields(state="busy")
        with self.assertRaises(ValueError):
            fa.fields(progress=1.5)
        with self.assertRaises(ValueError):
            fa.fields(choices=[])

    def test_edits_carry_fields_in_new_content(self):
        c = fa.edit("$s", "Testing", state="working", progress=0.5)
        self.assertEqual(c["m.relates_to"], {"rel_type": "m.replace", "event_id": "$s"})
        self.assertEqual(c["m.new_content"][fa.KEY]["progress"], 0.5)
        self.assertEqual(c["m.new_content"]["body"], "Testing")

    def test_reaction_answers(self):
        r = {"m.relates_to": {"rel_type": "m.annotation", "event_id": "$p", "key": "✅"}}
        self.assertEqual(fa.choice_from_reaction(r, "$p", [("✅️", "Yes"), ("❌", "No")]), "✅️")
        self.assertIsNone(fa.choice_from_reaction(r, "$other", [("✅", "Yes")]))


class RunTest(unittest.TestCase):
    def test_a_run_edits_one_status_message(self):
        send = Recorder()

        async def go():
            run = fa.Run(send, "$req", model="opus-5")
            await run.start("Reading")
            await run.step("Testing", progress=0.5)
            prompt = await run.ask("Ship it?", [("✅", "Yes"), ("❌", "No")])
            await run.answered(prompt, "Approved")
            await run.done("Shipped")
            return prompt

        prompt = asyncio.run(go())
        first = send.sent[0]
        self.assertEqual(first["m.relates_to"]["rel_type"], "m.thread")
        self.assertEqual(first[fa.KEY]["state"], "working")
        self.assertEqual(first[fa.KEY]["model"], "opus-5")
        # Later status updates are edits of the first message.
        edits = [c for c in send.sent if c.get("m.relates_to", {}).get("rel_type") == "m.replace"]
        self.assertTrue(all(c["m.relates_to"]["event_id"] in ("$e1", prompt) for c in edits))
        ask = next(c for c in send.sent if c[fa.KEY].get("choices") and c[fa.KEY]["state"] == "waiting" and "m.new_content" not in c)
        self.assertEqual(ask[fa.KEY]["choices"], [{"key": "✅", "label": "Yes"}, {"key": "❌", "label": "No"}])
        last = send.sent[-1]["m.new_content"][fa.KEY]
        self.assertEqual((last["state"], last["progress"], last["steps"]), ("done", 1.0, 2))


if __name__ == "__main__":
    unittest.main()
