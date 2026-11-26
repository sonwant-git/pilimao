//! 64-bit precision 10-band classic Winamp Equalizer with zipper-noise-free smoothing.

use std::f64::consts::{PI, SQRT_2};

/// Classic Winamp 2.x 10-band center frequencies (Hz).
pub const EQ_BAND_FREQUENCIES: [f64; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

/// Display labels for the 10 classic EQ bands.
pub const EQ_BAND_LABELS: [&str; 10] = [
    "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K",
];

/// Minimum EQ slider gain in dB.
pub const EQ_MIN_DB: f64 = -12.0;

/// Maximum EQ slider gain in dB.
pub const EQ_MAX_DB: f64 = 12.0;

/// Default Q factor for classic 10-band graphic EQ peaking filters.
const DEFAULT_BAND_Q: f64 = SQRT_2;

/// Smoothing time constant in seconds (~8 ms) to prevent zipper noise.
const SMOOTH_TAU_SECONDS: f64 = 0.008;

/// Equalizer user settings (Preamp + 10 bands, ±12 dB).
#[derive(Debug, Clone, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f64,
    pub bands_db: [f64; 10],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self::flat()
    }
}

impl EqSettings {
    /// Returns a flat (0 dB) EQ configuration with EQ disabled.
    #[must_use]
    pub fn flat() -> Self {
        Self {
            enabled: false,
            preamp_db: 0.0,
            bands_db: [0.0; 10],
        }
    }

    /// Returns true if EQ is effectively transparent (disabled, or preamp and all bands are 0 dB).
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        if !self.enabled {
            return true;
        }
        self.preamp_db.abs() < 1e-6 && self.bands_db.iter().all(|g| g.abs() < 1e-6)
    }

    /// Clamps preamp and all 10 bands into `[EQ_MIN_DB, EQ_MAX_DB]`.
    pub fn clamp(&mut self) {
        self.preamp_db = self.preamp_db.clamp(EQ_MIN_DB, EQ_MAX_DB);
        for g in &mut self.bands_db {
            *g = g.clamp(EQ_MIN_DB, EQ_MAX_DB);
        }
    }
}

/// 64-bit Transposed Direct Form II biquad filter (normalized with a0 = 1).
#[derive(Debug, Clone, Copy)]
struct Biquad64 {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1_l: f64,
    z2_l: f64,
    z1_r: f64,
    z2_r: f64,
}

impl Biquad64 {
    fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1_l: 0.0,
            z2_l: 0.0,
            z1_r: 0.0,
            z2_r: 0.0,
        }
    }

    /// Computes RBJ Audio EQ Cookbook peaking EQ coefficients in f64.
    fn update_peaking(&mut self, sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) {
        // Safely bypass bands too close to or above Nyquist, or near-zero gain.
        if freq_hz >= sample_rate * 0.49 || gain_db.abs() < 1e-6 {
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
            return;
        }

        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    #[inline]
    fn process_stereo(&mut self, left: f64, right: f64) -> (f64, f64) {
        let out_l = self.b0 * left + self.z1_l;
        self.z1_l = self.b1 * left - self.a1 * out_l + self.z2_l;
        self.z2_l = self.b2 * left - self.a2 * out_l;

        let out_r = self.b0 * right + self.z1_r;
        self.z1_r = self.b1 * right - self.a1 * out_r + self.z2_r;
        self.z2_r = self.b2 * right - self.a2 * out_r;

        (out_l, out_r)
    }

    fn reset_state(&mut self) {
        self.z1_l = 0.0;
        self.z2_l = 0.0;
        self.z1_r = 0.0;
        self.z2_r = 0.0;
    }

    /// Evaluates magnitude response |H(e^{j*w})| at `freq_hz`.
    fn magnitude_at(&self, sample_rate: f64, freq_hz: f64) -> f64 {
        let w = 2.0 * PI * freq_hz / sample_rate;
        let cos1 = w.cos();
        let sin1 = w.sin();
        let cos2 = (2.0 * w).cos();
        let sin2 = (2.0 * w).sin();

        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -(self.b1 * sin1 + self.b2 * sin2);
        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -(self.a1 * sin1 + self.a2 * sin2);

        let num_mag_sq = num_re * num_re + num_im * num_im;
        let den_mag_sq = (den_re * den_re + den_im * den_im).max(1e-30);
        (num_mag_sq / den_mag_sq).sqrt()
    }
}

