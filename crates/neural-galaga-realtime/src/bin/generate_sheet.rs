use image::{Rgba, RgbaImage};

type Sprite16 = [[u8; 16]; 16];

const PALETTE: [[f32; 4]; 11] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.2, 0.9, 0.3, 1.0],
    [0.1, 0.8, 0.9, 1.0],
    [0.9, 0.1, 0.2, 1.0],
    [1.0, 0.6, 0.1, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 0.2, 1.0],
    [0.6, 0.2, 0.9, 1.0],
    [0.2, 0.4, 1.0, 1.0],
    [0.1, 0.5, 0.2, 1.0],
    [1.0, 0.2, 0.6, 1.0],
];

const PLAYER_SPRITE: Sprite16 = [
    [0, 0, 0, 0, 0, 0, 0, 5, 5, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 5, 2, 2, 5, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 5, 2, 2, 5, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 5, 1, 2, 2, 1, 5, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 5, 1, 1, 1, 1, 5, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 5, 1, 1, 1, 1, 1, 1, 5, 0, 0, 0, 0],
    [0, 0, 0, 0, 5, 1, 1, 1, 1, 1, 1, 5, 0, 0, 0, 0],
    [0, 0, 0, 5, 1, 1, 1, 5, 5, 1, 1, 1, 5, 0, 0, 0],
    [0, 0, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 5, 0, 0],
    [0, 0, 5, 1, 9, 1, 1, 1, 1, 1, 1, 9, 1, 5, 0, 0],
    [0, 5, 1, 1, 9, 1, 1, 1, 1, 1, 1, 9, 1, 1, 5, 0],
    [0, 5, 1, 1, 9, 9, 1, 1, 1, 1, 9, 9, 1, 1, 5, 0],
    [5, 2, 1, 1, 1, 9, 1, 1, 1, 1, 9, 1, 1, 1, 2, 5],
    [5, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 5],
    [0, 5, 2, 5, 0, 0, 0, 5, 5, 0, 0, 0, 5, 2, 5, 0],
    [0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0],
];

