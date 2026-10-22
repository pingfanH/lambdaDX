//! Center statistics are drawn below notes, using the same score snapshot as the HUD.

use macroquad::prelude::*;

use crate::app::params::{CenterCombo, CenterDisplay};
use crate::app::types::PadGeom;
use crate::player::{font, state::PadPreviewState};

#[derive(Debug, PartialEq)]
struct Stat {
    label: &'static str,
    value: String,
    size: f32,
}

fn statistics(app: &PadPreviewState) -> Vec<Stat> {
    let p = &app.params;
    let mut lines = Vec::with_capacity(2);
    if matches!(p.center_display, CenterDisplay::Achievement | CenterDisplay::Both) {
        let index = p.center_achievement.rate_index();
        let rate = app.acc_rates().map(|rates| rates[index].1).unwrap_or(0.0);
        lines.push(Stat { label: crate::app::params::CenterAchievement::ALL[index].1,
            value: format!("{rate:.4}%"), size: 34.0 * bounded_size(p.center_achievement_scale) });
    }
    if matches!(p.center_display, CenterDisplay::Combo | CenterDisplay::Both) {
        let (label, value) = match p.center_combo {
            CenterCombo::Combo => ("COMBO", app.combo()),
            CenterCombo::Perfect => ("PERFECT COMBO", app.p_combo()),
            CenterCombo::CriticalPerfect => ("CRITICAL PERFECT COMBO", app.c_p_combo()),
        };
        lines.push(Stat { label, value: value.to_string(),
            size: 42.0 * bounded_size(p.center_combo_scale) });
    }
    lines
}

fn bounded_size(value: f32) -> f32 {
    if value.is_finite() { value.clamp(0.3, 3.0) } else { 1.0 }
}

pub fn draw(app: &PadPreviewState, pad: PadGeom, scale: f32) {
    let lines = statistics(app);
    if lines.is_empty() { return; }
    let max_width = pad.outer_r * 0.95;
    let line_height = |line: &Stat| (line.size + 28.0) * scale;
    let height: f32 = lines.iter().map(line_height).sum();
    let fit = (pad.outer_r * 0.85 / height).min(1.0);
    let mut y = pad.cy - height * fit * 0.5;
    for line in lines {
        let label_size = (line.size * 0.30).clamp(10.0, 18.0) * scale * fit;
        let value_size = line.size * scale * fit;
        y += label_size;
        centered(line.label, pad.cx, y, label_size, max_width,
            Color::from_rgba(210, 218, 226, 230), scale);
        y += value_size + 4.0 * scale * fit;
        centered(&line.value, pad.cx, y, value_size, max_width, WHITE, scale);
        y += 12.0 * scale * fit;
    }
}

fn centered(text: &str, x: f32, y: f32, size: f32, max_width: f32, color: Color, scale: f32) {
    let width = font::text_width(text, size);
    let size = size * (max_width / width.max(1.0)).min(1.0);
    let x = x - font::text_width(text, size) * 0.5;
    font::text(text, x + scale, y + scale, size, Color::from_rgba(0, 0, 0, 190));
    font::text(text, x, y, size, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_sizes_work_without_a_score_or_graphics_context() {
        let mut app = crate::player::render::scale::tests::app();
        for (mode, _) in CenterDisplay::ALL {
            app.params.center_display = mode;
            let lines = statistics(&app);
            assert_eq!(lines.len(), match mode { CenterDisplay::Off => 0, CenterDisplay::Both => 2, _ => 1 });
        }
        app.params.center_display = CenterDisplay::Both;
        app.params.center_combo_scale = 0.5;
        app.params.center_achievement_scale = 2.0;
        let lines = statistics(&app);
        assert_eq!(lines[0].value, "0.0000%");
        assert_eq!(lines[0].size, 68.0);
        assert_eq!(lines[1].value, "0");
        assert_eq!(lines[1].size, 21.0);
    }

    #[test]
    fn invalid_sizes_cannot_break_layout() {
        assert_eq!(bounded_size(f32::NAN), 1.0);
        assert_eq!(bounded_size(f32::INFINITY), 1.0);
        assert_eq!(bounded_size(-10.0), 0.3);
        assert_eq!(bounded_size(100.0), 3.0);
    }
}
