use egui_macroquad::egui::{self, Color32, Frame, RichText, Stroke};

use crate::state::{PlayerPage, PlayerState};

use super::{theme, widgets};

pub fn draw(ctx: &egui::Context, app: &mut PlayerState) {
    let rect = ctx.screen_rect();
    egui::Area::new("pause_overlay".into())
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(210));
            ui.vertical_centered(|ui| {
                ui.add_space((rect.height() * 0.18).max(32.0));
                Frame::new()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                    .corner_radius(theme::RADIUS_PANEL)
                    .inner_margin(32.0)
                    .show(ui, |ui| {
                        ui.set_width(rect.width().min(440.0));
                        widgets::overline(ui, "PLAY SESSION");
                        ui.add_space(8.0);
                        widgets::heading(ui, "已暂停", 32.0);
                        ui.label(
                            RichText::new(&app.chart.title)
                                .size(15.0)
                                .color(theme::TEXT_SECONDARY),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!(
                                "当前时间  {}",
                                format_time(app.mode_song_offset)
                            ))
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(24.0);

                        if widgets::command_button(ui, "继续游玩", widgets::ButtonKind::Primary)
                            .clicked()
                        {
                            app.toggle_play();
                            app.player_ui.page = PlayerPage::Gameplay;
                        }
                        ui.add_space(8.0);
                        if widgets::command_button(ui, "重新开始", widgets::ButtonKind::Secondary)
                            .clicked()
                        {
                            app.toggle_replay();
                            app.player_ui.page = PlayerPage::Gameplay;
                        }
                        ui.add_space(8.0);
                        if widgets::command_button(ui, "游玩设置", widgets::ButtonKind::Quiet)
                            .clicked()
                        {
                            app.player_ui.open_settings();
                        }
                        ui.add_space(8.0);
                        if widgets::command_button(ui, "退出到选歌", widgets::ButtonKind::Danger)
                            .clicked()
                        {
                            app.stop_audio_if_any();
                            app.mode_song_offset = 0.0;
                            app.player_ui.page = PlayerPage::SongSelect;
                        }
                    });
            });
        });
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
