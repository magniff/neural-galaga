mod constants;
mod enemies;
mod framebuffer;
mod game;
pub mod session;
pub mod shield;
mod sprites;
mod starfield;
mod text;

pub use constants::{
    BULLET_HEIGHT, BULLET_SPEED, BULLET_WIDTH, ENEMY_BULLET_SPEED, ENEMY_HEIGHT, ENEMY_WIDTH,
    FIXED_DT, GAME_HEIGHT, GAME_WIDTH, PLAYER_HEIGHT, PLAYER_SPEED, PLAYER_WIDTH, PLAYER_Y,
};
pub use framebuffer::Framebuffer;
pub use sprites::{
    REGION_BOSS_F1, REGION_BULLET, REGION_BW_F1, REGION_BW_F2, REGION_ENEMY_A_F1,
    REGION_ENEMY_A_F2, REGION_ENEMY_B_F1, REGION_ENEMY_BULLET, REGION_GREEN_F1, REGION_GREEN_F2,
    REGION_LIVES_ICON, REGION_PLAYER, REGION_RED_F1, REGION_RED_F2, REGION_YELLOW_F1,
    REGION_YELLOW_F2, SpriteRegion, SpriteSheet, enemy_sprite_region,
};
pub use starfield::{BattleStarfield, Starfield};
pub use text::BitmapFont;

pub use enemies::EnemyClass;

use constants::*;
use enemies::SpawnState;
use game::PlayState;

const SPRITE_PNG: &[u8] = include_bytes!("../../../assets/sprites.png");
const FONT_TTF: &[u8] = include_bytes!("../../../assets/fonts/PressStart2P-Regular.ttf");

/// Input action for one simulation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Left,
    Right,
    Fire,
    /// Trade one spare life for a fire-rate powerup.
    BuyRate,
    /// Trade one spare life for a speed powerup.
    BuySpeed,
    /// Trade one spare life for double bullets.
    BuyDouble,
    /// Trade one spare life for triple bullets.
    BuyTriple,
    /// Trade one spare life for a shield.
    BuyShield,
    /// Sell a rate powerup (from inventory or active stack) for +1 life.
    SellRate,
    /// Sell a speed powerup (from inventory or active stack) for +1 life.
    SellSpeed,
    /// Sell a double powerup (from inventory, or downgrade 2→1) for +1 life.
    SellDouble,
    /// Sell a triple powerup (from inventory, or downgrade 3→2) for +1 life.
    SellTriple,
    /// Sell a shield (from inventory, or active only if Full) for +1 life.
    SellShield,
    /// Buy an extra life for 2 currency.
    BuyLife,
    /// Sell a spare life for 1 currency.
    SellLife,
}

/// Status of the game after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStatus {
    Intro,
    Playing,
    WaveComplete,
    Lost,
}

/// Sound events produced by a single step.
#[derive(Clone, Copy, Default, Debug)]
pub struct SoundEvents {
    pub player_fired: bool,
    pub enemy_fired: bool,
    pub player_hit: bool,
    pub enemy_hit: bool,
    /// Generic shield-hit event for sound playback. Fires for either side
    /// (player shield blocking incoming fire OR enemy shield being damaged
    /// by player bullets). For agents that need to distinguish the two, use
    /// `enemy_shield_hit` and `player_shield_hit`.
    pub shield_hit: bool,
    /// True when a player bullet hits an enemy's shield (the agent damaged a foe's shield).
    pub enemy_shield_hit: bool,
    /// True when an enemy bullet / shotgun ball / diver hits the player's shield
    /// and the shield absorbs it (the player would have been hit otherwise).
    pub player_shield_hit: bool,
    pub powerup_picked: bool,
    /// True when a life was traded for a powerup via a Buy action.
    pub powerup_bought: bool,
    /// True when a powerup was sold for a life via a Sell action.
    pub powerup_sold: bool,
}

/// Position of an actor in game coordinates.
#[derive(Clone, Copy, Debug)]
pub struct ActorPos {
    pub x: f32,
    pub y: f32,
}

/// A bullet's position and direction.
#[derive(Clone, Copy, Debug)]
pub struct BulletInfo {
    pub x: f32,
    pub y: f32,
    /// Negative = player bullet (going up), positive = enemy bullet (going down).
    pub dy: f32,
}

