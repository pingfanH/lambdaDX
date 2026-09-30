//! XFL XML parsing: `DOMDocument.xml` + `LIBRARY/*.xml` → [`XflAtlas`].

use std::collections::HashMap;

use roxmltree::Node;

use super::model::*;
use super::shape;
use super::XflError;

type NodeRef<'a, 'i> = Node<'a, 'i>;

/// XFL stores shape (`edges`) coordinates in twips: 1/20 of a pixel.
/// Instance matrices (`a b c d tx ty`) are already in pixels.
const TWIPS_PER_PX: f32 = 20.0;

/// Parse a whole project from its in-memory file set (`path` → bytes).
pub fn parse(files: &HashMap<String, Vec<u8>>) -> Result<XflAtlas, XflError> {
    let (doc_key, doc_text) = find_document(files)?;
    let doc = roxmltree::Document::parse(doc_text)
        .map_err(|e| XflError(format!("DOMDocument.xml parse error: {e}")))?;
    let root = doc.root_element();

    let frame_rate = attr_f32(root, "frameRate").unwrap_or(24.0);
    let width = attr_f32(root, "width").unwrap_or(550.0);
    let height = attr_f32(root, "height").unwrap_or(400.0);

    let mut bitmaps: HashMap<String, BitmapAsset> = HashMap::new();
    for item in root.descendants().filter(|n| n.tag_name().name() == "DOMBitmapItem") {
        let Some(name) = item.attribute("name") else {
            continue;
        };
        let Some(href) = item.attribute("href") else {
            continue;
        };
        let path = resolve(files, &doc_key, href).unwrap_or_else(|| href.replace('\\', "/"));
        // Embedded bitmap data lives in `bin/<bitmapDataHRef>`.
        let dat = item
            .attribute("bitmapDataHRef")
            .filter(|s| !s.is_empty())
            .and_then(|name| {
                let wanted = format!("bin/{}", name.replace('\\', "/"));
                let want_base = base_name(name).to_ascii_lowercase();
                files
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(&wanted))
                    .or_else(|| {
                        files
                            .keys()
                            .find(|k| base_name(k).to_ascii_lowercase() == want_base)
                    })
                    .cloned()
            })
            .and_then(|key| files.get(&key).cloned());
        bitmaps.insert(
            name.to_string(),
            BitmapAsset {
                name: name.to_string(),
                bytes: files.get(&path).cloned(),
                path,
                external: item
                    .attribute("sourceExternalFilepath")
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                dat,
            },
        );
    }

    let mut symbols: HashMap<String, (SymbolType, Timeline)> = HashMap::new();
    for inc in root.descendants().filter(|n| n.tag_name().name() == "Include") {
        let Some(href) = inc.attribute("href") else {
            continue;
        };
        let Some(key) = resolve(files, &doc_key, href) else {
            continue;
        };
        let Some(text) = files.get(&key).and_then(|b| std::str::from_utf8(b).ok()) else {
            continue;
        };
        let Ok(sym) = roxmltree::Document::parse(text) else {
            continue;
        };
        let sym_root = sym.root_element();
        let sym_name = sym_root
            .attribute("name")
            .map(str::to_string)
            .or_else(|| {
                inc.attribute("href")
                    .map(|h| base_name(h).trim_end_matches(".xml").to_string())
            });
        let Some(sym_name) = sym_name else { continue };
        let sym_type = parse_symbol_type(sym_root.attribute("symbolType"));
        let timeline = parse_timeline(first_descendant(sym_root, "DOMTimeline"));
        symbols.insert(sym_name, (sym_type, timeline));
    }

    let scene = parse_timeline(first_descendant(root, "DOMTimeline"));

    let mut animations: HashMap<String, Animation> = HashMap::new();
    animations.insert(
        "Scene 1".to_string(),
        Animation {
            frames: scene.frame_count(),
            start_frame: 0,
            target: AnimTarget::Scene,
        },
    );
    for (name, (_, timeline)) in &symbols {
        animations.insert(
            name.clone(),
            Animation {
                frames: timeline.frame_count(),
                start_frame: 0,
                target: AnimTarget::Symbol(name.clone()),
            },
        );
    }
    // Frame labels on the main scene become named animations.
    let scene_len = scene.frame_count();
    for (label, index) in scene_labels(&scene) {
        animations.insert(
            label,
            Animation {
                frames: scene_len.saturating_sub(index).max(1),
                start_frame: index,
                target: AnimTarget::Scene,
            },
        );
    }

    Ok(XflAtlas {
        frame_rate,
        width,
        height,
        scene,
        symbols,
        bitmaps,
        animations,
    })
}

/// Frame labels declared on the main timeline: `(name, frame index)`.
fn scene_labels(scene: &Timeline) -> Vec<(String, usize)> {
    scene
        .layers
        .iter()
        .flat_map(|l| l.frames.iter())
        .filter_map(|f| f.label.as_ref().map(|name| (name.clone(), f.index)))
        .collect()
}

