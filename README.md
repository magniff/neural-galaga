# Neural Galaga

A Galaga clone built from scratch in Rust with wgpu, designed for both human play and reinforcement learning experiments.

The game renders at the original Galaga resolution of 224x288 and upscales to 1120x1440 with nearest-neighbor filtering for a crisp pixel-art look. It features infinite waves of increasing difficulty, four enemy classes with distinct behaviors, a shield system for both player and enemies, and collectible powerups.

## Running the Game

```bash
cargo run --release -p neural-galaga-realtime
```

Requires Rust 1.85+ (edition 2024).

## Controls

| Key | Action |
|---|---|
| Arrow Left / A | Move left |
| Arrow Right / D | Move right |
| Space | Fire |
| Enter | Confirm menu selection |
| Arrow Up/Down / W/S | Navigate menus |
| Escape | Pause (in-game) / Back (menus) |
