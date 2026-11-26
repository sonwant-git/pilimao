//! Windows WASAPI Shared & Exclusive (Bit-Perfect) + MMCSS `"Pro Audio"` output backend.

use rtrb::Consumer;

use crate::EngineError;
use crate::sink::{ActiveSink, AudioDeviceInfo, OutputMode};

/// Lists available audio output devices on the current platform.
#[must_use]
pub fn enumerate_output_devices() -> Vec<AudioDeviceInfo> {
    #[cfg(windows)]
    {
        win_impl::enumerate_output_devices().unwrap_or_else(|_| vec![null_device()])
    }
    #[cfg(not(windows))]
    {
        vec![null_device()]
    }
}

fn null_device() -> AudioDeviceInfo {
    AudioDeviceInfo {
        id: "null".to_string(),
        name: "Null Audio Sink (Cross-Platform)".to_string(),
        is_default: true,
    }
}

/// Starts a Windows WASAPI Shared or Exclusive output sink reading lock-free from `consumer`.
/// On non-Windows targets, returns [`EngineError::WasapiUnavailable`].
pub fn spawn_wasapi_sink(
    consumer: Consumer<f32>,
    mode: OutputMode,
    requested_sample_rate: u32,
) -> Result<ActiveSink, EngineError> {
    #[cfg(windows)]
    {
        win_impl::spawn_wasapi_sink(consumer, mode, requested_sample_rate)
    }
    #[cfg(not(windows))]
    {
        let _ = (consumer, mode, requested_sample_rate);
        Err(EngineError::WasapiUnavailable)
    }
}

