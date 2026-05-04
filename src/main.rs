use macroquad_sim::{run_app, window_conf};

#[macroquad::main(window_conf)]
async fn main() {
    run_app().await;
}
