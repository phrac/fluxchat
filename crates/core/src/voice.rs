//! Voice message recording: mic capture via cpal → Opus encoding → OGG container.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Record audio from the default input device.
/// Returns a handle that can be stopped to get the encoded OGG Opus bytes.
pub fn start_recording() -> Result<RecordingHandle, String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No audio input device")?;
    let config = device.default_input_config().map_err(|e| format!("No input config: {e}"))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut buf = samples_clone.lock().unwrap();
            // Convert f32 to i16 and downmix to mono if needed
            if channels == 1 {
                buf.extend(data.iter().map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16));
            } else {
                // Downmix to mono by averaging channels
                for chunk in data.chunks(channels) {
                    let avg = chunk.iter().sum::<f32>() / channels as f32;
                    buf.push((avg.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                }
            }
        },
        |err| tracing::warn!("Audio input error: {err}"),
        None,
    ).map_err(|e| format!("Failed to build input stream: {e}"))?;

    stream.play().map_err(|e| format!("Failed to start recording: {e}"))?;

    Ok(RecordingHandle {
        _stream: stream,
        samples,
        sample_rate,
        start_time: std::time::Instant::now(),
    })
}

pub struct RecordingHandle {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    sample_rate: u32,
    start_time: std::time::Instant,
}

impl RecordingHandle {
    /// Get the recording duration so far.
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Stop recording and encode to OGG Opus bytes.
    /// Returns (ogg_bytes, duration_ms).
    pub fn stop_and_encode(self) -> Result<(Vec<u8>, u64), String> {
        // Stream is dropped here, stopping capture
        drop(self._stream);

        let samples = self.samples.lock().unwrap().clone();
        let duration_ms = self.start_time.elapsed().as_millis() as u64;

        if samples.is_empty() {
            return Err("No audio recorded".into());
        }

        encode_opus_ogg(&samples, self.sample_rate, duration_ms)
    }
}

/// Encode i16 mono PCM samples to OGG Opus.
fn encode_opus_ogg(samples: &[i16], sample_rate: u32, duration_ms: u64) -> Result<(Vec<u8>, u64), String> {
    use opus::Encoder;
    use ogg::writing::PacketWriter;

    // Opus requires 48kHz — resample if needed
    let opus_rate = 48000u32;
    let resampled = if sample_rate != opus_rate {
        resample(samples, sample_rate, opus_rate)
    } else {
        samples.to_vec()
    };

    // Normalize audio levels — boost if too quiet
    let max_sample = resampled.iter().map(|s| s.unsigned_abs()).max().unwrap_or(1) as f32;
    let gain = if max_sample < 8000.0 { 32767.0 / max_sample.max(1.0) } else { 1.0 };
    let resampled: Vec<i16> = if gain > 1.0 {
        resampled.iter().map(|&s| (s as f32 * gain).clamp(-32767.0, 32767.0) as i16).collect()
    } else {
        resampled
    };

    let mut encoder = Encoder::new(opus_rate, opus::Channels::Mono, opus::Application::Voip)
        .map_err(|e| format!("Opus encoder init failed: {e}"))?;
    let _ = encoder.set_bitrate(opus::Bitrate::Bits(64000));

    // Opus frame size: 20ms at 48kHz = 960 samples
    let frame_size = 960;
    let mut ogg_data = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut ogg_data);

    {
        let mut writer = PacketWriter::new(&mut cursor);
        let serial = 1u32;

        // Write Opus header (OpusHead)
        let mut head = Vec::new();
        head.extend_from_slice(b"OpusHead");
        head.push(1); // version
        head.push(1); // channels (mono)
        head.extend_from_slice(&0u16.to_le_bytes()); // pre-skip
        head.extend_from_slice(&opus_rate.to_le_bytes()); // input sample rate
        head.extend_from_slice(&0i16.to_le_bytes()); // output gain
        head.push(0); // channel mapping family
        writer.write_packet(head, serial, ogg::writing::PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| format!("OGG write error: {e}"))?;

        // Write Opus tags (OpusTags)
        let mut tags = Vec::new();
        tags.extend_from_slice(b"OpusTags");
        let vendor = b"fluxchat";
        tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        tags.extend_from_slice(vendor);
        tags.extend_from_slice(&0u32.to_le_bytes()); // no user comments
        writer.write_packet(tags, serial, ogg::writing::PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| format!("OGG write error: {e}"))?;

        // Encode and write audio frames
        let mut granule_pos = 0u64;
        let mut offset = 0;
        let mut output = vec![0u8; 4000]; // max Opus packet size

        while offset < resampled.len() {
            let end = (offset + frame_size).min(resampled.len());
            let mut frame = resampled[offset..end].to_vec();
            // Pad last frame with silence if needed
            frame.resize(frame_size, 0);

            let encoded_len = encoder.encode(&frame, &mut output)
                .map_err(|e| format!("Opus encode error: {e}"))?;

            granule_pos += frame_size as u64;
            let is_last = offset + frame_size >= resampled.len();
            let end_info = if is_last {
                ogg::writing::PacketWriteEndInfo::EndStream
            } else {
                ogg::writing::PacketWriteEndInfo::NormalPacket
            };

            writer.write_packet(output[..encoded_len].to_vec(), serial, end_info, granule_pos)
                .map_err(|e| format!("OGG write error: {e}"))?;

            offset += frame_size;
        }
    }

    Ok((ogg_data, duration_ms))
}

/// Simple linear resampling from one sample rate to another.
fn resample(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut result = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;
        let s0 = samples[idx.min(samples.len() - 1)] as f64;
        let s1 = samples[(idx + 1).min(samples.len() - 1)] as f64;
        result.push((s0 + frac * (s1 - s0)) as i16);
    }
    result
}
