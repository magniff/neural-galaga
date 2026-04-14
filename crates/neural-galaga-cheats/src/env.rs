//! Environment that wraps GameSim and emits structured observations.
//!
//! Observations are 2-frame stacked: `[prev_frame || current_frame]`, giving the
//! network implicit velocity/acceleration information.

use std::sync::atomic::{AtomicU32, Ordering};

use neural_galaga_core::{
    Action, ENEMY_HEIGHT, ENEMY_WIDTH, GameSim, GameStatus, PLAYER_HEIGHT, PLAYER_WIDTH, StepResult,
};

use crate::obs::{OBS_SIZE, encode};

/// Frame skip — how many sim ticks between agent decisions.
pub const FRAME_SKIP: usize = 2;

/// Number of frames stacked in the observation.
pub const FRAME_STACK: usize = 2;

/// Total observation size exposed to the model (2 × single-frame obs).
pub const STACKED_OBS_SIZE: usize = FRAME_STACK * OBS_SIZE;

/// Current shield-block penalty magnitude, stored as f32 bits in an AtomicU32.
/// Starts at 0.0 and ramps to `SHIELD_PENALTY_MAX` over `SHIELD_PENALTY_RAMP_STEPS`
/// once training performance crosses the activation threshold.
static SHIELD_PENALTY_BITS: AtomicU32 = AtomicU32::new(0); // 0.0f32

/// Maximum shield-block penalty (negative value applied as reward).
pub const SHIELD_PENALTY_MAX: f32 = 5.0;
/// Number of steps over which the penalty ramps from 0 to max.
pub const SHIELD_PENALTY_RAMP_STEPS: u64 = 20_000_000_000;

/// Set the current shield-block penalty magnitude. Called by the trainer.
pub fn set_shield_penalty(value: f32) {
    SHIELD_PENALTY_BITS.store(value.to_bits(), Ordering::Relaxed);
}

/// Get the current shield-block penalty magnitude.
pub fn shield_penalty() -> f32 {
    f32::from_bits(SHIELD_PENALTY_BITS.load(Ordering::Relaxed))
}

fn action_to_list(action: usize) -> Vec<Action> {
    match action {
        0 => vec![],
        1 => vec![Action::Left],
        2 => vec![Action::Right],
        3 => vec![Action::Fire],
        4 => vec![Action::Left, Action::Fire],
        5 => vec![Action::Right, Action::Fire],
        6 => vec![Action::BuyRate],
        7 => vec![Action::BuySpeed],
        8 => vec![Action::BuyDouble],
        9 => vec![Action::BuyTriple],
        10 => vec![Action::BuyShield],
        11 => vec![Action::BuyLife],
        12 => vec![Action::SellRate],
        13 => vec![Action::SellSpeed],
        14 => vec![Action::SellDouble],
        15 => vec![Action::SellTriple],
        16 => vec![Action::SellShield],
        17 => vec![Action::SellLife],
        _ => vec![],
    }
}

/// Count nearby downward projectiles (enemy bullets + shotgun) within `radius` of the player.
fn count_nearby_projectiles(result: &StepResult, radius: f32) -> usize {
    let px = result.player.x + PLAYER_WIDTH / 2.0;
    let py = result.player.y + PLAYER_HEIGHT / 2.0;
    let r2 = radius * radius;
    let mut count = 0;

    for b in &result.bullets {
        if b.dy <= 0.0 {
            continue;
        }
        let dx = b.x - px;
        let dy = b.y - py;
        if dx * dx + dy * dy < r2 {
            count += 1;
        }
    }
    for s in &result.shotgun_balls {
        if s.dy <= 0.0 {
            continue;
        }
        let dx = s.x - px;
        let dy = s.y - py;
        if dx * dx + dy * dy < r2 {
            count += 1;
        }
    }
    count
}

/// Single env. Returns flat float observations of length [`STACKED_OBS_SIZE`].
pub struct CheatsEnv {
    sim: GameSim,
    /// Per-wave kill count from the most recent step (resets each new wave).
    prev_kills: usize,
    /// Lifetime kill count for the current episode (does not reset on wave transitions).
    total_kills: usize,
    done: bool,
    /// Previous frame's observation (for 2-frame stacking).
    prev_obs: Vec<f32>,
}

