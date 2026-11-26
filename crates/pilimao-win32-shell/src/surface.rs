//! Cross-platform 32-bit BGRA surface and 1x/2x/3x Nearest-Neighbor integer upscaler.

use pilimao_skin_wsz::{Rgb, SpriteImage, draw_bitmap_char_5x6};

/// Top-down 32-bit BGRA pixel buffer matching Win32 `BI_RGB` DIBSection layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgraSurface {
    pub width: u32,
    pub height: u32,
    /// Tightly packed `[B, G, R, A]` pixel bytes (`width * height * 4`).
    pub bgra: Vec<u8>,
}

impl BgraSurface {
    /// Allocates a new BGRA surface filled with `color`.
    #[must_use]
    pub fn filled(width: u32, height: u32, color: Rgb) -> Self {
        let count = (width as usize) * (height as usize);
        let mut bgra = Vec::with_capacity(count * 4);
        for _ in 0..count {
            bgra.extend_from_slice(&[color.b, color.g, color.r, 255]);
        }
        Self {
            width,
            height,
            bgra,
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, c: Rgb) {
        if x < self.width && y < self.height {
            let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
            self.bgra[idx] = c.b;
            self.bgra[idx + 1] = c.g;
            self.bgra[idx + 2] = c.r;
            self.bgra[idx + 3] = 255;
        }
    }

    #[inline]
    #[must_use]
    pub fn get_pixel_rgb(&self, x: u32, y: u32) -> Rgb {
        if x < self.width && y < self.height {
            let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
            Rgb::new(self.bgra[idx + 2], self.bgra[idx + 1], self.bgra[idx])
        } else {
            Rgb::new(0, 0, 0)
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: Rgb) {
        let max_x = (x.saturating_add(w)).min(self.width);
        let max_y = (y.saturating_add(h)).min(self.height);
        for py in y..max_y {
            for px in x..max_x {
                self.set_pixel(px, py, color);
            }
        }
    }

    pub fn draw_rect_outline(&mut self, x: u32, y: u32, w: u32, h: u32, color: Rgb) {
        if w == 0 || h == 0 {
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

    /// Blits a rectangular region from an RGBA [`SpriteImage`] onto this BGRA surface.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_sprite(
        &mut self,
        sprite: &SpriteImage,
        src_x: u32,
        src_y: u32,
        dst_x: u32,
        dst_y: u32,
        w: u32,
        h: u32,
    ) {
        for dy in 0..h {
            let sy = src_y + dy;
            let ty = dst_y + dy;
            if sy >= sprite.height || ty >= self.height {
                continue;
            }
            for dx in 0..w {
                let sx = src_x + dx;
                let tx = dst_x + dx;
                if sx >= sprite.width || tx >= self.width {
                    continue;
                }
                let px = sprite.get_pixel(sx, sy);
                if px[3] > 0 {
                    self.set_pixel(tx, ty, Rgb::new(px[0], px[1], px[2]));
                }
            }
        }
    }

    /// Draws ASCII text using the clean-room 5x6 pixel font clipped to `max_w` pixels.
    pub fn draw_ascii_text(&mut self, x: u32, y: u32, max_w: u32, text: &str, color: Rgb) {
        let mut cursor_x = x;
        let limit_x = x.saturating_add(max_w);
        let mut glyph = SpriteImage::filled(5, 6, Rgb::new(0, 0, 0));

        for ch in text.chars() {
            if cursor_x + 5 > limit_x {
                break;
            }
            glyph.rgba.fill(0);
            draw_bitmap_char_5x6(&mut glyph, 0, 0, ch, color);
            for dy in 0..6u32 {
                for dx in 0..5u32 {
                    let px = glyph.get_pixel(dx, dy);
                    if px[3] > 0 && (px[0] > 0 || px[1] > 0 || px[2] > 0) {
                        self.set_pixel(cursor_x + dx, y + dy, Rgb::new(px[0], px[1], px[2]));
                    }
                }
            }
            cursor_x += 6;
        }
    }

    /// Performs pixel-perfect Nearest-Neighbor integer upscaling (`1x`, `2x`, or `3x`).
    #[must_use]
    pub fn upscale_nearest(&self, scale: u32) -> Self {
        let s = scale.clamp(1, 3);
        if s == 1 {
            return self.clone();
        }
        let dst_w = self.width * s;
        let dst_h = self.height * s;
        let mut dst = vec![0u8; (dst_w as usize) * (dst_h as usize) * 4];

        let src_w = self.width as usize;
        let dst_w_us = dst_w as usize;
        let s_us = s as usize;

        for sy in 0..(self.height as usize) {
            let src_row = &self.bgra[sy * src_w * 4..(sy + 1) * src_w * 4];
            let first_dst_row_start = (sy * s_us) * dst_w_us * 4;
            let first_dst_row = &mut dst[first_dst_row_start..first_dst_row_start + dst_w_us * 4];

            for sx in 0..src_w {
                let pixel = &src_row[sx * 4..sx * 4 + 4];
                for rep in 0..s_us {
                    let off = (sx * s_us + rep) * 4;
                    first_dst_row[off..off + 4].copy_from_slice(pixel);
                }
            }

            // Duplicate the horizontally expanded row vertically `s - 1` times
            for vrep in 1..s_us {
                let dst_row_start = (sy * s_us + vrep) * dst_w_us * 4;
                dst.copy_within(
                    first_dst_row_start..first_dst_row_start + dst_w_us * 4,
                    dst_row_start,
                );
            }
        }

        Self {
            width: dst_w,
            height: dst_h,
            bgra: dst,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_neighbor_upscale_1x_2x_3x_is_pixel_exact() {
        let mut surf = BgraSurface::filled(4, 3, Rgb::new(10, 20, 30));
        surf.set_pixel(1, 2, Rgb::new(200, 150, 50));

        let s1 = surf.upscale_nearest(1);
        assert_eq!(s1.width, 4);
        assert_eq!(s1.height, 3);
        assert_eq!(s1.get_pixel_rgb(1, 2), Rgb::new(200, 150, 50));

        let s2 = surf.upscale_nearest(2);
        assert_eq!(s2.width, 8);
        assert_eq!(s2.height, 6);
        for dy in 0..2 {
            for dx in 0..2 {
                assert_eq!(s2.get_pixel_rgb(2 + dx, 4 + dy), Rgb::new(200, 150, 50));
            }
        }

        let s3 = surf.upscale_nearest(3);
        assert_eq!(s3.width, 12);
        assert_eq!(s3.height, 9);
        for dy in 0..3 {
            for dx in 0..3 {
                assert_eq!(s3.get_pixel_rgb(3 + dx, 6 + dy), Rgb::new(200, 150, 50));
            }
        }
    }
}
