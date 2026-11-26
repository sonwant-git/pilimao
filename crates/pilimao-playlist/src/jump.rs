//! Classic Winamp Jump-to-File (`J` key) fuzzy search powered by `nucleo-matcher`.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::queue::Playlist;

/// Ranked fuzzy search match returned for Jump-to-File (`J`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpMatch {
    /// 0-based index into [`Playlist::entries`].
    pub playlist_index: usize,
    /// Match relevance score (higher is better).
    pub score: u32,
    /// Character indices inside `display` that matched the query (for UI highlighting).
    pub matched_indices: Vec<u32>,
    /// Display string of the matched playlist entry.
    pub display: String,
}

/// High-speed fuzzy matcher for the Jump-to-File (`J`) dialog.
pub struct JumpMatcher {
    matcher: Matcher,
    utf32_buf: Vec<char>,
}

impl Default for JumpMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpMatcher {
    /// Creates a new [`JumpMatcher`] with default `nucleo-matcher` configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            utf32_buf: Vec::with_capacity(256),
        }
    }

    /// Searches `playlist` using `query` and returns ranked [`JumpMatch`] results.
    /// An empty query returns all playlist entries in order.
    pub fn search(&mut self, playlist: &Playlist, query: &str) -> Vec<JumpMatch> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return playlist
                .entries()
                .iter()
                .enumerate()
                .map(|(i, e)| JumpMatch {
                    playlist_index: i,
                    score: 0,
                    matched_indices: Vec::new(),
                    display: e.display_title(),
                })
                .collect();
        }

        let pattern = Pattern::parse(trimmed, CaseMatching::Ignore, Normalization::Smart);
        let mut results = Vec::new();
        let mut indices = Vec::new();

        for (i, entry) in playlist.entries().iter().enumerate() {
            let display = entry.display_title();
            self.utf32_buf.clear();
            let haystack = Utf32Str::new(&display, &mut self.utf32_buf);
            indices.clear();

            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(JumpMatch {
                    playlist_index: i,
                    score,
                    matched_indices: indices.clone(),
                    display,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.playlist_index.cmp(&b.playlist_index))
        });
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::PlaylistEntry;
    use std::path::PathBuf;

    #[test]
    fn fuzzy_jump_ranks_best_matches_first() {
        let mut pl = Playlist::new();
        for title in [
            "Daft Punk - Digital Love",
            "周杰倫 - 晴天",
            "Queen - Bohemian Rhapsody",
            "Pink Floyd - Time",
        ] {
            pl.push(PlaylistEntry {
                path: PathBuf::from(format!("{title}.flac")),
                title: Some(title.to_string()),
                duration_secs: Some(240),
            });
        }

        let mut jump = JumpMatcher::new();
        let matches = jump.search(&pl, "boh rhap");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].playlist_index, 2);
        assert!(!matches[0].matched_indices.is_empty());

        let cjk = jump.search(&pl, "晴天");
        assert_eq!(cjk.len(), 1);
        assert_eq!(cjk[0].playlist_index, 1);
    }
}
