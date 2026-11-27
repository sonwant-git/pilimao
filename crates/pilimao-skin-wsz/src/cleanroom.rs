//! 100% Clean-Room procedurally generated Pilimao Obsidian & Mint default skin.
//!
//! All sprite sheets match classic Winamp 2.x sprite layout coordinates while using a
//! refined, disciplined dark-studio minimalist palette with zero third-party bitmap data.

use crate::pledit::PleditStyle;
use crate::region::RegionData;
use crate::viscolor::{Rgb, VisColors};

/// Raw RGBA8 sprite sheet image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteImage {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8 pixel data (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

impl SpriteImage {
    /// Creates a solid-filled RGBA image.
    #[must_use]
    pub fn filled(width: u32, height: u32, color: Rgb) -> Self {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            rgba.extend_from_slice(&[color.r, color.g, color.b, 255]);
        }
        Self {
            width,
            height,
            rgba,
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, c: Rgb) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.rgba[idx] = c.r;
            self.rgba[idx + 1] = c.g;
            self.rgba[idx + 2] = c.b;
            self.rgba[idx + 3] = 255;
        }
    }

    #[inline]
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            [
                self.rgba[idx],
                self.rgba[idx + 1],
                self.rgba[idx + 2],
                self.rgba[idx + 3],
            ]
        } else {
            [0, 0, 0, 255]
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: Rgb) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, c);
            }
        }
    }

    /// Draws a crisp 1px outline rectangle.
    pub fn draw_border(&mut self, x: u32, y: u32, w: u32, h: u32, color: Rgb) {
        if w < 2 || h < 2 {
            return;
        }
        for dx in 0..w {
            self.set_pixel(x + dx, y, color);
            self.set_pixel(x + dx, y + h - 1, color);
        }
        for dy in 0..h {
            self.set_pixel(x, y + dy, color);
            self.set_pixel(x + w - 1, y + dy, color);
        }
    }

    /// Draws a subtle, modern 1px bevel frame (raised if `raised == true`, recessed otherwise).
    pub fn draw_bevel(&mut self, x: u32, y: u32, w: u32, h: u32, raised: bool) {
        if w < 2 || h < 2 {
            return;
        }
        let top_left = if raised { BORDER_LIGHT } else { BORDER_DARK };
        let bot_right = if raised { BORDER_DARK } else { BORDER_SUBTLE };
        for dx in 0..w {
            self.set_pixel(x + dx, y, top_left);
            self.set_pixel(x + dx, y + h - 1, bot_right);
        }
        for dy in 0..h {
            self.set_pixel(x, y + dy, top_left);
            self.set_pixel(x + w - 1, y + dy, bot_right);
        }
    }

    /// Draws a vertical gradient rectangle.
    pub fn fill_v_gradient(&mut self, x: u32, y: u32, w: u32, h: u32, top: Rgb, bottom: Rgb) {
        if h == 0 {
            return;
        }
        for dy in 0..h {
            let t = dy as f32 / (h.saturating_sub(1).max(1)) as f32;
            let c = Rgb::new(
                lerp_u8(top.r, bottom.r, t),
                lerp_u8(top.g, bottom.g, t),
                lerp_u8(top.b, bottom.b, t),
            );
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, c);
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// All standard Winamp 2.x classic skin sprite sheet slots.
#[derive(Debug, Clone)]
pub struct SkinSprites {
    pub main: SpriteImage,
    pub titlebar: SpriteImage,
    pub cbuttons: SpriteImage,
    pub volume: SpriteImage,
    pub balance: SpriteImage,
    pub posbar: SpriteImage,
    pub shufrep: SpriteImage,
    pub monoster: SpriteImage,
    pub playpaus: SpriteImage,
    pub numbers: SpriteImage,
    pub text: SpriteImage,
    pub eqmain: SpriteImage,
    pub eq_ex: SpriteImage,
    pub pledit: SpriteImage,
}

/// Diagnostic report describing which skin assets were loaded from `.wsz` vs fallback.
#[derive(Debug, Clone, Default)]
pub struct SkinReport {
    pub source_name: String,
    pub loaded_files: Vec<String>,
    pub fallback_assets: Vec<String>,
    pub warnings: Vec<String>,
}

/// Complete Winamp 2.x compatible skin ready for rendering and Win32 region masking.
#[derive(Debug, Clone)]
pub struct WszSkin {
    pub name: String,
    pub sprites: SkinSprites,
    pub vis_colors: VisColors,
    pub pledit_style: PleditStyle,
    pub regions: RegionData,
    pub report: SkinReport,
}

impl WszSkin {
    /// Generates the 100% Clean-Room default "Pilimao Obsidian & Mint" skin in pure Rust.
    #[must_use]
    pub fn clean_room_default() -> Self {
        let sprites = SkinSprites {
            main: build_main_sprite(),
            titlebar: build_titlebar_sprite(),
            cbuttons: build_cbuttons_sprite(),
            volume: build_volume_sprite(),
            balance: build_balance_sprite(),
            posbar: build_posbar_sprite(),
            shufrep: build_shufrep_sprite(),
            monoster: build_monoster_sprite(),
            playpaus: build_playpaus_sprite(),
            numbers: build_numbers_sprite(),
            text: build_text_sprite(),
            eqmain: build_eqmain_sprite(),
            eq_ex: build_eq_ex_sprite(),
            pledit: build_pledit_sprite(),
        };

        let report = SkinReport {
            source_name: "Pilimao Obsidian & Mint (Clean-Room)".to_string(),
            loaded_files: vec![
                "cleanroom:main".into(),
                "cleanroom:titlebar".into(),
                "cleanroom:cbuttons".into(),
                "cleanroom:volume".into(),
                "cleanroom:balance".into(),
                "cleanroom:posbar".into(),
                "cleanroom:shufrep".into(),
                "cleanroom:monoster".into(),
                "cleanroom:playpaus".into(),
                "cleanroom:nums_ex".into(),
                "cleanroom:text".into(),
                "cleanroom:eqmain".into(),
                "cleanroom:eq_ex".into(),
                "cleanroom:pledit".into(),
            ],
            fallback_assets: Vec::new(),
            warnings: Vec::new(),
        };

        Self {
            name: "Pilimao Obsidian & Mint (Clean-Room)".to_string(),
            sprites,
            vis_colors: VisColors::clean_room(),
            pledit_style: PleditStyle::clean_room(),
            regions: RegionData::default(),
            report,
        }
    }
}

// Cohesive 4-tone minimalist Obsidian & Mint palette
const CHASSIS_TOP: Rgb = Rgb::new(30, 34, 41);
const CHASSIS_BOT: Rgb = Rgb::new(22, 25, 31);
const HEADER_ACTIVE_TOP: Rgb = Rgb::new(39, 45, 55);
const HEADER_ACTIVE_BOT: Rgb = Rgb::new(31, 36, 44);
const HEADER_INACTIVE_TOP: Rgb = Rgb::new(28, 32, 38);
const HEADER_INACTIVE_BOT: Rgb = Rgb::new(24, 27, 33);
const LCD_BG: Rgb = Rgb::new(12, 15, 19);
const BORDER_LIGHT: Rgb = Rgb::new(56, 64, 76);
const BORDER_SUBTLE: Rgb = Rgb::new(42, 48, 58);
const BORDER_DARK: Rgb = Rgb::new(14, 17, 22);
const BTN_TOP: Rgb = Rgb::new(40, 46, 56);
const BTN_BOT: Rgb = Rgb::new(30, 35, 43);
const BTN_PRESSED: Rgb = Rgb::new(20, 24, 30);
const MINT: Rgb = Rgb::new(94, 234, 212);
const MINT_DIM: Rgb = Rgb::new(46, 66, 68);
const TEXT_SOFT: Rgb = Rgb::new(186, 198, 210);
const TEXT_MUTED: Rgb = Rgb::new(112, 124, 138);
const AMBER: Rgb = Rgb::new(245, 178, 68);
const CLOSE_SOFT: Rgb = Rgb::new(232, 114, 114);

fn build_main_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(275, 116, CHASSIS_BOT);
    img.fill_v_gradient(0, 0, 275, 116, CHASSIS_TOP, CHASSIS_BOT);
    img.draw_border(0, 0, 275, 116, BORDER_LIGHT);
    img.draw_border(1, 1, 273, 114, BORDER_DARK);

    // Left unified OLED display recess (status + clock + 19-band spectrum)
    img.fill_rect(10, 21, 93, 42, LCD_BG);
    img.draw_border(9, 20, 95, 44, BORDER_SUBTLE);

    // Right marquee title strip
    img.fill_rect(109, 21, 157, 15, LCD_BG);
    img.draw_border(108, 20, 159, 17, BORDER_SUBTLE);

    // kbps & kHz LCD wells + crisp micro-labels
    img.fill_rect(109, 41, 21, 10, LCD_BG);
    img.draw_border(108, 40, 23, 12, BORDER_SUBTLE);
    draw_micro_text_3x5(&mut img, 133, 44, "KBPS", TEXT_MUTED);

    img.fill_rect(155, 41, 16, 10, LCD_BG);
    img.draw_border(154, 40, 18, 12, BORDER_SUBTLE);
    draw_micro_text_3x5(&mut img, 174, 44, "KHZ", TEXT_MUTED);

    // Stereo / Mono housing
    img.fill_rect(211, 40, 56, 12, LCD_BG);
    img.draw_border(210, 39, 58, 14, BORDER_SUBTLE);

    // Seek bar housing
    img.fill_rect(16, 72, 248, 10, LCD_BG);
    img.draw_border(15, 71, 250, 12, BORDER_SUBTLE);

    img
}

