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
use crate::theme::{BG_TOOLBAR, OVERVIEW_H, PADDING, TEXT, TEXT_DIM, TOOLBAR_H};

use layout::Layout;

/// `(keys, action)` rows shown in the `?` shortcut-help overlay.
const SHORTCUTS: &[(&str, &str)] = &[
    ("V / T / H", "Select · Tap · Hold"),
    ("C / S / E", "Touch · Slide · Eraser"),
    ("Cmd+Z / ⇧Cmd+Z", "Undo · Redo"),
    ("Cmd+C / Cmd+X", "Copy · Cut selection"),
    ("Cmd+V", "Paste at playhead"),
    ("← →", "Nudge selection in time (±1 grid)"),
    ("↑ ↓", "Nudge selection one lane"),
    ("Del / Backspace", "Delete selection"),
    ("Right-click", "Delete nearest note"),
    ("B / X / N", "Toggle Break · Ex · both"),
    ("Double/Middle-click a slide", "Slide edit: click pad/lane zones to add a waypoint"),
    ("Q/P/S/Z/V/W + 1-8", "Slide edit: set shape to an end lane (⇧=QQ/PP, ,/./;=</>/^, -=line)"),
    ("⌫ / Esc", "Slide edit: undo waypoint · exit"),
    ("Cmd+wheel", "Zoom timeline"),
    ("Wheel", "Scroll / scrub view"),
    ("P", "Cycle preview mode"),
    ("?", "Toggle this help"),
];

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

        if editor.show_help {
            self.draw_help(ctx, editor);
        }
        if editor.pending_action.is_some() {
            self.draw_confirm(ctx, editor, &mut app.chart);
        }
    }

    /// Unsaved-changes confirmation for a deferred load / open / quit. `Save` and
    /// `Discard` set `pending_confirmed` (the runtime runs the action next frame);
    /// `Cancel` drops it.
    fn draw_confirm(
        &self,
        ctx: &egui::Context,
        editor: &mut EditorState,
        chart: &mut crate::app::types::ChartDoc,
    ) {
        let Some(action) = editor.pending_action else {
            return;
        };
        let what = match action {
            crate::state::PendingAction::Reload => "reload the chart",
            crate::state::PendingAction::Open => "open another chart",
            crate::state::PendingAction::Quit => "quit",
        };
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("Save changes before you {what}?"))
                        .size(crate::theme::FONT_BODY)
                        .color(TEXT),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        crate::io::save_chart(editor, chart);
                        editor.pending_confirmed = true;
                    }
                    if ui.button("Discard").clicked() {
                        editor.dirty = false;
                        editor.pending_confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        editor.pending_action = None;
                    }
                });
            });
    }

    /// The `?` shortcut-help overlay (a small centered window).
    fn draw_help(&self, ctx: &egui::Context, editor: &mut EditorState) {
        let mut open = true;
        egui::Window::new("Keyboard shortcuts")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                for (keys, what) in SHORTCUTS {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [140.0, 16.0],
                            egui::Label::new(
                                egui::RichText::new(*keys).monospace().color(TEXT),
                            ),
                        );
                        ui.label(egui::RichText::new(*what).color(TEXT_DIM));
                    });
                }
            });
        if !open {
            editor.show_help = false;
        }
    }

    /// Floating tool palette, timeline interaction and progress bar (the
    /// timeline column). The palette overlays the timeline at the design's
    /// top-left offset rather than consuming a strip of its own.
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
        let handle_w = layout::PANEL_HANDLE_W;
        let handle = egui::Rect::from_min_max(
            egui::pos2(workspace.right() - handle_w, workspace.top()),
            workspace.right_bottom(),
        );
        let content_right = handle.left();
        self.draw_panel_handle(ui, handle);

        let overview_rect = egui::Rect::from_min_max(
            egui::pos2(workspace.left(), workspace.bottom() - OVERVIEW_H),
            egui::pos2(content_right, workspace.bottom()),
        );
        // The timeline fills the column between the toolbar and the progress bar.
        let timeline_rect = egui::Rect::from_min_max(
            egui::pos2(workspace.left(), workspace.top()),
            egui::pos2(content_right, overview_rect.top()),
        );
        self.layout.timeline = timeline_rect;
        self.layout.overview = overview_rect;

        // Floating palette at the design offset from the timeline's top-left.
        let tools_rect = egui::Rect::from_min_size(
            egui::pos2(
                timeline_rect.left() + tools::PANEL_X,
                timeline_rect.top() + tools::PANEL_Y,
            ),
            egui::vec2(tools::PANEL_W, tools::panel_height(crate::state::Tool::ALL.len())),
        );
        self.layout.tools = tools_rect;

        // Timeline host first (lower z), then the palette on top so it captures
        // the pointer where the two overlap.
        let response = ui.interact(
            timeline_rect,
            ui.id().with("editor_timeline"),
            egui::Sense::click_and_drag(),
        );
        let tools_hovered = tools::draw(ui, tools_rect, editor);

        if !tools_hovered {
            crate::timeline::handle_input(
                editor.layout,
                &response,
                timeline_rect,
                editor,
                chart,
                current_time,
                ui,
            );
        }
        crate::timeline::handle_edit_shortcuts(ui, editor, chart, current_time);
        // Slide-edit mode reuses the letter keys (`Q/P/S/Z/V/W`) for shapes, so
        // suppress the single-key tool shortcuts while it is active.
        if !editor.is_slide_edit() {
            tools::handle_shortcuts(ui, editor);
        }

        // Progress / overview minimap, aligned with the timeline's plot area.
        let plot_left = overview_rect.left() + crate::timeline::LANE_GUTTER;
        crate::timeline::draw_overview(ui, overview_rect, editor, chart, current_time, plot_left);
        let _ = waveform;
    }

    /// The small chevron tab that collapses / expands the right panel (design
    /// shows a 12×28 tab at the panel's mid-height; the whole seam is clickable).
    fn draw_panel_handle(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let response = ui.interact(rect, ui.id().with("editor_panel_handle"), egui::Sense::click());
        let tab = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 6.0, rect.center().y),
            egui::vec2(12.0, 28.0),
        );
        let bg = if response.hovered() {
            crate::theme::BG_WIDGET_HOVER
        } else {
            crate::theme::BG_HANDLE
        };
        ui.painter().rect_filled(tab, 2.0, bg);
        let chevron = if self.sidebar_open { "›" } else { "‹" };
        ui.painter().text(
            tab.center(),
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
