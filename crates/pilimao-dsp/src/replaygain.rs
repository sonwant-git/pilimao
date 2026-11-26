//! ReplayGain 2.0 gain calculation with optional peak clipping prevention.

use crate::eq::{db_to_linear, linear_to_db};

/// ReplayGain playback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayGainMode {
    /// Disable ReplayGain.
    #[default]
    Off,
    /// Prefer Track gain (falls back to Album gain if Track gain is absent).
    Track,
    /// Prefer Album gain (falls back to Track gain if Album gain is absent).
    Album,
}

/// ReplayGain 2.0 tag values extracted from track metadata.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReplayGainInfo {
    pub track_gain_db: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain_db: Option<f64>,
    pub album_peak: Option<f64>,
}

impl ReplayGainInfo {
    /// Returns true if any ReplayGain tag is present.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.track_gain_db.is_some()
            || self.album_gain_db.is_some()
            || self.track_peak.is_some()
            || self.album_peak.is_some()
    }
}

/// ReplayGain 2.0 user configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayGainConfig {
    pub mode: ReplayGainMode,
    /// Preamp adjustment applied when ReplayGain tags are present (default: 0.0 dB).
    pub preamp_with_rg_db: f64,
    /// Preamp adjustment applied when ReplayGain tags are missing (default: 0.0 dB).
    pub preamp_without_rg_db: f64,
    /// When enabled, lowers gain if `gain * peak > 1.0` to prevent digital clipping.
    pub prevent_clipping: bool,
}

impl Default for ReplayGainConfig {
    fn default() -> Self {
        Self {
            mode: ReplayGainMode::Off,
            preamp_with_rg_db: 0.0,
            preamp_without_rg_db: 0.0,
            prevent_clipping: true,
        }
    }
}

impl ReplayGainConfig {
    /// Computes the effective linear gain multiplier for a track's [`ReplayGainInfo`].
    #[must_use]
    pub fn effective_gain_linear(&self, info: &ReplayGainInfo) -> f64 {
        let (gain_db, peak) = match self.mode {
            ReplayGainMode::Off => return 1.0,
            ReplayGainMode::Track => (
                info.track_gain_db.or(info.album_gain_db),
                info.track_peak.or(info.album_peak),
            ),
            ReplayGainMode::Album => (
                info.album_gain_db.or(info.track_gain_db),
                info.album_peak.or(info.track_peak),
            ),
        };

        let target_db = match gain_db {
            Some(g) => g + self.preamp_with_rg_db,
            None => self.preamp_without_rg_db,
        };

        let mut linear = db_to_linear(target_db);
        if self.prevent_clipping
            && let Some(p) = peak
            && p > 0.0
            && linear * p > 1.0
        {
            linear = 1.0 / p;
        }
        linear
    }

    /// Computes the effective gain in dB.
    #[must_use]
    pub fn effective_gain_db(&self, info: &ReplayGainInfo) -> f64 {
        linear_to_db(self.effective_gain_linear(info))
    }

    /// Returns true if ReplayGain leaves the signal at unity gain (`0.0 dB`).
    #[must_use]
    pub fn is_unity(&self, info: &ReplayGainInfo) -> bool {
        (self.effective_gain_linear(info) - 1.0).abs() < 1e-6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_mode_applies_gain_and_prevents_clipping() {
        let info = ReplayGainInfo {
            track_gain_db: Some(6.0),
            track_peak: Some(0.8),
            album_gain_db: Some(-3.0),
            album_peak: Some(0.95),
        };
        let cfg = ReplayGainConfig {
            mode: ReplayGainMode::Track,
            preamp_with_rg_db: 0.0,
            preamp_without_rg_db: 0.0,
            prevent_clipping: true,
        };
        // +6 dB (~2.0x) * 0.8 peak = 1.6 > 1.0 -> capped at 1/0.8 = 1.25
        let gain = cfg.effective_gain_linear(&info);
        assert!((gain - 1.25).abs() < 1e-6);
    }

    #[test]
    fn album_mode_uses_album_gain() {
        let info = ReplayGainInfo {
            track_gain_db: Some(-1.0),
            track_peak: Some(0.5),
            album_gain_db: Some(-6.0),
            album_peak: Some(0.5),
        };
        let cfg = ReplayGainConfig {
            mode: ReplayGainMode::Album,
            ..Default::default()
        };
        let db = cfg.effective_gain_db(&info);
        assert!((db - (-6.0)).abs() < 1e-5);
    }
}