fn draw_window_glyphs(img: &mut SpriteImage, base_x: u32, base_y: u32, active: bool) {
    let icon_col = if active { TEXT_SOFT } else { TEXT_MUTED };
    let close_col = if active { CLOSE_SOFT } else { TEXT_MUTED };

    // Minimize glyph '−' at (base_x + 244, base_y + 3)
    img.fill_rect(base_x + 246, base_y + 8, 5, 1, icon_col);

    // Windowshade glyph '═' at (base_x + 254, base_y + 3)
    img.fill_rect(base_x + 256, base_y + 4, 5, 1, icon_col);
    img.fill_rect(base_x + 256, base_y + 6, 5, 1, icon_col);
    img.fill_rect(base_x + 256, base_y + 8, 5, 1, icon_col);

    // Close glyph '×' at (base_x + 264, base_y + 3)
    for d in 0..5u32 {
        img.set_pixel(base_x + 266 + d, base_y + 4 + d, close_col);
        img.set_pixel(base_x + 270 - d, base_y + 4 + d, close_col);
    }
}

fn build_titlebar_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(344, 87, CHASSIS_BOT);

    // Active titlebar at (27, 0, 275x14)
    img.fill_v_gradient(27, 0, 275, 14, HEADER_ACTIVE_TOP, HEADER_ACTIVE_BOT);
    img.draw_border(27, 0, 275, 14, BORDER_LIGHT);
    img.fill_rect(28, 13, 273, 1, BORDER_DARK);
    img.fill_rect(31, 4, 2, 6, MINT);
    draw_window_glyphs(&mut img, 27, 0, true);

    // Inactive titlebar at (27, 15, 275x14)
    img.fill_v_gradient(27, 15, 275, 14, HEADER_INACTIVE_TOP, HEADER_INACTIVE_BOT);
    img.draw_border(27, 15, 275, 14, BORDER_SUBTLE);
    img.fill_rect(31, 19, 2, 6, MINT_DIM);
    draw_window_glyphs(&mut img, 27, 15, false);

    // Windowshade active at (27, 29, 275x14)
    img.fill_v_gradient(27, 29, 275, 14, HEADER_ACTIVE_TOP, HEADER_ACTIVE_BOT);
    img.draw_border(27, 29, 275, 14, BORDER_LIGHT);
    img.fill_rect(31, 33, 2, 6, MINT);
    draw_window_glyphs(&mut img, 27, 29, true);

    // Windowshade inactive at (27, 42, 275x14)
    img.fill_v_gradient(27, 42, 275, 14, HEADER_INACTIVE_TOP, HEADER_INACTIVE_BOT);
    img.draw_border(27, 42, 275, 14, BORDER_SUBTLE);
    img.fill_rect(31, 46, 2, 6, MINT_DIM);
    draw_window_glyphs(&mut img, 27, 42, false);

    img
}

