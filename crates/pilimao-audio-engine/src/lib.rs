//! `pilimao-audio-engine`: Lock-free `rtrb` ring buffer + `triple_buffer` FFT visualizer tap,
//! Windows WASAPI Shared/Exclusive (Bit-Perfect) + MMCSS `"Pro Audio"` output, and cross-platform Null Sink.

pub mod engine;
pub mod sink;
pub mod vis_tap;
pub mod wasapi;

use thiserror::Error;

pub use engine::{AudioEngine, EngineConfig, EngineSnapshot, PlaybackState, RING_BUFFER_SAMPLES};
pub use sink::{ActiveSink, AudioDeviceInfo, OutputMode, SinkCounters};
pub use vis_tap::{VisTapReader, VisTapWriter, create_vis_tap};
pub use wasapi::{enumerate_output_devices, spawn_wasapi_sink};

/// Errors returned by the audio engine or WASAPI backend.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("WASAPI is only available on Windows")]
    WasapiUnavailable,
    #[error("WASAPI error: {0}")]
    Wasapi(String),
    #[error("DSP initialization error: {0}")]
    Dsp(String),
    #[error("failed to spawn audio thread: {0}")]
    ThreadSpawn(String),
}
