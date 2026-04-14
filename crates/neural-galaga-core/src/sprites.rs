use crate::framebuffer::Framebuffer;

pub struct SpriteRegion {
    /// Pixel x-offset in the sprite sheet.
    pub x: u32,
    pub width: u32,
    pub height: u32,
}

// Column helper — first 12 columns are 16px-wide cells
const fn col(c: u32) -> u32 {
    c * 16
}

pub const REGION_PLAYER: SpriteRegion = SpriteRegion {
    x: col(0),
    width: 16,
    height: 16,
};

pub const REGION_YELLOW_F1: SpriteRegion = SpriteRegion {
    x: col(1),
    width: 16,
    height: 16,
};
pub const REGION_YELLOW_F2: SpriteRegion = SpriteRegion {
    x: col(2),
    width: 16,
    height: 16,
};
pub const REGION_RED_F1: SpriteRegion = SpriteRegion {
    x: col(3),
    width: 16,
    height: 16,
};
pub const REGION_RED_F2: SpriteRegion = SpriteRegion {
    x: col(4),
    width: 16,
    height: 16,
};
pub const REGION_GREEN_F1: SpriteRegion = SpriteRegion {
    x: col(5),
    width: 16,
    height: 16,
};
pub const REGION_GREEN_F2: SpriteRegion = SpriteRegion {
    x: col(6),
    width: 16,
    height: 16,
};
pub const REGION_BW_F1: SpriteRegion = SpriteRegion {
    x: col(7),
    width: 16,
    height: 16,
};
pub const REGION_BW_F2: SpriteRegion = SpriteRegion {
    x: col(8),
    width: 16,
    height: 16,
};

pub const REGION_BULLET: SpriteRegion = SpriteRegion {
    x: col(9),
    width: 4,
    height: 8,
};
pub const REGION_ENEMY_BULLET: SpriteRegion = SpriteRegion {
    x: col(10),
    width: 4,
    height: 8,
};
pub const REGION_LIVES_ICON: SpriteRegion = SpriteRegion {
    x: col(11),
    width: 8,
    height: 8,
};

// Explosion frames — 24x24 each, placed after the 12 columns (at pixel 192)
pub const REGION_EXPLODE_1: SpriteRegion = SpriteRegion {
    x: 192,
    width: 24,
    height: 24,
};
pub const REGION_EXPLODE_2: SpriteRegion = SpriteRegion {
    x: 216,
    width: 24,
    height: 24,
};
pub const REGION_EXPLODE_3: SpriteRegion = SpriteRegion {
    x: 240,
    width: 24,
    height: 24,
};
pub const REGION_EXPLODE_4: SpriteRegion = SpriteRegion {
    x: 264,
    width: 24,
    height: 24,
};

pub const EXPLOSION_REGIONS: [&SpriteRegion; 4] = [
    &REGION_EXPLODE_1,
    &REGION_EXPLODE_2,
    &REGION_EXPLODE_3,
    &REGION_EXPLODE_4,
];

// Shotgun ball — 4x4, after explosion frames
pub const REGION_SHOTGUN_BALL: SpriteRegion = SpriteRegion {
    x: 288,
    width: 4,
    height: 4,
};

// Powerup sprites — 16x16 each, starting at x=292
pub const REGION_POWERUP_LIFE: SpriteRegion = SpriteRegion {
    x: 292,
    width: 16,
    height: 16,
};
pub const REGION_POWERUP_RATE: SpriteRegion = SpriteRegion {
    x: 308,
    width: 16,
    height: 16,
};
pub const REGION_POWERUP_SPEED: SpriteRegion = SpriteRegion {
    x: 324,
    width: 16,
    height: 16,
};
pub const REGION_POWERUP_DOUBLE: SpriteRegion = SpriteRegion {
    x: 340,
    width: 16,
    height: 16,
};
pub const REGION_POWERUP_TRIPLE: SpriteRegion = SpriteRegion {
    x: 356,
    width: 16,
    height: 16,
};
pub const REGION_POWERUP_SHIELD: SpriteRegion = SpriteRegion {
    x: 372,
    width: 16,
    height: 16,
};

// Aliases for menu rendering
pub const REGION_ENEMY_A_F1: SpriteRegion = SpriteRegion {
    x: col(1),
    width: 16,
    height: 16,
};
pub const REGION_ENEMY_A_F2: SpriteRegion = SpriteRegion {
    x: col(2),
    width: 16,
    height: 16,
};
pub const REGION_BOSS_F1: SpriteRegion = SpriteRegion {
    x: col(1),
    width: 16,
    height: 16,
};
pub const REGION_ENEMY_B_F1: SpriteRegion = SpriteRegion {
    x: col(3),
    width: 16,
    height: 16,
};

