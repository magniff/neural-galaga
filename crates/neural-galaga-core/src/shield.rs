/// Shield charge level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShieldLevel {
    Full,
    Damaged,
    /// Blinks every 10 game steps before disappearing on next hit.
    Critical,
}

/// Optional shield on a player or enemy.
#[derive(Clone, Debug)]
pub struct Shield {
    pub level: ShieldLevel,
    /// Phase offset for pulsation, so shields aren't in sync.
    pub phase: u64,
}

impl Shield {
    pub fn new(phase: u64) -> Self {
        Self {
            level: ShieldLevel::Full,
            phase,
        }
    }

    /// Take a hit. Returns true if the shield absorbed it (still alive),
    /// false if the shield just broke (caller should remove it).
    pub fn hit(&mut self) -> bool {
        match self.level {
            ShieldLevel::Full => {
                self.level = ShieldLevel::Damaged;
                true
            }
            ShieldLevel::Damaged => {
                self.level = ShieldLevel::Critical;
                true
            }
            ShieldLevel::Critical => false,
        }
    }

    /// RGBA color for rendering based on level and frame counter (for blinking).
    pub fn color(&self, frame_counter: u64) -> [f32; 4] {
        match self.level {
            ShieldLevel::Full => [1.0, 0.98, 1.0, 1.0],
            ShieldLevel::Damaged => [0.45, 0.45, 0.45, 1.0],
            ShieldLevel::Critical => {
                if ((frame_counter + self.phase) / 10) % 2 == 0 {
                    [0.2, 0.2, 0.2, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                }
            }
        }
    }

    /// Pulsating radius based on frame counter and per-shield phase offset.
    pub fn radius(&self, frame_counter: u64) -> f32 {
        const RADII: [f32; 5] = [9.0, 10.0, 11.0, 10.0, 9.0];
        let idx = (((frame_counter + self.phase) / 6) % 5) as usize;
        RADII[idx]
    }
}
