//! PPO training for the cheats agent.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};

use burn::grad_clipping::GradientClippingConfig;
use burn::module::AutodiffModule;
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::*;
use burn::record::CompactRecorder;
use rayon::prelude::*;

use crate::NUM_ACTIONS;
use crate::env::STACKED_OBS_SIZE;
use crate::env::{CheatsEnv, SHIELD_PENALTY_MAX, SHIELD_PENALTY_RAMP_STEPS, set_shield_penalty};
use crate::model::{CheatsActorCritic, CheatsActorCriticConfig};

/// Avg reward threshold that activates the shield-penalty ramp.
const SHIELD_PENALTY_ACTIVATION_REWARD: f64 = 120.0;

// --- Hyperparameters ---
const NUM_ENVS: usize = 512;
const ROLLOUT_STEPS: usize = 128;
const MINIBATCH_SIZE: usize = 2048;
const PPO_EPOCHS: usize = 4;
const GAMMA: f32 = 0.99;
const GAE_LAMBDA: f32 = 0.95;
const CLIP_EPS: f32 = 0.1;
const VALUE_COEFF: f32 = 0.5;
const ENTROPY_COEFF: f32 = 0.01;
const LR_MAX: f64 = 3e-4;
const LR_MIN: f64 = 1e-5;
const LR_DECAY_UPDATES: usize = 2000;
const MAX_GRAD_NORM: f32 = 0.5;
const REWARD_WINDOW: usize = 100;
const LOG_INTERVAL: usize = 1;
const CHECKPOINT_INTERVAL: usize = 10;
const CHECKPOINT_DIR: &str = "checkpoints/cheats";
/// CSV log of update → avg_reward. Truncated and re-created on each
/// `train()` call so each run starts fresh.
const CSV_LOG_PATH: &str = "checkpoints/cheats/training_log.csv";

/// A single transition stored during rollout collection.
struct Transition {
    obs: Vec<f32>,
    action: usize,
    log_prob: f32,
    reward: f32,
    done: bool,
    value: f32,
}

fn obs_to_tensor<B: Backend>(obs: &[Vec<f32>], device: &B::Device) -> Tensor<B, 2> {
    let batch = obs.len();
    let mut data = Vec::with_capacity(batch * STACKED_OBS_SIZE);
    for o in obs {
        data.extend_from_slice(o);
    }
    Tensor::<B, 1>::from_floats(data.as_slice(), device).reshape([batch, STACKED_OBS_SIZE])
}

fn compute_gae(
    rewards: &[f32],
    values: &[f32],
    dones: &[bool],
    last_value: f32,
) -> (Vec<f32>, Vec<f32>) {
    let len = rewards.len();
    let mut advantages = vec![0.0f32; len];
    let mut gae = 0.0f32;
    for t in (0..len).rev() {
        let next_value = if t == len - 1 {
            last_value
        } else {
            values[t + 1]
        };
        let next_non_terminal = if t == len - 1 {
            if dones[t] { 0.0 } else { 1.0 }
        } else if dones[t] {
            0.0
        } else {
            1.0
        };
        let delta = rewards[t] + GAMMA * next_value * next_non_terminal - values[t];
        gae = delta + GAMMA * GAE_LAMBDA * next_non_terminal * gae;
        advantages[t] = gae;
    }
    let returns: Vec<f32> = advantages
        .iter()
        .zip(values.iter())
        .map(|(a, v)| a + v)
        .collect();
    (advantages, returns)
}

