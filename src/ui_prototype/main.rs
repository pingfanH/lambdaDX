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
    next_frame().await;

    loop {
        clear_background(Color::from_rgba(31, 31, 36, 255));

        egui_macroquad::ui(|egui_ctx| {
            lambda_dx::ui_prototype::draw_editor(egui_ctx);
        });

        egui_macroquad::draw();
        next_frame().await;
    }
}
