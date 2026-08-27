use crate::ui_prototype::components::button::*;
use crate::ui_prototype::style::*;
use egui_macroquad::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TopBottomPanel, Vec2,
};
use std::sync::{Arc, Once};
pub mod draw_timeline;
use super::chart;
use super::simai_io;
use super::state::AppState;
use super::template;
use super::types::{Mode, PlaceTool, PlacementState};

// ── Blender color palette ──────────────────────────────────────────────
const BG_DARK: Color32 = Color32::from_rgb(30, 30, 30);
const BG_PANEL: Color32 = Color32::from_rgb(45, 45, 45);
const BG_WIDGET: Color32 = Color32::from_rgb(60, 60, 60);
const BG_HOVER: Color32 = Color32::from_rgb(72, 72, 72);
const BG_ACTIVE: Color32 = Color32::from_rgb(74, 125, 170);
const BORDER: Color32 = Color32::from_rgb(20, 20, 20);
const BORDER_LIGHT: Color32 = Color32::from_rgb(60, 60, 60);
const TEXT: Color32 = Color32::from_rgb(224, 224, 224);
const TEXT_DIM: Color32 = Color32::from_rgb(144, 144, 144);
const ACCENT_BLUE: Color32 = Color32::from_rgb(90, 141, 186);
const ACCENT_ORANGE: Color32 = Color32::from_rgb(230, 149, 48);

const CR2: CornerRadius = CornerRadius::same(2);

static FONT_ONCE: Once = Once::new();
static mut APP_SCENE_DRAWER_OPEN: bool = false;

/// Load font: try bundled assets/Arial.ttf first, then system fonts.
fn load_system_font(ctx: &egui::Context) {
    FONT_ONCE.call_once(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let font_candidates: Vec<String> = {
            let mut v = vec![
                "assets/Arial.ttf".to_string(),
                "assets/arial.ttf".to_string(),
                "assets/font.ttf".to_string(),
            ];
            if let Some(ref dir) = exe_dir {
                v.push(dir.join("assets/Arial.ttf").to_string_lossy().to_string());
            }
            v.extend([
                "/System/Library/Fonts/Helvetica.ttc".to_string(),
                "/Library/Fonts/Arial.ttf".to_string(),
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string(),
                "C:\\Windows\\Fonts\\segoeui.ttf".to_string(),
            ]);
            v
        };

        for path in &font_candidates {
            if let Ok(data) = std::fs::read(path) {
                let mut fonts = FontDefinitions::default();
                fonts
                    .font_data
                    .insert("system".to_owned(), Arc::new(FontData::from_owned(data)));
                if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
                    proportional.insert(0, "system".to_owned());
                }
                ctx.set_fonts(fonts);
                return;
            }
        }
    });
}

/// Apply Blender-inspired dark theme to the egui context.
pub fn apply_blender_style(ctx: &egui::Context) {
    load_system_font(ctx);

    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = Vec2::new(4.0, 4.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.spacing.window_margin = Margin::same(6);

    style.text_styles = [
        (
            egui::TextStyle::Heading,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CR2;
    visuals.panel_fill = BG_DARK;
    visuals.faint_bg_color = Color32::from_rgb(35, 35, 35);
    visuals.extreme_bg_color = Color32::from_rgb(24, 24, 24);
    visuals.selection.bg_fill = BG_ACTIVE;
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(120, 170, 220));
    visuals.widgets.noninteractive.bg_fill = BG_DARK;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(0.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = CR2;
    visuals.widgets.inactive.bg_fill = BG_WIDGET;
    visuals.widgets.inactive.weak_bg_fill = BG_WIDGET;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);
    visuals.widgets.inactive.corner_radius = CR2;
    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.hovered.corner_radius = CR2;
    visuals.widgets.active.bg_fill = BG_ACTIVE;
    visuals.widgets.active.weak_bg_fill = BG_ACTIVE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.active.corner_radius = CR2;
    visuals.widgets.open.bg_fill = BG_HOVER;
    visuals.widgets.open.weak_bg_fill = BG_HOVER;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.open.corner_radius = CR2;

    style.visuals = visuals;
    ctx.set_style(style);
}

fn section_sep(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, CR2, BORDER_LIGHT);
    ui.add_space(2.0);
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT_DIM).small());
}

fn toggle_btn(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let btn = egui::Button::new(egui::RichText::new(label).color(if active {
        Color32::WHITE
    } else {
        TEXT
    }))
    .fill(if active { BG_ACTIVE } else { BG_WIDGET });
    ui.add(btn).clicked()
}

fn step_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let btn =
        egui::Button::new(egui::RichText::new(label).color(TEXT)).min_size(Vec2::new(20.0, 20.0));
    ui.add(btn)
}

/// Draw egui toolbar on top. Pad + timeline are native macroquad below.
fn import_from_path(app: &mut super::state::AppState) {
    let path = app.import_path_input.trim().to_string();
    if path.is_empty() {
        return;
    }
    match simai_io::import_from_file_path(&path) {
        Ok(import) => {
            let n = import.chart.notes.len();
            app.set_chart(import.chart);
            app.set_selected_note(None);
            app.set_editing_slide_path(None);
            if let (Some(bytes), Some(ext)) = (&import.audio_bytes, &import.audio_ext) {
                if let Some(pcm) = super::audio::load_audio_from_bytes(bytes, ext) {
                    app.audio_source_name = Some(import.title.clone());
                    app.audio_wav_pcm = Some(pcm);
                    app.audio_cache.clear();
                    app.request_audio_start();
                }
            }
            app.set_status(format!("Opened {} ({n} notes)", import.title));
        }
        Err(e) => app.set_status(format!("Import: {e}")),
    }
}

