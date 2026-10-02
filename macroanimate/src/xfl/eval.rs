//! Timeline evaluation: sample a timeline at a frame into [`DrawPart`]s.

use super::model::*;

impl XflAtlas {
    /// Look up a bitmap asset by library name.
    pub fn bitmap(&self, name: &str) -> Option<&BitmapAsset> {
        self.bitmaps.get(name)
    }

    /// Evaluate `anim` at `frame` into a flat list of drawable parts, parenting
    /// nested symbol instances and groups into a single transform.
    pub fn parts(&self, anim: &str, frame: usize) -> Vec<DrawPart> {
        let Some(animation) = self.animations.get(anim) else {
            return Vec::new();
        };
        let total = animation.frames.max(1);
        let frame = animation.start_frame + frame % total;
        let mut out = Vec::new();
        match &animation.target {
            AnimTarget::Scene => self.collect(&self.scene, frame, IDENTITY, 1.0, &mut out),
            AnimTarget::Symbol(name) => {
                if let Some((_, timeline)) = self.symbols.get(name) {
                    self.collect(timeline, frame, IDENTITY, 1.0, &mut out);
                }
            }
        }
        out
    }

    fn collect(
        &self,
        timeline: &Timeline,
        frame: usize,
        parent_matrix: Matrix,
        parent_alpha: f32,
        out: &mut Vec<DrawPart>,
    ) {
        for layer in timeline.layers.iter().rev() {
            for (element, matrix, alpha) in sampled_elements(&layer.frames, frame) {
                let combined = mul(&parent_matrix, &matrix);
                self.collect_element(&element, combined, parent_alpha * alpha, frame, out);
            }
        }
    }

    fn collect_element(
        &self,
        element: &Element,
        matrix: Matrix,
        alpha: f32,
        local_frame: usize,
        out: &mut Vec<DrawPart>,
    ) {
        match element {
            Element::Bitmap { name, .. } => out.push(DrawPart {
                content: PartContent::Bitmap(name.clone()),
                matrix,
                alpha,
            }),
            Element::Shape { paths, .. } => out.push(DrawPart {
                content: PartContent::Vector(paths.clone()),
                matrix,
                alpha,
            }),
            Element::Text(run) => out.push(DrawPart {
                content: PartContent::Text(run.clone()),
                matrix,
                alpha,
            }),
            Element::Group { members, .. } => {
                for member in members {
                    let child = mul(&matrix, &member.matrix());
                    self.collect_element(member, child, alpha * member.alpha(), local_frame, out);
                }
            }
            Element::Instance {
                name,
                loop_mode,
                first_frame,
                scale9,
                ..
            } => {
                let Some((_, timeline)) = self.symbols.get(name) else {
                    return;
                };
                let total = timeline.frame_count().max(1);
                let raw = local_frame + first_frame;
                let child_frame = match loop_mode {
                    LoopMode::Loop => raw % total,
                    LoopMode::PlayOnce => raw.min(total - 1),
                    LoopMode::SingleFrame => (*first_frame).min(total - 1),
                };
                // Instance-level grid (if any) overrides the symbol-level one.
                if let Some(grid) = (*scale9).or_else(|| self.scale9.get(name).copied()) {
                    // Collect the child in its own frame so the 9-slice can
                    // stretch its geometry between the grid lines.
                    let mut child = Vec::new();
                    self.collect(timeline, child_frame, IDENTITY, 1.0, &mut child);
                    let mut nat = [
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    ];
                    let mut has_geometry = false;
                    for p in &child {
                        if let PartContent::Vector(paths) = &p.content {
                            let m = p.matrix;
                            for path in paths {
                                for pt in &path.points {
                                    let x = m[0] * pt[0] + m[2] * pt[1] + m[4];
                                    let y = m[1] * pt[0] + m[3] * pt[1] + m[5];
                                    nat[0] = nat[0].min(x);
                                    nat[1] = nat[1].min(y);
                                    nat[2] = nat[2].max(x);
                                    nat[3] = nat[3].max(y);
                                    has_geometry = true;
                                }
                            }
                        }
                    }
                    if has_geometry {
                        out.push(DrawPart {
                            content: PartContent::NineSlice {
                                parts: child,
                                natural: nat,
                                grid,
                            },
                            matrix,
                            alpha,
                        });
                        return;
                    }
                }
                self.collect(timeline, child_frame, matrix, alpha, out);
            }
        }
    }
}

