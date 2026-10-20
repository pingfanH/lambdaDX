//! Slide notes. Thin adapter over `app::slide_render::draw_slide`, which builds
//! the sampled path, draws the trail tiles and flies the star.

use macroquad::color::Color;
use macroquad::math::Vec2;

use crate::app::slide::segmentation::{self, SlideSegmentation};
use crate::app::slide_render::{self, SlideLayer};
use crate::app::types::{
    Note, NoteType, PadGeom, SLIDE_MIN_DURATION_S, Slide, SlideShape, mdur_to_secs, note_secs,
};
use crate::player::render::timing::NoteTiming;
use crate::player::render::skin;
use crate::player::state::{PadPreviewState, SlideJudgeGrade};
use crate::app::params;

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
    layer: SlideLayer,
) {
    if note.slide.is_empty() {
        return;
    }
    let bpms = &app.chart.bpms;
    let ns = note_secs(note, bpms);
    let Some(ref svg) = app.pad_svg else {
        return;
    };

    // Sub-slide order within the note (multi-slide / chained slides).
    let subs = preview_subslides(note, app.edit_solo_slide, params::slide_sub_reverse());

    for (si, sl) in subs {
        let rendered = slide_render::render_slide_legs(sl);
        let sl = rendered.as_ref();
        let slide_dur_s =
            mdur_to_secs(sl.slide_duration, note.time, bpms).max(SLIDE_MIN_DURATION_S);
        let fade_in_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(slide_dur_s - 0.001)
            .max(0.001);

        // Pick the correct trail/star variant for this note's flags (central
        // skin table in `render::skin`).
        let trail_tex = skin::body_or_normal(
            app,
            skin::SkinKind::SlideTrail,
            skin::SkinVariant::of_flags(sl.slide_is_break, note.is_each),
        );
        let star_variant = skin::star_body(app, note);
        let star_fb = skin::body(app, skin::star_kind(note), skin::SkinVariant::Normal);
        let star_ex = skin::star_ex(app, note);

        // The star guide inherits the **tap** guide (same texture/variant), so a
        // slide head looks like a tap.
        let head_each = skin::SkinVariant::of_flags(note.is_break, note.is_each_head);
        let guide = match head_each {
            skin::SkinVariant::Each => {
                app.tap_guide_each_tex.as_ref().or(app.tap_guide_tex.as_ref())
            }
            skin::SkinVariant::Break => {
                app.tap_guide_break_tex.as_ref().or(app.tap_guide_tex.as_ref())
            }
            skin::SkinVariant::Normal => app.tap_guide_tex.as_ref(),
        };

        let tex = slide_render::SlideTextures {
            trail: trail_tex,
            star: star_variant.or(star_fb),
            star_fallback: app.star_tex.as_ref(),
            star_ex,
            star_ex_fallback: None,
            wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
            guide,
        };

        // Trail consumption is driven by lnmai-core's render commands, mapped
        // per runtime arc into the sub-slide's segment ranges and stored in
        // `slide_progress`. Without an engine the trail is fully drawn.
        let core_driven = app.use_core() && app.edit_solo_slide.is_none();
        let progress = if core_driven {
            app.slide_progress.get(&(note.id, si))
        } else {
            None
        };
        // Wifi hides each of its three tracks independently: hand `draw_slide`
        // the per-track fractions (falling back to the sub-slide's overall one).
        let mut track_fracs = [0.0_f32; 3];
        let mut track_bars: [Option<usize>; 3] = [None; 3];
        let seg_frac: &[f32] = if let Some(progress) = progress {
            let is_wifi = sl
                .segments
                .iter()
                .any(|s| matches!(s.shape, SlideShape::Wifi));
            if is_wifi {
                // Overall progress carries `HideAllSlideBars` (all tracks done)
                // and is also the fallback when a track has no separate update.
                let overall = progress.seg_frac.first().copied().unwrap_or(0.0);
                track_fracs = std::array::from_fn(|j| {
                    progress
                        .track_frac
                        .get(&(j as u64))
                        .copied()
                        .unwrap_or(0.0)
                        .max(overall)
                });
                // The visible wifi ribbon uses the middle track's texture. Core
                // reports `traveled` as the amount still represented by each
                // track, so the longest track is the one with the smallest
                // value. Use that track for the shared visual progress.
                let longest = track_fracs.iter().copied().fold(1.0, f32::min);
                track_fracs[1] = longest;
                track_bars =
                    std::array::from_fn(|j| progress.track_hidden_until.get(&(j as u64)).copied());
                if let Some(longest_bar) = track_bars.iter().flatten().copied().min() {
                    track_bars[1] = Some(longest_bar);
                }
                &track_fracs
            } else {
                progress.seg_frac.as_slice()
            }
        } else {
            &[]
        };
        // Every segment consumed ⇒ lnmai-core owns it and reports it done.
        if core_driven && !seg_frac.is_empty() && seg_frac.iter().all(|f| *f >= 1.0) {
            continue;
        }

        // A continuous chain is one sub-slide with several drawable legs. Draw
        // its trail **per arc** so the arcs are separated by the head/tail gap
        // (one seamless ribbon otherwise). The star layer is untouched: it still
        // draws a single star over the whole chain.
        if layer == SlideLayer::Trail && sl.segments.len() > 1 {
            let mut seg_note = note.clone();
            for (seg_idx, segment) in sl.segments.iter().enumerate() {
                let seg_slide = Slide {
                    segments: vec![segment.clone()],
                    slide_duration: sl.slide_duration,
                    slide_start_delay: sl.slide_start_delay,
                    slide_is_break: sl.slide_is_break,
                    runtime_parts: 1,
                };
                let seg_tex = slide_render::SlideTextures {
                    trail: trail_tex,
                    star: star_variant.or(star_fb),
                    star_fallback: app.star_tex.as_ref(),
                    star_ex,
                    star_ex_fallback: None,
                    wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
                    // Only the first arc draws the guide (at the head), matching
                    // the single-sub-slide behaviour.
                    guide: if seg_idx == 0 { guide } else { None },
                };
                let frac = progress
                    .and_then(|p| p.seg_frac.get(seg_idx).copied())
                    .unwrap_or(0.0);
                let seg_frac_one = [frac];
                let draw_seg = || {
                    slide_render::draw_slide(
                        &seg_note,
                        &seg_slide,
                        current_t,
                        ns,
                        slide_dur_s,
                        fade_in_s,
                        pad,
                        svg,
                        scale,
                        spawn_cx,
                        outer_r,
                        &seg_tex,
                        app.edit_solo_slide.is_some(),
                        t.speed_scale,
                        app.note_speed,
                        params::slide_fade_in(),
                        &seg_frac_one,
                        [None; 3],
                        core_driven,
                        SlideLayer::Trail,
                        true,
                    );
                };
                if note.is_break || sl.slide_is_break {
                    skin::with_break_shine(app, current_t, draw_seg);
                } else {
                    draw_seg();
                }
                if let Some(last) = segment.points.last() {
                    seg_note.lane = last.zone.to_id();
                }
            }
            continue;
        }

        let draw_slide = || {
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
                app.edit_solo_slide.is_some(),
                t.speed_scale,
                app.note_speed,
                params::slide_fade_in(),
                seg_frac,
                track_bars,
                core_driven,
                layer,
                false,
            );
        };
        if note.is_break || sl.slide_is_break {
            skin::with_break_shine(app, current_t, draw_slide);
        } else {
            draw_slide();
        }
    }
}

