/// Native game resolution (original Galaga was 224x288, portrait orientation).
pub const GAME_WIDTH: f32 = 224.0;
pub const GAME_HEIGHT: f32 = 288.0;

/// Fixed simulation timestep. The core is timeless — each step() advances
/// by exactly this amount. At 60 steps/sec this matches realtime.
pub const FIXED_DT: f32 = 1.0 / 60.0;

pub const SPRITE_SCALE: f32 = 1.0;
pub const PLAYER_WIDTH: f32 = 16.0;
pub const PLAYER_HEIGHT: f32 = 16.0;
pub const PLAYER_SPEED: f32 = 110.0;
pub const PLAYER_Y: f32 = GAME_HEIGHT - 22.0;
pub const PLAYER_LIVES: u32 = 3;
pub const RESPAWN_INVULN: f32 = 2.0;

pub const ENEMY_WIDTH: f32 = 16.0;
pub const ENEMY_HEIGHT: f32 = 16.0;
pub const ENEMY_ROWS: usize = 4;
pub const ENEMY_SPACING_X: f32 = 18.0;
pub const ENEMY_SPACING_Y: f32 = 18.0;
pub const ENEMY_START_Y: f32 = 40.0;

/// Columns per row: 10-8-10-8 interleaving pattern.
pub const ENEMY_ROW_COLS: [usize; ENEMY_ROWS] = [10, 8, 10, 8];
pub const ENEMY_STEP: f32 = 4.0;
pub const ENEMY_MOVE_INTERVAL: f32 = 0.5;
pub const ENEMY_DIVE_SPEED: f32 = 80.0;
pub fn enemy_dive_interval(wave: u32) -> f32 {
    match wave {
        1 => 2.5,
        2 => 2.0,
        3 => 1.5,
        4 => 1.0,
        5 => 0.5,
        _ => 0.3,
    }
}
pub const ENEMY_SHOOT_INTERVAL: f32 = 1.2;
pub const ENEMY_BULLET_SPEED: f32 = 224.0;

pub const BULLET_SCALE: f32 = 1.0;
pub const BULLET_WIDTH: f32 = 4.0;
pub const BULLET_HEIGHT: f32 = 8.0;
pub const BULLET_SPEED: f32 = 359.0;
pub const SHOOT_COOLDOWN: f32 = 0.25;

pub const LIVES_ICON_SCALE: f32 = 1.0;
pub const LIVES_ICON_SPACING: f32 = 10.0;
