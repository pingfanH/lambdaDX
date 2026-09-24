//! Debug export: render every possible slide curve to an SVG so the path can
//! be inspected and the curve/fillet parameters tuned.
//!
//! Run with `lambda_dx_pad_preview --dump-slides-svg [PATH]` (default
//! `output/slide_curves.svg`).

use macroquad::math::vec2;

use crate::app::pad_svg::PadSvgDef;
use crate::app::slide_render::build_slide_path;
use crate::app::types::zone::PadZone;
use crate::app::types::{
    Note, NoteType, PadGeom, Slide, SlidePoint, SlideSegment, SlideShape,
};

/// Shapes exported (wifi has its own three-track renderer and is skipped).
const SHAPES: &[SlideShape] = &[
    SlideShape::Q,
    SlideShape::QQ,
    SlideShape::P,
    SlideShape::PP,
    SlideShape::Left,
    SlideShape::Right,
    SlideShape::Caret,
    SlideShape::Z,
    SlideShape::S,
    SlideShape::VShape,
    SlideShape::BigV,
    SlideShape::Line,
];

fn color(shape: SlideShape) -> &'static str {
    match shape {
        SlideShape::Q => "#ff5555",
        SlideShape::QQ => "#ff9955",
        SlideShape::P => "#ffe45e",
        SlideShape::PP => "#b6ff5e",
        SlideShape::Left => "#5eff8b",
        SlideShape::Right => "#5ef0ff",
        SlideShape::Caret => "#5ea8ff",
        SlideShape::Z => "#b55eff",
        SlideShape::S => "#ff5edb",
        SlideShape::VShape => "#ff5ea8",
        SlideShape::BigV => "#ffffff",
        SlideShape::Line => "#9a9aa2",
        SlideShape::Wifi => "#888888",
    }
}

/// Build the SVG document for every slide path.
pub fn all_paths_svg() -> String {
    let svg = PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
        .expect("bundled pad.svg must parse");
    let pad = PadGeom {
        cx: 500.0,
        cy: 500.0,
        outer_r: 400.0,
    };
    let spawn = svg
        .pad_visual_center(&pad)
        .unwrap_or(vec2(pad.cx, pad.cy));

    let mut out = String::new();
    out.push_str(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"1000\" \
         viewBox=\"0 0 1000 1000\">\n",
    );
    out.push_str("<rect width=\"1000\" height=\"1000\" fill=\"#111\"/>\n");

    // Zone polygons as a reference frame.
    for z in &svg.zones {
        let verts = svg.def_screen_verts(z, &pad);
        out.push_str("<polygon points=\"");
        for v in &verts {
            out.push_str(&format!("{:.2},{:.2} ", v.x, v.y));
        }
        out.push_str("\" fill=\"none\" stroke=\"#3a3a40\" stroke-width=\"1\"/>\n");
    }

    for &shape in SHAPES {
        for lane in 1..=8u8 {
            for target in 1..=8u8 {
                let note = Note {
                    lane,
                    note_type: NoteType::Slide,
                    ..Default::default()
                };
                let slide = Slide {
                    segments: vec![SlideSegment {
                        points: vec![SlidePoint {
                            zone: PadZone::from(target),
                            beat_offset: 0.0,
                        }],
                        shape,
                    }],
                    slide_duration: 1.0,
                    slide_start_delay: 0.0,
                    slide_is_break: false,
                };
                let path = build_slide_path(&note, &slide, &pad, &svg, 1.0, spawn, pad.outer_r);
                if path.len() < 2 {
                    continue;
                }
                out.push_str(&format!(
                    "<polyline fill=\"none\" stroke=\"{}\" stroke-width=\"1.2\" \
                     stroke-opacity=\"0.85\" points=\"",
                    color(shape)
                ));
                for p in &path {
                    out.push_str(&format!("{:.2},{:.2} ", p.x, p.y));
                }
                out.push_str("\"/>\n");
            }
        }
    }

    // Legend.
    out.push_str("<g font-family=\"monospace\" font-size=\"12\" fill=\"#ddd\">\n");
    for (i, &shape) in SHAPES.iter().enumerate() {
        let y = 16.0 + i as f32 * 15.0;
        out.push_str(&format!(
            "<line x1=\"12\" y1=\"{y}\" x2=\"34\" y2=\"{y}\" stroke=\"{}\" stroke-width=\"2\"/>\
             <text x=\"40\" y=\"{}\">{:?}</text>\n",
            color(shape),
            y + 4.0,
            shape
        ));
    }
    out.push_str("</g>\n</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::all_paths_svg;

    #[test]
    fn exports_paths_and_legend() {
        let svg = all_paths_svg();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("<polyline"), "should contain slide paths");
        assert!(svg.contains("Q"), "legend should list shapes");
    }
}
