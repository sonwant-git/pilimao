//! Winamp / SHOUTcast `.pls` (PLS v2) playlist parser and writer.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::PlaylistError;
use crate::m3u::resolve_entry_path;
use crate::queue::PlaylistEntry;

#[derive(Default)]
struct PlsSlot {
    file: Option<String>,
    title: Option<String>,
    length: Option<i64>,
}

/// Parses a `.pls` playlist string into ordered [`PlaylistEntry`] items.
#[must_use]
pub fn parse_pls_str(content: &str, base_dir: Option<&Path>) -> Vec<PlaylistEntry> {
    let cleaned = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let mut slots: BTreeMap<usize, PlsSlot> = BTreeMap::new();

    for raw_line in cleaned.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with('[')
            || line.starts_with(';')
            || line.starts_with('#')
        {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key_lower = key.trim().to_ascii_lowercase();
        let val = val.trim();

        if let Some(idx_str) = key_lower.strip_prefix("file")
            && let Ok(idx) = idx_str.parse::<usize>()
        {
            slots.entry(idx).or_default().file = Some(val.to_string());
        } else if let Some(idx_str) = key_lower.strip_prefix("title")
            && let Ok(idx) = idx_str.parse::<usize>()
        {
            if !val.is_empty() {
                slots.entry(idx).or_default().title = Some(val.to_string());
            }
        } else if let Some(idx_str) = key_lower.strip_prefix("length")
            && let Ok(idx) = idx_str.parse::<usize>()
        {
            slots.entry(idx).or_default().length = val.parse::<i64>().ok().filter(|&d| d >= 0);
        }
    }

    slots
        .into_values()
        .filter_map(|slot| {
            let raw_file = slot.file?;
            Some(PlaylistEntry {
                path: resolve_entry_path(&raw_file, base_dir),
                title: slot.title,
                duration_secs: slot.length,
            })
        })
        .collect()
}

/// Serializes playlist entries into standard Winamp PLS v2 format.
#[must_use]
pub fn write_pls(entries: &[PlaylistEntry]) -> String {
    let mut out = String::from("[playlist]\n");
    for (i, entry) in entries.iter().enumerate() {
        let idx = i + 1;
        out.push_str(&format!("File{idx}={}\n", entry.path.to_string_lossy()));
        out.push_str(&format!("Title{idx}={}\n", entry.display_title()));
        let len = entry.duration_secs.unwrap_or(-1);
        out.push_str(&format!("Length{idx}={len}\n"));
    }
    out.push_str(&format!("NumberOfEntries={}\n", entries.len()));
    out.push_str("Version=2\n");
    out
}

/// Loads a `.pls` file from disk.
pub fn load_pls_file(path: impl AsRef<Path>) -> Result<Vec<PlaylistEntry>, PlaylistError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)?;
    Ok(parse_pls_str(&content, path.parent()))
}

/// Saves entries to a `.pls` file on disk.
pub fn save_pls_file(
    path: impl AsRef<Path>,
    entries: &[PlaylistEntry],
) -> Result<(), PlaylistError> {
    let content = write_pls(entries);
    fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn round_trips_pls_playlist() {
        let pls = "[playlist]\nFile2=track2.mp3\nTitle2=Second\nLength2=180\nFile1=track1.flac\nTitle1=First\nLength1=210\nNumberOfEntries=2\nVersion=2\n";
        let entries = parse_pls_str(pls, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("track1.flac"));
        assert_eq!(entries[0].title.as_deref(), Some("First"));
        assert_eq!(entries[0].duration_secs, Some(210));
        assert_eq!(entries[1].path, PathBuf::from("track2.mp3"));

        let serialized = write_pls(&entries);
        let reparsed = parse_pls_str(&serialized, None);
        assert_eq!(reparsed.len(), 2);
        assert_eq!(reparsed[0].title, entries[0].title);
        assert_eq!(reparsed[1].duration_secs, entries[1].duration_secs);
    }
}
