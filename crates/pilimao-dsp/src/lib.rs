//! `pilimao-dsp`: 64-bit internal precision audio DSP library for the Pilimao player.

pub mod bitperfect;
pub mod dither;
pub mod eq;
pub mod limiter;
pub mod pipeline;
pub mod ramp;
pub mod replaygain;
pub mod resampler;
pub mod spectrum;

pub use bitperfect::{
    BitPerfectBlockReason, BitPerfectQuery, BitPerfectStatus, evaluate_bit_perfect,
};
pub use dither::{DitherDepth, TpdfDither};
pub use eq::{
    EQ_BAND_FREQUENCIES, EQ_BAND_LABELS, EQ_MAX_DB, EQ_MIN_DB, EqSettings, Equalizer10Band,
    db_to_linear, linear_to_db,
};
pub use limiter::{LimiterConfig, SoftKneeLimiter};
pub use pipeline::{DspConfig, DspPipeline};
pub use ramp::{ClicklessRamp, DEFAULT_RAMP_MS};
pub use replaygain::{ReplayGainConfig, ReplayGainInfo, ReplayGainMode};
pub use resampler::{DspError, SincResampler};
pub use spectrum::{FFT_SIZE, OSCILLOSCOPE_SAMPLES, SPECTRUM_BANDS, SpectrumAnalyzer, VisSnapshot};
