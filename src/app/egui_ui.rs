use std::sync::{Arc, Once};
use egui_macroquad::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontId, FontFamily, Margin, Stroke, TopBottomPanel, Vec2};

use super::chart;
use super::simai_io;
use super::state::AppState;
use super::template;
use super::types::Mode;

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
                fonts.font_data.insert(
                    "system".to_owned(),
                    Arc::new(FontData::from_owned(data)),
                );
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
        (egui::TextStyle::Heading, FontId::new(16.0, FontFamily::Proportional)),
        (egui::TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (egui::TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (egui::TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (egui::TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ].into();

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
    let btn = egui::Button::new(
        egui::RichText::new(label).color(if active { Color32::WHITE } else { TEXT })
    ).fill(if active { BG_ACTIVE } else { BG_WIDGET });
    ui.add(btn).clicked()
}

fn step_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let btn = egui::Button::new(
        egui::RichText::new(label).color(TEXT)
    ).min_size(Vec2::new(20.0, 20.0));
    ui.add(btn)
}

/// Draw egui toolbar on top. Pad + timeline are native macroquad below.
pub fn draw_egui_ui(ctx: &egui::Context, app: &mut AppState) {
    apply_blender_style(ctx);

    TopBottomPanel::top("toolbar")
        .frame(egui::Frame::none()
            .fill(BG_DARK)
            .inner_margin(Margin::symmetric(8, 4))
            .stroke(Stroke::new(1.0, BORDER)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // ── App title ──
                ui.label(egui::RichText::new("LambdaDX Demo")
                    .font(FontId::new(16.0, FontFamily::Proportional))
                    .color(ACCENT_ORANGE).strong());
                section_sep(ui);

                // ── Mode indicator ──
                let (mode_text, mode_color) = match app.mode {
                    Mode::Idle => ("IDLE", TEXT_DIM),
                    Mode::Recording => ("REC", ACCENT_ORANGE),
                    Mode::Playing => ("PLAY", ACCENT_BLUE),
                };
                ui.label(egui::RichText::new(mode_text)
                    .font(FontId::new(13.0, FontFamily::Monospace))
                    .color(mode_color).strong());
                section_sep(ui);

                // ── Transport ──
                section_label(ui, "Transport");
                if toggle_btn(ui, "Play", app.mode == Mode::Playing) { app.toggle_play(); }
                if toggle_btn(ui, "Rec", app.mode == Mode::Recording) { app.toggle_record(); }
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
                        Ok(c) => { let n = c.notes.len(); app.set_chart(c); app.set_status(format!("{n} notes")); }
                        Err(e) => app.set_status(format!("Load: {e}")),
                    }
                }
                section_sep(ui);

                // ── Simai ──
                section_label(ui, "Simai");
                if ui.button(egui::RichText::new("Import").color(TEXT))
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
                if ui.button(egui::RichText::new("Export").color(TEXT))
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
                if ui.button(egui::RichText::new("Clear").color(TEXT)).clicked() {
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
                if step_btn(ui, "-").clicked() { app.set_record_speed((app.record_speed - 0.1).max(0.1)); }
                ui.label(egui::RichText::new(format!("{:.1}x", app.record_speed)).color(TEXT));
                if step_btn(ui, "+").clicked() { app.set_record_speed((app.record_speed + 0.1).min(3.0)); }

                ui.add_space(4.0);
                ui.label(egui::RichText::new("Play").color(TEXT_DIM));
                if step_btn(ui, "-").clicked() { app.set_play_speed((app.play_speed - 0.1).max(0.1)); }
                ui.label(egui::RichText::new(format!("{:.1}x", app.play_speed)).color(TEXT));
                if step_btn(ui, "+").clicked() { app.set_play_speed((app.play_speed + 0.1).min(3.0)); }
                section_sep(ui);

                // ── Offset ──
                section_label(ui, "Offset");
                if step_btn(ui, "-").on_hover_text("Shift audio earlier (-50ms)").clicked() {
                    app.chart.audio_offset = (app.chart.audio_offset - 0.05).max(-5.0);
                    app.audio_cache.clear();
                    if matches!(app.mode, Mode::Playing) { app.request_audio_start(); }
                    app.set_status(format!("Offset: {:.3}s", app.chart.audio_offset));
                }
                ui.label(egui::RichText::new(format!("{:.2}s", app.chart.audio_offset)).color(TEXT));
                if step_btn(ui, "+").on_hover_text("Shift audio later (+50ms)").clicked() {
                    app.chart.audio_offset = (app.chart.audio_offset + 0.05).min(30.0);
                    app.audio_cache.clear();
                    if matches!(app.mode, Mode::Playing) { app.request_audio_start(); }
                    app.set_status(format!("Offset: {:.3}s", app.chart.audio_offset));
                }
                section_sep(ui);

                // ── Toggles ──
                if toggle_btn(ui, if app.audio_enabled { "Audio" } else { "Muted" }, app.audio_enabled) {
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
        .frame(egui::Frame::none()
            .fill(BG_DARK)
            .inner_margin(Margin::symmetric(8, 3))
            .stroke(Stroke::new(0.5, BORDER)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                section_label(ui, "Template");

                // New empty template → enters isolation directly.
                if ui.button(egui::RichText::new("New").color(TEXT))
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
                if ui.button(egui::RichText::new("New from Sel").color(TEXT))
                    .on_hover_text("Create template from selected notes")
                    .clicked()
                {
                    let name = format!("Template {}", app.next_template_id);
                    match template::create_template(app, &name) {
                        Ok(id) => {
                            app.selected_template_idx = app.chart.templates.iter().position(|t| t.id == id);
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
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let path = template::breadcrumb_path(app);
                            for (i, name) in path.iter().enumerate().rev() {
                                if i < path.len() - 1 {
                                    ui.label(
                                        egui::RichText::new(">").color(TEXT_DIM).small(),
                                    );
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
                        },
                    );
                }
            });
        });

    // Store the approximate toolbar bottom for input blocking.
    // The toolbar has 2 panels (main + template bar), each ~30px, plus margins.
    app.egui_toolbar_bottom = 75.0;
}
