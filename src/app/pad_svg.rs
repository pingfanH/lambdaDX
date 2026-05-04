use macroquad::prelude::{draw_line, draw_triangle, Color, Vec2, vec2};

// SVG pad geometry constants (from the viewBox and bg circle)
const SVG_BG_CX: f32 = 422.9;
const SVG_BG_CY: f32 = 348.8;
const SVG_BG_R: f32 = 326.57;

/// A single parsed touch zone from the SVG.
#[derive(Debug, Clone)]
pub(crate) struct ZoneDef {
    pub(crate) zone: u8,
    pub(crate) label: String,
    /// Polygon vertices in SVG viewBox coordinates.
    pub(crate) svg_verts: Vec<Vec2>,
    /// Precomputed polygon centroid in SVG coordinates.
    pub(crate) centroid: Vec2,
}

/// Parsed SVG definition for the entire touch pad.
#[derive(Debug, Clone)]
pub(crate) struct PadSvgDef {
    pub(crate) zones: Vec<ZoneDef>,
}

impl PadSvgDef {
    pub(crate) fn from_svg_str(svg_xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(svg_xml)
            .map_err(|e| format!("XML parse error: {e}"))?;

        // Find the <g id="touch"> element
        let touch_group = doc
            .descendants()
            .find(|n| n.is_element() && n.attribute("id") == Some("touch"))
            .ok_or_else(|| "SVG missing <g id=\"touch\"> element".to_string())?;

        let mut zones: Vec<ZoneDef> = Vec::new();

        for child in touch_group.children() {
            if !child.is_element() {
                continue;
            }
            collect_zone_elements(child, &mut zones)?;
        }

        if zones.is_empty() {
            return Err("No touch zones found in SVG".to_string());
        }

        Ok(PadSvgDef { zones })
    }

    pub(crate) fn zone_def(&self, zone: u8) -> Option<&ZoneDef> {
        self.zones.iter().find(|z| z.zone == zone)
    }

    pub(crate) fn zone_screen_verts(&self, zone: u8, pad: &super::types::PadGeom) -> Option<Vec<Vec2>> {
        let def = self.zone_def(zone)?;
        Some(def.svg_verts.iter().map(|&v| svg_to_screen(v, pad)).collect())
    }

    pub(crate) fn zone_screen_centroid(&self, zone: u8, pad: &super::types::PadGeom) -> Option<Vec2> {
        let def = self.zone_def(zone)?;
        Some(svg_to_screen(def.centroid, pad))
    }

    /// Hit-test a screen-space point against all zones.
    /// Returns the zone number if the point is inside a zone polygon.
    /// Transform a single ZoneDef's vertices and centroid to screen coordinates.
    pub(crate) fn def_screen_verts(&self, def: &ZoneDef, pad: &super::types::PadGeom) -> Vec<Vec2> {
        def.svg_verts.iter().map(|&v| svg_to_screen(v, pad)).collect()
    }

    pub(crate) fn def_screen_centroid(&self, def: &ZoneDef, pad: &super::types::PadGeom) -> Vec2 {
        svg_to_screen(def.centroid, pad)
    }

    pub(crate) fn hit_test(&self, screen_point: Vec2, pad: &super::types::PadGeom) -> Option<u8> {
        let svg_pt = screen_to_svg(screen_point, pad);

        for def in &self.zones {
            if point_in_polygon(svg_pt, &def.svg_verts) {
                return Some(def.zone);
            }
        }
        None
    }
}

/// Recursively collect zone elements. Handles `<g>` wrappers (like C1).
fn collect_zone_elements(
    node: roxmltree::Node,
    zones: &mut Vec<ZoneDef>,
) -> Result<(), String> {
    let tag = node.tag_name().name();

    match tag {
        "polygon" => {
            if let Some(zone_info) = parse_zone_element(node) {
                zones.push(zone_info);
            }
            // If no zone info, skip (might be inside an unrecognized group)
        }
        "rect" => {
            if let Some(zone_info) = parse_zone_element(node) {
                zones.push(zone_info);
            }
        }
        "g" => {
            // Recurse into group children
            for child in node.children() {
                if child.is_element() {
                    collect_zone_elements(child, zones)?;
                }
            }
        }
        "circle" => {
            // Skip — this is the background circle, not a zone
        }
        _ => {
            // Skip unknown elements
        }
    }

    Ok(())
}

