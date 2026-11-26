//! Classic 3-window (Main / Equalizer / Playlist) software compositor for `.wsz` skins.

use pilimao_audio_engine::PlaybackState;
use pilimao_dsp::{EQ_MAX_DB, EQ_MIN_DB, EqSettings, SPECTRUM_BANDS, VisSnapshot};
use pilimao_playlist::{JumpMatch, Playlist, RepeatMode};
use pilimao_skin_wsz::{EQ_HEIGHT, EQ_WIDTH, MAIN_HEIGHT, MAIN_WIDTH, Rgb, SHADE_HEIGHT, WszSkin};

use crate::surface::BgraSurface;

/// Classic Winamp visualization display mode inside Main window's 76x16 LCD area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisMode {
    #[default]
    Spectrum,
    Oscilloscope,
    Off,
}

impl VisMode {
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            Self::Spectrum => Self::Oscilloscope,
            Self::Oscilloscope => Self::Off,
            Self::Off => Self::Spectrum,
        }
    }
}

/// Complete UI render state passed into [`ShellRenderer`].
#[derive(Debug, Clone)]
pub struct UiRenderState {
    pub title_marquee: String,
    pub marquee_offset: usize,
    pub playback_state: PlaybackState,
    pub elapsed_secs: u64,
    pub duration_secs: u64,
    pub bitrate_kbps: u32,
    pub sample_rate_khz: u32,
    pub is_stereo: bool,
    pub volume: f64,
    pub balance: f64,
    pub eq_visible: bool,
    pub pl_visible: bool,
    pub eq: EqSettings,
    pub bit_perfect: bool,
    pub vis_mode: VisMode,
    pub vis: VisSnapshot,
    pub pressed_transport: Option<usize>,
    pub jump_open: bool,
    pub jump_query: String,
    pub jump_matches: Vec<JumpMatch>,
    pub pl_scroll_offset: usize,
}

impl Default for UiRenderState {
    fn default() -> Self {
        Self {
            title_marquee: "PILIMAO 0.1 - READY TO PLAY".to_string(),
            marquee_offset: 0,
            playback_state: PlaybackState::Stopped,
            elapsed_secs: 0,
            duration_secs: 0,
            bitrate_kbps: 0,
            sample_rate_khz: 44,
            is_stereo: true,
            volume: 1.0,
            balance: 0.0,
            eq_visible: true,
            pl_visible: true,
            eq: EqSettings::flat(),
            bit_perfect: false,
            vis_mode: VisMode::Spectrum,
            vis: VisSnapshot::default(),
            pressed_transport: None,
            jump_open: false,
            jump_query: String::new(),
            jump_matches: Vec::new(),
            pl_scroll_offset: 0,
        }
    }
}

/// Composes Main, Equalizer, and Playlist windows from a [`WszSkin`] at 1x, 2x, or 3x scale.
pub struct ShellRenderer;

