use std::io::{BufReader, Cursor};
use std::sync::Arc;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

pub enum MusicTrack {
    Menu,
    Game,
    None,
}

pub struct Audio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    music_sink: Sink,
    current_track: MusicTrack,
    muted: bool,
    sfx_player_fire: Arc<[u8]>,
    sfx_enemy_fire: Arc<[u8]>,
    sfx_player_hit: Arc<[u8]>,
    sfx_enemy_hit: Arc<[u8]>,
    sfx_shield_hit: Arc<[u8]>,
    sfx_powerup_pickup: Arc<[u8]>,
    music_menu: Arc<[u8]>,
    music_game: Arc<[u8]>,
}

impl Audio {
    pub fn new(preferred_device: Option<String>) -> Self {
        let (stream, handle) =
            Self::open_output(preferred_device).expect("Failed to open audio output");
        let music_sink = Sink::try_new(&handle).unwrap();
        music_sink.set_volume(1.0);

        let assets = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
        let load = |name: &str| -> Arc<[u8]> {
            std::fs::read(format!("{assets}/{name}"))
                .unwrap_or_else(|e| panic!("Failed to load {name}: {e}"))
                .into()
        };

        Self {
            _stream: stream,
            handle,
            music_sink,
            current_track: MusicTrack::None,
            muted: false,
            sfx_player_fire: load("player_fire.wav"),
            sfx_enemy_fire: load("enemy_fire.wav"),
            sfx_player_hit: load("player_hit.wav"),
            sfx_enemy_hit: load("enemy_hit.wav"),
            sfx_shield_hit: load("shield_hit.wav"),
            sfx_powerup_pickup: load("powerup_pickup.wav"),
            music_menu: load("menu_music.mp3"),
            music_game: load("game_music.mp3"),
        }
    }

    fn open_output(
        preferred: Option<String>,
    ) -> Result<(OutputStream, OutputStreamHandle), rodio::StreamError> {
        if let Some(ref pref_name) = preferred {
            let host = rodio::cpal::default_host();
            if let Ok(devices) = host.output_devices() {
                for dev in devices {
                    if let Ok(name) = dev.name() {
                        if &name == pref_name {
                            if let Ok(result) = OutputStream::try_from_device(&dev) {
                                return Ok(result);
                            }
                        }
                    }
                }
            }
        }
        OutputStream::try_default()
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if self.muted {
            self.music_sink.set_volume(0.0);
        } else {
            self.music_sink.set_volume(1.0);
        }
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    fn play_sfx(&self, data: &Arc<[u8]>) {
        if self.muted {
            return;
        }
        if let Ok(source) = Decoder::new(BufReader::new(Cursor::new(data.clone()))) {
            let _ = self.handle.play_raw(source.convert_samples::<f32>());
        }
    }

    pub fn player_fire(&self) {
        self.play_sfx(&self.sfx_player_fire);
    }
    pub fn enemy_fire(&self) {
        self.play_sfx(&self.sfx_enemy_fire);
    }
    pub fn player_hit(&self) {
        self.play_sfx(&self.sfx_player_hit);
    }
    pub fn enemy_hit(&self) {
        self.play_sfx(&self.sfx_enemy_hit);
    }
    pub fn shield_hit(&self) {
        self.play_sfx(&self.sfx_shield_hit);
    }
    pub fn powerup_pickup(&self) {
        self.play_sfx(&self.sfx_powerup_pickup);
    }

    pub fn play_music(&mut self, track: MusicTrack) {
        match (&self.current_track, &track) {
            (MusicTrack::Menu, MusicTrack::Menu) | (MusicTrack::Game, MusicTrack::Game) => return,
            _ => {}
        }
        self.music_sink.stop();
        self.music_sink = Sink::try_new(&self.handle).unwrap();
        // Honor the current mute state — otherwise switching tracks (or starting one
        // after launch) silently re-enables music even when the user expected mute.
        self.music_sink
            .set_volume(if self.muted { 0.0 } else { 1.0 });

        let data = match &track {
            MusicTrack::Menu => self.music_menu.clone(),
            MusicTrack::Game => self.music_game.clone(),
            MusicTrack::None => {
                self.current_track = MusicTrack::None;
                return;
            }
        };

        if let Ok(source) = Decoder::new(BufReader::new(Cursor::new(data))) {
            self.music_sink.append(source.repeat_infinite());
        }
        self.current_track = track;
    }
}

pub fn scan_preferred_device() -> std::thread::JoinHandle<Option<String>> {
    std::thread::spawn(|| {
        let host = rodio::cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if let Ok(name) = dev.name() {
                    if name == "pipewire" || name == "pulse" {
                        return Some(name);
                    }
                }
            }
        }
        None
    })
}