pub(super) fn preview_subslides(
    note: &Note,
    solo: Option<(u64, usize)>,
    reverse: bool,
) -> Box<dyn Iterator<Item = (usize, &Slide)> + '_> {
    let selected = solo.map(|(id, index)| (id, index.min(note.slide.len().saturating_sub(1))));
    let subs = note.slide.iter().enumerate().filter(move |(index, _)| {
        selected.is_none_or(|(id, selected_index)| id == note.id && *index == selected_index)
    });
    if reverse {
        Box::new(subs.rev())
    } else {
        Box::new(subs)
    }
}

/// Number of trail bars to hide as the star advances, **grouped by sensor
/// segment**.
///
/// `star_dist` is the star's distance along the path. Every judge segment that
/// lies entirely behind the star is hidden at once, so the trail disappears in
/// chunks (a whole zone's worth of tiles) rather than tile by tile.
fn hidden_bars_for_star(seg: &SlideSegmentation, star_dist: f32) -> usize {
    // Bars are ordered by increasing distance from the path start, so the last
    // bar at or before the star is its current bar.
    let Some(bar_idx) = seg
        .bars
        .iter()
        .rposition(|b| b.distance_along <= star_dist)
    else {
        return 0;
    };
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

/// Draw source judgment symbols, with the classic skin as a custom-path fallback.
///
/// Drawn as its own pass (not inside [`draw`]) so an overlay still shows after
/// lnmai-core has already hidden the trail/star it belongs to.
pub fn draw_just_overlays(app: &PadPreviewState, pad: &PadGeom, scale: f32, spawn_cx: Vec2) {
    if app.edit_solo_slide.is_some() || params::hide_slide_just() {
        return;
    }
    let Some(ref svg) = app.pad_svg else {
        return;
    };
    let now = app.fx_clock();
    for note in app.playback_chart().notes.iter() {
        if !matches!(note.note_type, NoteType::Slide) {
            continue;
        }
        for (si, sl) in note.slide.iter().enumerate() {
            let Some(fx) = app.slide_judgment(note, si) else {
                continue;
            };
            let age = now - fx.started;
            let (alpha, additive) = super::slide_judgment::animation(age);
            if alpha <= 0.0 {
                continue;
            }
            if draw_result_sprite(app, note, sl, fx.grade, age as f32, pad, spawn_cx, alpha, additive) {
                continue;
            }
            let variant = slideok_variant(note, sl);
            let stem = format!("{}_{}{}", fx.grade.family(), variant, fx.grade.suffix());
            let Some(tex) = app.slideok_tex.get(&stem) else {
                continue;
            };
            let c = fx.grade.tint();
            let tint = Color::new(c.r, c.g, c.b, alpha);
            slide_render::draw_slide_just(
                note,
                sl,
                pad,
                svg,
                scale,
                spawn_cx,
                pad.outer_r,
                tex,
                tint,
                params::slide_just_scale(),
            );
        }
    }
}

fn draw_result_sprite(
    app: &PadPreviewState, note: &Note, slide: &Slide, grade: SlideJudgeGrade, age: f32,
    pad: &PadGeom, spawn: Vec2, alpha: f32, additive: bool,
) -> bool {
    use crate::player::feedback::Grade;
    let Some(resources) = &app.feedback_resources else { return false };
    let feedback = grade.feedback();
    // The source has no shaped MISS sprite; retain the skin's miss/toofast family.
    if feedback.grade == Grade::Miss { return false; }
    let Some(pose) = super::slide_judgment::resolve(note, slide) else { return false };
    let (anchor, size, rotation) = pose.screen(spawn, pad.outer_r + params::tap_target_offset());
    let key = format!("{}_{}", pose.key, feedback.sprite_suffix(true));
    let size = size * params::slide_just_scale();
    let drawn = resources.draw_sprite(&key, anchor, size, rotation, alpha, age);
    if drawn && slide.slide_is_break && additive {
        resources.draw_sprite_additive(&format!("{key}_Break"), anchor, size, rotation, alpha, age);
    }
    drawn
}

/// Shape family of a slide, selecting the classic fallback sprite set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlideJustShape {
    Str,
    Curv,
    Wifi,
}

