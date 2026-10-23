//! Desktop window events and Android's direct lifecycle callbacks share one gate.

use macroquad::{input::utils, miniquad::EventHandler};

pub struct WindowEvents { subscriber: usize }

impl WindowEvents {
    pub fn new() -> Self { Self { subscriber: utils::register_input_subscriber() } }
    pub fn poll(&mut self) { utils::repeat_all_miniquad_input(self, self.subscriber); }
}

impl EventHandler for WindowEvents {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn window_minimized_event(&mut self) {
        // Android's Java callback runs before the render thread is suspended.
        #[cfg(not(target_os = "android"))]
        crate::app::lifecycle::global().suspend();
    }
    fn window_restored_event(&mut self) { crate::app::lifecycle::global().restore(); }
}
