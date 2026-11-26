//! Output sink abstractions, device descriptors, and cross-platform Null Sink.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rtrb::Consumer;

/// Selected audio output backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Windows WASAPI Shared mode (compatible with system mixer).
    #[default]
    WasapiShared,
    /// Windows WASAPI Exclusive mode (hardware direct, enables Bit-Perfect pass-through).
    WasapiExclusive,
    /// Cross-platform silent Null Sink (for Linux/macOS, CI, and headless self-tests).
    Null,
}

/// Description of an available audio output device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Atomic telemetry shared between an active output sink thread and the engine.
#[derive(Debug, Default)]
pub struct SinkCounters {
    pub frames_rendered: AtomicU64,
    pub underruns: AtomicU64,
    pub mmcss_active: AtomicBool,
}

/// Running output sink handle (WASAPI or Null).
pub struct ActiveSink {
    pub mode: OutputMode,
    pub sample_rate: u32,
    pub counters: Arc<SinkCounters>,
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl ActiveSink {
    /// Spawns a cross-platform Null Sink draining `consumer` cleanly.
    /// When `realtime_pacing` is false, drains rapidly (ideal for automated tests).
    #[must_use]
    pub fn spawn_null(
        mut consumer: Consumer<f32>,
        sample_rate: u32,
        realtime_pacing: bool,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(SinkCounters::default());
        let stop_c = Arc::clone(&stop_flag);
        let cnt_c = Arc::clone(&counters);

        let join_handle = thread::Builder::new()
            .name("pilimao-null-sink".to_string())
            .spawn(move || {
                let block_frames = (sample_rate / 100).max(128) as usize; // ~10ms blocks
                let block_samples = block_frames * 2;
                let sleep_dur = if realtime_pacing {
                    Duration::from_millis(10)
                } else {
                    Duration::from_millis(1)
                };

                while !stop_c.load(Ordering::Relaxed) {
                    let avail = consumer.slots();
                    if avail >= 2 {
                        let take_samples = avail.min(block_samples) & !1;
                        if let Ok(chunk) = consumer.read_chunk(take_samples) {
                            let frames = (chunk.len() / 2) as u64;
                            chunk.commit_all();
                            cnt_c.frames_rendered.fetch_add(frames, Ordering::Relaxed);
                        }
                    } else {
                        thread::sleep(sleep_dur);
                    }
                }
            })
            .ok();

        Self {
            mode: OutputMode::Null,
            sample_rate,
            counters,
            stop_flag,
            join_handle,
        }
    }

    /// Creates an [`ActiveSink`] handle from an externally spawned backend thread (e.g., WASAPI).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn from_thread(
        mode: OutputMode,
        sample_rate: u32,
        counters: Arc<SinkCounters>,
        stop_flag: Arc<AtomicBool>,
        join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            mode,
            sample_rate,
            counters,
            stop_flag,
            join_handle: Some(join_handle),
        }
    }

    /// Stops and joins the sink thread cleanly.
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ActiveSink {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}
