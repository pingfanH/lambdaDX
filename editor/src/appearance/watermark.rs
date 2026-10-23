use egui_macroquad::egui;
use macroquad::prelude as mq;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Watermark {
    pub enabled: bool,
    pub text: String,
    color: [u8; 3],
    opacity: f32,
    font_size: f32,
    angle: f32,
    spacing: [f32; 2],
}

impl Default for Watermark {
    fn default() -> Self {
        Self {
            enabled: true,
            text: "禁止泄漏! Don't Leak".into(),
            color: [255; 3],
            opacity: 18.0,
            font_size: 32.0,
            angle: -25.0,
            spacing: [100.0, 64.0],
        }
    }
}

impl Watermark {
    pub fn normalize(&mut self) {
        let defaults = Self::default();
        for (value, min, max, fallback) in [
            (&mut self.opacity, 0.0, 100.0, defaults.opacity),
            (&mut self.font_size, 12.0, 96.0, defaults.font_size),
            (&mut self.angle, -90.0, 90.0, defaults.angle),
            (&mut self.spacing[0], 16.0, 640.0, defaults.spacing[0]),
        ] {
            *value = if value.is_finite() {
                value.clamp(min, max)
            } else {
                fallback
            };
        }
        self.spacing[1] = if self.spacing[1].is_finite() {
            self.spacing[1].clamp(16.0, 480.0)
        } else {
            defaults.spacing[1]
        };
        self.text = self
            .text
            .chars()
            .take(160)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.enabled, "背景水印");
            if crate::ui::style::icon(ui, lucide_icons::Icon::RotateCcw, "恢复默认水印", false)
                .clicked()
            {
                *self = Self::default();
            }
        });
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.text)
                    .hint_text("水印文字")
                    .char_limit(160)
                    .desired_width(ui.available_width()),
            );
            ui.horizontal(|ui| {
                ui.label("水印颜色");
                ui.color_edit_button_srgb(&mut self.color);
            });
            for (value, range, label, suffix) in [
                (&mut self.opacity, 0.0..=100.0, "水印不透明度", "%"),
                (&mut self.font_size, 12.0..=96.0, "水印字号", ""),
                (&mut self.angle, -90.0..=90.0, "水印角度", "°"),
                (&mut self.spacing[0], 16.0..=640.0, "水印横向间距", ""),
            ] {
                ui.add(egui::Slider::new(value, range).text(label).suffix(suffix));
            }
            ui.add(egui::Slider::new(&mut self.spacing[1], 16.0..=480.0).text("水印纵向间距"));
        });
    }

    pub fn draw(&self, screen: mq::Vec2) {
        if !self.enabled || self.text.trim().is_empty() || self.opacity <= 0.0 {
            return;
        }
        let font = crate::player::font::handle();
        let font_size = self.font_size.round() as u16;
        let size = mq::measure_text(&self.text, font.as_ref(), font_size, 1.0);
        let rotation = self.angle.to_radians();
        let layout = TileLayout::new(
            screen,
            mq::vec2(size.width, size.height),
            rotation,
            self.spacing,
        );
        let color = mq::Color::new(
            self.color[0] as f32 / 255.0,
            self.color[1] as f32 / 255.0,
            self.color[2] as f32 / 255.0,
            self.opacity / 100.0,
        );
        let (sin, cos) = rotation.sin_cos();
        // Center the actual glyph bounds, not the unrotated baseline.
        let local = mq::vec2(-size.width * 0.5, size.offset_y - size.height * 0.5);
        let baseline = mq::vec2(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
        for row in 0..layout.rows {
            for col in 0..layout.columns {
                let pos = layout.center(row, col) + baseline;
                mq::draw_text_ex(
                    &self.text,
                    pos.x,
                    pos.y,
                    mq::TextParams {
                        font: font.as_ref(),
                        font_size,
                        rotation,
                        color,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

struct TileLayout {
    step: mq::Vec2,
    origin: mq::Vec2,
    rows: usize,
    columns: usize,
}

impl TileLayout {
    fn new(screen: mq::Vec2, text: mq::Vec2, angle: f32, spacing: [f32; 2]) -> Self {
        let (sin, cos) = angle.sin_cos();
        let bounds = mq::vec2(
            text.x * cos.abs() + text.y * sin.abs(),
            text.x * sin.abs() + text.y * cos.abs(),
        );
        let step = bounds + mq::vec2(spacing[0], spacing[1]);
        Self {
            step,
            origin: mq::vec2(screen.x * 0.5, screen.y * 0.5),
            rows: (screen.y / step.y).ceil() as usize + 3,
            columns: (screen.x / step.x).ceil() as usize + 3,
        }
    }

    fn center(&self, row: usize, col: usize) -> mq::Vec2 {
        self.origin
            + mq::vec2(
                (col as f32 - (self.columns / 2) as f32 + (row % 2) as f32 * 0.5) * self.step.x,
                (row as f32 - (self.rows / 2) as f32) * self.step.y,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_settings_keep_defaults_and_roundtrip_custom_values() {
        let mut watermark: Watermark = serde_json::from_str(
            r#"{"text":"Private","color":[240,120,90],"angle":35,"spacing":[30,40]}"#,
        )
        .unwrap();
        assert!(watermark.enabled);
        assert_eq!(watermark.font_size, 32.0);
        watermark.enabled = false;
        let restored: Watermark =
            serde_json::from_str(&serde_json::to_string(&watermark).unwrap()).unwrap();
        assert_eq!(restored, watermark);
    }

    #[test]
    fn malformed_values_are_bounded_but_empty_text_is_preserved() {
        let mut watermark = Watermark {
            opacity: f32::NAN,
            font_size: 0.0,
            angle: 999.0,
            spacing: [f32::INFINITY, -20.0],
            text: String::new(),
            ..Default::default()
        };
        watermark.normalize();
        assert_eq!(watermark.opacity, Watermark::default().opacity);
        assert_eq!(watermark.font_size, 12.0);
        assert_eq!(watermark.angle, 90.0);
        assert_eq!(watermark.spacing, [100.0, 16.0]);
        assert!(watermark.text.is_empty());
        watermark.text = "水\n".repeat(200);
        watermark.normalize();
        assert_eq!(watermark.text.chars().count(), 160);
        assert!(!watermark.text.contains('\n'));
    }

    #[test]
    fn rotated_tiles_stay_separated_and_cover_small_and_large_windows() {
        for screen in [
            mq::vec2(640.0, 480.0),
            mq::vec2(1600.0, 900.0),
            mq::vec2(3840.0, 2160.0),
        ] {
            for angle in [-90.0_f32, -25.0, 0.0, 90.0] {
                let text = mq::vec2(320.0, 32.0);
                let layout = TileLayout::new(screen, text, angle.to_radians(), [100.0, 64.0]);
                let (sin, cos) = angle.to_radians().sin_cos();
                assert!(layout.step.x > text.x * cos.abs() + text.y * sin.abs());
                assert!(layout.step.y > text.x * sin.abs() + text.y * cos.abs());
                assert!(layout.center(0, 0).x < 0.0);
                assert!(layout.center(0, 0).y < 0.0);
                let last = layout.center(layout.rows - 1, layout.columns - 1);
                assert!(last.x >= screen.x && last.y >= screen.y);
                assert!(layout.rows * layout.columns < 1000);
            }
        }
    }
}
