//! Real-time lock-free audio engine combining `pilimao-decoder`, `pilimao-dsp`,
//! `rtrb` SPSC ring buffer, `triple_buffer` FFT tap, and WASAPI / Null Sink output.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pilimao_decoder::{GaplessDecoder, SymphoniaDecoder, TrackFormat};
use pilimao_dsp::{
    BitPerfectStatus, DitherDepth, DspConfig, DspPipeline, EqSettings, LimiterConfig,
    ReplayGainConfig, ReplayGainInfo, VisSnapshot,
};
use rtrb::RingBuffer;

use crate::EngineError;
use crate::sink::{ActiveSink, OutputMode};
use crate::vis_tap::{VisTapReader, VisTapWriter, create_vis_tap};
use crate::wasapi::spawn_wasapi_sink;

/// Size of the lock-free `rtrb` ring buffer in stereo interleaved `f32` samples
/// (65,536 samples = 32,768 stereo frames ≈ 682 ms @ 48 kHz).
pub const RING_BUFFER_SAMPLES: usize = 65_536;

/// High-level playback transport state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Snapshot of engine status safely readable from the UI thread.
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    pub state: PlaybackState,
    pub track_label: String,
    pub format: Option<TrackFormat>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub output_mode: OutputMode,
    pub output_sample_rate: u32,
    pub bit_perfect: BitPerfectStatus,
    pub volume: f64,
    pub balance: f64,
    pub eq: EqSettings,
    pub frames_rendered: u64,
    pub underruns: u64,
    pub mmcss_active: bool,
    pub last_error: Option<String>,
}

impl Default for EngineSnapshot {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            track_label: String::new(),
            format: None,
            position: Duration::ZERO,
            duration: None,
            output_mode: OutputMode::Null,
            output_sample_rate: 48_000,
            bit_perfect: BitPerfectStatus {
                is_bit_perfect: false,
                reasons: Vec::new(),
            },
            volume: 1.0,
            balance: 0.0,
            eq: EqSettings::flat(),
            frames_rendered: 0,
            underruns: 0,
            mmcss_active: false,
            last_error: None,
        }
    }
}

/// Configuration for initializing [`AudioEngine`].
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub output_mode: OutputMode,
    pub output_sample_rate: u32,
    /// If using `OutputMode::Null`, controls whether the sink paces at real-time speed.
    pub null_realtime_pacing: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            output_mode: if cfg!(windows) {
                OutputMode::WasapiShared
            } else {
                OutputMode::Null
            },
            output_sample_rate: 48_000,
            null_realtime_pacing: true,
        }
    }
}

enum EngineCommand {
    LoadFile {
        path: PathBuf,
        autoplay: bool,
        replay_gain: ReplayGainInfo,
    },
    LoadDecoder {
        label: String,
        decoder: SymphoniaDecoder,
        autoplay: bool,
        replay_gain: ReplayGainInfo,
    },
    QueueNextFile(PathBuf),
    QueueNextDecoder {
        label: String,
        decoder: SymphoniaDecoder,
    },
    Play,
    Pause,
    Stop,
    Seek(Duration),
    SetVolume(f64),
    SetBalance(f64),
    SetEq(EqSettings),
    SetReplayGainConfig(ReplayGainConfig),
    SetDither(DitherDepth),
    Shutdown,
}

/// Multi-threaded real-time audio engine with lock-free `rtrb` + `triple_buffer` data paths.
pub struct AudioEngine {
    cmd_tx: Sender<EngineCommand>,
    snapshot: Arc<Mutex<EngineSnapshot>>,
    vis_reader: Option<VisTapReader>,
    worker: Option<JoinHandle<()>>,
}

impl AudioEngine {
    /// Spawns the audio engine (decode/DSP thread + WASAPI/Null output sink thread).
    pub fn start(config: EngineConfig) -> Result<Self, EngineError> {
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_SAMPLES);
        let (vis_writer, vis_reader) = create_vis_tap();

        let sink = match config.output_mode {
            OutputMode::Null => ActiveSink::spawn_null(
                consumer,
                config.output_sample_rate,
                config.null_realtime_pacing,
            ),
            OutputMode::WasapiShared | OutputMode::WasapiExclusive => {
                match spawn_wasapi_sink(consumer, config.output_mode, config.output_sample_rate) {
                    Ok(s) => s,
                    Err(_) if !cfg!(windows) => {
                        // Should only happen if non-windows caller explicitly asked for WASAPI
                        return Err(EngineError::WasapiUnavailable);
                    }
                    Err(err) => return Err(err),
                }
            }
        };

