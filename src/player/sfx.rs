//! Central note-sound-effect configuration, mirroring `render::skin`.
//!
//! Every place that plays a per-note cue asks this module which buffer to use,
//! so the note → sound mapping lives in **one** editable table instead of
//! scattered `if is_break` chains. Variants mirror the skins
//! (`Normal`/`Each`/`Break`) plus an Ex overlay (e.g. tap ex → `tap_ex.wav`).
//!
//! Add a sound by adding a row; call sites never need to change.

use crate::app::audio;
use crate::app::audio::SfxBuffer;
use crate::player::render::skin::SkinVariant;
use crate::player::state::PadPreviewState;

/// Which sound of a note is being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxKind {
    Tap,
    Touch,
    Hanabi,
    Hold,
    /// Slide star-head cue (played when the star spawns).
    SlideCue,
    /// Slide judged.
    SlideJudge,
}

/// A buffer accessor on [`PadPreviewState`].
pub type SfxRef = fn(&PadPreviewState) -> Option<&SfxBuffer>;

/// `(kind, variant) -> sound`. The constant **is** the configuration.
#[rustfmt::skip]
const SFX_TABLE: &[(SfxKind, SkinVariant, SfxRef)] = &[
    (SfxKind::Tap,        SkinVariant::Normal, |a| a.sfx_tap.as_ref()),
    (SfxKind::Tap,        SkinVariant::Each,   |a| a.sfx_tap.as_ref()),
    (SfxKind::Tap,        SkinVariant::Break,  |a| a.sfx_break_tap.as_ref()),

    (SfxKind::Touch,      SkinVariant::Normal, |a| a.sfx_touch.as_ref()),
    (SfxKind::Touch,      SkinVariant::Each,   |a| a.sfx_touch.as_ref()),
    (SfxKind::Touch,      SkinVariant::Break,  |a| a.sfx_break.as_ref()),

    (SfxKind::Hanabi,     SkinVariant::Normal, |a| a.sfx_hanabi.as_ref()),

    (SfxKind::Hold,       SkinVariant::Normal, |a| a.sfx_hold.as_ref()),
    (SfxKind::Hold,       SkinVariant::Each,   |a| a.sfx_hold.as_ref()),
    (SfxKind::Hold,       SkinVariant::Break,  |a| a.sfx_break.as_ref()),

    (SfxKind::SlideCue,   SkinVariant::Normal, |a| a.sfx_slide.as_ref()),
    (SfxKind::SlideCue,   SkinVariant::Break,  |a| a.sfx_slide_break_start.as_ref()),

    (SfxKind::SlideJudge, SkinVariant::Normal, |a| a.sfx_slide.as_ref()),
    (SfxKind::SlideJudge, SkinVariant::Break,  |a| a.sfx_slide_break_slide.as_ref()),
];

/// `kind -> Ex overlay` (a note with `is_ex` prefers this over its body sound).
#[rustfmt::skip]
const SFX_EX_TABLE: &[(SfxKind, SfxRef)] = &[
    (SfxKind::Tap,        |a| a.sfx_ex.as_ref()),
    (SfxKind::SlideJudge, |a| a.sfx_ex.as_ref()),
];

/// Sound for exactly this kind/variant, if present.
fn table_sfx(app: &PadPreviewState, kind: SfxKind, variant: SkinVariant) -> Option<&SfxBuffer> {
    SFX_TABLE
        .iter()
        .find(|(k, v, _)| *k == kind && *v == variant)
        .and_then(|(_, _, sfx)| sfx(app))
}

fn ex_sfx(app: &PadPreviewState, kind: SfxKind) -> Option<&SfxBuffer> {
    SFX_EX_TABLE
        .iter()
        .find(|(k, _)| *k == kind)
        .and_then(|(_, sfx)| sfx(app))
}

