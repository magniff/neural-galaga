#![recursion_limit = "256"]

//! AI inference client for the cheats agent — full GUI like the human client,
//! but actions are produced by a model that consumes the structured game state
//! (positions, velocities, shields, etc.) instead of pixels.
//!
//! Press V to toggle saliency overlay: circles around the entities that most
//! influenced the agent's action choice.
//!
//! Usage:
//!   cargo run -p neural-galaga-ui --bin infer_cheats --release -- --model checkpoints/cheats/best

use std::sync::Arc;
use std::time::Instant;

use burn::backend::{Autodiff, Wgpu};
use burn::module::AutodiffModule;
use burn::prelude::*;
use burn::record::CompactRecorder;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use neural_galaga_ai::env::{FRAME_SKIP, STACKED_OBS_SIZE};
use neural_galaga_ai::model::{CheatsActorCritic, CheatsActorCriticConfig};
use neural_galaga_ai::obs::{
    ENEMIES_OFFSET, ENEMY_BULLET_SLOT_FLOATS, ENEMY_BULLETS_OFFSET, ENEMY_SLOT_FLOATS,
    NUM_ENEMY_BULLET_SLOTS, NUM_ENEMY_SLOTS, NUM_PLAYER_BULLET_SLOTS, NUM_POWERUP_SLOTS,
    NUM_SHOTGUN_SLOTS, OBS_SIZE, PLAYER_BULLET_SLOT_FLOATS, PLAYER_BULLETS_OFFSET,
    POWERUP_SLOT_FLOATS, POWERUPS_OFFSET, SHOTGUN_OFFSET, SHOTGUN_SLOT_FLOATS, encode,
};
use neural_galaga_core::session::GameSession;
use neural_galaga_core::*;
use neural_galaga_ui::audio::{self, Audio, MusicTrack};
use neural_galaga_ui::game::*;
use neural_galaga_ui::input::{self, *};
use neural_galaga_ui::render::GpuState;

type B = Autodiff<Wgpu>;
type IB = Wgpu;
const NUM_ACTIONS: usize = neural_galaga_ai::NUM_ACTIONS;

const REFERENCE_HEIGHT: u32 = 2160;
const REFERENCE_UPSCALE: u32 = 5;
const FONT_TTF: &[u8] = include_bytes!("../../../../assets/fonts/PressStart2P-Regular.ttf");

fn action_index_to_list(action: usize) -> Vec<Action> {
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

/// An entity that the saliency overlay wants to highlight.
struct SaliencyTarget {
    /// Screen x position (game pixels).
    x: f32,
    /// Screen y position (game pixels).
    y: f32,
    /// Importance score (absolute gradient magnitude, normalized to [0, 1]).
    importance: f32,
    /// Color: (R, G, B).
    color: (u8, u8, u8),
}

/// Draw a circle outline on the RGBA framebuffer.
fn draw_circle(fb: &mut [u8], cx: f32, cy: f32, radius: f32, color: (u8, u8, u8), alpha: f32) {
    let fb_w = GAME_WIDTH as usize;
    let fb_h = GAME_HEIGHT as usize;
    let steps = (radius * 6.28).max(16.0) as usize;
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let px = (cx + radius * angle.cos()).round() as i32;
        let py = (cy + radius * angle.sin()).round() as i32;
        if px >= 0 && py >= 0 && (px as usize) < fb_w && (py as usize) < fb_h {
            let idx = (py as usize * fb_w + px as usize) * 4;
            let one_minus_a = 1.0 - alpha;
            fb[idx] = (fb[idx] as f32 * one_minus_a + color.0 as f32 * alpha) as u8;
            fb[idx + 1] = (fb[idx + 1] as f32 * one_minus_a + color.1 as f32 * alpha) as u8;
            fb[idx + 2] = (fb[idx + 2] as f32 * one_minus_a + color.2 as f32 * alpha) as u8;
        }
    }
}