pub fn draw_egui_ui(ctx: &egui::Context, app: &mut AppState) {
    apply_blender_style(ctx);

    TopBottomPanel::top("toolbar")
        .frame(
            egui::Frame::none()
                .fill(BG_DARK)
                .inner_margin(Margin::symmetric(8, 4))
                .stroke(Stroke::new(1.0, BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // ── App title ──
                ui.label(
                    egui::RichText::new("LambdaDX Demo")
                        .font(FontId::new(16.0, FontFamily::Proportional))
                        .color(ACCENT_ORANGE)
                        .strong(),
                );
                section_sep(ui);

                // ── Mode indicator ──
                let (mode_text, mode_color) = match app.mode {
                    Mode::Idle => ("IDLE", TEXT_DIM),
                    Mode::Recording => ("REC", ACCENT_ORANGE),
                    Mode::Playing => ("PLAY", ACCENT_BLUE),
                };
                ui.label(
                    egui::RichText::new(mode_text)
                        .font(FontId::new(13.0, FontFamily::Monospace))
                        .color(mode_color)
                        .strong(),
                );
                section_sep(ui);

                // ── Transport ──
                section_label(ui, "Transport");
                if toggle_btn(ui, "Play", app.mode == Mode::Playing) {
                    app.toggle_play();
                }
                if toggle_btn(ui, "Rec", app.mode == Mode::Recording) {
                    app.toggle_record();
                }
                section_sep(ui);

                // ── File ──
                section_label(ui, "File");
                if ui.button(egui::RichText::new("Save").color(TEXT)).clicked() {
                    match chart::save_recording_doc(app) {
                        Ok(p) => app.set_status(format!("Saved {}", p.display())),
                        Err(e) => app.set_status(format!("Save: {e}")),
                    }
                }
                if ui.button(egui::RichText::new("Load").color(TEXT)).clicked() {
                    match chart::load_latest_saved_chart() {
                        Ok(c) => {
                            let n = c.notes.len();
                            app.set_chart(c);
                            app.set_status(format!("{n} notes"));
                        }
                        Err(e) => app.set_status(format!("Load: {e}")),
                    }
                }
                if ui
                    .button(egui::RichText::new("Open...").color(TEXT))
                    .on_hover_text("Pick chart file from dialog")
                    .clicked()
                {
                    app.pending_import = true;
                }
                if !app.import_levels.is_empty() {
                    ui.add_space(2.0);
                    let current_label = app
                        .import_levels
                        .iter()
                        .find(|(lv, _)| *lv == app.import_selected_level)
                        .map(|(_, s)| s.as_str())
                        .unwrap_or("?");
                    if ui
                        .button(
                            egui::RichText::new(format!("Lv.{}", current_label))
                                .color(ACCENT_ORANGE),
                        )
                        .clicked()
                    {
                        // Cycle to next level
                        let idx = app
                            .import_levels
                            .iter()
                            .position(|(lv, _)| *lv == app.import_selected_level)
                            .unwrap_or(0);
                        let next = (idx + 1) % app.import_levels.len();
                        let (new_lv, _) = app.import_levels[next];
                        app.import_selected_level = new_lv;
                        if let Some(ref simai) = app.imported_simai {
                            match simai_io::convert_simai_level(simai, new_lv) {
                                Ok(chart) => {
                                    app.set_chart(chart);
                                    app.set_status(format!(
                                        "Switched to Lv.{}",
                                        app.import_levels
                                            .iter()
                                            .find(|(lv, _)| *lv == new_lv)
                                            .map(|(_, s)| s.as_str())
                                            .unwrap_or("?")
                                    ));
                                }
                                Err(e) => app.set_status(format!("Level switch: {e}")),
                            }
                        }
                    }
                }
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Path:").color(TEXT_DIM));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.import_path_input)
                        .hint_text("maidata.txt path...")
                        .desired_width(140.0),
                );
                if resp.lost_focus()
                    && ui.input(|i| i.key_pressed(egui_macroquad::egui::Key::Enter))
                {
                    import_from_path(app);
                }
                if ui
                    .button(egui::RichText::new("Import").color(TEXT))
                    .clicked()
                {
                    import_from_path(app);
                }
                section_sep(ui);

                // ── Simai ──
                section_label(ui, "Simai");
                if ui
                    .button(egui::RichText::new("Import").color(TEXT))
                    .on_hover_text("Import from output/import.simai")
                    .clicked()
                {
                    match simai_io::import_from_simai_path("import.simai") {
                        Ok(c) => {
                            let n = c.notes.len();
                            app.set_chart(c);
                            app.set_selected_note(None);
                            app.set_editing_slide_path(None);
                            app.set_status(format!("Imported {n} notes"));
                        }
                        Err(e) => app.set_status(format!("Import: {e}")),
                    }
                }
                if ui
                    .button(egui::RichText::new("Export").color(TEXT))
                    .on_hover_text("Export to output/export.simai")
                    .clicked()
                {
                    match simai_io::export_to_simai_path(&app.chart, "export.simai") {
                        Ok(p) => app.set_status(format!("Exported {}", p.display())),
                        Err(e) => app.set_status(format!("Export: {e}")),
                    }
                }
                section_sep(ui);

                // ── Edit ──
                if ui
                    .button(egui::RichText::new("Clear").color(TEXT))
                    .clicked()
                {
                    app.recording_hits.clear();
                    app.recording_notes.clear();
                    app.active_record_holds.clear();
                    app.active_pointer_zones.clear();
                    app.prev_pointer_pos.clear();
                    app.set_status("Cleared".to_string());
                }
                if toggle_btn(ui, "Grid", app.record_snap_grid) {
                    app.record_snap_grid = !app.record_snap_grid;
                }
                section_sep(ui);

                // ── Speed controls ──
                section_label(ui, "Speed");
                ui.label(egui::RichText::new("Rec").color(TEXT_DIM));
                if step_btn(ui, "-").clicked() {
                    app.set_record_speed((app.record_speed - 0.1).max(0.1));
                }
                ui.label(egui::RichText::new(format!("{:.1}x", app.record_speed)).color(TEXT));
                if step_btn(ui, "+").clicked() {
                    app.set_record_speed((app.record_speed + 0.1).min(3.0));
                }

                ui.add_space(4.0);
                ui.label(egui::RichText::new("Play").color(TEXT_DIM));
                if step_btn(ui, "-").clicked() {
                    app.set_play_speed((app.play_speed - 0.1).max(0.1));
                }
                ui.label(egui::RichText::new(format!("{:.1}x", app.play_speed)).color(TEXT));
                if step_btn(ui, "+").clicked() {
                    app.set_play_speed((app.play_speed + 0.1).min(3.0));
                }
                section_sep(ui);

                // ── Offset ──
                section_label(ui, "Offset");
                if step_btn(ui, "-")
                    .on_hover_text("Shift audio earlier (-50ms)")
                    .clicked()
                {
                    app.chart.audio_offset = (app.chart.audio_offset - 0.05).max(-5.0);
                    app.audio_cache.clear();
                    if matches!(app.mode, Mode::Playing) {
                        app.request_audio_start();
                    }
                    app.set_status(format!("Offset: {:.3}s", app.chart.audio_offset));
                }
                ui.label(
                    egui::RichText::new(format!("{:.2}s", app.chart.audio_offset)).color(TEXT),
                );
                if step_btn(ui, "+")
                    .on_hover_text("Shift audio later (+50ms)")
                    .clicked()
                {
                    app.chart.audio_offset = (app.chart.audio_offset + 0.05).min(30.0);
                    app.audio_cache.clear();
                    if matches!(app.mode, Mode::Playing) {
                        app.request_audio_start();
                    }
                    app.set_status(format!("Offset: {:.3}s", app.chart.audio_offset));
                }
                section_sep(ui);

                // ── Toggles ──
                if toggle_btn(
                    ui,
                    if app.audio_enabled { "Audio" } else { "Muted" },
                    app.audio_enabled,
                ) {
                    app.audio_enabled = !app.audio_enabled;
                }
                if toggle_btn(ui, "Mobile", app.mobile_ui) {
                    app.mobile_ui = !app.mobile_ui;
                }

                // ── Status (right-aligned) ──
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&app.status).color(TEXT_DIM).small());
                });
            });
        });

    // ── Template controls (second row) ──
    TopBottomPanel::top("template_bar")
        .frame(
            egui::Frame::none()
                .fill(BG_DARK)
                .inner_margin(Margin::symmetric(8, 3))
                .stroke(Stroke::new(0.5, BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                section_label(ui, "Template");

                // New empty template → enters isolation directly.
                if ui
                    .button(egui::RichText::new("New").color(TEXT))
                    .on_hover_text("Create a new empty template and edit it")
                    .clicked()
                {
                    let name = format!("Template {}", app.next_template_id);
                    match template::create_new_template(app, &name) {
                        Ok(id) => {
                            app.selected_template_idx = Some(app.chart.templates.len() - 1);
                            // status already set by enter_isolation
                        }
                        Err(e) => app.set_status(format!("New template: {}", e)),
                    }
                }

                // Create template from selection.
                if ui
                    .button(egui::RichText::new("New from Sel").color(TEXT))
                    .on_hover_text("Create template from selected notes")
                    .clicked()
                {
                    let name = format!("Template {}", app.next_template_id);
                    match template::create_template(app, &name) {
                        Ok(id) => {
                            app.selected_template_idx =
                                app.chart.templates.iter().position(|t| t.id == id);
                            app.set_status(format!("Created template: {}", id));
                        }
                        Err(e) => app.set_status(format!("Create template: {}", e)),
                    }
                }

                // Template dropdown.
                let tpl_names: Vec<String> = app
                    .chart
                    .templates
                    .iter()
                    .map(|t| format!("{} (v{})", t.name, t.version))
                    .collect();

                if !tpl_names.is_empty() {
                    let selected_text = if let Some(idx) = app.selected_template_idx {
                        tpl_names.get(idx).cloned().unwrap_or_default()
                    } else {
                        "Select template...".to_string()
                    };

                    egui::ComboBox::from_id_source("tpl_select")
                        .selected_text(egui::RichText::new(selected_text).color(TEXT))
                        .show_ui(ui, |ui| {
                            for (i, name) in tpl_names.iter().enumerate() {
                                let is_selected = app.selected_template_idx == Some(i);
                                if ui
                                    .selectable_label(
                                        is_selected,
                                        egui::RichText::new(name).color(TEXT),
                                    )
                                    .clicked()
                                {
                                    app.selected_template_idx = Some(i);
                                }
                            }
                        });
                }

                section_sep(ui);

                // Insert instance at cursor.
                if ui
                    .button(egui::RichText::new("Insert").color(TEXT))
                    .on_hover_text("Insert template instance at current playback/view position")
                    .clicked()
                {
                    if let Some(idx) = app.selected_template_idx {
                        match template::insert_instance(app, idx) {
                            Ok(()) => app.set_status("Inserted template instance".to_string()),
                            Err(e) => app.set_status(format!("Insert: {}", e)),
                        }
                    } else {
                        app.set_status("Select a template first".to_string());
                    }
                }

                // Enter isolation mode.
                let in_isolation = template::is_in_isolation(app);
                if !in_isolation {
                    if ui
                        .button(egui::RichText::new("Edit").color(TEXT))
                        .on_hover_text("Enter isolation mode to edit template")
                        .clicked()
                    {
                        if let Some(idx) = app.selected_template_idx {
                            match template::enter_isolation(app, idx) {
                                Ok(()) => {}
                                Err(e) => app.set_status(format!("Enter: {}", e)),
                            }
                        } else {
                            app.set_status("Select a template first".to_string());
                        }
                    }
                } else {
                    // Exit isolation mode.
                    if toggle_btn(ui, "Exit Edit", true) {
                        match template::exit_isolation(app) {
                            Ok(()) => {}
                            Err(e) => app.set_status(format!("Exit: {}", e)),
                        }
                    }
                }

                // Rename template.
                if let Some(idx) = app.selected_template_idx {
                    if !in_isolation {
                        section_sep(ui);
                        if let Some(tpl) = app.chart.templates.get(idx) {
                            let mut rename_buf = tpl.name.clone();
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut rename_buf)
                                    .desired_width(100.0)
                                    .hint_text("Rename"),
                            );
                            if response.changed() {
                                let _ = template::rename_template(app, idx, &rename_buf);
                            }
                        }
                    }
                }

                // Delete template.
                if let Some(idx) = app.selected_template_idx {
                    if !in_isolation {
                        section_sep(ui);
                        if ui
                            .button(egui::RichText::new("Delete").color(ACCENT_ORANGE))
                            .on_hover_text("Delete selected template")
                            .clicked()
                        {
                            if idx < app.chart.templates.len() {
                                let name = app.chart.templates[idx].name.clone();
                                app.chart.templates.remove(idx);
                                app.selected_template_idx = None;
                                app.set_status(format!("Deleted template: {}", name));
                            }
                        }
                    }
                }

                // Right-aligned breadcrumb when in isolation.
                if in_isolation {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let path = template::breadcrumb_path(app);
                        for (i, name) in path.iter().enumerate().rev() {
                            if i < path.len() - 1 {
                                ui.label(egui::RichText::new(">").color(TEXT_DIM).small());
                            }
                            let is_current = i == path.len() - 1;
                            let btn = ui.button(
                                egui::RichText::new(name)
                                    .color(if is_current {
                                        ACCENT_ORANGE
                                    } else {
                                        ACCENT_BLUE
                                    })
                                    .small(),
                            );
                            if btn.clicked() && !is_current {
                                template::navigate_to_breadcrumb(app, i);
                            }
                        }
                    });
                }
            });
        });

    // Store the approximate toolbar bottom for input blocking.
    // The toolbar has 2 panels (main + template bar), each ~30px, plus margins.
    app.egui_toolbar_bottom = 75.0;
}
pub fn draw_editor(egui_ctx: &egui::Context, app: &mut AppState) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG_DARK;
    visuals.panel_fill = egui::Color32::TRANSPARENT;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.inactive.bg_fill = BG_BUTTON;
    visuals.widgets.hovered.bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.active.bg_fill = ACCENT_BLUE;
    egui_ctx.set_visuals(visuals);

    // Top panel: toolbar
    egui::TopBottomPanel::top("toolbar")
        .resizable(false)
        .exact_height(TOOLBAR_HEIGHT + PADDING)
        .show(egui_ctx, |ui| {
            draw_app_toolbar(ui, app);
        });

    // Left panel: sidebar — T/H/Star wired to real PlaceTool
    egui::SidePanel::left("sidebar")
        .resizable(false)
        .exact_width(SIDEBAR_WIDTH + PADDING)
        .show(egui_ctx, |ui| {
            draw_app_sidebar(ui, app);
        });

    // Central area: transparent — macroquad renders draw_timeline_panel + draw_pad_panel behind
    egui::CentralPanel::default().show(egui_ctx, |ui| {
        let screen_rect = ui.max_rect();
        let available = ui.available_size();
        let timeline_w = available.x * 0.7;
        let viewport_w = available.x - timeline_w;

        let mut saved_timeline_rect = egui::Rect::NOTHING;

        ui.horizontal_top(|ui| {
            // ── Timeline (70%) — allocate space; macroquad renders behind ──
            let (_, timeline_rect) = ui.allocate_space(egui::Vec2::new(timeline_w, available.y));
            saved_timeline_rect = timeline_rect;
            draw_app_scene_toggle_button(ui, timeline_rect);

            // ── Viewport (30%) — pad area (transparent) + properties (solid) ──
            let (_, viewport_rect) = ui.allocate_space(egui::Vec2::new(viewport_w, available.y));
            let props_h = viewport_rect.height() * 0.4;
            let status_h = 20.0;
            let pad_h = viewport_rect.height() - props_h - status_h;

            // Pad area — transparent, macroquad renders behind
            let _pad_rect = egui::Rect::from_min_size(
                viewport_rect.left_top(),
                egui::Vec2::new(viewport_rect.width(), pad_h),
            );

            // Note Properties + Chart Info bg
            let pr = egui::Rect::from_min_size(
                egui::Pos2::new(viewport_rect.left(), viewport_rect.top() + pad_h),
                egui::Vec2::new(viewport_rect.width(), props_h),
            );
            ui.painter().line_segment(
                [pr.left_top(), pr.right_top()],
                egui::Stroke::new(1.0, BORDER_LIGHT),
            );
            ui.painter()
                .rect_filled(pr, egui::CornerRadius::ZERO, BG_DARK);

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(pr.shrink(PADDING))
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            child.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING * 0.8;
                section_header(ui, "Note Properties");
                ui.add_space(SPACING);
                let sel = app
                    .selected_note
                    .and_then(|id| app.chart.notes.iter().find(|n| n.id == id));
                let nt_str = sel
                    .map(|n| format!("{:?}", n.note_type))
                    .unwrap_or("-".into());
                let time_str = sel.map(|n| format!("m{:.3}", n.time)).unwrap_or("-".into());
                let lane_str = sel.map(|n| format!("{}", n.lane)).unwrap_or("-".into());
                let dur_str = sel
                    .map(|n| format!("m{:.3}", n.hold_duration))
                    .unwrap_or("-".into());
                value_row(ui, "Type", &nt_str);
                value_row(ui, "Time", &time_str);
                value_row(ui, "Lane", &lane_str);
                value_row(ui, "Duration", &dur_str);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACING * 0.5;
                    let bw = ICON_SIZE * 3.0;
                    let bh = BUTTON_HEIGHT * 0.9;
                    if let Some(id) = app.selected_note {
                        if let Some(nidx) = app.find_note_index(id) {
                            if Button::new(
                                "Break",
                                ButtonKind::Toggle(app.chart.notes[nidx].is_break),
                                egui::Vec2::new(bw, bh),
                            )
                            .show(ui)
                            {
                                app.chart.notes[nidx].is_break = !app.chart.notes[nidx].is_break;
                            }
                            if Button::new(
                                "Ex",
                                ButtonKind::Toggle(app.chart.notes[nidx].is_ex),
                                egui::Vec2::new(bw, bh),
                            )
                            .show(ui)
                            {
                                app.chart.notes[nidx].is_ex = !app.chart.notes[nidx].is_ex;
                            }
                            if Button::new(
                                "Star",
                                ButtonKind::Toggle(app.chart.notes[nidx].is_star),
                                egui::Vec2::new(bw, bh),
                            )
                            .show(ui)
                            {
                                app.chart.notes[nidx].is_star = !app.chart.notes[nidx].is_star;
                            }
                            if Button::new(
                                "Tapless",
                                ButtonKind::Toggle(app.chart.notes[nidx].is_tapless),
                                egui::Vec2::new(bw, bh),
                            )
                            .show(ui)
                            {
                                app.chart.notes[nidx].is_tapless =
                                    !app.chart.notes[nidx].is_tapless;
                            }
                        }
                    } else {
                        for (flag, on) in &[
                            ("Break", false),
                            ("Ex", false),
                            ("Star", false),
                            ("Tapless", false),
                        ] {
                            Button::new(flag, ButtonKind::Toggle(*on), egui::Vec2::new(bw, bh))
                                .show(ui);
                        }
                    }
                });
            });
            child.add_space(SPACING);
            let sp = child.available_rect_before_wrap();
            child.painter().line_segment(
                [sp.left_top(), sp.right_top()],
                egui::Stroke::new(1.0, SEPARATOR),
            );
            child.add_space(SPACING);
            child.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING * 0.8;
                section_header(ui, "Chart Info");
                ui.add_space(SPACING);
                value_row(ui, "Title", &app.chart.title);
                value_row(ui, "Artist", &app.chart.artist);
                value_row(ui, "BPM", &format!("{:.1}", app.chart.bpm));
                value_row(ui, "Offset", &format!("{:.3}s", app.chart.audio_offset));
            });

            // Status bar
            let sr = egui::Rect::from_min_size(
                egui::Pos2::new(pr.left(), pr.bottom()),
                egui::Vec2::new(viewport_rect.width(), status_h),
            );
            ui.painter()
                .rect_filled(sr, egui::CornerRadius::ZERO, BG_DARK);
            ui.painter().line_segment(
                [sr.left_top(), sr.right_top()],
                egui::Stroke::new(1.0, BORDER_LIGHT),
            );
            ui.painter().text(
                egui::Pos2::new(sr.left() + PADDING, sr.center().y),
                egui::Align2::LEFT_CENTER,
                &format!(
                    "{} notes | BPM: {:.0} | 1.0x",
                    app.chart.notes.len(),
                    app.chart.bpm
                ),
                egui::FontId::proportional(FONT_SMALL),
                TEXT_DIM,
            );

            // Toggle button on right edge of viewport
            crate::app::egui_components::draw_drawer_toggle(ui, viewport_rect);
        });

        // Floating drawer overlays
        draw_app_template_drawer(egui_ctx, screen_rect, app);
        draw_app_scene_drawer(egui_ctx, saved_timeline_rect, app);
    });
}

