//! LiveKit connection + audio capture/playback.
//! Linux: PulseAudio Simple API (works with PipeWire via pipewire-pulse).
//! macOS/Windows: cpal.

use crate::rtc::{CallJoinInfo, CallParticipant};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Events emitted by the call session to the UI layer.
#[derive(Debug, Clone)]
pub enum CallEvent {
    Connected,
    Disconnected(String),
    ParticipantJoined(CallParticipant),
    ParticipantLeft(String),
    ActiveSpeakersChanged(Vec<String>),
    Failed(String),
}

/// Spawn a task that reads audio frames from a remote track and plays them.
fn spawn_audio_playback(audio_track: livekit::prelude::RemoteAudioTrack) {
    use livekit::webrtc::audio_stream::native::NativeAudioStream;
    use futures_util::StreamExt;

    let rtc_track = audio_track.rtc_track();

    #[cfg(target_os = "linux")]
    {
        use libpulse_binding::sample::{Format, Spec};
        use libpulse_binding::stream::Direction;
        use libpulse_simple_binding::Simple;

        let spec = Spec {
            format: Format::S16le,
            channels: 2,
            rate: 48000,
        };

        let pulse = match Simple::new(
            None,
            "FluxChat",
            Direction::Playback,
            None,
            "Call Audio",
            &spec,
            None,
            None,
        ) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                tracing::warn!("Failed to open PulseAudio playback: {e}");
                return;
            }
        };

        tracing::info!("Audio output: PulseAudio, S16LE 48000Hz 2ch");

        tokio::spawn(async move {
            let pulse = pulse;
            let mut native_stream = NativeAudioStream::new(rtc_track, 48000, 2);
            while let Some(frame) = native_stream.next().await {
                // Convert i16 samples to byte slice for PulseAudio
                let bytes: &[u8] = unsafe {
                    std::slice::from_raw_parts(
                        frame.data.as_ptr() as *const u8,
                        frame.data.len() * 2,
                    )
                };
                if let Err(e) = pulse.write(bytes) {
                    tracing::warn!("PulseAudio write error: {e}");
                    break;
                }
            }
            tracing::info!("Remote audio stream ended");
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use std::sync::Mutex;

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                tracing::warn!("No audio output device found");
                return;
            }
        };

        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("No output config: {e}");
                return;
            }
        };

        let sample_rate = config.sample_rate().0 as i32;
        let channels = config.channels() as i32;
        let stream_config: cpal::StreamConfig = config.into();

        // Ring buffer shared between cpal callback and LiveKit producer
        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let buffer_writer = buffer.clone();

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = buffer.lock().unwrap();
                let available = buf.len().min(data.len());
                if available > 0 {
                    data[..available].copy_from_slice(&buf[..available]);
                    buf.drain(..available);
                }
                for sample in &mut data[available..] {
                    *sample = 0.0;
                }
            },
            |err| tracing::warn!("Audio output error: {err}"),
            None,
        );

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to build output stream: {e}");
                return;
            }
        };

        if let Err(e) = stream.play() {
            tracing::warn!("Failed to start output stream: {e}");
            return;
        }

        tracing::info!("Audio output: cpal F32, {sample_rate}Hz, {channels}ch");

        #[allow(dead_code)]
        struct SendStream(*mut cpal::Stream);
        unsafe impl Send for SendStream {}
        let stream_holder = SendStream(Box::into_raw(Box::new(stream)));

        tokio::spawn(async move {
            let _stream_guard = stream_holder;
            let mut native_stream = NativeAudioStream::new(rtc_track, sample_rate, channels);
            while let Some(frame) = native_stream.next().await {
                let samples: Vec<f32> = frame.data.iter()
                    .map(|&s| s as f32 / i16::MAX as f32)
                    .collect();
                let mut buf = buffer_writer.lock().unwrap();
                let max_samples = (sample_rate as usize * channels as usize) / 2;
                if buf.len() < max_samples {
                    buf.extend_from_slice(&samples);
                }
            }
            tracing::info!("Remote audio stream ended");
        });
    }
}

