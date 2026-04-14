use crate::Action;
use crate::constants::*;
use crate::enemies::{DivePhase, EnemyClass, EnemyFormation, SpawnState, WaveConfig};
use crate::shield::{Shield, ShieldLevel};

#[derive(Clone)]
pub(crate) struct Player {
    pub x: f32,
    pub y: f32,
    pub shoot_cooldown: f32,
    pub shield: Option<Shield>,
}

#[derive(Clone)]
pub(crate) struct Bullet {
    pub x: f32,
    pub y: f32,
    pub dy: f32,
    pub active: bool,
}

#[derive(Clone)]
pub(crate) struct Explosion {
    pub x: f32,
    pub y: f32,
    /// Current frame (0-3). Advances every EXPLOSION_FRAME_TICKS steps.
    pub frame: u8,
    /// Ticks remaining in current frame.
    pub timer: u8,
}

/// Shotgun ball — has both dx and dy, optionally bounces off side walls.
#[derive(Clone)]
pub(crate) struct ShotgunBall {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub active: bool,
    /// Number of wall bounces remaining. 0 = no bouncing (flies off screen).
    pub bounces_left: u8,
}

const SHOTGUN_BALL_SIZE: f32 = 4.0;

const EXPLOSION_FRAME_TICKS: u8 = 6;
const EXPLOSION_FRAMES: u8 = 4;

#[derive(Clone)]
pub(crate) struct TrailParticle {
    pub x: f32,
    pub y: f32,
    /// Remaining lifetime in ticks. Starts at TRAIL_LIFETIME.
    pub life: u8,
    /// true = player bullet trail (cyan-ish), false = enemy bullet trail (orange-ish)
    pub is_player: bool,
}

const TRAIL_LIFETIME: u8 = 5;

/// A pending burst — fires remaining bullets over consecutive ticks.
#[derive(Clone)]
pub(crate) struct BurstQueue {
    pub x: f32,
    pub y: f32,
    pub remaining: u8,
    pub ticks_until_next: u8,
}

const BURST_SIZE: u8 = 3;
const BURST_TICK_INTERVAL: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PowerupKind {
    Life = 0,
    Rate = 1,
    Speed = 2,
    Double = 3,
    Triple = 4,
    Shield = 5,
}

#[derive(Clone)]
pub(crate) struct Powerup {
    pub x: f32,
    pub y: f32,
    pub kind: PowerupKind,
}

const POWERUP_FALL_SPEED: f32 = 72.0;
const POWERUP_SIZE: f32 = 16.0;
const POWERUP_BONUS_POINTS: i32 = 500;

/// Player upgrade state — accumulated powerups.
#[derive(Clone)]
pub(crate) struct PlayerUpgrades {
    pub fire_rate_mult: f32,
    pub fire_rate_stacks: u8,
    pub speed_mult: f32,
    pub speed_stacks: u8,
    /// 1 = single, 2 = double, 3 = triple
    pub bullet_count: u8,
}

/// Surplus powerup inventory — items that couldn't be applied because the
/// upgrade was already maxed. Can be sold for +1 life each.
#[derive(Clone, Default)]
pub(crate) struct PowerupInventory {
    pub rate: u8,
    pub speed: u8,
    pub double: u8,
    pub triple: u8,
    pub shield: u8,
}

#[derive(Clone)]
pub struct PlayState {
    pub(crate) player: Player,
    pub(crate) formation: EnemyFormation,
    pub(crate) bullets: Vec<Bullet>,
    pub(crate) shotgun_balls: Vec<ShotgunBall>,
    pub(crate) burst_queues: Vec<BurstQueue>,
    pub(crate) explosions: Vec<Explosion>,
    pub(crate) trail_particles: Vec<TrailParticle>,
    pub(crate) powerups: Vec<Powerup>,
    pub(crate) upgrades: PlayerUpgrades,
    pub(crate) inventory: PowerupInventory,
    /// Abstract currency for buying/selling powerups and lives.
    pub(crate) currency: u32,
    pub(crate) anim_timer: f32,
    pub(crate) anim_frame: bool,
    pub(crate) enemy_shoot_timer: f32,
    pub(crate) frame_counter: u64,
    pub(crate) score: i32,
    pub(crate) lives: u32,
    pub(crate) wave: u32,
    pub(crate) invuln_timer: f32,
    pub(crate) wave_complete_handled: bool,
    /// Set for one tick when a wave is cleared.
    pub(crate) wave_just_completed: bool,
    pub(crate) lost: bool,
}

