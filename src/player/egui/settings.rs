use egui_macroquad::egui::{self, Frame, RichText, Stroke};

use crate::state::{PlayerPage, PlayerSettingsSection, PlayerState};

use super::{theme, widgets};

pub fn draw(ctx: &egui::Context, app: &mut PlayerState) {
    let back_label = if app.player_ui.settings_return == PlayerPage::Pause {
        "返回暂停"
    } else {
        "返回"
    };
    if widgets::page_top_bar(ctx, back_label, "设置", |ui| {
        if widgets::compact_button(ui, "完成", widgets::ButtonKind::Primary).clicked() {
            app.player_ui.close_settings();
        }
    }) {
        app.player_ui.close_settings();
        return;
    }

    let narrow = ctx.screen_rect().width() < 760.0;
    if narrow {
        egui::CentralPanel::default().show(ctx, |ui| draw_settings(ui, app));
    } else {
        egui::SidePanel::left("settings_categories")
            .exact_width(240.0)
            .frame(Frame::new().fill(theme::BG_PANEL).inner_margin(20.0))
            .show(ctx, |ui| draw_categories(ui, app));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::BG_VOID).inner_margin(32.0))
            .show(ctx, |ui| draw_settings(ui, app));
    }
}

fn draw_categories(ui: &mut egui::Ui, app: &mut PlayerState) {
    widgets::overline(ui, "PLAYER SETTINGS");
    ui.add_space(12.0);
    for (section, label, description) in [
        (PlayerSettingsSection::Audio, "音频", "音乐与判定音效"),
        (PlayerSettingsSection::Gameplay, "游玩", "速度与辅助"),
        (PlayerSettingsSection::Display, "显示", "布局与视觉"),
    ] {
        let selected = app.player_ui.settings_section == section;
        let response = ui.add_sized(
            [ui.available_width(), 64.0],
            egui::Button::new(
                RichText::new(format!("{label}\n{description}"))
                    .size(14.0)
                    .color(if selected {
                        theme::BG_VOID
                    } else {
                        theme::TEXT_PRIMARY
                    }),
            )
            .fill(if selected {
                theme::ACCENT_CYAN
            } else {
                theme::BG_PANEL
            })
            .stroke(Stroke::new(1.0_f32, theme::BORDER)),
        );
        if response.clicked() {
            app.player_ui.settings_section = section;
        }
        ui.add_space(8.0);
    }
}

fn draw_settings(ui: &mut egui::Ui, app: &mut PlayerState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        widgets::overline(ui, "设置项");
        ui.add_space(6.0);
        widgets::heading(ui, settings_heading(app.player_ui.settings_section), 28.0);
        ui.add_space(24.0);
        match app.player_ui.settings_section {
            PlayerSettingsSection::Audio => draw_audio(ui, app),
            PlayerSettingsSection::Gameplay => draw_gameplay(ui, app),
            PlayerSettingsSection::Display => draw_display(ui, app),
        }
        ui.add_space(24.0);
        ui.separator();
        ui.add_space(16.0);
        if widgets::command_button(ui, "恢复默认设置", widgets::ButtonKind::Quiet).clicked() {
            app.audio_enabled = true;
            app.play_speed = 1.0;
            app.autoplay = true;
            app.waveform_threshold = 0.3;
            app.mobile_ui = false;
            app.ui_scale_override = None;
            app.set_status("已恢复默认设置".to_owned());
        }
    });
}

fn settings_heading(section: PlayerSettingsSection) -> &'static str {
    match section {
        PlayerSettingsSection::Audio => "音频",
        PlayerSettingsSection::Gameplay => "游玩体验",
        PlayerSettingsSection::Display => "显示与布局",
    }
}

fn draw_audio(ui: &mut egui::Ui, app: &mut PlayerState) {
    setting_toggle(
        ui,
        "音乐与判定音效",
        "关闭后游玩仍会继续，但不会播放声音。",
        &mut app.audio_enabled,
    );
}

fn draw_gameplay(ui: &mut egui::Ui, app: &mut PlayerState) {
    ui.label(RichText::new(format!("播放速度  {:.1}x", app.play_speed)).strong());
    ui.add(
        egui::Slider::new(&mut app.play_speed, 0.1..=3.0)
            .step_by(0.1)
            .text("速度"),
    );
    ui.add_space(16.0);
    ui.label(RichText::new(format!("波形阈值  {:.2}", app.waveform_threshold)).strong());
    ui.add(
        egui::Slider::new(&mut app.waveform_threshold, 0.05..=1.0)
            .step_by(0.05)
            .text("阈值"),
    );
    ui.add_space(12.0);
    setting_toggle(
        ui,
        "Slide 自动判定",
        "移动星星经过每个分段时自动完成并隐藏该段。",
        &mut app.autoplay,
    );
    ui.add_space(4.0);
    ui.label(RichText::new("快捷键").strong());
    ui.label(
        RichText::new("1–8 / T 触发触摸区域 · A 自动判定 · Space 播放 / 暂停 · R 重播")
            .color(theme::TEXT_SECONDARY),
    );
}

fn draw_display(ui: &mut egui::Ui, app: &mut PlayerState) {
    let mut scale = app.ui_scale_override.unwrap_or(1.0);
    ui.label(RichText::new(format!("界面缩放  {:.1}x", scale)).strong());
    if ui
        .add(
            egui::Slider::new(&mut scale, 0.8..=1.6)
                .step_by(0.1)
                .text("缩放"),
        )
        .changed()
    {
        app.ui_scale_override = Some(scale);
    }
    setting_toggle(
        ui,
        "移动端布局",
        "使用更大的控件和更紧凑的单列布局。",
        &mut app.mobile_ui,
    );
    ui.add_space(12.0);
    ui.label(RichText::new("当前状态").strong());
    ui.label(RichText::new(&app.status).color(theme::TEXT_SECONDARY));
}

fn setting_toggle(ui: &mut egui::Ui, title: &str, description: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).strong());
            ui.label(
                RichText::new(description)
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(value, "");
        });
    });
    ui.add_space(16.0);
}