#[cfg(windows)]
mod win_impl {
    use std::ptr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use rtrb::Consumer;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::Media::Audio::{
        AUDCLNT_SHAREMODE_EXCLUSIVE, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
        DEVICE_STATE_ACTIVE, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0, eConsole,
        eRender,
    };
    use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
    use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        CoUninitialize,
    };
    use windows::Win32::System::Threading::{
        AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW,
        WaitForSingleObject,
    };
    use windows::core::w;

    use crate::EngineError;
    use crate::sink::{ActiveSink, AudioDeviceInfo, OutputMode, SinkCounters};

    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    struct MmcssGuard(HANDLE);
    impl Drop for MmcssGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = AvRevertMmThreadCharacteristics(self.0);
                }
            }
        }
    }

    struct EventGuard(HANDLE);
    impl Drop for EventGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    pub fn enumerate_output_devices() -> Result<Vec<AudioDeviceInfo>, EngineError> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _com_guard = if hr.is_ok() { Some(ComGuard) } else { None };

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| EngineError::Wasapi(e.to_string()))?;

            let default_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .ok()
                .and_then(|dev| dev.GetId().ok())
                .and_then(|pw| pw.to_string().ok());

            let collection = enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
                .map_err(|e| EngineError::Wasapi(e.to_string()))?;
            let count = collection
                .GetCount()
                .map_err(|e| EngineError::Wasapi(e.to_string()))?;

            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count {
                if let Ok(dev) = collection.Item(i)
                    && let Ok(pw_id) = dev.GetId()
                    && let Ok(id_str) = pw_id.to_string()
                {
                    let is_default = default_id.as_deref() == Some(id_str.as_str());
                    out.push(AudioDeviceInfo {
                        id: id_str.clone(),
                        name: if is_default {
                            format!("WASAPI Default Endpoint ({id_str})")
                        } else {
                            format!("WASAPI Endpoint #{i} ({id_str})")
                        },
                        is_default,
                    });
                }
            }
            Ok(out)
        }
    }

    pub fn spawn_wasapi_sink(
        mut consumer: Consumer<f32>,
        mode: OutputMode,
        requested_sample_rate: u32,
    ) -> Result<ActiveSink, EngineError> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(SinkCounters::default());
        let stop_c = Arc::clone(&stop_flag);
        let cnt_c = Arc::clone(&counters);

        let (init_tx, init_rx) = mpsc::channel::<Result<u32, String>>();

        let handle = thread::Builder::new()
            .name("pilimao-wasapi-render".to_string())
            .spawn(move || {
                if let Err(err) = run_wasapi_loop(
                    &mut consumer,
                    mode,
                    requested_sample_rate,
                    &stop_c,
                    &cnt_c,
                    &init_tx,
                ) {
                    let _ = init_tx.send(Err(err));
                }
            })
            .map_err(|e| EngineError::Wasapi(e.to_string()))?;

        let negotiated_rate = init_rx
            .recv()
            .map_err(|e| EngineError::Wasapi(format!("WASAPI thread init failed: {e}")))?
            .map_err(EngineError::Wasapi)?;

        Ok(ActiveSink::from_thread(
            mode,
            negotiated_rate,
            counters,
            stop_flag,
            handle,
        ))
    }

    fn build_ieee_float_format(sample_rate: u32) -> WAVEFORMATEXTENSIBLE {
        let channels: u16 = 2;
        let bits_per_sample: u16 = 32;
        let block_align = channels * (bits_per_sample / 8);
        let avg_bytes_per_sec = sample_rate * u32::from(block_align);

        WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: channels,
                nSamplesPerSec: sample_rate,
                nAvgBytesPerSec: avg_bytes_per_sec,
                nBlockAlign: block_align,
                wBitsPerSample: bits_per_sample,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: bits_per_sample,
            },
            dwChannelMask: 0x3, // SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        }
    }

    fn run_wasapi_loop(
        consumer: &mut Consumer<f32>,
        mode: OutputMode,
        requested_sample_rate: u32,
        stop_flag: &AtomicBool,
        counters: &SinkCounters,
        init_tx: &mpsc::Sender<Result<u32, String>>,
    ) -> Result<(), String> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_err() {
                return Err(format!("CoInitializeEx failed: {hr:?}"));
            }
            let _com_guard = ComGuard;

            // Register render thread with MMCSS "Pro Audio" for glitch-free low-latency scheduling.
            let mut task_index: u32 = 0;
            let _mmcss_guard = match AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task_index)
            {
                Ok(h) => {
                    counters.mmcss_active.store(true, Ordering::Release);
                    Some(MmcssGuard(h))
                }
                Err(_) => {
                    counters.mmcss_active.store(false, Ordering::Release);
                    None
                }
            };

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("MMDeviceEnumerator: {e}"))?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| format!("GetDefaultAudioEndpoint: {e}"))?;
            let audio_client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Activate IAudioClient: {e}"))?;

            let mut default_period = 0i64;
            let mut min_period = 0i64;
            audio_client
                .GetDevicePeriod(Some(&mut default_period), Some(&mut min_period))
                .map_err(|e| format!("GetDevicePeriod: {e}"))?;

            let (actual_rate, share_mode, period) = match mode {
                OutputMode::WasapiExclusive => {
                    let desired = build_ieee_float_format(requested_sample_rate.max(44_100));
                    let desired_ptr = ptr::addr_of!(desired) as *const WAVEFORMATEX;
                    let support = audio_client.IsFormatSupported(
                        AUDCLNT_SHAREMODE_EXCLUSIVE,
                        desired_ptr,
                        None,
                    );
                    if support.is_err() {
                        return Err(format!(
                            "WASAPI Exclusive does not support {} Hz 32-bit float stereo ({support:?})",
                            requested_sample_rate
                        ));
                    }
                    audio_client
                        .Initialize(
                            AUDCLNT_SHAREMODE_EXCLUSIVE,
                            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                            min_period,
                            min_period,
                            desired_ptr,
                            None,
                        )
                        .map_err(|e| format!("IAudioClient::Initialize (Exclusive): {e}"))?;
                    (
                        desired.Format.nSamplesPerSec,
                        AUDCLNT_SHAREMODE_EXCLUSIVE,
                        min_period,
                    )
                }
                _ => {
                    let mix_fmt = audio_client
                        .GetMixFormat()
                        .map_err(|e| format!("GetMixFormat: {e}"))?;
                    let mix_rate = (*mix_fmt).nSamplesPerSec;
                    let init_res = audio_client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                        default_period,
                        0,
                        mix_fmt,
                        None,
                    );
                    CoTaskMemFree(Some(mix_fmt.cast()));
                    init_res.map_err(|e| format!("IAudioClient::Initialize (Shared): {e}"))?;
                    (mix_rate, AUDCLNT_SHAREMODE_SHARED, default_period)
                }
            };

            let _ = period;

            let event =
                CreateEventW(None, false, false, None).map_err(|e| format!("CreateEventW: {e}"))?;
            let _event_guard = EventGuard(event);

            audio_client
                .SetEventHandle(event)
                .map_err(|e| format!("SetEventHandle: {e}"))?;

            let buffer_frames = audio_client
                .GetBufferSize()
                .map_err(|e| format!("GetBufferSize: {e}"))?;

            let render_client: IAudioRenderClient = audio_client
                .GetService()
                .map_err(|e| format!("GetService IAudioRenderClient: {e}"))?;

            // Prime initial silence buffer before starting stream.
            let prime_ptr = render_client
                .GetBuffer(buffer_frames)
                .map_err(|e| format!("Initial GetBuffer: {e}"))?;
            ptr::write_bytes(prime_ptr, 0, (buffer_frames as usize) * 2 * 4);
            render_client
                .ReleaseBuffer(buffer_frames, 0)
                .map_err(|e| format!("Initial ReleaseBuffer: {e}"))?;

            audio_client
                .Start()
                .map_err(|e| format!("IAudioClient::Start: {e}"))?;

            let _ = init_tx.send(Ok(actual_rate));

            while !stop_flag.load(Ordering::Acquire) {
                let wait = WaitForSingleObject(event, 100);
                if wait != WAIT_OBJECT_0 {
                    continue;
                }

                let frames_to_write = if share_mode == AUDCLNT_SHAREMODE_EXCLUSIVE {
                    buffer_frames
                } else {
                    let padding = audio_client.GetCurrentPadding().unwrap_or(0);
                    buffer_frames.saturating_sub(padding)
                };

                if frames_to_write == 0 {
                    continue;
                }

                let Ok(buf_ptr) = render_client.GetBuffer(frames_to_write) else {
                    continue;
                };
                let dst = std::slice::from_raw_parts_mut(
                    buf_ptr.cast::<f32>(),
                    (frames_to_write as usize) * 2,
                );

                let needed_samples = dst.len();
                let avail = consumer.slots();
                let copy_samples = avail.min(needed_samples) & !1;

                if copy_samples > 0
                    && let Ok(chunk) = consumer.read_chunk(copy_samples)
                {
                    let (first, second) = chunk.as_slices();
                    let n1 = first.len();
                    dst[..n1].copy_from_slice(first);
                    if !second.is_empty() {
                        dst[n1..n1 + second.len()].copy_from_slice(second);
                    }
                    chunk.commit_all();
                }

                if copy_samples < needed_samples {
                    dst[copy_samples..].fill(0.0);
                    if copy_samples == 0 {
                        counters.underruns.fetch_add(1, Ordering::Relaxed);
                    }
                }

                let _ = render_client.ReleaseBuffer(frames_to_write, 0);
                counters
                    .frames_rendered
                    .fetch_add(u64::from(frames_to_write), Ordering::Relaxed);
            }

            let _ = audio_client.Stop();
            Ok(())
        }
    }
}
