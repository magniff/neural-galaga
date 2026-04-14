use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use neural_galaga_core::session::GameSession;
use neural_galaga_core::*;
use neural_galaga_ui::audio::{Audio, MusicTrack};
use neural_galaga_ui::game::*;
use neural_galaga_ui::input::*;
use neural_galaga_ui::render::GpuState;

/// On a 4K display (2160px tall) we use 5x upscale (window 1440px tall).
/// For other resolutions we scale proportionally, keeping integer multiples.
const REFERENCE_HEIGHT: u32 = 2160;
const REFERENCE_UPSCALE: u32 = 5;

struct AppState {
    gpu: GpuState,
    audio: Audio,
    sheet: SpriteSheet,
    font: BitmapFont,
    starfield: Starfield,
    phase: GamePhase,
    input: InputState,
    fb: Framebuffer,
    last_frame: Instant,
    tick_accum: f32,
    window: Arc<Window>,
}

struct App {
    wgpu_instance: wgpu::Instance,
    audio_scan: Option<std::thread::JoinHandle<Option<String>>>,
    state: Option<AppState>,
}

impl App {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let audio_scan = neural_galaga_ui::audio::scan_preferred_device();
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
            state: None,
        }
    }
}

const SPRITE_PNG: &[u8] = include_bytes!("../../../assets/sprites.png");
const FONT_TTF: &[u8] = include_bytes!("../../../assets/fonts/PressStart2P-Regular.ttf");

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // Compute upscale factor proportional to monitor height
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
            .with_title("Neural Galaga")
            .with_inner_size(winit::dpi::PhysicalSize::new(window_w, window_h))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let gpu = GpuState::new(window.clone(), &self.wgpu_instance);

        let preferred = self.audio_scan.take().and_then(|h| h.join().ok()).flatten();
        let mut audio = Audio::new(preferred);
        audio.play_music(MusicTrack::Menu);

        self.state = Some(AppState {
            gpu,
            audio,
            sheet: SpriteSheet::load_from_memory(SPRITE_PNG),
            font: BitmapFont::load_from_memory(FONT_TTF),
            starfield: Starfield::new(),
            phase: GamePhase::Menu { selected: 0 },
            input: InputState::new(),
            fb: Framebuffer::new(GAME_WIDTH as u32, GAME_HEIGHT as u32),
            last_frame: Instant::now(),
            tick_accum: 0.0,
            window,
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
                    ElementState::Pressed => s.input.key_down(key),
                    ElementState::Released => s.input.key_up(key),
                }
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - s.last_frame).as_secs_f32().min(0.05);
                s.last_frame = now;

                // Accumulate time — only tick the sim at 60Hz
                const SIM_DT: f32 = 1.0 / 60.0;
                s.tick_accum += dt;

                // Starfield background for client-rendered screens
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
                    GamePhase::Menu { selected } => {
                        if s.input.just_pressed(key_up()) || s.input.just_pressed(key_char("w")) {
                            if *selected > 0 {
                                *selected -= 1;
                            }
                        }
                        if s.input.just_pressed(key_down()) || s.input.just_pressed(key_char("s")) {
                            if *selected < 1 {
                                *selected += 1;
                            }
                        }
                        let sel = *selected;
                        if s.input.just_pressed(key_enter()) || s.input.just_pressed(key_space()) {
                            match sel {
                                0 => {
                                    s.phase = GamePhase::Playing(GameSession::new());
                                    s.audio.play_music(MusicTrack::Game);
                                    s.input.clear();
                                }
                                1 => s.phase = GamePhase::Credits,
                                _ => {}
                            }
                        }
                        match &s.phase {
                            GamePhase::Menu { selected } => {
                                render_menu(&mut s.fb, &s.sheet, &mut s.font, *selected);
                            }
                            _ => {}
                        }
                        s.tick_accum = 0.0; // no sim accumulation in menu
                    }
                    GamePhase::Playing(session) => {
                        let actions = s.input.to_actions();

                        // Step sim at fixed 60Hz, potentially multiple times to catch up
                        while s.tick_accum >= SIM_DT {
                            s.tick_accum -= SIM_DT;
                            let result = session.step(&actions).clone();
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
                            // Check for wave clear — pause client-side
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

                        // Copy composed framebuffer
                        if let GamePhase::Playing(session) = &s.phase {
                            s.fb.pixels.copy_from_slice(session.framebuffer());

                            // Check for pause
                            if s.input.just_pressed(key_escape()) {
                                let GamePhase::Playing(mut session) = std::mem::replace(
                                    &mut s.phase,
                                    GamePhase::Menu { selected: 0 },
                                ) else {
                                    unreachable!()
                                };
                                let forks = session.list_forks();
                                s.phase = GamePhase::Paused {
                                    session,
                                    selected: 0,
                                    forks: forks.clone(),
                                };
                                render_pause(&mut s.fb, &mut s.font, 0, &forks);
                            } else if session.is_done() {
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
                        // Render the last game frame with wave clear overlay
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
                    GamePhase::GameOver {
                        session,
                        timer,
                        score,
                    } => {
                        *timer -= dt;
                        s.fb.pixels.copy_from_slice(session.framebuffer());
                        render_game_over(&mut s.fb, &mut s.font, *score);

                        if *timer <= 0.0 {
                            s.phase = GamePhase::Menu { selected: 0 };
                            s.audio.play_music(MusicTrack::Menu);
                        }
                    }
                    GamePhase::Paused {
                        session,
                        selected,
                        forks,
                    } => {
                        let menu_len = pause_menu_labels(forks).len();

                        if s.input.just_pressed(key_up()) || s.input.just_pressed(key_char("w")) {
                            if *selected > 0 {
                                *selected -= 1;
                            }
                        }
                        if s.input.just_pressed(key_down()) || s.input.just_pressed(key_char("s")) {
                            if *selected + 1 < menu_len {
                                *selected += 1;
                            }
                        }

                        if s.input.just_pressed(key_escape()) {
                            // Escape always resumes
                            let GamePhase::Paused { session, .. } =
                                std::mem::replace(&mut s.phase, GamePhase::Menu { selected: 0 })
                            else {
                                unreachable!()
                            };
                            s.phase = GamePhase::Playing(session);
                        } else if s.input.just_pressed(key_enter())
                            || s.input.just_pressed(key_space())
                        {
                            let action = pause_menu_action(*selected, forks);
                            match action {
                                PauseAction::Resume => {
                                    let GamePhase::Paused { session, .. } = std::mem::replace(
                                        &mut s.phase,
                                        GamePhase::Menu { selected: 0 },
                                    ) else {
                                        unreachable!()
                                    };
                                    s.phase = GamePhase::Playing(session);
                                }
                                PauseAction::CreateFork => {
                                    let new_id = session.fork();
                                    log::info!("created fork #{new_id}");
                                    *forks = session.list_forks();
                                }
                                PauseAction::OpenForkDetail(fork_id, fork_step) => {
                                    let GamePhase::Paused { session, forks, .. } =
                                        std::mem::replace(
                                            &mut s.phase,
                                            GamePhase::Menu { selected: 0 },
                                        )
                                    else {
                                        unreachable!()
                                    };
                                    s.phase = GamePhase::ForkDetail {
                                        session,
                                        fork_id,
                                        fork_step,
                                        selected: 0,
                                        forks,
                                    };
                                }
                                PauseAction::Exit => {
                                    s.phase = GamePhase::Menu { selected: 0 };
                                    s.audio.play_music(MusicTrack::Menu);
                                }
                            }
                        }

                        // Render pause overlay (only if still paused)
                        if let GamePhase::Paused {
                            session,
                            selected,
                            forks,
                        } = &mut s.phase
                        {
                            s.fb.pixels.copy_from_slice(session.framebuffer());
                            render_pause(&mut s.fb, &mut s.font, *selected, forks);
                        }

                        s.tick_accum = 0.0;
                    }
                    GamePhase::ForkDetail {
                        session,
                        fork_id,
                        selected,
                        ..
                    } => {
                        if s.input.just_pressed(key_up()) || s.input.just_pressed(key_char("w")) {
                            if *selected > 0 {
                                *selected -= 1;
                            }
                        }
                        if s.input.just_pressed(key_down()) || s.input.just_pressed(key_char("s")) {
                            if *selected < 2 {
                                *selected += 1;
                            }
                        }

                        if s.input.just_pressed(key_escape()) {
                            // Back to pause menu
                            let GamePhase::ForkDetail { session, forks, .. } =
                                std::mem::replace(&mut s.phase, GamePhase::Menu { selected: 0 })
                            else {
                                unreachable!()
                            };
                            s.phase = GamePhase::Paused {
                                session,
                                selected: 0,
                                forks,
                            };
                        } else if s.input.just_pressed(key_enter())
                            || s.input.just_pressed(key_space())
                        {
                            match *selected {
                                // ENTER — switch to this fork and resume
                                0 => {
                                    let fid = *fork_id;
                                    let GamePhase::ForkDetail { mut session, .. } =
                                        std::mem::replace(
                                            &mut s.phase,
                                            GamePhase::Menu { selected: 0 },
                                        )
                                    else {
                                        unreachable!()
                                    };
                                    match session.restore(fid) {
                                        Ok(()) => {
                                            s.phase = GamePhase::Playing(session);
                                        }
                                        Err(e) => {
                                            log::error!("restore fork failed: {e}");
                                        }
                                    }
                                }
                                // DELETE — kill this fork and go back to pause
                                1 => {
                                    let fid = *fork_id;
                                    match session.kill_fork(fid) {
                                        Ok(()) => {
                                            log::info!("deleted fork #{fid}");
                                            let new_forks = session.list_forks();
                                            let GamePhase::ForkDetail { session, .. } =
                                                std::mem::replace(
                                                    &mut s.phase,
                                                    GamePhase::Menu { selected: 0 },
                                                )
                                            else {
                                                unreachable!()
                                            };
                                            s.phase = GamePhase::Paused {
                                                session,
                                                selected: 0,
                                                forks: new_forks,
                                            };
                                        }
                                        Err(e) => {
                                            log::error!("delete fork failed: {e}");
                                        }
                                    }
                                }
                                // BACK — go back to pause menu
                                _ => {
                                    let GamePhase::ForkDetail { session, forks, .. } =
                                        std::mem::replace(
                                            &mut s.phase,
                                            GamePhase::Menu { selected: 0 },
                                        )
                                    else {
                                        unreachable!()
                                    };
                                    s.phase = GamePhase::Paused {
                                        session,
                                        selected: 0,
                                        forks,
                                    };
                                }
                            }
                        }

                        // Render fork detail overlay (only if still in this phase)
                        if let GamePhase::ForkDetail {
                            session,
                            fork_id,
                            fork_step,
                            selected,
                            ..
                        } = &s.phase
                        {
                            s.fb.pixels.copy_from_slice(session.framebuffer());
                            render_fork_detail(
                                &mut s.fb,
                                &mut s.font,
                                *fork_id,
                                *fork_step,
                                *selected,
                            );
                        }

                        s.tick_accum = 0.0;
                    }
                    GamePhase::Credits => {
                        render_credits(&mut s.fb, &mut s.font);
                        if s.input.just_pressed(key_escape())
                            || s.input.just_pressed(key_enter())
                            || s.input.just_pressed(key_space())
                        {
                            s.phase = GamePhase::Menu { selected: 0 };
                        }
                    }
                    GamePhase::ServerError => {
                        render_server_error(&mut s.fb, &mut s.font);
                        if s.input.just_pressed(key_enter()) || s.input.just_pressed(key_space()) {
                            s.phase = GamePhase::Menu { selected: 0 };
                        }
                    }
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
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
