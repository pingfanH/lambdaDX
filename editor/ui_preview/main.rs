//! Memory-only design prototype: Chart, Autoplay and Combined workspaces.
//! Deliberately independent of the editor runtime and gameplay core.

#[allow(dead_code)]
#[path = "../../src/app/egui_bridge.rs"]
mod egui_bridge;
mod model;
mod panels;
mod theme;
mod workbench;

use macroquad::{Window, prelude::*};

fn main() {
    let size = std::env::var("MAI2_UI_PREVIEW_WINDOW")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?))
        })
        .unwrap_or((1500, 940));
    Window::from_config(
        Conf {
            window_title: "lambdaDX | Workbench UI Preview".into(),
            window_width: size.0.max(800),
            window_height: size.1.max(600),
            window_resizable: true,
            high_dpi: true,
            ..Default::default()
        },
        run(),
    );
}

async fn run() {
    egui_bridge::cfg(theme::install);
    let mut app = None;
    let screenshot = std::env::var("MAI2_UI_PREVIEW_SHOT").ok();
    let mut frames = 0;
    loop {
        clear_background(Color::from_rgba(30, 31, 32, 255));
        let (x, y) = mouse_position();
        egui_bridge::pointer(
            vec2(x, y),
            is_mouse_button_down(MouseButton::Left),
            is_mouse_button_pressed(MouseButton::Left),
            is_mouse_button_released(MouseButton::Left),
        );
        egui_bridge::ui(|ctx| {
            let app = app.get_or_insert_with(|| workbench::Workbench::new(ctx));
            app.show(ctx, get_frame_time().min(0.05));
        });
        egui_bridge::draw();
        frames += 1;
        if frames == 12 {
            if let Some(path) = &screenshot {
                get_screen_data().export_png(path);
                return;
            }
        }
        next_frame().await;
    }
}
