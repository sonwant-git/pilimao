//! `pilimao-metadata`: Audio tag & ReplayGain 2.0 parsing via `lofty`, plus legacy
//! Big5/GBK/Shift-JIS mojibake restoration.

pub mod encoding;
pub mod replaygain;
pub mod tags;

pub use encoding::{
    DetectedEncoding, LegacyEncodingHint, RestoredText, restore_mojibake, restore_raw_bytes,
};
pub use replaygain::{
    ReplayGainTags, extract_replay_gain, parse_gain_db, parse_peak, parse_r128_gain_db,
};
pub use tags::{
    MetadataError, RestorationRecord, TrackMetadata, read_track_metadata, read_track_metadata_bytes,
};
