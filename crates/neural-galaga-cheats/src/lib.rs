//! Galaga "cheats" agent: structured-state RL.
//!
//! The agent bypasses pixel perception entirely. Each step, the environment encodes
//! the full game state (player position, enemy formation, bullets, powerups, etc.)
//! into a fixed-length float vector, which feeds an MLP actor-critic.
//!
//! Crate layout:
//! - [obs] — encode `StepResult` into a fixed-size observation vector
//! - [env] — `CheatsEnv`, mirrors `GalagaEnv` but emits structured observations
//! - [model] — MLP actor-critic
//! - [ppo] — PPO training loop
//!
//! Binaries:
//! - `train` — train the cheats model
//!
//! The inference binary lives in `neural-galaga-realtime` (it needs the GPU/audio
//! window stack), in `bin/infer_cheats.rs`.

pub mod env;
pub mod model;
pub mod obs;
pub mod ppo;

/// Number of discrete actions the agent can take.
/// 0=noop 1=left 2=right 3=fire 4=left+fire 5=right+fire
/// 6=buy_rate 7=buy_speed 8=buy_double 9=buy_triple 10=buy_shield 11=buy_life
/// 12=sell_rate 13=sell_speed 14=sell_double 15=sell_triple 16=sell_shield 17=sell_life
pub const NUM_ACTIONS: usize = 18;
