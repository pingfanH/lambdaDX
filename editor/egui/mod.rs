//! Responsive editor chrome built with egui.

use egui_macroquad::egui;
use macroquad::prelude::Vec2;

use crate::app::types::PadGeom;
use crate::editor_state::EditorLayout;

pub const DESIGN_W: f32 = 1570.0;
pub const DESIGN_H: f32 = 856.0;
pub const HEADER_H: f32 = 42.0;
/// Width of the vertical tool column docked at the left of the central area.
pub const TOOLBAR_W: f32 = 52.0;
pub const SIDEBAR_MIN: f32 = 360.0;
pub const SIDEBAR_DEFAULT: f32 = 468.0;
pub const SIDEBAR_MAX: f32 = 560.0;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub preview: egui::Rect,
    pub workspace: egui::Rect,
    pub timeline: egui::Rect,
    /// Screen rect of the pad/inspector sidebar (left in `Horizontal`, right in
    /// `Vertical`). Used by `editor_main` to paint the panel background.
    pub sidebar: egui::Rect,
    pub pad: PadGeom,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            preview: egui::Rect::NOTHING,
            workspace: egui::Rect::NOTHING,
            timeline: egui::Rect::NOTHING,
            sidebar: egui::Rect::NOTHING,
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
    /// Multiplies the pad preview radius (player size).
    pub pad_scale: f32,
    /// Fraction of the central area height used by the timeline.
    pub timeline_frac: f32,
    fonts_loaded: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            sidebar_width: SIDEBAR_DEFAULT,
            playhead: 0.683,
            inspector_expanded: [true, true, true, false, false],
            pad_scale: 1.0,
            timeline_frac: 1.0,
            ..Default::default()
        }
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        editor: &mut crate::editor_state::EditorState,
        chart: &mut crate::app::types::ChartDoc,
        current_time: f32,
        _waveform: &crate::editor_audio::WaveformPyramid,
    ) {
        self.ensure_fonts(ctx);

        egui::TopBottomPanel::top("editor_header")
            .exact_height(HEADER_H)
            .frame(
                egui::Frame::NONE
                    .fill(rgb(32, 32, 32))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| header_ui(ui, editor));

        let screen_w = ctx.screen_rect().width();
        let vertical = editor.layout == EditorLayout::Vertical;

        // Sidebar (pad preview + inspector): left in the horizontal layout,
        // right in the vertical layout ("info on the right"). In the vertical
        // layout its width is responsive (a third of the usable width) so the
        // timeline stays exactly twice as wide as the info panel, and the ratio
        // holds as the window resizes.
        let sidebar_frame = egui::Frame::NONE.inner_margin(egui::Margin::same(12));
        if vertical {
            let side = vertical_sidebar_width(screen_w);
            egui::SidePanel::right("editor_sidebar")
                .resizable(false)
                .exact_width(side)
                .frame(sidebar_frame)
                .show(ctx, |ui| {
                    self.layout.sidebar = ui.max_rect();
                    self.sidebar_contents(ui, false);
                });
        } else {
            let sidebar_width = self.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
            egui::SidePanel::left("editor_sidebar")
                .resizable(true)
                .default_width(sidebar_width)
                .min_width(SIDEBAR_MIN)
                .max_width(SIDEBAR_MAX)
                .frame(sidebar_frame)
                .show(ctx, |ui| {
                    self.layout.sidebar = ui.max_rect();
                    self.sidebar_contents(ui, true);
                });
        }

        // Bottom overview: added after the sidebar so it spans the same x-range
        // as the timeline (time head/tail stay aligned) in both layouts.
        egui::TopBottomPanel::bottom("editor_overview")
            .exact_height(crate::editor_timeline::OVERVIEW_HEIGHT)
            .frame(egui::Frame::NONE.fill(rgb(43, 43, 45)))
            .show(ctx, |ui| {
                crate::editor_timeline::draw_overview(ui, editor, chart, current_time);
            });

        // Central: vertical tool column on the left, timeline to its right. The
        // timeline occupies `timeline_frac` of the central height (bottom
        // aligned, just above the overview); the rest is spare workspace.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let workspace = ui.max_rect();
                self.layout.workspace = workspace;
                let toolbar_rect = egui::Rect::from_min_max(
                    workspace.left_top(),
                    egui::pos2(workspace.left() + TOOLBAR_W, workspace.bottom()),
                );
                let tl_h = workspace.height() * self.timeline_frac.clamp(0.25, 1.0);
                let timeline_rect = egui::Rect::from_min_max(
                    egui::pos2(workspace.left() + TOOLBAR_W, workspace.bottom() - tl_h),
                    workspace.right_bottom(),
                );
                toolbar_ui(ui, toolbar_rect, editor);
                self.layout.timeline = timeline_rect;
                let response = ui.interact(
                    timeline_rect,
                    ui.id().with("editor_timeline"),
                    egui::Sense::click_and_drag(),
                );
                match editor.layout {
                    EditorLayout::Horizontal => crate::editor_timeline::handle_input(
                        &response,
                        timeline_rect,
                        editor,
                        chart,
                        current_time,
                        ui,
                    ),
                    EditorLayout::Vertical => crate::editor_timeline::handle_input_vertical(
                        &response,
                        timeline_rect,
                        editor,
                        chart,
                        current_time,
                        ui,
                    ),
                }
            });
    }

    /// Pad preview + inspector contents, shared by the left/right side panels.
    /// `record_width` remembers the panel width for the (resizable) horizontal
    /// layout; the vertical layout is responsive and leaves it alone.
    fn sidebar_contents(&mut self, ui: &mut egui::Ui, record_width: bool) {
        if record_width {
            self.sidebar_width = ui.available_width() + 24.0;
        }
        let preview_h = (ui.available_width() * 1.0).min(ui.available_height() * 0.66);
        let (preview_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), preview_h),
            egui::Sense::hover(),
        );
        self.layout.preview = preview_rect;
        preview_background(ui, preview_rect);
        ui.add_space(10.0);
        inspector_ui(
            ui,
            &mut self.inspector_expanded,
            &mut self.pad_scale,
            &mut self.timeline_frac,
        );
    }

    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }
        let mut fonts = egui::FontDefinitions::default();
        // Prefer the same CJK-capable font the F1 params panel uses, so opening
        // that panel does not swap the editor's typeface. Fall back to the
        // bundled Arial when no system CJK font is available.
        if let Some(bytes) = crate::player::font::bytes() {
            fonts.font_data.insert(
                "editor_cjk".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "editor_cjk".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("editor_cjk".into());
        } else {
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
        }
        ctx.set_fonts(fonts);
        self.fonts_loaded = true;
    }

    pub fn finish_layout(&mut self, ctx: &egui::Context) {
        let _screen = ctx.screen_rect();
        let preview = self.layout.preview;
        let r = preview.width().min(preview.height()) * 0.42 * self.pad_scale.clamp(0.2, 3.0);
        let c = preview.center();
        self.layout.pad = PadGeom {
            cx: c.x,
            cy: c.y,
            outer_r: r,
        };
    }
}