fn build_cbuttons_sprite() -> SpriteImage {
    // 136x36: row 0 (y=0..18) normal, row 1 (y=18..36) pressed
    let mut img = SpriteImage::filled(136, 36, CHASSIS_BOT);
    let buttons = [(0, 23), (23, 23), (46, 23), (69, 23), (92, 22), (114, 22)];

    for (idx, &(bx, bw)) in buttons.iter().enumerate() {
        // Normal sleek dark flat button with subtle 1px border
        img.fill_v_gradient(bx + 1, 1, bw - 2, 16, BTN_TOP, BTN_BOT);
        img.draw_border(bx, 0, bw, 18, BORDER_SUBTLE);
        img.fill_rect(bx + 1, 1, bw - 2, 1, BORDER_LIGHT);
        let glyph_col = if idx == 1 { MINT } else { TEXT_SOFT };
        draw_transport_glyph(&mut img, idx, bx, 0, glyph_col);

        // Pressed state
        img.fill_rect(bx + 1, 19, bw - 2, 16, BTN_PRESSED);
        img.draw_border(bx, 18, bw, 18, MINT);
        draw_transport_glyph(&mut img, idx, bx, 19, MINT);
    }
    img
}

fn draw_transport_glyph(img: &mut SpriteImage, idx: usize, bx: u32, by: u32, col: Rgb) {
    match idx {
        0 => {
            // Prev: |<<
            img.fill_rect(bx + 6, by + 5, 2, 7, col);
            for d in 0..4 {
                img.fill_rect(bx + 9 + d, by + 8 - d, 1, d * 2 + 1, col);
                img.fill_rect(bx + 13 + d, by + 8 - d, 1, d * 2 + 1, col);
            }
        }
        1 => {
            // Play: >
            for d in 0..5 {
                img.fill_rect(bx + 9 + d, by + 4 + d, 1, 9 - d * 2, col);
            }
        }
        2 => {
            // Pause: ||
            img.fill_rect(bx + 8, by + 5, 2, 7, col);
            img.fill_rect(bx + 13, by + 5, 2, 7, col);
        }
        3 => {
            // Stop: []
            img.fill_rect(bx + 8, by + 5, 7, 7, col);
        }
        4 => {
            // Next: >>|
            for d in 0..4 {
                img.fill_rect(bx + 5 + d, by + 5 + d, 1, 7 - d * 2, col);
                img.fill_rect(bx + 9 + d, by + 5 + d, 1, 7 - d * 2, col);
            }
            img.fill_rect(bx + 14, by + 5, 2, 7, col);
        }
        _ => {
            // Eject: ^ over _
            for d in 0..4 {
                img.fill_rect(bx + 10 - d, by + 5 + d, d * 2 + 1, 1, col);
            }
            img.fill_rect(bx + 7, by + 11, 7, 2, col);
        }
    }
}