fn aabb_overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

/// Apply a powerup to the player. If the upgrade is already maxed, route to inventory.
fn apply_powerup(
    kind: PowerupKind,
    upgrades: &mut PlayerUpgrades,
    inventory: &mut PowerupInventory,
    lives: &mut u32,
    player_shield: &mut Option<Shield>,
    frame_counter: u64,
) {
    match kind {
        PowerupKind::Life => {
            *lives += 1;
        }
        PowerupKind::Rate => {
            if upgrades.fire_rate_stacks < 2 {
                upgrades.fire_rate_mult *= 0.8;
                upgrades.fire_rate_stacks += 1;
            } else {
                inventory.rate = inventory.rate.saturating_add(1);
            }
        }
        PowerupKind::Speed => {
            if upgrades.speed_stacks < 2 {
                upgrades.speed_mult *= 1.1;
                upgrades.speed_stacks += 1;
            } else {
                inventory.speed = inventory.speed.saturating_add(1);
            }
        }
        PowerupKind::Double => {
            if upgrades.bullet_count < 2 {
                upgrades.bullet_count = 2;
            } else {
                inventory.double = inventory.double.saturating_add(1);
            }
        }
        PowerupKind::Triple => {
            if upgrades.bullet_count < 3 {
                upgrades.bullet_count = 3;
            } else {
                inventory.triple = inventory.triple.saturating_add(1);
            }
        }
        PowerupKind::Shield => {
            if player_shield.is_none() {
                *player_shield = Some(Shield::new(frame_counter));
            } else {
                inventory.shield = inventory.shield.saturating_add(1);
            }
        }
    }
}

