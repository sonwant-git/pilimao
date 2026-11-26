//! Triangular Probability Density Function (TPDF) Dither for 16-bit and 24-bit quantization.

/// Target integer quantization depth for TPDF dithering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DitherDepth {
    /// Bypass dithering (e.g., floating-point output or bit-perfect pass-through).
    None,
    /// 16-bit signed integer PCM (`i16`).
    Bits16,
    /// 24-bit signed integer PCM (stored in `i32` range `[-8_388_608, 8_388_607]`).
    Bits24,
}

/// Fast deterministic TPDF dither generator backed by SplitMix64.
#[derive(Debug, Clone)]
pub struct TpdfDither {
    state: u64,
    depth: DitherDepth,
}

impl Default for TpdfDither {
    fn default() -> Self {
        Self::new(DitherDepth::Bits16)
    }
}

impl TpdfDither {
    /// Creates a new TPDF ditherer with a deterministic non-zero seed.
    #[must_use]
    pub fn new(depth: DitherDepth) -> Self {
        Self::with_seed(depth, 0x9E37_79B9_7F4A_7C15)
    }

    /// Creates a new TPDF ditherer with a custom seed.
    #[must_use]
    pub fn with_seed(depth: DitherDepth, seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
            depth,
        }
    }

    /// Sets the target bit depth.
    pub fn set_depth(&mut self, depth: DitherDepth) {
        self.depth = depth;
    }

    /// Returns the configured bit depth.
    #[must_use]
    pub fn depth(&self) -> DitherDepth {
        self.depth
    }

    /// Generates a uniform random `f64` in `[0.0, 1.0)`.
    #[inline]
    fn next_uniform(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // Top 53 bits mapped to [0, 1)
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Generates a triangular PDF noise sample in `(-1.0, 1.0)` (1 LSB peak-to-peak amplitude).
    #[inline]
    pub fn next_tpdf(&mut self) -> f64 {
        self.next_uniform() - self.next_uniform()
    }

    /// Quantizes a normalized `f64` sample in `[-1.0, 1.0]` to `i16` with TPDF dither.
    #[inline]
    pub fn quantize_i16(&mut self, sample: f64) -> i16 {
        let scaled = sample.clamp(-1.0, 1.0) * 32_768.0;
        let dithered = match self.depth {
            DitherDepth::None => scaled,
            DitherDepth::Bits16 | DitherDepth::Bits24 => scaled + self.next_tpdf(),
        };
        dithered
            .round()
            .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
    }

    /// Quantizes a normalized `f64` sample in `[-1.0, 1.0]` to signed 24-bit integer (`i32`).
    #[inline]
    pub fn quantize_i24(&mut self, sample: f64) -> i32 {
        const MIN_24: f64 = -8_388_608.0;
        const MAX_24: f64 = 8_388_607.0;
        let scaled = sample.clamp(-1.0, 1.0) * 8_388_608.0;
        let dithered = match self.depth {
            DitherDepth::None => scaled,
            DitherDepth::Bits16 | DitherDepth::Bits24 => scaled + self.next_tpdf(),
        };
        dithered.round().clamp(MIN_24, MAX_24) as i32
    }

    /// Applies in-place float-domain TPDF dither quantized to the configured bit depth,
    /// returning normalized `f64` samples (useful when outputting quantized audio over float streams).
    pub fn process_float_in_place(&mut self, samples: &mut [f64]) {
        match self.depth {
            DitherDepth::None => {}
            DitherDepth::Bits16 => {
                for s in samples.iter_mut() {
                    let q = self.quantize_i16(*s);
                    *s = f64::from(q) / 32_768.0;
                }
            }
            DitherDepth::Bits24 => {
                for s in samples.iter_mut() {
                    let q = self.quantize_i24(*s);
                    *s = f64::from(q) / 8_388_608.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tpdf_noise_has_zero_mean_and_expected_variance() {
        let mut dither = TpdfDither::new(DitherDepth::Bits16);
        let n = 100_000;
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        for _ in 0..n {
            let x = dither.next_tpdf();
            assert!((-1.0..=1.0).contains(&x));
            sum += x;
            sum_sq += x * x;
        }
        let mean = sum / n as f64;
        let var = sum_sq / n as f64 - mean * mean;
        assert!(mean.abs() < 0.01, "mean={mean}");
        // Theoretical variance of U(0,1)-U(0,1) is 1/6 ≈ 0.1667
        assert!((var - (1.0 / 6.0)).abs() < 0.01, "var={var}");
    }

    #[test]
    fn quantize_i16_and_i24_clamps_bounds() {
        let mut d = TpdfDither::new(DitherDepth::Bits16);
        assert!(d.quantize_i16(1.5) >= i16::MAX - 1);
        assert!(d.quantize_i16(-1.5) <= i16::MIN + 1);
        d.set_depth(DitherDepth::Bits24);
        assert!(d.quantize_i24(1.5) >= 8_388_606);
        assert!(d.quantize_i24(-1.5) <= -8_388_607);
        d.set_depth(DitherDepth::None);
        assert_eq!(d.quantize_i16(1.5), i16::MAX);
        assert_eq!(d.quantize_i16(-1.5), i16::MIN);
    }
}