fn build_volume_sprite() -> SpriteImage {
    // 28 frames of 68x13 stacked vertically (y = frame * 15) + thumb at y = 422
    let mut img = SpriteImage::filled(68, 433, CHASSIS_BOT);
    for frame in 0..28u32 {
        let y0 = frame * 15;
        let t = frame as f32 / 27.0;
        let fill_col = if t > 0.85 { AMBER } else { MINT };
        img.fill_rect(0, y0, 68, 13, CHASSIS_BOT);
        // Slim dark recessed track groove
        img.fill_rect(2, y0 + 5, 64, 3, LCD_BG);
        img.draw_border(1, y0 + 4, 66, 5, BORDER_SUBTLE);
        let fill_w = ((frame + 1) * 62 / 28).min(62);
        img.fill_rect(2, y0 + 5, fill_w, 3, fill_col);
    }
    // Sleek pill thumb normal (15, 422, 14x11) & pressed (0, 422, 14x11)
    img.fill_v_gradient(16, 423, 12, 9, Rgb::new(72, 82, 96), Rgb::new(48, 56, 68));
    img.draw_border(15, 422, 14, 11, BORDER_LIGHT);
    img.fill_rect(21, 425, 2, 5, MINT);

    img.fill_rect(1, 423, 12, 9, BTN_PRESSED);
    img.draw_border(0, 422, 14, 11, MINT);
    img.fill_rect(6, 425, 2, 5, MINT);
    img
}

