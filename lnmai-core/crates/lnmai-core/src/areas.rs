//! Shared finite area types and total adapters for Lean's unbounded Nat inputs.
pub use lnmai_core_verify::areas::{ButtonZone, OuterSlot, SensorArea};

use crate::natural::Natural;

/// `SensorArea.rotate`; reduce before entering the extracted machine kernel.
pub fn rotate_sensor_area(steps: impl Into<Natural>, area: SensorArea) -> SensorArea {
    area.rotate(rotation_steps(steps))
}

/// `ButtonZone.rotate`, for every Lean natural rotation count.
pub fn rotate_button_zone(steps: impl Into<Natural>, zone: ButtonZone) -> ButtonZone {
    zone.rotate(rotation_steps(steps))
}

/// `OuterSlot.rotate`, for every Lean natural rotation count.
pub fn rotate_outer_slot(steps: impl Into<Natural>, slot: OuterSlot) -> OuterSlot {
    slot.rotate(rotation_steps(steps))
}

fn rotation_steps(steps: impl Into<Natural>) -> usize {
    (steps.into() % 8).to_usize().expect("modulo eight")
}

/// `ButtonZone.ofIndex?`; out-of-range naturals return `None`.
pub fn button_zone_of_index(index: impl Into<Natural>) -> Option<ButtonZone> {
    index.into().to_usize().and_then(ButtonZone::of_index)
}

/// `OuterSlot.ofIndex?`; out-of-range naturals return `None`.
pub fn outer_slot_of_index(index: impl Into<Natural>) -> Option<OuterSlot> {
    index.into().to_usize().and_then(OuterSlot::of_index)
}

pub fn sensor_area_code(value: SensorArea) -> &'static str {
    match value {
        SensorArea::A1 => "A1",
        SensorArea::A2 => "A2",
        SensorArea::A3 => "A3",
        SensorArea::A4 => "A4",
        SensorArea::A5 => "A5",
        SensorArea::A6 => "A6",
        SensorArea::A7 => "A7",
        SensorArea::A8 => "A8",
        SensorArea::B1 => "B1",
        SensorArea::B2 => "B2",
        SensorArea::B3 => "B3",
        SensorArea::B4 => "B4",
        SensorArea::B5 => "B5",
        SensorArea::B6 => "B6",
        SensorArea::B7 => "B7",
        SensorArea::B8 => "B8",
        SensorArea::C => "C",
        SensorArea::D1 => "D1",
        SensorArea::D2 => "D2",
        SensorArea::D3 => "D3",
        SensorArea::D4 => "D4",
        SensorArea::D5 => "D5",
        SensorArea::D6 => "D6",
        SensorArea::D7 => "D7",
        SensorArea::D8 => "D8",
        SensorArea::E1 => "E1",
        SensorArea::E2 => "E2",
        SensorArea::E3 => "E3",
        SensorArea::E4 => "E4",
        SensorArea::E5 => "E5",
        SensorArea::E6 => "E6",
        SensorArea::E7 => "E7",
        SensorArea::E8 => "E8",
    }
}

pub fn sensor_area_label(value: SensorArea) -> &'static str {
    match value {
        SensorArea::A1 => "Sensor A1",
        SensorArea::A2 => "Sensor A2",
        SensorArea::A3 => "Sensor A3",
        SensorArea::A4 => "Sensor A4",
        SensorArea::A5 => "Sensor A5",
        SensorArea::A6 => "Sensor A6",
        SensorArea::A7 => "Sensor A7",
        SensorArea::A8 => "Sensor A8",
        SensorArea::B1 => "Sensor B1",
        SensorArea::B2 => "Sensor B2",
        SensorArea::B3 => "Sensor B3",
        SensorArea::B4 => "Sensor B4",
        SensorArea::B5 => "Sensor B5",
        SensorArea::B6 => "Sensor B6",
        SensorArea::B7 => "Sensor B7",
        SensorArea::B8 => "Sensor B8",
        SensorArea::C => "Sensor C",
        SensorArea::D1 => "Sensor D1",
        SensorArea::D2 => "Sensor D2",
        SensorArea::D3 => "Sensor D3",
        SensorArea::D4 => "Sensor D4",
        SensorArea::D5 => "Sensor D5",
        SensorArea::D6 => "Sensor D6",
        SensorArea::D7 => "Sensor D7",
        SensorArea::D8 => "Sensor D8",
        SensorArea::E1 => "Sensor E1",
        SensorArea::E2 => "Sensor E2",
        SensorArea::E3 => "Sensor E3",
        SensorArea::E4 => "Sensor E4",
        SensorArea::E5 => "Sensor E5",
        SensorArea::E6 => "Sensor E6",
        SensorArea::E7 => "Sensor E7",
        SensorArea::E8 => "Sensor E8",
    }
}

pub fn button_zone_code(value: ButtonZone) -> &'static str {
    match value {
        ButtonZone::K1 => "K1",
        ButtonZone::K2 => "K2",
        ButtonZone::K3 => "K3",
        ButtonZone::K4 => "K4",
        ButtonZone::K5 => "K5",
        ButtonZone::K6 => "K6",
        ButtonZone::K7 => "K7",
        ButtonZone::K8 => "K8",
    }
}

pub fn outer_slot_code(value: OuterSlot) -> &'static str {
    match value {
        OuterSlot::S1 => "S1",
        OuterSlot::S2 => "S2",
        OuterSlot::S3 => "S3",
        OuterSlot::S4 => "S4",
        OuterSlot::S5 => "S5",
        OuterSlot::S6 => "S6",
        OuterSlot::S7 => "S7",
        OuterSlot::S8 => "S8",
    }
}
