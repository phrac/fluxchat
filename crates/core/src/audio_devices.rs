//! Audio device selection and processing options shared by calls and voice
//! messages. The app sets the user's choices once (`set_audio_prefs`); the
//! capture/playback code asks for devices through `input_device` /
//! `output_device`, which fall back to the system default when the chosen
//! device isn't present.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// User audio choices.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioPrefs {
    /// Input device name; `None` = system default.
    pub input: Option<String>,
    /// Output device name; `None` = system default.
    pub output: Option<String>,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub auto_gain: bool,
}

impl Default for AudioPrefs {
    fn default() -> Self {
        Self { input: None, output: None, echo_cancellation: true, noise_suppression: true, auto_gain: true }
    }
}

static PREFS: RwLock<Option<AudioPrefs>> = RwLock::new(None);

/// Set the audio choices used by subsequent calls / recordings / playback.
pub fn set_audio_prefs(prefs: AudioPrefs) {
    *PREFS.write().unwrap_or_else(|e| e.into_inner()) = Some(prefs);
}

/// Current audio choices.
pub fn audio_prefs() -> AudioPrefs {
    PREFS.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

/// Names of the available input devices.
pub fn list_input_devices() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// Names of the available output devices.
pub fn list_output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// The chosen input device, or the default if it isn't available.
pub fn input_device() -> Option<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = audio_prefs().input {
        if let Some(d) = host.input_devices().ok()
            .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(name.as_str())))
        {
            return Some(d);
        }
        tracing::warn!("Input device {name:?} not found; using the default");
    }
    host.default_input_device()
}

/// The chosen output device, or the default if it isn't available.
pub fn output_device() -> Option<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = audio_prefs().output {
        if let Some(d) = host.output_devices().ok()
            .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(name.as_str())))
        {
            return Some(d);
        }
        tracing::warn!("Output device {name:?} not found; using the default");
    }
    host.default_output_device()
}

/// A running microphone test: captures from the chosen input device and
/// exposes the current level (0.0–1.0). Stops when dropped.
pub struct MicTest {
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for MicTest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicTest").field("level", &self.level()).finish()
    }
}

impl MicTest {
    /// Start capturing on its own thread (cpal streams aren't Send).
    pub fn start() -> Result<Self, String> {
        let level = Arc::new(AtomicU32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let (lvl, stp) = (level.clone(), stop.clone());
        std::thread::Builder::new()
            .name("mic-test".into())
            .spawn(move || {
                let run = || -> Result<cpal::Stream, String> {
                    let device = input_device().ok_or("No microphone found")?;
                    let config = device.default_input_config().map_err(|e| format!("Microphone unavailable: {e}"))?;
                    let lvl = lvl.clone();
                    let stream = device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if data.is_empty() {
                                return;
                            }
                            let rms = (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                            // Rough perceptual scaling so speech fills the meter.
                            let scaled = (rms * 4.0).min(1.0);
                            let prev = f32::from_bits(lvl.load(Ordering::Relaxed));
                            // Fast attack, slow release.
                            let next = if scaled > prev { scaled } else { prev * 0.85 + scaled * 0.15 };
                            lvl.store(next.to_bits(), Ordering::Relaxed);
                        },
                        |e| tracing::warn!("Mic test stream error: {e}"),
                        None,
                    ).map_err(|e| format!("Couldn't open microphone: {e}"))?;
                    stream.play().map_err(|e| format!("Couldn't start microphone: {e}"))?;
                    Ok(stream)
                };
                match run() {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        while !stp.load(Ordering::Relaxed) {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|e| e.to_string())??;
        Ok(Self { level, stop })
    }

    /// Current input level, 0.0–1.0.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

impl Drop for MicTest {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