fn build_balance_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(68, 433, CHASSIS_BOT);
    for frame in 0..28u32 {
        let y0 = frame * 15;
        img.fill_rect(9, y0, 38, 13, CHASSIS_BOT);
        img.fill_rect(10, y0 + 5, 36, 3, LCD_BG);
        img.draw_border(9, y0 + 4, 38, 5, BORDER_SUBTLE);
        // Subtle center tick mark
        img.fill_rect(27, y0 + 2, 2, 9, BORDER_LIGHT);
    }
    img.fill_v_gradient(16, 423, 12, 9, Rgb::new(72, 82, 96), Rgb::new(48, 56, 68));
    img.draw_border(15, 422, 14, 11, BORDER_LIGHT);
    img.fill_rect(21, 425, 2, 5, TEXT_SOFT);

    img.fill_rect(1, 423, 12, 9, BTN_PRESSED);
    img.draw_border(0, 422, 14, 11, MINT);
    img
}

fn build_posbar_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(307, 10, LCD_BG);
    img.draw_border(0, 0, 248, 10, BORDER_SUBTLE);
    img.fill_rect(3, 4, 242, 2, Rgb::new(34, 42, 52));

    // Thumb normal at (248, 0, 29x10)
    img.fill_v_gradient(249, 1, 27, 8, Rgb::new(66, 76, 90), Rgb::new(42, 50, 62));
    img.draw_border(248, 0, 29, 10, BORDER_LIGHT);
    img.fill_rect(261, 2, 3, 6, MINT);

    // Thumb pressed at (278, 0, 29x10)
    img.fill_rect(279, 1, 27, 8, BTN_PRESSED);
    img.draw_border(278, 0, 29, 10, MINT);
    img.fill_rect(291, 2, 3, 6, MINT);
    img
}

fn build_shufrep_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(92, 85, CHASSIS_BOT);
    // Repeat (0,0, 28x15) off / (0,30) on
    // Shuffle (28,0, 47x15) off / (28,30) on
    // EQ (0,61, 23x12) off / (0,73) on
    // PL (23,61, 23x12) off / (23,73) on
    let buttons: [(u32, u32, u32, u32, &str, bool); 8] = [
        (0, 0, 28, 15, "REP", false),
        (0, 30, 28, 15, "REP", true),
        (28, 0, 47, 15, "SHUF", false),
        (28, 30, 47, 15, "SHUF", true),
        (0, 61, 23, 12, "EQ", false),
        (0, 73, 23, 12, "EQ", true),
        (23, 61, 23, 12, "PL", false),
        (23, 73, 23, 12, "PL", true),
    ];

    for (x, y, w, h, label, lit) in buttons {
        img.fill_v_gradient(x + 1, y + 1, w - 2, h - 2, BTN_TOP, BTN_BOT);
        img.draw_border(x, y, w, h, if lit { MINT } else { BORDER_SUBTLE });
        // Crisp active LED bar along top inner edge
        if lit {
            img.fill_rect(x + 2, y + 1, w - 4, 1, MINT);
        }
        let text_w = (label.len() as u32) * 4 - 1;
        let tx = x + (w.saturating_sub(text_w)) / 2;
        let ty = y + (h.saturating_sub(5)) / 2;
        draw_micro_text_3x5(&mut img, tx, ty, label, if lit { MINT } else { TEXT_MUTED });
    }
    img
}

