//! 100% Clean-Room procedurally generated Pilimao Classic default skin.
//!
//! All sprite sheets match classic Winamp 2.x sprite layout coordinates while containing
//! zero third-party or copyrighted bitmap data.

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

    /// Draws a crisp retro 3D beveled frame (raised if `raised == true`, sunken otherwise).
    pub fn draw_bevel(&mut self, x: u32, y: u32, w: u32, h: u32, raised: bool) {
        if w < 2 || h < 2 {
            return;
        }
        let light = if raised {
            Rgb::new(108, 122, 140)
        } else {
            Rgb::new(14, 18, 24)
        };
        let dark = if raised {
            Rgb::new(16, 20, 28)
        } else {
            Rgb::new(92, 106, 124)
        };
        for dx in 0..w {
            self.set_pixel(x + dx, y, light);
            self.set_pixel(x + dx, y + h - 1, dark);
        }
        for dy in 0..h {
            self.set_pixel(x, y + dy, light);
            self.set_pixel(x + w - 1, y + dy, dark);
        }
    }

    /// Draws a horizontal metallic gradient rectangle.
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
    /// Generates the 100% Clean-Room default "Pilimao Classic" skin in pure Rust.
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
            source_name: "Pilimao Clean-Room Default".to_string(),
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
            name: "Pilimao Classic (Clean-Room)".to_string(),
            sprites,
            vis_colors: VisColors::clean_room(),
            pledit_style: PleditStyle::clean_room(),
            regions: RegionData::default(),
            report,
        }
    }
}

const CHASSIS_TOP: Rgb = Rgb::new(56, 66, 80);
const CHASSIS_BOT: Rgb = Rgb::new(34, 41, 52);
const LCD_BG: Rgb = Rgb::new(10, 14, 20);
const LED_GREEN: Rgb = Rgb::new(82, 240, 148);
const LED_DIM: Rgb = Rgb::new(30, 56, 46);
const ACCENT_AMBER: Rgb = Rgb::new(248, 184, 56);
const BTN_TOP: Rgb = Rgb::new(78, 90, 108);
const BTN_BOT: Rgb = Rgb::new(48, 56, 70);
const BTN_PRESSED: Rgb = Rgb::new(36, 44, 56);

fn build_main_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(275, 116, CHASSIS_BOT);
    img.fill_v_gradient(0, 0, 275, 116, CHASSIS_TOP, CHASSIS_BOT);
    img.draw_bevel(0, 0, 275, 116, true);

    // Subtle horizontal brushed-metal grooves
    for y in (16..114).step_by(4) {
        for x in 3..272 {
            let px = img.get_pixel(x, y);
            img.set_pixel(
                x,
                y,
                Rgb::new(
                    px[0].saturating_sub(4),
                    px[1].saturating_sub(4),
                    px[2].saturating_sub(3),
                ),
            );
        }
    }

    // Left LCD panel (play state + time + 76x16 spectrum area)
    img.fill_rect(10, 22, 92, 42, LCD_BG);
    img.draw_bevel(9, 21, 94, 44, false);

    // Right LCD marquee title strip (111, 24, 154x12)
    img.fill_rect(110, 23, 155, 14, LCD_BG);
    img.draw_bevel(109, 22, 157, 16, false);

    // kbps & kHz LCD wells
    img.fill_rect(110, 42, 18, 9, LCD_BG);
    img.draw_bevel(109, 41, 20, 11, false);
    img.fill_rect(155, 42, 14, 9, LCD_BG);
    img.draw_bevel(154, 41, 16, 11, false);

    // Stereo / Mono recess
    img.fill_rect(211, 40, 57, 14, LCD_BG);
    img.draw_bevel(210, 39, 59, 16, false);

    // Posbar track recess (16, 72, 248x10)
    img.fill_rect(16, 72, 248, 10, LCD_BG);
    img.draw_bevel(15, 71, 250, 12, false);

    img
}

