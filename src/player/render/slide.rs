//! Slide notes. Thin adapter over `app::slide_render::draw_slide`, which builds
//! the sampled path, draws the trail tiles and flies the star.

use macroquad::math::Vec2;

use crate::app::slide::segmentation::{self, SlideSegmentation};
use crate::app::slide_render;
use crate::app::types::{
    PadGeom, SLIDE_MIN_DURATION_S, SLIDE_TILE_SPACING, mdur_to_secs, note_secs,
};
use crate::player::render::timing::NoteTiming;
use crate::player::state::PadPreviewState;

/// Draw every sub-slide of a slide note for the current time.
///
/// `ns` is the head time; each sub-slide has its own span
/// (`slide_duration`) and start delay (`slide_start_delay`), both in measures.
/// The delay is clamped to just under the span so a zero-delay slide still gets
/// a tiny fade-in window.
pub fn draw(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
    current_t: f32,
    t: &NoteTiming,
) {
    if note.slide.is_empty() {
        return;
    }
    let bpms = &app.chart.bpms;
    let ns = note_secs(note, bpms);
    let Some(ref svg) = app.pad_svg else {
        return;
    };

    for sl in note.slide.iter() {
        let slide_dur_s =
            mdur_to_secs(sl.slide_duration, note.time, bpms).max(SLIDE_MIN_DURATION_S);
        let fade_in_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(slide_dur_s - 0.001)
            .max(0.001);

        // Pick the correct trail/star variant for this note's flags.
        let dbl = note.is_star;
        let trail_tex = if sl.slide_is_break {
            app.slide_break_tex.as_ref()
        } else if note.is_each {
            app.slide_each_tex.as_ref()
        } else {
            app.slide_tex.as_ref()
        };
        let star_variant = if note.is_break {
            if dbl {
                app.star_double_break_tex.as_ref()
            } else {
                app.star_break_tex.as_ref()
            }
        } else if note.is_each {
            if dbl {
                app.star_double_each_tex.as_ref()
            } else {
                app.star_each_tex.as_ref()
            }
        } else if dbl {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let star_fb = if dbl {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let ex_variant = if note.is_ex {
            if dbl {
                app.star_double_ex_tex.as_ref()
            } else {
                app.star_ex_tex.as_ref()
            }
        } else {
            None
        };

        // `draw_slide` resolves the Ex overlay as
        // `star_ex.or(star_ex_fallback)`. That fallback must ONLY be offered
        // when the note really is Ex; otherwise every star head gets an Ex ring
        // (the upstream player had this bug). Resolve the variant/generic Ex
        // choice here and leave `star_ex_fallback` empty.
        let star_ex = if note.is_ex {
            ex_variant.or(app.star_ex_tex.as_ref())
        } else {
            None
        };

        let tex = slide_render::SlideTextures {
            trail: trail_tex,
            star: star_variant.or(star_fb),
            star_fallback: app.star_tex.as_ref(),
            star_ex,
            star_ex_fallback: None,
            wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
        };

        // Trail consumption: hide the trail the star has passed. Bars are
        // hidden **per sensor segment** (a run of bars in one zone), matching
        // the original player where the engine emitted `HideSlideBars
        // { end_index }` per judged segment — so several tiles vanish at once
        // instead of one tile at a time.
        let star_start_s = ns + fade_in_s;
        let travel_s = (slide_dur_s - fade_in_s).max(SLIDE_MIN_DURATION_S);
        let star_t = if current_t <= star_start_s {
            0.0
        } else {
            ((current_t - star_start_s) / travel_s).clamp(0.0, 1.0)
        };
        let spacing = SLIDE_TILE_SPACING * scale;
        let path = slide_render::build_slide_path(note, sl, pad, svg, scale, spawn_cx, outer_r);
        let seg = segmentation::build(&path, spacing, svg, pad);
        let total_len: f32 = path.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        let hidden_until_bar = hidden_bars_for_star(&seg, star_t * total_len, spacing);

        slide_render::draw_slide(
            note,
            sl,
            current_t,
            ns,
            slide_dur_s,
            fade_in_s,
            pad,
            svg,
            scale,
            spawn_cx,
            outer_r,
            &tex,
            false,
            t.speed_scale,
            app.note_speed,
            app.slide_fade_in,
            hidden_until_bar,
        );
    }
}

/// Number of trail bars to hide as the star advances, **grouped by sensor
/// segment**.
///
/// `star_dist` is the star's distance along the path. Every judge segment that
/// lies entirely behind the star is hidden at once, so the trail disappears in
/// chunks (a whole zone's worth of tiles) rather than tile by tile.
fn hidden_bars_for_star(seg: &SlideSegmentation, star_dist: f32, spacing: f32) -> usize {
    if seg.bars.is_empty() {
        return 0;
    }
    let last = seg.bars.len() - 1;
    let bar_idx = ((star_dist / spacing.max(1.0)).floor() as usize).min(last);
    let mut hidden = 0;
    for s in &seg.judge_segments {
        // Hide the segment once the star reaches its last bar (so the final
        // segment still clears when the star lands on the tail).
        if bar_idx + 1 >= s.end_bar {
            hidden = s.end_bar;
        } else {
            break;
        }
    }
    hidden.min(seg.bars.len())
}

#[cfg(test)]
mod tests {
    use super::hidden_bars_for_star;
    use crate::app::slide::segmentation::{SlideBar, SlideJudgeSegment, SlideSegmentation};
    use crate::app::types::zone::PadZone;
    use macroquad::math::vec2;

    fn sample_segmentation() -> SlideSegmentation {
        let bars = (0..10)
            .map(|i| SlideBar {
                position: vec2(i as f32, 0.0),
                rotation: 0.0,
                zone: None,
            })
            .collect();
        SlideSegmentation {
            bars,
            judge_segments: vec![
                SlideJudgeSegment {
                    zone: PadZone::A1,
                    start_bar: 0,
                    end_bar: 3,
                },
                SlideJudgeSegment {
                    zone: PadZone::A2,
                    start_bar: 3,
                    end_bar: 7,
                },
                SlideJudgeSegment {
                    zone: PadZone::A3,
                    start_bar: 7,
                    end_bar: 10,
                },
            ],
        }
    }

    #[test]
    fn trail_hides_in_segment_chunks() {
        let seg = sample_segmentation();
        // At the head nothing is consumed.
        assert_eq!(hidden_bars_for_star(&seg, 0.0, 1.0), 0);
        // Mid first segment: still nothing fully passed.
        assert_eq!(hidden_bars_for_star(&seg, 1.0, 1.0), 0);
        // Star reaches bar 3: the whole first segment (3 tiles) hides at once.
        assert_eq!(hidden_bars_for_star(&seg, 3.0, 1.0), 3);
        // Inside the second segment: unchanged (no per-tile hiding).
        assert_eq!(hidden_bars_for_star(&seg, 5.0, 1.0), 3);
        // Star reaches bar 7: second segment hides, cumulative 0..7.
        assert_eq!(hidden_bars_for_star(&seg, 7.0, 1.0), 7);
        // Star at the end: everything hidden.
        assert_eq!(hidden_bars_for_star(&seg, 10.0, 1.0), 10);
    }

    #[test]
    fn empty_segmentation_is_safe() {
        let seg = SlideSegmentation::default();
        assert_eq!(hidden_bars_for_star(&seg, 5.0, 2.0), 0);
    }
}
