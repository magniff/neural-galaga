use std::collections::HashMap;

use crate::constants::GAME_WIDTH;
use crate::framebuffer::Framebuffer;

const NATIVE_PX: f32 = 8.0;

#[derive(Clone)]
struct RasterizedGlyph {
    bitmap: Vec<u8>,
    width: usize,
    height: usize,
    advance_pixels: f32,
    y_offset: i32,
}

#[derive(Clone)]
pub struct BitmapFont {
    glyphs: HashMap<char, RasterizedGlyph>,
    font: fontdue::Font,
}

impl BitmapFont {
    pub fn load_from_memory(data: &[u8]) -> Self {
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .expect("Failed to parse font");
        Self {
            glyphs: HashMap::new(),
            font,
        }
    }

    fn get_glyph(&mut self, ch: char) -> &RasterizedGlyph {
        if !self.glyphs.contains_key(&ch) {
            let (metrics, bitmap) = self.font.rasterize(ch, NATIVE_PX);
            // ymin is the distance from the bottom of the glyph to the baseline (can be negative).
            // We need the offset from the top of the em-square to the top of the glyph bitmap.
            let y_offset = NATIVE_PX as i32 - metrics.height as i32 - metrics.ymin;
            let glyph = RasterizedGlyph {
                bitmap,
                width: metrics.width,
                height: metrics.height,
                advance_pixels: metrics.advance_width,
                y_offset,
            };
            self.glyphs.insert(ch, glyph);
        }
        &self.glyphs[&ch]
    }

    pub fn draw_text(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        x: f32,
        y: f32,
        scale: f32,
        color: [f32; 4],
    ) {
        let s = scale.round().max(1.0);
        let mut cursor_x = x.round();
        let y = y.round();
        for ch in text.chars() {
            let glyph = self.get_glyph(ch);
            let w = glyph.width;
            let h = glyph.height;
            let gy = y + glyph.y_offset as f32 * s;
            for row in 0..h {
                for col in 0..w {
                    let alpha = glyph.bitmap[row * w + col];
                    if alpha < 128 {
                        continue;
                    }
                    fb.fill_rect(
                        (cursor_x + col as f32 * s) as i32,
                        (gy + row as f32 * s) as i32,
                        s as u32,
                        s as u32,
                        color,
                    );
                }
            }
            cursor_x += (glyph.advance_pixels * s).round();
        }
    }

    pub fn text_width(&mut self, text: &str, scale: f32) -> f32 {
        let s = scale.round().max(1.0);
        text.chars()
            .map(|ch| (self.get_glyph(ch).advance_pixels * s).round())
            .sum()
    }

    pub fn draw_text_centered(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        y: f32,
        scale: f32,
        color: [f32; 4],
    ) {
        let w = self.text_width(text, scale);
        let x = ((GAME_WIDTH - w) / 2.0).round();
        self.draw_text(fb, text, x, y, scale, color);
    }
}
