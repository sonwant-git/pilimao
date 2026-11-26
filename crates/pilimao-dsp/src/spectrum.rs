//! Classic Winamp 19-band FFT spectrum analyzer & 76-point oscilloscope powered by `rustfft`.

use std::f32::consts::PI;
use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

/// Number of classic Winamp 2.x spectrum analyzer bars.
pub const SPECTRUM_BANDS: usize = 19;

/// Number of horizontal pixels in classic Winamp 2.x visualization area (76x16).
pub const OSCILLOSCOPE_SAMPLES: usize = 76;

/// FFT window size for spectrum computation.
pub const FFT_SIZE: usize = 1024;

/// Visualization snapshot suitable for triple-buffered lock-free transfer to the UI thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisSnapshot {
    /// 19 normalized spectrum bar heights in `0.0..=1.0`.
    pub bands: [f32; SPECTRUM_BANDS],
    /// 19 peak-hold indicator heights in `0.0..=1.0`.
    pub peaks: [f32; SPECTRUM_BANDS],
    /// 76 mono waveform samples in `-1.0..=1.0` for oscilloscope mode.
    pub waveform: [f32; OSCILLOSCOPE_SAMPLES],
    /// Instantaneous stereo peak levels (`left`, `right`) in `0.0..=1.0`.
    pub vu_levels: [f32; 2],
}

impl Default for VisSnapshot {
    fn default() -> Self {
        Self {
            bands: [0.0; SPECTRUM_BANDS],
            peaks: [0.0; SPECTRUM_BANDS],
            waveform: [0.0; OSCILLOSCOPE_SAMPLES],
            vu_levels: [0.0; 2],
        }
    }
}

/// Real-time 19-band FFT spectrum analyzer with Winamp-style falloff and peak hold.
pub struct SpectrumAnalyzer {
    sample_rate: u32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    ring: Vec<f32>,
    ring_pos: usize,
    scratch: Vec<Complex<f32>>,
    fft_buf: Vec<Complex<f32>>,
    band_ranges: [(usize, usize); SPECTRUM_BANDS],
    bands: [f32; SPECTRUM_BANDS],
    peaks: [f32; SPECTRUM_BANDS],
    peak_hold_frames: [u8; SPECTRUM_BANDS],
    waveform: [f32; OSCILLOSCOPE_SAMPLES],
    vu_levels: [f32; 2],
}

impl SpectrumAnalyzer {
    /// Creates a new 19-band spectrum analyzer for `sample_rate`.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let scratch_len = fft.get_inplace_scratch_len();

        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
            .collect();