fn parse_symbol_type(v: Option<&str>) -> SymbolType {
    match v.unwrap_or("movieclip").to_ascii_lowercase().as_str() {
        "graphic" => SymbolType::Graphic,
        "button" => SymbolType::Button,
        _ => SymbolType::MovieClip,
    }
}

fn parse_loop(v: Option<&str>) -> LoopMode {
    match v.unwrap_or("loop").to_ascii_lowercase().as_str() {
        "playonce" | "play_once" => LoopMode::PlayOnce,
        "singleframe" | "single_frame" => LoopMode::SingleFrame,
        _ => LoopMode::Loop,
    }
}

fn parse_timeline(node: Option<NodeRef>) -> Timeline {
    let Some(node) = node else {
        return Timeline::default();
    };
    let name = node.attribute("name").unwrap_or("").to_string();
    let layers = first_child(node, "layers")
        .map(|l| elements(l).collect::<Vec<_>>())
        .unwrap_or_else(|| elements(node).collect());
    let layers = layers
        .into_iter()
        .filter(|n| n.tag_name().name() == "DOMLayer")
        .map(parse_layer)
        .collect();
    Timeline { name, layers }
}

fn parse_layer(node: NodeRef) -> Layer {
    let name = node.attribute("name").unwrap_or("").to_string();
    let frames_src = first_child(node, "frames")
        .map(|f| elements(f).collect::<Vec<_>>())
        .unwrap_or_else(|| elements(node).collect());
    let mut frames: Vec<Frame> = frames_src
        .into_iter()
        .filter(|n| n.tag_name().name() == "DOMFrame")
        .map(parse_frame)
        .collect();
    frames.sort_by_key(|f| f.index);
    Layer { name, frames }
}

fn parse_frame(node: NodeRef) -> Frame {
    let index = attr_usize(node, "index").unwrap_or(0);
    let duration = attr_usize(node, "duration").unwrap_or(1).max(1);
    let tween = node.attribute("tweenType").is_some();
    let elements = first_child(node, "elements")
        .map(|e| elements(e).filter_map(parse_element).collect())
        .unwrap_or_default();
    Frame {
        index,
        duration,
        tween,
        label: node.attribute("name").map(str::to_string),
        elements,
    }
}

fn parse_element(node: NodeRef) -> Option<Element> {
    match node.tag_name().name() {
        "DOMSymbolInstance" => {
            let name = node.attribute("libraryItemName")?.to_string();
            let matrix = instance_matrix(node);
            Some(Element::Instance {
                name,
                matrix,
                loop_mode: parse_loop(node.attribute("loop")),
                first_frame: attr_usize(node, "firstFrame").unwrap_or(0),
                alpha: parse_alpha(node),
            })
        }
        "DOMBitmapInstance" => {
            let name = node.attribute("libraryItemName")?.to_string();
            Some(Element::Bitmap {
                name,
                matrix: instance_matrix(node),
                alpha: parse_alpha(node),
            })
        }
        "DOMShape" => Some(parse_shape(node)),
        "DOMGroup" => {
            let matrix = instance_matrix(node);
            let members = first_child(node, "members")
                .map(|m| elements(m).filter_map(parse_element).collect())
                .unwrap_or_default();
            Some(Element::Group {
                matrix,
                members,
                alpha: parse_alpha(node),
            })
        }
        _ => None,
    }
}

fn parse_shape(node: NodeRef) -> Element {
    let matrix = instance_matrix(node);
    let fills = parse_fills(node);
    let strokes = parse_strokes(node);

    let mut paths = Vec::new();
    for edge in node.descendants().filter(|n| n.tag_name().name() == "Edge") {
        let Some(text) = edge.attribute("edges") else {
            continue;
        };
        // XFL shape coordinates are in twips (1/20 px); matrices are in px.
        let points: Vec<[f32; 2]> = shape::parse_edge_path(text)
            .iter()
            .map(|p| [p[0] / TWIPS_PER_PX, p[1] / TWIPS_PER_PX])
            .collect();
        if points.len() < 3 {
            continue;
        }
        let fill = attr_usize(edge, "fillStyle1")
            .or_else(|| attr_usize(edge, "fillStyle0"))
            .and_then(|i| fills.get(&i).copied());
        let stroke = attr_usize(edge, "strokeStyle").and_then(|i| strokes.get(&i).copied());
        paths.push(ShapePath {
            points,
            fill,
            stroke,
        });
    }
    Element::Shape { matrix, paths }
}

fn parse_fills(node: NodeRef) -> HashMap<usize, SolidColor> {
    let mut out = HashMap::new();
    let Some(fills) = first_child(node, "fills") else {
        return out;
    };
    for style in elements(fills).filter(|n| n.tag_name().name() == "FillStyle") {
        let Some(idx) = attr_usize(style, "index") else {
            continue;
        };
        if let Some(color) = find_solid_color(style) {
            out.insert(idx, color);
        }
    }
    out
}