/// Manages an active LiveKit call session.
pub struct CallSession {
    room: Arc<livekit::Room>,
    audio_track: Option<livekit::prelude::LocalAudioTrack>,
    #[cfg(not(target_os = "linux"))]
    _audio_stream: Option<cpal::Stream>,
    #[cfg(target_os = "linux")]
    _pulse_capture: Option<Arc<std::sync::atomic::AtomicBool>>,
}

// cpal::Stream is not Send, but we only access it from the thread that created it
unsafe impl Send for CallSession {}
unsafe impl Sync for CallSession {}

impl CallSession {
    /// Connect to LiveKit and start the event loop.
    pub async fn connect(
        info: &CallJoinInfo,
        event_tx: mpsc::UnboundedSender<CallEvent>,
    ) -> Result<Self, String> {
        let (room, mut rx) = livekit::Room::connect(
            &info.livekit_url,
            &info.jwt,
            livekit::RoomOptions::default(),
        )
        .await
        .map_err(|e| format!("LiveKit connect failed: {e}"))?;

        let room = Arc::new(room);

        // Spawn event drain loop
        let tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                use livekit::RoomEvent;
                match event {
                    RoomEvent::ParticipantConnected(participant) => {
                        let _ = tx.send(CallEvent::ParticipantJoined(CallParticipant {
                            user_id: participant.identity().to_string(),
                            device_id: String::new(),
                            display_name: participant.name().to_string(),
                            avatar_url: None,
                            is_local: false,
                        }));
                    }
                    RoomEvent::ParticipantDisconnected(participant) => {
                        let _ = tx.send(CallEvent::ParticipantLeft(
                            participant.identity().to_string(),
                        ));
                    }
                    RoomEvent::ActiveSpeakersChanged { speakers } => {
                        let ids: Vec<String> = speakers
                            .iter()
                            .map(|p| p.identity().to_string())
                            .collect();
                        let _ = tx.send(CallEvent::ActiveSpeakersChanged(ids));
                    }
                    RoomEvent::Disconnected { reason } => {
                        let _ = tx.send(CallEvent::Disconnected(format!("{reason:?}")));
                        break;
                    }
                    RoomEvent::Connected { participants_with_tracks } => {
                        for (_participant, tracks) in participants_with_tracks {
                            for publication in tracks {
                                if let Some(livekit::prelude::RemoteTrack::Audio(audio_track)) = publication.track() {
                                    tracing::info!("Starting playback for existing remote audio track");
                                    spawn_audio_playback(audio_track);
                                }
                            }
                        }
                    }
                    RoomEvent::TrackSubscribed { track, .. } => {
                        match track {
                            livekit::prelude::RemoteTrack::Audio(audio_track) => {
                                tracing::info!("Subscribed to remote audio track — starting playback");
                                spawn_audio_playback(audio_track);
                            }
                            livekit::prelude::RemoteTrack::Video(_) => {
                                tracing::info!("Subscribed to remote video track");
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        let _ = event_tx.send(CallEvent::Connected);

        let session = CallSession {
            room: room.clone(),
            audio_track: None,
            #[cfg(not(target_os = "linux"))]
            _audio_stream: None,
            #[cfg(target_os = "linux")]
            _pulse_capture: None,
        };

        // Report existing participants
        for (_, participant) in room.remote_participants() {
            let _ = event_tx.send(CallEvent::ParticipantJoined(CallParticipant {
                user_id: participant.identity().to_string(),
                device_id: String::new(),
                display_name: participant.name().to_string(),
                avatar_url: None,
                is_local: false,
            }));
        }

        Ok(session)
    }

    /// Publish microphone audio to the call.
    pub async fn publish_microphone(&mut self) -> Result<(), String> {
        use livekit::webrtc::audio_source::native::NativeAudioSource;
        use livekit::webrtc::prelude::{AudioSourceOptions, RtcAudioSource};
        use livekit::webrtc::audio_frame::AudioFrame;

        let sample_rate: u32;
        let channels: u32;

        #[cfg(target_os = "linux")]
        {
            sample_rate = 48000;
            channels = 2; // PulseAudio default spec matches pactl info
        }

        #[cfg(not(target_os = "linux"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait};
            let host = cpal::default_host();
            let device = host
                .default_input_device()
                .ok_or("No audio input device found")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("No input config: {e}"))?;
            sample_rate = config.sample_rate().0;
            channels = config.channels() as u32;
        }

        let audio_source = NativeAudioSource::new(
            AudioSourceOptions::default(),
            sample_rate,
            channels,
            1000,
        );

        let track = livekit::prelude::LocalAudioTrack::create_audio_track(
            "microphone",
            RtcAudioSource::Native(audio_source.clone()),
        );

        self.room
            .local_participant()
            .publish_track(
                livekit::prelude::LocalTrack::Audio(track.clone()),
                livekit::options::TrackPublishOptions {
                    source: livekit::prelude::TrackSource::Microphone,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Failed to publish audio track: {e}"))?;

        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<i16>>();
        let sr = sample_rate;
        let ch = channels;
        let source = audio_source;

        // Tokio task drains audio samples and feeds LiveKit
        tokio::spawn(async move {
            while let Some(samples) = audio_rx.recv().await {
                let mut frame = AudioFrame {
                    data: samples.into(),
                    sample_rate: sr,
                    num_channels: ch,
                    samples_per_channel: 0,
                };
                frame.samples_per_channel = frame.data.len() as u32 / ch;
                let _ = source.capture_frame(&frame).await;
            }
        });

        #[cfg(target_os = "linux")]
        {
            use libpulse_binding::sample::{Format, Spec};
            use libpulse_binding::stream::Direction;
            use libpulse_simple_binding::Simple;
            use std::sync::atomic::{AtomicBool, Ordering};

            let spec = Spec {
                format: Format::S16le,
                channels: channels as u8,
                rate: sample_rate,
            };

            let pulse = Simple::new(
                None,
                "FluxChat",
                Direction::Record,
                None,
                "Microphone",
                &spec,
                None,
                None,
            )
            .map_err(|e| format!("Failed to open PulseAudio capture: {e}"))?;

            tracing::info!("Audio input: PulseAudio, S16LE {sample_rate}Hz {channels}ch");

            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();

            std::thread::spawn(move || {
                // PulseAudio Simple read is blocking, so use a dedicated thread
                let mut buf = vec![0i16; (sample_rate as usize * channels as usize) / 50]; // 20ms
                while running_clone.load(Ordering::Relaxed) {
                    let bytes: &mut [u8] = unsafe {
                        std::slice::from_raw_parts_mut(
                            buf.as_mut_ptr() as *mut u8,
                            buf.len() * 2,
                        )
                    };
                    match pulse.read(bytes) {
                        Ok(()) => {
                            let _ = audio_tx.send(buf.clone());
                        }
                        Err(e) => {
                            tracing::warn!("PulseAudio read error: {e}");
                            break;
                        }
                    }
                }
            });

            self.audio_track = Some(track);
            self._pulse_capture = Some(running);
        }

        #[cfg(not(target_os = "linux"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

            let host = cpal::default_host();
            let device = host
                .default_input_device()
                .ok_or("No audio input device found")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("No input config: {e}"))?;

            let stream = device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let samples: Vec<i16> = data
                            .iter()
                            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                            .collect();
                        let _ = audio_tx.send(samples);
                    },
                    |err| {
                        tracing::warn!("Audio input error: {err}");
                    },
                    None,
                )
                .map_err(|e| format!("Failed to build audio stream: {e}"))?;

            stream
                .play()
                .map_err(|e| format!("Failed to start audio stream: {e}"))?;

            self.audio_track = Some(track);
            self._audio_stream = Some(stream);
        }

        Ok(())
    }

    /// Toggle microphone mute state.
    pub fn set_mic_muted(&self, muted: bool) {
        if let Some(ref track) = self.audio_track {
            if muted {
                track.mute();
            } else {
                track.unmute();
            }
        }
    }

    /// Disconnect from the call.
    pub async fn disconnect(self) {
        #[cfg(target_os = "linux")]
        if let Some(running) = &self._pulse_capture {
            running.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        self.room.close().await.ok();
    }
}