        let sr = sample_rate.max(8_000);
        Self {
            sample_rate: sr,
            fft,
            window,
            ring: vec![0.0; FFT_SIZE],
            ring_pos: 0,
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            band_ranges: compute_band_ranges(sr),
            bands: [0.0; SPECTRUM_BANDS],
            peaks: [0.0; SPECTRUM_BANDS],
            peak_hold_frames: [0; SPECTRUM_BANDS],
            waveform: [0.0; OSCILLOSCOPE_SAMPLES],
            vu_levels: [0.0; 2],
        }
    }

    /// Updates sample rate and recomputes logarithmic frequency bin ranges.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        let sr = sample_rate.max(8_000);
        if self.sample_rate != sr {
            self.sample_rate = sr;
            self.band_ranges = compute_band_ranges(sr);
        }
    }

    /// Clears analyzer state (e.g. on Stop).
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.ring_pos = 0;
        self.bands = [0.0; SPECTRUM_BANDS];
        self.peaks = [0.0; SPECTRUM_BANDS];
        self.peak_hold_frames = [0; SPECTRUM_BANDS];
        self.waveform = [0.0; OSCILLOSCOPE_SAMPLES];
        self.vu_levels = [0.0; 2];
    }

    /// Pushes interleaved stereo `f32` samples into the analyzer ring buffer.
    pub fn push_interleaved_f32(&mut self, samples: &[f32]) {
        let mut max_l = 0.0_f32;
        let mut max_r = 0.0_f32;
        for frame in samples.as_chunks::<2>().0 {
            let l = frame[0];
            let r = frame[1];
            max_l = max_l.max(l.abs());
            max_r = max_r.max(r.abs());
            self.ring[self.ring_pos] = 0.5 * (l + r);
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        self.vu_levels[0] = (self.vu_levels[0] * 0.85).max(max_l.min(1.0));
        self.vu_levels[1] = (self.vu_levels[1] * 0.85).max(max_r.min(1.0));
    }

    /// Pushes interleaved stereo `f64` samples into the analyzer ring buffer.
    pub fn push_interleaved_f64(&mut self, samples: &[f64]) {
        let mut max_l = 0.0_f32;
        let mut max_r = 0.0_f32;
        for frame in samples.as_chunks::<2>().0 {
            let l = frame[0] as f32;
            let r = frame[1] as f32;
            max_l = max_l.max(l.abs());
            max_r = max_r.max(r.abs());
            self.ring[self.ring_pos] = 0.5 * (l + r);
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        self.vu_levels[0] = (self.vu_levels[0] * 0.85).max(max_l.min(1.0));
        self.vu_levels[1] = (self.vu_levels[1] * 0.85).max(max_r.min(1.0));
    }

    /// Runs the 1024-point FFT and computes an updated [`VisSnapshot`].
    pub fn analyze(&mut self) -> VisSnapshot {
        // Copy ring buffer in chronological order with Hann window applied.
        for i in 0..FFT_SIZE {
            let idx = (self.ring_pos + i) % FFT_SIZE;
            let sample = self.ring[idx];
            self.fft_buf[i] = Complex::new(sample * self.window[i], 0.0);
        }

        // Capture most recent 76 samples for classic oscilloscope view.
        let wave_start = (self.ring_pos + FFT_SIZE - OSCILLOSCOPE_SAMPLES) % FFT_SIZE;
        for i in 0..OSCILLOSCOPE_SAMPLES {
            self.waveform[i] = self.ring[(wave_start + i) % FFT_SIZE].clamp(-1.0, 1.0);
        }

        self.fft
            .process_with_scratch(&mut self.fft_buf, &mut self.scratch);

        // Normalize FFT magnitude (compensate for Hann window coherent gain ~0.5)
        let norm = 4.0 / FFT_SIZE as f32;
        const FLOOR_DB: f32 = -60.0;
        const BAR_FALLOFF: f32 = 0.065;
        const PEAK_FALLOFF: f32 = 0.025;
        const PEAK_HOLD_TICKS: u8 = 12;

        for (band_idx, &(bin_start, bin_end)) in self.band_ranges.iter().enumerate() {
            let mut sum_sq = 0.0_f32;
            let count = (bin_end - bin_start).max(1);
            for c in &self.fft_buf[bin_start..bin_end] {
                let mag = c.norm() * norm;
                sum_sq += mag * mag;
            }
            let rms_mag = (sum_sq / count as f32).sqrt();
            let db = 20.0 * rms_mag.max(1e-6).log10();
            let normalized = ((db - FLOOR_DB) / (-FLOOR_DB)).clamp(0.0, 1.0);

            if normalized >= self.bands[band_idx] {
                self.bands[band_idx] = normalized;
            } else {
                self.bands[band_idx] = (self.bands[band_idx] - BAR_FALLOFF).max(normalized);
            }

            if self.bands[band_idx] >= self.peaks[band_idx] {
                self.peaks[band_idx] = self.bands[band_idx];
                self.peak_hold_frames[band_idx] = PEAK_HOLD_TICKS;
            } else if self.peak_hold_frames[band_idx] > 0 {
                self.peak_hold_frames[band_idx] -= 1;
            } else {
                self.peaks[band_idx] = (self.peaks[band_idx] - PEAK_FALLOFF).max(0.0);
            }
        }

        VisSnapshot {
            bands: self.bands,
            peaks: self.peaks,
            waveform: self.waveform,
            vu_levels: self.vu_levels,
        }
    }
}

/// Computes 19 logarithmically spaced FFT bin ranges from ~60 Hz up to min(18 kHz, Nyquist).
fn compute_band_ranges(sample_rate: u32) -> [(usize, usize); SPECTRUM_BANDS] {
    let nyquist = (sample_rate as f32) * 0.5;
    let f_min = 60.0_f32;
    let f_max = 18_000.0_f32.min(nyquist * 0.95).max(f_min * 2.0);
    let bin_hz = (sample_rate as f32) / (FFT_SIZE as f32);
    let max_bin = FFT_SIZE / 2;

    let mut ranges = [(1usize, 2usize); SPECTRUM_BANDS];
    let ratio = (f_max / f_min).powf(1.0 / SPECTRUM_BANDS as f32);

    let mut prev_end = 1usize;
    for (i, slot) in ranges.iter_mut().enumerate() {
        let f_lo = f_min * ratio.powi(i as i32);
        let f_hi = f_min * ratio.powi((i + 1) as i32);
        let start = ((f_lo / bin_hz).floor() as usize)
            .max(prev_end)
            .min(max_bin - 1);
        let end = ((f_hi / bin_hz).ceil() as usize)
            .max(start + 1)
            .min(max_bin);
        *slot = (start, end);
        prev_end = start.max(1);
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_1khz_tone_in_mid_spectrum_band() {
        let sr = 48_000;
        let mut analyzer = SpectrumAnalyzer::new(sr);
        let mut samples = Vec::with_capacity(FFT_SIZE * 2);
        for n in 0..FFT_SIZE {
            let s = (2.0 * PI * 1000.0 * n as f32 / sr as f32).sin();
            samples.push(s);
            samples.push(s);
        }
        analyzer.push_interleaved_f32(&samples);
        let snap = analyzer.analyze();

        let (peak_band, &peak_val) = snap
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("19 bands");

        // 1 kHz sits near the middle of log-spaced 60 Hz..18 kHz (around band 8..11)
        assert!(
            (7..=11).contains(&peak_band),
            "expected peak band near 1 kHz (7..=11), got band {peak_band} (val={peak_val})"
        );
        assert!(peak_val > 0.7, "expected strong peak, got {peak_val}");
        assert!(snap.peaks[peak_band] >= peak_val);
    }
}