const ENEMY_A_F1: Sprite16 = [
    [0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 3, 0, 0, 3, 3, 0, 0, 3, 0, 0, 0, 0],
    [0, 0, 0, 3, 0, 0, 3, 6, 6, 3, 0, 0, 3, 0, 0, 0],
    [0, 0, 0, 3, 0, 3, 3, 3, 3, 3, 3, 0, 3, 0, 0, 0],
    [0, 0, 3, 6, 0, 3, 6, 3, 3, 6, 3, 0, 6, 3, 0, 0],
    [0, 3, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 6, 3, 3, 0],
    [3, 0, 3, 3, 3, 6, 6, 3, 3, 6, 6, 3, 3, 3, 0, 3],
    [3, 0, 0, 3, 3, 3, 3, 6, 6, 3, 3, 3, 3, 0, 0, 3],
    [3, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 3],
    [0, 3, 0, 0, 3, 3, 6, 3, 3, 6, 3, 3, 0, 0, 3, 0],
    [0, 0, 3, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 3, 0, 0],
    [0, 0, 0, 3, 0, 0, 3, 3, 3, 3, 0, 0, 3, 0, 0, 0],
    [0, 0, 0, 0, 3, 0, 0, 3, 3, 0, 0, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 3, 0, 0, 3, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

const ENEMY_A_F2: Sprite16 = [
    [0, 0, 0, 0, 0, 0, 0, 3, 3, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 3, 6, 6, 3, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 3, 3, 6, 3, 3, 6, 3, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0],
    [0, 0, 0, 3, 6, 3, 3, 3, 3, 3, 3, 6, 3, 0, 0, 0],
    [0, 0, 0, 3, 0, 6, 6, 3, 3, 6, 6, 0, 3, 0, 0, 0],
    [0, 0, 3, 0, 0, 3, 3, 6, 6, 3, 3, 0, 0, 3, 0, 0],
    [0, 0, 3, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 3, 0, 0],
    [0, 0, 0, 3, 0, 3, 6, 3, 3, 6, 3, 0, 3, 0, 0, 0],
    [0, 0, 0, 0, 3, 0, 3, 3, 3, 3, 0, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

const BULLET_SPRITE: [[u8; 4]; 8] = [
    [0, 5, 5, 0],
    [0, 2, 2, 0],
    [5, 2, 2, 5],
    [5, 2, 2, 5],
    [0, 2, 2, 0],
    [0, 2, 2, 0],
    [0, 5, 5, 0],
    [0, 0, 0, 0],
];

const ENEMY_BULLET_SPRITE: [[u8; 4]; 8] = [
    [0, 6, 6, 0],
    [6, 4, 4, 6],
    [0, 4, 4, 0],
    [0, 4, 4, 0],
    [0, 4, 4, 0],
    [6, 4, 4, 6],
    [0, 6, 6, 0],
    [0, 0, 0, 0],
];

const LIVES_ICON: [[u8; 8]; 8] = [
    [0, 0, 0, 5, 5, 0, 0, 0],
    [0, 0, 5, 2, 2, 5, 0, 0],
    [0, 0, 5, 1, 1, 5, 0, 0],
    [0, 5, 1, 1, 1, 1, 5, 0],
    [5, 1, 1, 5, 5, 1, 1, 5],
    [5, 1, 9, 1, 1, 9, 1, 5],
    [5, 2, 1, 1, 1, 1, 2, 5],
    [0, 5, 0, 5, 5, 0, 5, 0],
];

fn to_rgba(pidx: u8) -> Rgba<u8> {
    if pidx == 0 {
        Rgba([0, 0, 0, 0])
    } else {
        let p = PALETTE[pidx as usize];
        Rgba([
            (p[0] * 255.0).round() as u8,
            (p[1] * 255.0).round() as u8,
            (p[2] * 255.0).round() as u8,
            (p[3] * 255.0).round() as u8,
        ])
    }
}

fn hue_shift(r: f32, g: f32, b: f32, degrees: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta < 0.001 {
        return (r, g, b);
    }
    let mut h = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    if h < 0.0 {
        h += 360.0;
    }
    let s = delta / max;
    let v = max;
    h = (h + degrees) % 360.0;
    if h < 0.0 {
        h += 360.0;
    }
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r1 + m, g1 + m, b1 + m)
}

enum ColorTransform {
    None,
    HueShift(f32),
    White,
}

fn transform_pixel(px: Rgba<u8>, transform: &ColorTransform) -> Rgba<u8> {
    if px[3] == 0 {
        return px;
    }
    match transform {
        ColorTransform::None => px,
        ColorTransform::HueShift(deg) => {
            let (r, g, b) = hue_shift(
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
                *deg,
            );
            Rgba([
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                px[3],
            ])
        }
        ColorTransform::White => {
            let brightness = px[0] as f32 * 0.299 + px[1] as f32 * 0.587 + px[2] as f32 * 0.114;
            let v = (brightness / 255.0).powf(0.6);
            Rgba([
                (v * 255.0).round() as u8,
                (v * 255.0).round() as u8,
                (v * 255.0).round() as u8,
                px[3],
            ])
        }
    }
}

fn main() {
    let cell = 16u32;
    // Layout: player, yellow_f1, yellow_f2, red_f1, red_f2, green_f1, green_f2,
    //         bw_f1, bw_f2, bullet, enemy_bullet, lives_icon
    // 12 cols * 16 + 4 explosions * 24 + shotgun ball 4 + 6 powerups * 16 = 392 wide, 24px tall
    let img_w = 392u32;
    let img_h = 24u32;
    let mut img = RgbaImage::new(img_w, img_h);

    let write_16 = |img: &mut RgbaImage, ci: u32, sprite: &Sprite16, transform: &ColorTransform| {
        for (r, row) in sprite.iter().enumerate() {
            for (c, &pidx) in row.iter().enumerate() {
                let px = to_rgba(pidx);
                let px = transform_pixel(px, transform);
                img.put_pixel(ci * cell + c as u32, r as u32, px);
            }
        }
    };

    // Col 0: player
    write_16(&mut img, 0, &PLAYER_SPRITE, &ColorTransform::None);

    // Col 1-2: yellow enemy (unchanged)
    write_16(&mut img, 1, &ENEMY_A_F1, &ColorTransform::None);
    write_16(&mut img, 2, &ENEMY_A_F2, &ColorTransform::None);

    // Col 3-4: red enemy
    write_16(&mut img, 3, &ENEMY_A_F1, &ColorTransform::HueShift(40.0));
    write_16(&mut img, 4, &ENEMY_A_F2, &ColorTransform::HueShift(40.0));

    // Col 5-6: green enemy
    write_16(&mut img, 5, &ENEMY_A_F1, &ColorTransform::HueShift(120.0));
    write_16(&mut img, 6, &ENEMY_A_F2, &ColorTransform::HueShift(120.0));

    // Col 7-8: white enemy
    write_16(&mut img, 7, &ENEMY_A_F1, &ColorTransform::White);
    write_16(&mut img, 8, &ENEMY_A_F2, &ColorTransform::White);

    // Col 9: player bullet
    for (r, row) in BULLET_SPRITE.iter().enumerate() {
        for (c, &pidx) in row.iter().enumerate() {
            img.put_pixel(9 * cell + c as u32, r as u32, to_rgba(pidx));
        }
    }

    // Col 10: enemy bullet
    for (r, row) in ENEMY_BULLET_SPRITE.iter().enumerate() {
        for (c, &pidx) in row.iter().enumerate() {
            img.put_pixel(10 * cell + c as u32, r as u32, to_rgba(pidx));
        }
    }

    // Col 11: lives icon (8x8)
    for (r, row) in LIVES_ICON.iter().enumerate() {
        for (c, &pidx) in row.iter().enumerate() {
            img.put_pixel(11 * cell + c as u32, r as u32, to_rgba(pidx));
        }
    }

    // Explosion frames — 4 stages of expanding particles, 20x20 each at pixel x=192
    {
        let exp_size = 24u32;
        let exp_x0 = 192u32;
        let num_particles = 24;
        let center = exp_size as f32 / 2.0;

        let mut seed: u32 = 54321;
        let mut rng = || -> f32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) as f32 / 65535.0
        };

        let mut angles: Vec<f32> = Vec::new();
        let mut speeds: Vec<f32> = Vec::new();
        for _ in 0..num_particles {
            angles.push(rng() * std::f32::consts::TAU);
            speeds.push(0.3 + rng() * 0.7);
        }

        let exp_colors: [Rgba<u8>; 6] = [
            Rgba([255, 255, 100, 255]), // bright yellow
            Rgba([255, 220, 50, 255]),  // gold
            Rgba([255, 140, 30, 255]),  // orange
            Rgba([255, 80, 20, 255]),   // deep orange
            Rgba([255, 255, 255, 255]), // white-hot
            Rgba([100, 200, 255, 255]), // cyan spark
        ];

        for frame in 0..4u32 {
            let fx = exp_x0 + frame * exp_size;
            let spread = (frame as f32 + 0.5) / 3.5;
            let alpha_mult = 1.0 - frame as f32 * 0.15;

            for i in 0..num_particles {
                let dist = speeds[i] * spread * (exp_size as f32 / 2.0 - 1.0);
                let px = center + angles[i].cos() * dist;
                let py = center + angles[i].sin() * dist;
                let ix = px.round() as i32;
                let iy = py.round() as i32;

                let particle_size = if frame < 2 { 2 } else { 1 };

                let color_idx = i % exp_colors.len();
                let mut c = exp_colors[color_idx];
                c[3] = (c[3] as f32 * alpha_mult).round() as u8;

                for dy in 0..particle_size {
                    for dx in 0..particle_size {
                        let sx = ix + dx;
                        let sy = iy + dy;
                        if sx >= 0 && sx < exp_size as i32 && sy >= 0 && sy < exp_size as i32 {
                            img.put_pixel(fx + sx as u32, sy as u32, c);
                        }
                    }
                }
            }

            // Bright core for early frames
            if frame < 3 {
                let core_size = match frame {
                    0 => 6,
                    1 => 4,
                    _ => 2,
                };
                let core_start = (exp_size as i32 - core_size) / 2;
                let core_color = match frame {
                    0 => Rgba([255, 255, 240, 255]),
                    1 => Rgba([255, 240, 180, 240]),
                    _ => Rgba([255, 200, 100, 200]),
                };
                for dy in 0..core_size {
                    for dx in 0..core_size {
                        let sx = (core_start + dx) as u32;
                        let sy = (core_start + dy) as u32;
                        img.put_pixel(fx + sx, sy, core_color);
                    }
                }
            }
        }
    }

    // Shotgun ball sprite — 4x4 filled circle, bright magenta/white
    {
        let bx = 288u32;
        let center = 1.5f32; // center of 4x4
        for py in 0..4u32 {
            for px in 0..4u32 {
                let dx = px as f32 + 0.5 - (center + 0.5);
                let dy = py as f32 + 0.5 - (center + 0.5);
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= 2.0 {
                    let brightness = 1.0 - (dist / 2.0) * 0.3;
                    let r = (brightness * 255.0) as u8;
                    let g = (brightness * 200.0) as u8;
                    let b = (brightness * 255.0) as u8;
                    img.put_pixel(bx + px, py, Rgba([r, g, b, 255]));
                }
            }
        }
    }

    // Powerup sprites — 16x16 golden hexagons with text labels
    // Placed at x=292, 308, 324, 340, 356
    {
        let labels = ["+1", "+R", "+V", " 2", " 3", "+S"];
        let hex_color = Rgba([220, 180, 40, 255]); // gold border
        let bg_color = Rgba([60, 40, 10, 255]); // dark gold fill
        let text_color = Rgba([255, 255, 200, 255]); // bright gold text

        // Simple 3x5 pixel font for digits and letters
        let glyph = |ch: char| -> [[u8; 3]; 5] {
            match ch {
                '+' => [[0, 1, 0], [1, 1, 1], [0, 1, 0], [0, 0, 0], [0, 0, 0]],
                '1' => [[0, 1, 0], [1, 1, 0], [0, 1, 0], [0, 1, 0], [1, 1, 1]],
                '2' => [[1, 1, 0], [0, 0, 1], [0, 1, 0], [1, 0, 0], [1, 1, 1]],
                '3' => [[1, 1, 0], [0, 0, 1], [0, 1, 0], [0, 0, 1], [1, 1, 0]],
                'R' => [[1, 1, 0], [1, 0, 1], [1, 1, 0], [1, 0, 1], [1, 0, 1]],
                'V' => [[1, 0, 1], [1, 0, 1], [1, 0, 1], [0, 1, 0], [0, 1, 0]],
                'S' => [[0, 1, 1], [1, 0, 0], [0, 1, 0], [0, 0, 1], [1, 1, 0]],
                _ => [[0; 3]; 5],
            }
        };

        for (idx, label) in labels.iter().enumerate() {
            let bx = 292 + idx as u32 * 16;

            // Draw hexagon shape (pointy-top approximation in 16x16)
            for py in 0..16u32 {
                // Hexagon: narrow at top/bottom, wide in middle
                let row_from_center = (py as f32 - 7.5).abs();
                let half_width = if row_from_center < 4.0 {
                    7.0
                } else {
                    7.0 - (row_from_center - 3.0) * 1.8
                };
                let half_width = half_width.max(0.0);
                let x0 = (8.0 - half_width).round() as u32;
                let x1 = (8.0 + half_width).round() as u32;

                for px in x0..x1.min(16) {
                    // Border: outermost pixel of the hex
                    let is_border =
                        px == x0 || px == x1 - 1 || py == 0 || py == 15 || (row_from_center >= 3.0);
                    let c = if is_border && (px == x0 || px == x1 - 1 || row_from_center >= 5.5) {
                        hex_color
                    } else {
                        bg_color
                    };
                    img.put_pixel(bx + px, py, c);
                }
            }

            // Draw text — 2 characters, centered
            let chars: Vec<char> = label.chars().collect();
            let text_start_x = if chars.len() == 2 { 4u32 } else { 6 };
            for (ci, &ch) in chars.iter().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let g = glyph(ch);
                let cx = text_start_x + ci as u32 * 4;
                let cy = 6u32;
                for (gy, row) in g.iter().enumerate() {
                    for (gx, &pixel) in row.iter().enumerate() {
                        if pixel != 0 {
                            let sx = cx + gx as u32;
                            let sy = cy + gy as u32;
                            if sx < 16 && sy < 16 {
                                img.put_pixel(bx + sx, sy, text_color);
                            }
                        }
                    }
                }
            }
        }
    }

    let out_dir = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = format!("{out_dir}/sprites.png");
    img.save(&path).unwrap();
    println!("Generated {path} ({}x{})", img_w, img_h);
}