/// An enemy's position and state.
#[derive(Clone, Debug)]
pub struct EnemyInfo {
    pub x: f32,
    pub y: f32,
    pub alive: bool,
    pub row: usize,
    pub class: EnemyClass,
    /// Rotation in radians. 0 = facing down.
    pub rotation: f32,
    /// Shield level: None, or Some(Full/Damaged/Critical).
    pub shield: Option<shield::ShieldLevel>,
    /// True when the enemy is actively diving toward the player.
    pub is_diving: bool,
}

/// An active powerup falling on screen.
#[derive(Clone, Copy, Debug)]
pub struct PowerupInfo {
    pub x: f32,
    pub y: f32,
    /// 0=Life, 1=Rate, 2=Speed, 3=Double, 4=Triple
    pub kind: u8,
}

/// A shotgun ball's position and velocity.
#[derive(Clone, Copy, Debug)]
pub struct ShotgunBallInfo {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
}

/// An active explosion.
#[derive(Clone, Copy, Debug)]
pub struct ExplosionInfo {
    pub x: f32,
    pub y: f32,
    pub frame: u8,
}

/// Result of a simulation step.
#[derive(Clone)]
pub struct StepResult {
    pub status: GameStatus,
    pub step: u64,
    pub score: i32,
    pub lives: u32,
    pub wave: u32,
    pub enemies_killed: usize,
    pub enemies_total: usize,
    pub sound_events: SoundEvents,
    pub player: ActorPos,
    pub player_shield: Option<shield::ShieldLevel>,
    /// Player movement speed (base + powerups).
    pub player_speed: f32,
    /// Player projectile speed (base * multiplier).
    pub projectile_speed: f32,
    /// Shield level: 0 = none, 1 = critical, 2 = damaged, 3 = full.
    pub shield_level: u8,
    /// True when the player is invulnerable (e.g. first 2 seconds of a wave, or after a death).
    pub is_invulnerable: bool,
    /// Current fire-rate upgrade stacks (0–2).
    pub fire_rate_stacks: u8,
    /// Current speed upgrade stacks (0–2).
    pub speed_stacks: u8,
    /// Current bullet count (1, 2, or 3).
    pub bullet_count: u8,
    /// Surplus powerup inventory (sellable for currency).
    pub inventory: [u8; 5],
    /// Current currency balance for buying/selling.
    pub currency: u32,
    pub enemies: Vec<EnemyInfo>,
    pub bullets: Vec<BulletInfo>,
    pub shotgun_balls: Vec<ShotgunBallInfo>,
    pub explosions: Vec<ExplosionInfo>,
    pub powerups: Vec<PowerupInfo>,
}

/// Headless game simulation. Each call to `step()` advances by one fixed tick.
/// Produces a 288x224 RGBA framebuffer accessible via `framebuffer()`.
#[derive(Clone)]
pub struct GameSim {
    state: PlayState,
    sheet: SpriteSheet,
    font: BitmapFont,
    fb: Framebuffer,
}

