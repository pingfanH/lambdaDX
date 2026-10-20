//! Cached note skins shared by editor, preview, and player.

use crate::app::note_skin::NoteSkin;
use crate::player::state::PadPreviewState;
use macroquad::miniquad::{FilterMode, MipmapFilterMode};
use macroquad::prelude::{Texture2D, load_texture};
use std::collections::HashMap;

#[derive(Default)]
pub struct NoteTextures {
    textures: HashMap<String, Texture2D>,
}

const NOTE_SLOTS: &[&str] = &[
    "tap",
    "tap_each",
    "tap_break",
    "tap_ex",
    "tap_break_ex",
    "hold",
    "hold_each",
    "hold_break",
    "hold_ex",
    "hold_break_ex",
    "star",
    "star_each",
    "star_break",
    "star_ex",
    "star_break_ex",
    "star_double",
    "star_double_each",
    "star_double_break",
    "star_double_ex",
    "star_double_break_ex",
    "slide",
    "slide_each",
    "slide_break",
    "touch",
    "touch_each",
    "touch_point",
    "touch_point_each",
    "touchhold_border",
    "tap_guide",
    "Each",
    "Break",
    "slide_guide",
    "Hold_End",
    "Hold_Each_End",
    "Hold_Break_End",
];

async fn first(paths: &[String]) -> Option<Texture2D> {
    for path in paths {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            let id = tex.raw_miniquad_id();
            let gl = unsafe { macroquad::prelude::get_internal_gl() };
            gl.quad_context.texture_generate_mipmaps(id);
            gl.quad_context
                .texture_set_filter(id, FilterMode::Linear, MipmapFilterMode::Linear);
            return Some(tex);
        }
    }
    None
}

impl NoteTextures {
    async fn load(skin: NoteSkin) -> Self {
        let mut textures = HashMap::new();
        let slots = NOTE_SLOTS
            .iter()
            .map(|name| name.to_string())
            .chain((0..4).map(|i| format!("touchhold_{i}")))
            .chain((0..11).map(|i| format!("wifi_{i}")));
        for name in slots {
            let mut paths = vec![format!("Skins/{}/{name}.png", skin.id())];
            if skin == NoteSkin::Classic {
                if name.ends_with("break_ex") {
                    continue;
                }
                paths.extend([format!("skins/classic/{name}.png"), format!("{name}.png")]);
            }
            if let Some(texture) = first(&paths).await {
                textures.insert(name, texture);
            }
        }
        Self { textures }
    }
}

pub async fn load_note_textures(app: &mut PadPreviewState) {
    for skin in NoteSkin::ALL {
        if !app.note_skin_cache.contains_key(&skin) {
            app.note_skin_cache
                .insert(skin, NoteTextures::load(skin).await);
        }
    }
    sync_note_skin(app);
    load_slideok_textures(app).await;
    if app.feedback_resources.is_none() {
        match crate::player::feedback::FeedbackResources::load().await {
            Ok(resources) => app.feedback_resources = Some(resources),
            Err(error) => eprintln!("LambdaDX feedback assets: {error}"),
        }
    }
}

