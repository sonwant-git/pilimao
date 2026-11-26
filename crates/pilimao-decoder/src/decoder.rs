//! Multi-format audio decoder backed by `symphonia`.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{
    CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP1, CODEC_TYPE_MP2,
    CODEC_TYPE_MP3, CODEC_TYPE_NULL, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE,
    CODEC_TYPE_PCM_F64BE, CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE,
    CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE, CODEC_TYPE_PCM_S32BE, CODEC_TYPE_PCM_S32LE,
    CODEC_TYPE_PCM_U8, CODEC_TYPE_VORBIS, CodecType, Decoder, DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use thiserror::Error;

/// Errors returned by [`SymphoniaDecoder`] and [`crate::GaplessDecoder`].
#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or unrecognized audio format: {0}")]
    UnsupportedFormat(String),
    #[error("no playable audio track found")]
    NoAudioTrack,
    #[error("codec initialization failed: {0}")]
    CodecInit(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("seek error: {0}")]
    Seek(String),
}

/// Stream format metadata for an opened audio track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u32>,
    pub total_frames: Option<u64>,
    pub duration: Option<Duration>,
    pub codec_name: String,
    pub bitrate_kbps: Option<u32>,
}

/// Multi-format Symphonia decoder producing interleaved stereo `f32` audio.
pub struct SymphoniaDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    track_format: TrackFormat,
    /// Leftover interleaved stereo `f32` samples from the last decoded packet.
    pending: Vec<f32>,
    pending_offset: usize,
    position_frames: u64,
    eof: bool,
}

