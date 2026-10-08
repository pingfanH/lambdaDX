//! Animate-authored editor chrome with a live player pad in the ui_pad slot.

use macroanimate::xfl::{XflAsset, XflDrawXf, XflError};
use macroquad::prelude::*;

use crate::app::types::PadGeom;
use crate::player::state::PadPreviewState;

const PAD_CENTER: Vec2 = Vec2::new(238.5, 258.0);
const PAD_RADIUS: f32 = 173.5;
const PREVIEW_PANEL: Rect = Rect::new(0.0, 42.0, 468.0, 686.0);
const CHROME: &[&str] = &[
    "UI/Background/ui_background",
    "UI/Header/ui_header",
    "UI/Timeline/ui_grid",
    "UI/Transport/ui_progress",
    "UI/Preview/ui_sidebar_background",
    "UI/Preview/ui_collapse",
    "UI/Inspector/ui_inspector",
    "UI/Tools/ui_toolbar",
];

pub struct EditorUi {
    asset: XflAsset,
}

impl EditorUi {
    pub fn load() -> Result<Self, XflError> {
        let asset = XflAsset::load(crate::app::platform::asset_dir().join("Editor-1.fla"))?;
        for name in CHROME.iter().copied().chain(["UI/Preview/ui_pad"]) {
            if asset.get(name).is_none() {
                return Err(XflError(format!("editor UI is missing {name}")));
            }
        }
        // Prepare static geometry once; subsequent frames reuse MacroAnimate's cache.
        for name in CHROME {
            asset.frame_parts(name, 0);
        }
        Ok(Self { asset })
    }

    pub fn pad_geom(&self) -> PadGeom {
        // Fit the existing pad's outer ring inside the authored circular slot.
        PadGeom {
            cx: PAD_CENTER.x,
            cy: PAD_CENTER.y,
            outer_r: PAD_RADIUS / crate::app::params::pad_circle_scale().max(1.0),
        }
    }

    pub fn wants_pad(&self, position: Vec2) -> bool {
        position.distance_squared(PAD_CENTER) <= PAD_RADIUS * PAD_RADIUS
    }

    pub fn draw(&self, app: &PadPreviewState, scale: f32) {
        let xf = XflDrawXf::default();
        self.asset.draw("UI/Background/ui_background", 0, &xf);
        self.asset.draw("UI/Preview/ui_sidebar_background", 0, &xf);

        // The player's outside-ring mask belongs to the preview panel, not
        // the whole editor. Macroquad's scissor uses window-space pixels.
        let top_left = crate::app::viewport::to_window(vec2(PREVIEW_PANEL.x, PREVIEW_PANEL.y));
        let bottom_right = crate::app::viewport::to_window(vec2(
            PREVIEW_PANEL.x + PREVIEW_PANEL.w,
            PREVIEW_PANEL.y + PREVIEW_PANEL.h,
        ));
        let left = top_left.x.floor().clamp(0.0, screen_width()) as i32;
        let top = top_left.y.floor().clamp(0.0, screen_height()) as i32;
        let right = bottom_right.x.ceil().clamp(0.0, screen_width()) as i32;
        let bottom = bottom_right.y.ceil().clamp(0.0, screen_height()) as i32;
        unsafe { get_internal_gl() }.quad_gl.scissor(Some((
            left,
            top,
            (right - left).max(0),
            (bottom - top).max(0),
        )));
        let pad = self.pad_geom();
        crate::player::render::pad::draw_pad_sim(app, pad.cx, pad.cy, pad.outer_r, scale);
        unsafe { get_internal_gl() }.quad_gl.scissor(None);

        // Every authored foreground component stays above the pad and mask.
        for name in CHROME {
            if !matches!(
                *name,
                "UI/Background/ui_background" | "UI/Preview/ui_sidebar_background"
            ) {
                self.asset.draw(name, 0, &xf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroanimate::xfl::{Element, XflAtlas};

    #[test]
    fn authored_editor_contains_all_chrome_and_replaceable_pad() {
        let atlas = XflAtlas::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Editor-1.fla"),
        )
        .unwrap();
        assert_eq!((atlas.width, atlas.height), (1570.0, 856.0));
        for name in CHROME {
            assert!(
                !atlas.parts(name, 0).is_empty(),
                "empty UI component: {name}"
            );
        }
        let (_, preview) = &atlas.symbols["UI/Preview/ui_preview_panel"];
        assert!(
            preview
                .layers
                .iter()
                .flat_map(|l| &l.frames)
                .flat_map(|f| &f.elements)
                .any(
                    |e| matches!(e, Element::Instance { name, .. } if name == "UI/Preview/ui_pad")
                )
        );
        assert!(!CHROME.contains(&"UI/Preview/ui_pad"));
    }
}
