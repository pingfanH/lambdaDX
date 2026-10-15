//! Judgment-time bands shared by both timeline orientations.

use super::geometry::{lane_index_of, zone_lane_index};
use crate::app::types::{ChartDoc, NoteType, hold_tail_time, note_is_touch, note_secs};
use crate::player::{judge_windows as windows, state::PadPreviewState};
use macroquad::prelude::*;

pub struct Axis {
    pub plot: Rect,
    pub vertical: bool,
    pub origin: f32,
    pub reference: f32,
    pub pps: f32,
    pub lane_size: f32,
}

impl Axis {
    fn time(&self, seconds: f32) -> f32 {
        self.origin + (seconds - self.reference) * self.pps * if self.vertical { -1.0 } else { 1.0 }
    }
    fn lane(&self, index: usize) -> f32 {
        (if self.vertical {
            self.plot.x
        } else {
            self.plot.y
        }) + (index as f32 + 0.5) * self.lane_size
    }
    fn rect(&self, lane: f32, width: f32, from: f32, to: f32) -> Rect {
        let a = self.time(from);
        let b = self.time(to);
        if self.vertical {
            Rect::new(lane - width / 2.0, a.min(b), width, (a - b).abs().max(1.0))
        } else {
            Rect::new(a.min(b), lane - width / 2.0, (a - b).abs().max(1.0), width)
        }
    }
    fn fill(&self, rect: Rect, color: Color) {
        let x = rect.x.max(self.plot.x);
        let y = rect.y.max(self.plot.y);
        let right = rect.right().min(self.plot.right());
        let bottom = rect.bottom().min(self.plot.bottom());
        if right > x && bottom > y {
            draw_rectangle(x, y, right - x, bottom - y, color);
        }
    }
    fn bands(&self, lane: f32, at: f32, ranges: &[windows::Interval], first_grade: usize) {
        let width = (self.lane_size * 0.7).min(56.0);
        for (i, interval) in ranges.iter().enumerate() {
            self.fill(
                self.rect(lane, width, at + interval.early, at + interval.late),
                windows::color(i + first_grade, 120),
            );
        }
        self.fill(self.rect(lane, width, at, at), WHITE);
    }
}

pub fn draw(app: &PadPreviewState, chart: &ChartDoc, axis: Axis) {
    if !crate::app::params::get().show_judge_windows {
        return;
    }
    let [r, g, b] = windows::HOLD_COLOR;
    let hold_color = Color::from_rgba(r, g, b, 220);
    for note in &chart.notes {
        let head = note_secs(note, &chart.bpms);
        let lane = axis.lane(lane_index_of(note));
        if note.note_type != NoteType::Slide || !note.is_tapless {
            axis.bands(lane, head, &windows::head(note), 0);
        }
        if matches!(note.note_type, NoteType::Hold | NoteType::TouchHold) {
            let tail = hold_tail_time(note, &chart.bpms);
            if app
                .judge_windows
                .classic_holds
                .get(&note.id)
                .copied()
                .unwrap_or(false)
            {
                axis.bands(lane, tail, &[windows::classic_hold_tail()], 2);
            } else if let Some(body) =
                windows::hold_requirement(head, tail - head, note_is_touch(note))
            {
                let offset =
                    (22.0 * crate::app::params::get().editor_note_scale).min(axis.lane_size * 0.4);
                axis.fill(
                    axis.rect(lane + offset, 4.0, body.body_start, body.perfect_release),
                    hold_color,
                );
                axis.fill(
                    axis.rect(
                        lane + offset,
                        14.0,
                        body.perfect_release,
                        body.perfect_release,
                    ),
                    hold_color,
                );
            }
        }
        for (sub, slide) in note.slide.iter().enumerate() {
            let Some(window) = app.judge_windows.slides.get(&(note.id, sub)) else {
                continue;
            };
            let endpoint = slide
                .segments
                .iter()
                .flat_map(|s| &s.points)
                .last()
                .map(|p| p.zone.to_id())
                .unwrap_or(note.lane);
            axis.bands(
                axis.lane(zone_lane_index(endpoint)),
                window.judge,
                &window.windows,
                1,
            );
        }
    }
}