impl GameSim {
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::with_seed_and_wave(seed, 1)
    }

    pub fn with_seed_and_wave(seed: u64, wave: u32) -> Self {
        Self {
            state: PlayState::with_seed_and_wave(seed, wave),
            sheet: SpriteSheet::load_from_memory(SPRITE_PNG),
            font: BitmapFont::load_from_memory(FONT_TTF),
            fb: Framebuffer::new(GAME_WIDTH as u32, GAME_HEIGHT as u32),
        }
    }

    /// Reset to a fresh game with a new random seed.
    pub fn reset(&mut self) {
        self.reset_to_wave(1);
    }

    /// Reset to a fresh game starting at a specific wave.
    pub fn reset_to_wave(&mut self, wave: u32) {
        let seed = self
            .state
            .frame_counter
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.state = PlayState::with_seed_and_wave(seed, wave);
    }

    /// Advance by one tick and render to the internal framebuffer.
    pub fn step(&mut self, actions: &[Action]) -> StepResult {
        let sfx = self.state.tick(actions);
        self.render();

        let total = self.state.formation.enemies.len();
        let killed = self
            .state
            .formation
            .enemies
            .iter()
            .filter(|e| !e.alive)
            .count();

        let status = if self.state.wave_just_completed {
            GameStatus::WaveComplete
        } else if self.state.lost {
            GameStatus::Lost
        } else if !self.state.formation.intro_done() {
            GameStatus::Intro
        } else {
            GameStatus::Playing
        };

        let player = ActorPos {
            x: self.state.player.x,
            y: self.state.player.y,
        };

        let enemies: Vec<EnemyInfo> = self
            .state
            .formation
            .enemies
            .iter()
            .map(|e| EnemyInfo {
                x: e.x,
                y: e.y,
                alive: e.alive,
                row: e.row,
                class: e.class,
                rotation: e.rotation,
                shield: e.shield.as_ref().map(|s| s.level),
                is_diving: matches!(e.dive, enemies::DivePhase::Diving { .. }),
            })
            .collect();

        let bullets: Vec<BulletInfo> = self
            .state
            .bullets
            .iter()
            .filter(|b| b.active)
            .map(|b| BulletInfo {
                x: b.x,
                y: b.y,
                dy: b.dy,
            })
            .collect();

        let shotgun_balls: Vec<ShotgunBallInfo> = self
            .state
            .shotgun_balls
            .iter()
            .filter(|b| b.active)
            .map(|b| ShotgunBallInfo {
                x: b.x,
                y: b.y,
                dx: b.dx,
                dy: b.dy,
            })
            .collect();

        let powerups: Vec<PowerupInfo> = self
            .state
            .powerups
            .iter()
            .map(|p| PowerupInfo {
                x: p.x,
                y: p.y,
                kind: p.kind as u8,
            })
            .collect();

        let explosions: Vec<ExplosionInfo> = self
            .state
            .explosions
            .iter()
            .map(|exp| ExplosionInfo {
                x: exp.x,
                y: exp.y,
                frame: exp.frame,
            })
            .collect();

        let shield_level = match self.state.player.shield.as_ref().map(|s| s.level) {
            None => 0,
            Some(shield::ShieldLevel::Critical) => 1,
            Some(shield::ShieldLevel::Damaged) => 2,
            Some(shield::ShieldLevel::Full) => 3,
        };

        StepResult {
            status,
            step: self.state.frame_counter,
            score: self.state.score,
            lives: self.state.lives,
            wave: self.state.wave,
            enemies_killed: killed,
            enemies_total: total,
            sound_events: sfx,
            player,
            player_shield: self.state.player.shield.as_ref().map(|s| s.level),
            player_speed: PLAYER_SPEED * self.state.upgrades.speed_mult,
            projectile_speed: BULLET_SPEED * self.state.upgrades.speed_mult,
            shield_level,
            is_invulnerable: self.state.invuln_timer > 0.0,
            fire_rate_stacks: self.state.upgrades.fire_rate_stacks,
            speed_stacks: self.state.upgrades.speed_stacks,
            bullet_count: self.state.upgrades.bullet_count,
            inventory: [
                self.state.inventory.rate,
                self.state.inventory.speed,
                self.state.inventory.double,
                self.state.inventory.triple,
                self.state.inventory.shield,
            ],
            currency: self.state.currency,
            enemies,
            bullets,
            shotgun_balls,
            explosions,
            powerups,
        }
    }

    /// Is the game over?
    pub fn is_done(&self) -> bool {
        self.state.lost
    }

    /// Current lives remaining.
    pub fn lives(&self) -> u32 {
        self.state.lives
    }

    /// Inject a random powerup at a random x position near the top of the screen.
    /// Used for vision pretraining to expose the model to powerups more frequently.
    pub fn inject_random_powerup(&mut self, rng_seed: u64) {
        // Cheap LCG to get a few values from the seed.
        let mut s = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r0 = (s >> 33) as u32;
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r1 = (s >> 33) as u32;

        let kind = match r0 % 6 {
            0 => game::PowerupKind::Life,
            1 => game::PowerupKind::Rate,
            2 => game::PowerupKind::Speed,
            3 => game::PowerupKind::Double,
            4 => game::PowerupKind::Triple,
            _ => game::PowerupKind::Shield,
        };
        let x = (r1 % (GAME_WIDTH as u32 - 16)) as f32;
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r2 = (s >> 33) as u32;
        let y = (r2 % (GAME_HEIGHT as u32 - 16)) as f32;
        self.state.powerups.push(game::Powerup { x, y, kind });
    }

    /// Access the rendered framebuffer (288x224 RGBA, row-major).
    pub fn framebuffer(&self) -> &[u8] {
        &self.fb.pixels
    }

    pub fn framebuffer_width(&self) -> u32 {
        GAME_WIDTH as u32
    }
    pub fn framebuffer_height(&self) -> u32 {
        GAME_HEIGHT as u32
    }

    /// Render current state to framebuffer.
    fn render(&mut self) {
        self.fb.clear(0, 0, 0, 0); // transparent — let the runner composite over its own background

        // Player
        let draw_player = if self.state.lost && self.state.lives == 0 {
            false
        } else {
            self.state.invuln_timer <= 0.0 || (self.state.invuln_timer * 10.0) as u32 % 2 == 0
        };
        if draw_player {
            self.sheet.draw(
                &mut self.fb,
                &REGION_PLAYER,
                self.state.player.x,
                self.state.player.y,
                SPRITE_SCALE,
            );
            // Player shield
            if let Some(ref shield) = self.state.player.shield {
                let color = shield.color(self.state.frame_counter);
                let radius = shield.radius(self.state.frame_counter);
                let cx = self.state.player.x + PLAYER_WIDTH / 2.0;
                let cy = self.state.player.y + PLAYER_HEIGHT / 2.0;
                self.fb.draw_circle(cx, cy, radius, 1.0, color);
            }
        }

        // Enemies
        for e in &self.state.formation.enemies {
            if !e.alive || matches!(e.spawn, SpawnState::Waiting) {
                continue;
            }
            let region = sprites::enemy_sprite_region(e.class.sprite_row(), self.state.anim_frame);
            self.sheet
                .draw_rotated(&mut self.fb, region, e.x, e.y, SPRITE_SCALE, e.rotation);
            // Enemy shield
            if let Some(ref shield) = e.shield {
                let color = shield.color(self.state.frame_counter);
                let radius = shield.radius(self.state.frame_counter);
                let cx = e.x + ENEMY_WIDTH / 2.0;
                let cy = e.y + ENEMY_HEIGHT / 2.0;
                self.fb.draw_circle(cx, cy, radius, 1.0, color);
            }
        }

        // Bullet trail particles
        for p in &self.state.trail_particles {
            let fade = p.life as f32 / 5.0; // 1.0 → 0.0 over lifetime
            let color = if p.is_player {
                [0.3, 0.7, 1.0, fade * 0.7] // cyan
            } else {
                [1.0, 0.5, 0.15, fade * 0.7] // orange
            };
            self.fb.fill_rect(p.x as i32, p.y as i32, 1, 1, color);
        }

        // Bullets
        for b in &self.state.bullets {
            if !b.active {
                continue;
            }
            if b.dy < 0.0 {
                self.sheet
                    .draw(&mut self.fb, &REGION_BULLET, b.x, b.y, BULLET_SCALE);
            } else {
                self.sheet
                    .draw(&mut self.fb, &REGION_ENEMY_BULLET, b.x, b.y, BULLET_SCALE);
            }
        }

        // Shotgun balls
        for b in &self.state.shotgun_balls {
            if b.active {
                self.sheet
                    .draw(&mut self.fb, &sprites::REGION_SHOTGUN_BALL, b.x, b.y, 1.0);
            }
        }

        // Powerups
        for p in &self.state.powerups {
            let region = match p.kind {
                game::PowerupKind::Life => &sprites::REGION_POWERUP_LIFE,
                game::PowerupKind::Rate => &sprites::REGION_POWERUP_RATE,
                game::PowerupKind::Speed => &sprites::REGION_POWERUP_SPEED,
                game::PowerupKind::Double => &sprites::REGION_POWERUP_DOUBLE,
                game::PowerupKind::Triple => &sprites::REGION_POWERUP_TRIPLE,
                game::PowerupKind::Shield => &sprites::REGION_POWERUP_SHIELD,
            };
            self.sheet.draw(&mut self.fb, region, p.x, p.y, 1.0);
        }

        // Explosions
        for exp in &self.state.explosions {
            let region = sprites::EXPLOSION_REGIONS[exp.frame as usize];
            self.sheet.draw(&mut self.fb, region, exp.x, exp.y, 1.0);
        }

        // HUD
        self.font.draw_text(
            &mut self.fb,
            &format!("S:{} | W:{}", self.state.score, self.state.wave),
            2.0,
            2.0,
            1.0,
            [0.7, 0.7, 0.7, 1.0],
        );

        let lives_to_show = if self.state.lost {
            0
        } else {
            self.state.lives - 1
        };
        for i in 0..lives_to_show {
            let lx = GAME_WIDTH - (i as f32 + 1.0) * LIVES_ICON_SPACING - 2.0;
            let ly = GAME_HEIGHT - LIVES_ICON_SPACING;
            self.sheet
                .draw(&mut self.fb, &REGION_LIVES_ICON, lx, ly, LIVES_ICON_SCALE);
        }
    }
}
