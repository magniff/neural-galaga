use crate::constants::*;
use crate::shield::Shield;

/// Entry pattern for a wave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPattern {
    /// Two parallel streams arcing left/right (current default).
    DualArc,
}

/// Configuration for a single wave.
#[derive(Clone, Debug)]
pub struct WaveConfig {
    /// Wave number (1-based).
    pub wave: u32,
    /// Enemy class for each row (index 0 = top row).
    pub row_classes: [EnemyClass; ENEMY_ROWS],
    /// Entry pattern.
    pub pattern: EntryPattern,
    /// Number of rows (from bottom) that get shields.
    pub shielded_rows: usize,
}

impl WaveConfig {
    /// Generate a wave config for the given wave number (1-based).
    pub fn for_wave(wave: u32) -> Self {
        let row_classes = match wave {
            1 => [
                EnemyClass::White,
                EnemyClass::Red,
                EnemyClass::Yellow,
                EnemyClass::Green,
            ],
            2 => [
                EnemyClass::White,
                EnemyClass::Red,
                EnemyClass::Yellow,
                EnemyClass::Yellow,
            ],
            3 => [
                EnemyClass::White,
                EnemyClass::Red,
                EnemyClass::Red,
                EnemyClass::Yellow,
            ],
            4 => [
                EnemyClass::White,
                EnemyClass::White,
                EnemyClass::Red,
                EnemyClass::Red,
            ],
            _ => [
                EnemyClass::White,
                EnemyClass::White,
                EnemyClass::Red,
                EnemyClass::Red,
            ],
        };
        let shielded_rows = (wave as usize).min(ENEMY_ROWS);
        Self {
            wave,
            row_classes,
            pattern: EntryPattern::DualArc,
            shielded_rows,
        }
    }
}

/// Enemy class — determines color, stats, and behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EnemyClass {
    Green = 0,  // weakest
    Yellow = 1, // moderate
    Red = 2,    // strong
    White = 3,  // strongest
}

/// Per-class properties.
pub struct EnemyClassStats {
    pub score: u32,
    pub bullet_speed: f32,
    pub fire_weight: u32,
    pub dive_speed: f32,
    pub has_shield: bool,
}

impl EnemyClass {
    pub fn stats(self) -> EnemyClassStats {
        match self {
            EnemyClass::Green => EnemyClassStats {
                score: 50,
                bullet_speed: ENEMY_BULLET_SPEED * 0.8,
                fire_weight: 1,
                dive_speed: ENEMY_DIVE_SPEED * 0.8,
                has_shield: false,
            },
            EnemyClass::Yellow => EnemyClassStats {
                score: 100,
                bullet_speed: ENEMY_BULLET_SPEED,
                fire_weight: 2,
                dive_speed: ENEMY_DIVE_SPEED,
                has_shield: false,
            },
            EnemyClass::Red => EnemyClassStats {
                score: 200,
                bullet_speed: ENEMY_BULLET_SPEED * 1.2,
                fire_weight: 3,
                dive_speed: ENEMY_DIVE_SPEED * 1.2,
                has_shield: true,
            },
            EnemyClass::White => EnemyClassStats {
                score: 400,
                bullet_speed: ENEMY_BULLET_SPEED * 1.4,
                fire_weight: 5,
                dive_speed: ENEMY_DIVE_SPEED * 1.5,
                has_shield: true,
            },
        }
    }

    /// Map formation row to enemy class. Bottom rows = weakest, top = strongest.
    pub fn from_row(row: usize) -> Self {
        match row {
            0 => EnemyClass::White, // top row — strongest
            1 => EnemyClass::Red,
            2 => EnemyClass::Yellow,
            _ => EnemyClass::Green, // bottom row(s) — weakest
        }
    }

    /// Sprite row index for sprite sheet lookup.
    pub fn sprite_row(self) -> usize {
        match self {
            EnemyClass::Yellow => 1,
            EnemyClass::Red => 0,
            EnemyClass::Green => 2,
            EnemyClass::White => 3,
        }
    }
}

#[derive(Clone)]
pub enum DivePhase {
    None,
    Diving {
        timer: f32,
        start_x: f32,
        /// Spin phase for spiral-firing Red enemies (radians).
        spin_angle: f32,
    },
    Returning {
        timer: f32,
    },
}

#[derive(Clone)]
pub enum SpawnState {
    Waiting,
    FlyingIn { t: f32 },
    Arrived,
}

