use egui_macroquad::egui::{self, *};
use lucide_icons::{Icon, LUCIDE_FONT_BYTES};
use std::sync::Arc;

pub const WINDOW: Color32 = Color32::from_rgb(30, 31, 32);
pub const CANVAS: Color32 = Color32::from_rgb(35, 36, 37);
pub const PANEL: Color32 = Color32::from_rgb(38, 39, 40);
pub const CONTROL: Color32 = Color32::from_rgb(48, 49, 50);
pub const BORDER: Color32 = Color32::from_rgb(62, 63, 64);
pub const TEXT: Color32 = Color32::from_rgb(227, 230, 232);
pub const MUTED: Color32 = Color32::from_rgb(172, 178, 184);
pub const BLUE: Color32 = Color32::from_rgb(121, 184, 244);
pub const GOLD: Color32 = Color32::from_rgb(229, 195, 122);
pub const RED: Color32 = Color32::from_rgb(241, 109, 122);
pub const GREEN: Color32 = Color32::from_rgb(107, 200, 175);
pub const PINK: Color32 = Color32::from_rgb(231, 138, 187);

pub fn glass(translucent: bool) -> Color32 {
    Color32::from_rgba_unmultiplied(35, 36, 37, if translucent { 208 } else { 255 })
}

pub fn glass_edge() -> Stroke {
    Stroke::new(1.0_f32, Color32::from_white_alpha(4))
}

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "lucide".into(),
        Arc::new(FontData::from_static(LUCIDE_FONT_BYTES)),
    );
    fonts
        .families
        .insert(FontFamily::Name("lucide".into()), vec!["lucide".into()]);
    ctx.set_fonts(fonts);
    let mut style = Style::default();
    for (kind, size) in [
        (TextStyle::Body, 13.0),
        (TextStyle::Button, 12.0),
        (TextStyle::Small, 11.0),
        (TextStyle::Heading, 16.0),
    ] {
        style.text_styles.insert(kind, FontId::proportional(size));
    }
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));
    style.spacing.item_spacing = vec2(5.0, 4.0);
    style.spacing.button_padding = vec2(4.0, 2.0);
    style.spacing.interact_size = vec2(22.0, 22.0);
    style.spacing.slider_width = 110.0;
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = WINDOW;
    style.visuals.faint_bg_color = CONTROL;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_white_alpha(4);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, BLUE);
    style.visuals.window_corner_radius = CornerRadius::same(4);
    style.visuals.window_stroke = glass_edge();
    style.visuals.window_highlight_topmost = false;
    for w in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(2);
        w.bg_fill = CONTROL;
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0_f32, TEXT);
    }
    style.visuals.widgets.inactive.fg_stroke.color = MUTED;
    style.visuals.widgets.hovered.bg_fill = Color32::from_gray(52);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(4);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_white_alpha(8);
    style.visuals.widgets.open.weak_bg_fill = Color32::from_white_alpha(6);
    style.visuals.widgets.noninteractive.bg_stroke = glass_edge();
    ctx.set_style(style);
}

pub fn glyph(icon: Icon, size: f32) -> RichText {
    RichText::new(char::from(icon).to_string())
        .font(FontId::new(size, FontFamily::Name("lucide".into())))
}

pub fn icon(ui: &mut Ui, symbol: Icon, name: &str, active: bool) -> Response {
    ui.add_sized(
        vec2(24.0, 24.0),
        Button::new(glyph(symbol, 15.0).color(if active { BLUE } else { MUTED }))
            .stroke(Stroke::NONE),
    )
    .on_hover_text(name)
}

pub fn text(p: &Painter, at: Pos2, align: Align2, s: impl ToString, size: f32, color: Color32) {
    p.text(at, align, s, FontId::proportional(size), color);
}

pub fn dock_style(style: &egui::Style, translucent: bool) -> egui_dock::Style {
    let mut s = egui_dock::Style::from_egui(style);
    s.dock_area_padding = Some(Margin::ZERO);
    s.main_surface_border_rounding = CornerRadius::ZERO;
    s.main_surface_border_stroke = Stroke::NONE;
    s.tab_bar.height = 26.0;
    s.tab_bar.fill_tab_bar = true;
    s.tab_bar.bg_fill = glass(translucent);
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    s.tab_bar.hline_color = Color32::TRANSPARENT;
    s.tab.tab_body.inner_margin = Margin::ZERO;
    s.tab.tab_body.bg_fill = CANVAS;
    s.tab.tab_body.stroke = Stroke::NONE;
    s.tab.tab_body.corner_radius = CornerRadius::ZERO;
    for state in [
        &mut s.tab.active,
        &mut s.tab.focused,
        &mut s.tab.inactive,
        &mut s.tab.hovered,
        &mut s.tab.active_with_kb_focus,
        &mut s.tab.inactive_with_kb_focus,
        &mut s.tab.focused_with_kb_focus,
    ] {
        state.corner_radius = CornerRadius::ZERO;
        state.bg_fill = Color32::TRANSPARENT;
        state.outline_color = Color32::TRANSPARENT;
        state.text_color = TEXT;
    }
    s.tab.inactive.text_color = MUTED;
    s.separator.width = 2.0;
    s.separator.extra_interact_width = 7.0;
    s.separator.extra = 110.0;
    s.separator.color_idle = WINDOW;
    s.separator.color_hovered = BLUE;
    s.separator.color_dragged = BLUE;
    s.overlay.overlay_type = egui_dock::OverlayType::HighlightedAreas;
    s.overlay.selection_color = BLUE.gamma_multiply(0.22);
    s
}

pub struct Assets {
    pub tap: TextureHandle,
    pub each: TextureHandle,
    pub hold: TextureHandle,
    pub star: TextureHandle,
    pub touch: TextureHandle,
    pub slide: TextureHandle,
}

impl Assets {
    pub fn new(ctx: &Context) -> Self {
        fn load(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
            let image = image::load_from_memory(bytes)
                .expect("bundled note image")
                .to_rgba8();
            ctx.load_texture(
                name,
                ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                TextureOptions::LINEAR,
            )
        }
        Self {
            tap: load(
                ctx,
                "tap",
                include_bytes!("../../assets/Skins/classic/tap.png"),
            ),
            each: load(
                ctx,
                "each",
                include_bytes!("../../assets/Skins/classic/tap_each.png"),
            ),
            hold: load(
                ctx,
                "hold",
                include_bytes!("../../assets/Skins/classic/hold.png"),
            ),
            star: load(
                ctx,
                "star",
                include_bytes!("../../assets/Skins/classic/star.png"),
            ),
            touch: load(
                ctx,
                "touch",
                include_bytes!("../../assets/Skins/classic/touch_point.png"),
            ),
            slide: load(
                ctx,
                "slide",
                include_bytes!("../../assets/Skins/classic/slide.png"),
            ),
        }
    }
}

pub fn sprite(p: &Painter, texture: &TextureHandle, rect: Rect, opacity: f32) {
    p.image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        Color32::WHITE.gamma_multiply(opacity),
    );
}
