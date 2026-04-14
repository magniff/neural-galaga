use std::collections::HashMap;

use crate::{
    Action, BattleStarfield, FIXED_DT, Framebuffer, GAME_HEIGHT, GAME_WIDTH, GameSim, GameStatus,
    StepResult,
};

/// Info about a saved fork/snapshot.
#[derive(Debug, Clone)]
pub struct ForkInfo {
    pub fork_id: u32,
    pub step: u64,
}

/// A complete game session with starfield compositing, fork support, and composed framebuffer.
/// Replaces the old server + protocol architecture — everything runs in-process.
#[derive(Clone)]
pub struct GameSession {
    sim: GameSim,
    starfield: BattleStarfield,
    fb: Framebuffer,
    step_count: u64,
    snapshots: HashMap<u32, SessionSnapshot>,
    next_fork_id: u32,
    last_result: Option<StepResult>,
}

#[derive(Clone)]
struct SessionSnapshot {
    sim: GameSim,
    starfield: BattleStarfield,
    fb: Framebuffer,
    step_count: u64,
}

impl GameSession {
    pub fn new() -> Self {
        Self::with_start_wave(1)
    }

    /// Start a new game session at a specific wave.
    pub fn with_start_wave(wave: u32) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        Self {
            sim: GameSim::with_seed_and_wave(seed, wave),
            starfield: BattleStarfield::new(),
            fb: Framebuffer::new(GAME_WIDTH as u32, GAME_HEIGHT as u32),
            step_count: 0,
            snapshots: HashMap::new(),
            next_fork_id: 1,
            last_result: None,
        }
    }

    /// Advance the game by one tick with the given actions.
    /// Returns a reference to the step result.
    pub fn step(&mut self, actions: &[Action]) -> &StepResult {
        let result = self.sim.step(actions);
        self.step_count += 1;

        // Compose full frame: starfield background + game layer
        self.fb.clear(3, 3, 8, 255);
        self.starfield.update(FIXED_DT);
        self.starfield.draw(&mut self.fb);

        let game_fb = self.sim.framebuffer();
        for (dst, src) in self
            .fb
            .pixels
            .chunks_exact_mut(4)
            .zip(game_fb.chunks_exact(4))
        {
            if src[3] == 0 {
                continue;
            }
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = 255;
        }

        self.last_result = Some(result);
        self.last_result.as_ref().unwrap()
    }

    /// Get the fully composed framebuffer (starfield + game).
    pub fn framebuffer(&self) -> &[u8] {
        &self.fb.pixels
    }

    /// Get the last step result.
    pub fn last_result(&self) -> Option<&StepResult> {
        self.last_result.as_ref()
    }

    /// Get the last score.
    pub fn last_score(&self) -> i32 {
        self.last_result.as_ref().map_or(0, |r| r.score)
    }

    /// Is the game over?
    pub fn is_done(&self) -> bool {
        self.last_result
            .as_ref()
            .is_some_and(|r| r.status == GameStatus::Lost)
    }

    /// Snapshot the current state. Returns the fork ID.
    pub fn fork(&mut self) -> u32 {
        let id = self.next_fork_id;
        self.next_fork_id += 1;
        self.snapshots.insert(
            id,
            SessionSnapshot {
                sim: self.sim.clone(),
                starfield: self.starfield.clone(),
                fb: self.fb.clone(),
                step_count: self.step_count,
            },
        );
        id
    }

    /// Restore from a snapshot. The snapshot is preserved (can be restored again).
    pub fn restore(&mut self, fork_id: u32) -> Result<(), String> {
        let snapshot = self
            .snapshots
            .get(&fork_id)
            .ok_or_else(|| format!("unknown fork {fork_id}"))?;
        self.sim = snapshot.sim.clone();
        self.starfield = snapshot.starfield.clone();
        self.fb = snapshot.fb.clone();
        self.step_count = snapshot.step_count;
        // Step once to refresh the frame
        self.step(&[]);
        Ok(())
    }

    /// List all forks.
    pub fn list_forks(&self) -> Vec<ForkInfo> {
        let mut forks: Vec<ForkInfo> = self
            .snapshots
            .iter()
            .map(|(&id, snap)| ForkInfo {
                fork_id: id,
                step: snap.step_count,
            })
            .collect();
        forks.sort_by_key(|f| f.fork_id);
        forks
    }

    /// Delete a fork.
    pub fn kill_fork(&mut self, fork_id: u32) -> Result<(), String> {
        self.snapshots
            .remove(&fork_id)
            .map(|_| ())
            .ok_or_else(|| format!("unknown fork {fork_id}"))
    }
}
