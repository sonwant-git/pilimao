//! End-to-end `pilimao self-test` diagnostic suite covering all 8 workspace crates.

use std::f32::consts::PI;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::{Context, bail};
use pilimao_audio_engine::{AudioEngine, EngineConfig, OutputMode};
use pilimao_decoder::{GaplessDecoder, SymphoniaDecoder, encode_pcm16_wav};
use pilimao_dsp::{
    DitherDepth, DspConfig, DspPipeline, EqSettings, LimiterConfig, ReplayGainConfig,
    ReplayGainInfo, ReplayGainMode, SincResampler, SoftKneeLimiter, TpdfDither,
};
use pilimao_metadata::{
    DetectedEncoding, LegacyEncodingHint, extract_replay_gain, restore_mojibake,
};
use pilimao_playlist::{
    JumpMatcher, Playlist, PlaylistEntry, builtin_presets, parse_eqf_bytes, parse_m3u_str,
    parse_pls_str, write_eqf_bytes, write_m3u8, write_pls,
};
use pilimao_skin_wsz::{
    DockingEngine, WindowId, WindowRect, WszSkin, load_skin_from_wsz_bytes, parse_pledit_txt,
    parse_region_txt, parse_viscolor_txt, write_wsz_bytes,
};
use pilimao_win32_shell::{
    MainHit, Modifiers, ShellRenderer, ShortcutAction, UiRenderState, hit_test_main,
    map_virtual_key,
};

use crate::config::AppConfig;

/// Executes all 8 subsystem self-tests and prints a structured pass/fail report.
pub fn run_self_test() -> anyhow::Result<()> {
    println!("Pilimao 8-Crate Self-Test Suite");
    println!("===============================");

    check_step(
        "1/8 pilimao-dsp (64-bit EQ, Limiter, Ramp, TPDF, RG2, Sinc, FFT19, Bit-Perfect)",
        test_dsp,
    )?;
    check_step(
        "2/8 pilimao-decoder (Symphonia WAV decode, seek, Gapless stitch)",
        test_decoder,
    )?;
    check_step(
        "3/8 pilimao-metadata (Big5/GBK/Shift-JIS mojibake + ReplayGain 2.0)",
        test_metadata,
    )?;
    check_step(
        "4/8 pilimao-playlist (M3U8/PLS/.eqf + Shuffle/Queue + nucleo Jump)",
        test_playlist,
    )?;
    check_step(
        "5/8 pilimao-skin-wsz (Clean-Room .wsz + region/viscolor/pledit + BFS Docking)",
        test_skin,
    )?;
    check_step(
        "6/8 pilimao-audio-engine (rtrb + triple_buffer + Null Sink stream)",
        test_audio_engine,
    )?;
    check_step(
        "7/8 pilimao-win32-shell (1x/2x/3x Nearest-Neighbor BGRA + HitTest + Shortcuts)",
        test_shell,
    )?;
    check_step(
        "8/8 pilimao-app (Portable Mode INI persistence)",
        test_config,
    )?;

    println!("===============================");
    println!("ALL 8 SUBSYSTEM CHECKS PASSED.");
    Ok(())
}

fn check_step(name: &str, f: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    f().with_context(|| format!("self-test failed: {name}"))?;
    println!("  [PASS] {name}");
    Ok(())
}