impl SymphoniaDecoder {
    /// Opens an audio file from disk with gapless trimming enabled.
    pub fn open_file(path: impl AsRef<Path>) -> Result<Self, DecoderError> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let file_len = file.metadata().ok().map(|m| m.len());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        Self::open_source(Box::new(file), hint, file_len)
    }

    /// Opens an audio stream from in-memory bytes (optional extension hint like `"wav"` or `"mp3"`).
    pub fn open_bytes(bytes: Vec<u8>, extension_hint: Option<&str>) -> Result<Self, DecoderError> {
        let len = Some(bytes.len() as u64);
        let mut hint = Hint::new();
        if let Some(ext) = extension_hint {
            hint.with_extension(ext);
        }
        Self::open_source(Box::new(Cursor::new(bytes)), hint, len)
    }

    fn open_source(
        source: Box<dyn MediaSource>,
        hint: Hint,
        byte_len: Option<u64>,
    ) -> Result<Self, DecoderError> {
        let mss = MediaSourceStream::new(source, Default::default());
        let fmt_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let meta_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| DecoderError::UnsupportedFormat(e.to_string()))?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(DecoderError::NoAudioTrack)?;

        let track_id = track.id;
        let params = track.codec_params.clone();

        let sample_rate = params.sample_rate.unwrap_or(44_100);
        let channels = params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2)
            .max(1);
        let total_frames = params.n_frames;
        let duration = total_frames
            .map(|frames| Duration::from_secs_f64(frames as f64 / f64::from(sample_rate)));
        let bitrate_kbps = match (byte_len, duration) {
            (Some(bytes), Some(dur)) if dur.as_secs_f64() > 0.01 => {
                Some(((bytes as f64 * 8.0) / dur.as_secs_f64() / 1000.0).round() as u32)
            }
            _ => None,
        };

        let track_format = TrackFormat {
            sample_rate,
            channels,
            bits_per_sample: params.bits_per_sample,
            total_frames,
            duration,
            codec_name: codec_display_name(params.codec),
            bitrate_kbps,
        };

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| DecoderError::CodecInit(e.to_string()))?;

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            track_format,
            pending: Vec::new(),
            pending_offset: 0,
            position_frames: 0,
            eof: false,
        })
    }

    /// Returns static stream metadata for this track.
    #[must_use]
    pub fn format(&self) -> &TrackFormat {
        &self.track_format
    }

    /// Returns current playback position in frames.
    #[must_use]
    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    /// Returns current playback position as a [`Duration`].
    #[must_use]
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(
            self.position_frames as f64 / f64::from(self.track_format.sample_rate.max(1)),
        )
    }

    /// Returns true if the stream has reached EOF and no buffered samples remain.
    #[must_use]
    pub fn is_eof(&self) -> bool {
        self.eof && self.pending_offset >= self.pending.len()
    }

    /// Seeks to `target` duration and resets decoder state.
    pub fn seek(&mut self, target: Duration) -> Result<Duration, DecoderError> {
        let secs = target.as_secs();
        let frac = f64::from(target.subsec_nanos()) / 1_000_000_000.0;
        let seeked = self
            .format_reader
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::new(secs, frac),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| DecoderError::Seek(e.to_string()))?;

        self.decoder.reset();
        self.pending.clear();
        self.pending_offset = 0;
        self.eof = false;
        self.position_frames = seeked.actual_ts;

        // Discard frames between packet boundary (`actual_ts`) and requested frame (`required_ts`)
        // for true sample-accurate seeking across large packet sizes (e.g. WAV 4096-frame blocks).
        if seeked.required_ts > seeked.actual_ts {
            let mut to_skip = (seeked.required_ts - seeked.actual_ts) as usize;
            let mut discard = Vec::new();
            while to_skip > 0 {
                discard.clear();
                let step = to_skip.min(2048);
                let got = self.read_stereo_frames(&mut discard, step)?;
                if got == 0 {
                    break;
                }
                to_skip = to_skip.saturating_sub(got);
            }
        }
        Ok(self.position())
    }

    /// Reads up to `max_frames` interleaved stereo `f32` frames into `out`.
    /// Returns the number of stereo frames written (`0` indicates EOF).
    pub fn read_stereo_frames(
        &mut self,
        out: &mut Vec<f32>,
        max_frames: usize,
    ) -> Result<usize, DecoderError> {
        let mut written_frames = 0usize;

        while written_frames < max_frames {
            if self.pending_offset < self.pending.len() {
                let avail_samples = self.pending.len() - self.pending_offset;
                let avail_frames = avail_samples / 2;
                let take_frames = avail_frames.min(max_frames - written_frames);
                let take_samples = take_frames * 2;
                out.extend_from_slice(
                    &self.pending[self.pending_offset..self.pending_offset + take_samples],
                );
                self.pending_offset += take_samples;
                if self.pending_offset >= self.pending.len() {
                    self.pending.clear();
                    self.pending_offset = 0;
                }
                written_frames += take_frames;
                self.position_frames += take_frames as u64;
                continue;
            }

            if self.eof {
                break;
            }

            if !self.decode_next_packet()? {
                self.eof = true;
                break;
            }
        }

        Ok(written_frames)
    }

    /// Decodes the next packet for `self.track_id` into `self.pending` as stereo interleaved `f32`.
    /// Returns `Ok(false)` on EOF.
    fn decode_next_packet(&mut self) -> Result<bool, DecoderError> {
        loop {
            let packet = match self.format_reader.next_packet() {
                Ok(pkt) => pkt,
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(false);
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(err) => return Err(DecoderError::Decode(err.to_string())),
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let ch = spec.channels.count().max(1);
                    let frames = decoded.frames();
                    if frames == 0 {
                        continue;
                    }
                    let mut sample_buf = SampleBuffer::<f32>::new(frames as u64, spec);
                    sample_buf.copy_interleaved_ref(decoded);
                    let raw = sample_buf.samples();

                    self.pending.clear();
                    self.pending_offset = 0;
                    self.pending.reserve(frames * 2);

                    match ch {
                        1 => {
                            for &s in raw {
                                self.pending.push(s);
                                self.pending.push(s);
                            }
                        }
                        2 => {
                            self.pending.extend_from_slice(raw);
                        }
                        n => {
                            // Downmix multi-channel to stereo preserving L/R primary channels + center
                            for frame in raw.chunks_exact(n) {
                                let mut l = frame[0];
                                let mut r = frame[1];
                                if n >= 3 {
                                    let center = frame[2] * 0.707_106_77;
                                    l = (l + center).clamp(-1.0, 1.0);
                                    r = (r + center).clamp(-1.0, 1.0);
                                }
                                self.pending.push(l);
                                self.pending.push(r);
                            }
                        }
                    }
                    return Ok(true);
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Resilient playback: skip corrupt packet like Winamp.
                    continue;
                }
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(false);
                }
                Err(err) => return Err(DecoderError::Decode(err.to_string())),
            }
        }
    }
}