fn build_monoster_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(58, 24, LCD_BG);
    // Stereo lit (0,0,29x12), dim (0,12,29x12)
    draw_micro_text_3x5(&mut img, 3, 4, "STEREO", MINT);
    draw_micro_text_3x5(&mut img, 3, 16, "STEREO", MINT_DIM);
    // Mono lit (29,0,29x12), dim (29,12,29x12)
    draw_micro_text_3x5(&mut img, 35, 4, "MONO", MINT);
    draw_micro_text_3x5(&mut img, 35, 16, "MONO", MINT_DIM);
    img
}

fn build_playpaus_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(42, 9, LCD_BG);
    // Play (0,0, 9x9)
    for d in 0..4 {
        img.fill_rect(2 + d, 1 + d, 1, 7 - d * 2, MINT);
    }
    // Pause (9,0, 9x9)
    img.fill_rect(11, 1, 2, 7, AMBER);
    img.fill_rect(15, 1, 2, 7, AMBER);
    // Stop (18,0, 9x9)
    img.fill_rect(20, 2, 5, 5, TEXT_MUTED);
    img
}

fn build_numbers_sprite() -> SpriteImage {
    // 12 glyphs of 9x13: '0'..'9', ' ', '-'
    let mut img = SpriteImage::filled(108, 13, LCD_BG);
    const SEGMENTS: [u8; 12] = [
        0b0011_1111, // 0
        0b0000_0110, // 1
        0b0101_1011, // 2
        0b0100_1111, // 3
        0b0110_0110, // 4
        0b0110_1101, // 5
        0b0111_1101, // 6
        0b0000_0111, // 7
        0b0111_1111, // 8
        0b0110_1111, // 9
        0b0000_0000, // blank
        0b0100_0000, // minus
    ];
    for (i, &mask) in SEGMENTS.iter().enumerate() {
        let ox = (i as u32) * 9;
        // Faint unlit segment ghosting for authentic hi-fi LCD polish
        let dim = Rgb::new(20, 30, 32);
        img.fill_rect(ox + 2, 1, 5, 1, dim);
        img.fill_rect(ox + 7, 2, 1, 4, dim);
        img.fill_rect(ox + 7, 7, 1, 4, dim);
        img.fill_rect(ox + 2, 11, 5, 1, dim);
        img.fill_rect(ox + 1, 7, 1, 4, dim);
        img.fill_rect(ox + 1, 2, 1, 4, dim);
        img.fill_rect(ox + 2, 6, 5, 1, dim);

        if mask & 0b0000_0001 != 0 {
            img.fill_rect(ox + 2, 1, 5, 1, MINT);
        }
        if mask & 0b0000_0010 != 0 {
            img.fill_rect(ox + 7, 2, 1, 4, MINT);
        }
        if mask & 0b0000_0100 != 0 {
            img.fill_rect(ox + 7, 7, 1, 4, MINT);
        }
        if mask & 0b0000_1000 != 0 {
            img.fill_rect(ox + 2, 11, 5, 1, MINT);
        }
        if mask & 0b0001_0000 != 0 {
            img.fill_rect(ox + 1, 7, 1, 4, MINT);
        }
        if mask & 0b0010_0000 != 0 {
            img.fill_rect(ox + 1, 2, 1, 4, MINT);
        }
        if mask & 0b0100_0000 != 0 {
            img.fill_rect(ox + 2, 6, 5, 1, MINT);
        }
    }
    img
}

