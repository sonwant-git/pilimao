//! `pilimao`: Windows 10/11 Native Rust Music Player (paying homage to classic Winamp 2.x).

mod cli;
mod config;
mod selftest;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use pilimao_audio_engine::OutputMode;
use pilimao_metadata::LegacyEncodingHint;
use pilimao_playlist::{Playlist, PlaylistEntry, load_eqf_file, load_playlist_file};
use pilimao_skin_wsz::{WszSkin, load_skin};
use pilimao_win32_shell::{ShellLaunchConfig, run_win32_shell};

use crate::config::{AppConfig, DataPaths};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliEncodingHint {
    Auto,
    Big5,
    Gbk,
    ShiftJis,
}

impl From<CliEncodingHint> for LegacyEncodingHint {
    fn from(value: CliEncodingHint) -> Self {
        match value {
            CliEncodingHint::Auto => Self::Auto,
            CliEncodingHint::Big5 => Self::PreferBig5,
            CliEncodingHint::Gbk => Self::PreferGbk,
            CliEncodingHint::ShiftJis => Self::PreferShiftJis,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOutputMode {
    Shared,
    Exclusive,
    Null,
}

impl From<CliOutputMode> for OutputMode {
    fn from(value: CliOutputMode) -> Self {
        match value {
            CliOutputMode::Shared => Self::WasapiShared,
            CliOutputMode::Exclusive => Self::WasapiExclusive,
            CliOutputMode::Null => Self::Null,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "pilimao",
    version,
    about = "Pilimao - Windows 10/11 native Rust music player paying homage to classic Winamp 2.x"
)]
struct Cli {
    /// Force Portable Mode (store pilimao.ini/pilimao.eqf/pilimao.m3u8 next to executable).
    #[arg(long, global = true)]
    portable: bool,

    /// Integer UI scale override (1, 2, or 3).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=3))]
    scale: Option<u32>,

    /// Path to a Winamp `.wsz` skin archive or directory.
    #[arg(long)]
    skin: Option<PathBuf>,

    /// Audio output mode override (`shared`, `exclusive`, or `null`).
    #[arg(long, value_enum)]
    output: Option<CliOutputMode>,

    /// Optional audio tracks or playlists (`.m3u`, `.m3u8`, `.pls`) to load on launch.
    #[arg(value_name = "FILES")]
    files: Vec<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run the 8-crate end-to-end diagnostic self-test suite.
    SelfTest,
    /// Inspect a `.wsz` skin file, folder, or `default` clean-room skin.
    InspectSkin {
        /// Path to `.wsz` file, directory, or `"default"`.
        target: String,
    },
    /// Inspect a Winamp binary `.eqf` / `winamp.q1` equalizer preset file.
    InspectEqf {
        /// Path to the `.eqf` file.
        path: PathBuf,
    },
    /// Inspect an audio file's tags, mojibake restoration, ReplayGain 2.0, and codec format.
    InspectTrack {
        /// Path to the audio track.
        path: PathBuf,
        /// Legacy CJK encoding preference hint for short ambiguous tags.
        #[arg(long, value_enum, default_value_t = CliEncodingHint::Auto)]
        encoding: CliEncodingHint,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    if let Some(cmd) = args.command {
        return match cmd {
            Commands::SelfTest => selftest::run_self_test(),
            Commands::InspectSkin { target } => cli::inspect_skin(&target),
            Commands::InspectEqf { path } => cli::inspect_eqf(&path),
            Commands::InspectTrack { path, encoding } => cli::inspect_track(&path, encoding.into()),
        };
    }

    // Resolve Portable Mode (`portable.ini`) or standard user config directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = DataPaths::resolve(&exe_dir, args.portable);
    let mut cfg = AppConfig::load_or_default(&paths.config_ini);

    if let Some(scale) = args.scale {
        cfg.scale = scale;
    }
    if let Some(output) = args.output {
        cfg.output_mode = output.into();
    }

    // Load requested skin, saved skin, or 100% Clean-Room default
    let skin = if let Some(skin_path) = &args.skin {
        load_skin(skin_path)
            .with_context(|| format!("failed to load skin '{}'", skin_path.display()))?
    } else if let Some(saved_skin) = &cfg.skin {
        load_skin(saved_skin).unwrap_or_else(|_| WszSkin::clean_room_default())
    } else {
        WszSkin::clean_room_default()
    };

    // Load saved `.eqf` state if present
    if paths.eqf_path.exists()
        && let Ok(presets) = load_eqf_file(&paths.eqf_path)
        && let Some(first) = presets.first()
    {
        cfg.eq.preamp_db = first.preamp_db;
        cfg.eq.bands_db = first.bands_db;
    }

    // Build initial playlist from CLI arguments or saved `pilimao.m3u8`
    let mut playlist = Playlist::new();
    if !args.files.is_empty() {
        for file in &args.files {
            let ext = file
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if matches!(ext.as_str(), "m3u" | "m3u8" | "pls") {
                if let Ok(entries) = load_playlist_file(file) {
                    playlist.extend(entries);
                }
            } else {
                playlist.push(PlaylistEntry::from_path(file));
            }
        }
    } else if paths.playlist_m3u8.exists()
        && let Ok(entries) = load_playlist_file(&paths.playlist_m3u8)
    {
        playlist.extend(entries);
    }

    let launch = ShellLaunchConfig {
        skin,
        scale: cfg.scale,
        playlist,
        eq: cfg.eq.clone(),
        volume: cfg.volume,
        balance: cfg.balance,
        output_mode: cfg.output_mode,
        origin_x: cfg.origin_x,
        origin_y: cfg.origin_y,
    };

    match run_win32_shell(launch) {
        Ok(()) => {
            let _ = cfg.save(&paths.config_ini);
            Ok(())
        }
        Err(pilimao_win32_shell::ShellError::WindowsOnly) => {
            println!(
                "Pilimao GUI requires Windows 10/11 (portable={}).",
                paths.portable
            );
            println!("On Linux/macOS, run CLI tools instead:");
            println!("  pilimao self-test");
            println!("  pilimao inspect-skin default");
            println!("  pilimao inspect-eqf <file.eqf>");
            println!("  pilimao inspect-track <audio_file>");
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}
