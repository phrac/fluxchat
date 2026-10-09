#!/usr/bin/env python3
"""A stand-in for Hermes Agent on a throwaway homeserver, to drive the
fluxchat Android app end to end. Stdlib only.

  python3 fake_hermes.py setup   # accounts, rooms, agent list
  python3 fake_hermes.py run     # behave like Hermes

Behaviour (as Hermes's Matrix adapter does it):
- a request from you gets 👀; outside DMs the answer goes in a thread on it;
- a progress message with a line per step, edited as steps are added;
- a request mentioning "deploy" posts an approval prompt the bot reacts to
  itself with ✅ / ❌; your reaction resolves it (bot takes its reactions back);
- "?" in a request: a ❓ clarify question with numbered choices, answered by
  a reply;
- then the answer, ✅ on the request, 👀 taken back.
- "!stop": stops the current run (❌).
"""
import json
import sys
import threading
import time
import urllib.parse
import urllib.request
import uuid

HS = "http://localhost:8008"
ME, ME_PW = "derek", "testpass-derek"
BOT, BOT_PW = "hermes", "testpass-hermes"
STATE = __file__.rsplit("/", 1)[0] + "/fake_hermes_state.json"
EYES, DONE, FAIL = "\U0001F440", "✅", "❌"


def call(method, path, token=None, body=None):
    req = urllib.request.Request(HS + path, method=method, data=json.dumps(body).encode() if body is not None else None)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path}: {e.code} {e.read().decode()[:300]}")


def login(user, pw):
    try:
        call("POST", "/_matrix/client/v3/register", body={"username": user, "password": pw, "auth": {"type": "m.login.dummy"}})
    except RuntimeError as e:
        if "M_USER_IN_USE" not in str(e):
            raise
    r = call("POST", "/_matrix/client/v3/login", body={"type": "m.login.password", "identifier": {"type": "m.id.user", "user": user}, "password": pw})
    return r["access_token"], r["user_id"]


def q(s):
    return urllib.parse.quote(s, safe="")


def send(token, room, content, etype="m.room.message"):
    return call("PUT", f"/_matrix/client/v3/rooms/{q(room)}/send/{etype}/{uuid.uuid4().hex}", token, content)["event_id"]


def react(token, room, event, key):
    return send(token, room, {"m.relates_to": {"rel_type": "m.annotation", "event_id": event, "key": key}}, "m.reaction")


def redact(token, room, event):
    call("PUT", f"/_matrix/client/v3/rooms/{q(room)}/redact/{q(event)}/{uuid.uuid4().hex}", token, {})


def typing(token, room, user, on):
    call("PUT", f"/_matrix/client/v3/rooms/{q(room)}/typing/{q(user)}", token, {"typing": on, "timeout": 15000} if on else {"typing": False})


def setup():
    me_tok, me_id = login(ME, ME_PW)
    bot_tok, bot_id = login(BOT, BOT_PW)
    call("PUT", f"/_matrix/client/v3/profile/{q(bot_id)}/displayname", bot_tok, {"displayname": "Hermes"})
    call("PUT", f"/_matrix/client/v3/profile/{q(me_id)}/displayname", me_tok, {"displayname": "Derek"})
    ops = call("POST", "/_matrix/client/v3/createRoom", bot_tok, {"name": "Ops", "preset": "private_chat", "invite": [me_id]})["room_id"]
    dm = call("POST", "/_matrix/client/v3/createRoom", bot_tok, {"preset": "trusted_private_chat", "is_direct": True, "invite": [me_id]})["room_id"]
    for room in (ops, dm):
        call("POST", f"/_matrix/client/v3/join/{q(room)}", me_tok, {})
    call("PUT", f"/_matrix/client/v3/user/{q(me_id)}/account_data/m.direct", me_tok, {bot_id: [dm]})
    # The agent list desktop fluxchat would have saved.
    call("PUT", f"/_matrix/client/v3/user/{q(me_id)}/account_data/org.fluxchat.agents", me_tok,
         {"agents": [{"user_id": bot_id, "harness": "hermes"}]})
    send(bot_tok, ops, {"msgtype": "m.text", "body": "Hermes Agent ready. Ask me anything."})
    json.dump({"ops": ops, "dm": dm, "me": me_id, "bot": bot_id, "me_token": me_tok}, open(STATE, "w"))
    print(f"me {me_id} / {ME_PW}\nbot {bot_id}\nOps {ops}\nDM {dm}")