fn parse_strokes(node: NodeRef) -> HashMap<usize, (SolidColor, f32)> {
    let mut out = HashMap::new();
    let Some(strokes) = first_child(node, "strokes") else {
        return out;
    };
    for style in elements(strokes).filter(|n| n.tag_name().name() == "StrokeStyle") {
        let Some(idx) = attr_usize(style, "index") else {
            continue;
        };
        let stroke = first_child(style, "SolidStroke");
        let weight = stroke
            .and_then(|s| attr_f32(s, "weight"))
            .unwrap_or(1.0);
        if let Some(color) = find_solid_color(style) {
            out.insert(idx, (color, weight));
        }
    }
    out
}

/// First `<SolidColor>` anywhere under `node`.
fn find_solid_color(node: NodeRef) -> Option<SolidColor> {
    elements(node)
        .filter(|n| n.tag_name().name() == "SolidColor")
        .find_map(|c| {
            let hex = c.attribute("color").unwrap_or("#000000");
            let (r, g, b) = parse_hex(hex)?;
            Some(SolidColor {
                r,
                g,
                b,
                a: attr_f32(c, "alpha").unwrap_or(1.0),
            })
        })
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

/// An instance's matrix, corrected for its transformation point.
fn instance_matrix(node: NodeRef) -> Matrix {
    let matrix = parse_matrix(node);
    let tp = first_child(node, "transformationPoint")
        .and_then(|p| first_child(p, "Point"))
        .map(|p| [attr_f32(p, "x").unwrap_or(0.0), attr_f32(p, "y").unwrap_or(0.0)])
        .unwrap_or([0.0, 0.0]);
    // `world = M * (p - P) + P`
    let [a, b, c, d, tx, ty] = matrix;
    let [px, py] = tp;
    [a, b, c, d, tx - (a * px + c * py) + px, ty - (b * px + d * py) + py]
}

/// An instance's opacity from `<color><Color alphaMultiplier="…"/></color>`.
fn parse_alpha(node: NodeRef) -> f32 {
    node.descendants()
        .find(|n| n.tag_name().name() == "Color")
        .and_then(|c| attr_f32(c, "alphaMultiplier"))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

/// Read `<matrix><Matrix .../></matrix>` (identity when absent).
fn parse_matrix(node: NodeRef) -> Matrix {
    let Some(mx) = first_child(node, "matrix") else {
        return IDENTITY;
    };
    let Some(m) = first_child(mx, "Matrix") else {
        return IDENTITY;
    };
    [
        attr_f32(m, "a").unwrap_or(1.0),
        attr_f32(m, "b").unwrap_or(0.0),
        attr_f32(m, "c").unwrap_or(0.0),
        attr_f32(m, "d").unwrap_or(1.0),
        attr_f32(m, "tx").unwrap_or(0.0),
        attr_f32(m, "ty").unwrap_or(0.0),
    ]
}

// ── XML helpers ───────────────────────────────────────────────────────────

fn first_child<'a, 'i>(node: NodeRef<'a, 'i>, name: &str) -> Option<NodeRef<'a, 'i>> {
    elements(node).find(|n| n.tag_name().name() == name)
}

fn first_descendant<'a, 'i>(node: NodeRef<'a, 'i>, name: &str) -> Option<NodeRef<'a, 'i>> {
    node.descendants().find(|n| n.tag_name().name() == name)
}

pub(crate) fn elements<'a, 'i>(
    node: NodeRef<'a, 'i>,
) -> impl Iterator<Item = NodeRef<'a, 'i>> {
    node.children().filter(|n| n.is_element())
}

fn attr_f32(node: NodeRef, name: &str) -> Option<f32> {
    node.attribute(name)?.trim().parse().ok()
}

fn attr_usize(node: NodeRef, name: &str) -> Option<usize> {
    node.attribute(name)?.trim().parse().ok()
}

fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

// ── File resolution ───────────────────────────────────────────────────────

fn find_document(files: &HashMap<String, Vec<u8>>) -> Result<(String, &str), XflError> {
    let key = files
        .keys()
        .find(|k| k.to_ascii_lowercase().ends_with("domdocument.xml"))
        .ok_or_else(|| XflError("no DOMDocument.xml found in project".into()))?;
    let text = std::str::from_utf8(&files[key])
        .map_err(|e| XflError(format!("DOMDocument.xml is not utf-8: {e}")))?;
    Ok((key.clone(), text))
}

/// Resolve an `href` relative to `base` against the project's file set, falling
/// back to a case-insensitive basename match (Adobe often drops the `LIBRARY/`
/// prefix in `<Include href>`).
pub fn resolve(files: &HashMap<String, Vec<u8>>, base: &str, href: &str) -> Option<String> {
    let href = href.replace('\\', "/");
    let base_dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let candidate = if href.starts_with('/') {
        href.trim_start_matches('/').to_string()
    } else if base_dir.is_empty() {
        href.clone()
    } else {
        format!("{base_dir}/{href}")
    };
    if let Some((k, _)) = files
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&candidate))
    {
        return Some(k.clone());
    }
    let wanted = base_name(&href).to_ascii_lowercase();
    files
        .keys()
        .find(|k| base_name(k).to_ascii_lowercase() == wanted)
        .cloned()
}