/// 64-bit 10-band stereo classic Winamp Equalizer with smooth zipper-free transitions.
#[derive(Debug, Clone)]
pub struct Equalizer10Band {
    sample_rate: f64,
    smooth_alpha: f64,
    settings: EqSettings,
    current_preamp_db: f64,
    current_bands_db: [f64; 10],
    filters: [Biquad64; 10],
}

impl Equalizer10Band {
    /// Creates a new 10-band EQ configured for `sample_rate`.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate.max(8_000));
        Self {
            sample_rate: sr,
            smooth_alpha: compute_smooth_alpha(sr),
            settings: EqSettings::flat(),
            current_preamp_db: 0.0,
            current_bands_db: [0.0; 10],
            filters: [Biquad64::identity(); 10],
        }
    }

    /// Updates sample rate and recomputes filter coefficients while preserving settings.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        let sr = f64::from(sample_rate.max(8_000));
        if (self.sample_rate - sr).abs() < 1e-6 {
            return;
        }
        self.sample_rate = sr;
        self.smooth_alpha = compute_smooth_alpha(sr);
        for (i, filter) in self.filters.iter_mut().enumerate() {
            filter.reset_state();
            filter.update_peaking(
                self.sample_rate,
                EQ_BAND_FREQUENCIES[i],
                DEFAULT_BAND_Q,
                self.current_bands_db[i],
            );
        }
    }

    /// Applies new target settings; internal gains smoothly converge without zipper noise.
    pub fn set_settings(&mut self, mut settings: EqSettings) {
        settings.clamp();
        self.settings = settings;
    }

    /// Immediately snaps internal smoothed gains to the target settings (useful on track load/reset).
    pub fn snap_to_target(&mut self) {
        let target_preamp = if self.settings.enabled {
            self.settings.preamp_db
        } else {
            0.0
        };
        self.current_preamp_db = target_preamp;
        for i in 0..10 {
            let target = if self.settings.enabled {
                self.settings.bands_db[i]
            } else {
                0.0
            };
            self.current_bands_db[i] = target;
            self.filters[i].update_peaking(
                self.sample_rate,
                EQ_BAND_FREQUENCIES[i],
                DEFAULT_BAND_Q,
                target,
            );
        }
    }

    /// Returns current target EQ settings.
    #[must_use]
    pub fn settings(&self) -> &EqSettings {
        &self.settings
    }

    /// Resets filter delay lines (e.g., on seek or track switch).
    pub fn reset_state(&mut self) {
        for filter in &mut self.filters {
            filter.reset_state();
        }
    }

    /// Returns true if any internal smoothed parameter is still converging toward target.
    #[must_use]
    pub fn is_smoothing(&self) -> bool {
        let target_preamp = if self.settings.enabled {
            self.settings.preamp_db
        } else {
            0.0
        };
        if (self.current_preamp_db - target_preamp).abs() > 1e-5 {
            return true;
        }
        for i in 0..10 {
            let target = if self.settings.enabled {
                self.settings.bands_db[i]
            } else {
                0.0
            };
            if (self.current_bands_db[i] - target).abs() > 1e-5 {
                return true;
            }
        }
        false
    }

    /// Processes a single stereo sample pair in 64-bit precision.
    #[inline]
    pub fn process_frame(&mut self, mut left: f64, mut right: f64) -> (f64, f64) {
        let target_preamp = if self.settings.enabled {
            self.settings.preamp_db
        } else {
            0.0
        };

        // Smooth preamp dB
        let preamp_diff = target_preamp - self.current_preamp_db;
        if preamp_diff.abs() > 1e-5 {
            self.current_preamp_db += preamp_diff * self.smooth_alpha;
        } else {
            self.current_preamp_db = target_preamp;
        }

        if self.current_preamp_db.abs() > 1e-6 {
            let preamp_lin = db_to_linear(self.current_preamp_db);
            left *= preamp_lin;
            right *= preamp_lin;
        }

        // Smooth each band and filter
        for i in 0..10 {
            let target = if self.settings.enabled {
                self.settings.bands_db[i]
            } else {
                0.0
            };
            let diff = target - self.current_bands_db[i];
            if diff.abs() > 1e-5 {
                self.current_bands_db[i] += diff * self.smooth_alpha;
                self.filters[i].update_peaking(
                    self.sample_rate,
                    EQ_BAND_FREQUENCIES[i],
                    DEFAULT_BAND_Q,
                    self.current_bands_db[i],
                );
            } else if (self.current_bands_db[i] - target).abs() > 0.0 {
                self.current_bands_db[i] = target;
                self.filters[i].update_peaking(
                    self.sample_rate,
                    EQ_BAND_FREQUENCIES[i],
                    DEFAULT_BAND_Q,
                    target,
                );
            }

            if self.current_bands_db[i].abs() > 1e-6 {
                (left, right) = self.filters[i].process_stereo(left, right);
            }
        }

        (left, right)
    }

    /// Processes an interleaved stereo `f64` buffer in place.
    pub fn process_interleaved(&mut self, samples: &mut [f64]) {
        for frame in samples.as_chunks_mut::<2>().0 {
            let (l, r) = self.process_frame(frame[0], frame[1]);
            frame[0] = l;
            frame[1] = r;
        }
    }

    /// Evaluates the combined EQ magnitude response in dB at `freq_hz` (for UI spline display).
    #[must_use]
    pub fn response_db_at(&self, freq_hz: f64) -> f64 {
        if !self.settings.enabled {
            return 0.0;
        }
        let mut mag = db_to_linear(self.settings.preamp_db);
        for (i, &center) in EQ_BAND_FREQUENCIES.iter().enumerate() {
            let mut bq = Biquad64::identity();
            bq.update_peaking(
                self.sample_rate,
                center,
                DEFAULT_BAND_Q,
                self.settings.bands_db[i],
            );
            mag *= bq.magnitude_at(self.sample_rate, freq_hz);
        }
        20.0 * mag.max(1e-12).log10()
    }
}