pub fn train<B: burn::tensor::backend::AutodiffBackend>(resume_from: Option<String>) {
    let device = B::Device::default();

    let mut model = CheatsActorCriticConfig.init::<B>(&device);
    if let Some(ref path) = resume_from {
        model = model
            .load_file(path.as_str(), &CompactRecorder::new(), &device)
            .unwrap_or_else(|e| panic!("failed to load checkpoint at {path}: {e}"));
        log::info!("loaded checkpoint from {path}");
    }
    let mut optimizer = AdamConfig::new()
        .with_grad_clipping(Some(GradientClippingConfig::Norm(MAX_GRAD_NORM)))
        .init::<B, CheatsActorCritic<B>>();

    let mut envs: Vec<CheatsEnv> = (0..NUM_ENVS)
        .map(|i| CheatsEnv::with_seed(i as u64 * 7919 + 1))
        .collect();
    let mut current_obs: Vec<Vec<f32>> = envs.iter_mut().map(|e| e.reset()).collect();

    std::fs::create_dir_all(CHECKPOINT_DIR).expect("failed to create checkpoint dir");

    let csv_file = File::create(CSV_LOG_PATH).expect("failed to create training_log.csv");
    let mut csv_writer = BufWriter::new(csv_file);
    writeln!(csv_writer, "update,avg_reward").expect("failed to write CSV header");

    let mut episode_rewards = vec![0.0f32; NUM_ENVS];
    let mut recent_episode_rewards: VecDeque<f32> = VecDeque::with_capacity(REWARD_WINDOW);
    let mut total_episodes = 0u64;
    let mut best_avg_reward = f64::NEG_INFINITY;
    let start_time = std::time::Instant::now();

    let mut shield_penalty_start_step: Option<u64> = None;

    type IB<B> = <B as burn::tensor::backend::AutodiffBackend>::InnerBackend;

    for update in 0usize.. {
        let rollout_start = std::time::Instant::now();

        // --- Rollout collection (no-grad via model.valid()) ---
        let inference_model = model.valid();
        let mut all_transitions: Vec<Vec<Transition>> = (0..NUM_ENVS)
            .map(|_| Vec::with_capacity(ROLLOUT_STEPS))
            .collect();

        for _step in 0..ROLLOUT_STEPS {
            let obs_tensor = obs_to_tensor::<IB<B>>(&current_obs, &device);
            let (actions, output) = inference_model.sample_actions(obs_tensor);

            let log_probs_data: Vec<f32> = output.log_probs.to_data().to_vec().unwrap();
            let values_data: Vec<f32> = output.value.to_data().to_vec().unwrap();

            let step_results: Vec<_> = envs
                .par_iter_mut()
                .enumerate()
                .map(|(i, env)| {
                    let action = actions[i];
                    let (next_obs, reward, done) = env.step(action);
                    let reset_obs = if done { Some(env.reset()) } else { None };
                    (i, action, next_obs, reward, done, reset_obs)
                })
                .collect();

            for (i, action, next_obs, reward, done, reset_obs) in step_results {
                let log_prob = log_probs_data[i * NUM_ACTIONS + action];
                let value = values_data[i];

                all_transitions[i].push(Transition {
                    obs: current_obs[i].clone(),
                    action,
                    log_prob,
                    reward,
                    done,
                    value,
                });

                episode_rewards[i] += reward;

                if done {
                    total_episodes += 1;
                    if recent_episode_rewards.len() == REWARD_WINDOW {
                        recent_episode_rewards.pop_front();
                    }
                    recent_episode_rewards.push_back(episode_rewards[i]);
                    episode_rewards[i] = 0.0;
                    current_obs[i] = reset_obs.unwrap();
                } else {
                    current_obs[i] = next_obs;
                }
            }
        }

        // --- Compute last values for GAE bootstrap ---
        let last_obs_tensor = obs_to_tensor::<IB<B>>(&current_obs, &device);
        let last_output = inference_model.forward(last_obs_tensor);
        let last_values: Vec<f32> = last_output.value.to_data().to_vec().unwrap();

        let mut env_advantages: Vec<Vec<f32>> = Vec::with_capacity(NUM_ENVS);
        let mut env_returns: Vec<Vec<f32>> = Vec::with_capacity(NUM_ENVS);
        for i in 0..NUM_ENVS {
            let rewards: Vec<f32> = all_transitions[i].iter().map(|t| t.reward).collect();
            let values: Vec<f32> = all_transitions[i].iter().map(|t| t.value).collect();
            let dones: Vec<bool> = all_transitions[i].iter().map(|t| t.done).collect();
            let (advantages, returns) = compute_gae(&rewards, &values, &dones, last_values[i]);
            env_advantages.push(advantages);
            env_returns.push(returns);
        }

        // Normalize advantages
        let total_transitions = NUM_ENVS * ROLLOUT_STEPS;
        let adv_mean: f32 = env_advantages.iter().flatten().sum::<f32>() / total_transitions as f32;
        let adv_var: f32 = env_advantages
            .iter()
            .flatten()
            .map(|a| (a - adv_mean).powi(2))
            .sum::<f32>()
            / total_transitions as f32;
        let adv_std = (adv_var + 1e-8).sqrt();
        for env_advs in &mut env_advantages {
            for a in env_advs.iter_mut() {
                *a = (*a - adv_mean) / adv_std;
            }
        }

        // --- Flatten all transitions into a single pool for minibatch sampling ---
        let mut all_obs: Vec<Vec<f32>> = Vec::with_capacity(total_transitions);
        let mut all_actions: Vec<usize> = Vec::with_capacity(total_transitions);
        let mut all_old_lp: Vec<f32> = Vec::with_capacity(total_transitions);
        let mut all_adv: Vec<f32> = Vec::with_capacity(total_transitions);
        let mut all_ret: Vec<f32> = Vec::with_capacity(total_transitions);

        for env_idx in 0..NUM_ENVS {
            for step in 0..ROLLOUT_STEPS {
                let t = &all_transitions[env_idx][step];
                all_obs.push(t.obs.clone());
                all_actions.push(t.action);
                all_old_lp.push(t.log_prob);
                all_adv.push(env_advantages[env_idx][step]);
                all_ret.push(env_returns[env_idx][step]);
            }
        }

        let batch_return_sum: f32 = all_ret.iter().sum();
        let mean_return = batch_return_sum / total_transitions as f32;

        // --- LR schedule (cosine LR_MAX → LR_MIN over LR_DECAY_UPDATES, then floor) ---
        let lr = if update >= LR_DECAY_UPDATES {
            LR_MIN
        } else {
            let progress = update as f64 / LR_DECAY_UPDATES as f64;
            let cosine = 0.5 * (1.0 + (std::f64::consts::PI * progress).cos());
            LR_MIN + (LR_MAX - LR_MIN) * cosine
        };

        // --- PPO updates ---
        let mut indices: Vec<usize> = (0..total_transitions).collect();

        for _epoch in 0..PPO_EPOCHS {
            // Shuffle indices
            for i in (1..indices.len()).rev() {
                let j = (rand::random::<f32>() * (i + 1) as f32) as usize % (i + 1);
                indices.swap(i, j);
            }

            for mb_start in (0..total_transitions).step_by(MINIBATCH_SIZE) {
                let mb_end = (mb_start + MINIBATCH_SIZE).min(total_transitions);
                let mb_indices = &indices[mb_start..mb_end];
                let mb_size = mb_indices.len();

                // Gather minibatch data
                let mb_obs: Vec<Vec<f32>> =
                    mb_indices.iter().map(|&i| all_obs[i].clone()).collect();
                let obs_tensor = obs_to_tensor::<B>(&mb_obs, &device);
                let output = model.forward(obs_tensor);

                let mut action_mask_data = vec![0.0f32; mb_size * NUM_ACTIONS];
                for (i, &idx) in mb_indices.iter().enumerate() {
                    action_mask_data[i * NUM_ACTIONS + all_actions[idx]] = 1.0;
                }
                let action_mask = Tensor::<B, 1>::from_floats(action_mask_data.as_slice(), &device)
                    .reshape([mb_size, NUM_ACTIONS]);
                let new_log_probs = (output.log_probs.clone() * action_mask)
                    .sum_dim(1)
                    .reshape([mb_size]);

                let values = output.value.flatten(0, 1);

                let probs = output.log_probs.clone().exp();
                let entropy = -(probs * output.log_probs).sum_dim(1).mean();

                let mb_old_lp: Vec<f32> = mb_indices.iter().map(|&i| all_old_lp[i]).collect();
                let mb_adv: Vec<f32> = mb_indices.iter().map(|&i| all_adv[i]).collect();
                let mb_ret: Vec<f32> = mb_indices.iter().map(|&i| all_ret[i]).collect();

                let old_log_probs = Tensor::<B, 1>::from_floats(mb_old_lp.as_slice(), &device);
                let advantages = Tensor::<B, 1>::from_floats(mb_adv.as_slice(), &device);
                let returns = Tensor::<B, 1>::from_floats(mb_ret.as_slice(), &device);

                let ratio = (new_log_probs - old_log_probs).exp();
                let surr1 = ratio.clone() * advantages.clone();
                let surr2 = ratio.clamp(1.0 - CLIP_EPS, 1.0 + CLIP_EPS) * advantages;
                let stacked: Tensor<B, 2> = Tensor::cat(
                    vec![surr1.reshape([mb_size, 1]), surr2.reshape([mb_size, 1])],
                    1,
                );
                let min_surr: Tensor<B, 1> = stacked.min_dim(1).reshape([mb_size]);
                let policy_loss = -min_surr.mean();
                let value_loss = (values - returns).powf_scalar(2.0).mean();
                let loss = policy_loss + value_loss * VALUE_COEFF - entropy * ENTROPY_COEFF;

                let grads = loss.backward();
                let grad_params = GradientsParams::from_grads(grads, &model);
                model = optimizer.step(lr, model, grad_params);
            }
        }

        // --- Logging ---
        let update_elapsed = rollout_start.elapsed().as_secs_f32();
        let total_steps = (update + 1) as u64 * NUM_ENVS as u64 * ROLLOUT_STEPS as u64;
        let total_elapsed = start_time.elapsed().as_secs_f32();
        let steps_per_sec = total_steps as f32 / total_elapsed;

        if update % LOG_INTERVAL == 0 {
            let avg_reward = if !recent_episode_rewards.is_empty() {
                recent_episode_rewards.iter().sum::<f32>() as f64
                    / recent_episode_rewards.len() as f64
            } else {
                0.0
            };

            if shield_penalty_start_step.is_none()
                && recent_episode_rewards.len() == REWARD_WINDOW
                && avg_reward >= SHIELD_PENALTY_ACTIVATION_REWARD
            {
                shield_penalty_start_step = Some(total_steps);
                log::info!(
                    "shield penalty ramp activated at step {} (avg_reward {:.1} ≥ {:.1})",
                    total_steps,
                    avg_reward,
                    SHIELD_PENALTY_ACTIVATION_REWARD
                );
            }
            if let Some(start) = shield_penalty_start_step {
                let elapsed = total_steps.saturating_sub(start);
                let t = (elapsed as f32 / SHIELD_PENALTY_RAMP_STEPS as f32).min(1.0);
                set_shield_penalty(t * SHIELD_PENALTY_MAX);
            }

            let current_penalty = crate::env::shield_penalty();
            let sample_kills: Vec<usize> = envs.iter().take(4).map(|e| e.kills()).collect();
            let sample_lives: Vec<u32> = envs.iter().take(4).map(|e| e.lives()).collect();

            let _ = writeln!(csv_writer, "{},{:.4}", update, avg_reward);
            let _ = csv_writer.flush();

            log::info!(
                "update {} | ep: {} | avg_ep_reward: {:.1} | mean_return: {:.2} | steps: {} | {:.0} sps | {:.1}s | lr: {:.2e} | shield_pen: {:.2} | kills: {:?} | lives: {:?}",
                update,
                total_episodes,
                avg_reward,
                mean_return,
                total_steps,
                steps_per_sec,
                update_elapsed,
                lr,
                current_penalty,
                sample_kills,
                sample_lives,
            );

            if recent_episode_rewards.len() == REWARD_WINDOW && avg_reward > best_avg_reward {
                best_avg_reward = avg_reward;
                model
                    .clone()
                    .save_file(format!("{CHECKPOINT_DIR}/best"), &CompactRecorder::new())
                    .expect("failed to save best model");
                log::info!("new best model saved (avg_reward: {:.1})", avg_reward);
            }
        }

        if update % CHECKPOINT_INTERVAL == 0 && update > 0 {
            model
                .clone()
                .save_file(
                    format!("{CHECKPOINT_DIR}/update_{update:06}"),
                    &CompactRecorder::new(),
                )
                .expect("failed to save checkpoint");
            log::info!("checkpoint saved at update {update}");
        }
    }
}
