//! Gapless playback decoder chaining consecutive tracks with zero silence insertion.

use std::path::Path;
use std::time::Duration;

use crate::decoder::{DecoderError, SymphoniaDecoder, TrackFormat};

/// Notification emitted when [`GaplessDecoder`] seamlessly transitions to the queued next track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaplessTransition {
    /// Label or path string associated with the newly active track.
    pub label: String,
    /// Format of the newly active track.
    pub format: TrackFormat,
    /// Stereo frame offset within the returned chunk where the new track begins.
    pub frame_offset_in_chunk: usize,
    /// True if sample rate changed and the caller should reconfigure DSP/output at this boundary.
    pub format_changed: bool,
}

/// Output of a single [`GaplessDecoder::read_frames`] call.
#[derive(Debug, Clone)]
pub struct GaplessChunk {
    /// Interleaved stereo `f32` audio samples.
    pub samples: Vec<f32>,
    /// Number of stereo frames (`samples.len() / 2`).
    pub frames: usize,
    /// Present if a track transition occurred during this chunk.
    pub transition: Option<GaplessTransition>,
    /// True when the entire queue (current + next) has reached EOF.
    pub reached_end: bool,
}

struct TrackSlot {
    label: String,
    decoder: SymphoniaDecoder,
}

/// Wraps [`SymphoniaDecoder`] with pre-opened next-track queueing for true gapless transitions.
pub struct GaplessDecoder {
    current: TrackSlot,
    next: Option<TrackSlot>,
}

impl GaplessDecoder {
    /// Starts a gapless stream from an already-opened decoder.
    #[must_use]
    pub fn new(label: impl Into<String>, decoder: SymphoniaDecoder) -> Self {
        Self {
            current: TrackSlot {
                label: label.into(),
                decoder,
            },
            next: None,
        }
    }

    /// Starts a gapless stream directly from a file path.
    pub fn open_file(path: impl AsRef<Path>) -> Result<Self, DecoderError> {
        let p = path.as_ref();
        let decoder = SymphoniaDecoder::open_file(p)?;
        Ok(Self::new(p.display().to_string(), decoder))
    }

    /// Pre-opens and queues the next file for gapless transition.
    pub fn queue_next_file(&mut self, path: impl AsRef<Path>) -> Result<(), DecoderError> {
        let p = path.as_ref();
        let decoder = SymphoniaDecoder::open_file(p)?;
        self.queue_next_decoder(p.display().to_string(), decoder);
        Ok(())
    }

    /// Queues an already-opened decoder for gapless transition.
    pub fn queue_next_decoder(&mut self, label: impl Into<String>, decoder: SymphoniaDecoder) {
        self.next = Some(TrackSlot {
            label: label.into(),
            decoder,
        });
    }

    /// Clears any queued next track.
    pub fn clear_next(&mut self) {
        self.next = None;
    }

    /// Returns true if a next track is pre-opened and ready.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }

    /// Returns the currently playing track's label.
    #[must_use]
    pub fn current_label(&self) -> &str {
        &self.current.label
    }

    /// Returns the currently playing track's [`TrackFormat`].
    #[must_use]
    pub fn current_format(&self) -> &TrackFormat {
        self.current.decoder.format()
    }

    /// Returns the current track's playback position.
    #[must_use]
    pub fn position(&self) -> Duration {
        self.current.decoder.position()
    }

    /// Seeks within the currently active track.
    pub fn seek(&mut self, target: Duration) -> Result<Duration, DecoderError> {
        self.current.decoder.seek(target)
    }

    /// Reads up to `requested_frames` stereo frames, seamlessly bridging across track boundaries
    /// when sample rates match.
    pub fn read_frames(&mut self, requested_frames: usize) -> Result<GaplessChunk, DecoderError> {
        let mut samples = Vec::with_capacity(requested_frames * 2);
        let first_frames = self
            .current
            .decoder
            .read_stereo_frames(&mut samples, requested_frames)?;

        if first_frames == requested_frames {
            return Ok(GaplessChunk {
                samples,
                frames: first_frames,
                transition: None,
                reached_end: false,
            });
        }

        // Current track hit EOF before filling requested_frames.
        if let Some(next_slot) = self.next.take() {
            let prev_rate = self.current.decoder.format().sample_rate;
            let next_rate = next_slot.decoder.format().sample_rate;
            let next_format = next_slot.decoder.format().clone();
            let next_label = next_slot.label.clone();

            self.current = next_slot;

            if prev_rate == next_rate {
                // True sample-accurate gapless stitch within the exact same audio buffer!
                let remaining = requested_frames - first_frames;
                let extra_frames = self
                    .current
                    .decoder
                    .read_stereo_frames(&mut samples, remaining)?;
                return Ok(GaplessChunk {
                    samples,
                    frames: first_frames + extra_frames,
                    transition: Some(GaplessTransition {
                        label: next_label,
                        format: next_format,
                        frame_offset_in_chunk: first_frames,
                        format_changed: false,
                    }),
                    reached_end: false,
                });
            }

            // Sample rate changed: return the tail of the old track first and signal boundary.
            return Ok(GaplessChunk {
                samples,
                frames: first_frames,
                transition: Some(GaplessTransition {
                    label: next_label,
                    format: next_format,
                    frame_offset_in_chunk: first_frames,
                    format_changed: true,
                }),
                reached_end: false,
            });
        }

        Ok(GaplessChunk {
            samples,
            frames: first_frames,
            transition: None,
            reached_end: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::encode_pcm16_wav;

    #[test]
    fn gapless_stitches_two_tracks_with_zero_gap_samples() {
        let sr = 48_000;
        // Track A: 500 frames of +0.5; Track B: 500 frames of -0.5
        let wav_a = encode_pcm16_wav(sr, &vec![0.5; 500 * 2]);
        let wav_b = encode_pcm16_wav(sr, &vec![-0.5; 500 * 2]);

        let dec_a = SymphoniaDecoder::open_bytes(wav_a, Some("wav")).expect("track A");
        let dec_b = SymphoniaDecoder::open_bytes(wav_b, Some("wav")).expect("track B");

        let mut gapless = GaplessDecoder::new("track_a.wav", dec_a);
        gapless.queue_next_decoder("track_b.wav", dec_b);

        // Request 800 frames -> must receive 500 from A + 300 from B seamlessly!
        let chunk = gapless.read_frames(800).expect("read gapless");
        assert_eq!(chunk.frames, 800);
        assert!(!chunk.reached_end);
        let tr = chunk.transition.expect("must emit transition");
        assert_eq!(tr.label, "track_b.wav");
        assert_eq!(tr.frame_offset_in_chunk, 500);
        assert!(!tr.format_changed);

        // Verify last frame of A is ~+0.5 and very next frame is ~-0.5 (no zero sample inserted)
        assert!(chunk.samples[499 * 2] > 0.45);
        assert!(chunk.samples[500 * 2] < -0.45);

        // Read remaining 200 frames of Track B
        let tail = gapless.read_frames(800).expect("read tail");
        assert_eq!(tail.frames, 200);
        assert!(tail.reached_end);
    }
}
