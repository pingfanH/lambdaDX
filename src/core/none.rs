//! `backend-none` stub: just enough of the core's type/session API for the
//! player to compile without any gameplay core. No judging happens.

use serde::{Deserialize, Serialize};

pub mod types {
    use serde::{Deserialize, Serialize};

    /// Input timestamps are microseconds.
    pub type TimePoint = i64;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub enum ComboState {
        None,
        FC,
        FCPlus,
        AP,
        APPlus,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub enum ButtonZone {
        K1,
        K2,
        K3,
        K4,
        K5,
        K6,
        K7,
        K8,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub enum SensorArea {
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

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TimedInputEvent {
        #[serde(rename = "buttonClick")]
        ButtonClick { tp: TimePoint, zone: ButtonZone },
        #[serde(rename = "buttonHold")]
        ButtonHold {
            tp: TimePoint,
            zone: ButtonZone,
            #[serde(rename = "isDown")]
            is_down: bool,
        },
        #[serde(rename = "sensorClick")]
        SensorClick { tp: TimePoint, area: SensorArea },
        #[serde(rename = "sensorHold")]
        SensorHold {
            tp: TimePoint,
            area: SensorArea,
            #[serde(rename = "isDown")]
            is_down: bool,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct ScoreState {
        pub combo: u64,
        pub p_combo: u64,
        pub c_p_combo: u64,
        pub total_base: u64,
        pub total_extra: u64,
        pub earned_base: u64,
        pub earned_extra: u64,
        pub earned_classic_extra: u64,
        pub lost_base: u64,
        pub lost_extra: u64,
        pub lost_classic_extra: u64,
        pub dx_score: i64,
        pub max_dx_score: u64,
        pub fast_count: u64,
        pub late_count: u64,
    }

    impl ScoreState {
        pub fn dx_score_remaining(&self) -> i64 {
            self.max_dx_score as i64 + self.dx_score
        }
        pub fn combo_state(&self) -> ComboState {
            ComboState::None
        }
    }
}

pub mod session {
    /// Placeholder error type.
    #[derive(Debug)]
    pub struct LnmaiError {
        pub json: String,
    }

    /// The core is disabled; there is nothing to initialise.
    pub fn ensure_runtime() -> Result<(), LnmaiError> {
        Ok(())
    }
}

pub mod api {}
