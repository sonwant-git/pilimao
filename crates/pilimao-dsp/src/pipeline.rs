//! Full 64-bit audio DSP pipeline integrating EQ, ReplayGain, Limiter, Ramp, Resampler, Dither,
//! 19-band FFT Spectrum, and Bit-Perfect pass-through short-circuiting.

use crate::bitperfect::{BitPerfectQuery, BitPerfectStatus, evaluate_bit_perfect};
use crate::dither::{DitherDepth, TpdfDither};
use crate::eq::{EqSettings, Equalizer10Band};
use crate::limiter::{LimiterConfig, SoftKneeLimiter};
use crate::ramp::{ClicklessRamp, DEFAULT_RAMP_MS};
use crate::replaygain::{ReplayGainConfig, ReplayGainInfo};
use crate::resampler::{DspError, SincResampler};
use crate::spectrum::{SpectrumAnalyzer, VisSnapshot};

/// Configuration parameters for [`DspPipeline`].
#[derive(Debug, Clone)]
pub struct DspConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub exclusive_mode: bool,
    /// Master volume in `0.0..=1.0`.
    pub volume: f64,
    /// Stereo balance in `-1.0` (full left) .. `0.0` (center) .. `+1.0` (full right).
    pub balance: f64,
    pub eq: EqSettings,
    pub limiter: LimiterConfig,
    pub replay_gain: ReplayGainConfig,
    pub dither_depth: DitherDepth,
}

impl Default for DspConfig {
    fn default() -> Self {
        Self {
            input_sample_rate: 44_100,
            output_sample_rate: 48_000,
            exclusive_mode: false,
            volume: 1.0,
            balance: 0.0,
            eq: EqSettings::flat(),
            limiter: LimiterConfig::default(),
            replay_gain: ReplayGainConfig::default(),
            dither_depth: DitherDepth::None,
        }
    }
}

/// End-to-end stereo 64-bit DSP pipeline with Bit-Perfect bypass.
pub struct DspPipeline {
    config: DspConfig,
    track_rg: ReplayGainInfo,
    eq: Equalizer10Band,
    limiter: SoftKneeLimiter,
    ramp: ClicklessRamp,
    dither: TpdfDither,
    resampler: Option<SincResampler>,
    spectrum: SpectrumAnalyzer,
}

impl DspPipeline {
    /// Creates a new DSP pipeline from [`DspConfig`].
    pub fn new(config: DspConfig) -> Result<Self, DspError> {
        let mut eq = Equalizer10Band::new(config.input_sample_rate);
        eq.set_settings(config.eq.clone());
        eq.snap_to_target();

        let limiter = SoftKneeLimiter::new(config.input_sample_rate, config.limiter);
        let ramp = ClicklessRamp::new(config.input_sample_rate, DEFAULT_RAMP_MS);
        let dither = TpdfDither::new(config.dither_depth);
        let resampler = if config.input_sample_rate != config.output_sample_rate {
            Some(SincResampler::new(
                config.input_sample_rate,
                config.output_sample_rate,
            )?)
        } else {
            None
        };
        let spectrum = SpectrumAnalyzer::new(config.output_sample_rate);

        Ok(Self {
            config,
            track_rg: ReplayGainInfo::default(),
            eq,
            limiter,
            ramp,
            dither,
            resampler,
            spectrum,
        })
    }

    /// Updates sample rates, recreating the Sinc resampler only when needed.
    pub fn reconfigure_rates(
        &mut self,
        input_sample_rate: u32,
        output_sample_rate: u32,
    ) -> Result<(), DspError> {
        if self.config.input_sample_rate == input_sample_rate
            && self.config.output_sample_rate == output_sample_rate
        {
            return Ok(());
        }
        self.config.input_sample_rate = input_sample_rate;
        self.config.output_sample_rate = output_sample_rate;
        self.eq.set_sample_rate(input_sample_rate);
        self.limiter.set_sample_rate(input_sample_rate);
        self.ramp.set_sample_rate(input_sample_rate);
        self.spectrum.set_sample_rate(output_sample_rate);
        self.resampler = if input_sample_rate != output_sample_rate {
            Some(SincResampler::new(input_sample_rate, output_sample_rate)?)
        } else {
            None
        };
        Ok(())
    }

    /// Sets master software volume (`0.0..=1.0`).
    pub fn set_volume(&mut self, volume: f64) {
        self.config.volume = volume.clamp(0.0, 1.0);
    }

    /// Sets stereo balance (`-1.0..=1.0`).
    pub fn set_balance(&mut self, balance: f64) {
        self.config.balance = balance.clamp(-1.0, 1.0);
    }

    /// Updates 10-band EQ settings with zipper-free smoothing.
    pub fn set_eq(&mut self, settings: EqSettings) {
        self.config.eq = settings.clone();
        self.eq.set_settings(settings);
    }

    /// Updates ReplayGain configuration.
    pub fn set_replay_gain_config(&mut self, cfg: ReplayGainConfig) {
        self.config.replay_gain = cfg;
    }

