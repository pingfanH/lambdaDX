use egui_macroquad::egui;
use lucide_icons::{Icon, LUCIDE_FONT_BYTES};
use std::sync::Arc;

use crate::lambdadx_effects::EffectInstance;

pub const PANEL_HEIGHT: f32 = 100.0;
const FRAME: f32 = 1.0 / 60.0;

pub struct PlaybackControls {
    pub paused: bool,
    pub speed: f32,
    pub looping: bool,
    scrubbing: bool,
}

impl PlaybackControls {
    pub fn new(paused: bool, speed: f32) -> Self {
        Self {
            paused,
            speed,
            looping: true,
            scrubbing: false,
        }
    }

    pub fn step(&mut self, playback: &mut EffectInstance, direction: f32, end: f32) {
        self.paused = true;
        playback.elapsed = seek_time(playback.elapsed + direction * FRAME, end);
    }

    pub fn advance(&mut self, playback: &mut EffectInstance, dt: f32, end: f32) {
        if !self.paused && !self.scrubbing {
            playback.update(dt.max(0.0) * self.speed);
            playback.elapsed = advance_time(playback.elapsed, 0.0, self.speed, end, self.looping);
            if !self.looping && playback.elapsed >= end {
                self.paused = true;
            }
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, playback: &mut EffectInstance, end: f32) {
        egui::TopBottomPanel::bottom("effect_playback")
            .exact_height(PANEL_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 29, 34))
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 182.0).max(80.0);
                ui.horizontal(|ui| {
                    ui.label("Time");
                    let progress = ui.add(
                        egui::Slider::new(&mut playback.elapsed, 0.0..=end)
                            .show_value(false)
                            .max_decimals(4)
                            .step_by(0.0001),
                    );
                    self.scrubbing = progress.dragged() || progress.is_pointer_button_down_on();
                    let time = ui.add(
                        egui::DragValue::new(&mut playback.elapsed)
                            .range(0.0..=end)
                            .speed(0.001)
                            .fixed_decimals(3)
                            .suffix(" s"),
                    );
                    if time.changed() {
                        self.paused = true;
                    }
                    ui.label(format!("/ {end:.3} s"));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if icon(ui, Icon::SkipBack, "Restart", false).clicked() {
                        playback.elapsed = 0.0;
                    }
                    if icon(ui, Icon::StepBack, "Previous frame", false).clicked() {
                        self.step(playback, -1.0, end);
                    }
                    if icon(
                        ui,
                        if self.paused { Icon::Play } else { Icon::Pause },
                        "Play / pause",
                        !self.paused,
                    )
                    .clicked()
                    {
                        if self.paused && playback.elapsed >= end {
                            playback.elapsed = 0.0;
                        }
                        self.paused = !self.paused;
                    }
                    if icon(ui, Icon::StepForward, "Next frame", false).clicked() {
                        self.step(playback, 1.0, end);
                    }
                    if icon(ui, Icon::Repeat2, "Loop", self.looping).clicked() {
                        self.looping = !self.looping;
                    }
                    ui.separator();
                    ui.label("Speed");
                    egui::ComboBox::from_id_salt("effect_speed")
                        .selected_text(format!("{:.2}x", self.speed))
                        .width(76.0)
                        .show_ui(ui, |ui| {
                            for speed in [0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0] {
                                ui.selectable_value(&mut self.speed, speed, format!("{speed:.2}x"));
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut self.speed)
                            .range(0.05..=4.0)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix("x"),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "Frame {}",
                            (playback.elapsed / FRAME).round() as u32
                        ));
                    });
                });
            });
    }
}

pub fn configure() {
    egui_macroquad::cfg(configure_context);
}

fn configure_context(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "effect_icons".into(),
        Arc::new(egui::FontData::from_static(LUCIDE_FONT_BYTES)),
    );
    fonts.families.insert(
        egui::FontFamily::Name("effect_icons".into()),
        vec!["effect_icons".into()],
    );
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.spacing.interact_size.y = 24.0;
    style.visuals = egui::Visuals::dark();
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(50, 124, 119);
    ctx.set_style(style);
}