/// Parse a polygon or rect element into a ZoneDef.
fn parse_zone_element(node: roxmltree::Node) -> Option<ZoneDef> {
    let id = node.attribute("id")?;

    // Skip the background circle by id
    if id == "bg" {
        return None;
    }

    let (zone, label) = svg_id_to_zone(id)?;

    let svg_verts = match node.tag_name().name() {
        "polygon" => parse_polygon_points(node.attribute("points")?)?,
        "rect" => {
            let x: f32 = node.attribute("x")?.parse().ok()?;
            let y: f32 = node.attribute("y")?.parse().ok()?;
            let w: f32 = node.attribute("width")?.parse().ok()?;
            let h: f32 = node.attribute("height")?.parse().ok()?;

            let mut corners = [
                vec2(x, y),
                vec2(x + w, y),
                vec2(x + w, y + h),
                vec2(x, y + h),
            ];

            if let Some(transform_str) = node.attribute("transform") {
                corners = apply_rect_transform(transform_str, &corners);
            }

            corners.to_vec()
        }
        _ => return None,
    };

    if svg_verts.len() < 3 {
        return None;
    }

    // Compute centroid
    let centroid = {
        let sum = svg_verts
            .iter()
            .fold(vec2(0.0, 0.0), |acc, &v| acc + v);
        sum / svg_verts.len() as f32
    };

    Some(ZoneDef {
        zone,
        label: label.to_string(),
        svg_verts,
        centroid,
    })
}

/// Parse SVG polygon points attribute.
/// Format: "x1 y1 x2 y2 x3 y3 ..." (space-separated alternating x,y)
fn parse_polygon_points(points_str: &str) -> Option<Vec<Vec2>> {
    let nums: Vec<f32> = points_str
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();

    if nums.len() < 6 || nums.len() % 2 != 0 {
        return None;
    }

    Some(
        nums.chunks(2)
            .map(|chunk| vec2(chunk[0], chunk[1]))
            .collect(),
    )
}

/// Parse and apply a rect transform string like "translate(tx ty) rotate(angle)".
fn apply_rect_transform(transform_str: &str, corners: &[Vec2; 4]) -> [Vec2; 4] {
    if transform_str.is_empty() {
        return *corners;
    }

    // Parse individual transform commands
    #[derive(Debug)]
    enum XfCmd {
        Translate(f32, f32),
        Rotate(f32), // radians
    }

    let mut cmds: Vec<XfCmd> = Vec::new();

    for part in transform_str.split(')') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(args) = part.strip_prefix("translate(") {
            let nums: Vec<f32> = args
                .split_whitespace()
                .filter_map(|s| s.trim().trim_end_matches(',').parse::<f32>().ok())
                .collect();
            if nums.len() >= 2 {
                cmds.push(XfCmd::Translate(nums[0], nums[1]));
            }
        } else if let Some(args) = part.strip_prefix("rotate(") {
            if let Ok(deg) = args.trim().parse::<f32>() {
                cmds.push(XfCmd::Rotate(deg.to_radians()));
            }
        }
    }

    // SVG applies transforms right-to-left: for "translate(A) rotate(B)",
    // the visual result is translate(rotate(point)). We iterate the parsed
    // commands in order and build: cmd_n(...(cmd_1(point))). Since we parsed
    // left-to-right into `cmds` (translate then rotate), that's already correct:
    // first apply rotate (cmds[1]), then translate (cmds[0])? No — for
    // "translate(A) rotate(B)" parsed into [Translate, Rotate], SVG applies
    // rotate first then translate. So we reverse: apply in reverse order.
    cmds.reverse();

    let transform_point = |p: Vec2| -> Vec2 {
        let mut result = p;
        for cmd in &cmds {
            match *cmd {
                XfCmd::Translate(tx, ty) => {
                    result = vec2(result.x + tx, result.y + ty);
                }
                XfCmd::Rotate(rad) => {
                    let (s, c) = (rad.sin(), rad.cos());
                    result = vec2(
                        result.x * c - result.y * s,
                        result.x * s + result.y * c,
                    );
                }
            }
        }
        result
    };

    corners.map(transform_point)
}

