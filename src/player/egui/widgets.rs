use egui_macroquad::egui::{
    self, Color32, FontFamily, FontId, Response, RichText, Stroke, Ui, Vec2,
};

use super::theme::{
    ACCENT_CORAL, ACCENT_CYAN, BG_RAISED, BG_VOID, BORDER, RADIUS_CONTROL, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, Copy)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Quiet,
    Danger,
}

pub fn command_button(ui: &mut Ui, label: &str, kind: ButtonKind) -> Response {
    let (fill, text, stroke) = match kind {
        ButtonKind::Primary => (ACCENT_CYAN, BG_VOID, Stroke::new(1.0_f32, ACCENT_CYAN)),
        ButtonKind::Secondary => (BG_RAISED, TEXT_PRIMARY, Stroke::new(1.0_f32, BORDER)),
        ButtonKind::Quiet => (
            Color32::TRANSPARENT,
            TEXT_SECONDARY,
            Stroke::new(1.0_f32, BORDER),
        ),
        ButtonKind::Danger => (ACCENT_CORAL, BG_VOID, Stroke::new(1.0_f32, ACCENT_CORAL)),
    };
    ui.add_sized(
        Vec2::new(ui.available_width(), 48.0),
        egui::Button::new(RichText::new(label).strong().color(text))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(RADIUS_CONTROL),
    )
}

pub fn compact_button(ui: &mut Ui, label: &str, kind: ButtonKind) -> Response {
    let (fill, text) = match kind {
        ButtonKind::Primary => (ACCENT_CYAN, BG_VOID),
        ButtonKind::Secondary => (BG_RAISED, TEXT_PRIMARY),
        ButtonKind::Quiet => (Color32::TRANSPARENT, TEXT_SECONDARY),
        ButtonKind::Danger => (ACCENT_CORAL, BG_VOID),
    };
    ui.add_sized(
        [96.0, 44.0],
        egui::Button::new(RichText::new(label).strong().color(text))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(RADIUS_CONTROL),
    )
}

pub fn overline(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(11.0, FontFamily::Proportional))
            .strong()
            .color(ACCENT_CYAN),
    );
}

pub fn heading(ui: &mut Ui, text: &str, size: f32) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(size, FontFamily::Proportional))
            .strong()
            .color(TEXT_PRIMARY),
    );
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(TEXT_MUTED).size(13.0));
}

pub fn page_top_bar(
    ctx: &egui::Context,
    back_label: &str,
    title: &str,
    mut trailing: impl FnMut(&mut Ui),
) -> bool {
    let mut go_back = false;
    egui::TopBottomPanel::top("player_page_top_bar")
        .exact_height(68.0)
        .frame(
            egui::Frame::new()
                .fill(BG_VOID)
                .inner_margin(egui::Margin::symmetric(24, 10))
                .stroke(Stroke::new(1.0_f32, BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if compact_button(ui, back_label, ButtonKind::Quiet).clicked() {
                    go_back = true;
                }
                ui.add_space(8.0);
                ui.label(RichText::new(title).size(20.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    trailing(ui);
                });
            });
        });
    go_back
}