fn build_titlebar_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(344, 87, CHASSIS_BOT);
    // Active titlebar at (27, 0, 275x14)
    img.fill_v_gradient(27, 0, 275, 14, Rgb::new(42, 94, 148), Rgb::new(24, 52, 86));
    img.draw_bevel(27, 0, 275, 14, true);
    // Inactive titlebar at (27, 15, 275x14)
    img.fill_v_gradient(27, 15, 275, 14, Rgb::new(54, 62, 74), Rgb::new(36, 42, 52));
    img.draw_bevel(27, 15, 275, 14, true);
    // Windowshade active at (27, 29, 275x14)
    img.fill_v_gradient(27, 29, 275, 14, Rgb::new(36, 82, 130), Rgb::new(20, 44, 74));
    img.draw_bevel(27, 29, 275, 14, true);
    // Windowshade inactive at (27, 42, 275x14)
    img.fill_v_gradient(27, 42, 275, 14, Rgb::new(50, 58, 68), Rgb::new(34, 40, 48));
    img.draw_bevel(27, 42, 275, 14, true);

    // Decorative titlebar accent rails
    for x in 78..236 {
        img.set_pixel(x, 4, Rgb::new(110, 178, 232));
        img.set_pixel(x, 7, Rgb::new(72, 132, 192));
        img.set_pixel(x, 10, Rgb::new(110, 178, 232));
    }

    // Window buttons (minimize, shade, close) on active titlebar
    for &(bx, col) in &[
        (27 + 244, LED_GREEN),
        (27 + 254, ACCENT_AMBER),
        (27 + 264, Rgb::new(240, 88, 88)),
    ] {
        img.fill_rect(bx, 3, 9, 9, BTN_TOP);
        img.draw_bevel(bx, 3, 9, 9, true);
        img.fill_rect(bx + 3, 6, 3, 3, col);
    }

    img
}

fn build_cbuttons_sprite() -> SpriteImage {
    // 136x36: row 0 (y=0..18) normal, row 1 (y=18..36) pressed
    // Prev(0..23), Play(23..46), Pause(46..69), Stop(69..92), Next(92..114), Eject(114..136)
    let mut img = SpriteImage::filled(136, 36, CHASSIS_BOT);
    let buttons = [(0, 23), (23, 23), (46, 23), (69, 23), (92, 22), (114, 22)];

    for (idx, &(bx, bw)) in buttons.iter().enumerate() {
        // Normal
        img.fill_v_gradient(bx, 0, bw, 18, BTN_TOP, BTN_BOT);
        img.draw_bevel(bx, 0, bw, 18, true);
        draw_transport_glyph(&mut img, idx, bx, 0, LED_GREEN);

        // Pressed
        img.fill_rect(bx, 18, bw, 18, BTN_PRESSED);
        img.draw_bevel(bx, 18, bw, 18, false);
        draw_transport_glyph(&mut img, idx, bx + 1, 19, ACCENT_AMBER);
    }
    img
}

fn draw_transport_glyph(img: &mut SpriteImage, idx: usize, bx: u32, by: u32, col: Rgb) {
    match idx {
        0 => {
            // Prev: |<<
            img.fill_rect(bx + 6, by + 5, 2, 8, col);
            for d in 0..4 {
                img.fill_rect(bx + 9 + d, by + 9 - d, 1, d * 2 + 1, col);
                img.fill_rect(bx + 13 + d, by + 9 - d, 1, d * 2 + 1, col);
            }
        }
        1 => {
            // Play: >
            for d in 0..6 {
                img.fill_rect(bx + 8 + d, by + 4 + d / 2, 1, 10 - d, col);
            }
        }
        2 => {
            // Pause: ||
            img.fill_rect(bx + 7, by + 5, 3, 8, col);
            img.fill_rect(bx + 12, by + 5, 3, 8, col);
        }
        3 => {
            // Stop: []
            img.fill_rect(bx + 7, by + 5, 8, 8, col);
        }
        4 => {
            // Next: >>|
            for d in 0..4 {
                img.fill_rect(bx + 5 + d, by + 5 + d, 1, 9 - d * 2, col);
                img.fill_rect(bx + 9 + d, by + 5 + d, 1, 9 - d * 2, col);
            }
            img.fill_rect(bx + 14, by + 5, 2, 8, col);
        }
        _ => {
            // Eject: ^ over _
            for d in 0..4 {
                img.fill_rect(bx + 10 - d, by + 4 + d, d * 2 + 1, 1, col);
            }
            img.fill_rect(bx + 6, by + 10, 9, 2, col);
        }
    }
}