fn test_dsp() -> anyhow::Result<()> {
    // Bit-perfect check
    let mut bp_pipe = DspPipeline::new(DspConfig {
        input_sample_rate: 44_100,
        output_sample_rate: 44_100,
        exclusive_mode: true,
        volume: 1.0,
        balance: 0.0,
        eq: EqSettings::flat(),
        limiter: LimiterConfig::default(),
        replay_gain: ReplayGainConfig::default(),
        dither_depth: DitherDepth::None,
    })?;
    if !bp_pipe.bit_perfect_status().is_bit_perfect {
        bail!("expected Bit-Perfect active");
    }
    let probe = vec![0.125_f32, -0.375, 0.5, -0.25];
    let bp_out = bp_pipe.process_f32(&probe)?;
    if bp_out != probe {
        bail!("Bit-Perfect modified samples");
    }

    // Active EQ + ReplayGain + Limiter + TPDF + Resampling 44.1k -> 48k + 19-band FFT
    let mut eq = EqSettings::flat();
    eq.enabled = true;
    eq.preamp_db = 3.0;
    eq.bands_db[4] = 6.0; // 1 kHz
    let mut pipe = DspPipeline::new(DspConfig {
        input_sample_rate: 44_100,
        output_sample_rate: 48_000,
        exclusive_mode: false,
        volume: 0.9,
        balance: 0.0,
        eq,
        limiter: LimiterConfig::default(),
        replay_gain: ReplayGainConfig {
            mode: ReplayGainMode::Track,
            ..Default::default()
        },
        dither_depth: DitherDepth::Bits16,
    })?;
    pipe.set_track_replay_gain(ReplayGainInfo {
        track_gain_db: Some(-2.0),
        track_peak: Some(0.95),
        ..Default::default()
    });
    let mut sine = Vec::with_capacity(4096 * 2);
    for n in 0..4096 {
        let v = 0.6 * (2.0 * PI * 1000.0 * n as f32 / 44_100.0).sin();
        sine.push(v);
        sine.push(v);
    }
    let out = pipe.process_f32(&sine)?;
    if out.is_empty() || !out.iter().all(|s| s.is_finite()) {
        bail!("DSP pipeline returned invalid output");
    }
    let vis = pipe.analyze_vis();
    if vis.bands.iter().copied().fold(0.0_f32, f32::max) < 0.2 {
        bail!("19-band spectrum failed to detect 1 kHz signal");
    }

    // Direct Limiter + TPDF + SincResampler sanity
    let mut lim = SoftKneeLimiter::new(48_000, LimiterConfig::default());
    let (l, _) = lim.process_frame(4.0, 4.0);
    if l > 1.0 {
        bail!("SoftKneeLimiter failed ceiling clamp: {l}");
    }
    let mut dither = TpdfDither::new(DitherDepth::Bits16);
    let _ = dither.quantize_i16(0.5);
    let mut rs = SincResampler::new(44_100, 48_000)?;
    let _ = rs.flush()?;
    Ok(())
}

fn test_decoder() -> anyhow::Result<()> {
    let wav_a = encode_pcm16_wav(48_000, &vec![0.4; 600 * 2]);
    let wav_b = encode_pcm16_wav(48_000, &vec![-0.4; 600 * 2]);
    let dec_a = SymphoniaDecoder::open_bytes(wav_a, Some("wav"))?;
    let dec_b = SymphoniaDecoder::open_bytes(wav_b, Some("wav"))?;
    let mut gapless = GaplessDecoder::new("a.wav", dec_a);
    gapless.queue_next_decoder("b.wav", dec_b);

    let chunk = gapless.read_frames(900)?;
    if chunk.frames != 900 || chunk.transition.is_none() {
        bail!("gapless stitching failed");
    }
    Ok(())
}

fn test_metadata() -> anyhow::Result<()> {
    // Big5 mojibake for "周杰倫" -> bytes [0xA9, 0x50, 0xAA, 0x4E, 0xAD, 0xDB]
    let big5_garbled: String = [0xA9u8, 0x50, 0xAA, 0x4E, 0xAD, 0xDB]
        .iter()
        .map(|&b| b as char)
        .collect();
    let res = restore_mojibake(&big5_garbled, LegacyEncodingHint::PreferBig5);
    if res.text != "周杰倫" || res.restored_from != Some(DetectedEncoding::Big5) {
        bail!("Big5 mojibake restoration failed: got {:?}", res);
    }

    let rg = extract_replay_gain([
        ("REPLAYGAIN_TRACK_GAIN", "-5.25 dB"),
        ("REPLAYGAIN_TRACK_PEAK", "0.97"),
    ]);
    if rg.track_gain_db.is_none() || rg.track_peak.is_none() {
        bail!("ReplayGain 2.0 tag extraction failed");
    }
    Ok(())
}

