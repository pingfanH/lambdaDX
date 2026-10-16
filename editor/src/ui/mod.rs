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
pub mod history;
pub mod navigation;
pub mod layout;
pub mod style;
pub mod toolbar;
pub mod templates;
pub mod tools;
pub mod widgets;
pub mod workbench;

#[cfg(test)]
mod tests;

use egui_macroquad::egui;

use crate::audio::WaveformPyramid;
use crate::player::state::PadPreviewState;
use crate::state::{EditorState, Workspace};
use crate::theme::{BG_PANEL, BG_TOOLBAR, TEXT, TEXT_DIM, TOOLBAR_H};

use layout::Layout;
use workbench::{PaneKind, Workbench};

/// `(keys, action)` rows shown in the `?` shortcut-help overlay.
const SHORTCUTS: &[(&str, &str)] = &[
    ("F1 / Cmd+,", "Settings"),
    ("F2 / Shift+F2", "Select candidate / toggle candidate selection"),
    ("Cmd+Enter / ⇧Cmd+Enter", "Start box selection / additive box; Enter confirms, Esc cancels"),
    ("F3 / F4", "Commands / tools"),
    ("F5 / F7", "Properties / Hold and Slide handles"),
    ("F6 / Shift+F6", "Focus next / previous visible pane"),
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
    ("Hold middle + move", "Scroll toward pointer; distance controls speed"),
    ("Shift+wheel", "Pan time without seeking"),
    ("Cmd+wheel / pinch", "Zoom around pointer; suspend follow"),
    ("Wheel", "Vertical: seek playback; horizontal: browse time"),
    ("Shift+drag overview", "Set loop range; Esc cancels"),
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
    ("Click clip / Enter", "Select and open sensor detail"),
    ("Drag interval / endpoints", "Move interval / adjust Press or Release"),
    ("G / Shift+D", "Move / duplicate selection, then Enter or click to confirm"),
    ("Tab / Shift+Tab", "Next / previous clip or detail endpoint"),
    ("Enter / D / V", "Open sensor detail / draw / select"),
    ("Del / Backspace", "Delete selected event, interval or clip"),
    ("Esc", "Cancel draft / leave drawing / clear selection / close detail"),
    ("F8", "Toggle grid snapping"),
    ("Click / drag ruler", "Seek playback"),
    ("Shift+drag ruler", "Set loop range; Esc cancels"),
    ("Hold middle + move", "Scroll toward pointer; distance controls speed"),
    ("Shift+wheel", "Pan time without seeking"),
    ("Wheel", "Scroll lanes"),
    ("Cmd+wheel / pinch", "Zoom around pointer; suspend follow"),
    ("F3 / F6 / Shift+F6", "Commands / next pane / previous pane"),
    ("F1 / Cmd+,", "Settings"),
    ("F12 / ?", "Toggle this help"),
];