fn build_volume_sprite() -> SpriteImage {
    // 28 frames of 68x13 stacked vertically (y = frame * 15) + thumb at y = 422
    let mut img = SpriteImage::filled(68, 433, CHASSIS_BOT);
    for frame in 0..28u32 {
        let y0 = frame * 15;
        let t = frame as f32 / 27.0;
        let bar_col = Rgb::new(
            lerp_u8(48, 240, t),
            lerp_u8(224, 72, t),
            lerp_u8(116, 48, t),
        );
        img.fill_rect(0, y0, 68, 13, LCD_BG);
        img.draw_bevel(0, y0, 68, 13, false);
        let fill_w = ((frame + 1) * 64 / 28).min(64);
        img.fill_rect(2, y0 + 3, fill_w, 7, bar_col);
    }
    // Thumb normal (15, 422, 14x11) & pressed (0, 422, 14x11)
    img.fill_v_gradient(15, 422, 14, 11, BTN_TOP, BTN_BOT);
    img.draw_bevel(15, 422, 14, 11, true);
    img.fill_rect(0, 422, 14, 11, BTN_PRESSED);
    img.draw_bevel(0, 422, 14, 11, false);
    img
}

fn build_balance_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(68, 433, CHASSIS_BOT);
    for frame in 0..28u32 {
        let y0 = frame * 15;
        let t = frame as f32 / 27.0;
        let col = Rgb::new(
            lerp_u8(60, 248, t),
            lerp_u8(220, 148, t),
            lerp_u8(140, 48, t),
        );
        img.fill_rect(9, y0, 38, 13, LCD_BG);
        img.draw_bevel(9, y0, 38, 13, false);
        img.fill_rect(11, y0 + 4, 34, 5, col);
    }
    img.fill_v_gradient(15, 422, 14, 11, BTN_TOP, BTN_BOT);
    img.draw_bevel(15, 422, 14, 11, true);
    img.fill_rect(0, 422, 14, 11, BTN_PRESSED);
    img.draw_bevel(0, 422, 14, 11, false);
    img
}

fn build_posbar_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(307, 10, LCD_BG);
    img.draw_bevel(0, 0, 248, 10, false);
    img.fill_rect(2, 4, 244, 2, Rgb::new(34, 62, 86));
    // Thumb normal at (248, 0, 29x10), pressed at (278, 0, 29x10)
    img.fill_v_gradient(248, 0, 29, 10, BTN_TOP, BTN_BOT);
    img.draw_bevel(248, 0, 29, 10, true);
    img.fill_rect(261, 2, 3, 6, LED_GREEN);
    img.fill_rect(278, 0, 29, 10, BTN_PRESSED);
    img.draw_bevel(278, 0, 29, 10, false);
    img.fill_rect(291, 2, 3, 6, ACCENT_AMBER);
    img
}

fn build_shufrep_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(92, 85, CHASSIS_BOT);
    // Repeat (0,0, 28x15) off/on, Shuffle (28,0, 47x15) off/on, EQ (0,61, 23x12), PL (23,61, 23x12)
    for &(x, y, w, h, lit) in &[
        (0, 0, 28, 15, false),
        (0, 30, 28, 15, true),
        (28, 0, 47, 15, false),
        (28, 30, 47, 15, true),
        (0, 61, 23, 12, false),
        (0, 73, 23, 12, true),
        (23, 61, 23, 12, false),
        (23, 73, 23, 12, true),
    ] {
        img.fill_v_gradient(x, y, w, h, BTN_TOP, BTN_BOT);
        img.draw_bevel(x, y, w, h, true);
        img.fill_rect(x + 3, y + 3, 4, 3, if lit { LED_GREEN } else { LED_DIM });
    }
    img
}

fn build_monoster_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(58, 24, LCD_BG);
    // Stereo lit (0,0,29x12), dim (0,12,29x12); Mono lit (29,0,29x12), dim (29,12,29x12)
    img.fill_rect(3, 4, 23, 4, LED_GREEN);
    img.fill_rect(3, 16, 23, 4, LED_DIM);
    img.fill_rect(32, 4, 23, 4, LED_GREEN);
    img.fill_rect(32, 16, 23, 4, LED_DIM);
    img
}

fn build_playpaus_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(42, 9, LCD_BG);
    // Play (0,0, 9x9)
    for d in 0..4 {
        img.fill_rect(2 + d, 1 + d, 1, 7 - d * 2, LED_GREEN);
    }
    // Pause (9,0, 9x9)
    img.fill_rect(11, 1, 2, 7, ACCENT_AMBER);
    img.fill_rect(15, 1, 2, 7, ACCENT_AMBER);
    // Stop (18,0, 9x9)
    img.fill_rect(20, 2, 5, 5, Rgb::new(235, 92, 92));
    img
}

