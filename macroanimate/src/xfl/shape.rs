//! Adobe Animate edge-string parsing.
//!
//! `DOMShape` stores its outline as compact edge strings. Each `!` introduces
//! one edge: `!x0 y0 <op><flag>|x1 y1[|x2 y2…]` where `<op>` is `S` (straight),
//! `Q` (quadratic) or `B` (cubic). The optional digit right after the opcode is
//! a style hint and is ignored. This flattens the edges into a polygon in the
//! shape's local coordinates — enough for solid fills and straight strokes.

/// Parse one `<Edge>` element's `edges="…"` value into a polygon.
pub fn parse_edge_path(text: &str) -> Vec<[f32; 2]> {
    let mut points: Vec<[f32; 2]> = Vec::new();

    for chunk in text.split('!').filter(|c| !c.trim().is_empty()) {
        let Some((start, segment)) = parse_chunk(chunk) else {
            continue;
        };
        if points.last() != Some(&start) {
            points.push(start);
        }
        points.extend(segment);
    }

    if points.len() >= 3 && points.first() != points.last() {
        let first = points[0];
        points.push(first);
    }
    points
}

/// Returns `(start_point, flattened_segment_points)` for one `!` chunk.
fn parse_chunk(chunk: &str) -> Option<([f32; 2], Vec<[f32; 2]>)> {
    let Some(letter) = chunk.find(|c: char| c.is_ascii_alphabetic()) else {
        // No opcode: `x0 y0|x1 y1[…]`, an implicit straight line segment.
        let pairs: Vec<[f32; 2]> = chunk.split('|').filter_map(parse_pair).collect();
        let start = *pairs.first()?;
        let segment = pairs.get(1).copied().map(|end| vec![end]).unwrap_or_default();
        return Some((start, segment));
    };

    let start_nums = numbers(&chunk[..letter]);
    let start = [*start_nums.first()?, *start_nums.get(1)?];

    let op = chunk[letter..].chars().next()?;
    let mut idx = letter + op.len_utf8();
    // Skip the style flag digit(s) that follow the opcode.
    while idx < chunk.len() && chunk.as_bytes()[idx].is_ascii_digit() {
        idx += 1;
    }
    let pairs: Vec<[f32; 2]> = chunk[idx..].split('|').filter_map(parse_pair).collect();

    let segment = match op {
        'Q' if pairs.len() >= 2 => flatten_quadratic(start, pairs[0], pairs[1]),
        'B' if pairs.len() >= 3 => flatten_cubic(start, pairs[0], pairs[1], pairs[2]),
        _ => pairs.last().copied().map(|end| vec![end]).unwrap_or_default(),
    };
    Some((start, segment))
}

fn parse_pair(s: &str) -> Option<[f32; 2]> {
    let n = numbers(s);
    (n.len() >= 2).then(|| [n[0], n[1]])
}

fn flatten_quadratic(p0: [f32; 2], c: [f32; 2], p1: [f32; 2]) -> Vec<[f32; 2]> {
    const STEPS: usize = 8;
    (1..=STEPS)
        .map(|i| {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            [
                u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p1[0],
                u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p1[1],
            ]
        })
        .collect()
}

fn flatten_cubic(p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p1: [f32; 2]) -> Vec<[f32; 2]> {
    const STEPS: usize = 12;
    (1..=STEPS)
        .map(|i| {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            [
                u * u * u * p0[0] + 3.0 * u * u * t * c1[0] + 3.0 * u * t * t * c2[0] + t * t * t * p1[0],
                u * u * u * p0[1] + 3.0 * u * u * t * c1[1] + 3.0 * u * t * t * c2[1] + t * t * t * p1[1],
            ]
        })
        .collect()
}

/// Extract every float in `s` (handles leading `-` and decimals).
fn numbers(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f32>() {
                out.push(v);
            }
            cur.clear();
        }
    }
    if let Ok(v) = cur.parse::<f32>() {
        out.push(v);
    }
    out
}
#[cfg(test)]
mod tests {
    use super::parse_edge_path;

    #[test]
    fn parses_a_rectangle() {
        let text = "!578 -201S2|2308 -201!2308 -201|2308 1529!2308 1529|578 1529!578 1529|578 -201";
        let pts = parse_edge_path(text);
        assert_eq!(
            pts,
            vec![
                [578.0, -201.0],
                [2308.0, -201.0],
                [2308.0, 1529.0],
                [578.0, 1529.0],
                [578.0, -201.0],
            ]
        );
    }

    #[test]
    fn parses_reversed_rectangle_with_flag() {
        let text = "!-2307 -1506S6|-577 -1506!-577 -1506|-577 224!-577 224|-2307 224!-2307 224|-2307 -1506";
        let pts = parse_edge_path(text);
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0], [-2307.0, -1506.0]);
        assert_eq!(pts.last().copied(), Some([-2307.0, -1506.0]));
    }
}