fn draw_app_toolbar(ui: &mut egui::Ui, app: &mut AppState) {
    egui::Frame::new()
        .fill(BG_TOOLBAR)
        .stroke(Stroke::new(1.0, BORDER_LIGHT))
        .inner_margin(Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = SPACING;

                ui.label(
                    egui::RichText::new("Lambda DX")
                        .color(TEXT_PRIMARY)
                        .size(FONT_HEADER)
                        .strong(),
                );

                Button::new("|", ButtonKind::Separator, Vec2::new(1.0, BUTTON_HEIGHT)).show(ui);

                if Button::new(
                    "Save",
                    ButtonKind::Normal,
                    Vec2::new(ICON_SIZE * 2.2, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    match chart::save_recording_doc(app) {
                        Ok(path) => app.set_status(format!("Saved {}", path.display())),
                        Err(err) => app.set_status(format!("Save: {err}")),
                    }
                }
                if Button::new(
                    "Load",
                    ButtonKind::Normal,
                    Vec2::new(ICON_SIZE * 2.2, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    match chart::load_latest_saved_chart() {
                        Ok(chart) => {
                            let n = chart.notes.len();
                            app.set_chart(chart);
                            app.set_selected_note(None);
                            app.set_editing_slide_path(None);
                            app.set_status(format!("Loaded {n} notes"));
                        }
                        Err(err) => app.set_status(format!("Load: {err}")),
                    }
                }
                if Button::new(
                    "Open",
                    ButtonKind::Normal,
                    Vec2::new(ICON_SIZE * 2.2, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.pending_import = true;
                }
                if Button::new(
                    "Export",
                    ButtonKind::Normal,
                    Vec2::new(ICON_SIZE * 2.5, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    match simai_io::export_to_simai_path(&app.chart, "export.simai") {
                        Ok(path) => app.set_status(format!("Exported {}", path.display())),
                        Err(err) => app.set_status(format!("Export: {err}")),
                    }
                }

                if !app.import_levels.is_empty() {
                    let selected_level = app
                        .import_levels
                        .iter()
                        .find(|(level, _)| *level == app.import_selected_level)
                        .map(|(_, label)| label.clone())
                        .unwrap_or_else(|| "?".to_string());
                    egui::ComboBox::from_id_source("prototype_app_level_select")
                        .width(72.0)
                        .selected_text(
                            egui::RichText::new(format!("Lv.{selected_level}")).color(TEXT_PRIMARY),
                        )
                        .show_ui(ui, |ui| {
                            let levels = app.import_levels.clone();
                            for (level, label) in levels {
                                if ui
                                    .selectable_label(
                                        app.import_selected_level == level,
                                        egui::RichText::new(format!("Lv.{label}"))
                                            .color(TEXT_PRIMARY),
                                    )
                                    .clicked()
                                {
                                    app.import_selected_level = level;
                                    if let Some(ref simai) = app.imported_simai {
                                        match simai_io::convert_simai_level(simai, level) {
                                            Ok(chart) => {
                                                app.set_chart(chart);
                                                app.set_selected_note(None);
                                                app.set_editing_slide_path(None);
                                                app.set_status(format!("Switched to Lv.{label}"));
                                            }
                                            Err(err) => {
                                                app.set_status(format!("Level switch: {err}"))
                                            }
                                        }
                                    }
                                }
                            }
                        });
                }

                Button::new("|", ButtonKind::Separator, Vec2::new(1.0, BUTTON_HEIGHT)).show(ui);

                let play_label = if app.mode == Mode::Playing {
                    "Pause"
                } else {
                    "Play"
                };
                if Button::new(
                    play_label,
                    ButtonKind::Toggle(app.mode == Mode::Playing),
                    Vec2::new(ICON_SIZE * 2.4, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.toggle_play();
                }
                if Button::new(
                    "Rec",
                    ButtonKind::Toggle(app.mode == Mode::Recording),
                    Vec2::new(ICON_SIZE * 1.9, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.toggle_record();
                }

                Button::new("|", ButtonKind::Separator, Vec2::new(1.0, BUTTON_HEIGHT)).show(ui);

                if Button::new(
                    "Grid",
                    ButtonKind::Toggle(app.record_snap_grid),
                    Vec2::new(ICON_SIZE * 2.0, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.record_snap_grid = !app.record_snap_grid;
                    app.set_status(format!("Record snap to grid: {}", app.record_snap_grid));
                }
                if Button::new(
                    "Audio",
                    ButtonKind::Toggle(app.audio_enabled),
                    Vec2::new(ICON_SIZE * 2.2, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.audio_enabled = !app.audio_enabled;
                    if !app.audio_enabled {
                        app.stop_audio_if_any();
                    }
                }
                if Button::new(
                    "Mobile",
                    ButtonKind::Toggle(app.mobile_ui),
                    Vec2::new(ICON_SIZE * 2.5, BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    app.mobile_ui = !app.mobile_ui;
                }

                Button::new("|", ButtonKind::Separator, Vec2::new(1.0, BUTTON_HEIGHT)).show(ui);
                draw_template_toolbar(ui, app);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&app.status)
                            .color(TEXT_DIM)
                            .size(FONT_SMALL),
                    );
                    let mode = match app.mode {
                        Mode::Idle => "IDLE",
                        Mode::Playing => "PLAY",
                        Mode::Recording => "REC",
                    };
                    ui.label(
                        egui::RichText::new(mode)
                            .color(if app.mode == Mode::Recording {
                                ACCENT_ORANGE
                            } else {
                                TEXT_SECONDARY
                            })
                            .size(FONT_SMALL)
                            .strong(),
                    );
                });
            });
        });
}

fn draw_template_toolbar(ui: &mut egui::Ui, app: &mut AppState) {
    let in_isolation = template::is_in_isolation(app);

    if Button::new(
        "+Tpl",
        ButtonKind::Normal,
        Vec2::new(ICON_SIZE * 2.0, BUTTON_HEIGHT),
    )
    .show(ui)
    {
        let name = format!("Template {}", app.next_template_id);
        match template::create_new_template(app, &name) {
            Ok(_) => app.selected_template_idx = Some(app.chart.templates.len().saturating_sub(1)),
            Err(err) => app.set_status(format!("New template: {err}")),
        }
    }

    if Button::new(
        "From Sel",
        ButtonKind::Normal,
        Vec2::new(ICON_SIZE * 3.0, BUTTON_HEIGHT),
    )
    .show(ui)
    {
        let name = format!("Template {}", app.next_template_id);
        match template::create_template(app, &name) {
            Ok(id) => {
                app.selected_template_idx = app.chart.templates.iter().position(|t| t.id == id);
                app.set_status(format!("Created template: {id}"));
            }
            Err(err) => app.set_status(format!("Create template: {err}")),
        }
    }

    if !app.chart.templates.is_empty() {
        let selected_text = app
            .selected_template_idx
            .and_then(|idx| app.chart.templates.get(idx))
            .map(|tpl| tpl.name.clone())
            .unwrap_or_else(|| "Template".to_string());

        egui::ComboBox::from_id_source("prototype_app_template_select")
            .width(110.0)
            .selected_text(egui::RichText::new(selected_text).color(TEXT_PRIMARY))
            .show_ui(ui, |ui| {
                for (idx, tpl) in app.chart.templates.iter().enumerate() {
                    if ui
                        .selectable_label(
                            app.selected_template_idx == Some(idx),
                            egui::RichText::new(format!("{} v{}", tpl.name, tpl.version))
                                .color(TEXT_PRIMARY),
                        )
                        .clicked()
                    {
                        app.selected_template_idx = Some(idx);
                    }
                }
            });
    }

    if Button::new(
        "Insert",
        ButtonKind::Normal,
        Vec2::new(ICON_SIZE * 2.4, BUTTON_HEIGHT),
    )
    .show(ui)
    {
        if let Some(idx) = app.selected_template_idx {
            if let Err(err) = template::insert_instance(app, idx) {
                app.set_status(format!("Insert: {err}"));
            }
        } else {
            app.set_status("Select a template first".to_string());
        }
    }

    if in_isolation {
        if Button::new(
            "Exit",
            ButtonKind::Primary,
            Vec2::new(ICON_SIZE * 2.0, BUTTON_HEIGHT),
        )
        .show(ui)
        {
            if let Err(err) = template::exit_isolation(app) {
                app.set_status(format!("Exit: {err}"));
            }
        }
    } else if Button::new(
        "Edit",
        ButtonKind::Normal,
        Vec2::new(ICON_SIZE * 2.0, BUTTON_HEIGHT),
    )
    .show(ui)
    {
        if let Some(idx) = app.selected_template_idx {
            if let Err(err) = template::enter_isolation(app, idx) {
                app.set_status(format!("Edit: {err}"));
            }
        } else {
            app.set_status("Select a template first".to_string());
        }
    }

    let path = template::breadcrumb_path(app).join(" > ");
    ui.label(egui::RichText::new(path).color(TEXT_DIM).size(FONT_SMALL));
}

fn draw_app_template_drawer(ctx: &egui::Context, screen_rect: egui::Rect, app: &mut AppState) {
    let mut items: Vec<(String, usize, bool)> = app
        .chart
        .templates
        .iter()
        .enumerate()
        .map(|(idx, tpl)| {
            let instances = app
                .chart
                .template_instances
                .iter()
                .filter(|inst| inst.template_id == tpl.id)
                .count();
            (
                format!("{}  v{}", tpl.name, tpl.version),
                instances,
                app.selected_template_idx == Some(idx),
            )
        })
        .collect();

    let mut borrowed: Vec<(&str, usize, bool)> = items
        .iter_mut()
        .map(|(name, count, selected)| (name.as_str(), *count, *selected))
        .collect();

    let mut create_new = false;
    let mut selected_template = None;
    crate::app::egui_components::draw_drawer(
        ctx,
        screen_rect,
        "app_templates_drawer",
        "Templates",
        &mut borrowed,
        &mut |idx| {
            selected_template = Some(idx);
        },
        &mut || {
            create_new = true;
        },
    );

    if let Some(idx) = selected_template {
        if idx < app.chart.templates.len() {
            app.selected_template_idx = Some(idx);
        }
    }

    if create_new {
        let name = format!("Template {}", app.next_template_id);
        match template::create_new_template(app, &name) {
            Ok(_) => app.selected_template_idx = Some(app.chart.templates.len().saturating_sub(1)),
            Err(err) => app.set_status(format!("New template: {err}")),
        }
    }
}

fn draw_app_scene_drawer(ctx: &egui::Context, timeline_rect: egui::Rect, app: &mut AppState) {
    if timeline_rect == egui::Rect::NOTHING {
        return;
    }
    if !unsafe { APP_SCENE_DRAWER_OPEN } {
        return;
    }
    draw_scene_overlay(ctx, timeline_rect, app);
}

fn draw_app_scene_toggle_button(ui: &mut egui::Ui, timeline_rect: egui::Rect) {
    if unsafe { APP_SCENE_DRAWER_OPEN } {
        return;
    }

    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = BUTTON_HEIGHT * 1.2;
    let btn_rect = egui::Rect::from_min_size(
        egui::Pos2::new(timeline_rect.right() - btn_w, timeline_rect.top() + PADDING),
        egui::Vec2::new(btn_w, btn_h),
    );

    let resp = ui.allocate_rect(btn_rect, egui::Sense::click());
    let bg = if resp.hovered() {
        BG_BUTTON_HOVER
    } else {
        BG_BUTTON
    };
    ui.painter()
        .rect_filled(btn_rect, egui::CornerRadius::same(4), bg);
    ui.painter().rect_stroke(
        btn_rect,
        egui::CornerRadius::same(4),
        Stroke::new(1.0, BUTTON_BORDER),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "<",
        egui::FontId::proportional(FONT_SMALL),
        TEXT_PRIMARY,
    );

    if resp.clicked() {
        unsafe {
            APP_SCENE_DRAWER_OPEN = true;
        }
    }
}

fn draw_scene_overlay(ctx: &egui::Context, timeline_rect: egui::Rect, app: &mut AppState) {
    let drawer_w = (timeline_rect.width() / 3.0).max(220.0);
    let drawer_h = (timeline_rect.height() * 0.62).max(260.0);
    let drawer_rect = egui::Rect::from_min_size(
        egui::Pos2::new(timeline_rect.right() - drawer_w, timeline_rect.top()),
        egui::Vec2::new(drawer_w, drawer_h),
    );

    egui::Area::new(egui::Id::new("app_scene_drawer"))
        .fixed_pos(drawer_rect.left_top())
        .show(ctx, |ui| {
            ui.set_min_size(drawer_rect.size());
            ui.painter().rect_filled(
                egui::Rect::from_min_size(egui::Pos2::ZERO, drawer_rect.size()),
                egui::CornerRadius::same(6),
                Color32::from_rgba_premultiplied(42, 42, 46, 210),
            );
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(egui::Pos2::ZERO, drawer_rect.size()),
                egui::CornerRadius::same(6),
                Stroke::new(1.0, BORDER_LIGHT),
                egui::StrokeKind::Outside,
            );

            ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(
                    egui::Pos2::new(PADDING, PADDING),
                    drawer_rect.size() - egui::Vec2::splat(PADDING * 2.0),
                ),
                |ui| {
                    section_header(ui, "Scene");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Close").clicked() {
                            unsafe {
                                APP_SCENE_DRAWER_OPEN = false;
                            }
                        }
                    });
                    ui.add_space(SPACING);

                    let scene_name = if template::is_in_isolation(app) {
                        template::current_template_name(app)
                            .unwrap_or_else(|| "Template".to_string())
                    } else {
                        "Main Chart".to_string()
                    };
                    value_row(ui, "Active", &scene_name);
                    value_row(ui, "Notes", &app.chart.notes.len().to_string());
                    value_row(ui, "Templates", &app.chart.templates.len().to_string());
                    value_row(
                        ui,
                        "Instances",
                        &app.chart.template_instances.len().to_string(),
                    );

                    ui.add_space(SPACING);
                    section_header(ui, "Note Groups");
                    note_group_row(
                        ui,
                        "Tap",
                        app.chart
                            .notes
                            .iter()
                            .filter(|n| matches!(n.note_type, super::types::NoteType::Tap))
                            .count(),
                    );
                    note_group_row(
                        ui,
                        "Hold",
                        app.chart
                            .notes
                            .iter()
                            .filter(|n| matches!(n.note_type, super::types::NoteType::Hold))
                            .count(),
                    );
                    note_group_row(
                        ui,
                        "Slide",
                        app.chart
                            .notes
                            .iter()
                            .filter(|n| matches!(n.note_type, super::types::NoteType::Slide))
                            .count(),
                    );
                    note_group_row(
                        ui,
                        "Touch",
                        app.chart
                            .notes
                            .iter()
                            .filter(|n| matches!(n.note_type, super::types::NoteType::Touch))
                            .count(),
                    );

                    ui.add_space(SPACING);
                    section_header(ui, "Templates");
                    if app.chart.templates.is_empty() {
                        ui.label(
                            egui::RichText::new("No templates")
                                .color(TEXT_DIM)
                                .size(FONT_SMALL),
                        );
                    } else {
                        let mut action: Option<(usize, &'static str)> = None;
                        for (idx, tpl) in app.chart.templates.iter().enumerate() {
                            let selected = app.selected_template_idx == Some(idx);
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        selected,
                                        egui::RichText::new(&tpl.name).color(TEXT_PRIMARY),
                                    )
                                    .clicked()
                                {
                                    action = Some((idx, "select"));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Edit").clicked() {
                                            action = Some((idx, "edit"));
                                        }
                                        if ui.small_button("Insert").clicked() {
                                            action = Some((idx, "insert"));
                                        }
                                    },
                                );
                            });
                        }
                        if let Some((idx, kind)) = action {
                            match kind {
                                "select" => app.selected_template_idx = Some(idx),
                                "edit" => {
                                    app.selected_template_idx = Some(idx);
                                    if let Err(err) = template::enter_isolation(app, idx) {
                                        app.set_status(format!("Edit: {err}"));
                                    }
                                }
                                "insert" => {
                                    app.selected_template_idx = Some(idx);
                                    if let Err(err) = template::insert_instance(app, idx) {
                                        app.set_status(format!("Insert: {err}"));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    ui.add_space(SPACING);
                    section_header(ui, "Instances");
                    if app.chart.template_instances.is_empty() {
                        ui.label(
                            egui::RichText::new("No instances")
                                .color(TEXT_DIM)
                                .size(FONT_SMALL),
                        );
                    } else {
                        let mut edit_instance = None;
                        for (idx, inst) in app.chart.template_instances.iter().enumerate() {
                            let name = app
                                .chart
                                .templates
                                .iter()
                                .find(|tpl| tpl.id == inst.template_id)
                                .map(|tpl| tpl.name.as_str())
                                .unwrap_or(inst.template_id.as_str());
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{name} @ m{:.2}",
                                        inst.anchor_time
                                    ))
                                    .color(TEXT_PRIMARY)
                                    .size(FONT_SMALL),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Edit").clicked() {
                                            edit_instance = Some(idx);
                                        }
                                    },
                                );
                            });
                        }
                        if let Some(idx) = edit_instance {
                            if let Err(err) = template::enter_instance_isolation(app, idx) {
                                app.set_status(format!("Edit instance: {err}"));
                            }
                        }
                    }

                    if template::is_in_isolation(app) {
                        ui.add_space(SPACING);
                        if Button::new(
                            "Exit Template Edit",
                            ButtonKind::Primary,
                            egui::Vec2::new(ui.available_width(), BUTTON_HEIGHT),
                        )
                        .show(ui)
                        {
                            if let Err(err) = template::exit_isolation(app) {
                                app.set_status(format!("Exit: {err}"));
                            }
                        }
                    }
                },
            );

            let input = ui.input(|i| (i.pointer.primary_clicked(), i.pointer.latest_pos()));
            if let (true, Some(pos)) = input {
                let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, drawer_rect.size());
                if !local_rect.contains(pos - drawer_rect.left_top().to_vec2()) {
                    unsafe {
                        APP_SCENE_DRAWER_OPEN = false;
                    }
                }
            }
        });
}

