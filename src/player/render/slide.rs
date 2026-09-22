//! Slide notes. Thin adapter over `app::slide_render::draw_slide`, which builds
//! the sampled path, draws the trail tiles and flies the star.

use macroquad::math::Vec2;

use crate::app::slide::segmentation;
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

        // Trail consumption: hide the trail bars the star has already passed, so
        // the trail disappears following the head instead of lingering whole.
        let star_start_s = ns + fade_in_s;
        let travel_s = (slide_dur_s - fade_in_s).max(SLIDE_MIN_DURATION_S);
        let star_t = if current_t <= star_start_s {
            0.0
        } else {
            ((current_t - star_start_s) / travel_s).clamp(0.0, 1.0)
        };
        let path = slide_render::build_slide_path(note, sl, pad, svg, scale, spawn_cx, outer_r);
        let bar_count = segmentation::build(&path, SLIDE_TILE_SPACING * scale, svg, pad)
            .bars
            .len();
        let hidden_until_bar = consumed_bars(star_t, bar_count);

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

/// Number of trail bars behind the star that must be hidden so the trail is
/// consumed as the star advances. `star_t` is the star's progress along the
/// path (0 at the head, 1 at the tail); bars are sampled evenly along it.
fn consumed_bars(star_t: f32, bar_count: usize) -> usize {
    let t = star_t.clamp(0.0, 1.0);
    ((t * bar_count as f32).floor() as usize).min(bar_count)
}

#[cfg(test)]
mod tests {
    use super::consumed_bars;

    #[test]
    fn trail_is_consumed_as_the_star_advances() {
        // Nothing hidden at the head, everything hidden once the star lands.
        assert_eq!(consumed_bars(0.0, 40), 0);
        assert_eq!(consumed_bars(1.0, 40), 40);
        // Half-way hides roughly half the bars.
        assert_eq!(consumed_bars(0.5, 40), 20);
        // Out-of-range progress is clamped and never exceeds the bar count.
        assert_eq!(consumed_bars(-1.0, 40), 0);
        assert_eq!(consumed_bars(2.0, 40), 40);
        assert_eq!(consumed_bars(0.5, 0), 0);
    }
}
