//! Clickless raised-cosine ramp for smooth Play/Pause/Stop/Seek transitions.

use std::f64::consts::PI;

/// Default ramp duration in milliseconds (10 ms).
pub const DEFAULT_RAMP_MS: f64 = 10.0;

/// Smooth raised-cosine gain ramp preventing audible clicks on transport actions.
#[derive(Debug, Clone)]
pub struct ClicklessRamp {
    sample_rate: f64,
    ramp_ms: f64,
    start_gain: f64,
    target_gain: f64,
    current_gain: f64,
    current_frame: usize,
    total_frames: usize,
    active: bool,
}

impl ClicklessRamp {
    /// Creates a new clickless ramp starting at unity gain (`1.0`).
    #[must_use]
    pub fn new(sample_rate: u32, ramp_ms: f64) -> Self {
        Self {
            sample_rate: f64::from(sample_rate.max(8_000)),
            ramp_ms: ramp_ms.max(0.5),
            start_gain: 1.0,
            target_gain: 1.0,
            current_gain: 1.0,
            current_frame: 0,
            total_frames: 0,
            active: false,
        }
    }

    /// Updates sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = f64::from(sample_rate.max(8_000));
    }

    /// Triggers a smooth fade-in from the current gain to `1.0`.
    pub fn fade_in(&mut self) {
        self.ramp_to(1.0);
    }

    /// Triggers a smooth fade-out from the current gain to `0.0`.
    pub fn fade_out(&mut self) {
        self.ramp_to(0.0);
    }

    /// Starts a raised-cosine ramp from `current_gain` to `target`.
    pub fn ramp_to(&mut self, target: f64) {
        let target = target.clamp(0.0, 1.0);
        if (self.current_gain - target).abs() < 1e-9 {
            self.current_gain = target;
            self.target_gain = target;
            self.active = false;
            return;
        }
        self.start_gain = self.current_gain;
        self.target_gain = target;
        self.current_frame = 0;
        self.total_frames = ((self.ramp_ms / 1000.0) * self.sample_rate)
            .round()
            .max(1.0) as usize;
        self.active = true;
    }

    /// Immediately sets gain without ramping (e.g., before starting a fresh fade-in from silence).
    pub fn snap_to(&mut self, gain: f64) {
        let g = gain.clamp(0.0, 1.0);
        self.start_gain = g;
        self.target_gain = g;
        self.current_gain = g;
        self.current_frame = 0;
        self.total_frames = 0;
        self.active = false;
    }

    /// Returns true while a ramp transition is in progress.
    #[must_use]
    pub fn is_ramping(&self) -> bool {
        self.active
    }

    /// Returns true when the ramp has settled at silence (`0.0`).
    #[must_use]
    pub fn is_silent(&self) -> bool {
        !self.active && self.current_gain <= 1e-9
    }

    /// Returns true when the ramp is steady at unity gain (`1.0`).
    #[must_use]
    pub fn is_unity(&self) -> bool {
        !self.active && (self.current_gain - 1.0).abs() <= 1e-9
    }

    /// Advances by one sample frame and returns the raised-cosine gain multiplier.
    #[inline]
    pub fn next_gain(&mut self) -> f64 {
        if !self.active {
            return self.current_gain;
        }
        self.current_frame += 1;
        if self.current_frame >= self.total_frames {
            self.current_gain = self.target_gain;
            self.active = false;
            return self.current_gain;
        }
        let t = self.current_frame as f64 / self.total_frames as f64;
        // Raised cosine (Hann half-window) has zero derivative at both endpoints.
        let eased = 0.5 * (1.0 - (PI * t).cos());
        self.current_gain = self.start_gain + (self.target_gain - self.start_gain) * eased;
        self.current_gain
    }

    /// Applies the ramp to a stereo sample pair.
    #[inline]
    pub fn process_frame(&mut self, left: f64, right: f64) -> (f64, f64) {
        let g = self.next_gain();
        (left * g, right * g)
    }

    /// Applies the ramp to an interleaved stereo `f64` buffer in place.
    pub fn process_interleaved(&mut self, samples: &mut [f64]) {
        if self.is_unity() {
            return;
        }
        for frame in samples.as_chunks_mut::<2>().0 {
            let g = self.next_gain();
            frame[0] *= g;
            frame[1] *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_in_and_out_are_smooth_and_reach_targets() {
        let mut ramp = ClicklessRamp::new(48_000, 10.0);
        ramp.snap_to(0.0);
        assert!(ramp.is_silent());

        ramp.fade_in();
        assert!(ramp.is_ramping());

        let mut prev = 0.0;
        let mut max_delta = 0.0_f64;
        for _ in 0..480 {
            let g = ramp.next_gain();
            assert!(g >= prev - 1e-12);
            max_delta = max_delta.max(g - prev);
            prev = g;
        }
        assert!(ramp.is_unity());
        assert!(max_delta < 0.01, "ramp step too steep: {max_delta}");

        ramp.fade_out();
        for _ in 0..480 {
            let _ = ramp.next_gain();
        }
        assert!(ramp.is_silent());
    }
}
