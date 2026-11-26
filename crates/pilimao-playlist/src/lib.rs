//! `pilimao-playlist`: M3U/M3U8/PLS playlists, Winamp `.eqf` binary presets,
//! shuffle/repeat/queue engine, and `nucleo-matcher` fuzzy jump-to-file (`J`).

pub mod eqf;
pub mod jump;
pub mod m3u;
pub mod pls;
pub mod queue;

use std::path::Path;

use thiserror::Error;

pub use eqf::{
    EQF_HEADER, EQF_NAME_LEN, EQF_RECORD_SIZE, EqfPreset, builtin_presets, db_to_eqf_byte,
    eqf_byte_to_db, load_eqf_file, parse_eqf_bytes, save_eqf_file, write_eqf_bytes,
};
pub use jump::{JumpMatch, JumpMatcher};
pub use m3u::{load_m3u_file, parse_m3u_bytes, parse_m3u_str, save_m3u_file, write_m3u8};
pub use pls::{load_pls_file, parse_pls_str, save_pls_file, write_pls};
pub use queue::{Playlist, PlaylistEntry, RepeatMode};

/// Errors produced by playlist or `.eqf` preset I/O.
#[derive(Debug, Error)]
pub enum PlaylistError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported playlist format: {0}")]
    UnsupportedFormat(String),
    #[error("invalid Winamp .eqf file: {0}")]
    InvalidEqf(String),
}

/// Loads a `.m3u`, `.m3u8`, or `.pls` playlist file by inspecting its extension.
pub fn load_playlist_file(path: impl AsRef<Path>) -> Result<Vec<PlaylistEntry>, PlaylistError> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    match ext.as_str() {
        "m3u" | "m3u8" => load_m3u_file(path),
        "pls" => load_pls_file(path),
        other => Err(PlaylistError::UnsupportedFormat(other.to_string())),
    }
}