fn icon(ui: &mut egui::Ui, symbol: Icon, tooltip: &str, active: bool) -> egui::Response {
    let text = egui::RichText::new(char::from(symbol).to_string()).font(egui::FontId::new(
        16.0,
        egui::FontFamily::Name("effect_icons".into()),
    ));
    ui.add_sized([28.0, 26.0], egui::Button::new(text).selected(active))
        .on_hover_text(tooltip)
}

fn seek_time(time: f32, end: f32) -> f32 {
    time.clamp(0.0, end)
}

fn advance_time(time: f32, dt: f32, speed: f32, end: f32, looping: bool) -> f32 {
    let next = time + dt.max(0.0) * speed;
    if next >= end && looping {
        next.rem_euclid(end)
    } else {
        seek_time(next, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect() -> EffectInstance {
        EffectInstance::new(&Default::default(), 1)
    }

    #[test]
    fn pause_scrub_step_and_end_stop_control_absolute_time() {
        let mut effect = effect();
        let mut controls = PlaybackControls::new(false, 0.25);
        controls.advance(&mut effect, 0.04, 1.5);
        assert!((effect.elapsed - 0.01).abs() < 1e-6);
        controls.scrubbing = true;
        controls.advance(&mut effect, 0.04, 1.5);
        assert!((effect.elapsed - 0.01).abs() < 1e-6);
        controls.scrubbing = false;
        controls.step(&mut effect, 1.0, 1.5);
        assert!(controls.paused);
        assert!((effect.elapsed - 0.01 - FRAME).abs() < 1e-6);
        controls.advance(&mut effect, 0.04, 1.5);
        assert!((effect.elapsed - 0.01 - FRAME).abs() < 1e-6);
        controls.looping = false;
        controls.paused = false;
        effect.elapsed = 1.49;
        controls.advance(&mut effect, 0.1, 1.5);
        assert_eq!(effect.elapsed, 1.5);
        assert!(controls.paused);
    }

    #[test]
    fn egui_progress_drag_changes_time_without_changing_play_state() {
        let ctx = egui::Context::default();
        configure_context(&ctx);
        let mut controls = PlaybackControls::new(false, 0.25);
        let mut effect = effect();
        let mut frame = |events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| controls.ui(ctx, &mut effect, 1.5));
            (effect.elapsed, controls.paused, controls.scrubbing)
        };
        frame(vec![]);
        let point = egui::pos2(580.0, 722.0);
        let (time, paused, scrubbing) = frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ]);
        assert!((0.5..1.0).contains(&time), "slider did not seek: {time}");
        assert!(!paused);
        assert!(scrubbing);
        let point = egui::pos2(900.0, 722.0);
        let (later, _, _) = frame(vec![egui::Event::PointerMoved(point)]);
        assert!(later > time + 0.2);
        let (_, paused, scrubbing) = frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }]);
        assert!(!paused);
        assert!(!scrubbing);
    }

    #[test]
    fn scrubbing_and_frame_step_clamp_to_range() {
        assert_eq!(seek_time(-FRAME, 1.5), 0.0);
        assert_eq!(seek_time(2.0, 1.5), 1.5);
        assert!((seek_time(0.1 + FRAME, 1.5) - 0.11666667).abs() < 1e-6);
    }

    #[test]
    fn speed_applies_only_to_clock_and_loop_keeps_remainder() {
        assert!((advance_time(0.1, 0.04, 0.25, 1.5, true) - 0.11).abs() < 1e-6);
        assert!((advance_time(1.49, 0.02, 2.0, 1.5, true) - 0.03).abs() < 1e-6);
        assert_eq!(advance_time(1.49, 0.02, 2.0, 1.5, false), 1.5);
    }
}
