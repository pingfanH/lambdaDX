//! Data model for Adobe Animate XFL projects.
//!
//! An `.fla` file is just a ZIP archive of an XFL project; an XFL project can
//! also live as an unpacked directory. Both contain a `DOMDocument.xml` plus a
//! `LIBRARY/` folder with one XML per symbol. This module holds the parsed,
//! render-independent representation.

use std::collections::HashMap;

/// Flash column-major 2D affine matrix: `| a c tx | / | b d ty |`.
pub type Matrix = [f32; 6];

pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `result = parent * child` (child applied first).
pub fn mul(parent: &Matrix, child: &Matrix) -> Matrix {
    let [pa, pb, pc, pd, ptx, pty] = *parent;
    let [ca, cb, cc, cd, ctx, cty] = *child;
    [
        pa * ca + pc * cb,
        pb * ca + pd * cb,
        pa * cc + pc * cd,
        pb * cc + pd * cd,
        pa * ctx + pc * cty + ptx,
        pb * ctx + pd * cty + pty,
    ]
}

/// A solid colour with straight alpha in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolidColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl SolidColor {
    pub fn rgba(&self) -> [u8; 4] {
        [self.r, self.g, self.b, (self.a.clamp(0.0, 1.0) * 255.0) as u8]
    }
}

/// A bitmap library item (`DOMBitmapItem`) and, for `.fla` sources, its bytes.
#[derive(Clone, Debug)]
pub struct BitmapAsset {
    /// Library item name, referenced by `DOMBitmapInstance::libraryItemName`.
    pub name: String,
    /// Project-relative path, forward slashes (e.g. `LIBRARY/foo.png`).
    pub path: String,
    /// Original import path (`sourceExternalFilepath`), used as a fallback when
    /// the project-local image is missing (unpacked XFL often omits it).
    pub external: Option<String>,
    /// Raw image bytes when loaded from a `.fla`/ZIP; `None` for directories.
    pub bytes: Option<Vec<u8>>,
    /// Raw bytes of the embedded `bin/*.dat` (`bitmapDataHRef`), if any.
    pub dat: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolType {
    Graphic,
    MovieClip,
    Button,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    Loop,
    PlayOnce,
    SingleFrame,
}

/// One filled/stroked polygon in a shape's local coordinates.
#[derive(Clone, Debug)]
pub struct ShapePath {
    pub points: Vec<[f32; 2]>,
    pub fill: Option<SolidColor>,
    pub stroke: Option<(SolidColor, f32)>,
}

/// A drawable element placed on a timeline frame.
#[derive(Clone, Debug)]
pub enum Element {
    Bitmap {
        name: String,
        matrix: Matrix,
        alpha: f32,
    },
    Instance {
        name: String,
        matrix: Matrix,
        loop_mode: LoopMode,
        first_frame: usize,
        alpha: f32,
    },
    Shape {
        matrix: Matrix,
        paths: Vec<ShapePath>,
    },
    Group {
        matrix: Matrix,
        members: Vec<Element>,
        alpha: f32,
    },
}

impl Element {
    /// The element's own matrix.
    pub fn matrix(&self) -> Matrix {
        match self {
            Element::Bitmap { matrix, .. }
            | Element::Instance { matrix, .. }
            | Element::Shape { matrix, .. }
            | Element::Group { matrix, .. } => *matrix,
        }
    }

    /// The element's own opacity multiplier (1.0 when not specified).
    pub fn alpha(&self) -> f32 {
        match self {
            Element::Bitmap { alpha, .. }
            | Element::Instance { alpha, .. }
            | Element::Group { alpha, .. } => *alpha,
            Element::Shape { .. } => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub index: usize,
    pub duration: usize,
    /// `true` when the frame inter-polates toward the next keyframe.
    pub tween: bool,
    /// Frame label (`DOMFrame name="…"`), if any.
    pub label: Option<String>,
    pub elements: Vec<Element>,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub frames: Vec<Frame>,
}

#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub name: String,
    pub layers: Vec<Layer>,
}

impl Timeline {
    /// Highest `index + duration` across all layers (at least 1).
    pub fn frame_count(&self) -> usize {
        self.layers
            .iter()
            .flat_map(|l| l.frames.iter())
            .map(|f| f.index + f.duration.max(1))
            .max()
            .unwrap_or(0)
            .max(1)
    }
}

/// Where an animation's frames are rooted.
#[derive(Clone, Debug)]
pub enum AnimTarget {
    /// The `DOMDocument.xml` main timeline (`Scene 1`).
    Scene,
    Symbol(String),
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub frames: usize,
    /// First timeline frame this animation starts at (0 except frame labels).
    pub start_frame: usize,
    pub target: AnimTarget,
}

/// One drawable produced by evaluating a timeline at a frame.
#[derive(Clone, Debug)]
pub enum PartContent {
    /// Draw the named bitmap asset.
    Bitmap(String),
    /// Draw filled/stroked polygons (local coordinates).
    Vector(Vec<ShapePath>),
}

#[derive(Clone, Debug)]
pub struct DrawPart {
    pub content: PartContent,
    pub matrix: Matrix,
    /// Opacity multiplier (1.0 = opaque).
    pub alpha: f32,
}

/// A fully parsed XFL project, ready to evaluate.
pub struct XflAtlas {
    pub frame_rate: f32,
    pub width: f32,
    pub height: f32,
    /// The document's main timeline.
    pub scene: Timeline,
    /// Library symbols: name -> (type, timeline).
    pub symbols: HashMap<String, (SymbolType, Timeline)>,
    /// Bitmap items: name -> asset.
    pub bitmaps: HashMap<String, BitmapAsset>,
    /// Named animations (main-scene frame labels plus every library symbol).
    pub animations: HashMap<String, Animation>,
}
