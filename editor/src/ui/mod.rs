//! Editor chrome: egui panels layered over the macroquad pad + timeline.
//!
//! Frame order per tick (see `main.rs`):
//!   1. `EditorUi::draw` lays out egui and records its paint commands.
//!   2. macroquad paints the window background, the pad preview and the timeline
//!      into the rects recorded on [`Layout`].
//!   3. the egui bridge composites the recorded commands on top, so the toolbar,
//!      tools strip, inspector and overview sit above the macroquad layers while
//!      the timeline/preview regions stay transparent for interaction.
//!
//! Individual regions live in their own submodules; this file only wires them
//! together and owns the per-session chrome state.

pub mod inspector;
pub mod layout;
pub mod sidebar;
pub mod style;
pub mod tools;
pub mod toolbar;
pub mod widgets;

use egui_macroquad::egui;

use crate::audio::WaveformPyramid;
use crate::state::EditorState;
use crate::player::state::PadPreviewState;
use crate::theme::{BG_TOOLBAR, OVERVIEW_H, PADDING, TOOLBAR_H, TOOLS_STRIP_H};

use layout::Layout;

/// Chrome-owned state that survives across frames (unrelated to the document).
pub struct EditorUi {
    /// Screen regions computed last frame.
    pub layout: Layout,
    /// Pad preview zoom multiplier.
    pub pad_scale: f32,
    /// Whether the right-hand info panel is shown (design's collapse handle).
    pub sidebar_open: bool,
    /// Collapse state for the inspector sections (Note Properties, Chart Info).
    pub inspector_open: [bool; 2],
    style_applied: bool,
}

impl Default for EditorUi {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            layout: Layout::default(),
            pad_scale: 1.0,
            sidebar_open: true,
            inspector_open: [true, true],
            style_applied: false,
        }
    }

    /// Lay out and paint every egui region for this frame.
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
        current_time: f32,
        waveform: &WaveformPyramid,
    ) {
        if !self.style_applied {
            style::apply(ctx);
            self.style_applied = true;
        }

        let mut stop_audio = false;
        egui::TopBottomPanel::top("editor_toolbar")
            .exact_height(TOOLBAR_H)
            .frame(
                egui::Frame::NONE
                    .fill(BG_TOOLBAR)
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| {
                self.layout.toolbar = ui.max_rect();
                stop_audio = toolbar::draw(ui, editor, &mut app.chart, &mut app.audio_enabled);
            });
        if stop_audio {
            app.stop_audio_if_any();
        }

        let screen_w = ctx.screen_rect().width();
        if self.sidebar_open {
            egui::SidePanel::right("editor_sidebar")
                .resizable(false)
                .exact_width(layout::sidebar_width(screen_w))
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(PADDING as i8)))
                .show(ctx, |ui| {
                    sidebar::draw(ui, &mut self.layout, editor, &mut app.chart, &mut self.inspector_open);
                });
        } else {
            // Collapsed: hide the panel and the pad preview so the timeline (and
            // the macroquad layers) span the whole window.
            self.layout.sidebar = egui::Rect::NOTHING;
            self.layout.preview = egui::Rect::NOTHING;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.central(ui, editor, &mut app.chart, current_time, waveform));
    }

    /// Tools strip, timeline interaction and progress bar (the timeline column).
    fn central(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut EditorState,
        chart: &mut crate::app::types::ChartDoc,
        current_time: f32,
        waveform: &WaveformPyramid,
    ) {
        let workspace = ui.max_rect();

        // Collapse/expand handle on the seam between the timeline and the panel.
        let handle_w = 14.0;
        let handle = egui::Rect::from_min_max(
            egui::pos2(workspace.right() - handle_w, workspace.top()),
            workspace.right_bottom(),
        );
        let content_right = handle.left();
        self.draw_panel_handle(ui, handle);

        let tools_rect = egui::Rect::from_min_size(
            egui::pos2(workspace.left(), workspace.top()),
            egui::vec2(content_right - workspace.left(), TOOLS_STRIP_H),
        );
        let overview_rect = egui::Rect::from_min_max(
            egui::pos2(workspace.left(), workspace.bottom() - OVERVIEW_H),
            egui::pos2(content_right, workspace.bottom()),
        );
        let timeline_rect = egui::Rect::from_min_max(
            egui::pos2(workspace.left(), tools_rect.bottom()),
            egui::pos2(content_right, overview_rect.top()),
        );
        self.layout.tools = tools_rect;
        self.layout.timeline = timeline_rect;
        self.layout.overview = overview_rect;

        ui.painter().rect_filled(tools_rect, 0.0, BG_TOOLBAR);
        tools::draw(ui, tools_rect, editor);

        // Timeline: transparent host for the macroquad renderer + interaction.
        let response = ui.interact(
            timeline_rect,
            ui.id().with("editor_timeline"),
            egui::Sense::click_and_drag(),
        );
        crate::timeline::handle_input(
            editor.layout,
            &response,
            timeline_rect,
            editor,
            chart,
            current_time,
            ui,
        );
        crate::timeline::handle_edit_shortcuts(ui, editor, chart, current_time);
        tools::handle_shortcuts(ui, editor);

        // Progress / overview minimap, aligned with the timeline's plot area.
        let plot_left = overview_rect.left() + crate::timeline::LANE_GUTTER;
        crate::timeline::draw_overview(ui, overview_rect, editor, chart, current_time, plot_left);
        let _ = waveform;
    }

    /// The thin chevron strip that collapses / expands the right panel.
    fn draw_panel_handle(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let response = ui.interact(rect, ui.id().with("editor_panel_handle"), egui::Sense::click());
        let bg = if response.hovered() {
            crate::theme::BG_WIDGET_HOVER
        } else {
            crate::theme::BG_TOOLBAR
        };
        ui.painter().rect_filled(rect, 0.0, bg);
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            egui::Stroke::new(1.0_f32, crate::theme::BORDER_LIGHT),
        );
        let chevron = if self.sidebar_open { "›" } else { "‹" };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            chevron,
            egui::FontId::proportional(14.0),
            crate::theme::TEXT_DIM,
        );
        if response.clicked() {
            self.sidebar_open = !self.sidebar_open;
        }
    }

    /// Recompute the live pad geometry from the preview region (call once the
    /// layout for the frame is known). Skipped while the panel is collapsed so
    /// the pad radius does not collapse to zero.
    pub fn finish_layout(&mut self) {
        if self.layout.preview.width() > 1.0 && self.layout.preview.height() > 1.0 {
            self.layout.update_pad(self.pad_scale);
        }
    }
}
