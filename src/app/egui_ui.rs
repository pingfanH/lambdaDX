use egui_macroquad::egui::{self, TopBottomPanel};

use super::chart;
use super::simai_io;
use super::state::AppState;
use super::types::Mode;

/// Draw egui toolbar on top. Pad + timeline are native macroquad below.
pub(crate) fn draw_egui_ui(ctx: &egui::Context, app: &mut AppState) {
    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Mai2Chart");
            ui.separator();
            let mode_label = match app.mode {
                Mode::Idle => "IDLE", Mode::Recording => "REC", Mode::Playing => "PLAY",
            };
            ui.label(egui::RichText::new(mode_label).strong());

            if ui.button("▶ Play").clicked() { app.toggle_play(); }
            if ui.button("⏺ Rec").clicked() { app.toggle_record(); }
            ui.separator();
            if ui.button("💾 Save").clicked() {
                match chart::save_recording_doc(app) {
                    Ok(p) => app.status = format!("Saved {}", p.display()),
                    Err(e) => app.status = format!("Save: {e}"),
                }
            }
            if ui.button("📂 Load").clicked() {
                match chart::load_latest_saved_chart() {
                    Ok(c) => { let n = c.notes.len(); app.chart = c; app.status = format!("{n} notes"); }
                    Err(e) => app.status = format!("Load: {e}"),
                }
            }
            if ui.button("⬇ Simai")
                .on_hover_text("Import maidata.txt or chart body from output/import.simai")
                .clicked()
            {
                match simai_io::import_from_simai_path("import.simai") {
                    Ok(c) => {
                        let n = c.notes.len();
                        app.chart = c;
                        app.selected_note = None;
                        app.editing_slide_path = None;
                        app.status = format!("Imported Simai: {n} notes");
                    }
                    Err(e) => app.status = format!("Import Simai: {e}"),
                }
            }
            if ui.button("⬆ Simai")
                .on_hover_text("Export current chart to output/export.simai (Simai format)")
                .clicked()
            {
                match simai_io::export_to_simai_path(&app.chart, "export.simai") {
                    Ok(p) => app.status = format!("Exported Simai: {}", p.display()),
                    Err(e) => app.status = format!("Export Simai: {e}"),
                }
            }
            if ui.button("🗑 Clear").clicked() {
                app.recording_hits.clear(); app.recording_notes.clear();
                app.active_record_holds.clear(); app.active_pointer_zones.clear();
                app.prev_pointer_pos.clear(); app.status = "Cleared".to_string();
            }
            ui.separator();
            if ui.button(if app.record_snap_grid { "Grid ON" } else { "Grid OFF" }).clicked() { app.record_snap_grid = !app.record_snap_grid; }
            ui.label("Rec:");
            if ui.button("-").clicked() { app.set_record_speed((app.record_speed - 0.1).max(0.1)); }
            ui.label(format!("{:.1}x", app.record_speed));
            if ui.button("+").clicked() { app.set_record_speed((app.record_speed + 0.1).min(3.0)); }
            ui.label("Play:");
            if ui.button("- ").clicked() { app.set_play_speed((app.play_speed - 0.1).max(0.1)); }
            ui.label(format!("{:.1}x", app.play_speed));
            if ui.button("+ ").clicked() { app.set_play_speed((app.play_speed + 0.1).min(3.0)); }
            ui.separator();
            if ui.button(if app.audio_enabled { "🔊" } else { "🔇" }).clicked() { app.audio_enabled = !app.audio_enabled; }
            if ui.button("📱").clicked() { app.mobile_ui = !app.mobile_ui; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(&app.status);
            });
        });
    });
}
