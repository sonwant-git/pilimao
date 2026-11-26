//! Portable Mode (`portable.ini`) detection and INI configuration persistence.

use std::fs;
use std::path::{Path, PathBuf};

use pilimao_audio_engine::OutputMode;
use pilimao_dsp::{EqSettings, ReplayGainMode};

/// Resolved storage paths for Pilimao configuration and runtime files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub portable: bool,
    pub base_dir: PathBuf,
    pub config_ini: PathBuf,
    pub eqf_path: PathBuf,
    pub playlist_m3u8: PathBuf,
    pub skins_dir: PathBuf,
}

impl DataPaths {
    /// Resolves data directory based on `portable.ini` marker beside the executable or explicit override.
    #[must_use]
    pub fn resolve(exe_dir: &Path, force_portable: bool) -> Self {
        let marker = exe_dir.join("portable.ini");
        let portable = force_portable || marker.exists();

        let base_dir = if portable {
            exe_dir.to_path_buf()
        } else {
            default_user_config_dir(exe_dir)
        };

        Self {
            portable,
            config_ini: base_dir.join("pilimao.ini"),
            eqf_path: base_dir.join("pilimao.eqf"),
            playlist_m3u8: base_dir.join("pilimao.m3u8"),
            skins_dir: base_dir.join("skins"),
            base_dir,
        }
    }
}

fn default_user_config_dir(fallback: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("pilimao");
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("pilimao");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config").join("pilimao");
        }
    }
    fallback.to_path_buf()
}

/// Persistent Pilimao user preferences stored in `pilimao.ini`.
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub scale: u32,
    pub volume: f64,
    pub balance: f64,
    pub output_mode: OutputMode,
    pub replay_gain_mode: ReplayGainMode,
    pub skin: Option<String>,
    pub origin_x: i32,
    pub origin_y: i32,
    pub eq: EqSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scale: 2, // Crisp 2x default for modern 1080p/1440p Windows displays
            volume: 1.0,
            balance: 0.0,
            output_mode: if cfg!(windows) {
                OutputMode::WasapiShared
            } else {
                OutputMode::Null
            },
            replay_gain_mode: ReplayGainMode::Off,
            skin: None,
            origin_x: 120,
            origin_y: 120,
            eq: EqSettings::flat(),
        }
    }
}

impl AppConfig {
    /// Parses configuration from INI text, falling back to defaults for missing keys.
    #[must_use]
    pub fn from_ini_str(content: &str) -> Self {
        let mut cfg = Self::default();
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty()
                || line.starts_with('[')
                || line.starts_with(';')
                || line.starts_with('#')
            {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            match key.as_str() {
                "scale" => {
                    if let Ok(s) = val.parse::<u32>() {
                        cfg.scale = s.clamp(1, 3);
                    }
                }
                "volume" => {
                    if let Ok(v) = val.parse::<f64>() {
                        cfg.volume = v.clamp(0.0, 1.0);
                    }
                }
                "balance" => {
                    if let Ok(b) = val.parse::<f64>() {
                        cfg.balance = b.clamp(-1.0, 1.0);
                    }
                }
                "output_mode" => {
                    cfg.output_mode = match val.to_ascii_lowercase().as_str() {
                        "exclusive" | "wasapi_exclusive" => OutputMode::WasapiExclusive,
                        "null" => OutputMode::Null,
                        _ => OutputMode::WasapiShared,
                    };
                }
                "replay_gain" => {
                    cfg.replay_gain_mode = match val.to_ascii_lowercase().as_str() {
                        "track" => ReplayGainMode::Track,
                        "album" => ReplayGainMode::Album,
                        _ => ReplayGainMode::Off,
                    };
                }
                "skin" if !val.is_empty() => {
                    cfg.skin = Some(val.to_string());
                }
                "origin_x" => {
                    if let Ok(x) = val.parse::<i32>() {
                        cfg.origin_x = x;
                    }
                }
                "origin_y" => {
                    if let Ok(y) = val.parse::<i32>() {
                        cfg.origin_y = y;
                    }
                }
                "eq_enabled" => {
                    cfg.eq.enabled =
                        matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
                }
                "eq_preamp_db" => {
                    if let Ok(db) = val.parse::<f64>() {
                        cfg.eq.preamp_db = db;
                    }
                }
                "eq_bands_db" => {
                    for (i, tok) in val.split(',').enumerate().take(10) {
                        if let Ok(db) = tok.trim().parse::<f64>() {
                            cfg.eq.bands_db[i] = db;
                        }
                    }
                }
                _ => {}
            }
        }
        cfg.eq.clamp();
        cfg
    }