impl PlayState {
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::with_seed_and_wave(seed, 1)
    }

    pub fn with_seed_and_wave(seed: u64, wave: u32) -> Self {
        let config = WaveConfig::for_wave(wave);
        Self {
            player: Player {
                x: GAME_WIDTH / 2.0 - PLAYER_WIDTH / 2.0,
                y: PLAYER_Y,
                shoot_cooldown: 0.0,
                shield: Some(Shield::new(0)),
            },
            formation: EnemyFormation::new(&config),
            bullets: Vec::new(),
            shotgun_balls: Vec::new(),
            burst_queues: Vec::new(),
            explosions: Vec::new(),
            trail_particles: Vec::new(),
            powerups: Vec::new(),
            upgrades: PlayerUpgrades {
                fire_rate_mult: 1.0,
                fire_rate_stacks: 0,
                speed_mult: 1.0,
                speed_stacks: 0,
                bullet_count: 2,
            },
            inventory: PowerupInventory::default(),
            currency: 0,
            anim_timer: 0.0,
            anim_frame: false,
            enemy_shoot_timer: 0.0,
            frame_counter: seed,
            score: 0,
            lives: PLAYER_LIVES,
            wave,
            invuln_timer: RESPAWN_INVULN,
            wave_complete_handled: false,
            wave_just_completed: false,
            lost: false,
        }
    }

    fn kill_player(&mut self) {
        self.score -= 500;
        self.explosions.push(Explosion {
            x: self.player.x + PLAYER_WIDTH / 2.0 - 12.0,
            y: self.player.y + PLAYER_HEIGHT / 2.0 - 12.0,
            frame: 0,
            timer: EXPLOSION_FRAME_TICKS,
        });
        self.lives -= 1;
        // Reset powerups and inventory on death
        self.upgrades = PlayerUpgrades {
            fire_rate_mult: 1.0,
            fire_rate_stacks: 0,
            speed_mult: 1.0,
            speed_stacks: 0,
            bullet_count: 1,
        };
        self.inventory = PowerupInventory::default();
        self.currency = 0;
        if self.lives == 0 {
            self.lost = true;
        } else {
            self.player.x = GAME_WIDTH / 2.0 - PLAYER_WIDTH / 2.0;
            self.player.shield = None;
            self.invuln_timer = RESPAWN_INVULN;
            self.bullets.clear();
        }
    }

    /// Advance simulation by one fixed tick. Returns sound events.
    pub fn tick(&mut self, actions: &[Action]) -> crate::SoundEvents {
        let dt = FIXED_DT;
        let mut sfx = crate::SoundEvents::default();
        self.wave_just_completed = false;

        if self.lost {
            return sfx;
        }

        // Invuln countdown — only during active play
        if self.invuln_timer > 0.0 {
            self.invuln_timer -= dt;
        }

        self.frame_counter += 1;
        self.score -= 1; // time pressure — must kill to earn

        // Advance explosions
        self.explosions.retain_mut(|exp| {
            if exp.timer == 0 {
                exp.frame += 1;
                if exp.frame >= EXPLOSION_FRAMES {
                    return false; // done
                }
                exp.timer = EXPLOSION_FRAME_TICKS;
            }
            exp.timer -= 1;
            true
        });

        self.anim_timer += dt;
        if self.anim_timer >= 0.3 {
            self.anim_timer -= 0.3;
            self.anim_frame = !self.anim_frame;
        }

        // Player movement — always active
        let effective_player_speed = PLAYER_SPEED * self.upgrades.speed_mult;
        let has_left = actions.contains(&Action::Left);
        let has_right = actions.contains(&Action::Right);
        if has_left && !has_right {
            self.player.x -= effective_player_speed * dt;
        } else if has_right && !has_left {
            self.player.x += effective_player_speed * dt;
        }
        self.player.x = self.player.x.clamp(0.0, GAME_WIDTH - PLAYER_WIDTH);

        // Buy powerups/lives with currency.
        // Costs: Rate=1, Speed=1, Double=1, Triple=2, Shield=2, Life=2.
        {
            let (can_buy, cost) =
                if actions.contains(&Action::BuyRate) && self.upgrades.fire_rate_stacks < 2 {
                    (true, 1)
                } else if actions.contains(&Action::BuySpeed) && self.upgrades.speed_stacks < 2 {
                    (true, 1)
                } else if actions.contains(&Action::BuyDouble) && self.upgrades.bullet_count < 2 {
                    (true, 1)
                } else if actions.contains(&Action::BuyTriple) && self.upgrades.bullet_count < 3 {
                    (true, 2)
                } else if actions.contains(&Action::BuyShield) && self.player.shield.is_none() {
                    (true, 2)
                } else if actions.contains(&Action::BuyLife) {
                    (true, 2)
                } else {
                    (false, 0)
                };

            if can_buy && self.currency >= cost {
                self.currency -= cost;
                sfx.powerup_bought = true;
                if actions.contains(&Action::BuyRate) {
                    self.upgrades.fire_rate_mult *= 0.8;
                    self.upgrades.fire_rate_stacks += 1;
                } else if actions.contains(&Action::BuySpeed) {
                    self.upgrades.speed_mult *= 1.1;
                    self.upgrades.speed_stacks += 1;
                } else if actions.contains(&Action::BuyDouble) {
                    self.upgrades.bullet_count = 2;
                } else if actions.contains(&Action::BuyTriple) {
                    self.upgrades.bullet_count = 3;
                } else if actions.contains(&Action::BuyShield) {
                    self.player.shield = Some(Shield::new(self.frame_counter as u64));
                } else if actions.contains(&Action::BuyLife) {
                    self.lives += 1;
                }
            }
        }

        // Sell powerups/lives for currency.
        // Returns: 1 for everything, except damaged/critical shield = 0 (no-op).
        // Inventory items first, then active upgrades.
        {
            let sell_value = if actions.contains(&Action::SellRate) {
                if self.inventory.rate > 0 {
                    self.inventory.rate -= 1;
                    1
                } else if self.upgrades.fire_rate_stacks > 0 {
                    self.upgrades.fire_rate_mult /= 0.8;
                    self.upgrades.fire_rate_stacks -= 1;
                    1
                } else {
                    0
                }
            } else if actions.contains(&Action::SellSpeed) {
                if self.inventory.speed > 0 {
                    self.inventory.speed -= 1;
                    1
                } else if self.upgrades.speed_stacks > 0 {
                    self.upgrades.speed_mult /= 1.1;
                    self.upgrades.speed_stacks -= 1;
                    1
                } else {
                    0
                }
            } else if actions.contains(&Action::SellDouble) {
                if self.inventory.double > 0 {
                    self.inventory.double -= 1;
                    1
                } else if self.upgrades.bullet_count == 2 {
                    self.upgrades.bullet_count = 1;
                    1
                } else {
                    0
                }
            } else if actions.contains(&Action::SellTriple) {
                if self.inventory.triple > 0 {
                    self.inventory.triple -= 1;
                    1
                } else if self.upgrades.bullet_count == 3 {
                    self.upgrades.bullet_count = 2;
                    1
                } else {
                    0
                }
            } else if actions.contains(&Action::SellShield) {
                if self.inventory.shield > 0 {
                    self.inventory.shield -= 1;
                    1
                } else if self
                    .player
                    .shield
                    .as_ref()
                    .is_some_and(|s| s.level == ShieldLevel::Full)
                {
                    self.player.shield = None;
                    1
                } else {
                    0 // damaged/critical shield = worthless
                }
            } else if actions.contains(&Action::SellLife) {
                if self.lives > 1 {
                    self.lives -= 1;
                    1
                } else {
                    0
                }
            } else {
                0
            };
            if sell_value > 0 {
                self.currency += sell_value;
                sfx.powerup_sold = true;
            }
        }

        // Player shooting — always active, respects upgrades
        self.player.shoot_cooldown -= dt;
        let effective_cooldown = SHOOT_COOLDOWN * self.upgrades.fire_rate_mult;
        let effective_speed = BULLET_SPEED * self.upgrades.speed_mult;
        if actions.contains(&Action::Fire) && self.player.shoot_cooldown <= 0.0 {
            self.player.shoot_cooldown = effective_cooldown;
            let cx = self.player.x + PLAYER_WIDTH / 2.0;
            let by = self.player.y - BULLET_HEIGHT;
            match self.upgrades.bullet_count {
                3 => {
                    // Left, center, right
                    for &offset in &[-5.0, 0.0, 5.0] {
                        self.bullets.push(Bullet {
                            x: cx + offset - BULLET_WIDTH / 2.0,
                            y: by,
                            dy: -effective_speed,
                            active: true,
                        });
                    }
                }
                2 => {
                    // Left and right
                    for &offset in &[-4.0, 4.0] {
                        self.bullets.push(Bullet {
                            x: cx + offset - BULLET_WIDTH / 2.0,
                            y: by,
                            dy: -effective_speed,
                            active: true,
                        });
                    }
                }
                _ => {
                    self.bullets.push(Bullet {
                        x: cx - BULLET_WIDTH / 2.0,
                        y: by,
                        dy: -effective_speed,
                        active: true,
                    });
                }
            }
            sfx.player_fired = true;
        }

        // Process burst queues — spawn bullets from active bursts
        self.burst_queues.retain_mut(|burst| {
            if burst.ticks_until_next == 0 {
                self.bullets.push(Bullet {
                    x: burst.x,
                    y: burst.y,
                    dy: ENEMY_BULLET_SPEED,
                    active: true,
                });
                burst.remaining -= 1;
                burst.ticks_until_next = BURST_TICK_INTERVAL;
            } else {
                burst.ticks_until_next -= 1;
            }
            burst.remaining > 0
        });

        // Update bullets — always active
        for b in &mut self.bullets {
            if b.active {
                b.y += b.dy * dt;
                if b.y < -BULLET_HEIGHT || b.y > GAME_HEIGHT {
                    b.active = false;
                }
                // Spawn trail particle behind the bullet
                if b.active {
                    self.trail_particles.push(TrailParticle {
                        x: b.x + BULLET_WIDTH / 2.0,
                        y: if b.dy < 0.0 { b.y + BULLET_HEIGHT } else { b.y },
                        life: TRAIL_LIFETIME,
                        is_player: b.dy < 0.0,
                    });
                }
            }
        }

        // Update shotgun balls — move, bounce off walls
        let mut wall_explosions = Vec::new();
        for b in &mut self.shotgun_balls {
            if b.active {
                b.x += b.dx * dt;
                b.y += b.dy * dt;
                let hit_wall = b.x < 0.0 || b.x + SHOTGUN_BALL_SIZE > GAME_WIDTH;
                if hit_wall {
                    if b.bounces_left > 0 {
                        // Bounce
                        if b.x < 0.0 {
                            b.x = -b.x;
                            b.dx = -b.dx;
                        } else {
                            b.x = 2.0 * (GAME_WIDTH - SHOTGUN_BALL_SIZE) - b.x;
                            b.dx = -b.dx;
                        }
                        b.bounces_left -= 1;
                    } else if b.bounces_left == 0 && b.dx != 0.0 {
                        // Explode on wall
                        wall_explosions.push(Explosion {
                            x: b.x + SHOTGUN_BALL_SIZE / 2.0 - 12.0,
                            y: b.y + SHOTGUN_BALL_SIZE / 2.0 - 12.0,
                            frame: 0,
                            timer: EXPLOSION_FRAME_TICKS,
                        });
                        b.active = false;
                    }
                }
                // Remove if off screen
                if b.y < -SHOTGUN_BALL_SIZE || b.y > GAME_HEIGHT + SHOTGUN_BALL_SIZE {
                    b.active = false;
                }
                if b.x < -SHOTGUN_BALL_SIZE || b.x > GAME_WIDTH + SHOTGUN_BALL_SIZE {
                    b.active = false;
                }
                // Trail particle
                if b.active {
                    self.trail_particles.push(TrailParticle {
                        x: b.x + SHOTGUN_BALL_SIZE / 2.0,
                        y: b.y + SHOTGUN_BALL_SIZE / 2.0,
                        life: TRAIL_LIFETIME,
                        is_player: false,
                    });
                }
            }
        }
        self.shotgun_balls.retain(|b| b.active);
        self.explosions.extend(wall_explosions);

        // Age trail particles
        self.trail_particles.retain_mut(|p| {
            p.life -= 1;
            p.life > 0
        });

        // Enemy movement — always runs
        if !self.formation.intro_done() {
            self.formation.update_intro(dt);
        }
        self.formation.update_formation(dt);

        // Collision: player bullets vs enemies — always active
        let mut pending_powerups: Vec<Powerup> = Vec::new();
        for b in &mut self.bullets {
            if !b.active || b.dy > 0.0 {
                continue;
            }
            for e in &mut self.formation.enemies {
                if !e.alive {
                    continue;
                }
                if matches!(e.spawn, SpawnState::Waiting) {
                    continue;
                }
                if aabb_overlap(
                    b.x,
                    b.y,
                    BULLET_WIDTH,
                    BULLET_HEIGHT,
                    e.x,
                    e.y,
                    ENEMY_WIDTH,
                    ENEMY_HEIGHT,
                ) {
                    b.active = false;
                    self.score += 50; // hit bonus
                    if let Some(ref mut shield) = e.shield {
                        if !shield.hit() {
                            e.shield = None;
                        }
                        sfx.shield_hit = true;
                        sfx.enemy_shield_hit = true;
                    } else {
                        let drop_powerup = (e.class == EnemyClass::Red
                            || e.class == EnemyClass::White)
                            && (self.frame_counter % 10 == 0); // 10% chance
                        if drop_powerup {
                            // Pick a random powerup kind based on frame counter
                            let kind = match (self.frame_counter / 3) % 6 {
                                0 => PowerupKind::Life,
                                1 => PowerupKind::Rate,
                                2 => PowerupKind::Speed,
                                3 => PowerupKind::Double,
                                4 => PowerupKind::Triple,
                                _ => PowerupKind::Shield,
                            };
                            pending_powerups.push(Powerup {
                                x: e.x,
                                y: e.y,
                                kind,
                            });
                        }
                        e.alive = false;
                        self.score += e.class.stats().score as i32;
                        self.explosions.push(Explosion {
                            x: e.x + ENEMY_WIDTH / 2.0 - 12.0,
                            y: e.y + ENEMY_HEIGHT / 2.0 - 12.0,
                            frame: 0,
                            timer: EXPLOSION_FRAME_TICKS,
                        });
                        sfx.enemy_hit = true;
                    }
                    break;
                }
            }
        }
        self.powerups.extend(pending_powerups);

        self.bullets.retain(|b| b.active);

        // Update powerups: fall down, collect on player overlap
        for p in &mut self.powerups {
            p.y += POWERUP_FALL_SPEED * dt;
        }
        // Collect
        let player_x = self.player.x;
        let player_y = self.player.y;
        let upgrades = &mut self.upgrades;
        let inventory = &mut self.inventory;
        let score = &mut self.score;
        let lives = &mut self.lives;
        let player_shield = &mut self.player.shield;
        let frame_counter = self.frame_counter;
        let mut picked_any = false;
        self.powerups.retain(|p| {
            if p.y > GAME_HEIGHT {
                return false; // fell off screen
            }
            if aabb_overlap(
                p.x,
                p.y,
                POWERUP_SIZE,
                POWERUP_SIZE,
                player_x,
                player_y,
                PLAYER_WIDTH,
                PLAYER_HEIGHT,
            ) {
                *score += 500; // powerup capture bonus
                picked_any = true;
                apply_powerup(
                    p.kind,
                    upgrades,
                    inventory,
                    lives,
                    player_shield,
                    frame_counter,
                );
                return false; // consumed
            }
            true
        });
        if picked_any {
            sfx.powerup_picked = true;
        }

        if self.formation.all_dead() && !self.wave_complete_handled {
            self.wave_complete_handled = true;
            self.wave_just_completed = true;
            self.score += 5000; // wave complete bonus
            // Auto-collect all remaining powerups
            for p in self.powerups.drain(..) {
                self.score += 500; // powerup capture bonus
                apply_powerup(
                    p.kind,
                    &mut self.upgrades,
                    &mut self.inventory,
                    &mut self.lives,
                    &mut self.player.shield,
                    self.frame_counter,
                );
            }
            // Immediately start next wave
            self.wave += 1;
            let config = WaveConfig::for_wave(self.wave);
            self.formation = EnemyFormation::new(&config);
            self.bullets.clear();
            self.shotgun_balls.clear();
            self.burst_queues.clear();
            self.invuln_timer = RESPAWN_INVULN;
            self.wave_complete_handled = false;
        }

        // Enemy shooting — runs during intro too, faster when enemies are in motion
        let has_moving = self.formation.enemies.iter().any(|e| {
            e.alive
                && (!matches!(e.dive, DivePhase::None)
                    || matches!(e.spawn, SpawnState::FlyingIn { .. }))
        });
        let shoot_rate = if has_moving { 5.0 } else { 1.0 };
        self.enemy_shoot_timer -= dt * shoot_rate;
        if self.enemy_shoot_timer <= 0.0 {
            self.enemy_shoot_timer = ENEMY_SHOOT_INTERVAL;
            // Build weighted candidate list: diving/intro enemies get 20x weight
            let mut candidates: Vec<usize> = Vec::new();
            for (i, e) in self.formation.enemies.iter().enumerate() {
                if !e.alive || matches!(e.spawn, SpawnState::Waiting) {
                    continue;
                }
                // Only shoot if on screen
                if e.y < 0.0 || e.y > GAME_HEIGHT {
                    continue;
                }
                let is_moving = !matches!(e.dive, DivePhase::None)
                    || matches!(e.spawn, SpawnState::FlyingIn { .. });
                let weight = if is_moving {
                    // White enemies fire less aggressively while moving
                    if e.class == EnemyClass::White {
                        20
                    } else {
                        100
                    }
                } else {
                    1
                };
                for _ in 0..weight {
                    candidates.push(i);
                }
            }
            if !candidates.is_empty() {
                let shooter = candidates[self.frame_counter as usize % candidates.len()];
                let e = &self.formation.enemies[shooter];
                let ex = e.x + ENEMY_WIDTH / 2.0;
                let ey = e.y + ENEMY_HEIGHT;
                let is_white = e.class == EnemyClass::White;
                // White enemies: 50% chance shotgun, 50% regular
                let use_shotgun = is_white && (self.frame_counter % 10 < 3);

                if use_shotgun {
                    // 3 pairs at 60°, 45°, 30° from vertical (symmetric)
                    let speed = ENEMY_BULLET_SPEED * 1.5;
                    let angles_deg: [f32; 3] = [60.0, 45.0, 30.0];
                    for &angle_deg in &angles_deg {
                        let angle_rad = angle_deg * std::f32::consts::PI / 180.0;
                        let dx = speed * angle_rad.sin();
                        let dy = speed * angle_rad.cos();
                        // Left ball
                        self.shotgun_balls.push(ShotgunBall {
                            x: ex - SHOTGUN_BALL_SIZE / 2.0,
                            y: ey,
                            dx: -dx,
                            dy,
                            active: true,
                            bounces_left: 1,
                        });
                        // Right ball
                        self.shotgun_balls.push(ShotgunBall {
                            x: ex - SHOTGUN_BALL_SIZE / 2.0,
                            y: ey,
                            dx,
                            dy,
                            active: true,
                            bounces_left: 1,
                        });
                    }
                } else if e.class == EnemyClass::Red {
                    // Red enemies fire a burst
                    self.bullets.push(Bullet {
                        x: ex - BULLET_WIDTH / 2.0,
                        y: ey,
                        dy: ENEMY_BULLET_SPEED,
                        active: true,
                    });
                    self.burst_queues.push(BurstQueue {
                        x: ex - BULLET_WIDTH / 2.0,
                        y: ey,
                        remaining: BURST_SIZE - 1, // first bullet already fired
                        ticks_until_next: BURST_TICK_INTERVAL,
                    });
                } else {
                    self.bullets.push(Bullet {
                        x: ex - BULLET_WIDTH / 2.0,
                        y: ey,
                        dy: ENEMY_BULLET_SPEED,
                        active: true,
                    });
                }
                sfx.enemy_fired = true;
            }
        }

        // During intro, skip enemy diving/enemy-vs-player collisions
        if !self.formation.intro_done() {
            return sfx;
        }

        // Enemy diving
        self.formation.trigger_dive(dt);
        self.formation.update_dives(dt);

        // Red enemies: spiral fire during dive (every 8 frames)
        if self.frame_counter % 8 == 0 {
            for e in &self.formation.enemies {
                if !e.alive || e.class != EnemyClass::Red {
                    continue;
                }
                if let DivePhase::Diving { spin_angle, .. } = &e.dive {
                    let speed = ENEMY_BULLET_SPEED * 1.2;
                    let bx = e.x + ENEMY_WIDTH / 2.0 - BULLET_WIDTH / 2.0;
                    let by = e.y + ENEMY_HEIGHT / 2.0;
                    let dx = spin_angle.sin() * speed;
                    let dy = spin_angle.cos() * speed;
                    self.shotgun_balls.push(ShotgunBall {
                        x: bx,
                        y: by,
                        dx,
                        dy,
                        active: true,
                        bounces_left: 1,
                    });
                }
            }
        }

        // Yellow enemies: fire horizontal pair during dive (every 72 frames)
        if self.frame_counter % 72 == 0 {
            for e in &self.formation.enemies {
                if !e.alive || e.class != EnemyClass::Yellow {
                    continue;
                }
                if matches!(e.dive, DivePhase::Diving { .. }) {
                    let speed = ENEMY_BULLET_SPEED;
                    let bx = e.x + ENEMY_WIDTH / 2.0 - SHOTGUN_BALL_SIZE / 2.0;
                    let by = e.y + ENEMY_HEIGHT / 2.0;
                    // Left
                    self.shotgun_balls.push(ShotgunBall {
                        x: bx,
                        y: by,
                        dx: -speed,
                        dy: 0.0,
                        active: true,
                        bounces_left: 0,
                    });
                    // Right
                    self.shotgun_balls.push(ShotgunBall {
                        x: bx,
                        y: by,
                        dx: speed,
                        dy: 0.0,
                        active: true,
                        bounces_left: 0,
                    });
                }
            }
        }

        let is_vulnerable = self.invuln_timer <= 0.0;

        if is_vulnerable {
            for b in &mut self.bullets {
                if !b.active || b.dy < 0.0 {
                    continue;
                }
                if aabb_overlap(
                    b.x,
                    b.y,
                    BULLET_WIDTH,
                    BULLET_HEIGHT,
                    self.player.x,
                    self.player.y,
                    PLAYER_WIDTH,
                    PLAYER_HEIGHT,
                ) {
                    b.active = false;
                    if let Some(ref mut shield) = self.player.shield {
                        if !shield.hit() {
                            self.player.shield = None;
                        }
                        sfx.shield_hit = true;
                        sfx.player_shield_hit = true;
                        self.score -= 200;
                    } else {
                        sfx.player_hit = true;
                        self.kill_player();
                        return sfx;
                    }
                }
            }
        }

        // Shotgun balls vs player
        if is_vulnerable {
            for b in &mut self.shotgun_balls {
                if !b.active {
                    continue;
                }
                if aabb_overlap(
                    b.x,
                    b.y,
                    SHOTGUN_BALL_SIZE,
                    SHOTGUN_BALL_SIZE,
                    self.player.x,
                    self.player.y,
                    PLAYER_WIDTH,
                    PLAYER_HEIGHT,
                ) {
                    b.active = false;
                    if let Some(ref mut shield) = self.player.shield {
                        if !shield.hit() {
                            self.player.shield = None;
                        }
                        sfx.shield_hit = true;
                        sfx.player_shield_hit = true;
                        self.score -= 200;
                    } else {
                        sfx.player_hit = true;
                        self.kill_player();
                        return sfx;
                    }
                }
            }
        }

        if is_vulnerable {
            for e in &self.formation.enemies {
                if e.alive
                    && !matches!(e.dive, DivePhase::None)
                    && aabb_overlap(
                        e.x,
                        e.y,
                        ENEMY_WIDTH,
                        ENEMY_HEIGHT,
                        self.player.x,
                        self.player.y,
                        PLAYER_WIDTH,
                        PLAYER_HEIGHT,
                    )
                {
                    if let Some(ref mut shield) = self.player.shield {
                        if !shield.hit() {
                            self.player.shield = None;
                        }
                        sfx.shield_hit = true;
                        sfx.player_shield_hit = true;
                        self.score -= 200;
                    } else {
                        sfx.player_hit = true;
                        self.kill_player();
                        return sfx;
                    }
                }
            }
        }

        self.bullets.retain(|b| b.active);

        if self.formation.reached_player() {
            self.lost = true;
            return sfx;
        }

        sfx
    }
}
