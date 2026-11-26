//! Playlist state management: Repeat, Shuffle (with history), and Winamp-style Up-Next Queue (Q).

use std::collections::VecDeque;
use std::path::PathBuf;

/// Repeat playback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycles `Off -> All -> One -> Off`.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

/// Single entry inside a [`Playlist`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEntry {
    pub path: PathBuf,
    pub title: Option<String>,
    pub duration_secs: Option<i64>,
}

impl PlaylistEntry {
    /// Creates an entry from a file path with no pre-populated metadata.
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            title: None,
            duration_secs: None,
        }
    }

    /// Returns the display title (explicit title or file stem fallback).
    #[must_use]
    pub fn display_title(&self) -> String {
        if let Some(title) = &self.title
            && !title.trim().is_empty()
        {
            return title.trim().to_string();
        }
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    /// Formats duration as `M:SS` or empty string when unknown.
    #[must_use]
    pub fn formatted_duration(&self) -> String {
        match self.duration_secs {
            Some(secs) if secs >= 0 => format!("{}:{:02}", secs / 60, secs % 60),
            _ => String::new(),
        }
    }
}

/// Winamp-inspired playlist controller supporting Shuffle with history, Repeat, and Up-Next Queue.
#[derive(Debug, Clone)]
pub struct Playlist {
    entries: Vec<PlaylistEntry>,
    current: Option<usize>,
    selected: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    /// FIFO Up-Next play queue of indices into `entries` (Winamp `Q` feature).
    queue: VecDeque<usize>,
    /// Shuffled traversal order when `shuffle == true`.
    shuffle_order: Vec<usize>,
    /// Cursor inside `shuffle_order`.
    shuffle_pos: usize,
    /// Previously played track indices for accurate `previous()` navigation.
    history: Vec<usize>,
    /// Deterministic PRNG state for Fisher-Yates shuffle.
    rng_state: u64,
}

impl Default for Playlist {
    fn default() -> Self {
        Self::new()
    }
}