class Bot:
    def __init__(self):
        st = json.load(open(STATE))
        self.me, self.dm = st["me"], st["dm"]
        self.token, self.id = login(BOT, BOT_PW)
        self.waiting = {}  # prompt event -> threading.Event + answer
        self.questions = {}  # thread root -> (Event, answer)
        self.stop = set()

    def thread_rel(self, root):
        return {"rel_type": "m.thread", "event_id": root, "is_falling_back": True, "m.in_reply_to": {"event_id": root}}

    def post(self, room, root, body):
        c = {"msgtype": "m.text", "body": body}
        if root:
            c["m.relates_to"] = self.thread_rel(root)
        return send(self.token, room, c)

    def edit(self, room, event, body):
        send(self.token, room, {"msgtype": "m.text", "body": "* " + body, "m.new_content": {"msgtype": "m.text", "body": body},
                               "m.relates_to": {"rel_type": "m.replace", "event_id": event}})

    def handle(self, room, ev):
        body = ev["content"].get("body", "")
        req = ev["event_id"]
        rel = ev["content"].get("m.relates_to", {})
        in_thread = rel.get("rel_type") == "m.thread"
        root = rel.get("event_id") if in_thread else None
        if body.strip() == "!stop":
            self.stop.add(room)
            self.post(room, root, "⏹ Stopped.")
            return
        if in_thread and root in self.questions:
            done, box = self.questions[root]
            box.append(body)
            done.set()
            return
        threaded = room != self.dm
        thread = (root or req) if threaded else None
        self.stop.discard(room)
        eyes = react(self.token, room, req, EYES)
        typing(self.token, room, self.id, True)
        steps = ["\U0001F4D6 Reading notes.md"]
        progress = self.post(room, thread, "\n".join(steps))
        for step in ["\U0001F50D Searching the codebase for deploy", "\U0001F527 Running tests"]:
            time.sleep(3)
            if room in self.stop:
                break
            steps.append(step)
            self.edit(room, progress, "\n".join(steps))
        outcome = DONE
        if room not in self.stop and "deploy" in body.lower():
            prompt = self.post(room, thread, "⚠️ Dangerous command: `rm -rf build && ./deploy.sh`\nReact ✅ to approve once, ❌ to deny.")
            mine = [react(self.token, room, prompt, DONE), react(self.token, room, prompt, FAIL)]
            gate = threading.Event()
            answer = []
            self.waiting[prompt] = (gate, answer)
            typing(self.token, room, self.id, False)
            gate.wait(600)
            for r in mine:
                redact(self.token, room, r)
            if answer and answer[0] == FAIL:
                outcome = FAIL
                self.post(room, thread, "Denied, so I didn't run it.")
            else:
                steps.append("\U0001F4BB Running rm -rf build && ./deploy.sh")
                self.edit(room, progress, "\n".join(steps))
                typing(self.token, room, self.id, True)
                time.sleep(3)
        if outcome == DONE and room not in self.stop and "?" in body:
            qid = self.post(room, thread, "❓ Which environment?\n  1. staging\n  2. production")
            gate = threading.Event()
            box = []
            self.questions[thread or qid] = (gate, box)
            typing(self.token, room, self.id, False)
            gate.wait(600)
            self.post(room, thread, f"OK, going with: {box[0] if box else 'staging'}")
        if room in self.stop:
            outcome = FAIL
        elif outcome == DONE:
            self.post(room, thread, "All done. Tests pass (42 passed), and the build is deployed.\n\n```\n$ ./deploy.sh\ndeployed v1.4.2 in 12s\n```")
        typing(self.token, room, self.id, False)
        react(self.token, room, req, outcome)
        time.sleep(2)
        redact(self.token, room, eyes)

    def run(self):
        since = call("GET", "/_matrix/client/v3/sync?timeout=0", self.token)["next_batch"]
        print("fake Hermes running as", self.id, flush=True)
        while True:
            r = call("GET", f"/_matrix/client/v3/sync?timeout=30000&since={q(since)}", self.token)
            since = r["next_batch"]
            for room, data in r.get("rooms", {}).get("join", {}).items():
                for ev in data.get("timeline", {}).get("events", []):
                    if ev.get("sender") != self.me:
                        continue
                    if ev["type"] == "m.reaction":
                        rel = ev["content"].get("m.relates_to", {})
                        w = self.waiting.pop(rel.get("event_id"), None)
                        if w:
                            w[1].append(rel.get("key"))
                            w[0].set()
                        continue
                    if ev["type"] != "m.room.message" or ev["content"].get("m.relates_to", {}).get("rel_type") == "m.replace":
                        continue
                    print("request:", ev["content"].get("body"), flush=True)
                    threading.Thread(target=self.handle, args=(room, ev), daemon=True).start()


if __name__ == "__main__":
    setup() if sys.argv[1:] == ["setup"] else Bot().run()