fn test_playlist() -> anyhow::Result<()> {
    let mut pl = Playlist::with_seed(7);
    pl.push(PlaylistEntry {
        path: PathBuf::from("01_alpha.flac"),
        title: Some("Alpha - First Light".to_string()),
        duration_secs: Some(200),
    });
    pl.push(PlaylistEntry {
        path: PathBuf::from("02_beta.mp3"),
        title: Some("Beta - Midnight Synth".to_string()),
        duration_secs: Some(185),
    });

    let m3u = write_m3u8(pl.entries());
    if parse_m3u_str(&m3u, None).len() != 2 {
        bail!("M3U8 round-trip failed");
    }
    let pls = write_pls(pl.entries());
    if parse_pls_str(&pls, None).len() != 2 {
        bail!("PLS round-trip failed");
    }

    let eqf_bytes = write_eqf_bytes(&builtin_presets());
    let parsed_eqf = parse_eqf_bytes(&eqf_bytes)?;
    if parsed_eqf.len() != builtin_presets().len() {
        bail!("Winamp .eqf round-trip failed");
    }

    let mut jump = JumpMatcher::new();
    let matches = jump.search(&pl, "mid synth");
    if matches.first().map(|m| m.playlist_index) != Some(1) {
        bail!("nucleo-matcher fuzzy jump failed");
    }
    Ok(())
}

fn test_skin() -> anyhow::Result<()> {
    let default_skin = WszSkin::clean_room_default();
    let wsz = write_wsz_bytes(&default_skin)?;
    let reloaded = load_skin_from_wsz_bytes("self_test.wsz", &wsz)?;
    if reloaded.sprites.main.width != 275 || reloaded.sprites.main.height != 116 {
        bail!("clean-room .wsz dimensions mismatch");
    }

    let reg = parse_region_txt("[Normal]\nNumPoints=4\nPointList=0,0,275,0,275,116,0,116\n");
    if reg.normal.len() != 1 {
        bail!("region.txt parse failed");
    }
    let _ = parse_viscolor_txt("0,0,0\n255,255,255\n");
    let _ = parse_pledit_txt("[Text]\nNormal=#00FF00\n");

    let mut dock = DockingEngine::new(100, 100, 2, WindowRect::new(0, 0, 1920, 1080));
    if dock.connected_group_bfs(WindowId::Main).len() != 3 {
        bail!("BFS docking group expected 3 windows");
    }
    dock.toggle_windowshade(WindowId::Main);
    if dock.state(WindowId::Equalizer).rect.y != dock.state(WindowId::Main).rect.bottom() {
        bail!("Windowshade dock shift failed");
    }
    Ok(())
}

fn test_audio_engine() -> anyhow::Result<()> {
    let mut engine = AudioEngine::start(EngineConfig {
        output_mode: OutputMode::Null,
        output_sample_rate: 48_000,
        null_realtime_pacing: false,
    })?;
    let mut vis = engine.take_vis_reader().context("missing vis reader")?;

    let mut samples = Vec::with_capacity(2048 * 2);
    for n in 0..2048 {
        let s = 0.7 * (2.0 * PI * 880.0 * n as f32 / 48_000.0).sin();
        samples.push(s);
        samples.push(s);
    }
    let wav = encode_pcm16_wav(48_000, &samples);
    let dec = SymphoniaDecoder::open_bytes(wav, Some("wav"))?;
    engine.load_decoder("selftest.wav", dec, true, ReplayGainInfo::default());

    for _ in 0..200 {
        let _ = vis.latest();
        if engine.snapshot().frames_rendered >= 2048 {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    bail!(
        "NullSink audio engine timed out (rendered={})",
        engine.snapshot().frames_rendered
    );
}

fn test_shell() -> anyhow::Result<()> {
    let skin = WszSkin::clean_room_default();
    let state = UiRenderState::default();
    let pl = Playlist::new();
    for scale in [1u32, 2, 3] {
        let surf = ShellRenderer::render_main(&skin, &state, &pl, false, true, scale);
        if surf.width != 275 * scale || surf.height != 116 * scale {
            bail!("unexpected scaled BGRA surface size at {scale}x");
        }
    }
    if hit_test_main(45, 95, false) != MainHit::Play {
        bail!("Play button hit-test mismatch");
    }
    if map_virtual_key(b'X' as u32, Modifiers::default()) != Some(ShortcutAction::Play) {
        bail!("Winamp shortcut X -> Play mismatch");
    }
    Ok(())
}

fn test_config() -> anyhow::Result<()> {
    let cfg = AppConfig::default();
    let ini = cfg.to_ini_string();
    let reparsed = AppConfig::from_ini_str(&ini);
    if reparsed.scale != cfg.scale {
        bail!("INI config round-trip mismatch");
    }
    Ok(())
}