    /// Serializes configuration into clean human-editable INI format.
    #[must_use]
    pub fn to_ini_string(&self) -> String {
        let out_mode = match self.output_mode {
            OutputMode::WasapiShared => "shared",
            OutputMode::WasapiExclusive => "exclusive",
            OutputMode::Null => "null",
        };
        let rg_mode = match self.replay_gain_mode {
            ReplayGainMode::Off => "off",
            ReplayGainMode::Track => "track",
            ReplayGainMode::Album => "album",
        };
        let bands: Vec<String> = self.eq.bands_db.iter().map(|b| format!("{b:.2}")).collect();

        let mut s = String::from("[pilimao]\n");
        s.push_str(&format!("scale={}\n", self.scale));
        s.push_str(&format!("volume={:.4}\n", self.volume));
        s.push_str(&format!("balance={:.4}\n", self.balance));
        s.push_str(&format!("output_mode={out_mode}\n"));
        s.push_str(&format!("replay_gain={rg_mode}\n"));
        if let Some(skin) = &self.skin {
            s.push_str(&format!("skin={skin}\n"));
        }
        s.push_str(&format!("origin_x={}\n", self.origin_x));
        s.push_str(&format!("origin_y={}\n", self.origin_y));
        s.push_str(&format!("eq_enabled={}\n", self.eq.enabled));
        s.push_str(&format!("eq_preamp_db={:.2}\n", self.eq.preamp_db));
        s.push_str(&format!("eq_bands_db={}\n", bands.join(",")));
        s
    }

    /// Loads `pilimao.ini` if present, otherwise returns defaults.
    #[must_use]
    pub fn load_or_default(path: &Path) -> Self {
        fs::read_to_string(path)
            .map(|txt| Self::from_ini_str(&txt))
            .unwrap_or_default()
    }

    /// Saves configuration to disk, creating parent directories if needed.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_ini_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_app_config_ini() {
        let mut cfg = AppConfig {
            scale: 3,
            volume: 0.75,
            balance: -0.25,
            output_mode: OutputMode::WasapiExclusive,
            replay_gain_mode: ReplayGainMode::Album,
            skin: Some("skins/retro.wsz".to_string()),
            origin_x: 240,
            origin_y: 180,
            eq: EqSettings {
                enabled: true,
                preamp_db: -1.5,
                bands_db: [3.0, 2.0, 1.0, 0.0, -1.0, -2.0, 0.0, 1.5, 2.5, 3.5],
            },
        };
        let ini = cfg.to_ini_string();
        let parsed = AppConfig::from_ini_str(&ini);
        assert_eq!(parsed.scale, 3);
        assert_eq!(parsed.output_mode, OutputMode::WasapiExclusive);
        assert_eq!(parsed.replay_gain_mode, ReplayGainMode::Album);
        assert_eq!(parsed.skin.as_deref(), Some("skins/retro.wsz"));
        assert!(parsed.eq.enabled);
        cfg.eq.clamp();
        assert!((parsed.eq.bands_db[9] - 3.5).abs() < 1e-6);
    }

    #[test]
    fn portable_mode_resolves_paths_beside_executable() {
        let exe_dir = Path::new("/tmp/pilimao_portable_test");
        let paths = DataPaths::resolve(exe_dir, true);
        assert!(paths.portable);
        assert_eq!(paths.config_ini, exe_dir.join("pilimao.ini"));
        assert_eq!(paths.eqf_path, exe_dir.join("pilimao.eqf"));
        assert_eq!(paths.playlist_m3u8, exe_dir.join("pilimao.m3u8"));
    }
}
