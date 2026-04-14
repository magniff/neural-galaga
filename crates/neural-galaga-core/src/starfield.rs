use crate::constants::{GAME_HEIGHT, GAME_WIDTH};
use crate::framebuffer::Framebuffer;

const STAR_LAYERS: [(usize, f32, f32, f32); 3] = [
    (25, 5.0, 0.25, 1.0),
    (15, 12.0, 0.50, 1.0),
    (10, 25.0, 0.90, 1.0),
];

#[derive(Clone)]
struct Star {
    x: f32,
    y: f32,
    speed: f32,
    brightness: f32,
    size: f32,
    twinkle_phase: f32,
    twinkle_speed: f32,
}

#[derive(Clone)]
pub struct Starfield {
    stars: Vec<Star>,
}

impl Starfield {
    pub fn new() -> Self {
        let mut stars = Vec::new();
        let mut seed: u32 = 12345;
        let mut next = || -> u32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            seed
        };
        for &(count, speed, brightness, size) in &STAR_LAYERS {
            for i in 0..count {
                let x = (next() % (GAME_WIDTH as u32)) as f32;
                let y = (next() % (GAME_HEIGHT as u32)) as f32;
                let bright_var = brightness * (0.7 + 0.3 * (i as f32 / count as f32));
                let twinkle_phase = (next() % 1000) as f32 / 1000.0 * std::f32::consts::TAU;
                let twinkle_speed = 1.5 + (next() % 1000) as f32 / 1000.0 * 3.5;
                stars.push(Star {
                    x,
                    y,
                    speed,
                    brightness: bright_var,
                    size,
                    twinkle_phase,
                    twinkle_speed,
                });
            }
        }
        Self { stars }
    }

    pub fn update(&mut self, dt: f32) {
        for star in &mut self.stars {
            star.y += star.speed * dt;
            if star.y > GAME_HEIGHT {
                star.y -= GAME_HEIGHT + star.size;
            }
            star.twinkle_phase += star.twinkle_speed * dt;
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        for star in &self.stars {
            let twinkle = 0.7 + 0.3 * star.twinkle_phase.sin();
            let b = star.brightness * twinkle;
            fb.fill_rect(
                star.x as i32,
                star.y as i32,
                star.size as u32,
                star.size as u32,
                [b, b, (b * 1.1).min(1.0), 1.0],
            );
        }
    }
}

// --- Battle starfield with nebulas ---

/// Simple hash for deterministic noise.
fn hash_noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1103515245);
    h = h ^ (h >> 16);
    (h & 0xFFFF) as f32 / 65535.0
}

#[derive(Clone)]
struct Nebula {
    x: f32,
    y: f32,
    radius: f32,
    r: f32,
    g: f32,
    b: f32,
    alpha: f32,
    speed: f32,
    drift_phase: f32,
    drift_speed: f32,
    angle: f32,
    stretch: f32,
    noise_seed: u32,
    lobes: u8,
    lobe_phase: f32,
}

#[derive(Clone)]
pub struct BattleStarfield {
    stars: Starfield,
    nebulas: Vec<Nebula>,
}