#[inline]
fn compute_smooth_alpha(sample_rate: f64) -> f64 {
    1.0 - (-1.0 / (SMOOTH_TAU_SECONDS * sample_rate)).exp()
}

/// Converts decibels to linear amplitude multiplier.
#[inline]
#[must_use]
pub fn db_to_linear(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

/// Converts linear amplitude multiplier to decibels.
#[inline]
#[must_use]
pub fn linear_to_db(linear: f64) -> f64 {
    20.0 * linear.abs().max(1e-12).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_is_transparent() {
        let mut eq = Equalizer10Band::new(48_000);
        let (l, r) = eq.process_frame(0.42, -0.73);
        assert!((l - 0.42).abs() < 1e-12);
        assert!((r - (-0.73)).abs() < 1e-12);
    }

    #[test]
    fn boost_at_1khz_increases_1khz_sine_energy() {
        let sr = 48_000;
        let mut eq = Equalizer10Band::new(sr);
        let mut settings = EqSettings::flat();
        settings.enabled = true;
        settings.bands_db[4] = 12.0; // 1 kHz band
        eq.set_settings(settings);
        eq.snap_to_target();

        let mut rms_in = 0.0;
        let mut rms_out = 0.0;
        for n in 0..4800 {
            let x = 0.2 * (2.0 * PI * 1000.0 * n as f64 / sr as f64).sin();
            let (y, _) = eq.process_frame(x, x);
            if n >= 1000 {
                rms_in += x * x;
                rms_out += y * y;
            }
        }
        let gain_db = 10.0 * (rms_out / rms_in).log10();
        assert!(
            (gain_db - 12.0).abs() < 0.5,
            "expected ~+12 dB at 1 kHz, got {gain_db:.2} dB"
        );
    }

    #[test]
    fn smoothing_prevents_zipper_step_discontinuity() {
        let sr = 48_000;
        let mut eq = Equalizer10Band::new(sr);
        let mut settings = EqSettings::flat();
        settings.enabled = true;
        settings.preamp_db = 12.0;
        eq.set_settings(settings);

        let mut prev = 0.5;
        let mut max_step = 0.0_f64;
        for _ in 0..2000 {
            let (y, _) = eq.process_frame(0.5, 0.5);
            max_step = max_step.max((y - prev).abs());
            prev = y;
        }
        // Without smoothing, first sample would jump from 0.5 to ~1.99 (step ~1.49).
        assert!(
            max_step < 0.02,
            "zipper step too large: max_step={max_step}"
        );
        // Should converge close to +12 dB (~1.99)
        assert!((prev - 0.5 * db_to_linear(12.0)).abs() < 0.05);
    }

    #[test]
    fn handles_low_sample_rate_above_nyquist_bands() {
        let mut eq = Equalizer10Band::new(22_050);
        let mut settings = EqSettings::flat();
        settings.enabled = true;
        settings.bands_db = [12.0; 10]; // 12k, 14k, 16k exceed Nyquist (11025 Hz)
        eq.set_settings(settings);
        eq.snap_to_target();

        for _ in 0..256 {
            let (l, r) = eq.process_frame(0.1, -0.1);
            assert!(l.is_finite() && r.is_finite());
        }
    }
}