        let actual_rate = sink.sample_rate;
        let actual_mode = sink.mode;
        let dsp = DspPipeline::new(DspConfig {
            input_sample_rate: actual_rate,
            output_sample_rate: actual_rate,
            exclusive_mode: actual_mode == OutputMode::WasapiExclusive,
            volume: 1.0,
            balance: 0.0,
            eq: EqSettings::flat(),
            limiter: LimiterConfig::default(),
            replay_gain: ReplayGainConfig::default(),
            dither_depth: DitherDepth::None,
        })
        .map_err(|e| EngineError::Dsp(e.to_string()))?;

        let initial_snapshot = EngineSnapshot {
            output_mode: actual_mode,
            output_sample_rate: actual_rate,
            bit_perfect: dsp.bit_perfect_status(),
            ..Default::default()
        };
        let snapshot = Arc::new(Mutex::new(initial_snapshot));
        let snap_worker = Arc::clone(&snapshot);

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("pilimao-audio-worker".to_string())
            .spawn(move || {
                audio_worker_loop(cmd_rx, producer, vis_writer, sink, dsp, snap_worker);
            })
            .map_err(|e| EngineError::ThreadSpawn(e.to_string()))?;

        Ok(Self {
            cmd_tx,
            snapshot,
            vis_reader: Some(vis_reader),
            worker: Some(worker),
        })
    }

    /// Takes ownership of the lock-free `triple_buffer` visualization reader for the UI thread.
    pub fn take_vis_reader(&mut self) -> Option<VisTapReader> {
        self.vis_reader.take()
    }

    /// Returns a cloned snapshot of current engine telemetry.
    #[must_use]
    pub fn snapshot(&self) -> EngineSnapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn load_file(&self, path: impl Into<PathBuf>, autoplay: bool, replay_gain: ReplayGainInfo) {
        let _ = self.cmd_tx.send(EngineCommand::LoadFile {
            path: path.into(),
            autoplay,
            replay_gain,
        });
    }

    pub fn load_decoder(
        &self,
        label: impl Into<String>,
        decoder: SymphoniaDecoder,
        autoplay: bool,
        replay_gain: ReplayGainInfo,
    ) {
        let _ = self.cmd_tx.send(EngineCommand::LoadDecoder {
            label: label.into(),
            decoder,
            autoplay,
            replay_gain,
        });
    }

    pub fn queue_next_file(&self, path: impl Into<PathBuf>) {
        let _ = self.cmd_tx.send(EngineCommand::QueueNextFile(path.into()));
    }

    pub fn queue_next_decoder(&self, label: impl Into<String>, decoder: SymphoniaDecoder) {
        let _ = self.cmd_tx.send(EngineCommand::QueueNextDecoder {
            label: label.into(),
            decoder,
        });
    }

    pub fn play(&self) {
        let _ = self.cmd_tx.send(EngineCommand::Play);
    }

    pub fn pause(&self) {
        let _ = self.cmd_tx.send(EngineCommand::Pause);
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(EngineCommand::Stop);
    }

    pub fn seek(&self, position: Duration) {
        let _ = self.cmd_tx.send(EngineCommand::Seek(position));
    }

    pub fn set_volume(&self, volume: f64) {
        let _ = self.cmd_tx.send(EngineCommand::SetVolume(volume));
    }

    pub fn set_balance(&self, balance: f64) {
        let _ = self.cmd_tx.send(EngineCommand::SetBalance(balance));
    }

    pub fn set_eq(&self, settings: EqSettings) {
        let _ = self.cmd_tx.send(EngineCommand::SetEq(settings));
    }

    pub fn set_replay_gain_config(&self, cfg: ReplayGainConfig) {
        let _ = self.cmd_tx.send(EngineCommand::SetReplayGainConfig(cfg));
    }

    pub fn set_dither(&self, depth: DitherDepth) {
        let _ = self.cmd_tx.send(EngineCommand::SetDither(depth));
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(EngineCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn audio_worker_loop(
    cmd_rx: Receiver<EngineCommand>,
    mut producer: rtrb::Producer<f32>,
    mut vis_writer: VisTapWriter,
    sink: ActiveSink,
    mut dsp: DspPipeline,
    snapshot: Arc<Mutex<EngineSnapshot>>,
) {
    let output_rate = sink.sample_rate;
    let output_mode = sink.mode;
    let mut decoder: Option<GaplessDecoder> = None;
    let mut state = PlaybackState::Stopped;
    let mut volume = 1.0_f64;
    let mut balance = 0.0_f64;
    let mut eq_settings = EqSettings::flat();
    let mut pending_out: Vec<f32> = Vec::new();
    let mut pending_pos: usize = 0;
    let mut last_error: Option<String> = None;

    const DECODE_BLOCK_FRAMES: usize = 1024;

    loop {
        // 1. Drain all pending control commands without blocking when playing.
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                EngineCommand::Shutdown => {
                    sink.stop();
                    return;
                }
                EngineCommand::LoadFile {
                    path,
                    autoplay,
                    replay_gain,
                } => match GaplessDecoder::open_file(&path) {
                    Ok(dec) => {
                        let in_rate = dec.current_format().sample_rate;
                        let _ = dsp.reconfigure_rates(in_rate, output_rate);
                        dsp.reset_state();
                        dsp.set_track_replay_gain(replay_gain);
                        dsp.fade_in();
                        decoder = Some(dec);
                        pending_out.clear();
                        pending_pos = 0;
                        state = if autoplay {
                            PlaybackState::Playing
                        } else {
                            PlaybackState::Paused
                        };
                        last_error = None;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                },
                EngineCommand::LoadDecoder {
                    label,
                    decoder: raw_dec,
                    autoplay,
                    replay_gain,
                } => {
                    let dec = GaplessDecoder::new(label, raw_dec);
                    let in_rate = dec.current_format().sample_rate;
                    let _ = dsp.reconfigure_rates(in_rate, output_rate);
                    dsp.reset_state();
                    dsp.set_track_replay_gain(replay_gain);
                    dsp.fade_in();
                    decoder = Some(dec);
                    pending_out.clear();
                    pending_pos = 0;
                    state = if autoplay {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    };
                    last_error = None;
                }
                EngineCommand::QueueNextFile(path) => {
                    if let Some(dec) = &mut decoder
                        && let Err(e) = dec.queue_next_file(&path)
                    {
                        last_error = Some(e.to_string());
                    }
                }
                EngineCommand::QueueNextDecoder {
                    label,
                    decoder: next_dec,
                } => {
                    if let Some(dec) = &mut decoder {
                        dec.queue_next_decoder(label, next_dec);
                    }
                }
                EngineCommand::Play => {
                    if decoder.is_some() {
                        dsp.fade_in();
                        state = PlaybackState::Playing;
                    }
                }
                EngineCommand::Pause => {
                    if state == PlaybackState::Playing {
                        dsp.fade_out();
                        state = PlaybackState::Paused;
                    } else if state == PlaybackState::Paused && decoder.is_some() {
                        dsp.fade_in();
                        state = PlaybackState::Playing;
                    }
                }
                EngineCommand::Stop => {
                    state = PlaybackState::Stopped;
                    pending_out.clear();
                    pending_pos = 0;
                    dsp.reset_state();
                    if let Some(dec) = &mut decoder {
                        let _ = dec.seek(Duration::ZERO);
                    }
                    vis_writer.publish(VisSnapshot::default());
                }
                EngineCommand::Seek(pos) => {
                    if let Some(dec) = &mut decoder {
                        match dec.seek(pos) {
                            Ok(_) => {
                                pending_out.clear();
                                pending_pos = 0;
                                dsp.reset_state();
                                dsp.fade_in();
                            }
                            Err(e) => last_error = Some(e.to_string()),
                        }
                    }
                }
                EngineCommand::SetVolume(v) => {
                    volume = v.clamp(0.0, 1.0);
                    dsp.set_volume(volume);
                }
                EngineCommand::SetBalance(b) => {
                    balance = b.clamp(-1.0, 1.0);
                    dsp.set_balance(balance);
                }
                EngineCommand::SetEq(eq) => {
                    eq_settings = eq.clone();
                    dsp.set_eq(eq);
                }
                EngineCommand::SetReplayGainConfig(cfg) => {
                    dsp.set_replay_gain_config(cfg);
                }
                EngineCommand::SetDither(depth) => {
                    dsp.set_dither_depth(depth);
                }
            }
        }

        // 2. Push any leftover processed samples into the lock-free rtrb ring buffer.
        if pending_pos < pending_out.len() {
            let remaining = pending_out.len() - pending_pos;
            let free = producer.slots() & !1;
            let to_push = remaining.min(free);
            if to_push > 0
                && let Ok(mut chunk) = producer.write_chunk(to_push)
            {
                let (first, second) = chunk.as_mut_slices();
                let n1 = first.len();
                first.copy_from_slice(&pending_out[pending_pos..pending_pos + n1]);
                if !second.is_empty() {
                    second.copy_from_slice(
                        &pending_out[pending_pos + n1..pending_pos + n1 + second.len()],
                    );
                }
                chunk.commit_all();
                pending_pos += to_push;
                if pending_pos >= pending_out.len() {
                    pending_out.clear();
                    pending_pos = 0;
                }
            }
        }

        // 3. Decode & process the next audio block when Playing and ring buffer has room.
        let mut did_work = false;
        if state == PlaybackState::Playing
            && pending_pos >= pending_out.len()
            && producer.slots() >= DECODE_BLOCK_FRAMES * 2
            && let Some(dec) = &mut decoder
        {
            match dec.read_frames(DECODE_BLOCK_FRAMES) {
                Ok(chunk) => {
                    if !chunk.samples.is_empty() {
                        match dsp.process_f32(&chunk.samples) {
                            Ok(processed) => {
                                pending_out = processed;
                                pending_pos = 0;
                                vis_writer.publish(dsp.analyze_vis());
                                did_work = true;
                            }
                            Err(e) => {
                                last_error = Some(e.to_string());
                                state = PlaybackState::Stopped;
                            }
                        }
                    }

                    if let Some(tr) = chunk.transition
                        && tr.format_changed
                    {
                        let _ = dsp.reconfigure_rates(tr.format.sample_rate, output_rate);
                    }

                    if chunk.reached_end && pending_out.is_empty() {
                        state = PlaybackState::Stopped;
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    state = PlaybackState::Stopped;
                }
            }
        }

        // 4. Refresh UI state snapshot.
        if let Ok(mut snap) = snapshot.lock() {
            snap.state = state;
            snap.output_mode = output_mode;
            snap.output_sample_rate = output_rate;
            snap.bit_perfect = dsp.bit_perfect_status();
            snap.volume = volume;
            snap.balance = balance;
            snap.eq = eq_settings.clone();
            snap.frames_rendered = sink.counters.frames_rendered.load(Ordering::Relaxed);
            snap.underruns = sink.counters.underruns.load(Ordering::Relaxed);
            snap.mmcss_active = sink.counters.mmcss_active.load(Ordering::Relaxed);
            snap.last_error = last_error.clone();
            if let Some(dec) = &decoder {
                snap.track_label = dec.current_label().to_string();
                snap.format = Some(dec.current_format().clone());
                snap.position = dec.position();
                snap.duration = dec.current_format().duration;
            }
        }

        if !did_work {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pilimao_decoder::encode_pcm16_wav;
    use std::f32::consts::PI;

    #[test]
    fn streams_synthetic_wav_through_rtrb_and_triple_buffer_to_null_sink() {
        let mut engine = AudioEngine::start(EngineConfig {
            output_mode: OutputMode::Null,
            output_sample_rate: 48_000,
            null_realtime_pacing: false,
        })
        .expect("start null engine");

        let mut vis_reader = engine.take_vis_reader().expect("vis reader");

        let sr = 48_000;
        let total_frames = 4096;
        let mut samples = Vec::with_capacity(total_frames * 2);
        for n in 0..total_frames {
            let v = 0.8 * (2.0 * PI * 1000.0 * n as f32 / sr as f32).sin();
            samples.push(v);
            samples.push(v);
        }
        let wav = encode_pcm16_wav(sr, &samples);
        let dec = SymphoniaDecoder::open_bytes(wav, Some("wav")).expect("wav");

        engine.load_decoder("test_1khz.wav", dec, true, ReplayGainInfo::default());

        let mut peak_seen = 0.0_f32;
        for _ in 0..200 {
            let vis = vis_reader.latest();
            for &b in &vis.bands {
                peak_seen = peak_seen.max(b);
            }
            let snap = engine.snapshot();
            if snap.frames_rendered >= total_frames as u64 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        let snap = engine.snapshot();
        assert!(
            snap.frames_rendered >= total_frames as u64,
            "expected >= {total_frames} rendered frames, got {}",
            snap.frames_rendered
        );
        assert!(
            peak_seen > 0.4,
            "expected triple_buffer FFT spectrum activity, peak={peak_seen}"
        );
    }
}
