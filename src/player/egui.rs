mod gameplay;
mod library;
mod pause;
mod settings;
mod song_select;
mod start;
mod theme;
mod widgets;

use egui_macroquad::egui;

use crate::state::{PlayerPage, PlayerState};

pub fn draw_egui_ui(ctx: &egui::Context, app: &mut PlayerState) {
    theme::apply(ctx);
    library::ensure_cover_textures(ctx, app);

    match app.player_ui.page {
        PlayerPage::Start => start::draw(ctx, app),
        PlayerPage::SongSelect => song_select::draw(ctx, app),
        PlayerPage::Settings => settings::draw(ctx, app),
        PlayerPage::Gameplay => gameplay::draw(ctx, app),
        PlayerPage::Pause => {
            gameplay::draw(ctx, app);
            pause::draw(ctx, app);
        }
    }
}

pub fn finish_dialog_import(app: &mut PlayerState, import: lambda_dx::simai_io::DialogImport) {
    library::apply_import(app, import.clone(), None);
    match library::import_song_to_library(app, &import) {
        Ok(index) => {
            let _ = library::load_song(app, index);
            app.player_ui.selected_song = index;
        }
        Err(e) => app.set_status(format!("导入到曲库失败: {e}")),
    }
    app.player_ui.page = PlayerPage::SongSelect;
}
