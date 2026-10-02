//! Render-independent Animation IR.
//!
//! Phase 1 of the Flash/Animate → IR → Rive + macroquad migration. The IR is a
//! neutral description of an Animate document: timelines, symbols, shapes,
//! bitmaps, text and transforms. It deliberately does **not** depend on Rive
//! (nor on any renderer). XFL/FLA enters through [`from_xfl`] and is turned into
//! a [`Document`]; later phases feed the same IR to a Rive exporter and/or the
//! macroquad-native VFX system.
//!
//! ```no_run
//! let doc = animation_ir::Document::load("assets/ui").unwrap();
//! println!("{} symbols, {} animations", doc.symbols.len(), doc.animations.len());
//! ```

mod from_xfl;

pub use from_xfl::FromXflError;

use serde::{Deserialize, Serialize};

/// Flash column-major 2D affine matrix: `| a c tx | / | b d ty |`.
pub type Matrix = [f32; 6];

pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// A straight-alpha colour (`a` in `0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 1.0,
    };
}

/// A parsed Animate document: the main scene plus the library it references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub frame_rate: f32,
    pub width: f32,
    pub height: f32,
    /// The document's main timeline (`Scene 1`).
    pub scene: Timeline,
    /// Library symbols, sorted by name.
    pub symbols: Vec<Symbol>,
    /// Bitmap library items, sorted by name.
    pub bitmaps: Vec<Bitmap>,
    /// Named animations (scene frame labels plus every library symbol).
    pub animations: Vec<Animation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Graphic,
    MovieClip,
    Button,
}

/// A library symbol and its timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Symbol-level scale-9 grid `[left, top, right, bottom]` in local px.
    pub scale9: Option<[f32; 4]>,
    pub timeline: Timeline,
}

/// A referenced bitmap item (pixel data is not part of the IR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bitmap {
    pub name: String,
    /// Project-relative path with forward slashes.
    pub path: String,
    /// Original import path, when the project-local image is missing.
    pub external: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Loop,
    PlayOnce,
    SingleFrame,
}

/// Where an animation's frames are rooted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimTarget {
    /// The document's main timeline (`Scene 1`).
    Scene,
    Symbol(String),
}

/// A named, seekable range of a timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub frames: usize,
    /// First timeline frame this animation starts at (0 except frame labels).
    pub start_frame: usize,
    pub target: AnimTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub frames: Vec<Frame>,
}

/// How a frame interpolates toward the next keyframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TweenKind {
    None,
    Motion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub index: usize,
    pub duration: usize,
    pub tween: TweenKind,
    /// Frame label (`DOMFrame name="…"`), if any.
    pub label: Option<String>,
    pub elements: Vec<Element>,
}

/// A drawable placed on a timeline frame, in its parent's local space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Bitmap {
        /// Name of the referenced [`Bitmap`].
        asset: String,
        transform: Matrix,
        alpha: f32,
    },
    Instance {
        /// Name of the referenced [`Symbol`].
        symbol: String,
        transform: Matrix,
        loop_mode: LoopMode,
        first_frame: usize,
        alpha: f32,
        /// Per-instance scale-9 override (rare; usually symbol-level).
        scale9: Option<[f32; 4]>,
    },
    Shape {
        transform: Matrix,
        paths: Vec<ShapePath>,
    },
    Group {
        transform: Matrix,
        alpha: f32,
        members: Vec<Element>,
    },
    Text(Text),
}

/// A filled/stroked polygon in local coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapePath {
    pub points: Vec<[f32; 2]>,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// A static text field. The IR carries no font data; a renderer rasterizes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
    pub transform: Matrix,
    pub size: f32,
    pub color: Color,
    pub align: TextAlign,
    /// Box width in local px (used to resolve alignment).
    pub width: f32,
    pub alpha: f32,
}

impl Document {
    /// Parse an XFL directory or `.fla`/`.zip` file into IR.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, FromXflError> {
        let atlas = macroanimate::XflAtlas::load(path.as_ref())?;
        Ok(from_xfl::convert(&atlas))
    }

    /// The animation named `name`, if present.
    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.animations.iter().find(|a| a.name == name)
    }

    /// The symbol named `name`, if present.
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name)
    }

    /// Compact JSON snapshot of the document.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("animation_ir is always serializable")
    }

    /// Pretty JSON snapshot of the document.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("animation_ir is always serializable")
    }
}