struct InferState {
    model: CheatsActorCritic<B>,
    device: <IB as Backend>::Device,
    model_path: String,
    /// Previous frame's encoded observation (for 2-frame stacking).
    prev_obs: Vec<f32>,
    /// Most recent encoded observation (current frame, single-frame).
    last_obs: Vec<f32>,
    /// Most recent step result, cached for saliency overlay (need entity positions).
    last_result: Option<StepResult>,
    /// Most recent chosen action index.
    last_action: usize,
    /// Per-entity saliency targets computed after the last action.
    saliency_targets: Vec<SaliencyTarget>,
}

impl InferState {
    fn new(model_path: &str) -> Self {
        let device = <IB as Backend>::Device::default();
        log::info!("loading cheats model from {model_path}");
        let model: CheatsActorCritic<B> = CheatsActorCriticConfig
            .init::<B>(&device)
            .load_file(model_path, &CompactRecorder::new(), &device)
            .expect("failed to load model");
        log::info!("cheats model loaded");

        Self {
            model_path: model_path.to_string(),
            model,
            device,
            prev_obs: vec![0.0; OBS_SIZE],
            last_obs: vec![0.0; OBS_SIZE],
            last_result: None,
            last_action: 0,
            saliency_targets: Vec::new(),
        }
    }

    fn reload_model(&mut self) {
        log::info!("reloading model from {}", self.model_path);
        match CheatsActorCriticConfig.init::<B>(&self.device).load_file(
            &self.model_path,
            &CompactRecorder::new(),
            &self.device,
        ) {
            Ok(model) => {
                self.model = model;
                log::info!("model reloaded successfully");
            }
            Err(e) => {
                log::warn!("failed to reload model: {e}, keeping current");
            }
        }
    }

    fn update_obs(&mut self, result: &StepResult) {
        self.prev_obs = std::mem::replace(&mut self.last_obs, encode(result));
        self.last_result = Some(result.clone());
    }

    fn stacked_obs(&self) -> Vec<f32> {
        let mut stacked = Vec::with_capacity(STACKED_OBS_SIZE);
        stacked.extend_from_slice(&self.prev_obs);
        stacked.extend_from_slice(&self.last_obs);
        stacked
    }

    fn pick_action(&mut self) -> Vec<Action> {
        let stacked = self.stacked_obs();
        // Use the inner (non-autodiff) backend for fast inference — no graph overhead.
        let inference_model = self.model.valid();
        let obs_tensor = Tensor::<IB, 1>::from_floats(stacked.as_slice(), &self.device)
            .reshape([1usize, STACKED_OBS_SIZE]);
        let output = inference_model.forward(obs_tensor);
        let probs: Vec<f32> = output.log_probs.exp().to_data().to_vec().unwrap();

        let r: f32 = rand::random();
        let mut cumsum = 0.0;
        let mut action = probs.len() - 1;
        for (i, &p) in probs.iter().enumerate() {
            cumsum += p;
            if r < cumsum {
                action = i;
                break;
            }
        }

        self.last_action = action;
        action_index_to_list(action)
    }

