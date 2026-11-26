//! 64-bit Soft-Knee Stereo Limiter to prevent digital clipping on EQ/Preamp boosts.

use crate::eq::{db_to_linear, linear_to_db};

/// Configuration for [`SoftKneeLimiter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimiterConfig {
    /// Whether the limiter is active.
    pub enabled: bool,
    /// Threshold in dBFS where soft-knee gain reduction begins (default: -1.0 dBFS).
    pub threshold_db: f64,
    /// Soft knee width in dB (default: 3.0 dB).
    pub knee_db: f64,
    /// Hard safety ceiling in dBFS (default: -0.1 dBFS).
    pub ceiling_db: f64,
    /// Attack time in milliseconds (default: 0.5 ms).
    pub attack_ms: f64,
    /// Release time in milliseconds (default: 60.0 ms).
    pub release_ms: f64,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: -1.0,
            knee_db: 3.0,
            ceiling_db: -0.1,
            attack_ms: 0.5,
            release_ms: 60.0,
        }
    }
}

/// Stereo-linked 64-bit soft-knee limiter.
#[derive(Debug, Clone)]
pub struct SoftKneeLimiter {
    config: LimiterConfig,
    sample_rate: f64,
    attack_coeff: f64,
    release_coeff: f64,
    ceiling_linear: f64,
    envelope_db: f64,
}

impl SoftKneeLimiter {
    /// Creates a new soft-knee limiter for `sample_rate`.
    #[must_use]
    pub fn new(sample_rate: u32, config: LimiterConfig) -> Self {
        let mut limiter = Self {
            config,
            sample_rate: f64::from(sample_rate.max(8_000)),
            attack_coeff: 0.0,
            release_coeff: 0.0,
            ceiling_linear: 1.0,
            envelope_db: -120.0,
        };
        limiter.recompute_coeffs();
        limiter
    }

    /// Updates sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = f64::from(sample_rate.max(8_000));
        self.recompute_coeffs();
    }

    /// Updates limiter configuration.
    pub fn set_config(&mut self, config: LimiterConfig) {
        self.config = config;
        self.recompute_coeffs();
    }

    /// Returns current configuration.
    #[must_use]
    pub fn config(&self) -> LimiterConfig {
        self.config
    }

    /// Resets envelope state.
    pub fn reset(&mut self) {
        self.envelope_db = -120.0;
    }

    fn recompute_coeffs(&mut self) {
        let attack_sec = (self.config.attack_ms / 1000.0).max(1e-5);
        let release_sec = (self.config.release_ms / 1000.0).max(1e-4);
        self.attack_coeff = (-1.0 / (attack_sec * self.sample_rate)).exp();
        self.release_coeff = (-1.0 / (release_sec * self.sample_rate)).exp();
        self.ceiling_linear = db_to_linear(self.config.ceiling_db);
    }

    /// Computes soft-knee target gain reduction in dB (<= 0.0) for an input level in dBFS.
    #[inline]
    fn compute_gain_reduction_db(&self, level_db: f64) -> f64 {
        let thresh = self.config.threshold_db;
        let knee = self.config.knee_db.max(0.0);
        let half_knee = knee * 0.5;
        let overshoot = level_db - thresh;

        if knee > 1e-6 && overshoot > -half_knee && overshoot < half_knee {
            // Smooth quadratic soft knee (infinity:1 ratio above knee)
            let x = overshoot + half_knee;
            -(x * x) / (2.0 * knee)
        } else if overshoot >= half_knee {
            -overshoot
        } else {
            0.0
        }
    }

    /// Processes a stereo sample pair in `f64`.
    #[inline]
    pub fn process_frame(&mut self, left: f64, right: f64) -> (f64, f64) {
        if !self.config.enabled {
            return (left, right);
        }

        let peak = left.abs().max(right.abs());
        let peak_db = linear_to_db(peak);

        let coeff = if peak_db > self.envelope_db {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_db = coeff * self.envelope_db + (1.0 - coeff) * peak_db;

        // Use instant peak or smoothed envelope (whichever demands more reduction) for safe limiting
        let control_db = self.envelope_db.max(peak_db);
        let gr_db = self.compute_gain_reduction_db(control_db);
        let gain = db_to_linear(gr_db);

        let out_l = (left * gain).clamp(-self.ceiling_linear, self.ceiling_linear);
        let out_r = (right * gain).clamp(-self.ceiling_linear, self.ceiling_linear);
        (out_l, out_r)
    }

    /// Processes an interleaved stereo `f64` buffer in place.
    pub fn process_interleaved(&mut self, samples: &mut [f64]) {
        if !self.config.enabled {
            return;
        }
        for frame in samples.as_chunks_mut::<2>().0 {
            let (l, r) = self.process_frame(frame[0], frame[1]);
            frame[0] = l;
            frame[1] = r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_signal_is_clamped_below_ceiling() {
        let cfg = LimiterConfig::default();
        let ceiling = db_to_linear(cfg.ceiling_db);
        let mut limiter = SoftKneeLimiter::new(48_000, cfg);

        for i in 0..1024 {
            let x = 2.5 * (i as f64 * 0.1).sin();
            let (l, r) = limiter.process_frame(x, -x);
            assert!(l.abs() <= ceiling + 1e-12);
            assert!(r.abs() <= ceiling + 1e-12);
        }
    }

    #[test]
    fn quiet_signal_passes_untouched() {
        let mut limiter = SoftKneeLimiter::new(48_000, LimiterConfig::default());
        let (l, r) = limiter.process_frame(0.25, -0.25);
        assert!((l - 0.25).abs() < 1e-12);
        assert!((r - (-0.25)).abs() < 1e-12);
    }
}
