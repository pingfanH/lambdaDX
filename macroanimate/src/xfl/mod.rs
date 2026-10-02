//! Adobe Animate XFL / FLA parsing and rendering for Macroquad.
//!
//! An `.fla` is a ZIP of an **XFL** project; an XFL project is also usable as an
//! unpacked directory. Both contain `DOMDocument.xml`, a `LIBRARY/` folder with
//! one XML per symbol, and the bitmap files the symbols reference.
//!
//! ```no_run
//! use macroanimate::xfl::{XflAtlas, draw_parts};
//! # async fn demo() {
//! let atlas = XflAtlas::load("assets/ui").unwrap();
//! let parts = atlas.parts("Scene 1", 0);
//! // Textures are loaded by the caller (one per BitmapAsset) and passed in.
//! # let textures = std::collections::HashMap::new();
//! draw_parts(&parts, &textures, (0.0, 0.0));
//! # }
//! ```

mod dat;
mod eval;
mod model;
mod parse;
pub mod shape;

pub use model::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::{Color, Image, Texture2D, draw_line, vec2, vec3, vec4};

use crate::texture_atlas::{AnimateSprite, draw_part_mesh_tinted};

/// Anything that can go wrong loading an XFL project.
#[derive(Debug)]
pub struct XflError(pub String);

impl fmt::Display for XflError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XFL error: {}", self.0)
    }
}

impl std::error::Error for XflError {}

impl XflAtlas {
    /// Load a project from an XFL directory or a `.fla`/`.zip` file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, XflError> {
        let path = path.as_ref();
        if path.is_dir() {
            Self::from_dir(path)
        } else {
            let bytes = std::fs::read(path)
                .map_err(|e| XflError(format!("cannot read {}: {e}", path.display())))?;
            Self::from_fla_bytes(&bytes)
        }
    }

    /// Load an unpacked XFL directory.
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, XflError> {
        parse::parse(&read_dir_files(dir.as_ref())?)
    }

    /// Load a `.fla`/ZIP archive from raw bytes.
    pub fn from_fla_bytes(bytes: &[u8]) -> Result<Self, XflError> {
        parse::parse(&read_zip_files(bytes)?)
    }
}

/// Read a bitmap's original import path when it is not stored in the project.
fn read_external(asset: &BitmapAsset) -> std::io::Result<Vec<u8>> {
    match &asset.external {
        Some(path) => std::fs::read(path),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no external path",
        )),
    }
}

/// Read an XFL directory tree into `relative path -> bytes`.
fn read_dir_files(dir: &Path) -> Result<HashMap<String, Vec<u8>>, XflError> {
    let mut files = HashMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|e| XflError(format!("cannot read {}: {e}", current.display())))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(dir) {
                let key = normalize(rel.to_string_lossy().as_ref());
                if let Ok(bytes) = std::fs::read(&path) {
                    files.insert(key, bytes);
                }
            }
        }
    }
    Ok(files)
}