#[derive(Clone)]
pub struct Enemy {
    pub x: f32,
    pub y: f32,
    pub alive: bool,
    pub dive: DivePhase,
    pub home_x: f32,
    pub home_y: f32,
    pub row: usize,
    pub class: EnemyClass,
    pub spawn: SpawnState,
    /// Rotation in radians. 0 = facing down (default sprite orientation).
    pub rotation: f32,
    pub shield: Option<Shield>,
    /// Which wave this enemy belongs to (0 = arcs right, 1 = arcs left).
    wave: u8,
    prev_x: f32,
    prev_y: f32,
    spawn_delay: f32,
    spawn_index: usize,
}

/// Periodic "breathing" animation where rows spread apart and wiggle.
#[derive(Clone)]
struct BreathState {
    /// Time until next breath triggers.
    cooldown: f32,
    /// Current breath progress (0.0 = not active, >0 = in progress).
    timer: f32,
}

const BREATH_COOLDOWN: f32 = 8.0;
const BREATH_DURATION: f32 = 3.0;

impl BreathState {
    fn new() -> Self {
        Self {
            cooldown: BREATH_COOLDOWN,
            timer: 0.0,
        }
    }

    fn update(&mut self, dt: f32) {
        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.cooldown = BREATH_COOLDOWN;
            }
        } else {
            self.cooldown -= dt;
            if self.cooldown <= 0.0 {
                self.timer = BREATH_DURATION;
            }
        }
    }

    /// Returns 0.0 when inactive, ramps up to 1.0 at peak, back to 0.0.
    fn intensity(&self) -> f32 {
        if self.timer <= 0.0 {
            return 0.0;
        }
        let progress = 1.0 - self.timer / BREATH_DURATION; // 0→1 over duration
        // Smooth bell curve: peaks at 0.5
        let t = (progress * std::f32::consts::PI).sin();
        t
    }
}

#[derive(Clone)]
pub struct EnemyFormation {
    pub enemies: Vec<Enemy>,
    direction: f32,
    move_timer: f32,
    pub offset_x: f32,
    dive_timer: f32,
    intro_timer: f32,
    frame_counter: u64,
    breath: BreathState,
    wave: u32,
}

impl Enemy {
    /// Update rotation from position delta. Call after moving the enemy.
    /// 0 = facing down (sprite default), rotates based on velocity direction.
    fn update_rotation(&mut self) {
        let dx = self.x - self.prev_x;
        let dy = self.y - self.prev_y;
        if dx * dx + dy * dy > 0.01 {
            // atan2(dx, dy) gives 0 when moving straight down
            self.rotation = dx.atan2(dy);
        }
        self.prev_x = self.x;
        self.prev_y = self.y;
    }
}

impl EnemyFormation {
    pub fn new(config: &WaveConfig) -> Self {
        let total: usize = ENEMY_ROW_COLS.iter().sum();
        let mut enemies = Vec::with_capacity(total);
        let mut wave_counters = [0usize; 2];

        for row in 0..ENEMY_ROWS {
            let cols = ENEMY_ROW_COLS[row];
            let row_width = (cols - 1) as f32 * ENEMY_SPACING_X + ENEMY_WIDTH;
            let row_start_x = (GAME_WIDTH - row_width) / 2.0;
            let y = ENEMY_START_Y + row as f32 * ENEMY_SPACING_Y;

            for col in 0..cols {
                let x = row_start_x + col as f32 * ENEMY_SPACING_X;
                let wave = if row < 2 { 0u8 } else { 1u8 };
                let index_in_wave = wave_counters[wave as usize];
                wave_counters[wave as usize] += 1;
                let spawn_delay = index_in_wave as f32 * 0.06;

                let class = config.row_classes[row];

                // Shields are assigned to the bottom N rows based on wave
                let row_from_bottom = ENEMY_ROWS - 1 - row;
                let has_shield = row_from_bottom < config.shielded_rows;

                enemies.push(Enemy {
                    x: 0.0,
                    y: -ENEMY_HEIGHT,
                    alive: true,
                    dive: DivePhase::None,
                    home_x: x,
                    home_y: y,
                    row,
                    class,
                    spawn: SpawnState::Waiting,
                    rotation: 0.0,
                    shield: if has_shield {
                        Some(Shield::new(enemies.len() as u64 * 7))
                    } else {
                        None
                    },
                    wave,
                    prev_x: 0.0,
                    prev_y: -ENEMY_HEIGHT,
                    spawn_delay,
                    spawn_index: index_in_wave,
                });
            }
        }

        Self {
            enemies,
            direction: 1.0,
            move_timer: ENEMY_MOVE_INTERVAL,
            offset_x: 0.0,
            dive_timer: enemy_dive_interval(config.wave),
            intro_timer: 0.0,
            frame_counter: 0,
            breath: BreathState::new(),
            wave: config.wave,
        }
    }

