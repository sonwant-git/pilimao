//! Winamp 2.x binary `.eqf` / `winamp.q1` Equalizer preset file reader and writer.
//!
//! Binary layout:
//! - 31-byte header: `"Winamp EQ library file v1.1\x1a!--"`
//! - Followed by `N >= 1` records of 268 bytes each:
//!   - 257 bytes: null-terminated preset name (zero-padded)
//!   - 10 bytes: 10 EQ band levels (`0..=63`, where `0 = +12 dB`, `31 = 0 dB`, `63 = -12 dB`)
//!   - 1 byte: preamp level (`0..=63`, same inverted scale)

use std::fs;
use std::path::Path;

use crate::PlaylistError;

/// Exact 31-byte Winamp EQ library file v1.1 magic header.
pub const EQF_HEADER: &[u8; 31] = b"Winamp EQ library file v1.1\x1a!--";

/// Size of a single preset record inside an `.eqf` / `winamp.q1` file.
pub const EQF_RECORD_SIZE: usize = 268;

/// Size of the fixed name field inside each record.
pub const EQF_NAME_LEN: usize = 257;

/// Single Winamp equalizer preset (name, preamp dB, and 10 classic band gains in dB).
#[derive(Debug, Clone, PartialEq)]
pub struct EqfPreset {
    pub name: String,
    pub preamp_db: f64,
    pub bands_db: [f64; 10],
}

impl EqfPreset {
    /// Creates a flat (0 dB) preset with the given name.
    #[must_use]
    pub fn flat(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            preamp_db: 0.0,
            bands_db: [0.0; 10],
        }
    }
}

/// Converts a Winamp `.eqf` raw slider byte (`0..=63`) to dB (`+12.0` .. `-12.0`).
/// Byte `0` = `+12.0 dB`, `31` (`0x1F`) = exact `0.0 dB`, `63` (`0x3F`) = `-12.0 dB`.
#[must_use]
pub fn eqf_byte_to_db(raw: u8) -> f64 {
    let clamped = raw.min(63);
    if clamped <= 31 {
        f64::from(31 - clamped) / 31.0 * 12.0
    } else {
        -f64::from(clamped - 31) / 32.0 * 12.0
    }
}

/// Converts a gain in dB (`-12.0..=+12.0`) to a Winamp `.eqf` raw slider byte (`0..=63`).
#[must_use]
pub fn db_to_eqf_byte(db: f64) -> u8 {
    let clamped = db.clamp(-12.0, 12.0);
    if clamped >= 0.0 {
        (31.0 - (clamped / 12.0) * 31.0).round().clamp(0.0, 31.0) as u8
    } else {
        (31.0 + (-clamped / 12.0) * 32.0).round().clamp(31.0, 63.0) as u8
    }
}

/// Parses one or more Winamp presets from raw `.eqf` / `winamp.q1` binary data.
pub fn parse_eqf_bytes(bytes: &[u8]) -> Result<Vec<EqfPreset>, PlaylistError> {
    if bytes.len() < EQF_HEADER.len() + EQF_RECORD_SIZE {
        return Err(PlaylistError::InvalidEqf(format!(
            "file too short: {} bytes (minimum {})",
            bytes.len(),
            EQF_HEADER.len() + EQF_RECORD_SIZE
        )));
    }
    if &bytes[..EQF_HEADER.len()] != EQF_HEADER {
        return Err(PlaylistError::InvalidEqf(
            "invalid Winamp EQ library v1.1 header".to_string(),
        ));
    }

    let payload = &bytes[EQF_HEADER.len()..];
    let record_count = payload.len() / EQF_RECORD_SIZE;
    let mut presets = Vec::with_capacity(record_count);

    for i in 0..record_count {
        let rec = &payload[i * EQF_RECORD_SIZE..(i + 1) * EQF_RECORD_SIZE];
        let name_field = &rec[..EQF_NAME_LEN];
        let nul_pos = name_field
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(EQF_NAME_LEN);
        let name = String::from_utf8_lossy(&name_field[..nul_pos])
            .trim()
            .to_string();

        let mut bands_db = [0.0_f64; 10];
        for (b, slot) in bands_db.iter_mut().enumerate() {
            *slot = eqf_byte_to_db(rec[EQF_NAME_LEN + b]);
        }
        let preamp_db = eqf_byte_to_db(rec[EQF_NAME_LEN + 10]);

        presets.push(EqfPreset {
            name: if name.is_empty() {
                format!("Preset {}", i + 1)
            } else {
                name
            },
            preamp_db,
            bands_db,
        });
    }

    Ok(presets)
}