impl CheatsEnv {
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            sim: GameSim::with_seed_and_wave(seed, 1),
            prev_kills: 0,
            total_kills: 0,
            done: false,
            prev_obs: vec![0.0; OBS_SIZE],
        }
    }

    fn stack_obs(&self, current: &[f32]) -> Vec<f32> {
        let mut stacked = Vec::with_capacity(STACKED_OBS_SIZE);
        stacked.extend_from_slice(&self.prev_obs);
        stacked.extend_from_slice(current);
        stacked
    }

    /// Reset to a fresh game starting at wave 1. Returns initial observation.
    pub fn reset(&mut self) -> Vec<f32> {
        self.sim.reset_to_wave(1);
        self.prev_kills = 0;
        self.total_kills = 0;
        self.done = false;
        self.prev_obs = vec![0.0; OBS_SIZE];
        // Step once with no actions to populate the initial state.
        let r = self.sim.step(&[]);
        let current = encode(&r);
        let stacked = self.stack_obs(&current);
        self.prev_obs = current;
        stacked
    }

    /// Take an action. Returns `(observation, reward, done)`.
    pub fn step(&mut self, action: usize) -> (Vec<f32>, f32, bool) {
        if self.done {
            return (vec![0.0; STACKED_OBS_SIZE], 0.0, true);
        }

        let actions = action_to_list(action);
        let mut total_reward = 0.0f32;
        let mut last_result: Option<StepResult> = None;

        for _ in 0..FRAME_SKIP {
            let r = self.sim.step(&actions);

            // Kill reward: handle per-wave reset of `enemies_killed`.
            let kills_delta = if r.enemies_killed >= self.prev_kills {
                r.enemies_killed - self.prev_kills
            } else {
                r.enemies_killed
            };
            total_reward += kills_delta as f32;
            self.total_kills += kills_delta;
            self.prev_kills = r.enemies_killed;

            // Sound-event-driven shaping
            if r.sound_events.powerup_picked {
                total_reward += 5.0;
            }
            if r.sound_events.enemy_shield_hit {
                total_reward += 0.5;
            }
            if r.sound_events.player_shield_hit {
                total_reward -= shield_penalty();
            }
            if r.sound_events.player_hit {
                total_reward -= 3.0;
            }

            // Proximity penalty: penalize being close to alive enemies.
            if !r.is_invulnerable {
                let px = r.player.x + PLAYER_WIDTH / 2.0;
                let py = r.player.y + PLAYER_HEIGHT / 2.0;
                const PROXIMITY_SCALE: f32 = 20.0;
                let mut proximity_penalty = 0.0f32;
                for e in &r.enemies {
                    if !e.alive {
                        continue;
                    }
                    let ex = e.x + ENEMY_WIDTH / 2.0;
                    let ey = e.y + ENEMY_HEIGHT / 2.0;
                    let dx = px - ex;
                    let dy = py - ey;
                    let dist = (dx * dx + dy * dy).sqrt();
                    proximity_penalty += (-dist / PROXIMITY_SCALE).exp();
                }
                total_reward -= proximity_penalty * 0.01;
            }

            // Survival bonus: reward for having nearby projectiles and not getting hit.
            // Only applies when vulnerable and no player_hit this tick.
            if !r.is_invulnerable && !r.sound_events.player_hit {
                let nearby = count_nearby_projectiles(&r, 40.0);
                total_reward += nearby as f32 * 0.05;
            }

            let done = r.status == GameStatus::Lost;
            last_result = Some(r);
            if done {
                self.done = true;
                break;
            }
        }

        let result = last_result.unwrap();
        let current = encode(&result);
        let stacked = self.stack_obs(&current);
        self.prev_obs = current;
        (stacked, total_reward, self.done)
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn score(&self) -> i32 {
        0
    }

    /// Lifetime kill count for the current episode (across all waves).
    pub fn kills(&self) -> usize {
        self.total_kills
    }

    pub fn lives(&self) -> u32 {
        self.sim.lives()
    }
}

impl Default for CheatsEnv {
    fn default() -> Self {
        Self::new()
    }
}