    /// Compute per-entity saliency for the most recently chosen action.
    /// Runs a second forward pass with `require_grad` on the obs tensor, backprops the
    /// chosen action's log-probability, and aggregates gradient magnitudes per entity slot.
    fn compute_saliency(&mut self) {
        self.saliency_targets.clear();
        let Some(result) = &self.last_result else {
            return;
        };

        // Build stacked obs tensor that tracks gradients.
        let stacked = self.stacked_obs();
        let obs = Tensor::<B, 1>::from_floats(stacked.as_slice(), &self.device)
            .reshape([1usize, STACKED_OBS_SIZE])
            .require_grad();

        let output = self.model.forward(obs.clone());

        // Extract the chosen action's log-probability as a scalar and backprop.
        let action = self.last_action;
        let mut mask_data = vec![0.0f32; NUM_ACTIONS];
        mask_data[action] = 1.0;
        let mask = Tensor::<B, 1>::from_floats(mask_data.as_slice(), &self.device)
            .reshape([1, NUM_ACTIONS]);
        let chosen_lp = (output.log_probs * mask).sum();
        let grads = chosen_lp.backward();

        let full_grad: Vec<f32> = obs
            .grad(&grads)
            .map(|g| g.to_data().to_vec().unwrap())
            .unwrap_or_else(|| vec![0.0; STACKED_OBS_SIZE]);
        // Use gradients from the current frame (second half of stacked obs).
        let obs_grad = &full_grad[OBS_SIZE..];

        // Helper: sum of absolute gradient values over a contiguous slice.
        let slot_importance = |offset: usize, slot_floats: usize| -> f32 {
            obs_grad[offset..offset + slot_floats]
                .iter()
                .map(|v| v.abs())
                .sum()
        };

        // Collect per-entity importance with screen positions.
        // Enemies: 36 slots.
        for i in 0..NUM_ENEMY_SLOTS {
            if let Some(e) = result.enemies.get(i) {
                if !e.alive {
                    continue;
                }
                let offset = ENEMIES_OFFSET + i * ENEMY_SLOT_FLOATS;
                let imp = slot_importance(offset, ENEMY_SLOT_FLOATS);
                self.saliency_targets.push(SaliencyTarget {
                    x: e.x + 8.0,
                    y: e.y + 8.0,
                    importance: imp,
                    color: (255, 80, 80), // red
                });
            }
        }

        // Player bullets.
        let mut pb_idx = 0;
        for b in &result.bullets {
            if b.dy < 0.0 && pb_idx < NUM_PLAYER_BULLET_SLOTS {
                let offset = PLAYER_BULLETS_OFFSET + pb_idx * PLAYER_BULLET_SLOT_FLOATS;
                let imp = slot_importance(offset, PLAYER_BULLET_SLOT_FLOATS);
                self.saliency_targets.push(SaliencyTarget {
                    x: b.x + 2.0,
                    y: b.y + 4.0,
                    importance: imp,
                    color: (60, 220, 255), // cyan
                });
                pb_idx += 1;
            }
        }

        // Enemy bullets.
        let mut eb_idx = 0;
        for b in &result.bullets {
            if b.dy >= 0.0 && eb_idx < NUM_ENEMY_BULLET_SLOTS {
                let offset = ENEMY_BULLETS_OFFSET + eb_idx * ENEMY_BULLET_SLOT_FLOATS;
                let imp = slot_importance(offset, ENEMY_BULLET_SLOT_FLOATS);
                self.saliency_targets.push(SaliencyTarget {
                    x: b.x + 2.0,
                    y: b.y + 4.0,
                    importance: imp,
                    color: (255, 220, 60), // yellow
                });
                eb_idx += 1;
            }
        }

        // Shotgun balls.
        let mut sb_idx = 0;
        for s in &result.shotgun_balls {
            if sb_idx >= NUM_SHOTGUN_SLOTS {
                break;
            }
            let offset = SHOTGUN_OFFSET + sb_idx * SHOTGUN_SLOT_FLOATS;
            let imp = slot_importance(offset, SHOTGUN_SLOT_FLOATS);
            self.saliency_targets.push(SaliencyTarget {
                x: s.x + 2.0,
                y: s.y + 2.0,
                importance: imp,
                color: (255, 160, 60), // orange
            });
            sb_idx += 1;
        }

        // Powerups.
        let mut pu_idx = 0;
        for p in &result.powerups {
            if pu_idx >= NUM_POWERUP_SLOTS {
                break;
            }
            let offset = POWERUPS_OFFSET + pu_idx * POWERUP_SLOT_FLOATS;
            let imp = slot_importance(offset, POWERUP_SLOT_FLOATS);
            self.saliency_targets.push(SaliencyTarget {
                x: p.x + 8.0,
                y: p.y + 8.0,
                importance: imp,
                color: (60, 255, 80), // green
            });
            pu_idx += 1;
        }

        // Normalize importance to [0, 1].
        let max_imp = self
            .saliency_targets
            .iter()
            .map(|t| t.importance)
            .fold(0.0f32, f32::max)
            .max(1e-6);
        for t in &mut self.saliency_targets {
            t.importance /= max_imp;
        }
    }
}