/// Width of the vertical-layout info panel: a third of the usable width
/// (toolbar excluded), so the timeline (the other two thirds) is exactly twice
/// as wide. Never narrower than [`SIDEBAR_MIN`].
pub fn vertical_sidebar_width(screen_w: f32) -> f32 {
    ((screen_w - TOOLBAR_W) / 3.0).max(SIDEBAR_MIN)
}

fn header_ui(ui: &mut egui::Ui, editor: &mut crate::editor_state::EditorState) {
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
            let label = match editor.autoplay_mode % 3 {
                0 => "Autoplay · no judge",
                1 => "Manual · judge",
                _ => "Autoplay · judge",
            };
            let text = egui::RichText::new(format!("MODE: {label}  (P)"))
                .size(12.0)
                .color(rgb(255, 202, 57));
            if ui.add(egui::Button::new(text).frame(false)).clicked() {
                editor.autoplay_mode = (editor.autoplay_mode + 1) % 3;
                editor.mode_dirty = true;
            }
            ui.separator();
            let (layout_text, layout_color) = match editor.layout {
                EditorLayout::Horizontal => ("LAYOUT: ⿰ HORIZONTAL", rgb(150, 200, 255)),
                EditorLayout::Vertical => ("LAYOUT: ⿲ VERTICAL", rgb(150, 255, 190)),
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(layout_text).size(12.0).color(layout_color),
                    )
                    .frame(false),
                )
                .clicked()
            {
                editor.layout = editor.layout.toggled();
            }
            ui.separator();
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

fn inspector_ui(
    ui: &mut egui::Ui,
    expanded: &mut [bool; 5],
    pad_scale: &mut f32,
    timeline_frac: &mut f32,
) {
    ui.label(
        egui::RichText::new("EDITOR")
            .size(13.0)
            .color(rgb(208, 208, 212)),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("PLAYER").size(11.0).color(rgb(141, 141, 148)));
        ui.add(egui::Slider::new(pad_scale, 0.3..=2.5).show_value(true));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("TIMELINE").size(11.0).color(rgb(141, 141, 148)));
        ui.add(egui::Slider::new(timeline_frac, 0.25..=1.0).show_value(true));
    });
    ui.add_space(10.0);
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

fn toolbar_ui(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut crate::editor_state::EditorState) {
    egui::Area::new("editor_toolbar".into())
        .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(rgb(37, 37, 41))
                .corner_radius(4)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for i in 0..6 {
                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
                            let tool = match i {
                                0 => crate::editor_state::Tool::Select,
                                1 => crate::editor_state::Tool::Tap,
                                2 => crate::editor_state::Tool::Hold,
                                3 => crate::editor_state::Tool::Touch,
                                4 => crate::editor_state::Tool::Slide,
                                _ => crate::editor_state::Tool::Eraser,
                            };
                            let fill = if editor.tool == tool {
                                rgb(71, 125, 173)
                            } else {
                                rgb(48, 48, 53)
                            };
                            ui.painter().rect_filled(rect, 3.0, fill);
                            draw_tool_icon(ui.painter(), rect, i);
                            if response.clicked() {
                                editor.tool = tool;
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

fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_layout_keeps_timeline_twice_the_sidebar() {
        for w in [1600.0_f32, 1920.0, 2560.0, 3840.0] {
            let side = vertical_sidebar_width(w);
            let timeline = w - side - TOOLBAR_W;
            assert!(
                (timeline - 2.0 * side).abs() < 0.01,
                "w={w} side={side} timeline={timeline}"
            );
        }
        // Tiny windows fall back to the minimum sidebar width.
        assert_eq!(vertical_sidebar_width(600.0), SIDEBAR_MIN);
    }

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