/// Read a `.fla`/ZIP archive into `relative path -> bytes`.
fn read_zip_files(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, XflError> {
    use std::io::Read;

    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| XflError(format!("invalid FLA/ZIP: {e}")))?;
    let mut files = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| XflError(format!("corrupt FLA entry {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let key = normalize(entry.name());
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| XflError(format!("cannot read FLA entry {}: {e}", entry.name())))?;
        files.insert(key, buf);
    }
    Ok(files)
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

// ── Rendering ─────────────────────────────────────────────────────────────

/// Draw every part, offset so the project's `(0, 0)` lands at `origin`.
///
/// `textures` maps a [`BitmapAsset::name`] to its loaded texture. Missing
/// bitmaps are skipped.
pub fn draw_parts(parts: &[DrawPart], textures: &HashMap<String, Texture2D>, origin: (f32, f32)) {
    draw_parts_tinted(parts, textures, origin, [255, 255, 255, 255]);
}

/// Like [`draw_parts`] but multiplies every part by an RGBA `tint`.
pub fn draw_parts_tinted(
    parts: &[DrawPart],
    textures: &HashMap<String, Texture2D>,
    origin: (f32, f32),
    tint: [u8; 4],
) {
    for part in parts {
        draw_part_tinted(part, textures, origin, tint);
    }
}

/// Draw a single evaluated part with a white tint.
pub fn draw_part(part: &DrawPart, textures: &HashMap<String, Texture2D>, origin: (f32, f32)) {
    draw_part_tinted(part, textures, origin, [255, 255, 255, 255]);
}

/// Draw a single evaluated part, multiplying its colors by `tint` and its
/// opacity by [`DrawPart::alpha`].
pub fn draw_part_tinted(
    part: &DrawPart,
    textures: &HashMap<String, Texture2D>,
    origin: (f32, f32),
    tint: [u8; 4],
) {
    match &part.content {
        PartContent::Bitmap(name) => {
            let Some(texture) = textures.get(name) else {
                return;
            };
            let sprite = AnimateSprite {
                x: 0.0,
                y: 0.0,
                w: texture.width(),
                h: texture.height(),
                rotated: false,
            };
            let alpha = (part.alpha.clamp(0.0, 1.0) * tint[3] as f32 / 255.0 * 255.0) as u8;
            draw_part_mesh_tinted(
                texture,
                &sprite,
                &part.matrix,
                texture.width(),
                texture.height(),
                origin.0,
                origin.1,
                [tint[0], tint[1], tint[2], alpha],
            );
        }
        PartContent::Vector(paths) => draw_vector(paths, &part.matrix, origin, part.alpha, tint),
        // Text is rasterized by the caller (see `XflAsset::text_draws`).
        PartContent::Text(_) => {}
    }
}

/// Draw filled/stroked polygons (fan-triangulated, suited to convex outlines).
///
/// Colors are multiplied by `tint` and `alpha` (the part's opacity).
pub fn draw_vector(
    paths: &[ShapePath],
    matrix: &Matrix,
    origin: (f32, f32),
    alpha: f32,
    tint: [u8; 4],
) {
    draw_vector_ex(paths, matrix, origin, alpha, tint, None);
}

/// Like [`draw_vector`] but `rgb_override` **replaces** each path's fill/stroke
/// RGB (alpha still comes from the path, times `alpha`/`mul`). Used to tint a
/// loaded movie by grade while sharing the cached geometry.
pub(crate) fn draw_vector_ex(
    paths: &[ShapePath],
    matrix: &Matrix,
    origin: (f32, f32),
    alpha: f32,
    mul: [u8; 4],
    rgb_override: Option<[u8; 3]>,
) {
    let [a, b, c, d, tx, ty] = *matrix;
    let transform = |p: [f32; 2]| {
        vec3(
            origin.0 + a * p[0] + c * p[1] + tx,
            origin.1 + b * p[0] + d * p[1] + ty,
            0.0,
        )
    };
    let modulate = |rgba: [u8; 4]| -> [u8; 4] {
        let base = rgb_override.unwrap_or([rgba[0], rgba[1], rgba[2]]);
        [
            (base[0] as f32 * mul[0] as f32 / 255.0).clamp(0.0, 255.0) as u8,
            (base[1] as f32 * mul[1] as f32 / 255.0).clamp(0.0, 255.0) as u8,
            (base[2] as f32 * mul[2] as f32 / 255.0).clamp(0.0, 255.0) as u8,
            (rgba[3] as f32 * alpha.clamp(0.0, 1.0) * mul[3] as f32 / 255.0)
                .clamp(0.0, 255.0) as u8,
        ]
    };

    for path in paths {
        let mut ring = path.points.clone();
        if ring.len() >= 2 && ring.first() == ring.last() {
            ring.pop();
        }
        if ring.len() < 3 {
            continue;
        }
        let vertices: Vec<Vertex> = ring
            .iter()
            .map(|p| Vertex {
                position: transform(*p),
                uv: vec2(0.0, 0.0),
                color: [255, 255, 255, 255],
                normal: vec4(0.0, 0.0, 1.0, 0.0),
            })
            .collect();

        if let Some(fill) = path.fill {
            let rgba = modulate(fill.rgba());
            let mut vertices = vertices.clone();
            for v in &mut vertices {
                v.color = rgba;
            }
            let indices: Vec<u16> = (1..ring.len() as u16 - 1)
                .flat_map(|i| [0, i, i + 1])
                .collect();
            draw_mesh(&Mesh {
                vertices,
                indices,
                texture: None,
            });
        }

        if let Some((stroke, weight)) = path.stroke {
            let [r, g, b, a] = modulate(stroke.rgba());
            let color = Color::from_rgba(r, g, b, a);
            for i in 0..ring.len() {
                let p0 = vertices[i].position;
                let p1 = vertices[(i + 1) % ring.len()].position;
                draw_line(p0.x, p0.y, p1.x, p1.y, weight, color);
            }
        }
    }
}

/// A global transform + colour override for drawing a loaded movie.
#[derive(Clone, Copy, Debug)]
pub struct XflDrawXf {
    /// Screen position of the movie's origin.
    pub pos: (f32, f32),
    pub scale: f32,
    pub rotation: f32,
    pub alpha: f32,
    /// Replace vector fill/stroke RGB (and bitmap vertex colour). `None` keeps
    /// the colours authored in the project.
    pub tint: Option<[u8; 3]>,
}

impl Default for XflDrawXf {
    fn default() -> Self {
        Self {
            pos: (0.0, 0.0),
            scale: 1.0,
            rotation: 0.0,
            alpha: 1.0,
            tint: None,
        }
    }
}

/// Per-frame sampled-parts cache keyed by `(clip, frame)`.
type FrameCache = HashMap<(String, usize), Rc<Vec<DrawPart>>>;

/// A loaded XFL project plus its textures and a per-frame geometry cache.
///
/// Movies/sprites are referenced **by name inside the project**, e.g.
/// `ui.get("tap_perfect")`, not by file path.
pub struct XflAsset {
    atlas: XflAtlas,
    textures: HashMap<String, Texture2D>,
    cache: RefCell<FrameCache>,
}

impl XflAsset {
    /// Load from an XFL directory or a `.fla`/`.zip` file and build every
    /// bitmap texture the project references.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, XflError> {
        let path = path.as_ref();
        let atlas = XflAtlas::load(path)?;
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut textures = HashMap::new();
        for (name, asset) in &atlas.bitmaps {
            // Prefer the embedded `bin/*.dat`; fall back to a plain image file.
            let texture = if let Some(dat) = &asset.dat {
                match dat::decode(dat) {
                    Ok(dat::DatImage::Rgba {
                        width,
                        height,
                        bytes,
                    }) => Texture2D::from_image(&Image {
                        bytes,
                        width,
                        height,
                    }),
                    Ok(dat::DatImage::Encoded(bytes)) => {
                        Texture2D::from_file_with_format(&bytes, None)
                    }
                    Err(e) => {
                        eprintln!("xfl: bitmap {name}: {e}");
                        continue;
                    }
                }
            } else {
                let bytes = match &asset.bytes {
                    Some(bytes) => bytes.clone(),
                    None => match std::fs::read(dir.join(&asset.path))
                        .or_else(|_| read_external(asset))
                    {
                        Ok(bytes) => bytes,
                        // A missing image must not fail the whole project.
                        Err(e) => {
                            eprintln!("xfl: skipping bitmap {}: {e}", asset.path);
                            continue;
                        }
                    },
                };
                Texture2D::from_file_with_format(&bytes, None)
            };
            textures.insert(name.clone(), texture);
        }
        Ok(Self {
            atlas,
            textures,
            cache: RefCell::new(HashMap::new()),
        })
    }

    /// Document frame rate.
    pub fn frame_rate(&self) -> f32 {
        self.atlas.frame_rate
    }

    /// All movie/sprite names available in the project.
    pub fn clips(&self) -> impl Iterator<Item = &str> {
        self.atlas.animations.keys().map(String::as_str)
    }

    /// Get a named movie/sprite (e.g. `ui.get("tap_perfect")`).
    pub fn get(&self, name: &str) -> Option<XflClip<'_>> {
        let animation = self.atlas.animations.get(name)?;
        Some(XflClip {
            asset: self,
            name: name.to_string(),
            frames: animation.frames.max(1),
            fps: self.atlas.frame_rate,
        })
    }

    /// Sampled parts for a clip/frame, cached (geometry is time-invariant).
    pub fn frame_parts(&self, clip: &str, frame: usize) -> Rc<Vec<DrawPart>> {
        let key = (clip.to_string(), frame);
        if let Some(parts) = self.cache.borrow().get(&key) {
            return parts.clone();
        }
        let parts = Rc::new(self.atlas.parts(clip, frame));
        self.cache.borrow_mut().insert(key, parts.clone());
        parts
    }

    /// Draw a clip's frame with a global transform + optional tint.
    pub fn draw(&self, clip: &str, frame: usize, xf: &XflDrawXf) {
        let parts = self.frame_parts(clip, frame);
        draw_parts_xf(&parts, &self.textures, xf);
    }

    /// Static text of a clip/frame, already transformed into screen space.
    ///
    /// This crate has no font stack, so it does not rasterize text; the caller
    /// renders each [`TextDraw`] with its own font (e.g. macroquad).
    pub fn text_draws(&self, clip: &str, frame: usize, xf: &XflDrawXf) -> Vec<TextDraw> {
        let parts = self.frame_parts(clip, frame);
        let (sin, cos) = xf.rotation.sin_cos();
        let s = xf.scale;
        let g: Matrix = [s * cos, s * sin, -s * sin, s * cos, xf.pos.0, xf.pos.1];
        self.text_parts_xf(&parts, &g, xf.alpha)
    }

    fn text_parts_xf(&self, parts: &[DrawPart], m: &Matrix, alpha: f32) -> Vec<TextDraw> {
        let scale = (m[0] * m[0] + m[1] * m[1]).sqrt().max(1e-6);
        let mut out = Vec::new();
        for part in parts.iter() {
            if let PartContent::Text(run) = &part.content {
                let mm = mul(m, &part.matrix);
                out.push(TextDraw {
                    text: run.text.clone(),
                    pos: (mm[4], mm[5]),
                    size: run.size * scale,
                    color: run.color,
                    align: run.align,
                    width: run.width * scale,
                    alpha: part.alpha * alpha,
                });
            }
        }
        out
    }

    /// Draw a clip's frame with a raw 2×3 screen-space matrix (+ opacity).
    ///
    /// Unlike [`XflAsset::draw`] this allows non-uniform scale, which is what
    /// overlaying a state frame of a widget (e.g. a button scaled to its box)
    /// needs. Text is still skipped — use [`XflAsset::text_draws_matrix`].
    pub fn draw_matrix(&self, clip: &str, frame: usize, m: &Matrix, alpha: f32) {
        let parts = self.frame_parts(clip, frame);
        for part in parts.iter() {
            let mm = mul(m, &part.matrix);
            let a = part.alpha * alpha;
            match &part.content {
                PartContent::Bitmap(name) => {
                    let Some(texture) = self.textures.get(name) else {
                        continue;
                    };
                    let sprite = AnimateSprite {
                        x: 0.0,
                        y: 0.0,
                        w: texture.width(),
                        h: texture.height(),
                        rotated: false,
                    };
                    draw_part_mesh_tinted(
                        texture,
                        &sprite,
                        &mm,
                        texture.width(),
                        texture.height(),
                        0.0,
                        0.0,
                        [255, 255, 255, (a.clamp(0.0, 1.0) * 255.0) as u8],
                    );
                }
                PartContent::Vector(paths) => {
                    draw_vector_ex(paths, &mm, (0.0, 0.0), a, [255, 255, 255, 255], None)
                }
                PartContent::Text(_) => {}
            }
        }
    }

    /// Static text of a clip/frame under a raw screen-space matrix.
    pub fn text_draws_matrix(&self, clip: &str, frame: usize, m: &Matrix, alpha: f32) -> Vec<TextDraw> {
        let parts = self.frame_parts(clip, frame);
        self.text_parts_xf(&parts, m, alpha)
    }
}

