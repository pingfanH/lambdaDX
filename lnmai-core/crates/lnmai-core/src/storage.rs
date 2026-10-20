//! Storage shared with the extracted verification kernel.
//!
//! Generic equivalence requires value-preserving Clone and pure, total callbacks,
//! as stated by the Lean storage theorems.
pub use lnmai_core_verify::storage::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{ButtonZone, SensorArea};
    use crate::constants::SENSOR_AREA_COUNT;

    #[test]
    fn storage_index_roundtrip() {
        for i in 0..SENSOR_AREA_COUNT as usize {
            let area = sensor_area_of_storage_index(i).unwrap();
            assert_eq!(sensor_area_to_storage_index(area), i);
        }
        assert_eq!(sensor_area_of_storage_index(33), None);
    }

    #[test]
    fn sensor_storage_order_matches_lean() {
        let order = sensor_area_storage_order();
        assert_eq!(order.len(), 33);
        assert_eq!(order[0], SensorArea::A1);
        assert_eq!(order[8], SensorArea::D1);
        assert_eq!(order[16], SensorArea::C);
        assert_eq!(order[17], SensorArea::E1);
        assert_eq!(order[25], SensorArea::B1);
    }

    #[test]
    fn button_set_get() {
        let v = ButtonVec::replicate(8, 0u32);
        let v = v.set(ButtonZone::K3, 7);
        assert_eq!(v.get_d(ButtonZone::K3, 0), 7);
        assert_eq!(v.get_d(ButtonZone::K1, 0), 0);
    }
}