fn note_group_row(ui: &mut egui::Ui, label: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(TEXT_SECONDARY)
                .size(FONT_BODY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(count.to_string())
                    .color(TEXT_PRIMARY)
                    .size(FONT_BODY),
            );
        });
    });
}

// ── App sidebar with T/H/Star shape icons wired to PlaceTool ──

pub fn draw_app_sidebar(ui: &mut egui::Ui, app: &mut AppState) {
    egui::Frame::new()
        .fill(BG_SIDEBAR)
        .stroke(egui::Stroke::new(1.0, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;

                // ── Select (V) ──
                if icon_button(
                    ui,
                    "V",
                    matches!(app.placement, PlacementState::Idle) && app.selected_note.is_some(),
                ) {
                    app.placement = PlacementState::Idle;
                    app.set_status("Select mode".to_string());
                }

                // ── Tap (T) — pink circle shape ──
                if tool_shape_button(ui, ToolShape::Tap, app.place_tool == PlaceTool::Tap) {
                    app.place_tool = PlaceTool::Tap;
                    app.placement = PlacementState::Idle;
                    app.set_status("Tool: Tap".to_string());
                }

                // ── Hold (H) — pink rectangle shape ──
                if tool_shape_button(ui, ToolShape::Hold, app.place_tool == PlaceTool::Hold) {
                    app.place_tool = PlaceTool::Hold;
                    app.placement = PlacementState::Idle;
                    app.set_status("Tool: Hold".to_string());
                }

                // ── Slide (S) — placeholder ──
                if icon_button(ui, "S", false) {
                    app.set_status("Use Star tool to place slide notes".to_string());
                }

                // ── Touch (C) — placeholder ──
                if icon_button(ui, "C", false) {
                    app.set_status(
                        "Touch notes are placed by clicking the T lane with Tap/Hold".to_string(),
                    );
                }

                // ── Star (★) — yellow star shape ──
                if tool_shape_button(ui, ToolShape::Star, app.place_tool == PlaceTool::Star) {
                    app.place_tool = PlaceTool::Star;
                    app.placement = PlacementState::Idle;
                    app.set_status("Tool: Star / Slide".to_string());
                }

                // Separator
                ui.add_space(SPACING);
                let r = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [
                        egui::pos2(r.left() + SPACING, r.top()),
                        egui::pos2(r.right() - SPACING, r.top()),
                    ],
                    egui::Stroke::new(1.0, SEPARATOR),
                );
                ui.add_space(SPACING * 1.5);

                // Utility icons
                if icon_button(ui, "⚡", app.record_snap_grid) {
                    app.record_snap_grid = !app.record_snap_grid;
                }
                if icon_button(ui, "📏", app.show_pad_only) {
                    app.show_pad_only = !app.show_pad_only;
                }
                if icon_button(ui, "🔊", app.audio_enabled) {
                    app.audio_enabled = !app.audio_enabled;
                }

                // Settings at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if icon_button(ui, "📱", app.mobile_ui) {
                        app.mobile_ui = !app.mobile_ui;
                    }
                });
            });
        });
}

