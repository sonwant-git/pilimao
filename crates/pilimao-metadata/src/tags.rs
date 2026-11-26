//! High-level audio tag and technical property extraction powered by `lofty`.

use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag};
use thiserror::Error;

use crate::encoding::{DetectedEncoding, LegacyEncodingHint, restore_mojibake};
use crate::replaygain::{ReplayGainTags, extract_replay_gain};

/// Errors produced when inspecting track metadata.
#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("lofty metadata parse error: {0}")]
    Lofty(#[from] lofty::error::LoftyError),
}

/// Record of a tag field whose legacy encoding mojibake was automatically restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorationRecord {
    pub field: String,
    pub original: String,
    pub restored: String,
    pub encoding: DetectedEncoding,
}

/// Extracted track metadata, technical properties, ReplayGain tags, and mojibake diagnostics.
#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    pub replay_gain: ReplayGainTags,
    pub restorations: Vec<RestorationRecord>,
}

impl TrackMetadata {
    /// Formats a classic Winamp display title (`"Artist - Title"` or fallback filename stem).
    #[must_use]
    pub fn display_title(&self, fallback_name: &str) -> String {
        match (&self.artist, &self.title) {
            (Some(a), Some(t)) if !a.trim().is_empty() && !t.trim().is_empty() => {
                format!("{} - {}", a.trim(), t.trim())
            }
            (_, Some(t)) if !t.trim().is_empty() => t.trim().to_string(),
            _ => fallback_name.to_string(),
        }
    }
}

/// Reads metadata, ReplayGain 2.0 tags, and audio properties from a file on disk.
pub fn read_track_metadata(
    path: impl AsRef<Path>,
    hint: LegacyEncodingHint,
) -> Result<TrackMetadata, MetadataError> {
    let tagged_file = Probe::open(path.as_ref())?.guess_file_type()?.read()?;
    Ok(extract_from_tagged_file(&tagged_file, hint))
}

/// Reads metadata from in-memory audio bytes (useful for tests & streaming).
pub fn read_track_metadata_bytes(
    bytes: &[u8],
    hint: LegacyEncodingHint,
) -> Result<TrackMetadata, MetadataError> {
    let tagged_file = Probe::new(Cursor::new(bytes)).guess_file_type()?.read()?;
    Ok(extract_from_tagged_file(&tagged_file, hint))
}

fn extract_from_tagged_file(
    tagged_file: &lofty::file::TaggedFile,
    hint: LegacyEncodingHint,
) -> TrackMetadata {
    let props = tagged_file.properties();
    let duration = props.duration();

    let mut meta = TrackMetadata {
        duration: if duration.is_zero() {
            None
        } else {
            Some(duration)
        },
        sample_rate: props.sample_rate(),
        bitrate_kbps: props.audio_bitrate(),
        channels: props.channels(),
        bit_depth: props.bit_depth(),
        ..Default::default()
    };

    if let Some(tag) = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
    {
        populate_text_fields(&mut meta, tag, hint);
    }

    // Collect ReplayGain tags across all tag containers in the file (ID3v2, Vorbis, APE, MP4).
    let mut rg_pairs: Vec<(String, String)> = Vec::new();
    for tag in tagged_file.tags() {
        collect_replay_gain_items(tag, &mut rg_pairs);
    }
    let borrowed: Vec<(&str, &str)> = rg_pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    meta.replay_gain = extract_replay_gain(borrowed);

    meta
}

fn populate_text_fields(meta: &mut TrackMetadata, tag: &Tag, hint: LegacyEncodingHint) {
    let mut restore_field =
        |field: &str, value: Option<std::borrow::Cow<'_, str>>| -> Option<String> {
            let raw = value?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            let restored = restore_mojibake(trimmed, hint);
            if let Some(enc) = restored.restored_from {
                meta.restorations.push(RestorationRecord {
                    field: field.to_string(),
                    original: trimmed.to_string(),
                    restored: restored.text.clone(),
                    encoding: enc,
                });
            }
            Some(restored.text)
        };

    meta.title = restore_field("title", tag.title());
    meta.artist = restore_field("artist", tag.artist());
    meta.album = restore_field("album", tag.album());
    meta.album_artist = restore_field(
        "album_artist",
        tag.get_string(&ItemKey::AlbumArtist).map(Into::into),
    );
    meta.genre = restore_field("genre", tag.genre());
    meta.comment = restore_field("comment", tag.comment());
    meta.year = tag.year();
    meta.track_number = tag.track();
    meta.disc_number = tag.disk();
}

fn collect_replay_gain_items(tag: &Tag, out: &mut Vec<(String, String)>) {
    for item in tag.items() {
        let val = match item.value() {
            ItemValue::Text(s) | ItemValue::Locator(s) => s.as_str(),
            ItemValue::Binary(_) => continue,
        };
        match item.key() {
            ItemKey::ReplayGainTrackGain => {
                out.push(("REPLAYGAIN_TRACK_GAIN".to_string(), val.to_string()));
            }
            ItemKey::ReplayGainTrackPeak => {
                out.push(("REPLAYGAIN_TRACK_PEAK".to_string(), val.to_string()));
            }
            ItemKey::ReplayGainAlbumGain => {
                out.push(("REPLAYGAIN_ALBUM_GAIN".to_string(), val.to_string()));
            }
            ItemKey::ReplayGainAlbumPeak => {
                out.push(("REPLAYGAIN_ALBUM_PEAK".to_string(), val.to_string()));
            }
            ItemKey::Unknown(k) => {
                out.push((k.clone(), val.to_string()));
            }
            _ => {}
        }
    }
}
