//! Lock-free `triple_buffer` visualization tap transferring 19-band FFT spectrum
//! and oscilloscope snapshots from the audio pipeline to the UI thread with zero contention.

use pilimao_dsp::VisSnapshot;
use triple_buffer::{Input, Output, TripleBuffer};

/// Producer end of the visualization tap (owned by the audio/DSP thread).
pub struct VisTapWriter {
    input: Input<VisSnapshot>,
}

impl VisTapWriter {
    /// Publishes a fresh [`VisSnapshot`] lock-free without ever blocking the audio thread.
    #[inline]
    pub fn publish(&mut self, snapshot: VisSnapshot) {
        self.input.write(snapshot);
    }
}

/// Consumer end of the visualization tap (owned by the UI rendering thread).
pub struct VisTapReader {
    output: Output<VisSnapshot>,
}

impl VisTapReader {
    /// Reads the latest available [`VisSnapshot`] lock-free.
    #[inline]
    pub fn latest(&mut self) -> VisSnapshot {
        *self.output.read()
    }
}

/// Creates a connected lock-free `(VisTapWriter, VisTapReader)` pair backed by `triple_buffer`.
#[must_use]
pub fn create_vis_tap() -> (VisTapWriter, VisTapReader) {
    let (input, output) = TripleBuffer::new(&VisSnapshot::default()).split();
    (VisTapWriter { input }, VisTapReader { output })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triple_buffer_transfers_latest_vis_snapshot_lock_free() {
        let (mut writer, mut reader) = create_vis_tap();
        let mut snap = VisSnapshot::default();
        snap.bands[4] = 0.85;
        snap.peaks[4] = 0.92;
        writer.publish(snap);

        let read = reader.latest();
        assert!((read.bands[4] - 0.85).abs() < 1e-6);
        assert!((read.peaks[4] - 0.92).abs() < 1e-6);
    }
}