fn codec_display_name(codec: CodecType) -> String {
    match codec {
        CODEC_TYPE_MP3 => "MP3".to_string(),
        CODEC_TYPE_MP2 => "MP2".to_string(),
        CODEC_TYPE_MP1 => "MP1".to_string(),
        CODEC_TYPE_FLAC => "FLAC".to_string(),
        CODEC_TYPE_VORBIS => "Vorbis".to_string(),
        CODEC_TYPE_AAC => "AAC".to_string(),
        CODEC_TYPE_ALAC => "ALAC".to_string(),
        CODEC_TYPE_PCM_U8 => "PCM U8".to_string(),
        CODEC_TYPE_PCM_S16LE => "PCM S16LE".to_string(),
        CODEC_TYPE_PCM_S16BE => "PCM S16BE".to_string(),
        CODEC_TYPE_PCM_S24LE => "PCM S24LE".to_string(),
        CODEC_TYPE_PCM_S24BE => "PCM S24BE".to_string(),
        CODEC_TYPE_PCM_S32LE => "PCM S32LE".to_string(),
        CODEC_TYPE_PCM_S32BE => "PCM S32BE".to_string(),
        CODEC_TYPE_PCM_F32LE => "PCM F32LE".to_string(),
        CODEC_TYPE_PCM_F32BE => "PCM F32BE".to_string(),
        CODEC_TYPE_PCM_F64LE => "PCM F64LE".to_string(),
        CODEC_TYPE_PCM_F64BE => "PCM F64BE".to_string(),
        _ => format!("{codec:?}"),
    }
}

/// Encodes stereo interleaved `f32` samples into an in-memory 16-bit PCM WAV byte vector.
/// Useful for unit tests and CLI `self-test` verification.
#[must_use]
pub fn encode_pcm16_wav(sample_rate: u32, stereo_interleaved: &[f32]) -> Vec<u8> {
    let num_samples = stereo_interleaved.len();
    let data_bytes = (num_samples * 2) as u32;
    let riff_size = 36 + data_bytes;
    let byte_rate = sample_rate * 2 * 2;
    let block_align: u16 = 4;

    let mut buf = Vec::with_capacity(44 + data_bytes as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&riff_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    buf.extend_from_slice(&2u16.to_le_bytes()); // NumChannels = 2
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample = 16
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_bytes.to_le_bytes());

    for &s in stereo_interleaved {
        let q = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        buf.extend_from_slice(&q.to_le_bytes());
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn decodes_and_seeks_synthetic_wav() {
        let sr = 44_100;
        let total_frames = 4410; // 100 ms
        let mut samples = Vec::with_capacity(total_frames * 2);
        for n in 0..total_frames {
            let v = 0.5 * (2.0 * PI * 440.0 * n as f32 / sr as f32).sin();
            samples.push(v);
            samples.push(-v);
        }
        let wav = encode_pcm16_wav(sr, &samples);
        let mut dec = SymphoniaDecoder::open_bytes(wav, Some("wav")).expect("open wav");
        assert_eq!(dec.format().sample_rate, 44_100);
        assert_eq!(dec.format().channels, 2);
        assert_eq!(dec.format().total_frames, Some(total_frames as u64));

        let mut out = Vec::new();
        let read = dec.read_stereo_frames(&mut out, 2000).expect("read");
        assert_eq!(read, 2000);
        assert_eq!(out.len(), 4000);

        dec.seek(Duration::from_millis(50)).expect("seek");
        assert!((dec.position().as_millis() as i64 - 50).abs() <= 2);
    }
}