fn build_text_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(155, 18, LCD_BG);
    let rows: [&str; 3] = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@   ",
        "0123456789 ...:()-'!_+\\/[]^&%.=",
        "                               ",
    ];
    for (row_idx, row_chars) in rows.iter().enumerate() {
        for (col_idx, ch) in row_chars.chars().enumerate().take(31) {
            draw_bitmap_char_5x6(&mut img, col_idx as u32 * 5, row_idx as u32 * 6, ch, MINT);
        }
    }
    img
}

/// Crisp 3x5 micro bitmap font renderer for compact labels (EQ bands, button pills, KBPS/KHZ).
pub fn draw_micro_text_3x5(img: &mut SpriteImage, x: u32, y: u32, text: &str, color: Rgb) {
    let mut cx = x;
    for ch in text.chars() {
        let rows = micro_glyph_3x5(ch);
        for (dy, &bits) in rows.iter().enumerate() {
            for dx in 0..3u32 {
                if bits & (1 << (2 - dx)) != 0 {
                    img.set_pixel(cx + dx, y + dy as u32, color);
                }
            }
        }
        cx += 4;
    }
}

/// Returns the 5-row 3-bit bitmap for a micro 3x5 ASCII character.
#[must_use]
pub fn micro_glyph_3x5(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b111, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b110],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '[' => [0b110, 0b100, 0b100, 0b100, 0b110],
        ']' => [0b011, 0b001, 0b001, 0b001, 0b011],
        _ => [0, 0, 0, 0, 0],
    }
}

/// Clean-room 5x6 uppercase/digit bitmap font renderer.
pub fn draw_bitmap_char_5x6(img: &mut SpriteImage, x: u32, y: u32, ch: char, color: Rgb) {
    let rows: [u8; 5] = match ch.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10111, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b11100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b11110, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        '0' => [0b01110, 0b10011, 0b10101, 0b11001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00110, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b01110, 0b00001, 0b11110],
        '4' => [0b10010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b11110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b01110],
        '7' => [0b11111, 0b00010, 0b00100, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b01110, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b01111, 0b00001, 0b01110],
        '-' => [0b00000, 0b00000, 0b11111, 0b00000, 0b00000],
        ':' => [0b00000, 0b00100, 0b00000, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000],
        '(' => [0b00010, 0b00100, 0b00100, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00100, 0b00100, 0b01000],
        '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01110],
        ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b01110],
        '+' => [0b00000, 0b00100, 0b01110, 0b00100, 0b00000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '*' => [0b00000, 0b10101, 0b01110, 0b10101, 0b00000],
        _ => [0, 0, 0, 0, 0],
    };
    for (dy, &bits) in rows.iter().enumerate() {
        for dx in 0..5u32 {
            if bits & (1 << (4 - dx)) != 0 {
                img.set_pixel(x + dx, y + dy as u32, color);
            }
        }
    }
}

