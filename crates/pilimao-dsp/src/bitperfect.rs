//! Bit-Perfect audio pass-through detection and diagnostics.

use std::fmt;

use crate::dither::DitherDepth;
use crate::eq::EqSettings;
use crate::replaygain::{ReplayGainConfig, ReplayGainInfo};

/// Specific reason why Bit-Perfect pass-through is currently bypassed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitPerfectBlockReason {
    /// Output is not running in WASAPI Exclusive mode.
    NotExclusiveMode,
    /// Track sample rate differs from output device sample rate (resampling required).
    SampleRateMismatch,
    /// Track channel count differs from output device channel count.
    ChannelCountMismatch,
    /// Software volume is not at 100% unity (`1.0`).
    VolumeNotUnity,
    /// Stereo balance is not centered (`0.0`).
    BalanceNotCentered,
    /// 10-band EQ or preamp is active with non-zero gain.
    EqActive,
    /// ReplayGain is altering gain away from 0.0 dB.
    ReplayGainActive,
    /// Clickless ramp is currently fading or muted.
    RampActive,
    /// TPDF dithering is explicitly enabled.
    DitherEnabled,
}

impl fmt::Display for BitPerfectBlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotExclusiveMode => write!(f, "output is not in WASAPI Exclusive mode"),
            Self::SampleRateMismatch => write!(f, "sample rate mismatch (resampling active)"),
            Self::ChannelCountMismatch => write!(f, "channel count mismatch"),
            Self::VolumeNotUnity => write!(f, "software volume is not 100%"),
            Self::BalanceNotCentered => write!(f, "stereo balance is not centered"),
            Self::EqActive => write!(f, "10-band EQ or preamp is active"),
            Self::ReplayGainActive => write!(f, "ReplayGain is modifying gain"),
            Self::RampActive => write!(f, "clickless fade ramp is active"),
            Self::DitherEnabled => write!(f, "TPDF dither is enabled"),
        }
    }
}

/// Result of Bit-Perfect pass-through evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitPerfectStatus {
    /// True when audio samples can pass directly from decoder to output with zero DSP alteration.
    pub is_bit_perfect: bool,
    /// Empty when `is_bit_perfect` is true; otherwise lists all blocking conditions.
    pub reasons: Vec<BitPerfectBlockReason>,
}

/// Inputs required to determine whether the current audio path is Bit-Perfect.
#[derive(Debug, Clone)]
pub struct BitPerfectQuery<'a> {
    pub exclusive_mode: bool,
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub input_channels: u16,
    pub output_channels: u16,
    pub volume: f64,
    pub balance: f64,
    pub eq: &'a EqSettings,
    pub replay_gain_config: &'a ReplayGainConfig,
    pub replay_gain_info: &'a ReplayGainInfo,
    pub ramp_is_unity: bool,
    pub dither_depth: DitherDepth,
}

/// Evaluates whether the audio chain qualifies for Bit-Perfect pass-through.
#[must_use]
pub fn evaluate_bit_perfect(query: &BitPerfectQuery<'_>) -> BitPerfectStatus {
    let mut reasons = Vec::new();

    if !query.exclusive_mode {
        reasons.push(BitPerfectBlockReason::NotExclusiveMode);
    }
    if query.input_sample_rate != query.output_sample_rate {
        reasons.push(BitPerfectBlockReason::SampleRateMismatch);
    }
    if query.input_channels != query.output_channels {
        reasons.push(BitPerfectBlockReason::ChannelCountMismatch);
    }
    if (query.volume - 1.0).abs() > 1e-6 {
        reasons.push(BitPerfectBlockReason::VolumeNotUnity);
    }
    if query.balance.abs() > 1e-6 {
        reasons.push(BitPerfectBlockReason::BalanceNotCentered);
    }
    if !query.eq.is_transparent() {
        reasons.push(BitPerfectBlockReason::EqActive);
    }
    if !query.replay_gain_config.is_unity(query.replay_gain_info) {
        reasons.push(BitPerfectBlockReason::ReplayGainActive);
    }
    if !query.ramp_is_unity {
        reasons.push(BitPerfectBlockReason::RampActive);
    }
    if query.dither_depth != DitherDepth::None {
        reasons.push(BitPerfectBlockReason::DitherEnabled);
    }

    BitPerfectStatus {
        is_bit_perfect: reasons.is_empty(),
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replaygain::ReplayGainMode;

    #[test]
    fn bit_perfect_is_active_when_all_conditions_met() {
        let eq = EqSettings::flat();
        let rg_cfg = ReplayGainConfig::default();
        let rg_info = ReplayGainInfo::default();
        let status = evaluate_bit_perfect(&BitPerfectQuery {
            exclusive_mode: true,
            input_sample_rate: 44_100,
            output_sample_rate: 44_100,
            input_channels: 2,
            output_channels: 2,
            volume: 1.0,
            balance: 0.0,
            eq: &eq,
            replay_gain_config: &rg_cfg,
            replay_gain_info: &rg_info,
            ramp_is_unity: true,
            dither_depth: DitherDepth::None,
        });
        assert!(status.is_bit_perfect);
        assert!(status.reasons.is_empty());
    }

    #[test]
    fn bit_perfect_reports_all_blocking_reasons() {
        let mut eq = EqSettings::flat();
        eq.enabled = true;
        eq.bands_db[0] = 3.0;
        let rg_cfg = ReplayGainConfig {
            mode: ReplayGainMode::Track,
            ..Default::default()
        };
        let rg_info = ReplayGainInfo {
            track_gain_db: Some(-4.5),
            ..Default::default()
        };
        let status = evaluate_bit_perfect(&BitPerfectQuery {
            exclusive_mode: false,
            input_sample_rate: 44_100,
            output_sample_rate: 48_000,
            input_channels: 2,
            output_channels: 2,
            volume: 0.8,
            balance: -0.2,
            eq: &eq,
            replay_gain_config: &rg_cfg,
            replay_gain_info: &rg_info,
            ramp_is_unity: true,
            dither_depth: DitherDepth::Bits16,
        });
        assert!(!status.is_bit_perfect);
        assert!(
            status
                .reasons
                .contains(&BitPerfectBlockReason::NotExclusiveMode)
        );
        assert!(
            status
                .reasons
                .contains(&BitPerfectBlockReason::SampleRateMismatch)
        );
        assert!(status.reasons.contains(&BitPerfectBlockReason::EqActive));
        assert!(
            status
                .reasons
                .contains(&BitPerfectBlockReason::ReplayGainActive)
        );
        assert!(
            status
                .reasons
                .contains(&BitPerfectBlockReason::DitherEnabled)
        );
    }
}
