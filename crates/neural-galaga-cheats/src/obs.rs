//! Encode `StepResult` into a fixed-size float observation vector.
//!
//! All positions normalized to roughly [-1, 1] (centered on screen midpoint).
//! Velocities normalized by their respective max bullet speeds.
//! Relative features (dx/dy to player) included for all entities.
//! Enemy bullets and shotgun balls sorted by distance to player (closest first).
//! Danger columns encode the minimum time-to-impact per screen column.

use neural_galaga_core::{
    BULLET_SPEED, ENEMY_BULLET_SPEED, GAME_HEIGHT, GAME_WIDTH, PLAYER_SPEED, StepResult,
    shield::ShieldLevel,
};

// --- Player block ---
/// x, y, speed, proj_speed, shield, lives, wave_progress, vulnerable,
/// fire_rate_stacks, speed_stacks, bullet_count,
/// inv_rate, inv_speed, inv_double, inv_triple, inv_shield, currency
pub const PLAYER_FLOATS: usize = 17;

// --- Enemies block ---
pub const NUM_ENEMY_SLOTS: usize = 36;
/// valid, x, y, class×4, shield, is_diving, rel_x, rel_y
pub const ENEMY_SLOT_FLOATS: usize = 11;
pub const ENEMY_BLOCK_FLOATS: usize = NUM_ENEMY_SLOTS * ENEMY_SLOT_FLOATS;

// --- Player bullets ---
pub const NUM_PLAYER_BULLET_SLOTS: usize = 8;
/// valid, x, y, rel_x, rel_y
pub const PLAYER_BULLET_SLOT_FLOATS: usize = 5;
pub const PLAYER_BULLET_BLOCK_FLOATS: usize = NUM_PLAYER_BULLET_SLOTS * PLAYER_BULLET_SLOT_FLOATS;

// --- Enemy bullets (sorted by distance to player) ---
pub const NUM_ENEMY_BULLET_SLOTS: usize = 32;
/// valid, x, y, dy_norm, rel_x, rel_y, time_to_impact
pub const ENEMY_BULLET_SLOT_FLOATS: usize = 7;
pub const ENEMY_BULLET_BLOCK_FLOATS: usize = NUM_ENEMY_BULLET_SLOTS * ENEMY_BULLET_SLOT_FLOATS;

// --- Shotgun balls (sorted by distance to player) ---
pub const NUM_SHOTGUN_SLOTS: usize = 12;
/// valid, x, y, dx_norm, dy_norm, rel_x, rel_y, time_to_impact
pub const SHOTGUN_SLOT_FLOATS: usize = 8;
pub const SHOTGUN_BLOCK_FLOATS: usize = NUM_SHOTGUN_SLOTS * SHOTGUN_SLOT_FLOATS;

// --- Powerups ---
pub const NUM_POWERUP_SLOTS: usize = 4;
/// valid, x, y, kind_onehot×4
pub const POWERUP_SLOT_FLOATS: usize = 7;
pub const POWERUP_BLOCK_FLOATS: usize = NUM_POWERUP_SLOTS * POWERUP_SLOT_FLOATS;

// --- Danger columns ---
/// Number of discrete screen columns for the danger map.
pub const NUM_DANGER_COLS: usize = 14;
pub const DANGER_BLOCK_FLOATS: usize = NUM_DANGER_COLS;

/// Total observation size for a single frame.
pub const OBS_SIZE: usize = PLAYER_FLOATS
    + ENEMY_BLOCK_FLOATS
    + PLAYER_BULLET_BLOCK_FLOATS
    + ENEMY_BULLET_BLOCK_FLOATS
    + SHOTGUN_BLOCK_FLOATS
    + POWERUP_BLOCK_FLOATS
    + DANGER_BLOCK_FLOATS;

// Block offsets in the flat observation vector.
pub const PLAYER_OFFSET: usize = 0;
pub const ENEMIES_OFFSET: usize = PLAYER_OFFSET + PLAYER_FLOATS;
pub const PLAYER_BULLETS_OFFSET: usize = ENEMIES_OFFSET + ENEMY_BLOCK_FLOATS;
pub const ENEMY_BULLETS_OFFSET: usize = PLAYER_BULLETS_OFFSET + PLAYER_BULLET_BLOCK_FLOATS;
pub const SHOTGUN_OFFSET: usize = ENEMY_BULLETS_OFFSET + ENEMY_BULLET_BLOCK_FLOATS;
pub const POWERUPS_OFFSET: usize = SHOTGUN_OFFSET + SHOTGUN_BLOCK_FLOATS;
pub const DANGER_OFFSET: usize = POWERUPS_OFFSET + POWERUP_BLOCK_FLOATS;