/// Chrome-owned state that survives across frames (unrelated to the document).
pub struct EditorUi {
    /// Screen regions computed last frame.
    pub layout: Layout,
    /// Pad preview zoom multiplier.
    pub pad_scale: f32,
    pub workbench: Workbench,
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
            workbench: Workbench::default(),
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
            editor.pinned_note = None;
            editor.loop_range = None;
            editor.loop_enabled = false;
            editor.loop_drag = None;
        }
        crate::player::params_panel::ensure_egui_font(ctx);
        if !self.style_applied {
            style::apply(ctx);
            self.style_applied = true;
            self.input_blocked = true;
            // Named font families become available at the start of the next pass.
            ctx.request_discard("editor font initialization");
            return;
        }
        let overlay_was_open = app.show_params || editor.show_help || editor.pending_action.is_some()
            || editor.keyboard.dialog.is_some();
        let canvas_tab = (self.workbench.focused(PaneKind::Chart) || self.workbench.focused(PaneKind::Autoplay))
            && !overlay_was_open && !self.menu_open && !editor.keyboard.ime
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
        self.workbench.initialize(editor.workspace);
        self.layout = Layout { pad: self.layout.pad, ..Default::default() };
        let transaction = editor.selection_transform.is_some() || editor.keyboard.owns_input()
            || editor.autoplay_editor.dragging() || self.templates.dragging()
            || editor.dragging.is_some() || editor.handle_grab.is_some() || editor.timeline_pan.is_some()
            || editor.star_placement.is_some() || editor.hold_placement.is_some()
            || editor.pointer_press.is_some() || editor.marquee_start.is_some() || editor.scrubbing
            || editor.loop_drag.is_some();
        if !transaction && !overlay_was_open && !self.menu_open && !ctx.memory(|m| m.any_popup_open()) {
            let workspace = self.workbench.active_workspace(ctx, editor.workspace);
            self.change_workspace(editor, app, workspace);
            if !ctx.wants_keyboard_input() && !editor.keyboard.ime {
                crate::keyboard::panels::load_bindings(editor);
                let mut steps = Vec::new();
                ctx.input_mut(|i| i.events.retain(|event| {
                    if let egui::Event::Key { key, pressed: true, repeat, modifiers, .. } = event
                        && crate::keyboard::panels::bound_command(editor, *key, *modifiers) == Some("focus") {
                        if !repeat { steps.push(modifiers.shift); }
                        false
                    } else { true }
                }));
                for reverse in steps {
                    if let Some(workspace) = self.workbench.cycle_focus(reverse) {
                        self.change_workspace(editor, app, workspace);
                    }
                    ctx.memory_mut(|m| { if let Some(id) = m.focused() { m.surrender_focus(id); } });
                }
            }
        }
        let was_menu_open = self.menu_open;
        self.menu_open = false;
        let mut workspace = None;
        egui::TopBottomPanel::top("editor_toolbar")
            .exact_height(TOOLBAR_H)
            .frame(
                egui::Frame::NONE
                    .fill(BG_TOOLBAR)
                    .inner_margin(egui::Margin::symmetric(6, 0)),
            )
            .show(ctx, |ui| {
                self.layout.toolbar = ui.max_rect();
                let result = ui
                    .add_enabled_ui(!transaction && !overlay_was_open, |ui| {
                        toolbar::draw(ui, editor, app, &mut self.workbench)
                    })
                    .inner;
                self.menu_open = result.0;
                workspace = result.1;
            });
        if let Some(workspace) = workspace {
            self.change_workspace(editor, app, workspace);
            editor.template_panel = self.workbench.has(PaneKind::Templates);
        }
        crate::player::params_panel::draw_embedded_with(ctx, app, |ui| {
            toolbar::draw_editor_settings(ui, editor, &mut self.pad_scale, &mut self.workbench);
        });
        let workspace_blocked = overlay_was_open || app.show_params || editor.show_help
            || editor.pending_action.is_some() || was_menu_open || self.menu_open;
        self.input_blocked = workspace_blocked || ctx.memory(|m| m.any_popup_open());

        if !transaction {
            self.workbench.set_visible(PaneKind::Templates, editor.template_panel);
        }
        let chart_keys = editor.workspace == Workspace::Chart && self.workbench.focused(PaneKind::Chart);
        if !chart_keys { crate::keyboard::handle_modals(ctx, editor, &mut app.chart); }
        crate::autoplay_editor::prepare(ctx, editor, app, !chart_keys && !self.input_blocked
            && !editor.keyboard.owns_input() && (!transaction || editor.autoplay_editor.dragging()),
            self.workbench.focused(PaneKind::Autoplay));
        let mut regions = self.workbench.show(ctx, !self.input_blocked && !transaction, |ui, kind| {
            toolbar::pane_tools(ui, kind, editor, app, &mut self.pad_scale);
        });
        editor.template_panel = self.workbench.has(PaneKind::Templates);
        self.input_blocked |= ctx.memory(|m| m.any_popup_open()) || editor.show_help || editor.pending_action.is_some();

        // Egui assigns an unowned Tab to the first chrome widget during layout.
        // Return only that implicit focus to the canvas; existing fields keep Tab.
        if canvas_tab && !self.input_blocked {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() { m.surrender_focus(id); }
            });
        }
        let duration = app.song_duration();
        // Selection is resolved before Properties, regardless of pane placement.
        regions.sort_by_key(|r| match r.kind { PaneKind::Chart | PaneKind::Autoplay => 0, _ => 1 });
        for region in regions {
            if region.rect.width() < 1.0 || region.rect.height() < 1.0 { continue; }
            let mut ui = region.ui(ctx);
            match region.kind {
                PaneKind::Chart => self.central(&mut ui, editor, &mut app.chart, current_time, waveform, duration, raw_wheel),
                PaneKind::Pad => { self.layout.preview = region.rect; }
                PaneKind::Overview => {
                    self.layout.overview = region.rect;
                    crate::timeline::draw_overview(&mut ui, region.rect, editor, &app.chart, current_time, duration,
                        !self.input_blocked && (!transaction || editor.scrubbing || editor.loop_drag.is_some()));
                }
                PaneKind::Autoplay => crate::autoplay_editor::tracks_panel(&mut ui, editor, app,
                    !self.input_blocked && (!transaction || editor.autoplay_editor.dragging()
                        || editor.scrubbing || editor.loop_drag.is_some()), raw_wheel),
                PaneKind::Properties | PaneKind::Templates | PaneKind::History => {
                    egui::Frame::NONE.fill(BG_PANEL).inner_margin(8).show(&mut ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            let editable = !self.input_blocked && editor.loop_drag.is_none() && !editor.keyboard.owns_input()
                                && editor.selection_transform.is_none() && !self.templates.dragging()
                                && !editor.autoplay_editor.pointer_dragging() && editor.timeline_pan.is_none();
                            match region.kind {
                                PaneKind::History => { ui.add_enabled_ui(editable && !transaction, |ui| history::draw(ui, editor, &mut app.chart)); }
                                PaneKind::Templates => self.templates.panel(ui, editor, &app.chart, editable),
                                _ if editor.workspace == Workspace::Autoplay => crate::autoplay_editor::inspector_panel(ui, editor, app, editable),
                                _ => { ui.add_enabled_ui(editable, |ui| {
                                    inspector::object_header(ui, editor, &app.chart, &self.tool_textures);
                                    inspector::draw(ui, editor, &mut app.chart, &mut self.inspector_open);
                                }); }
                            }
                        });
                    });
                }
            }
        }
        crate::keyboard::panels::draw(ctx, editor, &mut app.chart);
        crate::keyboard::apply_transport(editor, app);
        self.apply_view_commands(editor, app);
        // Do not carry a pinch over a menu or another pane into a later frame.
        crate::gesture::take();
        self.input_blocked |= editor.keyboard.dialog.is_some() || app.show_params;

        if editor.show_help {
            self.draw_help(ctx, editor);
        }
        if editor.pending_action.is_some() {
            self.draw_confirm(ctx, editor, &mut app.chart);
        }
    }

    fn apply_view_commands(&mut self, editor: &mut EditorState, app: &mut PadPreviewState) {
        use crate::keyboard::ViewCommand;
        for command in std::mem::take(&mut editor.keyboard.view_commands) {
            match command {
                ViewCommand::FitSelection => {
                    if let Some(span) = navigation::selection_span(editor, &app.chart) { navigation::fit(editor, span); }
                    else { editor.status = Some("没有可定位的选择对象".into()); }
                }
                ViewCommand::FitSong => navigation::fit(editor, (0.0, app.song_duration())),
                ViewCommand::Current => navigation::current(editor, app.timeline_time()),
                ViewCommand::Follow => navigation::toggle_follow(editor, app.timeline_time()),
                ViewCommand::LoopSelection => navigation::loop_selection(editor, &app.chart),
                ViewCommand::ClearLoop => { editor.loop_range = None; editor.loop_enabled = false; }
                ViewCommand::Settings => app.show_params = true,
                ViewCommand::History => self.workbench.set_visible(PaneKind::History, true),
                ViewCommand::Maximize => self.workbench.maximize_focused(),
                ViewCommand::SaveLayout => self.workbench.remember_layout(),
                ViewCommand::RestoreLayout => self.workbench.restore_layout(),
                ViewCommand::ResetLayout => self.workbench.reset_layout(),
            }
        }
        editor.template_panel = self.workbench.has(PaneKind::Templates);
    }

    fn change_workspace(&mut self, editor: &mut EditorState, app: &mut PadPreviewState, workspace: Workspace) {
        if editor.workspace == workspace { return; }
        crate::templates::cancel_interaction(editor, &mut app.chart);
        if editor.template_session.is_some() { crate::templates::apply(editor, app, crate::templates::Action::Exit); }
        editor.autoplay_editor.cancel();
        editor.template_action = None;
        self.templates = Default::default();
        editor.workspace = workspace;
        if workspace == Workspace::Autoplay && editor.autoplay_mode < 2 {
            editor.autoplay_mode = 2;
            editor.mode_dirty = true;
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
        _duration: f32,
        raw_wheel: f32,
    ) {
        let timeline_rect = ui.max_rect();
        self.layout.timeline = timeline_rect;
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
        if editor.workspace != Workspace::Chart || editor.loop_drag.is_some() {
            tools::draw(ui, tools_rect, editor, &self.tool_textures, false);
            return;
        }
        let template_was_dragging = self.templates.dragging();
        let template_owned = self.templates.timeline(ui, timeline_rect, tools_rect, editor, chart, current_time,
            !self.input_blocked && editor.selection_transform.is_none() && !editor.keyboard.owns_input()
                && editor.timeline_pan.is_none() && !editor.scrubbing
                && editor.pointer_press.is_none() && editor.dragging.is_none()
                && editor.handle_grab.is_none() && editor.marquee_start.is_none());
        let was_transforming = editor.selection_transform.is_some();
        let shortcuts_owned = if template_was_dragging || self.templates.dragging() {
            true
        } else if self.workbench.focused(PaneKind::Chart)
            && (!self.input_blocked || editor.show_help || editor.pending_action.is_some()) {
            crate::timeline::handle_edit_shortcuts(ui, editor, chart, current_time)
        } else {
            editor.keyboard.suppress_pointer = false;
            true
        };
        self.input_blocked |= editor.keyboard.dialog.is_some() || editor.show_help || editor.pending_action.is_some();
        if self.input_blocked {
            tools::draw(ui, tools_rect, editor, &self.tool_textures, false);
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
        let navigation_owned = !template_was_dragging && !self.templates.dragging() && !transform_owned && !keyboard_owned
            && (!over_tools || editor.timeline_pan.is_some())
            && crate::timeline::input::navigate(ui, timeline_rect, editor, current_time, raw_wheel);
        if !template_owned && !transform_owned && !keyboard_owned && !navigation_owned && (editor.keyboard.active || !over_tools) {
            crate::timeline::placement::draw(
                ui, timeline_rect, editor, chart, current_time, &self.tool_textures,
            );
        }
        let tools_hovered = tools::draw(ui, tools_rect, editor, &self.tool_textures, !transform_owned && !keyboard_owned
            && !navigation_owned && !editor.scrubbing && !self.templates.dragging());
        let handle_active = !template_owned && !transform_owned && !keyboard_owned && !navigation_owned && !editor.scrubbing
            && crate::timeline::handles::draw(
                ui,
                timeline_rect,
                tools_rect,
                editor,
                chart,
                current_time,
            );

        if !template_owned && (!tools_hovered || editor.scrubbing) && !handle_active
            && !transform_owned && !keyboard_owned && !navigation_owned {
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
        // Slide-edit mode reuses the letter keys (`Q/P/S/Z/V/W`) for shapes, so
        // suppress the single-key tool shortcuts while it is active.
        if !editor.is_slide_edit() && !shortcuts_owned && !transform_owned {
            tools::handle_shortcuts(ui, editor);
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            editor.pointer_press = None;
        }

        crate::keyboard::navigation::draw(ui, editor, chart, current_time);
        let _ = waveform;
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
