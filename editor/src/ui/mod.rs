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
pub mod toolbar;
pub mod templates;
pub mod tools;
pub mod widgets;

#[cfg(test)]
mod tests;

use egui_macroquad::egui;

use crate::audio::WaveformPyramid;
use crate::player::state::PadPreviewState;
use crate::state::{EditorState, Workspace};
use crate::theme::{BG_TOOLBAR, OVERVIEW_H, PADDING, TEXT, TEXT_DIM, TOOLBAR_H};

use layout::Layout;

/// `(keys, action)` rows shown in the `?` shortcut-help overlay.
const SHORTCUTS: &[(&str, &str)] = &[
    ("F1 / Cmd+,", "Settings"),
    ("F2 / Shift+F2", "Select candidate / toggle candidate selection"),
    ("Cmd+Enter / ⇧Cmd+Enter", "Start box selection / additive box; Enter confirms, Esc cancels"),
    ("F3 / F4", "Commands / tools"),
    ("F5 / F7", "Properties / Hold and Slide handles"),
    ("V / T / H", "Select · Tap · Hold"),
    ("C / S / E", "Touch · Slide · Eraser"),
    ("Cmd+Z / ⇧Cmd+Z", "Undo · Redo"),
    ("Cmd+C / Cmd+X", "Copy · Cut selection"),
    ("Cmd+V", "Paste at edit cursor"),
    ("Cmd+O / Cmd+S", "Open chart / save to the loaded file"),
    ("Shift+Cmd+S / Shift+Cmd+E", "Save As / export Simai"),
    ("Shift+D", "Duplicate selection and move"),
    ("Shift+R", "Repeat last transform or latest two compatible placements"),
    ("Arrows / Cmd+Arrows", "Move cursor and playback; scroll at view edge; hold two axes for diagonals"),
    ("Shift+Arrows / 1-8", "Move one measure or four lanes / set lane"),
    ("Tab / Shift+Tab", "Next / previous note; hold to repeat (trajectory in Slide editing)"),
    ("Enter", "Place note / confirm Hold tail or Star stage"),
    ("J / F8 / [ / ]", "Jump cursor and playback / snap / grid division"),
    ("Shift+click/drag", "Add to selection"),
    ("Cmd+click/drag", "Toggle note / subtract from selection"),
    ("Cmd+A / ⇧Cmd+A", "Select all / clear selection"),
    (
        "G / S / R",
        "Move / time scale / rotate with pointer, wheel or arrow keys",
    ),
    (
        "Enter / click / Esc",
        "Confirm / confirm / cancel transform",
    ),
    ("E (slide selected)", "Toggle slide trajectory editing"),
    ("D / Del / Backspace", "Delete note at keyboard cursor, or mouse selection"),
    ("Cmd+Del / Cmd+Backspace", "Delete all selected notes"),
    ("Right-click", "Delete nearest note"),
    ("B / X / N", "Toggle Break · Ex · both"),
    ("Star placement", "Head / motion-start beat / tail"),
    (
        "Double/Middle-click trail",
        "Slide edit: click pad/lane zones to add a waypoint",
    ),
    (
        "Q/P/S/Z/V/W + 1-8",
        "Slide edit: set shape to an end lane (⇧=QQ/PP, ,/./;=</>/^, -=line)",
    ),
    ("⌫ / Esc", "Slide edit: undo waypoint · exit"),
    ("Cmd+wheel", "Zoom timeline"),
    ("Wheel", "Scroll / scrub view"),
    ("P / O", "Cycle preview mode / toggle autoplay"),
    ("Space / Alt+Space", "Play-pause / restart playback"),
    ("Alt+Enter / Alt+Home", "Seek to edit cursor / seek to start"),
    ("Alt+Left / Right", "Seek backward / forward one second"),
    ("F12 / ?", "Toggle this help"),
];