fn slideok_shape(slide: &Slide) -> SlideJustShape {
    // The overlay follows the **last** arc's type.
    match slide.segments.last().map(|s| s.shape) {
        Some(SlideShape::Wifi) => SlideJustShape::Wifi,
        Some(SlideShape::Line) | None => SlideJustShape::Str,
        Some(_) => SlideJustShape::Curv,
    }
}

/// Variant key selecting the classic fallback sprite set.
fn slideok_variant(note: &Note, slide: &Slide) -> &'static str {
    let side = slideok_side(note, slide);
    match slideok_shape(slide) {
        SlideJustShape::Wifi => {
            if side > 0 {
                "wifi_u"
            } else {
                "wifi_d"
            }
        }
        SlideJustShape::Curv => {
            if side > 0 {
                "curv_l"
            } else {
                "curv_r"
            }
        }
        SlideJustShape::Str => {
            if side > 0 {
                "str_r"
            } else {
                "str_l"
            }
        }
    }
}

/// Pick the `slideok` sprite stem (`<family>_<shape><suffix>`) for a sub-slide.
fn slideok_stem(note: &Note, slide: &Slide, grade: SlideJudgeGrade) -> String {
    format!(
        "{}_{}{}",
        grade.family(),
        slideok_variant(note, slide),
        grade.suffix()
    )
}

