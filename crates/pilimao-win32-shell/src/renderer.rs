//! Classic 3-window (Main / Equalizer / Playlist) software compositor for `.wsz` skins.

use pilimao_audio_engine::PlaybackState;
use pilimao_dsp::{EQ_MAX_DB, EQ_MIN_DB, EqSettings, SPECTRUM_BANDS, VisSnapshot};
use pilimao_playlist::{JumpMatch, Playlist, RepeatMode};
use pilimao_skin_wsz::{EQ_HEIGHT, EQ_WIDTH, MAIN_HEIGHT, MAIN_WIDTH, Rgb, SHADE_HEIGHT, WszSkin};

use crate::surface::BgraSurface;

const UI_MINT: Rgb = Rgb::new(94, 234, 212);
const UI_TITLE_ACTIVE: Rgb = Rgb::new(224, 232, 240);
const UI_TITLE_INACTIVE: Rgb = Rgb::new(124, 136, 150);
const UI_MUTED: Rgb = Rgb::new(112, 124, 138);
const UI_AMBER: Rgb = Rgb::new(245, 178, 68);
const UI_BORDER: Rgb = Rgb::new(48, 56, 68);
const UI_BORDER_LIGHT: Rgb = Rgb::new(62, 72, 86);
const UI_LCD_BG: Rgb = Rgb::new(12, 15, 19);
const UI_PANEL_BG: Rgb = Rgb::new(24, 28, 34);
const UI_CLOSE: Rgb = Rgb::new(232, 114, 114);

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
            title_marquee: "DROP AUDIO FILES OR PRESS [L]".to_string(),
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
        let mut surf = BgraSurface::filled(w, h, UI_PANEL_BG);
        let title_col = if active {
            UI_TITLE_ACTIVE
        } else {
            UI_TITLE_INACTIVE
        };

        if shaded {
            let ty = if active { 29 } else { 42 };
            surf.blit_sprite(&skin.sprites.titlebar, 27, ty, 0, 0, w, SHADE_HEIGHT as u32);
            surf.draw_micro_text(10, 5, 80, "PILIMAO", title_col);
            let time_str = format!(
                "{:02}:{:02}",
                state.elapsed_secs / 60,
                state.elapsed_secs % 60
            );
            surf.draw_micro_text(122, 5, 36, &time_str, UI_MINT);
            return surf.upscale_nearest(scale);
        }

        // 1. Base chassis & titlebar
        surf.blit_sprite(&skin.sprites.main, 0, 0, 0, 0, w, h);
        let tb_y = if active { 0 } else { 15 };
        surf.blit_sprite(&skin.sprites.titlebar, 27, tb_y, 0, 0, w, 14);
        surf.draw_micro_text(10, 5, 96, "PILIMAO", title_col);

        // 2. Play/Pause/Stop indicator glyph (20, 26, 9x9)
        let pp_x = match state.playback_state {
            PlaybackState::Playing => 0,
            PlaybackState::Paused => 9,
            PlaybackState::Stopped => 18,
        };
        surf.blit_sprite(&skin.sprites.playpaus, pp_x, 0, 20, 26, 9, 9);

        // 3. Time digits (MM:SS) with crisp colon dots
        let mins = (state.elapsed_secs / 60).min(99);
        let secs = state.elapsed_secs % 60;
        let digits = [
            (mins / 10) as u32,
            (mins % 10) as u32,
            (secs / 10) as u32,
            (secs % 10) as u32,
        ];
        let digit_xs = [46u32, 58, 76, 88];
        for (i, &d) in digits.iter().enumerate() {
            surf.blit_sprite(&skin.sprites.numbers, d * 9, 0, digit_xs[i], 25, 9, 13);
        }
        // Crisp 2x2 colon dots between MM and SS
        surf.fill_rect(71, 28, 2, 2, UI_MINT);
        surf.fill_rect(71, 33, 2, 2, UI_MINT);

        // 4. 76x16 Visualization area centered cleanly inside left OLED panel
        render_vis_box(&mut surf, skin, state, 18, 43);

        // 5. Scrolling marquee title inside right OLED strip (112, 26, 150px)
        let marquee = build_marquee_slice(&state.title_marquee, state.marquee_offset, 25);
        surf.draw_ascii_text(112, 26, 150, &marquee, UI_MINT);

        // 6. Bitrate & Sample Rate LCD readouts (micro 3x5 digits fit cleanly inside wells)
        let kbps = if state.bitrate_kbps > 0 {
            format!("{:03}", state.bitrate_kbps.min(999))
        } else {
            "---".to_string()
        };
        let khz = format!("{:02}", state.sample_rate_khz.min(99));
        surf.draw_micro_text(113, 44, 16, &kbps, UI_MINT);
        surf.draw_micro_text(159, 44, 12, &khz, UI_MINT);

        // Bit-Perfect subtle amber indicator pill
        if state.bit_perfect {
            surf.draw_micro_text(192, 44, 16, "BP", UI_AMBER);
        }

        // 7. Stereo / Mono indicators (212, 40)
        let stereo_y = if state.is_stereo { 0 } else { 12 };
        let mono_y = if state.is_stereo { 12 } else { 0 };
        surf.blit_sprite(&skin.sprites.monoster, 0, stereo_y, 238, 40, 29, 12);
        surf.blit_sprite(&skin.sprites.monoster, 29, mono_y, 211, 40, 29, 12);

        // 8. Volume slider (107, 57, 68x13)
        let vol_frame = ((state.volume.clamp(0.0, 1.0) * 27.0).round() as u32).min(27);
        surf.blit_sprite(&skin.sprites.volume, 0, vol_frame * 15, 107, 57, 68, 13);
        let vol_thumb_x = 107 + ((state.volume.clamp(0.0, 1.0) * 54.0).round() as u32);
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

        // 11. Seek Posbar (16, 72, 248x10) with live progress mint fill
        surf.blit_sprite(&skin.sprites.posbar, 0, 0, 16, 72, 248, 10);
        let seek_ratio = if state.duration_secs > 0 {
            (state.elapsed_secs as f64 / state.duration_secs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fill_w = (seek_ratio * 242.0).round() as u32;
        if fill_w > 0 {
            surf.fill_rect(19, 76, fill_w, 2, UI_MINT);
        }
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
            214,
            89,
            28,
            15,
        );

        // Repeat One subtle indicator dot
        if playlist.repeat_mode() == RepeatMode::One {
            surf.draw_micro_text(245, 94, 16, "1", UI_AMBER);
        }

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
        let mut surf = BgraSurface::filled(w, h, UI_PANEL_BG);
        let title_col = if active {
            UI_TITLE_ACTIVE
        } else {
            UI_TITLE_INACTIVE
        };

        if shaded {
            let ty = if active { 0 } else { 15 };
            surf.blit_sprite(&skin.sprites.eq_ex, 0, ty, 0, 0, w, SHADE_HEIGHT as u32);
            surf.draw_micro_text(10, 5, 120, "EQUALIZER", title_col);
            return surf.upscale_nearest(scale);
        }

        surf.blit_sprite(&skin.sprites.eqmain, 0, 0, 0, 0, w, h);
        let tb_y = if active { 134 } else { 149 };
        surf.blit_sprite(&skin.sprites.eqmain, 0, tb_y, 0, 0, w, 14);
        surf.draw_micro_text(10, 5, 110, "EQUALIZER", title_col);

        // Sleek ON toggle pill (14, 18, 25x12)
        let on_fg = if state.eq.enabled { UI_MINT } else { UI_MUTED };
        let on_border = if state.eq.enabled { UI_MINT } else { UI_BORDER };
        surf.fill_rect(14, 18, 25, 12, Rgb::new(28, 33, 40));
        surf.draw_rect_outline(14, 18, 25, 12, on_border);
        if state.eq.enabled {
            surf.fill_rect(16, 19, 21, 1, UI_MINT);
        }
        surf.draw_micro_text(22, 22, 12, "ON", on_fg);

        // Live 10-band EQ curve preview in (86, 17, 113x19)
        render_eq_curve(&mut surf, &state.eq, 86, 17, 113, 19);

        // Preamp slider (x=21, y=38..101)
        draw_eq_slider(&mut surf, 21, 38, state.eq.preamp_db, state.eq.enabled);

        // 10 classic EQ band sliders (x=78 + i*18)
        for (i, &gain_db) in state.eq.bands_db.iter().enumerate() {
            draw_eq_slider(
                &mut surf,
                78 + (i as u32) * 18,
                38,
                gain_db,
                state.eq.enabled,
            );
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
        let title_col = if active {
            UI_TITLE_ACTIVE
        } else {
            UI_TITLE_INACTIVE
        };

        // Top title bar
        let tb_src_y = if active { 0 } else { 21 };
        let tb_h = if shaded { SHADE_HEIGHT as u32 } else { 16 };
        for tx in (0..w).step_by(25) {
            let bw = 25.min(w - tx);
            surf.blit_sprite(&skin.sprites.pledit, 0, tb_src_y, tx, 0, bw, tb_h);
        }
        surf.fill_rect(4, 5, 2, 6, if active { UI_MINT } else { UI_BORDER });
        surf.draw_micro_text(10, 5, w.saturating_sub(42), "PLAYLIST", title_col);

        // Minimalist Shade & Close glyphs on Playlist titlebar
        let icon_col = if active { UI_TITLE_ACTIVE } else { UI_MUTED };
        let close_col = if active { UI_CLOSE } else { UI_MUTED };
        surf.fill_rect(w - 20, 4, 5, 1, icon_col);
        surf.fill_rect(w - 20, 6, 5, 1, icon_col);
        surf.fill_rect(w - 20, 8, 5, 1, icon_col);
        for d in 0..5u32 {
            surf.set_pixel(w - 10 + d, 4 + d, close_col);
            surf.set_pixel(w - 6 - d, 4 + d, close_col);
        }

        if shaded {
            surf.draw_rect_outline(0, 0, w, h, UI_BORDER_LIGHT);
            return surf.upscale_nearest(scale);
        }

        // Sleek outer border & bottom status footer (24px tall)
        let list_top = 18u32;
        let footer_h = 24u32;
        let list_bottom = h.saturating_sub(footer_h);
        surf.fill_rect(0, list_bottom, w, footer_h, UI_PANEL_BG);
        surf.fill_rect(1, list_bottom, w - 2, 1, UI_BORDER);
        surf.draw_rect_outline(0, 0, w, h, UI_BORDER_LIGHT);

        // Subtle diagonal resize grip in bottom-right corner
        for i in 0..3u32 {
            let offset = i * 3;
            for p in 0..(9 - offset) {
                surf.set_pixel(w - 4 - p, h - 12 + offset + p, UI_MUTED);
            }
        }

        // Bottom status summary
        let footer = format!("{} TRACKS   [L]OPEN  [J]JUMP  [Q]QUEUE", playlist.len());
        surf.draw_micro_text(8, list_bottom + 10, w.saturating_sub(24), &footer, UI_MUTED);

        // Track rows (10px each) + slim right scrollbar
        let row_h = 10u32;
        let max_rows = (list_bottom.saturating_sub(list_top + 2) / row_h) as usize;
        let list_w = w.saturating_sub(14);

        // Scrollbar track
        let sb_x = w - 8;
        let sb_h = list_bottom.saturating_sub(list_top + 4);
        if sb_h > 8 {
            surf.fill_rect(sb_x, list_top + 2, 4, sb_h, Rgb::new(22, 26, 32));
            let total = playlist.len().max(1);
            let thumb_h = ((max_rows as u32 * sb_h) / total as u32).clamp(8, sb_h);
            let max_scroll = playlist.len().saturating_sub(max_rows).max(1);
            let thumb_y = list_top
                + 2
                + ((state.pl_scroll_offset.min(max_scroll) as u32 * (sb_h - thumb_h))
                    / max_scroll as u32);
            surf.fill_rect(sb_x, thumb_y, 4, thumb_h, UI_BORDER_LIGHT);
        }

        if state.jump_open {
            surf.fill_rect(6, list_top + 1, list_w, 13, Rgb::new(22, 30, 38));
            surf.draw_rect_outline(6, list_top + 1, list_w, 13, UI_MINT);
            let prompt = format!("JUMP: {}_", state.jump_query);
            surf.draw_micro_text(
                9,
                list_top + 5,
                list_w.saturating_sub(6),
                &prompt,
                UI_TITLE_ACTIVE,
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
                surf.draw_micro_text(8, ry + 2, list_w.saturating_sub(4), &text, col);
            }
        } else if playlist.is_empty() {
            let cy = list_top + list_bottom.saturating_sub(list_top) / 2;
            surf.draw_micro_text(
                14,
                cy.saturating_sub(6),
                list_w.saturating_sub(14),
                "NO TRACKS LOADED",
                UI_TITLE_ACTIVE,
            );
            surf.draw_micro_text(
                14,
                cy + 3,
                list_w.saturating_sub(14),
                "DROP FILES OR CLICK EJECT / [L]",
                UI_MUTED,
            );
        } else {
            for row in 0..max_rows {
                let idx = state.pl_scroll_offset + row;
                let Some(entry) = playlist.entries().get(idx) else {
                    break;
                };
                let ry = list_top + 2 + (row as u32) * row_h;
                if playlist.selected_index() == Some(idx) {
                    surf.fill_rect(5, ry, list_w, row_h, skin.pledit_style.selected_bg);
                }
                let is_current = playlist.current_index() == Some(idx);
                let color = if is_current {
                    skin.pledit_style.current
                } else {
                    skin.pledit_style.normal
                };
                if is_current {
                    // Subtle active left accent bar
                    surf.fill_rect(5, ry + 1, 2, row_h - 2, UI_MINT);
                }

                let dur_str = entry.formatted_duration();
                let dur_w = if dur_str.is_empty() {
                    0
                } else {
                    (dur_str.len() as u32) * 4 + 4
                };
                if !dur_str.is_empty() {
                    surf.draw_micro_text(
                        5 + list_w.saturating_sub(dur_w),
                        ry + 2,
                        dur_w,
                        &dur_str,
                        if is_current { UI_MINT } else { UI_MUTED },
                    );
                }

                let q_badge = playlist
                    .queue_position(idx)
                    .map(|p| format!("[{p}] "))
                    .unwrap_or_default();
                let line = format!("{}. {q_badge}{}", idx + 1, entry.display_title());
                surf.draw_micro_text(9, ry + 2, list_w.saturating_sub(dur_w + 8), &line, color);
            }
        }

        surf.upscale_nearest(scale)
    }
}

