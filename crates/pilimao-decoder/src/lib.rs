//! `pilimao-decoder`: Multi-format audio decoding and gapless playback via `symphonia`.

pub mod decoder;
pub mod gapless;

pub use decoder::{DecoderError, SymphoniaDecoder, TrackFormat, encode_pcm16_wav};
pub use gapless::{GaplessChunk, GaplessDecoder, GaplessTransition};