/// Direction of a sub-slide's **last** arc: `+1` clockwise (simai `>`), `-1`
/// counter-clockwise (`<`).
///
/// `<`/`>` are read from the segment shape — their endpoints are identical to
/// the opposite turn, so the sign of `end - start` cannot tell them apart.
/// Other shapes fall back to that endpoint sign.
fn slideok_side(note: &Note, slide: &Slide) -> i32 {
    let Some(last_seg) = slide.segments.last() else {
        return 1;
    };
    match last_seg.shape {
        SlideShape::Right => return 1,
        SlideShape::Left => return -1,
        _ => {}
    }
    let start = if slide.segments.len() >= 2 {
        slide.segments[slide.segments.len() - 2]
            .points
            .last()
            .map(|p| p.zone.to_id() as i32)
    } else {
        None
    }
    .unwrap_or(note.lane as i32);
    let end = last_seg
        .points
        .last()
        .map(|p| p.zone.to_id() as i32)
        .unwrap_or(start);
    if start <= 8 && end <= 8 {
        let mut d = (end - start).rem_euclid(8);
        if d > 4 {
            d -= 8;
        }
        return if d < 0 { -1 } else { 1 };
    }
    1
}

#[cfg(test)]
mod tests {
    use super::hidden_bars_for_star;
    use crate::app::slide::segmentation::{SlideBar, SlideJudgeSegment, SlideSegmentation};
    use crate::app::types::zone::PadZone;
    use macroquad::math::vec2;

    #[test]
    fn editor_freeform_trail_uses_same_bars_as_standalone_first_leg() {
        use crate::app::{pad_svg::PadSvgDef, slide_render};
        use crate::app::types::{Note, PadGeom, Slide, SlidePoint, SlideSegment, SlideShape};
        let note = Note { lane: 1, ..Default::default() };
        let single = Slide {
            segments: vec![SlideSegment { shape: SlideShape::Line, points: vec![SlidePoint::from(PadZone::A5)] }],
            slide_duration: 1.0, slide_start_delay: 0.25, slide_is_break: false, runtime_parts: 1,
        };
        let mut chain = single.clone();
        chain.segments[0].points.push(SlidePoint::from(PadZone::A8));
        let rendered = slide_render::render_slide_legs(&chain);
        assert_eq!(rendered.segments.len(), 2, "draw must enter the per-leg trail branch");
        let first = Slide { segments: vec![rendered.segments[0].clone()], ..single.clone() };
        let svg = PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg")).unwrap();
        for radius in [100.0, 300.0, 480.0] {
            let pad = PadGeom { cx: 400.0, cy: 400.0, outer_r: radius };
            let path = |slide| slide_render::build_slide_path(&note, slide, &pad, &svg, 1.0, vec2(pad.cx, pad.cy), radius);
            let bars = |slide| crate::app::slide::segmentation::build(&path(slide), 16.0, 12.0, 12.0, &svg, &pad);
            let a = bars(&single);
            let b = bars(&first);
            assert_eq!(a.bars.len(), b.bars.len());
            assert_eq!(a.judge_segments, b.judge_segments);
            for (a, b) in a.bars.iter().zip(&b.bars) {
                assert_eq!(a.position, b.position);
                assert_eq!(a.rotation, b.rotation);
                assert_eq!(a.distance_along, b.distance_along);
            }
        }
        assert_eq!(chain.segments.len(), 1);
        assert_eq!(chain.segments[0].points.len(), 2);
    }

    #[test]
    fn editor_preview_isolates_the_selected_same_head_slide() {
        use super::preview_subslides;
        use crate::app::types::{Note, NoteType, Slide};

        let slide = Slide {
            segments: Vec::new(),
            slide_duration: 1.0,
            slide_start_delay: 0.25,
            slide_is_break: false,
            runtime_parts: 1,
        };
        let mut note = Note {
            id: 42,
            note_type: NoteType::Slide,
            slide: vec![slide.clone(), slide.clone(), slide],
            ..Default::default()
        };
        let indices = |note: &Note, solo, reverse| {
            preview_subslides(note, solo, reverse)
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        };
        for reverse in [false, true] {
            for index in 0..3 {
                assert_eq!(indices(&note, Some((42, index)), reverse), vec![index]);
            }
            assert!(indices(&note, Some((99, 1)), reverse).is_empty());
        }
        assert_eq!(indices(&note, None, false), vec![0, 1, 2]);
        assert_eq!(indices(&note, None, true), vec![2, 1, 0]);
        // Deleting a sibling clamps the selection just like the editor's edits.
        note.slide.pop();
        assert_eq!(indices(&note, Some((42, 2)), false), vec![1]);
        note.slide.clear();
        assert!(indices(&note, Some((42, 2)), false).is_empty());
    }

