//! Lenient parser for Winamp 2.x `viscolor.txt` (24-color spectrum & oscilloscope palette).

/// 24-bit RGB color triplet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// 24-entry classic Winamp visualization color table (`viscolor.txt`).
///
/// - `0`: background
/// - `1`: background grid dots
/// - `2..=17`: 16-level spectrum analyzer gradient (index 2 = top/peak, 17 = bottom)
/// - `18..=22`: 5 oscilloscope wave colors (bright -> dim)
/// - `23`: spectrum peak-hold dot color
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisColors {
    pub colors: [Rgb; 24],
}

impl Default for VisColors {
    fn default() -> Self {
        Self::clean_room()
    }
}

impl VisColors {
    /// 100% clean-room minimalist Obsidian & Mint visualization palette.
    #[must_use]
    pub fn clean_room() -> Self {
        Self {
            colors: [
                Rgb::new(12, 15, 19),    // 0: obsidian LCD background
                Rgb::new(20, 25, 32),    // 1: ultra-subtle background dots
                Rgb::new(245, 178, 68),  // 2: spectrum top (soft warm amber peak)
                Rgb::new(224, 196, 98),  // 3
                Rgb::new(182, 224, 148), // 4
                Rgb::new(136, 238, 196), // 5
                Rgb::new(104, 236, 208), // 6
                Rgb::new(94, 234, 212),  // 7: primary mint accent
                Rgb::new(82, 222, 200),  // 8
                Rgb::new(72, 208, 188),  // 9
                Rgb::new(62, 194, 176),  // 10
                Rgb::new(54, 178, 162),  // 11
                Rgb::new(46, 162, 148),  // 12
                Rgb::new(38, 144, 132),  // 13
                Rgb::new(32, 126, 116),  // 14
                Rgb::new(26, 108, 100),  // 15
                Rgb::new(22, 90, 84),    // 16
                Rgb::new(18, 74, 70),    // 17: spectrum bottom
                Rgb::new(236, 250, 248), // 18: oscilloscope brightest
                Rgb::new(148, 240, 222), // 19
                Rgb::new(94, 234, 212),  // 20
                Rgb::new(56, 176, 160),  // 21
                Rgb::new(30, 110, 102),  // 22: oscilloscope dimmest
                Rgb::new(232, 240, 246), // 23: crisp off-white peak cap
            ],
        }
    }

    #[must_use]
    pub fn background(&self) -> Rgb {
        self.colors[0]
    }

    #[must_use]
    pub fn grid_dots(&self) -> Rgb {
        self.colors[1]
    }

    /// Returns spectrum bar color for vertical level `row_from_top` in `0..16`.
    #[must_use]
    pub fn spectrum_row(&self, row_from_top: usize) -> Rgb {
        self.colors[2 + row_from_top.min(15)]
    }

    /// Returns oscilloscope color for intensity index `0..5`.
    #[must_use]
    pub fn oscilloscope(&self, idx: usize) -> Rgb {
        self.colors[18 + idx.min(4)]
    }

    #[must_use]
    pub fn peak_dot(&self) -> Rgb {
        self.colors[23]
    }
}

/// Leniently parses a Winamp `viscolor.txt` file, falling back to clean-room defaults for
/// missing or malformed lines.
#[must_use]
pub fn parse_viscolor_txt(content: &str) -> VisColors {
    let mut palette = VisColors::clean_room();
    let mut idx = 0usize;

    for raw_line in content.lines() {
        if idx >= 24 {
            break;
        }
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with(';')
            || line.starts_with('#')
        {
            continue;
        }

        // Extract first 3 integers separated by commas/spaces (ignoring trailing comments)
        let mut nums = [0u8; 3];
        let mut found = 0usize;
        for token in line.split(|c: char| c == ',' || c.is_whitespace() || c == '/') {
            if token.is_empty() {
                continue;
            }
            if let Ok(v) = token.parse::<i32>() {
                nums[found] = v.clamp(0, 255) as u8;
                found += 1;
                if found == 3 {
                    break;
                }
            } else {
                break;
            }
        }

        if found == 3 {
            palette.colors[idx] = Rgb::new(nums[0], nums[1], nums[2]);
            idx += 1;
        }
    }

    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_viscolor_txt_with_comments_and_missing_lines() {
        let sample = "0,0,0 // background\n24,33,41 // grid\n255, 0, 0, // top bar\n";
        let vis = parse_viscolor_txt(sample);
        assert_eq!(vis.background(), Rgb::new(0, 0, 0));
        assert_eq!(vis.grid_dots(), Rgb::new(24, 33, 41));
        assert_eq!(vis.spectrum_row(0), Rgb::new(255, 0, 0));
        // Unspecified peak color falls back to clean-room default
        assert_eq!(vis.peak_dot(), VisColors::clean_room().peak_dot());
    }
}