pub fn sync_note_skin(app: &mut PadPreviewState) {
    let selected = app.params.note_skin;
    if app.loaded_note_skin == Some(selected) {
        return;
    }
    let Some(skin) = app.note_skin_cache.get(&selected) else {
        return;
    };
    let classic = app.note_skin_cache.get(&NoteSkin::Classic);
    let texture = |name: &str| {
        skin.textures
            .get(name)
            .or_else(|| classic.and_then(|skin| skin.textures.get(name)))
            .cloned()
    };
    macro_rules! assign {
        ($($field:ident => $name:literal),* $(,)?) => { $(app.$field = texture($name);)* };
    }
    assign! {
        tap_texture => "tap", tap_each_tex => "tap_each", tap_break_tex => "tap_break", tap_ex_tex => "tap_ex",
        hold_texture => "hold", hold_each_tex => "hold_each", hold_break_tex => "hold_break", hold_ex_tex => "hold_ex",
        star_tex => "star", star_each_tex => "star_each", star_break_tex => "star_break", star_ex_tex => "star_ex",
        star_double_tex => "star_double", star_double_each_tex => "star_double_each",
        star_double_break_tex => "star_double_break", star_double_ex_tex => "star_double_ex",
        slide_tex => "slide", slide_each_tex => "slide_each", slide_break_tex => "slide_break",
        touch_tri_tex => "touch", touch_tri_each_tex => "touch_each",
        touch_point_tex => "touch_point", touch_point_each_tex => "touch_point_each",
        touchhold_border_tex => "touchhold_border", tap_guide_tex => "tap_guide",
        tap_guide_each_tex => "Each", tap_guide_break_tex => "Break", slide_guide_tex => "slide_guide",
        hold_end_guide_tex => "Hold_End", hold_end_each_guide_tex => "Hold_Each_End",
        hold_end_break_guide_tex => "Hold_Break_End",
    }
    app.touchhold_tex = std::array::from_fn(|i| texture(&format!("touchhold_{i}")));
    app.wifi_tex = std::array::from_fn(|i| texture(&format!("wifi_{i}")));
    app.loaded_note_skin = Some(selected);
}

pub fn extra_texture<'a>(app: &'a PadPreviewState, name: &str) -> Option<&'a Texture2D> {
    app.note_skin_cache
        .get(&app.loaded_note_skin?)
        .and_then(|skin| skin.textures.get(name))
}

async fn load_slideok_textures(app: &mut PadPreviewState) {
    const SHAPES: [&str; 6] = ["curv_l", "curv_r", "str_l", "str_r", "wifi_u", "wifi_d"];
    const SUFFIXES: [&str; 6] = ["", "_p", "_fast_gr", "_late_gr", "_fast_gd", "_late_gd"];
    for shape in SHAPES {
        let stems = SUFFIXES
            .iter()
            .map(|suffix| format!("just_{shape}{suffix}"))
            .chain([format!("miss_{shape}"), format!("toofast_{shape}")]);
        for stem in stems {
            if let Some(tex) = first(&[format!("Skins/classic/slideok/{stem}.png")]).await {
                app.slideok_tex.insert(stem, tex);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_skin_contains_every_renderer_slot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Skins/lambdadx");
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("skin.json")).unwrap())
                .unwrap();
        for slot in NOTE_SLOTS
            .iter()
            .map(|s| s.to_string())
            .chain((0..4).map(|i| format!("touchhold_{i}")))
            .chain((0..11).map(|i| format!("wifi_{i}")))
        {
            assert!(!manifest["slots"][&slot].is_null(), "{slot}");
            let image = image::open(root.join(format!("{slot}.png"))).unwrap();
            assert!(image.width() > 0 && image.height() > 0);
        }
        let defaults: crate::app::params::Params = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults.note_skin, NoteSkin::Classic);
        let mut selected = defaults;
        selected.note_skin = NoteSkin::Lambdadx;
        let roundtrip: crate::app::params::Params =
            serde_json::from_str(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert_eq!(roundtrip.note_skin, NoteSkin::Lambdadx);
        let normal = image::open(root.join("tap.png")).unwrap();
        let each = image::open(root.join("tap_each.png")).unwrap();
        assert_ne!(normal.to_rgba8(), each.to_rgba8());
        for i in 0..11 {
            let source =
                image::open(root.join(format!("source/slidefan/MM_UI_Game_{:03}.png", 53 + i)))
                    .unwrap();
            let full = image::open(root.join(format!("wifi_{i}.png"))).unwrap();
            assert_eq!(full.width(), source.width() * 2);
            assert_eq!(full.height(), source.height());
            assert_eq!(
                full.to_rgba8(),
                image::imageops::flip_horizontal(&full.to_rgba8())
            );
        }
    }
}
