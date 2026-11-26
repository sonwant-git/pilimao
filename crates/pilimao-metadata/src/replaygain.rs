//! ReplayGain 2.0 and Opus R128 tag value parsing.

/// Parsed ReplayGain 2.0 metadata values from audio tags.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReplayGainTags {
    pub track_gain_db: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain_db: Option<f64>,
    pub album_peak: Option<f64>,
}

impl ReplayGainTags {
    /// Returns true if any ReplayGain tag field was found.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.track_gain_db.is_some()
            || self.track_peak.is_some()
            || self.album_gain_db.is_some()
            || self.album_peak.is_some()
    }
}

/// Parses a ReplayGain gain string such as `"-6.42 dB"`, `"+1.20dB"`, or `"-3.5"`.
#[must_use]
pub fn parse_gain_db(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let num_part = lower.strip_suffix("db").unwrap_or(&lower).trim();
    let val: f64 = num_part.parse().ok()?;
    if val.is_finite() && (-60.0..=60.0).contains(&val) {
        Some(val)
    } else {
        None
    }
}

/// Parses a ReplayGain linear peak string such as `"0.988525"`.
#[must_use]
pub fn parse_peak(raw: &str) -> Option<f64> {
    let val: f64 = raw.trim().parse().ok()?;
    if val.is_finite() && val > 0.0 && val <= 16.0 {
        Some(val)
    } else {
        None
    }
}

/// Parses an Opus R128 Q7.8 fixed-point gain tag (`R128_TRACK_GAIN` / `R128_ALBUM_GAIN`)
/// and converts it from -23 LUFS reference to ReplayGain 2.0 (-18 LUFS) by adding `+5.0 dB`.
#[must_use]
pub fn parse_r128_gain_db(raw: &str) -> Option<f64> {
    let q78: i32 = raw.trim().parse().ok()?;
    Some(f64::from(q78) / 256.0 + 5.0)
}

/// Extracts ReplayGain 2.0 (and R128 fallback) values from arbitrary `(key, value)` tag pairs.
pub fn extract_replay_gain<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> ReplayGainTags {
    let mut rg = ReplayGainTags::default();
    let mut r128_track: Option<f64> = None;
    let mut r128_album: Option<f64> = None;

    for (key, value) in pairs {
        let normalized = key.trim().to_ascii_uppercase();
        // Handle MP4 freeform `----:com.apple.iTunes:REPLAYGAIN_TRACK_GAIN` style keys too
        let suffix = normalized.rsplit(':').next().unwrap_or(&normalized).trim();

        match suffix {
            "REPLAYGAIN_TRACK_GAIN" => {
                rg.track_gain_db = rg.track_gain_db.or_else(|| parse_gain_db(value));
            }
            "REPLAYGAIN_TRACK_PEAK" => {
                rg.track_peak = rg.track_peak.or_else(|| parse_peak(value));
            }
            "REPLAYGAIN_ALBUM_GAIN" => {
                rg.album_gain_db = rg.album_gain_db.or_else(|| parse_gain_db(value));
            }
            "REPLAYGAIN_ALBUM_PEAK" => {
                rg.album_peak = rg.album_peak.or_else(|| parse_peak(value));
            }
            "R128_TRACK_GAIN" => {
                r128_track = r128_track.or_else(|| parse_r128_gain_db(value));
            }
            "R128_ALBUM_GAIN" => {
                r128_album = r128_album.or_else(|| parse_r128_gain_db(value));
            }
            _ => {}
        }
    }

    if rg.track_gain_db.is_none() {
        rg.track_gain_db = r128_track;
    }
    if rg.album_gain_db.is_none() {
        rg.album_gain_db = r128_album;
    }

    rg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_and_r128_replay_gain() {
        let rg = extract_replay_gain([
            ("REPLAYGAIN_TRACK_GAIN", "-6.42 dB"),
            ("REPLAYGAIN_TRACK_PEAK", "0.988525"),
            ("----:com.apple.iTunes:replaygain_album_gain", "+1.25dB"),
            ("replaygain_album_peak", "0.991000"),
        ]);
        assert!((rg.track_gain_db.unwrap() - (-6.42)).abs() < 1e-9);
        assert!((rg.track_peak.unwrap() - 0.988525).abs() < 1e-9);
        assert!((rg.album_gain_db.unwrap() - 1.25).abs() < 1e-9);
        assert!((rg.album_peak.unwrap() - 0.991).abs() < 1e-9);

        let opus = extract_replay_gain([("R128_TRACK_GAIN", "-1280")]); // -5.0 + 5.0 = 0.0 dB
        assert!((opus.track_gain_db.unwrap() - 0.0).abs() < 1e-9);
    }
}
