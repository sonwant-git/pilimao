//! Lenient parser for Winamp 2.x `pledit.txt` playlist colors and font specification.

use crate::viscolor::Rgb;

/// Playlist editor styling parsed from `pledit.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PleditStyle {
    pub normal: Rgb,
    pub current: Rgb,
    pub normal_bg: Rgb,
    pub selected_bg: Rgb,
    pub mb_bg: Rgb,
    pub mb_fg: Rgb,
    pub font: String,
}

impl Default for PleditStyle {
    fn default() -> Self {
        Self::clean_room()
    }
}

impl PleditStyle {
    /// Clean-room dark slate & phosphor-green Playlist styling.
    #[must_use]
    pub fn clean_room() -> Self {
        Self {
            normal: Rgb::new(112, 232, 160),
            current: Rgb::new(255, 255, 255),
            normal_bg: Rgb::new(12, 16, 22),
            selected_bg: Rgb::new(36, 76, 132),
            mb_bg: Rgb::new(12, 16, 22),
            mb_fg: Rgb::new(160, 220, 190),
            font: "Consolas".to_string(),
        }
    }
}

/// Leniently parses a Winamp `pledit.txt` file.
#[must_use]
pub fn parse_pledit_txt(content: &str) -> PleditStyle {
    let mut style = PleditStyle::clean_room();

    for raw_line in content.lines() {
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
        let val_clean = val.split("//").next().unwrap_or(val).trim();

        match key_lower.as_str() {
            "normal" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.normal = c;
                }
            }
            "current" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.current = c;
                }
            }
            "normalbg" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.normal_bg = c;
                }
            }
            "selectedbg" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.selected_bg = c;
                }
            }
            "mbbg" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.mb_bg = c;
                }
            }
            "mbfg" => {
                if let Some(c) = parse_hex_rgb(val_clean) {
                    style.mb_fg = c;
                }
            }
            "font" if !val_clean.is_empty() => {
                style.font = val_clean.to_string();
            }
            _ => {}
        }
    }

    style
}

fn parse_hex_rgb(raw: &str) -> Option<Rgb> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Rgb::new(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pledit_txt_colors_and_font() {
        let sample = "[Text]\nNormal=#00FF00\nCurrent=#FFFFFF\nNormalBG=000000\nSelectedBG=#0000C6\nFont=Arial\n";
        let style = parse_pledit_txt(sample);
        assert_eq!(style.normal, Rgb::new(0, 255, 0));
        assert_eq!(style.current, Rgb::new(255, 255, 255));
        assert_eq!(style.normal_bg, Rgb::new(0, 0, 0));
        assert_eq!(style.selected_bg, Rgb::new(0, 0, 0xC6));
        assert_eq!(style.font, "Arial");
    }
}