const AUTOPLAY_SHORTCUTS: &[(&str, &str)] = &[
    ("Space", "Play / pause"),
    ("Cmd+Z / ⇧Cmd+Z", "Undo / redo event edits"),
    ("Cmd+O / Cmd+S", "Open / save chart and authored events"),
    ("⇧Cmd+S / ⇧Cmd+E", "Save As / export Simai"),
    ("Click / drag event", "Select / move event in its track"),
    ("Del / Backspace", "Delete selected event"),
    ("Esc", "Cancel unfinished event edit / clear event selection"),
    ("F8", "Toggle grid snapping"),
    ("Click / drag ruler", "Seek playback"),
    ("Horizontal scroll", "Pan event timeline"),
    ("F1 / Cmd+,", "Settings"),
    ("F12 / ?", "Toggle this help"),
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
    tool_textures: [Option<egui::TextureHandle>; 6],
    menu_open: bool,
    /// Chrome owns this frame's keys and pad/timeline pointer input.
    pub input_blocked: bool,
    pub templates: templates::TemplateUi,
    document_revision: Option<u64>,
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
            tool_textures: [None, None, None, None, None, None],
            menu_open: false,
            input_blocked: false,
            templates: Default::default(),
            document_revision: None,
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
        raw_wheel: f32,
    ) {
        if self.document_revision != Some(app.document_revision()) {
            self.document_revision = Some(app.document_revision());
            self.templates = Default::default();
            editor.autoplay_editor = Default::default();
        }
        crate::player::params_panel::ensure_egui_font(ctx);
        let overlay_was_open = app.show_params || editor.show_help || editor.pending_action.is_some()
            || editor.keyboard.dialog.is_some();
        let canvas_tab = !overlay_was_open && !self.menu_open && !editor.keyboard.ime
            && !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open())
            && ctx.input(|i| !i.pointer.any_pressed() && i.events.iter().any(|event| matches!(event,
                egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                    if !modifiers.alt && !modifiers.ctrl && !modifiers.command)));
        let settings_key = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)
            || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma));
        if settings_key && !editor.keyboard.owns_input() && editor.selection_transform.is_none()
            && !editor.autoplay_editor.dragging() && !self.templates.dragging() {
            app.show_params = !app.show_params;
        }
        if app.show_params && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            app.show_params = false;
        }
        for (slot, texture) in self.tool_textures.iter_mut().zip([
            app.tap_texture.as_ref(),
            app.hold_texture.as_ref(),
            app.star_tex.as_ref(),
            app.touch_tri_tex.as_ref(),
            app.touch_point_tex.as_ref(),
            app.tap_break_tex.as_ref(),
        ]) {
            if slot.is_none() {
                if let Some(texture) = texture {
                    let data = texture.get_texture_data();
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [data.width as usize, data.height as usize],
                        &data.bytes,
                    );
                    *slot = Some(ctx.load_texture(
                        format!("tool-{:?}", texture.raw_miniquad_id()),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
        if !self.style_applied {
            style::apply(ctx);
            self.style_applied = true;
        }

        let was_menu_open = self.menu_open;
        self.menu_open = false;
        let mut workspace = None;
        egui::TopBottomPanel::top("editor_toolbar")
            .exact_height(TOOLBAR_H)
            .frame(
                egui::Frame::NONE
                    .fill(BG_TOOLBAR)
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ctx, |ui| {
                self.layout.toolbar = ui.max_rect();
                let result = ui
                    .add_enabled_ui(editor.selection_transform.is_none() && !editor.keyboard.owns_input() && !overlay_was_open
                        && !editor.autoplay_editor.dragging() && !self.templates.dragging(), |ui| {
                        toolbar::draw(ui, editor, app, &mut self.sidebar_open)
                    })
                    .inner;
                self.menu_open = result.0;
                workspace = result.1;
            });
        if let Some(workspace) = workspace {
            crate::templates::cancel_interaction(editor, &mut app.chart);
            if editor.template_session.is_some() { crate::templates::apply(editor, app, crate::templates::Action::Exit); }
            editor.autoplay_editor.cancel();
            editor.template_action = None;
            self.templates = Default::default();
            editor.workspace = workspace;
            if workspace == Workspace::Autoplay && editor.autoplay_mode < 2 {
                editor.autoplay_mode = 2; editor.mode_dirty = true;
            }
        }
        crate::player::params_panel::draw_embedded_with(ctx, app, |ui| {
            toolbar::draw_editor_settings(ui, editor, &mut self.pad_scale, &mut self.sidebar_open);
        });
        let workspace_blocked = overlay_was_open || app.show_params || editor.show_help
            || editor.pending_action.is_some() || was_menu_open || self.menu_open;
        self.input_blocked = workspace_blocked || ctx.memory(|m| m.any_popup_open());

        if editor.workspace == Workspace::Chart {
        self.templates.panel(ctx, editor, &app.chart, !self.input_blocked && editor.selection_transform.is_none()
            && !editor.keyboard.owns_input() && !self.templates.dragging());
        let screen_w = ctx.available_rect().width();
        if self.sidebar_open {
            egui::SidePanel::right("editor_sidebar")
                .resizable(false)
                .exact_width(layout::sidebar_width(screen_w))
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(PADDING as i8)))
                .show(ctx, |ui| {
                    ui.add_enabled_ui(editor.selection_transform.is_none() && !editor.keyboard.owns_input() && !overlay_was_open && !app.show_params && !editor.show_help && editor.pending_action.is_none(), |ui| {
                        sidebar::draw(
                            ui,
                            &mut self.layout,
                            editor,
                            &mut app.chart,
                            &mut self.inspector_open,
                        );
                    });
                });
        } else {
            // Collapsed: hide the panel and the pad preview so the timeline (and
            // the macroquad layers) span the whole window.
            self.layout.sidebar = egui::Rect::NOTHING;
            self.layout.preview = egui::Rect::NOTHING;
        }

        // Egui assigns an unowned Tab to the first chrome widget during layout.
        // Return only that implicit focus to the canvas; existing fields keep Tab.
        if canvas_tab && !self.input_blocked {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() { m.surrender_focus(id); }
            });
        }
        let duration = app.song_duration();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.central(ui, editor, &mut app.chart, current_time, waveform, duration, raw_wheel)
            });
        crate::keyboard::panels::draw(ctx, editor, &mut app.chart);
        crate::keyboard::apply_transport(editor, app);
        } else {
            crate::keyboard::handle_modals(ctx, editor, &mut app.chart);
            crate::autoplay_editor::draw(ctx, editor, app, &mut self.layout, !workspace_blocked, raw_wheel);
        }
        self.input_blocked |= editor.keyboard.dialog.is_some();

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
                        editor.pending_confirmed = !editor.dirty;
                    }
                    if ui.button("Discard").clicked() {
                        editor.pending_confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        editor.cancel_file_action();
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
                egui::ScrollArea::vertical()
                    .max_height((ctx.screen_rect().height() - 100.0).max(120.0))
                    .show(ui, |ui| {
                        let shortcuts = if editor.workspace == Workspace::Autoplay { AUTOPLAY_SHORTCUTS } else { SHORTCUTS };
                        for (keys, what) in shortcuts {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [170.0, 16.0],
                                    egui::Label::new(egui::RichText::new(*keys).monospace().color(TEXT)),
                                );
                                ui.label(egui::RichText::new(*what).color(TEXT_DIM));
                            });
                        }
                    });
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
        duration: f32,
        raw_wheel: f32,
    ) {
        let workspace = ui.max_rect();

        // Collapse/expand handle on the seam between the timeline and the panel.
        let handle_w = layout::PANEL_HANDLE_W;
        let handle = egui::Rect::from_min_max(
            egui::pos2(workspace.right() - handle_w, workspace.top()),
            workspace.right_bottom(),
        );
        let content_right = handle.left();
        self.draw_panel_handle(ui, handle, !self.input_blocked);

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
        editor.keyboard.rect = timeline_rect;

        // Floating palette at the design offset from the timeline's top-left.
        let tools_rect = egui::Rect::from_min_size(
            egui::pos2(
                timeline_rect.left() + tools::PANEL_X,
                timeline_rect.top() + tools::PANEL_Y,
            ),
            egui::vec2(
                tools::PANEL_W,
                tools::panel_height(crate::state::Tool::ALL.len()),
            ),
        );
        self.layout.tools = tools_rect;
        let template_was_dragging = self.templates.dragging();
        let template_owned = self.templates.timeline(ui, timeline_rect, tools_rect, editor, chart, current_time,
            !self.input_blocked && editor.selection_transform.is_none() && !editor.keyboard.owns_input());
        let was_transforming = editor.selection_transform.is_some();
        let shortcuts_owned = if template_was_dragging || self.templates.dragging() {
            true
        } else if !self.input_blocked || editor.show_help || editor.pending_action.is_some() {
            crate::timeline::handle_edit_shortcuts(ui, editor, chart, current_time)
        } else {
            editor.keyboard.suppress_pointer = false;
            true
        };
        self.input_blocked |= editor.keyboard.dialog.is_some() || editor.show_help || editor.pending_action.is_some();
        if self.input_blocked {
            tools::draw(ui, tools_rect, editor, &self.tool_textures, false);
            crate::timeline::draw_overview(ui, overview_rect, editor, chart, current_time, duration, false);
            return;
        }

        // Timeline host first (lower z), then the palette on top so it captures
        // the pointer where the two overlap.
        let response = ui.interact(
            timeline_rect,
            ui.id().with("editor_timeline"),
            egui::Sense::click_and_drag(),
        );
        let transform_owned = crate::timeline::transform::handle_pointer(
            ui,
            timeline_rect,
            editor,
            chart,
            current_time,
            raw_wheel / 40.0,
        ) || was_transforming;
        let keyboard_owned = editor.keyboard.owns_input() || editor.keyboard.suppress_pointer;
        let over_tools = ui.input(|i| i.pointer.hover_pos().is_some_and(|p| tools_rect.contains(p)));
        if !template_owned && !transform_owned && !keyboard_owned && (editor.keyboard.active || !over_tools) {
            crate::timeline::placement::draw(
                ui, timeline_rect, editor, chart, current_time, &self.tool_textures,
            );
        }
        let tools_hovered = tools::draw(ui, tools_rect, editor, &self.tool_textures, !transform_owned && !keyboard_owned && !self.templates.dragging());
        let handle_active = !template_owned && !transform_owned && !keyboard_owned
            && crate::timeline::handles::draw(
                ui,
                timeline_rect,
                tools_rect,
                editor,
                chart,
                current_time,
            );

        if !template_owned && !tools_hovered && !handle_active && !transform_owned && !keyboard_owned {
            crate::timeline::handle_input(
                editor.layout,
                &response,
                timeline_rect,
                editor,
                chart,
                current_time,
                ui,
                raw_wheel,
            );
        }
        // Slide-edit mode reuses the letter keys (`Q/P/S/Z/V/W`) for shapes, so
        // suppress the single-key tool shortcuts while it is active.
        if !editor.is_slide_edit() && !shortcuts_owned && !transform_owned {
            tools::handle_shortcuts(ui, editor);
        }

        crate::keyboard::navigation::draw(ui, editor, chart, current_time);
        crate::timeline::draw_overview(ui, overview_rect, editor, chart, current_time, duration, !transform_owned && !keyboard_owned);
        let _ = waveform;
    }

    /// The small chevron tab that collapses / expands the right panel (design
    /// shows a 12×28 tab at the panel's mid-height; the whole seam is clickable).
    fn draw_panel_handle(&mut self, ui: &mut egui::Ui, rect: egui::Rect, interactive: bool) {
        let response = ui.interact(
            rect,
            ui.id().with("editor_panel_handle"),
            if interactive { egui::Sense::click() } else { egui::Sense::hover() },
        );
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
        if interactive && response.clicked() {
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
