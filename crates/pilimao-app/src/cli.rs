//! CLI inspection tools: `inspect-skin`, `inspect-eqf`, and `inspect-track`.

use std::path::Path;

use anyhow::Context;
use pilimao_decoder::SymphoniaDecoder;
use pilimao_dsp::{
    BitPerfectQuery, DitherDepth, EQ_BAND_LABELS, EqSettings, ReplayGainConfig, ReplayGainInfo,
    evaluate_bit_perfect,
};
use pilimao_metadata::{LegacyEncodingHint, read_track_metadata};
use pilimao_playlist::load_eqf_file;
use pilimao_skin_wsz::{WszSkin, load_skin};

/// Inspects a `.wsz` skin archive, skin directory, or `"default"` Clean-Room skin.
pub fn inspect_skin(target: &str) -> anyhow::Result<()> {
    let skin = if target.eq_ignore_ascii_case("default") {
        WszSkin::clean_room_default()
    } else {
        load_skin(target).with_context(|| format!("failed to load skin from '{target}'"))?
    };

    println!("Skin Name      : {}", skin.name);
    println!(
        "Main Sprite    : {}x{}",
        skin.sprites.main.width, skin.sprites.main.height
    );
    println!(
        "EQ Sprite      : {}x{}",
        skin.sprites.eqmain.width, skin.sprites.eqmain.height
    );
    println!(
        "PL Sprite      : {}x{}",
        skin.sprites.pledit.width, skin.sprites.pledit.height
    );
    println!(
        "Regions        : normal={}, shade={}, eq={}, eq_ws={}",
        skin.regions.normal.len(),
        skin.regions.window_shade.len(),
        skin.regions.equalizer.len(),
        skin.regions.equalizer_ws.len()
    );
    println!(
        "VisColor BG    : rgb({},{},{})  Peak: rgb({},{},{})",
        skin.vis_colors.background().r,
        skin.vis_colors.background().g,
        skin.vis_colors.background().b,
        skin.vis_colors.peak_dot().r,
        skin.vis_colors.peak_dot().g,
        skin.vis_colors.peak_dot().b
    );
    println!(
        "Pledit Style   : font='{}' normal=#{:02X}{:02X}{:02X} current=#{:02X}{:02X}{:02X}",
        skin.pledit_style.font,
        skin.pledit_style.normal.r,
        skin.pledit_style.normal.g,
        skin.pledit_style.normal.b,
        skin.pledit_style.current.r,
        skin.pledit_style.current.g,
        skin.pledit_style.current.b,
    );
    println!("Loaded Assets  : {} files", skin.report.loaded_files.len());
    if !skin.report.fallback_assets.is_empty() {
        println!(
            "Fallbacks Used : {}",
            skin.report.fallback_assets.join(", ")
        );
    }
    for warn in &skin.report.warnings {
        println!("Warning        : {warn}");
    }
    Ok(())
}

/// Inspects a Winamp binary `.eqf` / `winamp.q1` equalizer preset file.
pub fn inspect_eqf(path: &Path) -> anyhow::Result<()> {
    let presets = load_eqf_file(path)
        .with_context(|| format!("failed to parse Winamp .eqf file '{}'", path.display()))?;

    println!("EQF File       : {}", path.display());
    println!("Preset Count   : {}", presets.len());

    for (i, p) in presets.iter().enumerate() {
        println!("\n[{}] {} (Preamp: {:+.2} dB)", i + 1, p.name, p.preamp_db);
        for (band, &db) in p.bands_db.iter().enumerate() {
            let norm = ((db + 12.0) / 24.0 * 20.0).round().clamp(0.0, 20.0) as usize;
            let bar: String = (0..20)
                .map(|c| {
                    if c == 10 {
                        '|'
                    } else if c == norm {
                        '#'
                    } else {
                        '-'
                    }
                })
                .collect();
            println!(
                "  {:>4} Hz : {:+6.2} dB  [{}]",
                EQ_BAND_LABELS[band], db, bar
            );
        }
    }
    Ok(())
}

/// Inspects an audio track's tags, mojibake restorations, ReplayGain 2.0, and codec properties.
pub fn inspect_track(path: &Path, hint: LegacyEncodingHint) -> anyhow::Result<()> {
    println!("Track Path     : {}", path.display());

    match read_track_metadata(path, hint) {
        Ok(meta) => {
            println!(
                "Title          : {}",
                meta.title.as_deref().unwrap_or("<none>")
            );
            println!(
                "Artist         : {}",
                meta.artist.as_deref().unwrap_or("<none>")
            );
            println!(
                "Album          : {}",
                meta.album.as_deref().unwrap_or("<none>")
            );
            if let Some(year) = meta.year {
                println!("Year           : {year}");
            }
            if let Some(trk) = meta.track_number {
                println!("Track #        : {trk}");
            }
            if !meta.restorations.is_empty() {
                println!("Mojibake Fixed :");
                for r in &meta.restorations {
                    println!(
                        "  - {} ({}): '{}' -> '{}'",
                        r.field, r.encoding, r.original, r.restored
                    );
                }
            }
            if meta.replay_gain.has_any() {
                println!(
                    "ReplayGain 2.0 : track_gain={:?} dB, track_peak={:?}, album_gain={:?} dB, album_peak={:?}",
                    meta.replay_gain.track_gain_db,
                    meta.replay_gain.track_peak,
                    meta.replay_gain.album_gain_db,
                    meta.replay_gain.album_peak
                );
            } else {
                println!("ReplayGain 2.0 : <none>");
            }
        }
        Err(err) => {
            println!("Metadata Note  : {err}");
        }
    }

    let dec = SymphoniaDecoder::open_file(path)
        .with_context(|| format!("failed to open '{}' with symphonia", path.display()))?;
    let fmt = dec.format();
    println!("Codec          : {}", fmt.codec_name);
    println!("Sample Rate    : {} Hz", fmt.sample_rate);
    println!("Channels       : {}", fmt.channels);
    println!(
        "Bit Depth      : {}",
        fmt.bits_per_sample
            .map(|b| format!("{b}-bit"))
            .unwrap_or_else(|| "float/variable".to_string())
    );
    if let Some(kbps) = fmt.bitrate_kbps {
        println!("Bitrate        : {kbps} kbps");
    }
    if let Some(dur) = fmt.duration {
        println!("Duration       : {:.2} s", dur.as_secs_f64());
    }

    // Check theoretical Bit-Perfect eligibility at matching WASAPI Exclusive rate
    let eq = EqSettings::flat();
    let rg_cfg = ReplayGainConfig::default();
    let rg_info = ReplayGainInfo::default();
    let bp = evaluate_bit_perfect(&BitPerfectQuery {
        exclusive_mode: true,
        input_sample_rate: fmt.sample_rate,
        output_sample_rate: fmt.sample_rate,
        input_channels: fmt.channels,
        output_channels: fmt.channels,
        volume: 1.0,
        balance: 0.0,
        eq: &eq,
        replay_gain_config: &rg_cfg,
        replay_gain_info: &rg_info,
        ramp_is_unity: true,
        dither_depth: DitherDepth::None,
    });
    println!(
        "Bit-Perfect    : {}",
        if bp.is_bit_perfect {
            "Eligible (with WASAPI Exclusive @ native rate, flat EQ, 100% vol)"
        } else {
            "Unavailable"
        }
    );

    Ok(())
}
