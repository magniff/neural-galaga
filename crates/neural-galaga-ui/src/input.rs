use neural_galaga_core::Action;
use std::collections::HashSet;
use winit::keyboard::{Key, NamedKey};

pub struct InputState {
    pressed: HashSet<Key>,
    just_pressed: HashSet<Key>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
        }
    }

    pub fn key_down(&mut self, key: Key) {
        if self.pressed.insert(key.clone()) {
            self.just_pressed.insert(key);
        }
    }

    pub fn key_up(&mut self, key: Key) {
        self.pressed.remove(&key);
    }

    pub fn is_held(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    pub fn just_pressed(&self, key: Key) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn end_frame(&mut self) {
        self.just_pressed.clear();
    }

    /// Clear all state — used on phase transitions to prevent input bleed.
    pub fn clear(&mut self) {
        self.pressed.clear();
        self.just_pressed.clear();
    }

    /// Convert current keyboard state into a list of core Actions.
    pub fn to_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.is_held(key_left()) || self.is_held(key_char("a")) {
            actions.push(Action::Left);
        }
        if self.is_held(key_right()) || self.is_held(key_char("d")) {
            actions.push(Action::Right);
        }
        if self.is_held(key_space()) {
            actions.push(Action::Fire);
        }
        actions
    }
}

pub fn key_up() -> Key {
    Key::Named(NamedKey::ArrowUp)
}
pub fn key_down() -> Key {
    Key::Named(NamedKey::ArrowDown)
}
pub fn key_left() -> Key {
    Key::Named(NamedKey::ArrowLeft)
}
pub fn key_right() -> Key {
    Key::Named(NamedKey::ArrowRight)
}
pub fn key_enter() -> Key {
    Key::Named(NamedKey::Enter)
}
pub fn key_escape() -> Key {
    Key::Named(NamedKey::Escape)
}
pub fn key_space() -> Key {
    Key::Named(NamedKey::Space)
}
pub fn key_char(c: &str) -> Key {
    Key::Character(c.into())
}
