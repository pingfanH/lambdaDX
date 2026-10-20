//! List-backed note storage.
//!
//! Mirrors `LnmaiCore/Storage.lean`. The Lean module is generic over the
//! stored element type; this port preserves that with `T: Clone`.

use crate::areas::{ButtonZone, SensorArea};
use crate::constants::SENSOR_AREA_COUNT;

/// `sensorAreaToStorageIndex`: sensor areas are stored in A/D/C/E/B order.
pub fn sensor_area_to_storage_index(area: SensorArea) -> usize {
    match area {
        SensorArea::A1 => 0,
        SensorArea::A2 => 1,
        SensorArea::A3 => 2,
        SensorArea::A4 => 3,
        SensorArea::A5 => 4,
        SensorArea::A6 => 5,
        SensorArea::A7 => 6,
        SensorArea::A8 => 7,
        SensorArea::D1 => 8,
        SensorArea::D2 => 9,
        SensorArea::D3 => 10,
        SensorArea::D4 => 11,
        SensorArea::D5 => 12,
        SensorArea::D6 => 13,
        SensorArea::D7 => 14,
        SensorArea::D8 => 15,
        SensorArea::C => 16,
        SensorArea::E1 => 17,
        SensorArea::E2 => 18,
        SensorArea::E3 => 19,
        SensorArea::E4 => 20,
        SensorArea::E5 => 21,
        SensorArea::E6 => 22,
        SensorArea::E7 => 23,
        SensorArea::E8 => 24,
        SensorArea::B1 => 25,
        SensorArea::B2 => 26,
        SensorArea::B3 => 27,
        SensorArea::B4 => 28,
        SensorArea::B5 => 29,
        SensorArea::B6 => 30,
        SensorArea::B7 => 31,
        SensorArea::B8 => 32,
    }
}

/// `sensorAreaOfStorageIndex?`.
pub fn sensor_area_of_storage_index(index: usize) -> Option<SensorArea> {
    match index {
        0 => Some(SensorArea::A1),
        1 => Some(SensorArea::A2),
        2 => Some(SensorArea::A3),
        3 => Some(SensorArea::A4),
        4 => Some(SensorArea::A5),
        5 => Some(SensorArea::A6),
        6 => Some(SensorArea::A7),
        7 => Some(SensorArea::A8),
        8 => Some(SensorArea::D1),
        9 => Some(SensorArea::D2),
        10 => Some(SensorArea::D3),
        11 => Some(SensorArea::D4),
        12 => Some(SensorArea::D5),
        13 => Some(SensorArea::D6),
        14 => Some(SensorArea::D7),
        15 => Some(SensorArea::D8),
        16 => Some(SensorArea::C),
        17 => Some(SensorArea::E1),
        18 => Some(SensorArea::E2),
        19 => Some(SensorArea::E3),
        20 => Some(SensorArea::E4),
        21 => Some(SensorArea::E5),
        22 => Some(SensorArea::E6),
        23 => Some(SensorArea::E7),
        24 => Some(SensorArea::E8),
        25 => Some(SensorArea::B1),
        26 => Some(SensorArea::B2),
        27 => Some(SensorArea::B3),
        28 => Some(SensorArea::B4),
        29 => Some(SensorArea::B5),
        30 => Some(SensorArea::B6),
        31 => Some(SensorArea::B7),
        32 => Some(SensorArea::B8),
        _ => None,
    }
}

/// `ButtonZone.storageOrder` (identity order).
pub fn button_zone_storage_order() -> Vec<ButtonZone> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < 8 {
        if let Some(zone) = ButtonZone::of_index(i) {
            out.push(zone);
        }
        i += 1;
    }
    out
}

pub fn sensor_area_storage_order() -> Vec<SensorArea> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < SENSOR_AREA_COUNT as usize {
        if let Some(area) = sensor_area_of_storage_index(i) {
            out.push(area);
        }
        i += 1;
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonVec<T> {
    pub data: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorVec<T> {
    pub data: Vec<T>,
}

impl<T> Default for ButtonVec<T> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl<T> Default for SensorVec<T> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

fn clone_values<T: Clone>(data: &[T]) -> Vec<T> {
    data.to_vec()
}

fn list_set_at<T: Clone>(data: &[T], index: usize, value: T) -> Vec<T> {
    let mut out = clone_values(data);
    if index < out.len() {
        out[index] = value;
    }
    out
}

impl<T: Clone> ButtonVec<T> {
    pub fn replicate(n: usize, value: T) -> ButtonVec<T> {
        ButtonVec {
            data: vec![value; n],
        }
    }
    pub fn get_d(&self, zone: ButtonZone, default: T) -> T {
        let index = zone.to_index();
        if index < self.data.len() {
            self.data[index].clone()
        } else {
            default
        }
    }
    pub fn set(&self, zone: ButtonZone, value: T) -> ButtonVec<T> {
        ButtonVec {
            data: list_set_at(&self.data, zone.to_index(), value),
        }
    }
    pub fn to_list(&self) -> Vec<T> {
        clone_values(&self.data)
    }
    pub fn entries(&self) -> Vec<(ButtonZone, T)> {
        let keys = button_zone_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() && i < self.data.len() {
            out.push((keys[i], self.data[i].clone()));
            i += 1;
        }
        out
    }
    pub fn of_fn(f: impl Fn(ButtonZone) -> T) -> ButtonVec<T> {
        let keys = button_zone_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() {
            out.push(f(keys[i]));
            i += 1;
        }
        ButtonVec { data: out }
    }
    pub fn map_accum<S, B, F>(&self, mut state: S, f: F) -> (ButtonVec<B>, S)
    where
        F: Fn(ButtonZone, T, S) -> (B, S),
    {
        let keys = button_zone_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() && i < self.data.len() {
            let (mapped, next) = f(keys[i], self.data[i].clone(), state);
            out.push(mapped);
            state = next;
            i += 1;
        }
        (ButtonVec { data: out }, state)
    }
}

impl<T: Clone> SensorVec<T> {
    pub fn replicate(n: usize, value: T) -> SensorVec<T> {
        SensorVec {
            data: vec![value; n],
        }
    }
    pub fn get_d(&self, zone: SensorArea, default: T) -> T {
        let index = sensor_area_to_storage_index(zone);
        if index < self.data.len() {
            self.data[index].clone()
        } else {
            default
        }
    }
    pub fn set(&self, zone: SensorArea, value: T) -> SensorVec<T> {
        SensorVec {
            data: list_set_at(&self.data, sensor_area_to_storage_index(zone), value),
        }
    }
    pub fn to_list(&self) -> Vec<T> {
        clone_values(&self.data)
    }
    pub fn entries(&self) -> Vec<(SensorArea, T)> {
        let keys = sensor_area_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() && i < self.data.len() {
            out.push((keys[i], self.data[i].clone()));
            i += 1;
        }
        out
    }
    pub fn of_fn(f: impl Fn(SensorArea) -> T) -> SensorVec<T> {
        let keys = sensor_area_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() {
            out.push(f(keys[i]));
            i += 1;
        }
        SensorVec { data: out }
    }
    pub fn map_accum<S, B, F>(&self, mut state: S, f: F) -> (SensorVec<B>, S)
    where
        F: Fn(SensorArea, T, S) -> (B, S),
    {
        let keys = sensor_area_storage_order();
        let mut out = Vec::new();
        let mut i = 0;
        while i < keys.len() && i < self.data.len() {
            let (mapped, next) = f(keys[i], self.data[i].clone(), state);
            out.push(mapped);
            state = next;
            i += 1;
        }
        (SensorVec { data: out }, state)
    }
}