const HALF_W: f32 = GAME_WIDTH / 2.0;
const HALF_H: f32 = GAME_HEIGHT / 2.0;

#[inline]
fn normalize_x(x: f32) -> f32 {
    (x - HALF_W) / HALF_W
}
#[inline]
fn normalize_y(y: f32) -> f32 {
    (y - HALF_H) / HALF_H
}

/// Relative position normalized by half-screen dimensions.
#[inline]
fn rel_x(entity_x: f32, player_x: f32) -> f32 {
    (entity_x - player_x) / HALF_W
}
#[inline]
fn rel_y(entity_y: f32, player_y: f32) -> f32 {
    (entity_y - player_y) / HALF_H
}

fn shield_strength(level: Option<ShieldLevel>) -> f32 {
    match level {
        None => 0.0,
        Some(ShieldLevel::Critical) => 0.33,
        Some(ShieldLevel::Damaged) => 0.66,
        Some(ShieldLevel::Full) => 1.0,
    }
}

/// Estimated frames until a downward projectile reaches the player's Y.
/// Returns a normalized value: 0 = imminent, 1 = far away. Clamped to [0, 1].
#[inline]
fn time_to_impact_norm(bullet_y: f32, bullet_dy: f32, player_y: f32) -> f32 {
    if bullet_dy <= 0.0 {
        return 1.0; // moving away
    }
    let dy = player_y - bullet_y;
    if dy <= 0.0 {
        return 0.0; // already past player
    }
    // frames = distance / speed_per_frame, normalize by ~120 frames (~2 seconds)
    let frames = dy / bullet_dy;
    (frames / 120.0).clamp(0.0, 1.0)
}

