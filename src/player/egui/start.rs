use egui_macroquad::egui::{self, Color32, Frame, RichText, Stroke, Vec2};

use crate::state::{PlayerPage, PlayerState};

use super::{library, theme, widgets};

pub fn draw(ctx: &egui::Context, app: &mut PlayerState) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::BG_VOID).inner_margin(48.0))
        .show(ctx, |ui| {
            let wide = ui.available_width() >= 760.0;
            if wide {
                ui.columns(2, |columns| {
                    draw_copy(&mut columns[0], app);
                    draw_visual(&mut columns[1], app);
                });
            } else {
                draw_copy(ui, app);
                ui.add_space(24.0);
                draw_visual(ui, app);
            }
        });
}

fn draw_copy(ui: &mut egui::Ui, app: &mut PlayerState) {
    ui.vertical_centered(|ui| {
        ui.add_space(32.0);
        super::widgets::overline(ui, "ARCADE CHART PLAYER");
        ui.add_space(12.0);
        ui.label(
            RichText::new("LambdaDX")
                .size(46.0)
                .strong()
                .color(theme::TEXT_PRIMARY),
        );
        ui.label(
            RichText::new("PLAYER")
                .size(18.0)
                .strong()
                .color(theme::ACCENT_CYAN),
        );
        ui.add_space(12.0);
        ui.label(
            RichText::new("把节拍变成动作。选择谱面，设定难度，进入你的下一局。")
                .size(16.0)
                .color(theme::TEXT_SECONDARY),
        );
        ui.add_space(32.0);

        if widgets::command_button(ui, "开始 · 选歌", widgets::ButtonKind::Primary).clicked() {
            match library::load_song(app, app.player_ui.selected_song) {
                Ok(()) => app.player_ui.page = PlayerPage::SongSelect,
                Err(error) => app.player_ui.song_error = Some(error),
            }
        }
        ui.add_space(8.0);
        if widgets::command_button(ui, "设置", widgets::ButtonKind::Secondary).clicked() {
            app.player_ui.open_settings();
        }
        if let Some(error) = &app.player_ui.song_error {
            ui.add_space(12.0);
            ui.colored_label(theme::ACCENT_CORAL, error);
        }
    });
}

fn draw_visual(ui: &mut egui::Ui, app: &PlayerState) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        if let Some(texture) = &app.ui_logo_texture {
            ui.add(
                egui::Image::new((texture.id(), Vec2::splat(280.0)))
                    .corner_radius(egui::CornerRadius::same(12)),
            );
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(280.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 12.0, theme::BG_PANEL);
            ui.painter().rect_stroke(
                rect,
                12.0,
                Stroke::new(1.0_f32, theme::BORDER),
                egui::StrokeKind::Inside,
            );
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new("READY WHEN YOU ARE")
                .size(11.0)
                .strong()
                .color(theme::TEXT_MUTED),
        );
        ui.label(
            RichText::new("独立曲库 · 键盘与触摸输入")
                .size(13.0)
                .color(Color32::from_rgb(168, 180, 194)),
        );
    });
}