impl ShellRenderer {
    /// Renders the Main player window (Normal `275x116` or Windowshade `275x14`) scaled by `scale`.
    #[must_use]
    pub fn render_main(
        skin: &WszSkin,
        state: &UiRenderState,
        playlist: &Playlist,
        shaded: bool,
        active: bool,
        scale: u32,
    ) -> BgraSurface {
        let h = if shaded { SHADE_HEIGHT } else { MAIN_HEIGHT } as u32;
        let w = MAIN_WIDTH as u32;
        let mut surf = BgraSurface::filled(w, h, Rgb::new(16, 20, 26));

        if shaded {
            let ty = if active { 29 } else { 42 };
            surf.blit_sprite(&skin.sprites.titlebar, 27, ty, 0, 0, w, SHADE_HEIGHT as u32);
            let time_str = format!(
                "{:02}:{:02}",
                state.elapsed_secs / 60,
                state.elapsed_secs % 60
            );
            surf.draw_ascii_text(126, 4, 40, &time_str, Rgb::new(112, 240, 160));
            return surf.upscale_nearest(scale);
        }

        // 1. Base chassis & titlebar
        surf.blit_sprite(&skin.sprites.main, 0, 0, 0, 0, w, h);
        let tb_y = if active { 0 } else { 15 };
        surf.blit_sprite(&skin.sprites.titlebar, 27, tb_y, 0, 0, w, 14);
        surf.draw_ascii_text(10, 4, 90, "PILIMAO", Rgb::new(220, 240, 255));

        // 2. Play/Pause/Stop indicator glyph (24, 28, 9x9)
        let pp_x = match state.playback_state {
            PlaybackState::Playing => 0,
            PlaybackState::Paused => 9,
            PlaybackState::Stopped => 18,
        };
        surf.blit_sprite(&skin.sprites.playpaus, pp_x, 0, 24, 28, 9, 9);

        // 3. Time digits (4 digits: MM:SS via numbers sprite)
        let mins = (state.elapsed_secs / 60).min(99);
        let secs = state.elapsed_secs % 60;
        let digits = [
            (mins / 10) as u32,
            (mins % 10) as u32,
            (secs / 10) as u32,
            (secs % 10) as u32,
        ];
        let digit_xs = [48u32, 60, 78, 90];
        for (i, &d) in digits.iter().enumerate() {
            surf.blit_sprite(&skin.sprites.numbers, d * 9, 0, digit_xs[i], 26, 9, 13);
        }

        // 4. 76x16 Visualization area at (24, 43)
        render_vis_box(&mut surf, skin, state, 24, 43);

        // 5. Scrolling marquee title (111, 27, 152px)
        let marquee = build_marquee_slice(&state.title_marquee, state.marquee_offset, 25);
        surf.draw_ascii_text(112, 27, 152, &marquee, Rgb::new(112, 240, 160));

        // 6. Bitrate & Sample Rate LCD readouts
        let kbps = if state.bitrate_kbps > 0 {
            format!("{:03}", state.bitrate_kbps.min(999))
        } else {
            "---".to_string()
        };
        let khz = format!("{:02}", state.sample_rate_khz.min(99));
        surf.draw_ascii_text(111, 44, 18, &kbps, Rgb::new(112, 240, 160));
        surf.draw_ascii_text(156, 44, 14, &khz, Rgb::new(112, 240, 160));

        // Bit-Perfect LED badge
        if state.bit_perfect {
            surf.draw_ascii_text(176, 44, 30, "BP", Rgb::new(255, 212, 72));
        }

        // 7. Stereo / Mono LEDs (212, 41)
        let stereo_y = if state.is_stereo { 0 } else { 12 };
        let mono_y = if state.is_stereo { 12 } else { 0 };
        surf.blit_sprite(&skin.sprites.monoster, 0, stereo_y, 239, 41, 29, 12);
        surf.blit_sprite(&skin.sprites.monoster, 29, mono_y, 212, 41, 29, 12);

        // 8. Volume slider (107, 57, 68x13)
        let vol_frame = ((state.volume.clamp(0.0, 1.0) * 27.0).round() as u32).min(27);
        surf.blit_sprite(&skin.sprites.volume, 0, vol_frame * 15, 107, 57, 68, 13);
        let vol_thumb_x = 107 + ((state.volume.clamp(0.0, 1.0) * 51.0).round() as u32);
        surf.blit_sprite(&skin.sprites.volume, 15, 422, vol_thumb_x, 58, 14, 11);

        // 9. Balance slider (177, 57, 38x13)
        let bal_frame = ((state.balance.abs().clamp(0.0, 1.0) * 27.0).round() as u32).min(27);
        surf.blit_sprite(&skin.sprites.balance, 9, bal_frame * 15, 177, 57, 38, 13);
        let bal_norm = (state.balance.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let bal_thumb_x = 177 + ((bal_norm * 24.0).round() as u32);
        surf.blit_sprite(&skin.sprites.balance, 15, 422, bal_thumb_x, 58, 14, 11);

        // 10. EQ / PL toggles (219, 58)
        surf.blit_sprite(
            &skin.sprites.shufrep,
            0,
            if state.eq_visible { 73 } else { 61 },
            219,
            58,
            23,
            12,
        );
        surf.blit_sprite(
            &skin.sprites.shufrep,
            23,
            if state.pl_visible { 73 } else { 61 },
            242,
            58,
            23,
            12,
        );

        // 11. Seek Posbar (16, 72, 248x10)
        surf.blit_sprite(&skin.sprites.posbar, 0, 0, 16, 72, 248, 10);
        let seek_ratio = if state.duration_secs > 0 {
            (state.elapsed_secs as f64 / state.duration_secs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pos_thumb_x = 16 + ((seek_ratio * 219.0).round() as u32);
        surf.blit_sprite(&skin.sprites.posbar, 248, 0, pos_thumb_x, 72, 29, 10);

        // 12. Transport buttons (16, 88)
        let transport_defs = [
            (0u32, 23u32, 16u32),
            (23, 23, 39),
            (46, 23, 62),
            (69, 23, 85),
            (92, 22, 108),
            (114, 22, 136),
        ];
        for (idx, &(sx, sw, dx)) in transport_defs.iter().enumerate() {
            let sy = if state.pressed_transport == Some(idx) {
                18
            } else {
                0
            };
            surf.blit_sprite(&skin.sprites.cbuttons, sx, sy, dx, 88, sw, 18);
        }

        // 13. Shuffle & Repeat toggles (164, 89)
        surf.blit_sprite(
            &skin.sprites.shufrep,
            28,
            if playlist.is_shuffle() { 30 } else { 0 },
            164,
            89,
            47,
            15,
        );
        surf.blit_sprite(
            &skin.sprites.shufrep,
            0,
            if playlist.repeat_mode() != RepeatMode::Off {
                30
            } else {
                0
            },
            210,
            89,
            28,
            15,
        );

        surf.upscale_nearest(scale)
    }

    /// Renders the Equalizer window (`275x116` or Windowshade `275x14`) with live EQ sliders & curve.
    #[must_use]
    pub fn render_equalizer(
        skin: &WszSkin,
        state: &UiRenderState,
        shaded: bool,
        active: bool,
        scale: u32,
    ) -> BgraSurface {
        let w = EQ_WIDTH as u32;
        let h = if shaded { SHADE_HEIGHT } else { EQ_HEIGHT } as u32;
        let mut surf = BgraSurface::filled(w, h, Rgb::new(16, 20, 26));

        if shaded {
            let ty = if active { 0 } else { 15 };
            surf.blit_sprite(&skin.sprites.eq_ex, 0, ty, 0, 0, w, SHADE_HEIGHT as u32);
            surf.draw_ascii_text(8, 4, 120, "PILIMAO EQ", Rgb::new(220, 240, 255));
            return surf.upscale_nearest(scale);
        }

        surf.blit_sprite(&skin.sprites.eqmain, 0, 0, 0, 0, w, h);
        surf.blit_sprite(&skin.sprites.eqmain, 0, 134, 0, 0, w, 14);
        surf.draw_ascii_text(10, 4, 110, "EQUALIZER", Rgb::new(220, 240, 255));

        // ON indicator pill (14, 18, 25x12)
        let on_color = if state.eq.enabled {
            Rgb::new(82, 240, 148)
        } else {
            Rgb::new(48, 60, 72)
        };
        surf.fill_rect(14, 18, 25, 12, Rgb::new(20, 26, 36));
        surf.draw_rect_outline(14, 18, 25, 12, on_color);
        surf.draw_ascii_text(19, 21, 16, "ON", on_color);

        // Live 10-band EQ curve preview in (86, 17, 113x19)
        render_eq_curve(&mut surf, &state.eq, 86, 17, 113, 19);

        // Preamp slider (x=21, y=38..101)
        draw_eq_slider(&mut surf, 21, 38, state.eq.preamp_db);

        // 10 classic EQ band sliders (x=78 + i*18)
        for (i, &gain_db) in state.eq.bands_db.iter().enumerate() {
            draw_eq_slider(&mut surf, 78 + (i as u32) * 18, 38, gain_db);
        }

        surf.upscale_nearest(scale)
    }

    /// Renders the resizable Playlist Editor window (and optional Jump-to-File `J` overlay).
    #[must_use]
    pub fn render_playlist(
        skin: &WszSkin,
        state: &UiRenderState,
        playlist: &Playlist,
        logical_size: (u32, u32),
        shaded: bool,
        active: bool,
        scale: u32,
    ) -> BgraSurface {
        let (logical_w, logical_h) = logical_size;
        let w = logical_w.max(275);
        let h = if shaded {
            SHADE_HEIGHT as u32
        } else {
            logical_h.max(116)
        };
        let mut surf = BgraSurface::filled(w, h, skin.pledit_style.normal_bg);

        // Top title bar
        let tb_src_y = if active { 0 } else { 21 };
        let tb_h = if shaded { SHADE_HEIGHT as u32 } else { 20 };
        for tx in (0..w).step_by(25) {
            let bw = 25.min(w - tx);
            surf.blit_sprite(&skin.sprites.pledit, 0, tb_src_y, tx, 0, bw, tb_h);
        }
        surf.draw_ascii_text(
            10,
            4,
            w.saturating_sub(40),
            "PLAYLIST EDITOR",
            Rgb::new(220, 240, 255),
        );

        if shaded {
            return surf.upscale_nearest(scale);
        }

        // Frame borders & bottom toolbar (38px tall)
        let list_top = 20u32;
        let list_bottom = h.saturating_sub(38);
        surf.draw_rect_outline(0, 0, w, h, Rgb::new(68, 82, 102));
        surf.fill_rect(0, list_bottom, w, 38, Rgb::new(40, 48, 60));
        surf.draw_rect_outline(0, list_bottom, w, 38, Rgb::new(72, 86, 106));

        // Bottom status summary
        let footer = format!("TRACKS: {}  [Q]QUEUE [J]JUMP", playlist.len());
        surf.draw_ascii_text(
            10,
            list_bottom + 14,
            w.saturating_sub(20),
            &footer,
            Rgb::new(160, 212, 184),
        );

        // Track rows (9px each)
        let row_h = 9u32;
        let max_rows = (list_bottom.saturating_sub(list_top + 4) / row_h) as usize;
        let list_w = w.saturating_sub(18);

        if state.jump_open {
            // Jump-to-File (J) interactive fuzzy filter overlay
            surf.fill_rect(6, list_top + 2, list_w, 12, Rgb::new(22, 34, 52));
            surf.draw_rect_outline(6, list_top + 2, list_w, 12, Rgb::new(112, 232, 160));
            let prompt = format!("JUMP: {}_", state.jump_query);
            surf.draw_ascii_text(
                9,
                list_top + 5,
                list_w.saturating_sub(6),
                &prompt,
                Rgb::new(255, 255, 255),
            );

            for (row, m) in state
                .jump_matches
                .iter()
                .take(max_rows.saturating_sub(2))
                .enumerate()
            {
                let ry = list_top + 16 + (row as u32) * row_h;
                let text = format!("{}. {}", m.playlist_index + 1, m.display);
                let col = if row == 0 {
                    skin.pledit_style.current
                } else {
                    skin.pledit_style.normal
                };
                if row == 0 {
                    surf.fill_rect(6, ry, list_w, row_h, skin.pledit_style.selected_bg);
                }
                surf.draw_ascii_text(8, ry + 1, list_w.saturating_sub(4), &text, col);
            }
        } else {
            for row in 0..max_rows {
                let idx = state.pl_scroll_offset + row;
                let Some(entry) = playlist.entries().get(idx) else {
                    break;
                };
                let ry = list_top + 2 + (row as u32) * row_h;
                if playlist.selected_index() == Some(idx) {
                    surf.fill_rect(6, ry, list_w, row_h, skin.pledit_style.selected_bg);
                }
                let color = if playlist.current_index() == Some(idx) {
                    skin.pledit_style.current
                } else {
                    skin.pledit_style.normal
                };
                let q_badge = playlist
                    .queue_position(idx)
                    .map(|p| format!("[{p}] "))
                    .unwrap_or_default();
                let line = format!("{}. {q_badge}{}", idx + 1, entry.display_title());
                surf.draw_ascii_text(8, ry + 1, list_w.saturating_sub(4), &line, color);
            }
        }

        surf.upscale_nearest(scale)
    }
}

fn render_vis_box(surf: &mut BgraSurface, skin: &WszSkin, state: &UiRenderState, x: u32, y: u32) {
    let w = 76u32;
    let h = 16u32;
    surf.fill_rect(x, y, w, h, skin.vis_colors.background());

    // Classic subtle grid dots
    for gy in (1..h).step_by(2) {
        for gx in (1..w).step_by(2) {
            surf.set_pixel(x + gx, y + gy, skin.vis_colors.grid_dots());
        }
    }

    match state.vis_mode {
        VisMode::Off => {}
        VisMode::Spectrum => {
            // 19 classic spectrum bars, 3px wide + 1px gap = 76px exact!
            for band in 0..SPECTRUM_BANDS {
                let bx = x + (band as u32) * 4;
                let bar_h =
                    ((state.vis.bands[band].clamp(0.0, 1.0) * h as f32).round() as u32).min(h);
                for row in 0..bar_h {
                    let py = y + h - 1 - row;
                    let row_from_top = (h - 1 - row) as usize;
                    let col = skin.vis_colors.spectrum_row(row_from_top);
                    surf.set_pixel(bx, py, col);
                    surf.set_pixel(bx + 1, py, col);
                    surf.set_pixel(bx + 2, py, col);
                }
                let peak_h = ((state.vis.peaks[band].clamp(0.0, 1.0) * (h - 1) as f32).round()
                    as u32)
                    .min(h - 1);
                if peak_h > 0 {
                    let py = y + h - 1 - peak_h;
                    let peak_col = skin.vis_colors.peak_dot();
                    surf.set_pixel(bx, py, peak_col);
                    surf.set_pixel(bx + 1, py, peak_col);
                    surf.set_pixel(bx + 2, py, peak_col);
                }
            }
        }
        VisMode::Oscilloscope => {
            for (i, &sample) in state.vis.waveform.iter().enumerate().take(w as usize) {
                let norm = (1.0 - sample.clamp(-1.0, 1.0)) * 0.5;
                let py = y + ((norm * (h - 1) as f32).round() as u32).min(h - 1);
                let dist = (sample.abs() * 4.0).round() as usize;
                surf.set_pixel(x + i as u32, py, skin.vis_colors.oscilloscope(dist));
            }
        }
    }
}

fn draw_eq_slider(surf: &mut BgraSurface, x: u32, y: u32, gain_db: f64) {
    let norm = ((gain_db - EQ_MIN_DB) / (EQ_MAX_DB - EQ_MIN_DB)).clamp(0.0, 1.0);
    // Track fill colored green (0dB) -> amber/red (high) -> cyan (cut)
    let bar_col = if gain_db > 3.0 {
        Rgb::new(248, 124, 64)
    } else if gain_db < -3.0 {
        Rgb::new(64, 184, 240)
    } else {
        Rgb::new(82, 232, 140)
    };
    let thumb_y = y + (((1.0 - norm) * 51.0).round() as u32);
    surf.fill_rect(x + 2, y + 2, 10, 59, Rgb::new(14, 18, 26));
    // Fill from center (y + 31) to thumb
    let center_y = y + 31;
    let (top, bot) = if thumb_y < center_y {
        (thumb_y + 5, center_y)
    } else {
        (center_y, thumb_y + 5)
    };
    if bot > top {
        surf.fill_rect(x + 4, top, 6, bot - top, bar_col);
    }
    // Slider cap
    surf.fill_rect(x + 1, thumb_y, 12, 11, Rgb::new(104, 120, 140));
    surf.draw_rect_outline(x + 1, thumb_y, 12, 11, Rgb::new(200, 218, 238));
}

fn render_eq_curve(surf: &mut BgraSurface, eq: &EqSettings, x: u32, y: u32, w: u32, h: u32) {
    surf.fill_rect(x, y, w, h, Rgb::new(10, 14, 20));
    let mid_y = y + h / 2;
    for dx in 0..w {
        surf.set_pixel(x + dx, mid_y, Rgb::new(34, 48, 64));
    }
    let curve_col = if eq.enabled {
        Rgb::new(82, 240, 148)
    } else {
        Rgb::new(90, 104, 118)
    };
    for dx in 0..w {
        let pos = (dx as f64 / (w.saturating_sub(1).max(1)) as f64) * 9.0;
        let idx = (pos.floor() as usize).min(8);
        let frac = pos - idx as f64;
        let db = if eq.enabled {
            (eq.preamp_db + eq.bands_db[idx] * (1.0 - frac) + eq.bands_db[idx + 1] * frac)
                .clamp(EQ_MIN_DB, EQ_MAX_DB)
        } else {
            0.0
        };
        let norm = (db - EQ_MIN_DB) / (EQ_MAX_DB - EQ_MIN_DB);
        let py = y + (((1.0 - norm) * (h - 1) as f64).round() as u32).min(h - 1);
        surf.set_pixel(x + dx, py, curve_col);
    }
}

fn build_marquee_slice(text: &str, offset: usize, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let padded: Vec<char> = chars.iter().copied().chain("  ***  ".chars()).collect();
    let len = padded.len();
    (0..max_chars).map(|i| padded[(offset + i) % len]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_three_windows_at_1x_2x_3x() {
        let skin = WszSkin::clean_room_default();
        let state = UiRenderState::default();
        let pl = Playlist::new();

        for scale in [1u32, 2, 3] {
            let main = ShellRenderer::render_main(&skin, &state, &pl, false, true, scale);
            assert_eq!(main.width, 275 * scale);
            assert_eq!(main.height, 116 * scale);

            let main_ws = ShellRenderer::render_main(&skin, &state, &pl, true, true, scale);
            assert_eq!(main_ws.height, 14 * scale);

            let eq = ShellRenderer::render_equalizer(&skin, &state, false, true, scale);
            assert_eq!(eq.width, 275 * scale);
            assert_eq!(eq.height, 116 * scale);

            let pl_surf =
                ShellRenderer::render_playlist(&skin, &state, &pl, (300, 145), false, true, scale);
            assert_eq!(pl_surf.width, 300 * scale);
            assert_eq!(pl_surf.height, 145 * scale);
        }
    }
}