/// A named movie/sprite inside an [`XflAsset`].
pub struct XflClip<'a> {
    asset: &'a XflAsset,
    name: String,
    frames: usize,
    fps: f32,
}

impl XflClip<'_> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn frames(&self) -> usize {
        self.frames
    }
    pub fn fps(&self) -> f32 {
        self.fps
    }
    /// Clip length in seconds.
    pub fn duration(&self) -> f32 {
        self.frames as f32 / self.fps.max(1.0)
    }
    pub fn parts(&self, frame: usize) -> Rc<Vec<DrawPart>> {
        self.asset.frame_parts(&self.name, frame)
    }
    pub fn draw(&self, frame: usize, xf: &XflDrawXf) {
        self.asset.draw(&self.name, frame, xf);
    }
    /// Transformed static text of this frame (see [`XflAsset::text_draws`]).
    pub fn text_draws(&self, frame: usize, xf: &XflDrawXf) -> Vec<TextDraw> {
        self.asset.text_draws(&self.name, frame, xf)
    }
}

/// Draw already-sampled parts with a global transform + tint.
pub fn draw_parts_xf(parts: &[DrawPart], textures: &HashMap<String, Texture2D>, xf: &XflDrawXf) {
    let (sin, cos) = xf.rotation.sin_cos();
    let s = xf.scale;
    let g: Matrix = [s * cos, s * sin, -s * sin, s * cos, xf.pos.0, xf.pos.1];
    for part in parts {
        let m = mul(&g, &part.matrix);
        let alpha = part.alpha * xf.alpha;
        match &part.content {
            PartContent::Bitmap(name) => {
                let Some(texture) = textures.get(name) else {
                    continue;
                };
                let rgb = xf.tint.unwrap_or([255, 255, 255]);
                let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
                let sprite = AnimateSprite {
                    x: 0.0,
                    y: 0.0,
                    w: texture.width(),
                    h: texture.height(),
                    rotated: false,
                };
                draw_part_mesh_tinted(
                    texture,
                    &sprite,
                    &m,
                    texture.width(),
                    texture.height(),
                    0.0,
                    0.0,
                    [rgb[0], rgb[1], rgb[2], a],
                );
            }
            PartContent::Vector(paths) => {
                draw_vector_ex(paths, &m, (0.0, 0.0), alpha, [255, 255, 255, 255], xf.tint)
            }
            // Text has no font stack here; the caller draws it (see
            // [`XflAsset::text_draws`]).
            PartContent::Text(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> HashMap<String, Vec<u8>> {
        let mut files = HashMap::new();
        files.insert(
            "DOMDocument.xml".to_string(),
            br#"<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" width="960" height="640" frameRate="60">
                 <symbols><Include href="tap.xml"/></symbols>
                 <media><DOMBitmapItem name="dot_png" href="LIBRARY/dot.png"/></media>
                 <timelines><DOMTimeline name="Scene 1"><layers>
                   <DOMLayer name="Layer_1"><frames>
                     <DOMFrame index="0" duration="2">
                       <elements>
                         <DOMSymbolInstance libraryItemName="tap" symbolType="graphic" loop="loop">
                           <matrix><Matrix ty="10"/></matrix>
                         </DOMSymbolInstance>
                       </elements>
                     </DOMFrame>
                   </frames></DOMLayer>
                 </layers></DOMTimeline></timelines>
               </DOMDocument>"#
                .to_vec(),
        );
        files.insert(
            "LIBRARY/tap.xml".to_string(),
            br#"<DOMSymbolItem xmlns="http://ns.adobe.com/xfl/2008/" name="tap" symbolType="graphic">
                 <timeline><DOMTimeline name="tap"><layers>
                   <DOMLayer name="Layer_1"><frames>
                     <DOMFrame index="0">
                       <elements>
                         <DOMBitmapInstance libraryItemName="dot_png">
                           <matrix><Matrix tx="100" ty="50"/></matrix>
                         </DOMBitmapInstance>
                       </elements>
                     </DOMFrame>
                   </frames></DOMLayer>
                 </layers></DOMTimeline></timeline>
               </DOMSymbolItem>"#
                .to_vec(),
        );
        files.insert(
            "LIBRARY/dot.png".to_string(),
            vec![0x89, b'P', b'N', b'G'], // bytes are opaque to the parser
        );
        files
    }

    #[test]
    fn resolves_library_include_without_prefix() {
        let atlas = parse::parse(&fixture()).expect("parse");
        assert!(atlas.symbols.contains_key("tap"));
        assert!(atlas.bitmaps.contains_key("dot_png"));
        let bmp = atlas.bitmap("dot_png").unwrap();
        assert_eq!(bmp.path, "LIBRARY/dot.png");
        assert!(bmp.bytes.is_some());
    }

    #[test]
    fn evaluates_nested_bitmap_transform() {
        let atlas = parse::parse(&fixture()).expect("parse");
        let parts = atlas.parts("Scene 1", 0);
        assert_eq!(parts.len(), 1);
        match &parts[0].content {
            PartContent::Bitmap(name) => assert_eq!(name, "dot_png"),
            _ => panic!("expected bitmap"),
        }
        // symbol ty=10 * bitmap tx=100 ty=50 -> (100, 60)
        assert!((parts[0].matrix[4] - 100.0).abs() < 1e-4);
        assert!((parts[0].matrix[5] - 60.0).abs() < 1e-4);
    }

    #[test]
    fn interpolates_motion_tween_rotation() {
        let mut files = fixture();
        files.insert(
            "DOMDocument.xml".to_string(),
            br#"<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" width="960" height="640" frameRate="60">
                 <timelines><DOMTimeline name="Scene 1"><layers>
                   <DOMLayer name="Layer_1"><frames>
                     <DOMFrame index="0" duration="10" tweenType="motion">
                       <elements><DOMBitmapInstance libraryItemName="dot_png">
                         <matrix><Matrix/></matrix></DOMBitmapInstance></elements>
                     </DOMFrame>
                     <DOMFrame index="10">
                       <elements><DOMBitmapInstance libraryItemName="dot_png">
                         <matrix><Matrix a="0" b="1" c="-1" d="0" tx="0" ty="0"/></matrix>
                       </DOMBitmapInstance></elements>
                     </DOMFrame>
                   </frames></DOMLayer>
                 </layers></DOMTimeline></timelines>
               </DOMDocument>"#
                .to_vec(),
        );
        let atlas = parse::parse(&files).expect("parse");
        let mid = atlas.parts("Scene 1", 5);
        assert_eq!(mid.len(), 1);
        // 45° in: a = cos45 ≈ 0.707, unit scale preserved.
        let scale = (mid[0].matrix[0].powi(2) + mid[0].matrix[1].powi(2)).sqrt();
        assert!((scale - 1.0).abs() < 1e-3, "scale {scale}");
        assert!((mid[0].matrix[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn loads_from_fla_zip_bytes() {
        use std::io::Write;

        let files = fixture();
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            for (name, bytes) in &files {
                writer.start_file(name, options).unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        let atlas = XflAtlas::from_fla_bytes(&buf).expect("fla loads");
        assert!(atlas.symbols.contains_key("tap"));
        assert!(atlas.bitmaps["dot_png"].bytes.is_some());
        assert_eq!(atlas.parts("Scene 1", 0).len(), 1);
    }

    #[test]
    fn parses_vector_fill_from_shape() {
        let mut files = fixture();
        files.insert(
            "LIBRARY/tap.xml".to_string(),
            br##"<DOMSymbolItem xmlns="http://ns.adobe.com/xfl/2008/" name="tap" symbolType="graphic">
                 <timeline><DOMTimeline name="tap"><layers>
                   <DOMLayer name="Layer_1"><frames>
                     <DOMFrame index="0"><elements>
                       <DOMShape>
                         <fills><FillStyle index="1"><SolidColor color="#FFCC00" alpha="1"/></FillStyle></fills>
                         <edges><Edge fillStyle1="1" edges="!0 0S1|10 0!10 0|10 10!10 10|0 10!0 10|0 0"/></edges>
                       </DOMShape>
                     </elements></DOMFrame>
                   </frames></DOMLayer>
                 </layers></DOMTimeline></timeline>
               </DOMSymbolItem>"##
                .to_vec(),
        );
        let atlas = parse::parse(&files).expect("parse");
        let parts = atlas.parts("Scene 1", 0);
        match &parts[0].content {
            PartContent::Vector(paths) => {
                assert_eq!(paths[0].points.len(), 5);
                assert_eq!(paths[0].fill.unwrap().r, 0xFF);
                assert_eq!(paths[0].fill.unwrap().g, 0xCC);
            }
            _ => panic!("expected vector"),
        }
    }
}