struct AppState {
    gpu: GpuState,
    audio: Audio,
    font: BitmapFont,
    starfield: Starfield,
    phase: GamePhase,
    input: InputState,
    fb: Framebuffer,
    last_frame: Instant,
    tick_accum: f32,
    window: Arc<Window>,
    infer: InferState,
    current_actions: Vec<Action>,
    /// Counts sim ticks within the current frame-skip group.
    frameskip_counter: usize,
    paused: bool,
    step_once: bool,
    /// Toggle for saliency overlay (V).
    show_saliency: bool,
}

struct App {
    wgpu_instance: wgpu::Instance,
    audio_scan: Option<std::thread::JoinHandle<Option<String>>>,
    model_path: String,
    state: Option<AppState>,
}

impl App {
    fn new(event_loop: &EventLoop<()>, model_path: String) -> Self {
        let audio_scan = audio::scan_preferred_device();
        let wgpu_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: Some(Box::new(event_loop.owned_display_handle())),
        });
        Self {
            wgpu_instance,
            audio_scan: Some(audio_scan),
            model_path,
            state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let upscale = if let Some(monitor) = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
        {
            let screen_h = monitor.size().height;
            let scale = screen_h as f64 / REFERENCE_HEIGHT as f64;
            (REFERENCE_UPSCALE as f64 * scale).round().max(1.0) as u32
        } else {
            REFERENCE_UPSCALE
        };
        let window_w = GAME_WIDTH as u32 * upscale;
        let window_h = GAME_HEIGHT as u32 * upscale;

        let window_attrs = Window::default_attributes()
            .with_title("Neural Galaga — Cheats AI")
            .with_inner_size(winit::dpi::PhysicalSize::new(window_w, window_h))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let gpu = GpuState::new(window.clone(), &self.wgpu_instance);

        let preferred = self.audio_scan.take().and_then(|h| h.join().ok()).flatten();
        let mut audio_sys = Audio::new(preferred);
        if !audio_sys.is_muted() {
            audio_sys.toggle_mute();
        }
        audio_sys.play_music(MusicTrack::Game);

        let infer = InferState::new(&self.model_path);
        let phase = GamePhase::Playing(GameSession::with_start_wave(1));

        self.state = Some(AppState {
            gpu,
            audio: audio_sys,
            font: BitmapFont::load_from_memory(FONT_TTF),
            starfield: Starfield::new(),
            phase,
            input: InputState::new(),
            fb: Framebuffer::new(GAME_WIDTH as u32, GAME_HEIGHT as u32),
            last_frame: Instant::now(),
            tick_accum: 0.0,
            window,
            infer,
            current_actions: vec![],
            frameskip_counter: 0,
            paused: false,
            step_once: false,
            show_saliency: false,
        });
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = &mut self.state else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => s.gpu.resize(size.width, size.height),

            WindowEvent::KeyboardInput { event, .. } => {
                let key = event.logical_key.clone();
                match event.state {
                    ElementState::Pressed => {
                        s.input.key_down(key.clone());
                        if s.input.just_pressed(input::key_char("m")) {
                            s.audio.toggle_mute();
                        }
                        if s.input.just_pressed(input::key_space()) {
                            s.paused = !s.paused;
                            log::info!("paused: {}", s.paused);
                        }
                        if s.paused && s.input.just_pressed(input::key_char("s")) {
                            s.step_once = true;
                        }
                        if s.input.just_pressed(input::key_char("v")) {
                            s.show_saliency = !s.show_saliency;
                            log::info!(
                                "saliency overlay: {}",
                                if s.show_saliency { "ON" } else { "OFF" }
                            );
                        }
                        for wave in 1u32..=9 {
                            let key_str = wave.to_string();
                            if s.input.just_pressed(input::key_char(&key_str)) {
                                log::info!("jumping to wave {wave}");
                                s.current_actions = vec![];
                                s.phase = GamePhase::Playing(GameSession::with_start_wave(wave));
                                break;
                            }
                        }
                    }
                    ElementState::Released => s.input.key_up(key),
                }
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - s.last_frame).as_secs_f32().min(0.05);
                s.last_frame = now;

                const SIM_DT: f32 = 1.0 / 60.0;
                let advance_sim = !s.paused && matches!(s.phase, GamePhase::Playing(_));
                if advance_sim {
                    s.tick_accum += dt;
                } else {
                    s.tick_accum = 0.0;
                }

                let needs_local_bg = matches!(
                    s.phase,
                    GamePhase::Menu { .. } | GamePhase::Credits | GamePhase::ServerError
                );
                if needs_local_bg {
                    s.fb.clear(3, 3, 8, 255);
                    s.starfield.update(dt);
                    s.starfield.draw(&mut s.fb);
                }

                match &mut s.phase {
                    GamePhase::Playing(session) => {
                        loop {
                            let normal_tick = s.tick_accum >= SIM_DT;
                            let manual_tick = s.step_once;
                            if !normal_tick && !manual_tick {
                                break;
                            }
                            if normal_tick {
                                s.tick_accum -= SIM_DT;
                            }
                            s.step_once = false;

                            // Only query the model every FRAME_SKIP sim ticks,
                            // matching the training cadence.
                            if s.frameskip_counter == 0 {
                                s.current_actions = s.infer.pick_action();
                            }
                            s.frameskip_counter = (s.frameskip_counter + 1) % FRAME_SKIP;

                            let result = session.step(&s.current_actions).clone();
                            s.infer.update_obs(&result);

                            // Compute saliency only on decision frames.
                            if s.show_saliency && s.frameskip_counter == 0 {
                                s.infer.compute_saliency();
                            }

                            if result.sound_events.player_fired {
                                s.audio.player_fire();
                            }
                            if result.sound_events.enemy_fired {
                                s.audio.enemy_fire();
                            }
                            if result.sound_events.player_hit {
                                s.audio.player_hit();
                            }
                            if result.sound_events.enemy_hit {
                                s.audio.enemy_hit();
                            }
                            if result.sound_events.shield_hit {
                                s.audio.shield_hit();
                            }
                            if result.sound_events.powerup_picked {
                                s.audio.powerup_pickup();
                            }

                            if result.status == GameStatus::WaveComplete {
                                let wave = result.wave;
                                let GamePhase::Playing(session) = std::mem::replace(
                                    &mut s.phase,
                                    GamePhase::Menu { selected: 0 },
                                ) else {
                                    unreachable!()
                                };
                                s.phase = GamePhase::WaveClear {
                                    session,
                                    timer: 2.0,
                                    wave,
                                };
                                break;
                            }
                        }

                        if let GamePhase::Playing(session) = &s.phase {
                            s.fb.pixels.copy_from_slice(session.framebuffer());

                            if session.is_done() {
                                let score = session.last_score();
                                let GamePhase::Playing(session) = std::mem::replace(
                                    &mut s.phase,
                                    GamePhase::Menu { selected: 0 },
                                ) else {
                                    unreachable!()
                                };
                                s.phase = GamePhase::GameOver {
                                    session,
                                    timer: 2.0,
                                    score,
                                };
                            }
                        }
                    }
                    GamePhase::WaveClear {
                        session,
                        timer,
                        wave,
                    } => {
                        *timer -= dt;
                        s.fb.pixels.copy_from_slice(session.framebuffer());
                        render_wave_clear(&mut s.fb, &mut s.font, *wave);
                        if *timer <= 0.0 {
                            let GamePhase::WaveClear { session, .. } =
                                std::mem::replace(&mut s.phase, GamePhase::Menu { selected: 0 })
                            else {
                                unreachable!()
                            };
                            s.phase = GamePhase::Playing(session);
                        }
                    }
                    GamePhase::GameOver { timer, score, .. } => {
                        *timer -= dt;
                        render_game_over(&mut s.fb, &mut s.font, *score);
                        if *timer <= 0.0 {
                            s.infer.reload_model();
                            s.audio.play_music(MusicTrack::Game);
                            s.current_actions = vec![];
                            s.phase = GamePhase::Playing(GameSession::with_start_wave(1));
                        }
                    }
                    GamePhase::ServerError => {
                        render_server_error(&mut s.fb, &mut s.font);
                        if s.input.just_pressed(key_escape()) {
                            event_loop.exit();
                        }
                    }
                    _ => {}
                }

                if s.audio.is_muted() {
                    s.font
                        .draw_text(&mut s.fb, "MUTE", 190.0, 2.0, 1.0, [1.0, 0.3, 0.3, 0.6]);
                }

                s.font
                    .draw_text(&mut s.fb, "CHEATS", 2.0, 2.0, 1.0, [1.0, 0.6, 0.3, 0.8]);

                // Saliency overlay: draw circles around entities proportional to importance.
                if s.show_saliency {
                    for t in &s.infer.saliency_targets {
                        if t.importance < 0.05 {
                            continue;
                        }
                        // Circle radius scales with importance: 4px at 0.05 → 14px at 1.0.
                        let radius = 4.0 + t.importance * 10.0;
                        let alpha = (t.importance * 0.9).clamp(0.1, 0.9);
                        // Draw two concentric circles for visibility.
                        draw_circle(&mut s.fb.pixels, t.x, t.y, radius, t.color, alpha);
                        draw_circle(
                            &mut s.fb.pixels,
                            t.x,
                            t.y,
                            radius + 1.0,
                            t.color,
                            alpha * 0.5,
                        );
                    }
                    s.font
                        .draw_text(&mut s.fb, "SAL", 60.0, 2.0, 1.0, [0.3, 1.0, 1.0, 0.8]);
                }

                // Inventory / upgrades HUD (bottom-left)
                // Format: "LABEL active+inventory"
                if let Some(ref result) = s.infer.last_result {
                    let y_base = GAME_HEIGHT - 28.0;
                    let color = [0.8, 0.8, 0.8, 0.7];
                    let inv = &result.inventory;
                    let lines: [(&str, u8, u8); 5] = [
                        ("RAT", result.fire_rate_stacks, inv[0]),
                        ("SPD", result.speed_stacks, inv[1]),
                        ("DBL", if result.bullet_count >= 2 { 1 } else { 0 }, inv[2]),
                        ("TRP", if result.bullet_count >= 3 { 1 } else { 0 }, inv[3]),
                        ("SHL", result.shield_level.min(1), inv[4]),
                    ];
                    for (i, (label, active, stock)) in lines.iter().enumerate() {
                        let y = y_base - i as f32 * 10.0;
                        let text = format!("{} {}+{}", label, active, stock);
                        s.font.draw_text(&mut s.fb, &text, 2.0, y, 1.0, color);
                    }
                    // Currency display
                    let cur_y = y_base - 5.0 * 10.0;
                    let cur_text = format!("$ {}", result.currency);
                    s.font
                        .draw_text(&mut s.fb, &cur_text, 2.0, cur_y, 1.0, [1.0, 1.0, 0.3, 0.8]);
                }

                s.gpu.render_framebuffer(&s.fb.pixels);
                s.input.end_frame();
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let model_path = args
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "checkpoints/cheats/best".to_string());

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(&event_loop, model_path);
    event_loop.run_app(&mut app).unwrap();
}
