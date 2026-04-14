/// Software RGBA framebuffer for headless rendering.
#[derive(Clone)]
pub struct Framebuffer {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    /// Draw a filled rectangle with alpha blending.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: [f32; 4]) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x_end = x + w as i32;
        let y_end = y + h as i32;
        if x_end <= 0 || y_end <= 0 {
            return;
        } // fully off-screen
        let x1 = (x_end as u32).min(self.width);
        let y1 = (y_end as u32).min(self.height);

        let sr = (color[0] * 255.0) as u8;
        let sg = (color[1] * 255.0) as u8;
        let sb = (color[2] * 255.0) as u8;
        let sa = color[3];

        if sa >= 0.99 {
            // Fully opaque fast path
            for py in y0..y1 {
                for px in x0..x1 {
                    let idx = ((py * self.width + px) * 4) as usize;
                    self.pixels[idx] = sr;
                    self.pixels[idx + 1] = sg;
                    self.pixels[idx + 2] = sb;
                    self.pixels[idx + 3] = 255;
                }
            }
        } else if sa > 0.01 {
            // Alpha blend
            let inv_a = 1.0 - sa;
            for py in y0..y1 {
                for px in x0..x1 {
                    let idx = ((py * self.width + px) * 4) as usize;
                    self.pixels[idx] = (sr as f32 * sa + self.pixels[idx] as f32 * inv_a) as u8;
                    self.pixels[idx + 1] =
                        (sg as f32 * sa + self.pixels[idx + 1] as f32 * inv_a) as u8;
                    self.pixels[idx + 2] =
                        (sb as f32 * sa + self.pixels[idx + 2] as f32 * inv_a) as u8;
                    self.pixels[idx + 3] = 255;
                }
            }
        }
    }

    /// Draw a circle outline with alpha blending.
    pub fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, thickness: f32, color: [f32; 4]) {
        if color[3] < 0.01 {
            return;
        }
        let r_outer = radius + thickness * 0.5;
        let r_inner = radius - thickness * 0.5;
        let r_outer_sq = r_outer * r_outer;
        let r_inner_sq = r_inner * r_inner;

        let min_x = ((cx - r_outer).floor() as i32).max(0) as u32;
        let min_y = ((cy - r_outer).floor() as i32).max(0) as u32;
        let max_x = ((cx + r_outer).ceil() as i32 + 1).clamp(0, self.width as i32) as u32;
        let max_y = ((cy + r_outer).ceil() as i32 + 1).clamp(0, self.height as i32) as u32;

        let sr = (color[0] * 255.0) as u8;
        let sg = (color[1] * 255.0) as u8;
        let sb = (color[2] * 255.0) as u8;
        let sa = color[3];
        let inv_a = 1.0 - sa;

        for py in min_y..max_y {
            for px in min_x..max_x {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq >= r_inner_sq && dist_sq <= r_outer_sq {
                    let idx = ((py * self.width + px) * 4) as usize;
                    self.pixels[idx] = (sr as f32 * sa + self.pixels[idx] as f32 * inv_a) as u8;
                    self.pixels[idx + 1] =
                        (sg as f32 * sa + self.pixels[idx + 1] as f32 * inv_a) as u8;
                    self.pixels[idx + 2] =
                        (sb as f32 * sa + self.pixels[idx + 2] as f32 * inv_a) as u8;
                    self.pixels[idx + 3] = 255;
                }
            }
        }
    }
}
