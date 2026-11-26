//! High-quality Sinc audio resampler backed by `rubato`.

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use thiserror::Error;

/// Errors returned by the resampler or DSP pipeline.
#[derive(Debug, Error)]
pub enum DspError {
    #[error("invalid sample rate: input={input_rate}, output={output_rate}")]
    InvalidSampleRate { input_rate: u32, output_rate: u32 },
    #[error("resampler construction failed: {0}")]
    ResamplerInit(String),
    #[error("resampler processing failed: {0}")]
    ResamplerProcess(String),
}

/// Stereo Sinc resampler using 64-bit internal floating-point math (`rubato::SincFixedIn<f64>`).
pub struct SincResampler {
    input_rate: u32,
    output_rate: u32,
    chunk_size: usize,
    inner: SincFixedIn<f64>,
    pending_l: Vec<f64>,
    pending_r: Vec<f64>,
}

impl SincResampler {
    /// Creates a new stereo Sinc resampler from `input_rate` to `output_rate`.
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, DspError> {
        if input_rate == 0 || output_rate == 0 {
            return Err(DspError::InvalidSampleRate {
                input_rate,
                output_rate,
            });
        }
        let chunk_size = 1024;
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = f64::from(output_rate) / f64::from(input_rate);
        let inner = SincFixedIn::<f64>::new(ratio, 2.0, params, chunk_size, 2)
            .map_err(|e| DspError::ResamplerInit(e.to_string()))?;

        Ok(Self {
            input_rate,
            output_rate,
            chunk_size,
            inner,
            pending_l: Vec::with_capacity(chunk_size * 2),
            pending_r: Vec::with_capacity(chunk_size * 2),
        })
    }

    /// Returns the input sample rate in Hz.
    #[must_use]
    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Returns the output sample rate in Hz.
    #[must_use]
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Clears pending input buffers and resets internal filter state.
    pub fn reset(&mut self) {
        self.pending_l.clear();
        self.pending_r.clear();
        self.inner.reset();
    }

    /// Resamples an interleaved stereo `f64` buffer, returning interleaved stereo `f64` output.
    pub fn process_interleaved(&mut self, interleaved: &[f64]) -> Result<Vec<f64>, DspError> {
        for frame in interleaved.as_chunks::<2>().0 {
            self.pending_l.push(frame[0]);
            self.pending_r.push(frame[1]);
        }

        let mut out = Vec::new();
        while self.pending_l.len() >= self.chunk_size {
            let chunk_l: Vec<f64> = self.pending_l.drain(..self.chunk_size).collect();
            let chunk_r: Vec<f64> = self.pending_r.drain(..self.chunk_size).collect();
            let waves_in = [chunk_l, chunk_r];
            let waves_out = self
                .inner
                .process(&waves_in, None)
                .map_err(|e| DspError::ResamplerProcess(e.to_string()))?;

            let frames = waves_out[0].len();
            out.reserve(frames * 2);
            for i in 0..frames {
                out.push(waves_out[0][i]);
                out.push(waves_out[1][i]);
            }
        }
        Ok(out)
    }

    /// Flushes any remaining partial frames at end-of-stream, trimming zero-padded tail frames.
    pub fn flush(&mut self) -> Result<Vec<f64>, DspError> {
        if self.pending_l.is_empty() {
            return Ok(Vec::new());
        }
        let partial_in_frames = self.pending_l.len();
        let chunk_l = std::mem::take(&mut self.pending_l);
        let chunk_r = std::mem::take(&mut self.pending_r);
        let waves_in = [chunk_l, chunk_r];
        let waves_out = self
            .inner
            .process_partial(Some(&waves_in), None)
            .map_err(|e| DspError::ResamplerProcess(e.to_string()))?;

        let ratio = f64::from(self.output_rate) / f64::from(self.input_rate);
        let expected_frames = (partial_in_frames as f64 * ratio).round() as usize;
        let frames = waves_out[0].len().min(expected_frames);
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            out.push(waves_out[0][i]);
            out.push(waves_out[1][i]);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn resamples_44100_to_48000_preserving_ratio() {
        let mut resampler = SincResampler::new(44_100, 48_000).expect("resampler init");
        let in_frames = 4410; // 0.1 seconds
        let mut input = Vec::with_capacity(in_frames * 2);
        for n in 0..in_frames {
            let s = (2.0 * PI * 440.0 * n as f64 / 44_100.0).sin();
            input.push(s);
            input.push(s);
        }

        let mut output = resampler.process_interleaved(&input).expect("process");
        output.extend(resampler.flush().expect("flush"));

        let out_frames = output.len() / 2;
        // Expected ~4800 frames (within small filter latency tolerance)
        assert!(
            (4600..=5000).contains(&out_frames),
            "unexpected resampled frame count: {out_frames}"
        );
        assert!(output.iter().all(|v| v.is_finite()));
    }
}