    /// Updates active track's ReplayGain tags.
    pub fn set_track_replay_gain(&mut self, info: ReplayGainInfo) {
        self.track_rg = info;
    }

    /// Sets exclusive output mode flag used for Bit-Perfect pass-through evaluation.
    pub fn set_exclusive_mode(&mut self, exclusive: bool) {
        self.config.exclusive_mode = exclusive;
    }

    /// Sets TPDF dither bit depth.
    pub fn set_dither_depth(&mut self, depth: DitherDepth) {
        self.config.dither_depth = depth;
        self.dither.set_depth(depth);
    }

    /// Triggers a smooth clickless fade-in.
    pub fn fade_in(&mut self) {
        self.ramp.fade_in();
    }

    /// Triggers a smooth clickless fade-out.
    pub fn fade_out(&mut self) {
        self.ramp.fade_out();
    }

    /// Accesses the internal 10-band EQ (e.g., to query frequency response spline).
    #[must_use]
    pub fn equalizer(&self) -> &Equalizer10Band {
        &self.eq
    }

    /// Accesses the clickless ramp.
    #[must_use]
    pub fn ramp(&self) -> &ClicklessRamp {
        &self.ramp
    }

    /// Resets stateful filter/resampler/spectrum buffers (on seek or track stop).
    pub fn reset_state(&mut self) {
        self.eq.reset_state();
        self.limiter.reset();
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
        }
        self.spectrum.reset();
    }

    /// Evaluates current Bit-Perfect status.
    #[must_use]
    pub fn bit_perfect_status(&self) -> BitPerfectStatus {
        evaluate_bit_perfect(&BitPerfectQuery {
            exclusive_mode: self.config.exclusive_mode,
            input_sample_rate: self.config.input_sample_rate,
            output_sample_rate: self.config.output_sample_rate,
            input_channels: 2,
            output_channels: 2,
            volume: self.config.volume,
            balance: self.config.balance,
            eq: &self.config.eq,
            replay_gain_config: &self.config.replay_gain,
            replay_gain_info: &self.track_rg,
            ramp_is_unity: self.ramp.is_unity(),
            dither_depth: self.config.dither_depth,
        })
    }

    /// Processes interleaved stereo `f32` samples through the 64-bit DSP chain and outputs `f32`.
    /// When Bit-Perfect pass-through is active, samples bypass all math untouched.
    pub fn process_f32(&mut self, input: &[f32]) -> Result<Vec<f32>, DspError> {
        let bp = self.bit_perfect_status();
        if bp.is_bit_perfect && !self.eq.is_smoothing() {
            // Bit-Perfect short-circuit: zero sample alteration, tap spectrum read-only.
            self.spectrum.push_interleaved_f32(input);
            return Ok(input.to_vec());
        }

        let rg_gain = self
            .config
            .replay_gain
            .effective_gain_linear(&self.track_rg);
        // Classic Winamp linear pan balance: left attenuated when balance > 0, right when < 0
        let left_gain = self.config.volume * (1.0 - self.config.balance.max(0.0)) * rg_gain;
        let right_gain = self.config.volume * (1.0 + self.config.balance.min(0.0)) * rg_gain;

        let mut work = Vec::with_capacity(input.len());
        for frame in input.as_chunks::<2>().0 {
            let l = f64::from(frame[0]) * left_gain;
            let r = f64::from(frame[1]) * right_gain;
            let (l, r) = self.eq.process_frame(l, r);
            let (l, r) = self.limiter.process_frame(l, r);
            let (l, r) = self.ramp.process_frame(l, r);
            work.push(l);
            work.push(r);
        }

        let mut resampled = if let Some(resampler) = &mut self.resampler {
            resampler.process_interleaved(&work)?
        } else {
            work
        };

        self.dither.process_float_in_place(&mut resampled);
        self.spectrum.push_interleaved_f64(&resampled);

        Ok(resampled.into_iter().map(|s| s as f32).collect())
    }

    /// Computes the latest 19-band FFT spectrum & oscilloscope visualization snapshot.
    pub fn analyze_vis(&mut self) -> VisSnapshot {
        self.spectrum.analyze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_perfect_bypass_preserves_exact_f32_bits() {
        let cfg = DspConfig {
            input_sample_rate: 44_100,
            output_sample_rate: 44_100,
            exclusive_mode: true,
            volume: 1.0,
            balance: 0.0,
            eq: EqSettings::flat(),
            limiter: LimiterConfig::default(),
            replay_gain: ReplayGainConfig::default(),
            dither_depth: DitherDepth::None,
        };
        let mut pipe = DspPipeline::new(cfg).expect("pipeline");
        assert!(pipe.bit_perfect_status().is_bit_perfect);

        let input: Vec<f32> = vec![0.123_456_7, -0.987_654_3, 0.0, 0.5, -0.25, 0.75];
        let output = pipe.process_f32(&input).expect("process");
        for (a, b) in input.iter().zip(output.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "bit-perfect output must be exact");
        }
    }
}
