use egui_macroquad::egui::{self, Frame, RichText, ScrollArea, Stroke, Vec2};

use crate::state::{PlayerPage, PlayerState};

use super::{library, theme, widgets};

pub fn draw(ctx: &egui::Context, app: &mut PlayerState) {
    if app.player_ui.loaded_song.is_none() {
        let _ = library::load_song(app, app.player_ui.selected_song);
    }

    if widgets::page_top_bar(ctx, "返回", "选择歌曲", |ui| {
        if widgets::compact_button(ui, "设置", widgets::ButtonKind::Quiet).clicked() {
            app.player_ui.open_settings();
        }
    }) {
        app.player_ui.page = PlayerPage::Start;
        return;
    }

    let narrow = ctx.screen_rect().width() < 760.0;
    if narrow {
        egui::CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                draw_song_list(ui, app);
                ui.separator();
                draw_song_detail(ui, app);
            });
        });
    } else {
        egui::SidePanel::left("song_list")
            .exact_width(360.0)
            .frame(Frame::new().fill(theme::BG_PANEL).inner_margin(20.0))
            .show(ctx, |ui| draw_song_list(ui, app));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::BG_VOID).inner_margin(32.0))
            .show(ctx, |ui| draw_song_detail(ui, app));
    }
}

fn draw_song_list(ui: &mut egui::Ui, app: &mut PlayerState) {
    widgets::overline(ui, "曲库");
    ui.add_space(6.0);
    ui.label(RichText::new("选择一首歌开始").size(18.0).strong());
    ui.add_space(16.0);
    if app.song_library.is_empty() {
        ui.label(RichText::new("曲库目录中没有 maidata.txt").color(theme::TEXT_SECONDARY));
    }
    for index in 0..app.song_library.len() {
        let song = &app.song_library[index];
        let selected = app.player_ui.selected_song == index;
        let cover = app
            .ui_cover_textures
            .get(index)
            .and_then(|texture| texture.as_ref())
            .map(|texture| texture.id());
        let image = cover.map(|id| egui::Image::new((id, Vec2::splat(56.0))));
        let label = format!("{}\n{}", song.title, song.artist);
        let button = if let Some(image) = image {
            egui::Button::image_and_text(image, RichText::new(label).size(14.0))
        } else {
            egui::Button::new(RichText::new(label).size(14.0))
        };
        let response = ui.add_sized(
            [ui.available_width(), 76.0],
            button
                .fill(if selected {
                    theme::BG_RAISED
                } else {
                    theme::BG_PANEL
                })
                .stroke(Stroke::new(
                    if selected { 2.0_f32 } else { 1.0_f32 },
                    if selected {
                        theme::ACCENT_CYAN
                    } else {
                        theme::BORDER
                    },
                )),
        );
        if response.clicked() && app.player_ui.selected_song != index {
            app.player_ui.selected_song = index;
            if let Err(error) = library::load_song(app, index) {
                app.player_ui.song_error = Some(error);
            }
        } else if response.clicked() && app.player_ui.using_custom_song {
            if let Err(error) = library::load_song(app, index) {
                app.player_ui.song_error = Some(error);
            }
        }
        ui.add_space(8.0);
    }
    ui.add_space(8.0);
    if widgets::command_button(ui, "刷新曲库", widgets::ButtonKind::Quiet).clicked() {
        library::refresh_song_library(app);
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("本地谱面")
            .size(12.0)
            .strong()
            .color(theme::TEXT_MUTED),
    );
    if widgets::command_button(ui, "导入歌曲到曲库", widgets::ButtonKind::Quiet).clicked() {
        app.pending_import = true;
    }
}

fn draw_song_detail(ui: &mut egui::Ui, app: &mut PlayerState) {
    let song = app.song_library.get(app.player_ui.selected_song);
    let title = if app.player_ui.using_custom_song {
        app.chart.title.clone()
    } else {
        song.map(|entry| entry.title.as_str())
            .unwrap_or("未命名歌曲")
            .to_owned()
    };
    let artist = if app.player_ui.using_custom_song {
        app.chart.artist.clone()
    } else {
        song.map(|entry| entry.artist.as_str())
            .unwrap_or("")
            .to_owned()
    };
    let descriptor = if app.player_ui.using_custom_song {
        "本地导入谱面".to_owned()
    } else {
        song.map(|entry| entry.descriptor.as_str())
            .unwrap_or("")
            .to_owned()
    };
    ui.vertical_centered(|ui| {
        let width = ui.available_width().min(320.0);
        let cover = if app.player_ui.using_custom_song {
            app.ui_logo_texture.as_ref()
        } else {
            app.ui_cover_textures
                .get(app.player_ui.selected_song)
                .and_then(|texture| texture.as_ref())
        };
        if let Some(texture) = cover {
            ui.add(
                egui::Image::new((texture.id(), Vec2::splat(width)))
                    .corner_radius(egui::CornerRadius::same(8)),
            );
        }
        ui.add_space(20.0);
        widgets::overline(ui, "NOW SELECTING");
        ui.add_space(4.0);
        widgets::heading(ui, &title, 30.0);
        ui.label(
            RichText::new(&artist)
                .size(15.0)
                .color(theme::TEXT_SECONDARY),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(&descriptor)
                .size(13.0)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(24.0);

        let levels = app.import_levels.clone();
        if levels.is_empty() {
            ui.label(RichText::new("使用当前谱面").color(theme::TEXT_SECONDARY));
        } else {
            ui.horizontal_wrapped(|ui| {
                for (level, label) in levels {
                    let selected = app.import_selected_level == level;
                    let response = ui.add_sized(
                        [110.0, 48.0],
                        egui::Button::new(
                            RichText::new(format!("Lv.{level}  {label}"))
                                .strong()
                                .color(if selected {
                                    theme::BG_VOID
                                } else {
                                    theme::TEXT_PRIMARY
                                }),
                        )
                        .fill(if selected {
                            theme::ACCENT_CYAN
                        } else {
                            theme::BG_RAISED
                        })
                        .stroke(Stroke::new(1.0_f32, theme::BORDER)),
                    );
                    if response.clicked() && !selected {
                        if let Err(error) = library::select_difficulty(app, level) {
                            app.player_ui.song_error = Some(error);
                        }
                    }
                }
            });
        }
        ui.add_space(20.0);
        ui.label(
            RichText::new(format!(
                "{} notes · {:.0} BPM",
                app.chart.notes.len(),
                app.chart.bpm
            ))
            .size(13.0)
            .color(theme::TEXT_SECONDARY),
        );
        ui.add_space(20.0);
        if widgets::command_button(ui, "开始游玩", widgets::ButtonKind::Primary).clicked() {
            if let Err(error) = library::begin_gameplay(app) {
                app.player_ui.song_error = Some(error);
            }
        }
        if let Some(error) = &app.player_ui.song_error {
            ui.add_space(12.0);
            ui.colored_label(theme::ACCENT_CORAL, error);
        }
    });
}
