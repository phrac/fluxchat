# fluxchat for Android (agent companion)

A phone app for your AI agents on Matrix. It doesn't try to be a full client. It shows:
- the rooms your agents are in, and what each run is doing;
- the run's state, step and spend, with Stop / Steer / Queue;
- approvals and questions as buttons.

It reads agents the same way desktop fluxchat does:
- the same harness profiles (`../assets/agents/*.toml`, copied in at build time);
- the same agent protocol;
- the same agent list, which both apps share through `org.fluxchat.agents` account data.

Built on Element's prebuilt Kotlin bindings for matrix-rust-sdk (`org.matrix.rustcomponents:sdk-android`), so there's no NDK and no Rust build. Kotlin + Jetpack Compose.

## Build

You need Android Studio (or a JDK 21 and the Android SDK). AGP installs the compile SDK platform if it's missing.

```bash
cd android
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"   # macOS + Android Studio
./gradlew :app:assembleRelease      # R8-optimized; signed with the debug key unless KEYSTORE_PATH etc. are set
adb install -r app/build/outputs/apk/release/app-arm64-v8a-release.apk
```

Debug builds (`assembleDebug`, installed as `org.fluxchat.companion.debug` next to the release) start slowly; use release for real use.

The build makes one APK per ABI: arm64 for phones, x86_64 for Intel emulators. The 32-bit SDK libraries are left out.

Tests (the run inference, ported from `crates/app/src/agents.rs`; they read the desktop's `hermes.toml`):

```bash
./gradlew :app:testDebugUnitTest
```

Screenshots of the main screens with sample data, light and dark (Roborazzi on Robolectric, no device), land in `app/build/screenshots`:

```bash
./gradlew :app:testDebugUnitTest -Droborazzi.test.record=true
```

## Testing end to end

[testing/](testing/README.md) runs a throwaway Synapse in Docker and a fake Hermes (`fake_hermes.py`) so the app can be driven on an emulator without a real account or agent.

## Layout

| Path | What |
|---|---|
| `agents/` | `Runs` (request state, prompts, questions, step logs, protocol runs: agents.rs), `Protocol`, `Harness` |
| `matrix/` | `Matrix` (client, sync, agent list, agent rooms' live timelines), `TimelineModel` (SDK diffs → messages), `Vault` (session + store key under an Android Keystore key) |
| `ui/` | Compose screens (Material 3, dynamic colour, light/dark): rooms, chat (room or thread), agents, settings |
| `push/` | UnifiedPush (ntfy): registration and pusher, push handling, notifications with answer buttons, actions |