/// Serializes one or more presets into authentic Winamp `.eqf` binary format.
#[must_use]
pub fn write_eqf_bytes(presets: &[EqfPreset]) -> Vec<u8> {
    let mut out = Vec::with_capacity(EQF_HEADER.len() + presets.len() * EQF_RECORD_SIZE);
    out.extend_from_slice(EQF_HEADER);

    for preset in presets {
        let mut name_buf = [0u8; EQF_NAME_LEN];
        let name_bytes = preset.name.as_bytes();
        let copy_len = name_bytes.len().min(EQF_NAME_LEN - 1);
        name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        out.extend_from_slice(&name_buf);

        for &band in &preset.bands_db {
            out.push(db_to_eqf_byte(band));
        }
        out.push(db_to_eqf_byte(preset.preamp_db));
    }

    out
}

/// Loads Winamp `.eqf` presets from disk.
pub fn load_eqf_file(path: impl AsRef<Path>) -> Result<Vec<EqfPreset>, PlaylistError> {
    let bytes = fs::read(path)?;
    parse_eqf_bytes(&bytes)
}

/// Saves Winamp `.eqf` presets to disk.
pub fn save_eqf_file(path: impl AsRef<Path>, presets: &[EqfPreset]) -> Result<(), PlaylistError> {
    let bytes = write_eqf_bytes(presets);
    fs::write(path, bytes)?;
    Ok(())
}

/// Returns built-in classic Pilimao EQ presets.
#[must_use]
pub fn builtin_presets() -> Vec<EqfPreset> {
    vec![
        EqfPreset::flat("Flat"),
        EqfPreset {
            name: "Rock".to_string(),
            preamp_db: 0.0,
            bands_db: [4.5, 3.0, -2.5, -4.0, -1.5, 2.5, 4.5, 5.5, 5.5, 5.5],
        },
        EqfPreset {
            name: "Bass Boost".to_string(),
            preamp_db: -2.0,
            bands_db: [6.0, 5.0, 3.5, 1.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
        EqfPreset {
            name: "Classical".to_string(),
            preamp_db: 0.0,
            bands_db: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -3.5, -3.5, -3.5, -5.0],
        },
        EqfPreset {
            name: "Vocal".to_string(),
            preamp_db: 0.0,
            bands_db: [-2.0, -1.5, 0.0, 2.5, 4.0, 3.5, 1.5, 0.0, -1.0, -1.5],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_single_and_multi_preset_eqf() {
        let presets = vec![
            EqfPreset {
                name: "Entry1".to_string(),
                preamp_db: 0.0,
                bands_db: [12.0, 6.0, 0.0, -6.0, -12.0, 3.0, -3.0, 1.5, 0.0, 12.0],
            },
            EqfPreset::flat("Flat Preset"),
        ];
        let bytes = write_eqf_bytes(&presets);
        assert_eq!(bytes.len(), 31 + 2 * 268);

        let parsed = parse_eqf_bytes(&bytes).expect("parse eqf");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Entry1");
        assert!((parsed[0].preamp_db - 0.0).abs() < 1e-6);
        assert!((parsed[0].bands_db[0] - 12.0).abs() < 1e-6);
        assert!((parsed[0].bands_db[4] - (-12.0)).abs() < 1e-6);
        for (a, b) in parsed[0].bands_db.iter().zip(presets[0].bands_db.iter()) {
            assert!(
                (a - b).abs() < 0.4,
                "quantization delta too large: {a} vs {b}"
            );
        }
        assert_eq!(parsed[1].name, "Flat Preset");
        assert!(parsed[1].bands_db.iter().all(|&g| g.abs() < 1e-6));
    }

    #[test]
    fn single_preset_eqf_is_exactly_299_bytes() {
        let bytes = write_eqf_bytes(&[EqfPreset::flat("Default")]);
        assert_eq!(bytes.len(), 299);
    }
}
