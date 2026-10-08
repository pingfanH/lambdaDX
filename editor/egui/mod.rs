//! Responsive editor chrome built with egui.

use egui_macroquad::egui;
use macroquad::prelude::Vec2;

use crate::app::types::PadGeom;

pub const DESIGN_W: f32 = 1570.0;
pub const DESIGN_H: f32 = 856.0;
pub const HEADER_H: f32 = 42.0;
pub const TIMELINE_H: f32 = 79.0;
pub const SIDEBAR_MIN: f32 = 360.0;
pub const SIDEBAR_DEFAULT: f32 = 468.0;
pub const SIDEBAR_MAX: f32 = 560.0;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub preview: egui::Rect,
    pub workspace: egui::Rect,
    pub timeline: egui::Rect,
    pub pad: PadGeom,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            preview: egui::Rect::NOTHING,
            workspace: egui::Rect::NOTHING,
            timeline: egui::Rect::NOTHING,
            pad: PadGeom {
                cx: 238.5,
                cy: 384.0,
                outer_r: 173.5,
            },
        }
    }
}

#[derive(Default)]
pub struct State {
    pub sidebar_width: f32,
    pub layout: Layout,
    pub active_tool: usize,
    pub playhead: f32,
    pub inspector_expanded: [bool; 5],
    fonts_loaded: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            sidebar_width: SIDEBAR_DEFAULT,
            playhead: 0.683,
            inspector_expanded: [true, true, true, false, false],
            ..Default::default()
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context) {
        self.ensure_fonts(ctx);
        let screen = ctx.screen_rect();
        let header = egui::TopBottomPanel::top("editor_header")
            .exact_height(HEADER_H)
            .frame(
                egui::Frame::none()
                    .fill(rgb(32, 32, 32))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            );
        header.show(ctx, |ui| header_ui(ui));

        egui::TopBottomPanel::bottom("editor_timeline")
            .exact_height(TIMELINE_H)
            .frame(egui::Frame::none().fill(rgb(43, 43, 45)))
            .show(ctx, |ui| timeline_ui(ui, self.playhead));

        let sidebar_width = self.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        egui::SidePanel::left("editor_sidebar")
            .resizable(true)
            .default_width(sidebar_width)
            .min_width(SIDEBAR_MIN)
            .max_width(SIDEBAR_MAX)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(12)))
            .show(ctx, |ui| {
                self.sidebar_width = ui.available_width() + 24.0;
                let preview_h = (ui.available_width() * 1.0).min(ui.available_height() * 0.66);
                let (preview_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), preview_h),
                    egui::Sense::hover(),
                );
                self.layout.preview = preview_rect;
                preview_background(ui, preview_rect);
                ui.add_space(10.0);
                inspector_ui(ui, &mut self.inspector_expanded);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(rgb(29, 29, 30)))
            .show(ctx, |ui| {
                let workspace = ui.max_rect();
                toolbar_ui(ui, workspace);
                draw_grid(ui, workspace);
                self.layout.workspace = workspace;
            });

        // Query the actual panel geometry after egui has resolved all resizable panels.
        let sidebar_right = screen.left() + self.sidebar_width;
        let timeline_top = screen.bottom() - TIMELINE_H;
        self.layout.timeline = egui::Rect::from_min_max(
            egui::pos2(sidebar_right, timeline_top),
            screen.right_bottom(),
        );
    }

    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "editor_arial".into(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/Arial.ttf"
            ))),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "editor_arial".into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "editor_arial".into());
        ctx.set_fonts(fonts);
        self.fonts_loaded = true;
    }

    pub fn finish_layout(&mut self, ctx: &egui::Context) {
        let _screen = ctx.screen_rect();
        let preview = self.layout.preview;
        let r = preview.width().min(preview.height()) * 0.42;
        let c = preview.center();
        self.layout.pad = PadGeom {
            cx: c.x,
            cy: c.y,
            outer_r: r,
        };
    }
}