// ── Tool shape icons (Tap=circle, Hold=rect, Star=polygon) ──

enum ToolShape {
    Tap,
    Hold,
    Star,
}

fn tool_shape_button(ui: &mut egui::Ui, shape: ToolShape, active: bool) -> bool {
    let size = egui::Vec2::splat(ICON_SIZE);
    let bg = if active { ACCENT_BLUE } else { BG_BUTTON };

    let btn = ui.allocate_response(size, egui::Sense::click());
    let rect = btn.rect;

    // Background + border
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(5), bg);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(5),
        egui::Stroke::new(1.0, BUTTON_BORDER),
        egui::StrokeKind::Outside,
    );
    if btn.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(5),
            Color32::from_rgba_premultiplied(255, 255, 255, 10),
        );
    }

    let cx = rect.center().x;
    let cy = rect.center().y;
    let r = size.x * 0.3; // shape radius
    let tap_color = Color32::from_rgb(244, 114, 182);
    let hold_color = Color32::from_rgb(251, 113, 133);
    let star_color = Color32::from_rgb(250, 204, 21);
    let inactive_color = Color32::from_rgb(100, 100, 100);

    match shape {
        ToolShape::Tap => {
            let c = if active { tap_color } else { inactive_color };
            ui.painter().circle_filled(egui::Pos2::new(cx, cy), r, c);
        }
        ToolShape::Hold => {
            let c = if active { hold_color } else { inactive_color };
            let hw = r * 0.7;
            ui.painter().rect_filled(
                egui::Rect::from_center_size(egui::Pos2::new(cx, cy), egui::Vec2::new(hw, r * 2.0)),
                egui::CornerRadius::same(1),
                c,
            );
        }
        ToolShape::Star => {
            let c = if active { star_color } else { inactive_color };
            let pts: Vec<egui::Pos2> = (0..5)
                .map(|i| {
                    let angle =
                        -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                    egui::Pos2::new(cx + angle.cos() * r, cy + angle.sin() * r)
                })
                .collect();
            ui.painter()
                .add(egui::Shape::convex_polygon(pts, c, egui::Stroke::NONE));
        }
    }

    btn.clicked()
}
