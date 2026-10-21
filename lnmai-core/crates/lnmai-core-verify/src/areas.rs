//! Sensor areas, button zones and outer slots.
//!
//! Mirrors `LnmaiCore/Areas.lean`. Constructor order matches Lean so the
//! bridge to `LnmaiCore.SensorArea` etc. is a direct constructor mapping.
//!
//! The Lean module defines index/rotation functions only for `ButtonZone` and
//! `OuterSlot`; `SensorArea` has no `toIndex` in Lean, so none is provided here.

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SensorArea {
    #[default]
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    A8,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    B7,
    B8,
    C,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ButtonZone {
    #[default]
    K1,
    K2,
    K3,
    K4,
    K5,
    K6,
    K7,
    K8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum OuterSlot {
    #[default]
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
}

impl SensorArea {
    /// Constructor order from `SensorArea.all` in Lean.
    pub fn all() -> [Self; 33] {
        [
            Self::A1,
            Self::A2,
            Self::A3,
            Self::A4,
            Self::A5,
            Self::A6,
            Self::A7,
            Self::A8,
            Self::B1,
            Self::B2,
            Self::B3,
            Self::B4,
            Self::B5,
            Self::B6,
            Self::B7,
            Self::B8,
            Self::C,
            Self::D1,
            Self::D2,
            Self::D3,
            Self::D4,
            Self::D5,
            Self::D6,
            Self::D7,
            Self::D8,
            Self::E1,
            Self::E2,
            Self::E3,
            Self::E4,
            Self::E5,
            Self::E6,
            Self::E7,
            Self::E8,
        ]
    }
}

impl core::fmt::Display for SensorArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

impl ButtonZone {
    /// Constructor order from `ButtonZone.all` in Lean.
    pub fn all() -> [Self; 8] {
        [
            Self::K1,
            Self::K2,
            Self::K3,
            Self::K4,
            Self::K5,
            Self::K6,
            Self::K7,
            Self::K8,
        ]
    }

    pub fn storage_order() -> [Self; 8] {
        Self::all()
    }
}

impl core::fmt::Display for ButtonZone {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

impl OuterSlot {
    /// Constructor order from `OuterSlot.all` in Lean.
    pub fn all() -> [Self; 8] {
        [
            Self::S1,
            Self::S2,
            Self::S3,
            Self::S4,
            Self::S5,
            Self::S6,
            Self::S7,
            Self::S8,
        ]
    }

    pub fn storage_order() -> [Self; 8] {
        Self::all()
    }
}

impl core::fmt::Display for OuterSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

/// `((n - 1) + steps) % 8 + 1` with `steps` reduced first so the addition
/// cannot overflow. Matches Lean's `rotateRingIndex` for all step counts.
fn ring_index(steps: usize, n: usize) -> usize {
    ((n - 1 + (steps % 8)) % 8) + 1
}

fn ring_sensor_a(k: usize) -> SensorArea {
    match k {
        1 => SensorArea::A1,
        2 => SensorArea::A2,
        3 => SensorArea::A3,
        4 => SensorArea::A4,
        5 => SensorArea::A5,
        6 => SensorArea::A6,
        7 => SensorArea::A7,
        _ => SensorArea::A8,
    }
}

fn ring_sensor_b(k: usize) -> SensorArea {
    match k {
        1 => SensorArea::B1,
        2 => SensorArea::B2,
        3 => SensorArea::B3,
        4 => SensorArea::B4,
        5 => SensorArea::B5,
        6 => SensorArea::B6,
        7 => SensorArea::B7,
        _ => SensorArea::B8,
    }
}

fn ring_sensor_d(k: usize) -> SensorArea {
    match k {
        1 => SensorArea::D1,
        2 => SensorArea::D2,
        3 => SensorArea::D3,
        4 => SensorArea::D4,
        5 => SensorArea::D5,
        6 => SensorArea::D6,
        7 => SensorArea::D7,
        _ => SensorArea::D8,
    }
}

fn ring_sensor_e(k: usize) -> SensorArea {
    match k {
        1 => SensorArea::E1,
        2 => SensorArea::E2,
        3 => SensorArea::E3,
        4 => SensorArea::E4,
        5 => SensorArea::E5,
        6 => SensorArea::E6,
        7 => SensorArea::E7,
        _ => SensorArea::E8,
    }
}

fn ring_button(k: usize) -> ButtonZone {
    match k {
        1 => ButtonZone::K1,
        2 => ButtonZone::K2,
        3 => ButtonZone::K3,
        4 => ButtonZone::K4,
        5 => ButtonZone::K5,
        6 => ButtonZone::K6,
        7 => ButtonZone::K7,
        _ => ButtonZone::K8,
    }
}

fn ring_outer(k: usize) -> OuterSlot {
    match k {
        1 => OuterSlot::S1,
        2 => OuterSlot::S2,
        3 => OuterSlot::S3,
        4 => OuterSlot::S4,
        5 => OuterSlot::S5,
        6 => OuterSlot::S6,
        7 => OuterSlot::S7,
        _ => OuterSlot::S8,
    }
}

impl SensorArea {
    /// `SensorArea.rotate` in Lean. Sensor C is fixed; the A/B/D/E rings
    /// rotate independently.
    pub fn rotate(self, steps: usize) -> SensorArea {
        match self {
            SensorArea::C => SensorArea::C,
            SensorArea::A1 => ring_sensor_a(ring_index(steps, 1)),
            SensorArea::A2 => ring_sensor_a(ring_index(steps, 2)),
            SensorArea::A3 => ring_sensor_a(ring_index(steps, 3)),
            SensorArea::A4 => ring_sensor_a(ring_index(steps, 4)),
            SensorArea::A5 => ring_sensor_a(ring_index(steps, 5)),
            SensorArea::A6 => ring_sensor_a(ring_index(steps, 6)),
            SensorArea::A7 => ring_sensor_a(ring_index(steps, 7)),
            SensorArea::A8 => ring_sensor_a(ring_index(steps, 8)),
            SensorArea::B1 => ring_sensor_b(ring_index(steps, 1)),
            SensorArea::B2 => ring_sensor_b(ring_index(steps, 2)),
            SensorArea::B3 => ring_sensor_b(ring_index(steps, 3)),
            SensorArea::B4 => ring_sensor_b(ring_index(steps, 4)),
            SensorArea::B5 => ring_sensor_b(ring_index(steps, 5)),
            SensorArea::B6 => ring_sensor_b(ring_index(steps, 6)),
            SensorArea::B7 => ring_sensor_b(ring_index(steps, 7)),
            SensorArea::B8 => ring_sensor_b(ring_index(steps, 8)),
            SensorArea::D1 => ring_sensor_d(ring_index(steps, 1)),
            SensorArea::D2 => ring_sensor_d(ring_index(steps, 2)),
            SensorArea::D3 => ring_sensor_d(ring_index(steps, 3)),
            SensorArea::D4 => ring_sensor_d(ring_index(steps, 4)),
            SensorArea::D5 => ring_sensor_d(ring_index(steps, 5)),
            SensorArea::D6 => ring_sensor_d(ring_index(steps, 6)),
            SensorArea::D7 => ring_sensor_d(ring_index(steps, 7)),
            SensorArea::D8 => ring_sensor_d(ring_index(steps, 8)),
            SensorArea::E1 => ring_sensor_e(ring_index(steps, 1)),
            SensorArea::E2 => ring_sensor_e(ring_index(steps, 2)),
            SensorArea::E3 => ring_sensor_e(ring_index(steps, 3)),
            SensorArea::E4 => ring_sensor_e(ring_index(steps, 4)),
            SensorArea::E5 => ring_sensor_e(ring_index(steps, 5)),
            SensorArea::E6 => ring_sensor_e(ring_index(steps, 6)),
            SensorArea::E7 => ring_sensor_e(ring_index(steps, 7)),
            SensorArea::E8 => ring_sensor_e(ring_index(steps, 8)),
        }
    }

    /// `SensorArea.toOuterSlot?`.
    pub fn to_outer_slot(self) -> Option<OuterSlot> {
        match self {
            SensorArea::A1 => Some(OuterSlot::S1),
            SensorArea::A2 => Some(OuterSlot::S2),
            SensorArea::A3 => Some(OuterSlot::S3),
            SensorArea::A4 => Some(OuterSlot::S4),
            SensorArea::A5 => Some(OuterSlot::S5),
            SensorArea::A6 => Some(OuterSlot::S6),
            SensorArea::A7 => Some(OuterSlot::S7),
            SensorArea::A8 => Some(OuterSlot::S8),
            _ => None,
        }
    }

    /// `SensorArea.toOuterButtonZone?`.
    // Explicit matching keeps the extracted function first-order.
    #[allow(clippy::manual_map)]
    pub fn to_outer_button_zone(self) -> Option<ButtonZone> {
        match self.to_outer_slot() {
            Some(slot) => Some(slot.to_button_zone()),
            None => None,
        }
    }
}

impl ButtonZone {
    /// `ButtonZone.toIndex`.
    pub fn to_index(self) -> usize {
        match self {
            ButtonZone::K1 => 0,
            ButtonZone::K2 => 1,
            ButtonZone::K3 => 2,
            ButtonZone::K4 => 3,
            ButtonZone::K5 => 4,
            ButtonZone::K6 => 5,
            ButtonZone::K7 => 6,
            ButtonZone::K8 => 7,
        }
    }

    /// `ButtonZone.ofIndex?`.
    pub fn of_index(index: usize) -> Option<ButtonZone> {
        match index {
            0 => Some(ButtonZone::K1),
            1 => Some(ButtonZone::K2),
            2 => Some(ButtonZone::K3),
            3 => Some(ButtonZone::K4),
            4 => Some(ButtonZone::K5),
            5 => Some(ButtonZone::K6),
            6 => Some(ButtonZone::K7),
            7 => Some(ButtonZone::K8),
            _ => None,
        }
    }

    /// `ButtonZone.rotate`.
    pub fn rotate(self, steps: usize) -> ButtonZone {
        match self {
            ButtonZone::K1 => ring_button(ring_index(steps, 1)),
            ButtonZone::K2 => ring_button(ring_index(steps, 2)),
            ButtonZone::K3 => ring_button(ring_index(steps, 3)),
            ButtonZone::K4 => ring_button(ring_index(steps, 4)),
            ButtonZone::K5 => ring_button(ring_index(steps, 5)),
            ButtonZone::K6 => ring_button(ring_index(steps, 6)),
            ButtonZone::K7 => ring_button(ring_index(steps, 7)),
            ButtonZone::K8 => ring_button(ring_index(steps, 8)),
        }
    }

    /// `ButtonZone.toOuterSlot`.
    pub fn to_outer_slot(self) -> OuterSlot {
        match self {
            ButtonZone::K1 => OuterSlot::S1,
            ButtonZone::K2 => OuterSlot::S2,
            ButtonZone::K3 => OuterSlot::S3,
            ButtonZone::K4 => OuterSlot::S4,
            ButtonZone::K5 => OuterSlot::S5,
            ButtonZone::K6 => OuterSlot::S6,
            ButtonZone::K7 => OuterSlot::S7,
            ButtonZone::K8 => OuterSlot::S8,
        }
    }

    /// `ButtonZone.toOuterSensorArea`.
    pub fn to_outer_sensor_area(self) -> SensorArea {
        self.to_outer_slot().to_outer_sensor_area()
    }
}

impl OuterSlot {
    /// `OuterSlot.toIndex`.
    pub fn to_index(self) -> usize {
        match self {
            OuterSlot::S1 => 0,
            OuterSlot::S2 => 1,
            OuterSlot::S3 => 2,
            OuterSlot::S4 => 3,
            OuterSlot::S5 => 4,
            OuterSlot::S6 => 5,
            OuterSlot::S7 => 6,
            OuterSlot::S8 => 7,
        }
    }

    /// `OuterSlot.ofIndex?`.
    pub fn of_index(index: usize) -> Option<OuterSlot> {
        match index {
            0 => Some(OuterSlot::S1),
            1 => Some(OuterSlot::S2),
            2 => Some(OuterSlot::S3),
            3 => Some(OuterSlot::S4),
            4 => Some(OuterSlot::S5),
            5 => Some(OuterSlot::S6),
            6 => Some(OuterSlot::S7),
            7 => Some(OuterSlot::S8),
            _ => None,
        }
    }

    /// `OuterSlot.rotate`.
    pub fn rotate(self, steps: usize) -> OuterSlot {
        match self {
            OuterSlot::S1 => ring_outer(ring_index(steps, 1)),
            OuterSlot::S2 => ring_outer(ring_index(steps, 2)),
            OuterSlot::S3 => ring_outer(ring_index(steps, 3)),
            OuterSlot::S4 => ring_outer(ring_index(steps, 4)),
            OuterSlot::S5 => ring_outer(ring_index(steps, 5)),
            OuterSlot::S6 => ring_outer(ring_index(steps, 6)),
            OuterSlot::S7 => ring_outer(ring_index(steps, 7)),
            OuterSlot::S8 => ring_outer(ring_index(steps, 8)),
        }
    }

    /// `OuterSlot.toButtonZone`.
    pub fn to_button_zone(self) -> ButtonZone {
        match self {
            OuterSlot::S1 => ButtonZone::K1,
            OuterSlot::S2 => ButtonZone::K2,
            OuterSlot::S3 => ButtonZone::K3,
            OuterSlot::S4 => ButtonZone::K4,
            OuterSlot::S5 => ButtonZone::K5,
            OuterSlot::S6 => ButtonZone::K6,
            OuterSlot::S7 => ButtonZone::K7,
            OuterSlot::S8 => ButtonZone::K8,
        }
    }

    /// `OuterSlot.toOuterSensorArea`.
    pub fn to_outer_sensor_area(self) -> SensorArea {
        match self {
            OuterSlot::S1 => SensorArea::A1,
            OuterSlot::S2 => SensorArea::A2,
            OuterSlot::S3 => SensorArea::A3,
            OuterSlot::S4 => SensorArea::A4,
            OuterSlot::S5 => SensorArea::A5,
            OuterSlot::S6 => SensorArea::A6,
            OuterSlot::S7 => SensorArea::A7,
            OuterSlot::S8 => SensorArea::A8,
        }
    }
}
