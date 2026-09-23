//! LambdaDX player UI — a new front-end built on the existing pad view.
//!
//! This bin keeps the current macroquad pad rendering / playback
//! (`crate::player`) as the gameplay "view", and adds a pure-macroquad,
//! self-drawn interface around it (Start / Song select / Settings / Gameplay /
//! Pause). The interface logic mirrors the original `macroquad_sim` player
//! (`src/player/egui/*`: start, song_select, settings, gameplay, pause, library)
//! but is restyled: Blender/Editor black-minimal surfaces with a cyan playhead
//! accent, plus the motion language of a sticker-sheet layout — staggered list
//! reveals, spring pops, pressed-in offsets and a page sweep.
//!
//! The `app`, `player` and `simai` modules are mounted at the crate root through
//! `#[path]`, so the same source powers both this bin and the original preview.

#![allow(dead_code, unused_variables, unused_imports)]

#[path = "app/mod.rs"]
mod app;
#[path = "player/mod.rs"]
mod player;
#[path = "player_ui/mod.rs"]
mod player_ui;
#[path = "simai/mod.rs"]
mod simai;

use macroquad::Window;

fn main() {
    Window::from_config(player_ui::window_conf(), player_ui::run());
}
