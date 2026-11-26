//! Legacy Big5 / GBK / Shift-JIS / UTF-8-in-Latin1 mojibake detection and restoration.

use std::fmt;

use chardetng::EncodingDetector;
use encoding_rs::{BIG5, Encoding, GBK, SHIFT_JIS};

/// Encoding detected when restoring legacy mojibake tag text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8InLatin1,
    Big5,
    Gbk,
    ShiftJis,
}

impl fmt::Display for DetectedEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8InLatin1 => write!(f, "UTF-8 (stored as Latin-1)"),
            Self::Big5 => write!(f, "Big5"),
            Self::Gbk => write!(f, "GBK"),
            Self::ShiftJis => write!(f, "Shift-JIS"),
        }
    }
}

/// Optional preference hint when disambiguating ambiguous short CJK byte sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegacyEncodingHint {
    #[default]
    Auto,
    PreferBig5,
    PreferGbk,
    PreferShiftJis,
}

/// Outcome of running mojibake restoration on a tag string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredText {
    pub text: String,
    pub restored_from: Option<DetectedEncoding>,
}

/// Attempts to restore mojibake caused by legacy Big5/GBK/Shift-JIS (or UTF-8) bytes
/// stored inside an ISO-8859-1 tag field.
///
/// Leaves valid Unicode strings, pure ASCII strings, and genuine Western European
/// Latin-1 strings (e.g., `"Café"`, `"Björk"`) untouched.
#[must_use]
pub fn restore_mojibake(input: &str, hint: LegacyEncodingHint) -> RestoredText {
    // Only Latin-1 decoded strings (all codepoints <= U+00FF) containing high bytes (>= 0x80)
    // can be ISO-8859-1 mojibake.
    let mut has_high_byte = false;
    for ch in input.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return RestoredText {
                text: input.to_string(),
                restored_from: None,
            };
        }
        if code >= 0x80 {
            has_high_byte = true;
        }
    }

    if !has_high_byte {
        return RestoredText {
            text: input.to_string(),
            restored_from: None,
        };
    }

    let raw: Vec<u8> = input.chars().map(|c| c as u8).collect();
    restore_raw_bytes(&raw, Some(input), hint)
}