fn build_eqmain_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(275, 315, CHASSIS_BOT);
    // Main EQ window background (0, 0, 275x116)
    img.fill_v_gradient(0, 0, 275, 116, CHASSIS_TOP, CHASSIS_BOT);
    img.draw_border(0, 0, 275, 116, BORDER_LIGHT);
    img.draw_border(1, 1, 273, 114, BORDER_DARK);

    // Active titlebar strip (0, 134, 275x14)
    img.fill_v_gradient(0, 134, 275, 14, HEADER_ACTIVE_TOP, HEADER_ACTIVE_BOT);
    img.draw_border(0, 134, 275, 14, BORDER_LIGHT);
    img.fill_rect(1, 147, 273, 1, BORDER_DARK);
    img.fill_rect(4, 138, 2, 6, MINT);
    // Shade & Close glyphs on EQ titlebar
    img.fill_rect(256, 138, 5, 1, TEXT_SOFT);
    img.fill_rect(256, 140, 5, 1, TEXT_SOFT);
    img.fill_rect(256, 142, 5, 1, TEXT_SOFT);
    for d in 0..5u32 {
        img.set_pixel(266 + d, 138 + d, CLOSE_SOFT);
        img.set_pixel(270 - d, 138 + d, CLOSE_SOFT);
    }

    // Inactive titlebar strip (0, 149, 275x14)
    img.fill_v_gradient(0, 149, 275, 14, HEADER_INACTIVE_TOP, HEADER_INACTIVE_BOT);
    img.draw_border(0, 149, 275, 14, BORDER_SUBTLE);
    img.fill_rect(4, 153, 2, 6, MINT_DIM);
    img.fill_rect(256, 153, 5, 1, TEXT_MUTED);
    img.fill_rect(256, 155, 5, 1, TEXT_MUTED);
    img.fill_rect(256, 157, 5, 1, TEXT_MUTED);
    for d in 0..5u32 {
        img.set_pixel(266 + d, 153 + d, TEXT_MUTED);
        img.set_pixel(270 - d, 153 + d, TEXT_MUTED);
    }

    // EQ frequency spline graph recess (86, 17, 113x19)
    img.fill_rect(86, 17, 113, 19, LCD_BG);
    img.draw_border(85, 16, 115, 21, BORDER_SUBTLE);

    // Subtle dB scale markings (+12 / 0 / -12) between preamp and bands
    draw_micro_text_3x5(&mut img, 44, 39, "+12", TEXT_MUTED);
    draw_micro_text_3x5(&mut img, 52, 67, "0", TEXT_MUTED);
    draw_micro_text_3x5(&mut img, 44, 95, "-12", TEXT_MUTED);

    // Preamp & 10-band micro frequency labels directly below each fader
    draw_micro_text_3x5(&mut img, 22, 105, "PRE", TEXT_MUTED);
    const BAND_LABELS: [&str; 10] = [
        "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K",
    ];
    for (i, label) in BAND_LABELS.iter().enumerate() {
        let bx = 78 + (i as u32) * 18;
        let tw = (label.len() as u32) * 4 - 1;
        let lx = (bx + 7).saturating_sub(tw / 2);
        draw_micro_text_3x5(&mut img, lx, 105, label, TEXT_MUTED);
    }

    img
}

fn build_eq_ex_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(275, 82, CHASSIS_BOT);
    img.fill_v_gradient(0, 0, 275, 14, HEADER_ACTIVE_TOP, HEADER_ACTIVE_BOT);
    img.draw_border(0, 0, 275, 14, BORDER_LIGHT);
    img.fill_rect(4, 4, 2, 6, MINT);
    draw_window_glyphs(&mut img, 0, 0, true);

    img.fill_v_gradient(0, 15, 275, 14, HEADER_INACTIVE_TOP, HEADER_INACTIVE_BOT);
    img.draw_border(0, 15, 275, 14, BORDER_SUBTLE);
    img.fill_rect(4, 19, 2, 6, MINT_DIM);
    draw_window_glyphs(&mut img, 0, 15, false);
    img
}

fn build_pledit_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(280, 186, CHASSIS_BOT);
    // Active top bar pieces (y = 0..20)
    img.fill_v_gradient(0, 0, 280, 20, HEADER_ACTIVE_TOP, HEADER_ACTIVE_BOT);
    img.fill_rect(0, 0, 280, 1, BORDER_LIGHT);
    img.fill_rect(0, 19, 280, 1, BORDER_DARK);
    // Inactive top bar pieces (y = 21..41)
    img.fill_v_gradient(0, 21, 280, 20, HEADER_INACTIVE_TOP, HEADER_INACTIVE_BOT);
    img.fill_rect(0, 21, 280, 1, BORDER_SUBTLE);
    img.fill_rect(0, 40, 280, 1, BORDER_DARK);
    // Bottom bar pieces (y = 72..110)
    img.fill_v_gradient(0, 72, 280, 38, CHASSIS_TOP, CHASSIS_BOT);
    img.draw_border(0, 72, 280, 38, BORDER_SUBTLE);
    img
}
