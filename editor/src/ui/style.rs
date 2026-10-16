//! egui theme + font setup for the editor.

use egui_macroquad::egui;
use std::sync::Arc;

use crate::theme::*;
use lucide_icons::{Icon, LUCIDE_FONT_BYTES};

/// Install the CJK-capable font and the dark widget styling once per context.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_style(style());
}

pub fn refresh_surfaces(ctx: &egui::Context) {
    ctx.style_mut(|style| surface_visuals(&mut style.visuals));
}

fn surface_visuals(visuals: &mut egui::Visuals) {
    visuals.window_fill = floating_surface();
    visuals.panel_fill = surface(BG_TOOLBAR);
    visuals.extreme_bg_color = surface(BG_TOOLBAR);
    visuals.faint_bg_color = surface(BG_WIDGET);
    for widget in [
        &mut visuals.widgets.inactive, &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.active, &mut visuals.widgets.open,
    ] {
        widget.bg_fill = surface(BG_WIDGET);
    }
    visuals.widgets.hovered.bg_fill = surface(BG_WIDGET_HOVER);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Prefer the same CJK-capable font the F1 params panel uses so opening that
    // panel does not swap the editor's typeface. Fall back to bundled Arial.
    let (name, data) = match crate::player::font::bytes() {
        Some(bytes) => ("editor_cjk", egui::FontData::from_owned(bytes)),
        None => (
            "editor_arial",
            egui::FontData::from_static(include_bytes!("../../../assets/Arial.ttf")),
        ),
    };
    fonts.font_data.insert(name.into(), Arc::new(data));
    let mut code_fonts = fonts.families[&egui::FontFamily::Monospace].clone();
    code_fonts.push(name.into());
    fonts.families.insert(egui::FontFamily::Name("editor_code".into()), code_fonts);
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    fonts.font_data.insert(
        "lucide".into(),
        Arc::new(egui::FontData::from_static(LUCIDE_FONT_BYTES)),
    );
    fonts.families.insert(
        egui::FontFamily::Name("lucide".into()),
        vec!["lucide".into()],
    );
    ctx.set_fonts(fonts);
}

fn style() -> egui::Style {
    let mut style = egui::Style::default();
    for (kind, size) in [
        (egui::TextStyle::Body, 12.0),
        (egui::TextStyle::Button, 12.0),
        (egui::TextStyle::Small, 11.0),
        (egui::TextStyle::Heading, 14.0),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));
    style.spacing.item_spacing = egui::vec2(5.0, 3.0);
    style.spacing.button_padding = egui::vec2(4.0, 2.0);
    style.spacing.interact_size = egui::vec2(22.0, 22.0);
    style.spacing.slider_width = 100.0;

    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(121, 184, 244, 18);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT_BLUE);
    visuals.window_corner_radius = egui::CornerRadius::same(4);
    visuals.window_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(12));
    visuals.window_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: egui::Color32::from_black_alpha(80),
    };
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 3],
        blur: 8,
        spread: 0,
        color: egui::Color32::from_black_alpha(55),
    };
    visuals.window_highlight_topmost = false;

    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(2);
        widget.weak_bg_fill = egui::Color32::TRANSPARENT;
        widget.bg_stroke = egui::Stroke::NONE;
        widget.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    }
    visuals.widgets.inactive.fg_stroke.color = TEXT_DIM;
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_white_alpha(16);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_white_alpha(24);
    visuals.widgets.open.weak_bg_fill = egui::Color32::from_white_alpha(20);
    // Egui uses this color to fade disabled content; transparent hides it entirely.
    visuals.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    surface_visuals(&mut visuals);
    style.visuals = visuals;
    style
}

pub fn glyph(icon: Icon, size: f32) -> egui::RichText {
    egui::RichText::new(char::from(icon).to_string()).font(egui::FontId::new(
        size,
        egui::FontFamily::Name("lucide".into()),
    ))
}

pub fn icon(ui: &mut egui::Ui, symbol: Icon, name: &str, active: bool) -> egui::Response {
    ui.add_sized(
        egui::vec2(22.0, 22.0),
        egui::Button::new(glyph(symbol, 14.0).color(if active { ACCENT_BLUE } else { TEXT_DIM }))
            .stroke(egui::Stroke::NONE),
    )
    .on_hover_text(name)
}

pub fn dock_style(style: &egui::Style) -> egui_dock::Style {
    let mut s = egui_dock::Style::from_egui(style);
    s.dock_area_padding = Some(egui::Margin::ZERO);
    s.main_surface_border_rounding = egui::CornerRadius::ZERO;
    s.main_surface_border_stroke = egui::Stroke::NONE;
    s.tab_bar.height = 26.0;
    s.tab_bar.fill_tab_bar = true;
    // The custom pane header paints the surface once.
    s.tab_bar.bg_fill = egui::Color32::TRANSPARENT;
    s.tab_bar.corner_radius = egui::CornerRadius::ZERO;
    s.tab_bar.hline_color = egui::Color32::TRANSPARENT;
    s.tab.tab_body.inner_margin = egui::Margin::ZERO;
    s.tab.tab_body.bg_fill = egui::Color32::TRANSPARENT;
    s.tab.tab_body.stroke = egui::Stroke::NONE;
    s.tab.tab_body.corner_radius = egui::CornerRadius::ZERO;
    for state in [
        &mut s.tab.active,
        &mut s.tab.focused,
        &mut s.tab.inactive,
        &mut s.tab.hovered,
        &mut s.tab.active_with_kb_focus,
        &mut s.tab.inactive_with_kb_focus,
        &mut s.tab.focused_with_kb_focus,
    ] {
        state.corner_radius = egui::CornerRadius::ZERO;
        state.bg_fill = egui::Color32::TRANSPARENT;
        state.outline_color = egui::Color32::TRANSPARENT;
        state.text_color = TEXT;
    }
    s.separator.width = 3.0;
    s.separator.extra_interact_width = 7.0;
    s.separator.extra = 90.0;
    s.separator.color_idle = surface(BG_WINDOW);
    s.separator.color_hovered = ACCENT_BLUE;
    s.separator.color_dragged = ACCENT_BLUE;
    s
}
