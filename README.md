# Neural Galaga

A Galaga clone built from scratch in Rust with `wgpu`, designed for both human play and reinforcement-learning experiments.

The game renders at the original Galaga resolution of 224×288 and upscales to 1120×1440 with nearest-neighbour filtering for a crisp pixel-art look. It features infinite waves of increasing difficulty, four enemy classes with distinct behaviours, a shield system for both player and enemies, and collectible powerups.

The project doubles as a self-contained RL platform: a deterministic game simulator, a PPO training loop, and a live-inference viewer — all built on the Burn deep-learning framework running on the Wgpu GPU backend, so the same machine can render, train, and play back.

## Workspace Layout

```
crates/
├── neural-galaga-core   # headless game simulator + shared types
├── neural-galaga-ai     # PPO trainer, actor-critic model, structured observations
└── neural-galaga-ui     # wgpu renderer, audio, `play` and `infer` binaries
```

Three binaries:

| Binary | Purpose |
|---|---|
| `play`  | Play the game as a human |
| `train` | Train a PPO agent against the headless simulator |
| `infer` | Watch a trained checkpoint play the game in real time |

Requires Rust 1.85+ (edition 2024).

## Playing

```bash
cargo run --release -p neural-galaga-ui --bin play
```

| Key | Action |
|---|---|
| ← / A | Move left |
| → / D | Move right |
| Space | Fire |
| Enter | Confirm menu selection |
| ↑ ↓ / W S | Navigate menus |
| Escape | Pause (in-game) / Back (menus) |

## Training

```bash
cargo run --release -p neural-galaga-ai --bin train
# resume from the best checkpoint
cargo run --release -p neural-galaga-ai --bin train -- --resume
# or a specific path
cargo run --release -p neural-galaga-ai --bin train -- --resume checkpoints/cheats/update_001000
```

Logs are written to `checkpoints/cheats/training_log.csv`, plot with:

```bash
python scripts/plot_training.py checkpoints/cheats/training_log.csv
```

### Algorithm

Vanilla PPO with GAE advantages. The headless simulator is driven in parallel by a large pool of environments; every rollout is batched into a single GPU forward pass for the policy, then the collected transitions are shuffled into minibatches for several optimisation epochs.

| Hyperparameter | Value |
|---|---|
| Parallel envs      | 2048 |
| Rollout length     | 128 steps |
| Transitions/update | 262 144 |
| Minibatch size     | 4096 |
| PPO epochs         | 4 |
| γ (discount)       | 0.99 |
| λ (GAE)            | 0.95 |
| Clip ε             | 0.1 |
| Value coefficient  | 0.5 |
| Entropy bonus      | 0.01 |
| Learning rate      | 3e-4 → 1e-5 (cosine over 2000 updates, then floor) |
| Grad clip          | 0.5 (global norm) |
| Optimiser          | Adam |
| Frame skip         | 2 sim ticks per decision |

### Observations

Fully-structured (no pixels). Each frame is encoded as 815 floats; the two most recent frames are concatenated, giving the network an implicit velocity signal without needing a recurrent state.

| Block | Slots × floats | Floats | Contents |
|---|---:|---:|---|
| Player        | 1 × 17  | 17  | position, speed, projectile speed, shield, lives, wave progress, vulnerable flag, upgrade stacks, bullet count, inventory, currency |
| Enemies       | 36 × 11 | 396 | valid, abs/rel position, class one-hot, shield level, diving flag |
| Player bullets  | 8 × 5  | 40  | valid, abs/rel position |
| Enemy bullets   | 32 × 7 | 224 | valid, abs position, vertical speed, rel position, time-to-impact; **sorted by distance to player** |
| Shotgun balls   | 12 × 8 | 96  | valid, abs position, velocity, rel position, time-to-impact; **sorted by distance to player** |
| Powerups        | 4 × 7  | 28  | valid, position, kind one-hot |
| Danger columns  | 14     | 14  | min time-to-impact of any downward projectile in each vertical slice of the screen |
| **Total**       |        | **815** per frame, **1630** stacked |

All positions normalised to roughly [-1, 1]; velocities normalised by their respective max speeds. Sorting projectiles by distance means the closest threats always occupy the same slots, so the network doesn't have to learn attention over a shuffled list.

### Action Space

18 discrete actions:

```
0: noop              3: fire           6:  BuyRate    12: SellRate
1: left              4: left + fire    7:  BuySpeed   13: SellSpeed
2: right             5: right + fire   8:  BuyDouble  14: SellDouble
                                       9:  BuyTriple  15: SellTriple
                                       10: BuyShield  16: SellShield
                                       11: BuyLife    17: SellLife
```

The buy/sell actions run through an in-game currency economy. The agent is free to stockpile upgrades into its inventory and re-deploy them later — useful both for evolving strategy and for handing the network a richer action space.

### Reward Shaping

| Event | Reward |
|---|---:|
| Enemy killed            | +1.0 |
| Powerup collected       | +5.0 |
| Enemy shield hit        | +0.5 |
| Player hit              | −3.0 |
| Player shield absorbed a hit | −shield_penalty (see below) |
| Proximity to alive enemies   | soft −exp(−d/20) sum, scaled 0.01 |
| Surviving near projectiles   | +0.05 × (downward projectiles within 40 px) |

**Shield-penalty curriculum.** The shield-absorption penalty starts at 0 and is gated behind a performance threshold: once the 100-episode average reward crosses 120, the penalty begins linearly ramping to a maximum of 5.0 over the next ~20 B environment steps. Early in training, losing the shield is free (the agent is still learning to survive); later, the shield becomes a scarce resource the agent should protect.

### Model

A compact feedforward actor-critic — no LSTM, no attention. Frame-stacking alone carries the temporal signal:

```
obs (1630) → Linear 768 → ReLU
           → Linear 768 → ReLU
           → Linear 384 → ReLU
           → Linear 256 → ReLU (trunk)
           ├─ Linear 18 → log-softmax  (policy)
           └─ Linear 1                 (value)
```

Training uses `burn::backend::Autodiff<Wgpu>`; rollout inference calls `model.valid()` to get the forward-only inner backend and skip autograd bookkeeping.

### Checkpoints

* `checkpoints/cheats/best` — updated whenever the 100-episode moving average beats the previous best.
* `checkpoints/cheats/update_NNNNNN` — periodic snapshot every 10 updates.
* `checkpoints/cheats/training_log.csv` — per-update `(update, avg_reward)` rows, truncated on each fresh run.

## Inference

Watch a trained agent play the real game, with rendering, audio, and optional saliency overlay:

```bash
cargo run --release -p neural-galaga-ui --bin infer
# point at a specific checkpoint (defaults to checkpoints/cheats/best)
cargo run --release -p neural-galaga-ui --bin infer -- --model checkpoints/cheats/update_000500
```

The inference binary loads the checkpoint onto the Wgpu inner backend (`model.valid()`), ticks the same `GameSim` used during training, and queries the policy once every `FRAME_SKIP` ticks — matching the exact decision cadence of the trained agent. Between decisions the last action set continues to fire, so the visuals run at the full game frame rate while the policy runs at half.

The HUD in the top-left shows the agent's upgrade stacks and stored inventory (`RAT`, `SPD`, `DBL`, `TRP`, `SHL`) plus its accumulated currency (`$`). With saliency enabled, coloured rings are drawn around entities proportional to how much their observation features contribute to the policy's current action — handy for seeing *why* the agent dodges, stalls, or commits.
