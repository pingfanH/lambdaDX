//! Shared, extracted slide symmetry implementation.
pub use lnmai_core_verify::symmetry::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::SensorArea;

    #[test]
    fn direct_is_identity() {
        for area in crate::storage::sensor_area_storage_order() {
            assert_eq!(act_on_sensor_area(direct(), area), area);
        }
    }

    #[test]
    fn mirror_swaps_2_and_8() {
        assert_eq!(act_on_sensor_area(mirror(), SensorArea::A2), SensorArea::A8);
        assert_eq!(act_on_sensor_area(mirror(), SensorArea::A8), SensorArea::A2);
    }

    #[test]
    fn rotation_advances_ring() {
        let g = SlideSymmetry::R(1);
        assert_eq!(act_on_sensor_area(g, SensorArea::A1), SensorArea::A2);
        assert_eq!(act_on_sensor_area(g, SensorArea::A8), SensorArea::A1);
    }

    #[test]
    fn rotations_are_residue_classes() {
        for k in [0, 1, 7, 8, 9, u32::MAX] {
            for mirrored in [false, true] {
                let make = if mirrored {
                    SlideSymmetry::Sr
                } else {
                    SlideSymmetry::R
                };
                assert_eq!(make(k), make(k % 8));
                assert_eq!(rotation_steps(make(k)), k % 8);
                for area in crate::storage::sensor_area_storage_order() {
                    assert_eq!(
                        act_on_sensor_area(make(k), area),
                        act_on_sensor_area(make(k % 8), area)
                    );
                }
            }
        }
    }
}