/// Encode a step result into a flat observation vector of length `OBS_SIZE`.
pub fn encode(result: &StepResult) -> Vec<f32> {
    let mut obs = vec![0.0f32; OBS_SIZE];
    let mut cursor = 0usize;

    let px = result.player.x + 8.0; // center of 16px player sprite
    let py = result.player.y + 8.0;

    // --- Player block ---
    obs[cursor] = normalize_x(result.player.x);
    obs[cursor + 1] = normalize_y(result.player.y);
    obs[cursor + 2] = result.player_speed / PLAYER_SPEED;
    obs[cursor + 3] = result.projectile_speed / BULLET_SPEED;
    obs[cursor + 4] = shield_strength(result.player_shield);
    obs[cursor + 5] = (result.lives as f32 / 5.0).min(1.0);
    obs[cursor + 6] = if result.enemies_total > 0 {
        result.enemies_killed as f32 / result.enemies_total as f32
    } else {
        0.0
    };
    obs[cursor + 7] = if result.is_invulnerable { 0.0 } else { 1.0 };
    obs[cursor + 8] = result.fire_rate_stacks as f32 / 2.0;
    obs[cursor + 9] = result.speed_stacks as f32 / 2.0;
    obs[cursor + 10] = (result.bullet_count as f32 - 1.0) / 2.0; // 0.0=single, 0.5=double, 1.0=triple
    // Inventory counts, normalized (cap display at 4 for normalization)
    obs[cursor + 11] = (result.inventory[0] as f32 / 4.0).min(1.0); // rate
    obs[cursor + 12] = (result.inventory[1] as f32 / 4.0).min(1.0); // speed
    obs[cursor + 13] = (result.inventory[2] as f32 / 4.0).min(1.0); // double
    obs[cursor + 14] = (result.inventory[3] as f32 / 4.0).min(1.0); // triple
    obs[cursor + 15] = (result.inventory[4] as f32 / 4.0).min(1.0); // shield
    obs[cursor + 16] = (result.currency as f32 / 10.0).min(1.0); // currency
    cursor += PLAYER_FLOATS;

    // --- Enemies block ---
    for slot in 0..NUM_ENEMY_SLOTS {
        let base = cursor + slot * ENEMY_SLOT_FLOATS;
        if let Some(e) = result.enemies.get(slot) {
            if e.alive {
                let ex = e.x + 8.0;
                let ey = e.y + 8.0;
                obs[base] = 1.0;
                obs[base + 1] = normalize_x(e.x);
                obs[base + 2] = normalize_y(e.y);
                let class_idx = e.class as usize;
                if class_idx < 4 {
                    obs[base + 3 + class_idx] = 1.0;
                }
                obs[base + 7] = shield_strength(e.shield);
                obs[base + 8] = if e.is_diving { 1.0 } else { 0.0 };
                obs[base + 9] = rel_x(ex, px);
                obs[base + 10] = rel_y(ey, py);
            }
        }
    }
    cursor += ENEMY_BLOCK_FLOATS;

    // --- Player bullets ---
    let mut pb_filled = 0usize;
    for b in &result.bullets {
        if b.dy < 0.0 && pb_filled < NUM_PLAYER_BULLET_SLOTS {
            let base = cursor + pb_filled * PLAYER_BULLET_SLOT_FLOATS;
            obs[base] = 1.0;
            obs[base + 1] = normalize_x(b.x);
            obs[base + 2] = normalize_y(b.y);
            obs[base + 3] = rel_x(b.x, px);
            obs[base + 4] = rel_y(b.y, py);
            pb_filled += 1;
        }
    }
    cursor += PLAYER_BULLET_BLOCK_FLOATS;

    // --- Enemy bullets (sorted by distance to player, closest first) ---
    let mut enemy_bullets: Vec<_> = result
        .bullets
        .iter()
        .filter(|b| b.dy >= 0.0)
        .map(|b| {
            let dx = b.x - px;
            let dy = b.y - py;
            let dist_sq = dx * dx + dy * dy;
            (b, dist_sq)
        })
        .collect();
    enemy_bullets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (i, (b, _)) in enemy_bullets
        .iter()
        .enumerate()
        .take(NUM_ENEMY_BULLET_SLOTS)
    {
        let base = cursor + i * ENEMY_BULLET_SLOT_FLOATS;
        obs[base] = 1.0;
        obs[base + 1] = normalize_x(b.x);
        obs[base + 2] = normalize_y(b.y);
        obs[base + 3] = b.dy / ENEMY_BULLET_SPEED;
        obs[base + 4] = rel_x(b.x, px);
        obs[base + 5] = rel_y(b.y, py);
        obs[base + 6] = time_to_impact_norm(b.y, b.dy, py);
    }
    cursor += ENEMY_BULLET_BLOCK_FLOATS;

    // --- Shotgun balls (sorted by distance to player, closest first) ---
    let mut shotgun_sorted: Vec<_> = result
        .shotgun_balls
        .iter()
        .map(|s| {
            let dx = s.x - px;
            let dy = s.y - py;
            let dist_sq = dx * dx + dy * dy;
            (s, dist_sq)
        })
        .collect();
    shotgun_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (i, (s, _)) in shotgun_sorted.iter().enumerate().take(NUM_SHOTGUN_SLOTS) {
        let base = cursor + i * SHOTGUN_SLOT_FLOATS;
        obs[base] = 1.0;
        obs[base + 1] = normalize_x(s.x);
        obs[base + 2] = normalize_y(s.y);
        obs[base + 3] = s.dx / ENEMY_BULLET_SPEED;
        obs[base + 4] = s.dy / ENEMY_BULLET_SPEED;
        obs[base + 5] = rel_x(s.x, px);
        obs[base + 6] = rel_y(s.y, py);
        // time_to_impact for shotgun: use vertical component
        obs[base + 7] = time_to_impact_norm(s.y, s.dy, py);
    }
    cursor += SHOTGUN_BLOCK_FLOATS;

    // --- Powerups ---
    let mut pu_filled = 0usize;
    for p in &result.powerups {
        if pu_filled >= NUM_POWERUP_SLOTS {
            break;
        }
        let base = cursor + pu_filled * POWERUP_SLOT_FLOATS;
        obs[base] = 1.0;
        obs[base + 1] = normalize_x(p.x);
        obs[base + 2] = normalize_y(p.y);
        let k = p.kind.min(3) as usize;
        obs[base + 3 + k] = 1.0;
        pu_filled += 1;
    }
    cursor += POWERUP_BLOCK_FLOATS;

    // --- Danger columns ---
    // Divide the screen into NUM_DANGER_COLS vertical columns.
    // For each column, find the minimum time_to_impact of any downward projectile.
    // Output: 0.0 = immediate danger, 1.0 = safe.
    let col_width = GAME_WIDTH / NUM_DANGER_COLS as f32;
    // Initialize to 1.0 (safe)
    for c in 0..NUM_DANGER_COLS {
        obs[cursor + c] = 1.0;
    }

    // Enemy bullets
    for b in &result.bullets {
        if b.dy <= 0.0 {
            continue;
        }
        let col = ((b.x / col_width) as usize).min(NUM_DANGER_COLS - 1);
        let tti = time_to_impact_norm(b.y, b.dy, py);
        if tti < obs[cursor + col] {
            obs[cursor + col] = tti;
        }
    }
    // Shotgun balls
    for s in &result.shotgun_balls {
        if s.dy <= 0.0 {
            continue;
        }
        let col = ((s.x / col_width) as usize).min(NUM_DANGER_COLS - 1);
        let tti = time_to_impact_norm(s.y, s.dy, py);
        if tti < obs[cursor + col] {
            obs[cursor + col] = tti;
        }
    }

    obs
}
