//! `.m3u` and `.m3u8` playlist parser and writer.

use std::fs;
use std::path::{Path, PathBuf};

use encoding_rs::BIG5;

use crate::PlaylistError;
use crate::queue::PlaylistEntry;

/// Parses `.m3u` / `.m3u8` content from a UTF-8 string.
#[must_use]
pub fn parse_m3u_str(content: &str, base_dir: Option<&Path>) -> Vec<PlaylistEntry> {
    let cleaned = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let mut entries = Vec::new();
    let mut pending_duration: Option<i64> = None;
    let mut pending_title: Option<String> = None;

    for raw_line in cleaned.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(extinf) = line.strip_prefix("#EXTINF:") {
            if let Some((dur_str, title)) = extinf.split_once(',') {
                pending_duration = dur_str.trim().parse::<i64>().ok().filter(|&d| d >= 0);
                let title_trim = title.trim();
                pending_title = if title_trim.is_empty() {
                    None
                } else {
                    Some(title_trim.to_string())
                };
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let path = resolve_entry_path(line, base_dir);
        entries.push(PlaylistEntry {
            path,
            title: pending_title.take(),
            duration_secs: pending_duration.take(),
        });
    }

    entries
}

/// Parses `.m3u` / `.m3u8` from raw bytes (UTF-8 first, then legacy Big5 fallback).
#[must_use]
pub fn parse_m3u_bytes(bytes: &[u8], base_dir: Option<&Path>) -> Vec<PlaylistEntry> {
    if let Ok(utf8) = std::str::from_utf8(bytes) {
        return parse_m3u_str(utf8, base_dir);
    }
    let (decoded, _, _) = BIG5.decode(bytes);
    parse_m3u_str(&decoded, base_dir)
}

/// Serializes entries to UTF-8 `#EXTM3U` format (`.m3u8`).
#[must_use]
pub fn write_m3u8(entries: &[PlaylistEntry]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for entry in entries {
        let dur = entry.duration_secs.unwrap_or(-1);
        let title = entry.display_title();
        out.push_str(&format!("#EXTINF:{dur},{title}\n"));
        out.push_str(&entry.path.to_string_lossy());
        out.push('\n');
    }
    out
}

/// Loads an `.m3u` / `.m3u8` file from disk, resolving relative paths against its parent folder.
pub fn load_m3u_file(path: impl AsRef<Path>) -> Result<Vec<PlaylistEntry>, PlaylistError> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    Ok(parse_m3u_bytes(&bytes, path.parent()))
}

/// Saves playlist entries to an `.m3u8` file on disk.
pub fn save_m3u_file(
    path: impl AsRef<Path>,
    entries: &[PlaylistEntry],
) -> Result<(), PlaylistError> {
    let content = write_m3u8(entries);
    fs::write(path, content)?;
    Ok(())
}

pub(crate) fn resolve_entry_path(raw: &str, base_dir: Option<&Path>) -> PathBuf {
    // Preserve URLs or Windows drive-letter paths (e.g. C:\Music\foo.mp3) untouched.
    let bytes = raw.as_bytes();
    let is_windows_abs = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    let is_unc = raw.starts_with("\\\\");
    let is_url = raw.contains("://");

    let candidate = PathBuf::from(raw);
    if is_windows_abs || is_unc || is_url || candidate.is_absolute() {
        candidate
    } else if let Some(base) = base_dir {
        base.join(candidate)
    } else {
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_m3u8_entries() {
        let sample = "\u{FEFF}#EXTM3U\r\n#EXTINF:245,Jay Chou - Sunny Day\r\nmusic/sunny.flac\r\n#EXTINF:-1,Stream\r\nhttps://example.com/live.mp3\r\n";
        let entries = parse_m3u_str(sample, Some(Path::new("/library")));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title.as_deref(), Some("Jay Chou - Sunny Day"));
        assert_eq!(entries[0].duration_secs, Some(245));
        assert_eq!(entries[0].path, PathBuf::from("/library/music/sunny.flac"));
        assert_eq!(entries[1].duration_secs, None);
        assert_eq!(
            entries[1].path,
            PathBuf::from("https://example.com/live.mp3")
        );

        let written = write_m3u8(&entries);
        let reparsed = parse_m3u_str(&written, None);
        assert_eq!(reparsed.len(), 2);
        assert_eq!(reparsed[0].title, entries[0].title);
        assert_eq!(reparsed[0].duration_secs, entries[0].duration_secs);
    }
}