impl Playlist {
    /// Creates an empty playlist.
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(0x5049_4C49_4D41_4F31) // "PILIMAO1"
    }

    /// Creates an empty playlist with a specific shuffle PRNG seed (useful for tests).
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            selected: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            queue: VecDeque::new(),
            shuffle_order: Vec::new(),
            shuffle_pos: 0,
            history: Vec::new(),
            rng_state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[PlaylistEntry] {
        &self.entries
    }

    /// Returns number of tracks in the playlist.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the playlist has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the currently active track index.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// Returns the currently active entry.
    #[must_use]
    pub fn current_entry(&self) -> Option<&PlaylistEntry> {
        self.current.and_then(|i| self.entries.get(i))
    }

    /// Returns the highlighted/selected track index in the Playlist Editor.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// Sets the selected row in the Playlist Editor.
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index.filter(|&i| i < self.entries.len());
    }

    /// Returns whether Shuffle mode is active.
    #[must_use]
    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    /// Enables or disables Shuffle mode, rebuilding the shuffle order when enabled.
    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }
        self.shuffle = enabled;
        if enabled {
            self.rebuild_shuffle_order();
        }
    }

    /// Toggles Shuffle mode and returns the new state.
    pub fn toggle_shuffle(&mut self) -> bool {
        let next = !self.shuffle;
        self.set_shuffle(next);
        next
    }

    /// Returns current [`RepeatMode`].
    #[must_use]
    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    /// Sets [`RepeatMode`].
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Cycles [`RepeatMode`] and returns the new mode.
    pub fn cycle_repeat(&mut self) -> RepeatMode {
        self.repeat = self.repeat.cycle();
        self.repeat
    }

    /// Appends a single entry to the playlist.
    pub fn push(&mut self, entry: PlaylistEntry) {
        self.entries.push(entry);
        if self.selected.is_none() {
            self.selected = Some(0);
        }
        if self.shuffle {
            self.rebuild_shuffle_order();
        }
    }

    /// Appends multiple entries to the playlist.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = PlaylistEntry>) {
        self.entries.extend(iter);
        if self.selected.is_none() && !self.entries.is_empty() {
            self.selected = Some(0);
        }
        if self.shuffle {
            self.rebuild_shuffle_order();
        }
    }

    /// Clears all entries, queue, and history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
        self.selected = None;
        self.queue.clear();
        self.shuffle_order.clear();
        self.shuffle_pos = 0;
        self.history.clear();
    }

    /// Removes the entry at `index`, keeping `current`, `selected`, and `queue` consistent.
    pub fn remove(&mut self, index: usize) -> Option<PlaylistEntry> {
        if index >= self.entries.len() {
            return None;
        }
        let removed = self.entries.remove(index);

        // Adjust current index
        self.current = match self.current {
            Some(c) if c == index => None,
            Some(c) if c > index => Some(c - 1),
            other => other,
        };

        // Adjust selected index
        self.selected = if self.entries.is_empty() {
            None
        } else {
            self.selected
                .map(|s| s.min(self.entries.len().saturating_sub(1)))
        };

        // Adjust Up-Next queue indices
        self.queue.retain_mut(|q| {
            if *q == index {
                false
            } else {
                if *q > index {
                    *q -= 1;
                }
                true
            }
        });

        // Adjust history indices
        self.history.retain_mut(|h| {
            if *h == index {
                false
            } else {
                if *h > index {
                    *h -= 1;
                }
                true
            }
        });

        if self.shuffle {
            self.rebuild_shuffle_order();
        }

        Some(removed)
    }

    /// Jumps directly to `index` (e.g., double-click or Jump-to-File `J`).
    pub fn jump_to(&mut self, index: usize) -> Option<&PlaylistEntry> {
        if index >= self.entries.len() {
            return None;
        }
        if let Some(prev) = self.current
            && prev != index
        {
            self.history.push(prev);
        }
        self.current = Some(index);
        self.selected = Some(index);
        if self.shuffle
            && let Some(pos) = self.shuffle_order.iter().position(|&i| i == index)
        {
            self.shuffle_pos = pos;
        }
        self.entries.get(index)
    }

    /// Enqueues `index` into the Winamp Up-Next Queue (`Q`). Returns 1-based queue position.
    pub fn enqueue(&mut self, index: usize) -> Option<usize> {
        if index >= self.entries.len() {
            return None;
        }
        if let Some(pos) = self.queue.iter().position(|&q| q == index) {
            return Some(pos + 1);
        }
        self.queue.push_back(index);
        Some(self.queue.len())
    }

    /// Toggles `index` in/out of the Up-Next Queue (`Q` key). Returns new 1-based position if queued.
    pub fn toggle_queue(&mut self, index: usize) -> Option<usize> {
        if index >= self.entries.len() {
            return None;
        }
        if let Some(pos) = self.queue.iter().position(|&q| q == index) {
            self.queue.remove(pos);
            None
        } else {
            self.queue.push_back(index);
            Some(self.queue.len())
        }
    }

    /// Returns the 1-based queue badge position (e.g. `[1]`, `[2]`) if `index` is queued.
    #[must_use]
    pub fn queue_position(&self, index: usize) -> Option<usize> {
        self.queue.iter().position(|&q| q == index).map(|p| p + 1)
    }

    /// Returns the entire Up-Next queue in FIFO order.
    #[must_use]
    pub fn queue(&self) -> &VecDeque<usize> {
        &self.queue
    }

    /// Clears the Up-Next queue.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Peeks the next track index that will play after the current track (for Gapless pre-open).
    #[must_use]
    pub fn peek_next_index(&self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        if let Some(&queued) = self.queue.front() {
            return Some(queued);
        }
        if self.repeat == RepeatMode::One {
            return self.current.or(Some(0));
        }

        if self.shuffle {
            if self.shuffle_order.is_empty() {
                return None;
            }
            if self.current.is_none() {
                return self.shuffle_order.first().copied();
            }
            let next_pos = self.shuffle_pos + 1;
            if next_pos < self.shuffle_order.len() {
                return Some(self.shuffle_order[next_pos]);
            }
            if self.repeat == RepeatMode::All {
                return self.shuffle_order.first().copied();
            }
            return None;
        }

        match self.current {
            None => Some(0),
            Some(cur) if cur + 1 < self.entries.len() => Some(cur + 1),
            Some(_) if self.repeat == RepeatMode::All => Some(0),
            Some(_) => None,
        }
    }

    /// Advances to the next track (consuming from Up-Next queue first) and returns it.
    pub fn advance_next(&mut self) -> Option<&PlaylistEntry> {
        if self.entries.is_empty() {
            return None;
        }

        // 1. Up-Next queue always has highest priority (classic Winamp Q behavior).
        if let Some(queued) = self.queue.pop_front() {
            if let Some(prev) = self.current {
                self.history.push(prev);
            }
            self.current = Some(queued);
            return self.entries.get(queued);
        }

        // 2. Repeat One stays on current track.
        if self.repeat == RepeatMode::One {
            let idx = self.current.unwrap_or(0);
            self.current = Some(idx);
            return self.entries.get(idx);
        }

        // 3. Shuffle mode traversal.
        if self.shuffle {
            if self.shuffle_order.len() != self.entries.len() {
                self.rebuild_shuffle_order();
            }
            if let Some(prev) = self.current {
                let next_pos = self.shuffle_pos + 1;
                if next_pos < self.shuffle_order.len() {
                    self.history.push(prev);
                    self.shuffle_pos = next_pos;
                    let idx = self.shuffle_order[self.shuffle_pos];
                    self.current = Some(idx);
                    return self.entries.get(idx);
                }
                if self.repeat == RepeatMode::All {
                    self.history.push(prev);
                    self.rebuild_shuffle_order();
                    self.shuffle_pos = 0;
                    let idx = self.shuffle_order[0];
                    self.current = Some(idx);
                    return self.entries.get(idx);
                }
                return None;
            }
            self.shuffle_pos = 0;
            let idx = self.shuffle_order[0];
            self.current = Some(idx);
            return self.entries.get(idx);
        }

        // 4. Sequential playback.
        let next_idx = match self.current {
            None => Some(0),
            Some(cur) if cur + 1 < self.entries.len() => {
                self.history.push(cur);
                Some(cur + 1)
            }
            Some(cur) if self.repeat == RepeatMode::All => {
                self.history.push(cur);
                Some(0)
            }
            Some(_) => None,
        }?;
        self.current = Some(next_idx);
        self.entries.get(next_idx)
    }

    /// Navigates to the previous track (using playback history when available).
    pub fn advance_previous(&mut self) -> Option<&PlaylistEntry> {
        if self.entries.is_empty() {
            return None;
        }
        if let Some(prev) = self.history.pop()
            && prev < self.entries.len()
        {
            self.current = Some(prev);
            if self.shuffle
                && let Some(pos) = self.shuffle_order.iter().position(|&i| i == prev)
            {
                self.shuffle_pos = pos;
            }
            return self.entries.get(prev);
        }

        let prev_idx = match self.current {
            None => 0,
            Some(0) if self.repeat == RepeatMode::All => self.entries.len() - 1,
            Some(0) => 0,
            Some(cur) => cur - 1,
        };
        self.current = Some(prev_idx);
        self.entries.get(prev_idx)
    }

    fn next_rand(&mut self) -> u64 {
        // SplitMix64 step
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn rebuild_shuffle_order(&mut self) {
        let n = self.entries.len();
        self.shuffle_order = (0..n).collect();
        for i in (1..n).rev() {
            let j = (self.next_rand() as usize) % (i + 1);
            self.shuffle_order.swap(i, j);
        }
        // Keep currently playing track at position 0 if active
        if let Some(cur) = self.current
            && let Some(pos) = self.shuffle_order.iter().position(|&x| x == cur)
        {
            self.shuffle_order.swap(0, pos);
        }
        self.shuffle_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_playlist(n: usize) -> Playlist {
        let mut pl = Playlist::with_seed(42);
        for i in 0..n {
            pl.push(PlaylistEntry {
                path: PathBuf::from(format!("track_{i}.flac")),
                title: Some(format!("Track {i}")),
                duration_secs: Some(180 + i as i64),
            });
        }
        pl
    }

    #[test]
    fn queue_takes_priority_and_updates_on_remove() {
        let mut pl = sample_playlist(5);
        pl.jump_to(0);
        assert_eq!(pl.enqueue(3), Some(1));
        assert_eq!(pl.enqueue(1), Some(2));
        assert_eq!(pl.queue_position(3), Some(1));
        assert_eq!(pl.peek_next_index(), Some(3));

        let next = pl.advance_next().unwrap();
        assert_eq!(next.title.as_deref(), Some("Track 3"));
        assert_eq!(pl.queue_position(1), Some(1));

        let next2 = pl.advance_next().unwrap();
        assert_eq!(next2.title.as_deref(), Some("Track 1"));
    }

    #[test]
    fn shuffle_visits_all_tracks_once_and_history_steps_back() {
        let mut pl = sample_playlist(6);
        pl.set_shuffle(true);
        let mut visited = Vec::new();
        while let Some(entry) = pl.advance_next() {
            visited.push(entry.title.clone().unwrap());
        }
        assert_eq!(visited.len(), 6);
        let prev = pl.advance_previous().unwrap().title.clone().unwrap();
        assert_eq!(prev, visited[4]);
    }
}
