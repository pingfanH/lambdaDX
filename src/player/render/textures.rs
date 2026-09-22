//! Note skin loading. Each texture is looked up through a small candidate path
//! list so both `Skins/classic/<name>.png` and a flat `<name>.png` work. Any
//! missing texture falls back to primitive shapes in the renderers.

use macroquad::miniquad::FilterMode;
use macroquad::prelude::load_texture;
use macroquad::texture::Texture2D;

use crate::player::state::PadPreviewState;

/// Try each path in order; return the first texture that loads.
async fn first(paths: &[&str]) -> Option<Texture2D> {
    for path in paths {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            return Some(tex);
        }
    }
    None
}

/// Load every note texture into `app`.
pub async fn load_note_textures(app: &mut PadPreviewState) {
    app.tap_texture = first(&["Skins/classic/tap.png", "skins/classic/tap.png", "tap.png"]).await;
    app.hold_texture =
        first(&["Skins/classic/hold.png", "skins/classic/hold.png", "hold.png"]).await;
    app.touch_tri_tex = first(&["Skins/classic/touch.png", "touch.png"]).await;
    app.touch_point_tex = first(&["Skins/classic/touch_point.png", "touch_point.png"]).await;
    app.tap_each_tex = first(&["Skins/classic/tap_each.png", "tap_each.png"]).await;
    app.hold_each_tex = first(&["Skins/classic/hold_each.png", "hold_each.png"]).await;
    app.touch_tri_each_tex = first(&["Skins/classic/touch_each.png", "touch_each.png"]).await;
    app.touch_point_each_tex =
        first(&["Skins/classic/touch_point_each.png", "touch_point_each.png"]).await;

    // Touch-hold uses a 4-frame cross.
    for (i, name) in ["touchhold_0", "touchhold_1", "touchhold_2", "touchhold_3"]
        .iter()
        .enumerate()
    {
        let owned = [format!("Skins/classic/{name}.png"), format!("{name}.png")];
        let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        app.touchhold_tex[i] = first(&refs).await;
    }
    app.touchhold_border_tex =
        first(&["Skins/classic/touchhold_border.png", "touchhold_border.png"]).await;

    // Slide trails.
    app.slide_tex = first(&["Skins/classic/slide.png", "slide.png"]).await;
    app.slide_each_tex = first(&["Skins/classic/slide_each.png", "slide_each.png"]).await;
    for i in 0..11 {
        let owned = [
            format!("Skins/classic/wifi_{i}.png"),
            format!("wifi_{i}.png"),
        ];
        let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        app.wifi_tex[i] = first(&refs).await;
    }

    // Stars (slide heads / each / break / double variants).
    app.star_tex = first(&["Skins/classic/star.png", "star.png"]).await;
    app.star_each_tex = first(&["Skins/classic/star_each.png", "star_each.png"]).await;
    app.star_break_tex = first(&["Skins/classic/star_break.png", "star_break.png"]).await;
    app.star_double_tex = first(&["Skins/classic/star_double.png", "star_double.png"]).await;
    app.star_double_each_tex =
        first(&["Skins/classic/star_double_each.png", "star_double_each.png"]).await;

    // Break variants.
    app.tap_break_tex = first(&["Skins/classic/tap_break.png", "tap_break.png"]).await;
    app.hold_break_tex = first(&["Skins/classic/hold_break.png", "hold_break.png"]).await;
    app.slide_break_tex = first(&["Skins/classic/slide_break.png", "slide_break.png"]).await;
    app.star_double_break_tex = first(&[
        "Skins/classic/star_double_break.png",
        "star_double_break.png",
    ])
    .await;

    // Ex overlays.
    app.tap_ex_tex = first(&["Skins/classic/tap_ex.png", "tap_ex.png"]).await;
    app.hold_ex_tex = first(&["Skins/classic/hold_ex.png", "hold_ex.png"]).await;
    app.star_ex_tex = first(&["Skins/classic/star_ex.png", "star_ex.png"]).await;
    app.star_double_ex_tex = first(&[
        "Skins/classic/star_double_ex.png",
        "star_double_ex.png",
    ])
    .await;
}