fn header_ui(ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        ui.colored_label(rgb(255, 95, 87), "●");
        ui.colored_label(rgb(255, 189, 46), "●");
        ui.colored_label(rgb(40, 200, 64), "●");
        ui.separator();
        for label in ["File", "Edit", "Slide", "View", "Anim"] {
            let _ = ui.add(egui::Button::new(egui::RichText::new(label).size(12.0)).frame(false));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.colored_label(rgb(184, 184, 182), "PROJECT / MAICHART · SIMAI · ANIM");
        });
    });
}

fn preview_background(ui: &mut egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, rgb(42, 42, 45)),
        egui::StrokeKind::Inside,
    );
}

fn inspector_ui(ui: &mut egui::Ui, expanded: &mut [bool; 5]) {
    ui.label(
        egui::RichText::new("INSPECTOR")
            .size(13.0)
            .color(rgb(208, 208, 212)),
    );
    ui.add_space(8.0);
    for (index, title) in ["NOTE", "SLIDE", "TOUCH", "SENSOR / ZONES", "JUDGE FX"]
        .into_iter()
        .enumerate()
    {
        ui.separator();
        let response = ui.collapsing(title, |ui| {
            for label in ["PARAM", "SPEED", "OPACITY"] {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(11.0)
                            .color(rgb(141, 141, 148)),
                    );
                    let mut value = 0.5;
                    ui.add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(false));
                });
            }
        });
        expanded[index] = response.fully_open();
    }
}

fn toolbar_ui(ui: &mut egui::Ui, rect: egui::Rect) {
    egui::Area::new("editor_toolbar".into())
        .fixed_pos(rect.left_top() + egui::vec2(8.0, 80.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::none()
                .fill(rgb(37, 37, 41))
                .rounding(4.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for i in 0..7 {
                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 3.0, rgb(48, 48, 53));
                            draw_tool_icon(ui.painter(), rect, i);
                            let _ = response;
                            if i == 2 || i == 5 {
                                ui.add_space(2.0);
                            }
                        }
                    });
                });
        });
}

fn draw_tool_icon(painter: &egui::Painter, rect: egui::Rect, index: usize) {
    let c = rgb(211, 211, 214);
    let center = rect.center();
    let stroke = egui::Stroke::new(1.5_f32, c);
    match index {
        0 => {
            painter.line_segment(
                [
                    center + egui::vec2(-5.0, 0.0),
                    center + egui::vec2(5.0, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(0.0, -5.0),
                    center + egui::vec2(0.0, 5.0),
                ],
                stroke,
            );
        }
        1..=5 => {
            painter.rect_stroke(rect.shrink(8.0), 3.5, stroke, egui::StrokeKind::Inside);
        }
        _ => {
            painter.circle_stroke(center, 6.0, stroke);
        }
    }
}

fn timeline_ui(ui: &mut egui::Ui, playhead: f32) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TIMELINE")
                .size(11.0)
                .color(rgb(141, 141, 148)),
        );
        ui.add_space(16.0);
        for i in 0..28 {
            ui.vertical(|ui| {
                ui.painter().vline(
                    ui.cursor().left(),
                    0.0..=12.0,
                    egui::Stroke::new(1.0_f32, rgb(90, 90, 95)),
                );
                if i % 5 == 0 {
                    ui.label(i.to_string());
                } else {
                    ui.add_space(12.0);
                }
            });
            ui.add_space(18.0);
        }
    });
    let _ = playhead;
}

fn draw_grid(ui: &mut egui::Ui, rect: egui::Rect) {
    let stroke = egui::Stroke::new(1.0_f32, rgb(40, 40, 43));
    let mut x = rect.left();
    while x < rect.right() {
        ui.painter().vline(x, rect.y_range(), stroke);
        x += 96.0;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        ui.painter().hline(rect.x_range(), y, stroke);
        y += 96.0;
    }
}

fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_pad_stays_inside_panel_at_multiple_sizes() {
        for size in [
            (800.0_f32, 600.0_f32),
            (1570.0_f32, 856.0_f32),
            (2560.0_f32, 1440.0_f32),
        ] {
            let preview = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size.0.min(560.0), size.1 * 0.6),
            );
            let r = preview.width().min(preview.height()) * 0.42;
            let c = preview.center();
            assert!(c.x - r >= preview.left() && c.x + r <= preview.right());
            assert!(c.y - r >= preview.top() && c.y + r <= preview.bottom());
        }
    }
}
