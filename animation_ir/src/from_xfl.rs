//! XFL/FLA → Animation IR.
//!
//! The XML parsing lives in `macroanimate`; this module only walks the parsed,
//! render-independent model and re-shapes it into [`crate::Document`], so the IR
//! never leaks parser or renderer types.

use std::fmt;

use macroanimate::xfl as mx;

use crate::{
    AnimTarget, Animation, Bitmap, Color, Document, Element, Frame, Layer, LoopMode, ShapePath,
    Stroke, Symbol, SymbolKind, Text, TextAlign, Timeline, TweenKind,
};

/// Anything that can go wrong turning an XFL/FLA source into IR.
#[derive(Debug)]
pub struct FromXflError(pub macroanimate::XflError);

impl fmt::Display for FromXflError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FromXflError {}

impl From<macroanimate::XflError> for FromXflError {
    fn from(e: macroanimate::XflError) -> Self {
        Self(e)
    }
}

/// Convert a parsed XFL atlas into IR.
pub fn convert(atlas: &mx::XflAtlas) -> Document {
    let mut symbols: Vec<Symbol> = atlas
        .symbols
        .iter()
        .map(|(name, (kind, timeline))| Symbol {
            name: name.clone(),
            kind: symbol_kind(*kind),
            scale9: atlas.scale9.get(name).copied(),
            timeline: timeline_ir(timeline),
        })
        .collect();
    symbols.sort_by(|a, b| a.name.cmp(&b.name));

    let mut bitmaps: Vec<Bitmap> = atlas
        .bitmaps
        .iter()
        .map(|(name, asset)| Bitmap {
            name: name.clone(),
            path: asset.path.clone(),
            external: asset.external.clone(),
        })
        .collect();
    bitmaps.sort_by(|a, b| a.name.cmp(&b.name));

    let mut animations: Vec<Animation> = atlas
        .animations
        .iter()
        .map(|(name, anim)| Animation {
            name: name.clone(),
            frames: anim.frames,
            start_frame: anim.start_frame,
            target: match &anim.target {
                mx::AnimTarget::Scene => AnimTarget::Scene,
                mx::AnimTarget::Symbol(s) => AnimTarget::Symbol(s.clone()),
            },
        })
        .collect();
    animations.sort_by(|a, b| a.name.cmp(&b.name));

    Document {
        frame_rate: atlas.frame_rate,
        width: atlas.width,
        height: atlas.height,
        scene: timeline_ir(&atlas.scene),
        symbols,
        bitmaps,
        animations,
    }
}

fn symbol_kind(kind: mx::SymbolType) -> SymbolKind {
    match kind {
        mx::SymbolType::Graphic => SymbolKind::Graphic,
        mx::SymbolType::MovieClip => SymbolKind::MovieClip,
        mx::SymbolType::Button => SymbolKind::Button,
    }
}

fn timeline_ir(timeline: &mx::Timeline) -> Timeline {
    Timeline {
        name: timeline.name.clone(),
        layers: timeline.layers.iter().map(layer_ir).collect(),
    }
}

fn layer_ir(layer: &mx::Layer) -> Layer {
    Layer {
        name: layer.name.clone(),
        frames: layer.frames.iter().map(frame_ir).collect(),
    }
}

fn frame_ir(frame: &mx::Frame) -> Frame {
    Frame {
        index: frame.index,
        duration: frame.duration,
        tween: if frame.tween {
            TweenKind::Motion
        } else {
            TweenKind::None
        },
        label: frame.label.clone(),
        elements: frame.elements.iter().map(element_ir).collect(),
    }
}

fn element_ir(element: &mx::Element) -> Element {
    match element {
        mx::Element::Bitmap {
            name,
            matrix,
            alpha,
        } => Element::Bitmap {
            asset: name.clone(),
            transform: *matrix,
            alpha: *alpha,
        },
        mx::Element::Instance {
            name,
            matrix,
            loop_mode,
            first_frame,
            alpha,
            scale9,
        } => Element::Instance {
            symbol: name.clone(),
            transform: *matrix,
            loop_mode: loop_mode_ir(*loop_mode),
            first_frame: *first_frame,
            alpha: *alpha,
            scale9: *scale9,
        },
        mx::Element::Shape { matrix, paths } => Element::Shape {
            transform: *matrix,
            paths: paths.iter().map(shape_path_ir).collect(),
        },
        mx::Element::Group {
            matrix,
            members,
            alpha,
        } => Element::Group {
            transform: *matrix,
            alpha: *alpha,
            members: members.iter().map(element_ir).collect(),
        },
        mx::Element::Text(run) => Element::Text(text_ir(run)),
    }
}

fn loop_mode_ir(mode: mx::LoopMode) -> LoopMode {
    match mode {
        mx::LoopMode::Loop => LoopMode::Loop,
        mx::LoopMode::PlayOnce => LoopMode::PlayOnce,
        mx::LoopMode::SingleFrame => LoopMode::SingleFrame,
    }
}

fn shape_path_ir(path: &mx::ShapePath) -> ShapePath {
    ShapePath {
        points: path.points.clone(),
        fill: path.fill.map(color_ir),
        stroke: path.stroke.map(|(color, width)| Stroke {
            color: color_ir(color),
            width,
        }),
    }
}

fn color_ir(color: mx::SolidColor) -> Color {
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

fn text_ir(run: &mx::TextRun) -> Text {
    Text {
        text: run.text.clone(),
        transform: run.matrix,
        size: run.size,
        color: Color {
            r: run.color[0],
            g: run.color[1],
            b: run.color[2],
            a: run.color[3] as f32 / 255.0,
        },
        align: match run.align {
            mx::TextAlign::Left => TextAlign::Left,
            mx::TextAlign::Center => TextAlign::Center,
            mx::TextAlign::Right => TextAlign::Right,
        },
        width: run.width,
        alpha: run.alpha,
    }
}