    fn sample_segmentation() -> SlideSegmentation {
        let bars = (0..10)
            .map(|i| SlideBar {
                position: vec2(i as f32, 0.0),
                rotation: 0.0,
                zone: None,
                distance_along: i as f32,
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
        assert_eq!(hidden_bars_for_star(&seg, 0.0), 0);
        // Mid first segment: still nothing fully passed.
        assert_eq!(hidden_bars_for_star(&seg, 1.0), 0);
        // Star reaches bar 3: the whole first segment (3 tiles) hides at once.
        assert_eq!(hidden_bars_for_star(&seg, 3.0), 3);
        // Inside the second segment: unchanged (no per-tile hiding).
        assert_eq!(hidden_bars_for_star(&seg, 5.0), 3);
        // Star reaches bar 7: second segment hides, cumulative 0..7.
        assert_eq!(hidden_bars_for_star(&seg, 7.0), 7);
        // Star at the end: everything hidden.
        assert_eq!(hidden_bars_for_star(&seg, 10.0), 10);
    }

    #[test]
    fn empty_segmentation_is_safe() {
        let seg = SlideSegmentation::default();
        assert_eq!(hidden_bars_for_star(&seg, 5.0), 0);
    }

    #[test]
    fn slideok_stem_picks_grade_shape_and_turn() {
        use super::{slideok_side, slideok_stem};
        use crate::app::types::{Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape};
        use crate::player::state::SlideJudgeGrade;

        let build = |lane: u8, shape: SlideShape, end: u8| {
            let slide = Slide {
                segments: vec![SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(end))],
                    shape,
                }],
                slide_duration: 1.0,
                slide_start_delay: 0.0,
                slide_is_break: false,
                runtime_parts: 1,
            };
            let note = Note {
                lane,
                note_type: NoteType::Slide,
                ..Default::default()
            };
            (note, slide)
        };

        // `>` (Right) is clockwise -> `curv_l`; the endpoint sign is ignored.
        let (note, slide) = build(4, SlideShape::Right, 3);
        assert_eq!(slideok_side(&note, &slide), 1);
        assert_eq!(slideok_stem(&note, &slide, SlideJudgeGrade::Perfect), "just_curv_l_p");
        assert_eq!(
            slideok_stem(&note, &slide, SlideJudgeGrade::FastGreat),
            "just_curv_l_fast_gr"
        );
        assert_eq!(
            slideok_stem(&note, &slide, SlideJudgeGrade::LateGood),
            "just_curv_l_late_gd"
        );
        assert_eq!(slideok_stem(&note, &slide, SlideJudgeGrade::Miss), "miss_curv_l");
        assert_eq!(
            slideok_stem(&note, &slide, SlideJudgeGrade::TooFast),
            "toofast_curv_l"
        );

        // `<` (Left) is counter-clockwise -> `curv_r`, even though its endpoints
        // match the opposite turn.
        let (note, slide) = build(4, SlideShape::Left, 3);
        assert_eq!(slideok_side(&note, &slide), -1);
        assert_eq!(slideok_stem(&note, &slide, SlideJudgeGrade::Perfect), "just_curv_r_p");

        // lane 1 -> lane 3 is a clockwise straight.
        let (note, slide) = build(1, SlideShape::Line, 3);
        assert_eq!(slideok_side(&note, &slide), 1);
        assert_eq!(slideok_stem(&note, &slide, SlideJudgeGrade::Perfect), "just_str_r_p");

        // wifi has no turn (same zone) and defaults to the "up" sprite.
        let (note, slide) = build(1, SlideShape::Wifi, 1);
        assert_eq!(
            slideok_stem(&note, &slide, SlideJudgeGrade::Perfect),
            "just_wifi_u_p"
        );
    }

    #[test]
    fn slideok_uses_last_arc_shape_and_direction() {
        use super::{slideok_shape, slideok_side, SlideJustShape};
        use crate::app::types::{Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape};

        // A chain `Line` then `Left`: type/direction come from the last arc.
        let slide = Slide {
            segments: vec![
                SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(5))],
                    shape: SlideShape::Line,
                },
                SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(3))],
                    shape: SlideShape::Left,
                },
            ],
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 2,
        };
        let note = Note {
            lane: 1,
            note_type: NoteType::Slide,
            ..Default::default()
        };
        assert_eq!(slideok_shape(&slide), SlideJustShape::Curv);
        assert_eq!(slideok_side(&note, &slide), -1);
    }
}