impl BattleStarfield {
    pub fn new() -> Self {
        let mut seed: u32 = 99887;
        let mut rng = || -> f32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) as f32 / 65535.0
        };

        let nebula_colors: [(f32, f32, f32); 5] = [
            (0.4, 0.1, 0.7),  // purple
            (0.1, 0.2, 0.6),  // blue
            (0.6, 0.1, 0.2),  // red
            (0.1, 0.4, 0.4),  // teal
            (0.5, 0.3, 0.05), // amber
        ];

        let mut nebulas = Vec::new();

        for i in 0..10 {
            let ci = (rng() * nebula_colors.len() as f32) as usize % nebula_colors.len();
            let (r, g, b) = nebula_colors[ci];
            nebulas.push(Nebula {
                x: rng() * GAME_WIDTH,
                y: rng() * GAME_HEIGHT,
                radius: 25.0 + rng() * 50.0,
                r,
                g,
                b,
                alpha: 0.09 + rng() * 0.12,
                speed: 0.5 + rng() * 2.0,
                drift_phase: rng() * std::f32::consts::TAU,
                drift_speed: 0.2 + rng() * 0.4,
                angle: rng() * std::f32::consts::PI,
                stretch: 1.2 + rng() * 0.8,
                noise_seed: (i as u32 + 1) * 7919,
                lobes: 2 + (rng() * 4.0) as u8,
                lobe_phase: rng() * std::f32::consts::TAU,
            });
        }

        Self {
            stars: Starfield::new(),
            nebulas,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.stars.update(dt);
        for n in &mut self.nebulas {
            n.y += n.speed * dt;
            n.drift_phase += n.drift_speed * dt;
            if n.y - n.radius > GAME_HEIGHT {
                n.y -= GAME_HEIGHT + n.radius * 2.0;
            }
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        // Draw nebulas first (behind stars)
        for n in &self.nebulas {
            let drift_x = n.drift_phase.sin() * 10.0;
            let cx = n.x + drift_x;
            let cy = n.y;
            let outer = n.radius * n.stretch;

            let min_x = ((cx - outer).floor() as i32).clamp(0, fb.width as i32) as u32;
            let min_y = ((cy - outer).floor() as i32).clamp(0, fb.height as i32) as u32;
            let max_x = ((cx + outer).ceil() as i32 + 1).clamp(0, fb.width as i32) as u32;
            let max_y = ((cy + outer).ceil() as i32 + 1).clamp(0, fb.height as i32) as u32;

            let cos_a = n.angle.cos();
            let sin_a = n.angle.sin();

            for py in min_y..max_y {
                for px in min_x..max_x {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;

                    // Rotate into nebula's local frame and apply stretch
                    let lx = (cos_a * dx + sin_a * dy) / n.stretch;
                    let ly = -sin_a * dx + cos_a * dy;
                    let dist = (lx * lx + ly * ly).sqrt();

                    if dist >= n.radius {
                        continue;
                    }

                    let norm_dist = dist / n.radius;

                    // Wispy lobe modulation
                    let pixel_angle = ly.atan2(lx);
                    let lobe_mod = 1.0
                        + 0.3 * (pixel_angle * n.lobes as f32 + n.lobe_phase).sin()
                        + 0.15
                            * (pixel_angle * (n.lobes as f32 * 2.0 + 1.0) + n.lobe_phase * 1.7)
                                .sin();
                    let effective_dist = norm_dist / lobe_mod.max(0.3);
                    if effective_dist >= 1.0 {
                        continue;
                    }

                    // Hash noise for wispy texture
                    let noise = hash_noise(px as i32, py as i32, n.noise_seed);
                    let noise_factor = 0.5 + noise * 0.5;

                    let falloff = (1.0 - effective_dist) * (1.0 - effective_dist);
                    let a = n.alpha * falloff * noise_factor;

                    if a > 0.003 {
                        // Slight color variation from noise
                        let color_noise =
                            hash_noise(px as i32 + 1000, py as i32 + 1000, n.noise_seed);
                        let color_var = 0.85 + color_noise * 0.3;

                        let idx = ((py * fb.width + px) * 4) as usize;
                        let inv = 1.0 - a;
                        fb.pixels[idx] =
                            (n.r * color_var * 255.0 * a + fb.pixels[idx] as f32 * inv) as u8;
                        fb.pixels[idx + 1] =
                            (n.g * color_var * 255.0 * a + fb.pixels[idx + 1] as f32 * inv) as u8;
                        fb.pixels[idx + 2] =
                            (n.b * color_var * 255.0 * a + fb.pixels[idx + 2] as f32 * inv) as u8;
                    }
                }
            }
        }

        // Stars on top
        self.stars.draw(fb);
    }
}