#[derive(Clone)]
pub struct SpriteSheet {
    pixels: Vec<[f32; 4]>,
    width: u32,
}

impl SpriteSheet {
    pub fn load_from_memory(data: &[u8]) -> Self {
        let img = image::load_from_memory(data)
            .expect("Failed to decode sprite sheet")
            .to_rgba8();
        let (w, _h) = img.dimensions();
        let pixels: Vec<[f32; 4]> = img
            .pixels()
            .map(|p| {
                [
                    p[0] as f32 / 255.0,
                    p[1] as f32 / 255.0,
                    p[2] as f32 / 255.0,
                    p[3] as f32 / 255.0,
                ]
            })
            .collect();
        Self { pixels, width: w }
    }

    pub fn draw(&self, fb: &mut Framebuffer, region: &SpriteRegion, x: f32, y: f32, scale: f32) {
        let base_x = region.x;
        let x = x.round() as i32;
        let y = y.round() as i32;
        for row in 0..region.height {
            for col in 0..region.width {
                let idx = (row * self.width + base_x + col) as usize;
                let color = self.pixels[idx];
                if color[3] < 0.01 {
                    continue;
                }
                fb.fill_rect(
                    x + (col as f32 * scale) as i32,
                    y + (row as f32 * scale) as i32,
                    scale.max(1.0) as u32,
                    scale.max(1.0) as u32,
                    color,
                );
            }
        }
    }

    /// Draw a sprite rotated by `angle` radians around its center.
    pub fn draw_rotated(
        &self,
        fb: &mut Framebuffer,
        region: &SpriteRegion,
        x: f32,
        y: f32,
        scale: f32,
        angle: f32,
    ) {
        if angle.abs() < 0.01 {
            return self.draw(fb, region, x, y, scale);
        }

        let base_x = region.x;
        let sw = region.width as f32;
        let sh = region.height as f32;
        let cx = sw * 0.5;
        let cy = sh * 0.5;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let half_diag = (cx * cx + cy * cy).sqrt() * scale;
        let dest_cx = x + cx * scale;
        let dest_cy = y + cy * scale;
        let min_x = (dest_cx - half_diag).floor() as i32;
        let min_y = (dest_cy - half_diag).floor() as i32;
        let max_x = (dest_cx + half_diag).ceil() as i32;
        let max_y = (dest_cy + half_diag).ceil() as i32;

        for py in min_y..max_y {
            for px in min_x..max_x {
                let dx = (px as f32 + 0.5 - dest_cx) / scale;
                let dy = (py as f32 + 0.5 - dest_cy) / scale;
                let sx = cos_a * dx + sin_a * dy + cx;
                let sy = -sin_a * dx + cos_a * dy + cy;

                let si = sx.floor() as i32;
                let sj = sy.floor() as i32;
                if si < 0 || sj < 0 || si >= sw as i32 || sj >= sh as i32 {
                    continue;
                }

                let idx = (sj as u32 * self.width + base_x + si as u32) as usize;
                let color = self.pixels[idx];
                if color[3] < 0.01 {
                    continue;
                }
                fb.fill_rect(px, py, 1, 1, color);
            }
        }
    }

    pub fn draw_tinted(
        &self,
        fb: &mut Framebuffer,
        region: &SpriteRegion,
        x: f32,
        y: f32,
        scale: f32,
        tint: [f32; 4],
    ) {
        let base_x = region.x;
        let x = x.round() as i32;
        let y = y.round() as i32;
        for row in 0..region.height {
            for col in 0..region.width {
                let idx = (row * self.width + base_x + col) as usize;
                if self.pixels[idx][3] < 0.01 {
                    continue;
                }
                fb.fill_rect(
                    x + (col as f32 * scale) as i32,
                    y + (row as f32 * scale) as i32,
                    scale.max(1.0) as u32,
                    scale.max(1.0) as u32,
                    tint,
                );
            }
        }
    }
}

pub fn enemy_sprite_region(row: usize, anim_frame: bool) -> &'static SpriteRegion {
    match (row, anim_frame) {
        (0, false) => &REGION_YELLOW_F1,
        (0, true) => &REGION_YELLOW_F2,
        (1, false) => &REGION_RED_F1,
        (1, true) => &REGION_RED_F2,
        (2, false) => &REGION_GREEN_F1,
        (2, true) => &REGION_GREEN_F2,
        (3, false) => &REGION_BW_F1,
        (3, true) => &REGION_BW_F2,
        (_, false) => &REGION_YELLOW_F1,
        (_, true) => &REGION_YELLOW_F2,
    }
}
