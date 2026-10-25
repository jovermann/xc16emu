// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Host audio device adapter. The emulator core never calls a platform API.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// A few PAL frames of latency keep GUI scheduling jitter out of the sound.
/// In Fast Mode the producer can outpace the device, so old samples are dropped.
const MAX_BUFFER_DIVISOR: usize = 8; // 125 ms at any output sample rate.

pub(crate) struct AudioOutput {
    _stream: Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    played_nonzero: Arc<AtomicU64>,
    sample_rate: u32,
}

impl AudioOutput {
    /// Open and start a stream on the default audio output device.
    /// Arguments: none. Returns: an active output object or an error message.
    pub(crate) fn open() -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no default audio output device")?;
        // Use the system's active output configuration. Choosing an arbitrary
        // floating-point range can fail on external devices with a different
        // active rate or integer-only output.
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config = supported.config();
        let sample_rate = config.sample_rate;
        let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let played_nonzero = Arc::new(AtomicU64::new(0));
        let stream = match format {
            SampleFormat::F32 => build_stream::<f32>(&device, config, &queue, &played_nonzero),
            SampleFormat::F64 => build_stream::<f64>(&device, config, &queue, &played_nonzero),
            SampleFormat::I16 => build_stream::<i16>(&device, config, &queue, &played_nonzero),
            SampleFormat::U16 => build_stream::<u16>(&device, config, &queue, &played_nonzero),
            SampleFormat::I32 => build_stream::<i32>(&device, config, &queue, &played_nonzero),
            SampleFormat::U32 => build_stream::<u32>(&device, config, &queue, &played_nonzero),
            other => Err(format!("unsupported audio output sample format: {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        if std::env::var_os("XC16EMU_AUDIO_DEBUG").is_some() {
            eprintln!(
                "audio output: {} at {} Hz, {} channels, {format}",
                device
                    .description()
                    .map(|description| description.name().to_owned())
                    .unwrap_or_else(|_| "unknown device".into()),
                sample_rate,
                config.channels
            );
        }
        Ok(Self {
            _stream: stream,
            queue,
            played_nonzero,
            sample_rate,
        })
    }

    /// Read the negotiated output sample rate.
    /// Arguments: self is the active audio output. Returns: samples per second.
    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Discard queued audio without closing the stream.
    /// Arguments: self is the active audio output. Returns: nothing.
    pub(crate) fn clear(&self) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Read output queue length and playback activity.
    /// Arguments: self is the active audio output. Returns: queued sample count and played nonzero count.
    pub(crate) fn debug_counts(&self) -> (usize, u64) {
        (
            self.queue.lock().unwrap_or_else(|e| e.into_inner()).len(),
            self.played_nonzero.load(Ordering::Relaxed),
        )
    }

    /// Append mono samples and bound queue latency.
    /// Arguments: samples is the new mono sample block. Returns: nothing.
    pub(crate) fn push(&self, samples: Vec<f32>) {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.extend(samples);
        let max = self.sample_rate as usize / MAX_BUFFER_DIVISOR;
        if queue.len() > max {
            let excess = queue.len() - max;
            queue.drain(..excess);
        }
    }
}

/// Build the callback stream for one device sample type.
/// Arguments: device is the target device; config sets rate and channels; queue supplies samples; played_nonzero tracks activity. Returns: the stream or an error message.
fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    queue: &Arc<Mutex<VecDeque<f32>>>,
    played_nonzero: &Arc<AtomicU64>,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let queue = Arc::clone(queue);
    let played_nonzero = Arc::clone(played_nonzero);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
                let mut nonzero = 0;
                for frame in output.chunks_mut(channels) {
                    let sample = queue.pop_front().unwrap_or(0.0);
                    if sample.abs() > 0.001 {
                        nonzero += 1;
                    }
                    frame.fill(T::from_sample(sample));
                }
                played_nonzero.fetch_add(nonzero, Ordering::Relaxed);
            },
            |error| eprintln!("audio output error: {error}"),
            None,
        )
        .map_err(|e| e.to_string())
}