    pub fn intro_done(&self) -> bool {
        self.enemies
            .iter()
            .all(|e| matches!(e.spawn, SpawnState::Arrived))
    }

    pub fn update_intro(&mut self, dt: f32) {
        self.intro_timer += dt;
        let fly_in_speed = 1.0 / 4.0;
        let mid = GAME_WIDTH / 2.0;
        let stream_gap = ENEMY_WIDTH + 4.0;
        // Wave 0 enters left of center, wave 1 enters right of center
        let stream_x = [
            mid - stream_gap / 2.0 - ENEMY_WIDTH / 2.0,
            mid + stream_gap / 2.0 - ENEMY_WIDTH / 2.0,
        ];
        let column_gap = ENEMY_HEIGHT + 4.0;

        for e in &mut self.enemies {
            let entry_x = stream_x[e.wave as usize];

            match &mut e.spawn {
                SpawnState::Waiting => {
                    if self.intro_timer >= e.spawn_delay {
                        e.spawn = SpawnState::FlyingIn { t: 0.0 };
                    }
                    e.x = entry_x;
                    e.y = -(e.spawn_index as f32 + 1.0) * column_gap;
                }
                SpawnState::FlyingIn { t } => {
                    *t += fly_in_speed * dt;
                    if *t >= 1.0 {
                        e.x = e.home_x;
                        e.y = e.home_y;
                        e.spawn = SpawnState::Arrived;
                    } else {
                        let p = *t;
                        if p < 0.3 {
                            // Phase 1: straight down to the arc start
                            let phase_t = p / 0.3;
                            let target_y = GAME_HEIGHT * 0.4;
                            let start_y = -(e.spawn_index as f32 + 1.0) * column_gap;
                            e.x = entry_x;
                            e.y = start_y + phase_t * (target_y - start_y);
                        } else if p < 0.8 {
                            // Phase 2: wide arc sideways and downward, past screen edges
                            let phase_t = (p - 0.3) / 0.5;
                            let mid_y = GAME_HEIGHT * 0.3;
                            let angle = phase_t * std::f32::consts::PI * 1.2;
                            let radius = GAME_WIDTH * 0.8;
                            let dir = if e.wave == 0 { 1.0 } else { -1.0 };
                            e.x = entry_x + dir * radius * angle.sin();
                            e.y = mid_y + radius * 0.6 * (1.0 - angle.cos());
                        } else {
                            // Phase 3: lerp to grid position
                            let phase_t = (p - 0.8) / 0.2;
                            let end_angle = std::f32::consts::PI * 1.2;
                            let radius = GAME_WIDTH * 0.8;
                            let dir = if e.wave == 0 { 1.0 } else { -1.0 };
                            let mid_y = GAME_HEIGHT * 0.3;
                            let end_x = entry_x + dir * radius * end_angle.sin();
                            let end_y = mid_y + radius * 0.6 * (1.0 - end_angle.cos());
                            e.x = end_x + phase_t * (e.home_x - end_x);
                            e.y = end_y + phase_t * (e.home_y - end_y);
                        }
                    }
                }
                SpawnState::Arrived => {}
            }
            e.update_rotation();
        }
    }

