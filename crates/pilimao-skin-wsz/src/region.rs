//! Lenient parser for Winamp 2.x `region.txt` custom non-rectangular window masks.

/// A closed 2D polygon defined in 1x logical skin coordinates.
pub type Polygon = Vec<(i32, i32)>;

/// Parsed polygon masks from `region.txt` for each Winamp window state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionData {
    pub normal: Vec<Polygon>,
    pub window_shade: Vec<Polygon>,
    pub equalizer: Vec<Polygon>,
    pub equalizer_ws: Vec<Polygon>,
}

impl RegionData {
    /// Returns true if any custom polygon mask is defined.
    #[must_use]
    pub fn has_any(&self) -> bool {
        !self.normal.is_empty()
            || !self.window_shade.is_empty()
            || !self.equalizer.is_empty()
            || !self.equalizer_ws.is_empty()
    }

    /// Returns scaled polygons (`1x`, `2x`, `3x`) for the requested window mask.
    #[must_use]
    pub fn scaled_polygons(polygons: &[Polygon], scale: u32) -> Vec<Polygon> {
        let s = scale.max(1) as i32;
        polygons
            .iter()
            .map(|poly| poly.iter().map(|&(x, y)| (x * s, y * s)).collect())
            .collect()
    }
}

#[derive(Default)]
struct SectionAccum {
    num_points: Vec<usize>,
    coords: Vec<i32>,
}

impl SectionAccum {
    fn build_polygons(&self) -> Vec<Polygon> {
        let mut polygons = Vec::new();
        let mut idx = 0usize;

        if self.num_points.is_empty() {
            // Lenient fallback: treat all coords as a single polygon if >= 3 points
            let pts: Polygon = self
                .coords
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| (pair[0], pair[1]))
                .collect();
            if pts.len() >= 3 {
                polygons.push(pts);
            }
            return polygons;
        }

        for &count in &self.num_points {
            let needed = count * 2;
            if count < 3 || idx + needed > self.coords.len() {
                idx = idx.saturating_add(needed);
                continue;
            }
            let poly: Polygon = self.coords[idx..idx + needed]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| (pair[0], pair[1]))
                .collect();
            polygons.push(poly);
            idx += needed;
        }
        polygons
    }
}

/// Leniently parses a Winamp `region.txt` file into [`RegionData`].
#[must_use]
pub fn parse_region_txt(content: &str) -> RegionData {
    let mut normal = SectionAccum::default();
    let mut window_shade = SectionAccum::default();
    let mut equalizer = SectionAccum::default();
    let mut equalizer_ws = SectionAccum::default();

    enum Section {
        None,
        Normal,
        WindowShade,
        Equalizer,
        EqualizerWs,
    }

    let mut current = Section::None;

    for raw_line in content.lines() {
        // Strip inline comments (`;` or `//` or `#`)
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let sec = line[1..line.len() - 1].trim().to_ascii_lowercase();
            current = match sec.as_str() {
                "normal" => Section::Normal,
                "windowshade" => Section::WindowShade,
                "equalizer" => Section::Equalizer,
                "equalizerws" => Section::EqualizerWs,
                _ => Section::None,
            };
            continue;
        }

        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key_lower = key.trim().to_ascii_lowercase();
        let target = match current {
            Section::Normal => &mut normal,
            Section::WindowShade => &mut window_shade,
            Section::Equalizer => &mut equalizer,
            Section::EqualizerWs => &mut equalizer_ws,
            Section::None => continue,
        };

        if key_lower == "numpoints" {
            for tok in val.split(|c: char| c == ',' || c.is_whitespace()) {
                if let Ok(n) = tok.trim().parse::<usize>() {
                    target.num_points.push(n);
                }
            }
        } else if key_lower == "pointlist" {
            for tok in val.split(|c: char| c == ',' || c.is_whitespace()) {
                if let Ok(coord) = tok.trim().parse::<i32>() {
                    target.coords.push(coord);
                }
            }
        }
    }

    RegionData {
        normal: normal.build_polygons(),
        window_shade: window_shade.build_polygons(),
        equalizer: equalizer.build_polygons(),
        equalizer_ws: equalizer_ws.build_polygons(),
    }
}

fn strip_comment(line: &str) -> &str {
    let mut end = line.len();
    for marker in [";", "//", "#"] {
        if let Some(pos) = line.find(marker) {
            end = end.min(pos);
        }
    }
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_polygon_region_txt_leniently() {
        let txt = r#"
; Winamp custom region mask
[Normal]
NumPoints=4, 3
PointList=0,0, 275,0, 275,116, 0,116, 10,10, 20,10, 15,20

[WindowShade]
NumPoints=4
PointList=0,0 275,0 275,14 0,14 // inline comment
"#;
        let region = parse_region_txt(txt);
        assert_eq!(region.normal.len(), 2);
        assert_eq!(region.normal[0].len(), 4);
        assert_eq!(region.normal[1], vec![(10, 10), (20, 10), (15, 20)]);
        assert_eq!(region.window_shade.len(), 1);
        assert_eq!(
            RegionData::scaled_polygons(&region.window_shade, 2)[0],
            vec![(0, 0), (550, 0), (550, 28), (0, 28)]
        );
    }
}