fn render_vis_box(surf: &mut BgraSurface, skin: &WszSkin, state: &UiRenderState, x: u32, y: u32) {
    let w = 76u32;
    let h = 16u32;
    surf.fill_rect(x, y, w, h, skin.vis_colors.background());

    // Ultra-subtle baseline hairline for clean hi-fi look
    surf.fill_rect(x, y + h - 1, w, 1, skin.vis_colors.grid_dots());

    match state.vis_mode {
        VisMode::Off => {}
        VisMode::Spectrum => {
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

fn draw_eq_slider(surf: &mut BgraSurface, x: u32, y: u32, gain_db: f64, enabled: bool) {
    let norm = ((gain_db - EQ_MIN_DB) / (EQ_MAX_DB - EQ_MIN_DB)).clamp(0.0, 1.0);
    // Slim centered vertical groove (4px wide)
    surf.fill_rect(x + 5, y + 1, 4, 61, UI_LCD_BG);
    surf.draw_rect_outline(x + 4, y, 6, 63, UI_BORDER);
    // Subtle 0 dB center tick
    surf.fill_rect(x + 2, y + 31, 10, 1, UI_BORDER);

    let bar_col = if !enabled {
        Rgb::new(62, 72, 84)
    } else if gain_db > 6.0 {
        UI_AMBER
    } else {
        UI_MINT
    };

    let thumb_y = y + (((1.0 - norm) * 53.0).round() as u32);
    let center_y = y + 31;
    let (top, bot) = if thumb_y + 4 < center_y {
        (thumb_y + 4, center_y)
    } else {
        (center_y, thumb_y + 4)
    };
    if bot > top {
        surf.fill_rect(x + 6, top, 2, bot - top, bar_col);
    }

    // Refined slim horizontal fader cap (12x9)
    surf.fill_rect(x + 1, thumb_y, 12, 9, Rgb::new(56, 64, 76));
    surf.draw_rect_outline(x + 1, thumb_y, 12, 9, UI_BORDER_LIGHT);
    surf.fill_rect(
        x + 3,
        thumb_y + 4,
        8,
        1,
        if enabled { UI_MINT } else { UI_MUTED },
    );
}

fn render_eq_curve(surf: &mut BgraSurface, eq: &EqSettings, x: u32, y: u32, w: u32, h: u32) {
    surf.fill_rect(x, y, w, h, UI_LCD_BG);
    let mid_y = y + h / 2;
    for dx in (0..w).step_by(2) {
        surf.set_pixel(x + dx, mid_y, Rgb::new(34, 42, 52));
    }
    let curve_col = if eq.enabled { UI_MINT } else { UI_MUTED };
    let mut prev_py: Option<u32> = None;
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
        // Connect vertical steps smoothly so the curve never breaks into scattered dots
        if let Some(prev) = prev_py {
            let (lo, hi) = if prev < py { (prev, py) } else { (py, prev) };
            for cy in lo..=hi {
                surf.set_pixel(x + dx, cy, curve_col);
            }
        } else {
            surf.set_pixel(x + dx, py, curve_col);
        }
        prev_py = Some(py);
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
