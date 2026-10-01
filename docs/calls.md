# Voice and video

## Voice calls

fluxchat supports voice calls using MatrixRTC with a LiveKit server (your homeserver must advertise one; the call button is hidden otherwise).

- **Start or join**: the phone button in the room header. Rooms with an active call show a join banner and a phone icon in the room list.
- **In a call**, a panel above the messages shows the participants. Your microphone starts **muted**; use the mic button to unmute.
- **Leaving**: hang up with the red button. Quitting FluxChat (closing the window, or Cmd+Q on macOS) also leaves the call.
- **Other rooms**: if you switch rooms during a call, a small picture-in-picture panel stays in the member list.

## Devices and audio processing

Settings → **Voice & Video**:

- **Microphone** and **Speaker**: choose a device, or *System default*. If a chosen device is unplugged, the default is used.
- **Test microphone**: shows a live input level meter.
- **Echo cancellation**, **Noise suppression**, **Automatic gain**.

Changes apply to the next call and voice message.

If the test meter doesn't move, FluxChat shows a warning when the microphone delivers only silence. On macOS that almost always means FluxChat isn't allowed to use it: turn it on in System Settings → Privacy & Security → Microphone, then restart FluxChat. Older builds of the app never asked for access; rebuild it with `./build-macos.sh`. On Linux, call audio uses the PulseAudio default devices; the choices apply to voice messages.

## Voice messages

Click the microphone in the message box to record, click again to stop. Preview, then send. They're encoded as Opus and play inline.

## Video

Videos in messages play inline (GStreamer is required). Video calls aren't supported yet.
