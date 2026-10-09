# Testing without a real agent

A throwaway homeserver and a script that behaves like Hermes Agent on Matrix, to run the app end to end on an emulator. No real account or agent needed.

```bash
# 1. Synapse in Docker, on localhost:8008 (the emulator reaches it as 10.0.2.2:8008)
mkdir -p /tmp/flux-synapse
docker run --rm -v /tmp/flux-synapse:/data -e SYNAPSE_SERVER_NAME=localhost -e SYNAPSE_REPORT_STATS=no matrixdotorg/synapse:latest generate
cat >> /tmp/flux-synapse/homeserver.yaml <<'YAML'
enable_registration: true
enable_registration_without_verification: true
public_baseurl: "http://10.0.2.2:8008/"
rc_message: { per_second: 1000, burst_count: 1000 }
YAML
docker run -d --name flux-synapse -v /tmp/flux-synapse:/data -p 8008:8008 matrixdotorg/synapse:latest

# 2. Accounts (@derek / testpass-derek, @hermes), an "Ops" room, a DM, and the agent list
python3 testing/fake_hermes.py setup

# 3. The fake agent
python3 testing/fake_hermes.py run
```

In the app, sign in with homeserver `http://10.0.2.2:8008`, user `derek`, password `testpass-derek`. Then send:
- anything: 👀, a thread with step lines edited in, then the answer and ✅ (in a DM: no thread);
- something with "deploy": an approval prompt the bot reacts to with ✅ / ❌ (answer from the app or the notification);
- something with "?": a ❓ question with numbered choices;
- `!stop` (or Stop): stops the run.

Push works too: install [ntfy](https://f-droid.org/packages/io.heckel.ntfy/) on the emulator and turn push on. Synapse pushes through ntfy.sh's Matrix gateway; the pushes hold only event IDs.

Settings → Diagnostics shows what the app saw (sync, rooms, push, recent events).
