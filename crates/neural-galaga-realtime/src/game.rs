use neural_galaga_core::session::{ForkInfo, GameSession};
use neural_galaga_core::*;
use qrcode::QrCode;

pub enum GamePhase {
    Menu {
        selected: usize,
    },
    Playing(GameSession),
    GameOver {
        session: GameSession,
        timer: f32,
        score: i32,
    },
    WaveClear {
        session: GameSession,
        timer: f32,
        wave: u32,
    },
    Paused {
        session: GameSession,
        selected: usize,
        forks: Vec<ForkInfo>,
    },
    ForkDetail {
        session: GameSession,
        fork_id: u32,
        fork_step: u64,
        selected: usize,
        forks: Vec<ForkInfo>,
    },
    Credits,
    ServerError,
}

const PAUSE_MAX_FORKS: usize = 3;

/// Get the most recent snapshots, sorted most recent first.
pub fn recent_forks(forks: &[ForkInfo]) -> Vec<ForkInfo> {
    let mut recent: Vec<ForkInfo> = forks.to_vec();
    recent.sort_by(|a, b| b.fork_id.cmp(&a.fork_id));
    recent.truncate(PAUSE_MAX_FORKS);
    recent
}

/// Build the labels for the pause menu.
pub fn pause_menu_labels(forks: &[ForkInfo]) -> Vec<String> {
    let mut labels = vec!["RESUME".to_string(), "SAVE".to_string()];
    for f in &recent_forks(forks) {
        labels.push(format!("FORK #{} @{}", f.fork_id, f.step));
    }
    labels.push("EXIT".to_string());
    labels
}

pub enum PauseAction {
    Resume,
    CreateFork,
    OpenForkDetail(u32, u64),
    Exit,
}

pub fn pause_menu_action(selected: usize, forks: &[ForkInfo]) -> PauseAction {
    if selected == 0 {
        return PauseAction::Resume;
    }
    if selected == 1 {
        return PauseAction::CreateFork;
    }
    let labels = pause_menu_labels(forks);
    if selected == labels.len() - 1 {
        return PauseAction::Exit;
    }
    let recent = recent_forks(forks);
    let fork_index = selected - 2;
    if fork_index < recent.len() {
        PauseAction::OpenForkDetail(recent[fork_index].fork_id, recent[fork_index].step)
    } else {
        PauseAction::Exit
    }
}

/// Render the menu screen into a framebuffer.
pub fn render_menu(
    fb: &mut Framebuffer,
    sheet: &SpriteSheet,
    font: &mut BitmapFont,
    selected: usize,
) {
    font.draw_text_centered(fb, "NEURAL", 30.0, 3.0, [0.3, 0.6, 1.0, 1.0]);
    font.draw_text_centered(fb, "GALAGA", 58.0, 3.0, [0.3, 0.6, 1.0, 1.0]);

    let options = ["START GAME", "CREDITS"];
    for (i, label) in options.iter().enumerate() {
        let color = if i == selected {
            [1.0, 1.0, 0.2, 1.0]
        } else {
            [0.5, 0.5, 0.5, 1.0]
        };
        let y = 110.0 + i as f32 * 24.0;
        font.draw_text_centered(fb, label, y, 1.0, color);

        if i == selected {
            let label_w = font.text_width(label, 1.0);
            let label_x = (GAME_WIDTH - label_w) / 2.0;
            font.draw_text(fb, ">", label_x - 10.0, y, 1.0, [1.0, 1.0, 1.0, 1.0]);
        }
    }

    font.draw_text_centered(fb, "ARROWS/ENTER", 230.0, 1.0, [0.4, 0.4, 0.4, 1.0]);

    // 5 sprites: White, Red, Player, Yellow, Green — centered in 224px
    let x_start = (GAME_WIDTH - 4.0 * 24.0 - 16.0) / 2.0;
    let sprite_y = 196.0;
    sheet.draw(fb, &REGION_BW_F1, x_start, sprite_y, 1.0);
    sheet.draw(fb, &REGION_RED_F1, x_start + 24.0, sprite_y, 1.0);
    sheet.draw(fb, &REGION_PLAYER, x_start + 48.0, sprite_y, 1.0);
    sheet.draw(fb, &REGION_YELLOW_F1, x_start + 72.0, sprite_y, 1.0);
    sheet.draw(fb, &REGION_GREEN_F1, x_start + 96.0, sprite_y, 1.0);
}

fn render_dim_overlay(fb: &mut Framebuffer) {
    fb.fill_rect(
        0,
        0,
        GAME_WIDTH as u32,
        GAME_HEIGHT as u32,
        [0.0, 0.0, 0.0, 0.6],
    );
}

/// Render the game over overlay on top of the last game frame.
pub fn render_game_over(fb: &mut Framebuffer, font: &mut BitmapFont, score: i32) {
    render_dim_overlay(fb);
    font.draw_text_centered(fb, "GAME OVER", 110.0, 2.0, [1.0, 0.2, 0.2, 1.0]);
    font.draw_text_centered(
        fb,
        &format!("SCORE: {}", score),
        150.0,
        1.0,
        [0.8, 0.8, 0.8, 1.0],
    );
}