/// Pick the sound for a note/event: Ex wins, then the exact variant, then the
/// kind's `Normal` variant. Missing variants fall back so charts still sound.
pub fn select(
    app: &PadPreviewState,
    kind: SfxKind,
    variant: SkinVariant,
    is_ex: bool,
) -> Option<&SfxBuffer> {
    if is_ex {
        if let Some(sfx) = ex_sfx(app, kind) {
            return Some(sfx);
        }
    }
    table_sfx(app, kind, variant).or_else(|| table_sfx(app, kind, SkinVariant::Normal))
}

/// A successful firework layers over the ordinary touch sound, never over a miss.
pub fn hanabi_for_hit(
    app: &PadPreviewState,
    grade: crate::player::feedback::FeedbackGrade,
) -> Option<&SfxBuffer> {
    if !app.audio_enabled || app.scrubbing || !crate::app::params::hit_sfx()
        || grade.judgment().is_none()
    {
        return None;
    }
    select(app, SfxKind::Hanabi, SkinVariant::Normal, false)
}

/// Load every pad cue/judgment sound (shared by both front-ends so their pads
/// sound identical). Candidates are tried in order; a missing file is skipped.
pub async fn load_pad_sfx(app: &mut PadPreviewState) {
    app.answer_sfx = audio::load_sfx(&["Sfx/answer.wav"]).await;
    app.sfx_tap = audio::load_sfx(&[
        "Sfx/tap_perfect.wav",
        "Sfx/tap_great.wav",
        "Sfx/tap_good.wav",
        "Sfx/tap.wav",
    ])
    .await;
    app.sfx_ex = audio::load_sfx(&["Sfx/tap_ex.wav", "Sfx/tap_perfect.wav"]).await;
    app.sfx_touch = audio::load_sfx(&["Sfx/touch.wav"]).await;
    app.sfx_hanabi = audio::load_sfx(&["Sfx/touch_hanabi.wav"]).await;
    app.sfx_slide = audio::load_sfx(&["Sfx/slide.wav"]).await;
    app.sfx_hold = audio::load_sfx(&["Sfx/hold.wav"]).await;
    app.sfx_break = audio::load_sfx(&["Sfx/break.wav"]).await;
    app.sfx_break_tap = audio::load_sfx(&["Sfx/break_tap.wav", "Sfx/break.wav"]).await;
    app.sfx_break_slide = audio::load_sfx(&["Sfx/break_slide.wav", "Sfx/break.wav"]).await;
    app.sfx_slide_break_start =
        audio::load_sfx(&["Sfx/slide_break_start.wav", "Sfx/slide.wav"]).await;
    app.sfx_slide_break_slide =
        audio::load_sfx(&["Sfx/slide_break_slide.wav", "Sfx/slide.wav"]).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::JudgeGrade;

    #[test]
    fn hanabi_uses_its_own_decodable_sound_and_respects_audio_gates() {
        let previous = crate::app::params::get();
        let mut params = previous.clone();
        params.hit_sfx = true;
        params.hit_fx = false;
        crate::app::params::set(params.clone());
        let mut app = crate::player::render::scale::tests::app();
        app.audio_enabled = true;
        app.sfx_hanabi = SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch_hanabi.wav"));
        app.sfx_touch = SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch.wav"));
        let selected = hanabi_for_hit(&app, JudgeGrade::Perfect.into()).unwrap();
        assert!(std::ptr::eq(selected, app.sfx_hanabi.as_ref().unwrap()));
        assert!(!std::ptr::eq(selected, app.sfx_touch.as_ref().unwrap()));
        assert!(hanabi_for_hit(&app, JudgeGrade::Miss.into()).is_none());
        assert!(hanabi_for_hit(&app, JudgeGrade::TooFast.into()).is_none());
        app.scrubbing = true;
        assert!(hanabi_for_hit(&app, JudgeGrade::Perfect.into()).is_none());
        app.scrubbing = false;
        app.audio_enabled = false;
        assert!(hanabi_for_hit(&app, JudgeGrade::Perfect.into()).is_none());
        app.audio_enabled = true;
        params.hit_sfx = false;
        crate::app::params::set(params);
        assert!(hanabi_for_hit(&app, JudgeGrade::Perfect.into()).is_none());
        crate::app::params::set(previous);
    }
}