/// Decodes raw tag bytes (e.g., from ID3v1 or Latin-1 recovery) into UTF-8 using
/// UTF-8 / Big5 / GBK / Shift-JIS detection.
#[must_use]
pub fn restore_raw_bytes(
    raw: &[u8],
    original_latin1: Option<&str>,
    hint: LegacyEncodingHint,
) -> RestoredText {
    // 1. Check valid UTF-8 first (very low false-positive probability for multi-byte UTF-8).
    if let Ok(utf8_str) = std::str::from_utf8(raw)
        && utf8_str.chars().any(|c| c as u32 >= 0x80)
    {
        return RestoredText {
            text: utf8_str.to_string(),
            restored_from: Some(DetectedEncoding::Utf8InLatin1),
        };
    }

    // 2. Preserve legitimate Western European Latin-1 strings (e.g. "Café", "Björk", "Édith Piaf").
    if let Some(orig) = original_latin1
        && looks_like_western_latin1(raw)
    {
        return RestoredText {
            text: orig.to_string(),
            restored_from: None,
        };
    }

    // 3. Ask chardetng for an encoding guess.
    let mut detector = EncodingDetector::new();
    detector.feed(raw, true);
    let guessed = detector.guess(None, true);

    // 4. Evaluate Big5, GBK, and Shift-JIS candidates.
    let candidates: [(&'static Encoding, DetectedEncoding); 3] = [
        (BIG5, DetectedEncoding::Big5),
        (GBK, DetectedEncoding::Gbk),
        (SHIFT_JIS, DetectedEncoding::ShiftJis),
    ];

    let mut best: Option<(i32, String, DetectedEncoding)> = None;

    for (enc, tag) in candidates {
        let Some(decoded) = enc.decode_without_bom_handling_and_without_replacement(raw) else {
            continue;
        };

        let mut score = score_decoded_cjk(&decoded, tag);

        // Reward chardetng agreement
        if guessed == enc {
            score += 35;
        }

        // Apply optional user preference bias for ambiguous short byte strings
        match (hint, tag) {
            (LegacyEncodingHint::PreferBig5, DetectedEncoding::Big5)
            | (LegacyEncodingHint::PreferGbk, DetectedEncoding::Gbk)
            | (LegacyEncodingHint::PreferShiftJis, DetectedEncoding::ShiftJis) => {
                score += 25;
            }
            _ => {}
        }

        if best
            .as_ref()
            .is_none_or(|(best_score, _, _)| score > *best_score)
        {
            best = Some((score, decoded.into_owned(), tag));
        }
    }

    if let Some((score, text, enc)) = best
        && score > 0
    {
        return RestoredText {
            text,
            restored_from: Some(enc),
        };
    }

    RestoredText {
        text: original_latin1
            .map(ToString::to_string)
            .unwrap_or_else(|| String::from_utf8_lossy(raw).into_owned()),
        restored_from: None,
    }
}

/// Heuristic to recognize genuine Western European Latin-1 strings so we never corrupt them.
fn looks_like_western_latin1(raw: &[u8]) -> bool {
    let mut high_count = 0usize;
    let mut has_consecutive_high = false;

    for (i, &b) in raw.iter().enumerate() {
        if b >= 0x80 {
            high_count += 1;
            if i > 0 && raw[i - 1] >= 0x80 {
                has_consecutive_high = true;
            }
            let is_western_letter = matches!(
                b,
                0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0xFF | 0xA1 | 0xAB | 0xB0 | 0xBB | 0xBF
            );
            if !is_western_letter {
                return false;
            }
        }
    }

    // Legitimate European words rarely have adjacent non-ASCII bytes and have mostly ASCII letters.
    !has_consecutive_high && high_count * 100 <= raw.len() * 40
}

/// Scores a candidate decoded CJK string based on Unicode script distribution and plausibility.
fn score_decoded_cjk(s: &str, enc: DetectedEncoding) -> i32 {
    let mut score = 0i32;
    let mut cjk_count = 0i32;
    let mut kana_count = 0i32;

    for ch in s.chars() {
        match ch {
            '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' => {
                // Hiragana & Katakana strongly indicate Japanese Shift-JIS.
                kana_count += 1;
                score += if enc == DetectedEncoding::ShiftJis {
                    24
                } else {
                    4
                };
            }
            '\u{4E00}'..='\u{9FFF}' => {
                // CJK Unified Ideographs
                cjk_count += 1;
                score += 12;
            }
            '\u{3000}'..='\u{303F}' | '\u{FF01}'..='\u{FF5E}' => {
                // CJK symbols & fullwidth ASCII punctuation
                score += 6;
            }
            '\u{0020}'..='\u{007E}' => {
                // Normal printable ASCII
                score += 2;
            }
            '\u{FF61}'..='\u{FF9F}' => {
                // Halfwidth Katakana: valid in Shift-JIS, often noise if accidental
                score += if enc == DetectedEncoding::ShiftJis {
                    1
                } else {
                    -10
                };
            }
            c if c.is_control() => {
                score -= 30;
            }
            '\u{E000}'..='\u{F8FF}' | '\u{FFFD}' => {
                // Private Use Area / replacement char: strong negative signal
                score -= 40;
            }
            _ => {
                // Box-drawing, Cyrillic, Greek, etc. inside unexpected CJK decode
                score -= 8;
            }
        }
    }

    if cjk_count == 0 && kana_count == 0 {
        return -10;
    }

    // Slight bonus for pure Japanese Kana + Kanji in Shift-JIS
    if enc == DetectedEncoding::ShiftJis && kana_count >= 2 {
        score += 30;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_latin1_mojibake(bytes: &[u8]) -> String {
        bytes.iter().map(|&b| b as char).collect()
    }

    #[test]
    fn restores_big5_mojibake() {
        let (bytes, _, _) = BIG5.encode("周杰倫 - 晴天");
        let garbled = to_latin1_mojibake(&bytes);
        let res = restore_mojibake(&garbled, LegacyEncodingHint::PreferBig5);
        assert_eq!(res.text, "周杰倫 - 晴天");
        assert_eq!(res.restored_from, Some(DetectedEncoding::Big5));
    }

    #[test]
    fn restores_gbk_mojibake() {
        let (bytes, _, _) = GBK.encode("简体中文测试歌曲");
        let garbled = to_latin1_mojibake(&bytes);
        let res = restore_mojibake(&garbled, LegacyEncodingHint::PreferGbk);
        assert_eq!(res.text, "简体中文测试歌曲");
        assert_eq!(res.restored_from, Some(DetectedEncoding::Gbk));
    }

    #[test]
    fn restores_shift_jis_mojibake() {
        let (bytes, _, _) = SHIFT_JIS.encode("宇多田ヒカル - ファーストラブ");
        let garbled = to_latin1_mojibake(&bytes);
        let res = restore_mojibake(&garbled, LegacyEncodingHint::Auto);
        assert_eq!(res.text, "宇多田ヒカル - ファーストラブ");
        assert_eq!(res.restored_from, Some(DetectedEncoding::ShiftJis));
    }

    #[test]
    fn restores_utf8_in_latin1_mojibake() {
        let garbled = to_latin1_mojibake("霹靂貓播放器".as_bytes());
        let res = restore_mojibake(&garbled, LegacyEncodingHint::Auto);
        assert_eq!(res.text, "霹靂貓播放器");
        assert_eq!(res.restored_from, Some(DetectedEncoding::Utf8InLatin1));
    }

    #[test]
    fn preserves_western_latin1_strings() {
        for word in ["Café", "Björk", "Édith Piaf", "Motörhead"] {
            let res = restore_mojibake(word, LegacyEncodingHint::Auto);
            assert_eq!(res.text, word);
            assert_eq!(res.restored_from, None);
        }
    }
}
