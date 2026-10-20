//! Generated SDEZ170 SlideManager HitArea tables.
//!
//! The JSON is produced by tools/extract_slide_hit_areas.py from the official
//! SlideManager source. Shape indices match Manager.SlideType.

use std::sync::OnceLock;

pub const SHAPE_COUNT: usize = 14;
pub const ROTATION_COUNT: usize = 8;
pub const LENGTH_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct SlideHitArea {
    pub points: Vec<usize>,
    pub push_distance: f64,
    pub release_distance: f64,
}

type Table = Vec<Vec<Vec<Vec<SlideHitArea>>>>;

fn parse_table() -> Table {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("slide_hit_areas.json")).expect("slide table json");
    let shapes = value["shapes"].as_array().expect("shapes array");
    let mut table = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let rotations = shape.as_array().expect("rotations array");
        let mut rotation_table = Vec::with_capacity(rotations.len());
        for rotation in rotations {
            let lengths = rotation.as_array().expect("lengths array");
            let mut length_table = Vec::with_capacity(lengths.len());
            for length in lengths {
                let areas = length.as_array().expect("areas array");
                let mut parsed = Vec::with_capacity(areas.len());
                for area in areas {
                    let points = area["points"]
                        .as_array()
                        .expect("points array")
                        .iter()
                        .map(|point| point.as_u64().expect("point index") as usize)
                        .collect();
                    parsed.push(SlideHitArea {
                        points,
                        push_distance: area["push"].as_f64().expect("push distance"),
                        release_distance: area["release"].as_f64().expect("release distance"),
                    });
                }
                length_table.push(parsed);
            }
            rotation_table.push(length_table);
        }
        table.push(rotation_table);
    }
    table
}

pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(parse_table)
}

/// Official GetSlideTypeLength.
pub fn slide_type_length(slide_type: usize, start: usize, end: usize) -> usize {
    let mut length = if end < start {
        8 + end - start
    } else {
        end - start
    };
    if matches!(slide_type, 3 | 5 | 7 | 10 | 12) {
        length = (8 - length) % 8;
    }
    length
}

/// Official GetSlideHitArea lookup.
pub fn hit_areas(
    slide_type: usize,
    start: usize,
    end: usize,
    star_button: usize,
) -> &'static [SlideHitArea] {
    let length = slide_type_length(slide_type, start, end);
    let rotation = (start + 8 - star_button) % 8;
    &table()[slide_type][rotation][length]
}

pub fn total_distance(areas: &[SlideHitArea]) -> f64 {
    areas
        .iter()
        .map(|area| area.push_distance + area.release_distance)
        .sum()
}

/// SlideRoot.Initialize lastWaitTime initialization.
pub fn initial_last_wait_time(
    areas: &[SlideHitArea],
    star_launch_msec: f32,
    star_arrive_msec: f32,
) -> f32 {
    let Some(last) = areas.last() else {
        return 0.0;
    };
    let total = total_distance(areas);
    if total == 0.0 {
        return 0.0;
    }
    (star_arrive_msec - star_launch_msec) * (last.push_distance / total) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_all_shapes_and_rotations() {
        let table = table();
        assert_eq!(table.len(), SHAPE_COUNT);
        for shape in table {
            assert_eq!(shape.len(), ROTATION_COUNT);
            for rotation in shape {
                assert_eq!(rotation.len(), LENGTH_COUNT);
            }
        }
    }

    #[test]
    fn straight_line_rotates_with_the_star_button() {
        let first = hit_areas(1, 0, 2, 0);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].points, vec![0]);
        assert!((first[0].push_distance - 130.1907196044922).abs() < 1e-9);

        let rotated = hit_areas(1, 0, 2, 1);
        assert_eq!(rotated[0].points, vec![7]);
    }

    #[test]
    fn circle_right_uses_the_reverse_length() {
        assert_eq!(slide_type_length(2, 0, 3), 3);
        assert_eq!(slide_type_length(3, 0, 3), 5);
    }

    #[test]
    fn wait_time_uses_the_last_area_share() {
        let areas = hit_areas(1, 0, 2, 0);
        let wait = initial_last_wait_time(areas, 0.0, 1000.0);
        let total = total_distance(areas);
        let expected = 1000.0 * (areas.last().unwrap().push_distance / total) as f32;
        assert!((wait - expected).abs() < 1e-4);
    }
}
