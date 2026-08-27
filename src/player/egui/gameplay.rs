use egui_macroquad::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::state::PlayerState;

use super::{theme, widgets};

pub fn draw(ctx: &egui::Context, app: &mut PlayerState) {
    egui::TopBottomPanel::top("gameplay_hud")
        .exact_height(72.0)
        .frame(
            egui::Frame::new()
                .fill(theme::BG_PANEL)
                .inner_margin(egui::Margin::symmetric(20, 10))
                .stroke(Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if widgets::compact_button(ui, "暂停", widgets::ButtonKind::Danger).clicked() {
                    app.toggle_play();
                    app.player_ui.page = crate::state::PlayerPage::Pause;
                }
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&app.chart.title).strong());
                    ui.label(
                        RichText::new(format!(
                            "Lv.{}  ·  {:.1}x",
                            app.import_selected_level, app.play_speed
                        ))
                        .size(12.0)
                        .color(theme::TEXT_SECONDARY),
                    );
                });
                ui.add_space(18.0);
                let duration = app
                    .audio_wav_pcm
                    .as_ref()
                    .map(|pcm| {
                        pcm.samples.len() as f32
                            / f32::from(pcm.channels.max(1))
                            / pcm.sample_rate as f32
                    })
                    .unwrap_or(1.0)
                    .max(1.0);
                let progress = (app.song_time() / duration).clamp(0.0, 1.0);
                ui.add_sized(
                    Vec2::new((ui.available_width() - 160.0).max(80.0), 8.0),
                    egui::ProgressBar::new(progress)
                        .fill(theme::ACCENT_CYAN)
                        .text(""),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format_time(app.song_time()))
                        .monospace()
                        .color(theme::TEXT_SECONDARY),
                );
                ui.separator();
                ui.label(
                    RichText::new(if app.audio_enabled {
                        "音频 ON"
                    } else {
                        "静音"
                    })
                    .size(12.0)
                    .color(if app.audio_enabled {
                        theme::STATUS_SUCCESS
                    } else {
                        theme::TEXT_MUTED
                    }),
                );
            });
        });

    egui::Area::new("gameplay_hint".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -18.0])
        .show(ctx, |ui| {
            ui.label(
                RichText::new("SPACE 暂停  ·  ESC 暂停")
                    .size(12.0)
                    .color(Color32::from_rgba_unmultiplied(168, 180, 194, 200)),
            );
        });
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