/// Render the wave clear overlay on top of the last game frame.
pub fn render_wave_clear(fb: &mut Framebuffer, font: &mut BitmapFont, wave: u32) {
    render_dim_overlay(fb);
    font.draw_text_centered(fb, "WAVE CLEAR", 110.0, 2.0, [1.0, 1.0, 0.2, 1.0]);
    font.draw_text_centered(
        fb,
        &format!("WAVE {}", wave - 1),
        150.0,
        1.0,
        [0.6, 0.6, 0.6, 1.0],
    );
}

fn render_menu_items(
    fb: &mut Framebuffer,
    font: &mut BitmapFont,
    labels: &[String],
    selected: usize,
    start_y: f32,
) {
    for (i, label) in labels.iter().enumerate() {
        let color = if i == selected {
            [1.0, 1.0, 0.2, 1.0]
        } else {
            [0.6, 0.6, 0.6, 1.0]
        };
        let y = start_y + i as f32 * 16.0;
        font.draw_text_centered(fb, label, y, 1.0, color);

        if i == selected {
            let label_w = font.text_width(label, 1.0);
            let label_x = (GAME_WIDTH - label_w) / 2.0;
            font.draw_text(fb, ">", label_x - 10.0, y, 1.0, [1.0, 1.0, 1.0, 1.0]);
        }
    }
}

/// Render the pause overlay into a framebuffer (on top of existing content).
pub fn render_pause(
    fb: &mut Framebuffer,
    font: &mut BitmapFont,
    selected: usize,
    forks: &[ForkInfo],
) {
    render_dim_overlay(fb);

    font.draw_text_centered(fb, "PAUSED", 70.0, 2.0, [1.0, 1.0, 1.0, 1.0]);

    let labels = pause_menu_labels(forks);
    render_menu_items(fb, font, &labels, selected, 110.0);
}

/// Render the fork detail sub-menu.
pub fn render_fork_detail(
    fb: &mut Framebuffer,
    font: &mut BitmapFont,
    fork_id: u32,
    fork_step: u64,
    selected: usize,
) {
    render_dim_overlay(fb);

    font.draw_text_centered(fb, "FORK DETAIL", 60.0, 2.0, [1.0, 1.0, 1.0, 1.0]);
    font.draw_text_centered(
        fb,
        &format!("#{} @ STEP {}", fork_id, fork_step),
        90.0,
        1.0,
        [0.5, 0.8, 0.5, 1.0],
    );

    let labels = vec![
        "ENTER".to_string(),
        "DELETE".to_string(),
        "BACK".to_string(),
    ];
    render_menu_items(fb, font, &labels, selected, 130.0);
}

/// Render the credits screen into a framebuffer.
pub fn render_credits(fb: &mut Framebuffer, font: &mut BitmapFont) {
    font.draw_text_centered(fb, "CREDITS", 24.0, 2.0, [0.3, 0.6, 1.0, 1.0]);
    font.draw_text_centered(fb, "A GALAGA CLONE", 60.0, 1.0, [0.8, 0.8, 0.8, 1.0]);
    font.draw_text_centered(fb, "BY MAGNIFF", 78.0, 1.0, [1.0, 0.8, 0.3, 1.0]);

    // QR code for the source repo
    let url = "https://github.com/magniff/neural-galaga";
    let code = QrCode::new(url.as_bytes()).unwrap();
    let modules = code.to_colors();
    let qr_width = code.width() as u32;
    let pixel_size: u32 = 3;
    let qr_total = qr_width * pixel_size;
    let qr_x = ((GAME_WIDTH as u32) - qr_total) / 2;
    let qr_y: u32 = 104;

    // White background behind QR
    fb.fill_rect(
        qr_x as i32 - 3,
        qr_y as i32 - 3,
        qr_total + 6,
        qr_total + 6,
        [1.0, 1.0, 1.0, 1.0],
    );

    for (i, color) in modules.iter().enumerate() {
        let mx = (i as u32) % qr_width;
        let my = (i as u32) / qr_width;
        let c = if color.select(true, false) {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        fb.fill_rect(
            (qr_x + mx * pixel_size) as i32,
            (qr_y + my * pixel_size) as i32,
            pixel_size,
            pixel_size,
            c,
        );
    }

    let qr_bottom = qr_y + qr_total + 6;
    font.draw_text_centered(
        fb,
        "SOURCE CODE",
        qr_bottom as f32 + 4.0,
        1.0,
        [0.6, 0.6, 0.6, 1.0],
    );

    font.draw_text_centered(fb, "PRESS ESC", 264.0, 1.0, [0.4, 0.4, 0.4, 1.0]);
}

/// Render the server error screen into a framebuffer.
pub fn render_server_error(fb: &mut Framebuffer, font: &mut BitmapFont) {
    font.draw_text_centered(fb, "ERROR", 80.0, 2.0, [1.0, 0.2, 0.2, 1.0]);
    font.draw_text_centered(fb, "SERVER IS NOT", 130.0, 1.0, [0.8, 0.8, 0.8, 1.0]);
    font.draw_text_centered(fb, "REPLYING", 148.0, 1.0, [0.8, 0.8, 0.8, 1.0]);
    font.draw_text_centered(fb, "OH WELL", 200.0, 1.0, [1.0, 1.0, 0.2, 1.0]);
}