    pub fn update_formation(&mut self, dt: f32) {
        self.move_timer -= dt;
        if self.move_timer <= 0.0 {
            self.move_timer = ENEMY_MOVE_INTERVAL;
            self.offset_x += ENEMY_STEP * self.direction;

            // Find the bounding box of all alive, in-formation enemies
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut any_in_formation = false;
            for e in &self.enemies {
                if e.alive
                    && matches!(e.spawn, SpawnState::Arrived)
                    && matches!(e.dive, DivePhase::None)
                {
                    let ex = e.home_x + self.offset_x;
                    min_x = min_x.min(ex);
                    max_x = max_x.max(ex + ENEMY_WIDTH);
                    any_in_formation = true;
                }
            }

            if any_in_formation {
                // Clamp offset so formation stays on screen
                if max_x > GAME_WIDTH - 4.0 {
                    self.offset_x -= max_x - (GAME_WIDTH - 4.0);
                    self.direction = -1.0;
                    for e in &mut self.enemies {
                        e.home_y += ENEMY_STEP;
                    }
                } else if min_x < 4.0 {
                    self.offset_x += 4.0 - min_x;
                    self.direction = 1.0;
                    for e in &mut self.enemies {
                        e.home_y += ENEMY_STEP;
                    }
                }
            }
        }

        // Only breathe after intro is done
        if self.intro_done() {
            self.breath.update(dt);
        }
        let breath = self.breath.intensity();

        // Use a smooth continuous time base for all idle animations
        let t = self.frame_counter as f32;
        let center_x = GAME_WIDTH / 2.0;

        for e in &mut self.enemies {
            if e.alive
                && matches!(e.spawn, SpawnState::Arrived)
                && matches!(e.dive, DivePhase::None)
            {
                // Per-enemy phase ensures each one moves independently
                let phase = e.spawn_index as f32 * 1.8;

                // Idle bob: gentle vertical sine, always active
                let bob_y = (t * 0.07 + phase).sin() * 2.0;

                // Breath: rows spread apart vertically
                let breath_y = e.row as f32 * 6.0 * breath;

                // Breath: horizontal inflate from formation center (use home_x, not offset x)
                let dx_from_center = e.home_x + ENEMY_WIDTH / 2.0 - center_x;
                let breath_inflate_x = dx_from_center * 0.3 * breath;

                // Breath: per-row horizontal wiggle (slow, smooth)
                let breath_wiggle_x = (t * 0.05 + e.row as f32 * 2.0).sin() * 3.0 * breath;

                e.x = e.home_x + self.offset_x + breath_inflate_x + breath_wiggle_x;
                e.y = e.home_y + bob_y + breath_y;
                e.rotation = 0.0;
                e.prev_x = e.x;
                e.prev_y = e.y;
            }
        }
    }

    pub fn trigger_dive(&mut self, dt: f32) {
        self.frame_counter += 1;
        self.dive_timer -= dt;
        if self.dive_timer <= 0.0 {
            self.dive_timer = enemy_dive_interval(self.wave);
            let candidates: Vec<usize> = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.alive && matches!(e.dive, DivePhase::None))
                .map(|(i, _)| i)
                .collect();
            if !candidates.is_empty() {
                let idx = candidates[self.frame_counter as usize % candidates.len()];
                let start_x = self.enemies[idx].x;
                self.enemies[idx].dive = DivePhase::Diving {
                    timer: 0.0,
                    start_x,
                    spin_angle: 0.0,
                };
            }
        }
    }

    pub fn update_dives(&mut self, dt: f32) {
        for e in &mut self.enemies {
            if !e.alive {
                continue;
            }
            match &mut e.dive {
                DivePhase::None => {}
                DivePhase::Diving {
                    timer,
                    start_x,
                    spin_angle,
                } => {
                    *timer += dt;
                    let t = *timer;
                    e.y += ENEMY_DIVE_SPEED * dt;
                    e.x = *start_x + (t * 3.0).sin() * 30.0;
                    e.x = e.x.clamp(0.0, GAME_WIDTH - ENEMY_WIDTH);

                    // Red enemies spin rapidly during dive
                    if e.class == EnemyClass::Red {
                        *spin_angle += dt * 6.0; // ~1 full rotation per second
                        e.rotation = *spin_angle;
                        e.prev_x = e.x;
                        e.prev_y = e.y;
                    }

                    if e.y > GAME_HEIGHT + ENEMY_HEIGHT {
                        e.dive = DivePhase::Returning { timer: 0.0 };
                        e.y = e.home_y + self.offset_x.abs().min(40.0);
                    }
                }
                DivePhase::Returning { timer, .. } => {
                    *timer += dt;
                    let home_x = e.home_x + self.offset_x;
                    let home_y = e.home_y;
                    let dx = home_x - e.x;
                    let dy = home_y - e.y;
                    let speed = ENEMY_DIVE_SPEED * 1.5;
                    e.x += dx.signum() * speed.min(dx.abs() / dt) * dt;
                    e.y += dy.signum() * speed.min(dy.abs() / dt) * dt;

                    if (dx.abs() < 5.0 && dy.abs() < 5.0) || *timer > 4.0 {
                        e.x = home_x;
                        e.y = home_y;
                        e.rotation = 0.0;
                        e.prev_x = e.x;
                        e.prev_y = e.y;
                        e.dive = DivePhase::None;
                    }
                }
            }
            if !matches!(e.dive, DivePhase::None) {
                e.update_rotation();
            }
        }
    }

    pub fn all_dead(&self) -> bool {
        self.enemies.iter().all(|e| !e.alive)
    }

    pub fn reached_player(&self) -> bool {
        self.enemies
            .iter()
            .any(|e| e.alive && matches!(e.dive, DivePhase::None) && e.y + ENEMY_HEIGHT >= PLAYER_Y)
    }
}