/// The elements active on a layer at `frame`, with motion-tween interpolation
/// applied between the active keyframe and the next one. Returns each element's
/// own matrix and opacity.
fn sampled_elements(frames: &[Frame], frame: usize) -> Vec<(Element, Matrix, f32)> {
    let Some(i) = frames
        .iter()
        .rposition(|f| f.index <= frame && frame < f.index + f.duration.max(1))
    else {
        return Vec::new();
    };
    let current = &frames[i];
    let local = frame - current.index;

    let target = frames
        .get(i + 1)
        .filter(|n| current.tween && n.index > current.index);
    let Some(target) = target else {
        return current
            .elements
            .iter()
            .map(|e| (e.clone(), e.matrix(), e.alpha()))
            .collect();
    };

    let span = (target.index - current.index).max(1);
    let t = (local as f32 / span as f32).clamp(0.0, 1.0);
    current
        .elements
        .iter()
        .map(|start| {
            let end = target.elements.iter().find(|end| same_slot(start, end));
            match end {
                Some(end) => (
                    start.clone(),
                    lerp_matrix(&start.matrix(), &end.matrix(), t),
                    start.alpha() + (end.alpha() - start.alpha()) * t,
                ),
                None => (start.clone(), start.matrix(), start.alpha()),
            }
        })
        .collect()
}

fn same_slot(a: &Element, b: &Element) -> bool {
    match (a, b) {
        (Element::Bitmap { name: x, .. }, Element::Bitmap { name: y, .. })
        | (Element::Instance { name: x, .. }, Element::Instance { name: y, .. }) => x == y,
        (Element::Shape { .. }, Element::Shape { .. })
        | (Element::Group { .. }, Element::Group { .. }) => true,
        (Element::Text(a), Element::Text(b)) => a.text == b.text,
        _ => false,
    }
}

/// Interpolate two matrices by decomposing into translate/rotate/scale/shear so
/// rotations interpolate by angle instead of collapsing through the origin.
pub fn lerp_matrix(a: &Matrix, b: &Matrix, t: f32) -> Matrix {
    let (sax, say, ra, shxa, txa, tya) = decompose(a);
    let (sbx, sby, rb, shxb, txb, tyb) = decompose(b);
    let angle_delta = wrap_angle(rb - ra);
    let sx = sax + (sbx - sax) * t;
    let sy = say + (sby - say) * t;
    let r = ra + angle_delta * t;
    let shx = shxa + (shxb - shxa) * t;
    let tx = txa + (txb - txa) * t;
    let ty = tya + (tyb - tya) * t;

    let (sin, cos) = r.sin_cos();
    [
        sx * cos,
        sx * sin,
        shx * sy * cos - sy * sin,
        shx * sy * sin + sy * cos,
        tx,
        ty,
    ]
}

fn wrap_angle(a: f32) -> f32 {
    let mut a = a;
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a <= -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

fn decompose(m: &Matrix) -> (f32, f32, f32, f32, f32, f32) {
    let [a, b, c, d, tx, ty] = *m;
    let sx = (a * a + b * b).sqrt();
    let r = b.atan2(a);
    let (sin, cos) = r.sin_cos();
    let c2 = c * cos + d * sin;
    let d2 = -c * sin + d * cos;
    let sy = d2;
    let shear = if sy.abs() > 1e-6 { c2 / sy } else { 0.0 };
    (sx, sy, r, shear, tx, ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_and_recomposes_identity() {
        let m = lerp_matrix(&IDENTITY, &IDENTITY, 0.5);
        assert!((m[0] - 1.0).abs() < 1e-5);
        assert!((m[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rotation_interpolates_by_angle() {
        let a = IDENTITY;
        let mut b = IDENTITY;
        let (s, c) = 1.0_f32.sin_cos();
        b[0] = c;
        b[1] = s;
        b[2] = -s;
        b[3] = c;
        let mid = lerp_matrix(&a, &b, 0.5);
        // Midway rotation should stay unit-scale (no collapse toward 0).
        let scale = (mid[0] * mid[0] + mid[1] * mid[1]).sqrt();
        assert!((scale - 1.0).abs() < 1e-4, "scale {scale}");
    }
}