/// Map SVG element ID to game zone number and display label.
fn svg_id_to_zone(id: &str) -> Option<(u8, &'static str)> {
    match id {
        // Outer ring (zones 1-8)
        "A1" => Some((1, "A1")),
        "A2" => Some((2, "A2")),
        "A3" => Some((3, "A3")),
        "A4" => Some((4, "A4")),
        "A5" => Some((5, "A5")),
        "A6" => Some((6, "A6")),
        "A7" => Some((7, "A7")),
        "A8" => Some((8, "A8")),
        // Inner ring (zones 9-16)
        "B1" => Some((9, "B1")),
        "B2" => Some((10, "B2")),
        "B3" => Some((11, "B3")),
        "B4" => Some((12, "B4")),
        "B5" => Some((13, "B5")),
        "B6" => Some((14, "B6")),
        "B7" => Some((15, "B7")),
        "B8" => Some((16, "B8")),
        // Center — both halves map to zone 17
        "C1" => Some((17, "C1")),
        "C2" => Some((17, "C2")),
        // Left wing (zones 18-25)
        "D1" => Some((18, "D1")),
        "D2" => Some((19, "D2")),
        "D3" => Some((20, "D3")),
        "D4" => Some((21, "D4")),
        "D5" => Some((22, "D5")),
        "D6" => Some((23, "D6")),
        "D7" => Some((24, "D7")),
        "D8" => Some((25, "D8")),
        // Right wing (zones 26-33)
        "E1" => Some((26, "E1")),
        "E1-2" => Some((27, "E2")),
        "E1-3" => Some((28, "E3")),
        "E1-4" => Some((29, "E4")),
        "E1-5" => Some((30, "E5")),
        "E1-6" => Some((31, "E6")),
        "E1-7" => Some((32, "E7")),
        "E1-8" => Some((33, "E8")),
        _ => None,
    }
}

/// Transform a point from SVG viewBox coordinates to screen coordinates.
fn svg_to_screen(svg_pt: Vec2, pad: &super::types::PadGeom) -> Vec2 {
    let scale = pad.outer_r / SVG_BG_R;
    vec2(
        pad.cx + (svg_pt.x - SVG_BG_CX) * scale,
        pad.cy + (svg_pt.y - SVG_BG_CY) * scale,
    )
}

/// Transform a point from screen coordinates to SVG viewBox coordinates.
fn screen_to_svg(screen_pt: Vec2, pad: &super::types::PadGeom) -> Vec2 {
    let inv_scale = SVG_BG_R / pad.outer_r;
    vec2(
        SVG_BG_CX + (screen_pt.x - pad.cx) * inv_scale,
        SVG_BG_CY + (screen_pt.y - pad.cy) * inv_scale,
    )
}

/// Ray-casting point-in-polygon test.
fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (xi, yi) = (polygon[i].x, polygon[i].y);
        let (xj, yj) = (polygon[j].x, polygon[j].y);
        if ((yi > point.y) != (yj > point.y))
            && (point.x < (xj - xi) * (point.y - yi) / (yj - yi) + xi)
        {
            inside = !inside;
        }
    }
    inside
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Draw a filled convex polygon using triangle fan from the first vertex.
pub(crate) fn draw_polygon_fill(verts: &[Vec2], color: Color) {
    if verts.len() < 3 {
        return;
    }
    let v0 = verts[0];
    for i in 1..verts.len() - 1 {
        draw_triangle(v0, verts[i], verts[i + 1], color);
    }
}

/// Draw the outline of a polygon as a closed line loop.
pub(crate) fn draw_polygon_lines(verts: &[Vec2], thickness: f32, color: Color) {
    if verts.len() < 2 {
        return;
    }
    for i in 0..verts.len() {
        let j = (i + 1) % verts.len();
        draw_line(verts[i].x, verts[i].y, verts[j].x, verts[j].y, thickness, color);
    }
}