fn build_numbers_sprite() -> SpriteImage {
    // 12 glyphs of 9x13: '0'..'9', ' ', '-'
    let mut img = SpriteImage::filled(108, 13, LCD_BG);
    // 7-segment bitmask: [top, top-right, bot-right, bot, bot-left, top-left, mid]
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
        if mask & 0b0000_0001 != 0 {
            img.fill_rect(ox + 2, 1, 5, 1, LED_GREEN);
        }
        if mask & 0b0000_0010 != 0 {
            img.fill_rect(ox + 7, 2, 1, 4, LED_GREEN);
        }
        if mask & 0b0000_0100 != 0 {
            img.fill_rect(ox + 7, 7, 1, 4, LED_GREEN);
        }
        if mask & 0b0000_1000 != 0 {
            img.fill_rect(ox + 2, 11, 5, 1, LED_GREEN);
        }
        if mask & 0b0001_0000 != 0 {
            img.fill_rect(ox + 1, 7, 1, 4, LED_GREEN);
        }
        if mask & 0b0010_0000 != 0 {
            img.fill_rect(ox + 1, 2, 1, 4, LED_GREEN);
        }
        if mask & 0b0100_0000 != 0 {
            img.fill_rect(ox + 2, 6, 5, 1, LED_GREEN);
        }
    }
    img
}

fn build_text_sprite() -> SpriteImage {
    // Classic Winamp text.bmp layout: 155x18 (3 rows x 31 glyphs of 5x6)
    let mut img = SpriteImage::filled(155, 18, LCD_BG);
    let rows: [&str; 3] = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@   ",
        "0123456789 ...:()-'!_+\\/[]^&%.=",
        "                               ",
    ];
    for (row_idx, row_chars) in rows.iter().enumerate() {
        for (col_idx, ch) in row_chars.chars().enumerate().take(31) {
            draw_bitmap_char_5x6(
                &mut img,
                col_idx as u32 * 5,
                row_idx as u32 * 6,
                ch,
                LED_GREEN,
            );
        }
    }
    img
}

/// Minimal clean-room 5x6 uppercase/digit bitmap font renderer.
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
    img.draw_bevel(0, 0, 275, 116, true);
    // Titlebar strip (0, 134, 275x14)
    img.fill_v_gradient(0, 134, 275, 14, Rgb::new(42, 94, 148), Rgb::new(24, 52, 86));
    img.draw_bevel(0, 134, 275, 14, true);
    // EQ frequency spline graph recess (86, 17, 113x19)
    img.fill_rect(86, 17, 113, 19, LCD_BG);
    img.draw_bevel(85, 16, 115, 21, false);
    // Slider tracks (preamp at x=21, 10 bands starting at x=78 step 18, y=38..101)
    img.fill_rect(21, 38, 14, 63, LCD_BG);
    img.draw_bevel(20, 37, 16, 65, false);
    for b in 0..10u32 {
        let bx = 78 + b * 18;
        img.fill_rect(bx, 38, 14, 63, LCD_BG);
        img.draw_bevel(bx - 1, 37, 16, 65, false);
    }
    img
}

fn build_eq_ex_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(275, 82, CHASSIS_BOT);
    img.fill_v_gradient(0, 0, 275, 14, Rgb::new(36, 82, 130), Rgb::new(20, 44, 74));
    img.draw_bevel(0, 0, 275, 14, true);
    img.fill_v_gradient(0, 15, 275, 14, Rgb::new(50, 58, 68), Rgb::new(34, 40, 48));
    img.draw_bevel(0, 15, 275, 14, true);
    img
}

fn build_pledit_sprite() -> SpriteImage {
    let mut img = SpriteImage::filled(280, 186, CHASSIS_BOT);
    // Active top bar pieces (y = 0..20)
    img.fill_v_gradient(0, 0, 280, 20, Rgb::new(42, 94, 148), Rgb::new(24, 52, 86));
    img.draw_bevel(0, 0, 280, 20, true);
    // Inactive top bar pieces (y = 21..41)
    img.fill_v_gradient(0, 21, 280, 20, Rgb::new(54, 62, 74), Rgb::new(36, 42, 52));
    img.draw_bevel(0, 21, 280, 20, true);
    // Bottom bar pieces (y = 72..110)
    img.fill_v_gradient(0, 72, 280, 38, CHASSIS_TOP, CHASSIS_BOT);
    img.draw_bevel(0, 72, 280, 38, true);
    img
}
