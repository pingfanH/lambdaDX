use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Lambda DX Editor".to_string(),
        window_width: 1920,
        window_height: 1080,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Wait a frame for window to be ready
    next_frame().await;

    loop {
        lambda_dx::ui_prototype::draw_editor();
        next_frame().await;
    }
}
