use std::marker::PhantomData;

pub struct Empty;
pub struct Loaded;

#[derive(Debug, Clone)]
pub struct FfiEnvelope {
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct LnmaiError {
    pub json: String,
}

pub type Result<T> = std::result::Result<T, LnmaiError>;

pub struct Session<State> {
    handle: u64,
    _state: PhantomData<State>,
}

impl<State> Session<State> {
    pub fn handle(&self) -> u64 {
        self.handle
    }
}

pub unsafe fn initialize_runtime() -> std::result::Result<(), ()> {
    Ok(())
}

impl Session<Empty> {
    pub fn create() -> Result<Self> {
        let handle = lnmai_core_rs::ffi::create_empty_session();
        Ok(Session {
            handle,
            _state: PhantomData,
        })
    }

    pub fn load_chart_text(self, content: &str, level_index: u32) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let chart_spec_json = simai_text_to_chart_spec_json(content, level_index)?;
        self.load_chart_json(&chart_spec_json)
    }

    pub fn load_chart_json(self, chart_spec_json: &str) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle;
        lnmai_core_rs::ffi::load_chart_into_session(handle, chart_spec_json)
            .map_err(|e| LnmaiError { json: e })?;
        Ok((
            Session {
                handle,
                _state: PhantomData,
            },
            FfiEnvelope {
                json: "{\"ok\":true}".to_string(),
            },
        ))
    }

    pub fn free(self) -> Result<FfiEnvelope> {
        lnmai_core_rs::ffi::free_session(self.handle);
        Ok(FfiEnvelope {
            json: "{\"ok\":true}".to_string(),
        })
    }
}

impl Session<Loaded> {
    pub fn get_lowered_chart_json(&self) -> Result<FfiEnvelope> {
        let spec = lnmai_core_rs::ffi::get_chart_spec(self.handle)
            .map_err(|e| LnmaiError { json: e })?;
        let json = serde_json::json!({
            "result": {
                "taps": spec.taps.iter().map(|n| serde_json::json!({
                    "noteIndex": n.note_index,
                    "slot": n.slot.to_string(),
                })).collect::<Vec<_>>(),
                "holds": spec.holds.iter().map(|n| serde_json::json!({
                    "noteIndex": n.note_index,
                    "slot": n.slot.to_string(),
                })).collect::<Vec<_>>(),
                "slides": spec.slides.iter().map(|n| serde_json::json!({
                    "noteIndex": n.note_index,
                    "slot": "C",
                })).collect::<Vec<_>>(),
            }
        });
        Ok(FfiEnvelope {
            json: json.to_string(),
        })
    }

    pub fn advance_frame_light(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        let res = lnmai_core_rs::ffi::step_session_light(self.handle, batch_json)
            .map_err(|e| LnmaiError { json: e })?;
        let events: Vec<serde_json::Value> = res
            .events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "kind": e.kind,
                    "grade": e.grade,
                    "noteIndex": e.note_index,
                })
            })
            .collect();
        let result = serde_json::json!({
            "ok": true,
            "result": {
                "events": events,
                "score": {
                    "combo": res.score.combo,
                },
                "currentTime": res.current_time.ticks.0,
            }
        });
        Ok(FfiEnvelope {
            json: result.to_string(),
        })
    }

    pub fn advance_frame_full(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        let res = lnmai_core_rs::ffi::step_session(self.handle, batch_json)
            .map_err(|e| LnmaiError { json: e })?;
        let events: Vec<serde_json::Value> = res
            .events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "kind": e.kind,
                    "grade": e.grade,
                    "noteIndex": e.note_index,
                })
            })
            .collect();
        let result = serde_json::json!({
            "ok": true,
            "result": {
                "events": events,
                "state": res.state,
            }
        });
        Ok(FfiEnvelope {
            json: result.to_string(),
        })
    }

    pub fn get_state_json(&self) -> Result<FfiEnvelope> {
        let state = lnmai_core_rs::ffi::get_game_state_json(self.handle)
            .map_err(|e| LnmaiError { json: e })?;
        let json = serde_json::json!({
            "ok": true,
            "result": state,
        });
        Ok(FfiEnvelope {
            json: json.to_string(),
        })
    }

    pub fn unload_chart(self) -> Result<(Session<Empty>, FfiEnvelope)> {
        let handle = self.handle;
        lnmai_core_rs::ffi::unload_chart_from_session(handle)
            .map_err(|e| LnmaiError { json: e })?;
        Ok((
            Session {
                handle,
                _state: PhantomData,
            },
            FfiEnvelope {
                json: "{\"ok\":true}".to_string(),
            },
        ))
    }

    pub fn free(self) -> Result<FfiEnvelope> {
        lnmai_core_rs::ffi::free_session(self.handle);
        Ok(FfiEnvelope {
            json: "{\"ok\":true}".to_string(),
        })
    }
}

// ── Simai text → ChartSpec JSON conversion ──

use lnmai_core_rs::areas::{OuterSlot, SensorArea};
use lnmai_core_rs::chart_loader::{
    ChartSpec, HoldChartNote, SlideChartNote, TapChartNote, TouchChartNote, TouchHoldChartNote,
};
use lnmai_core_rs::time::{Duration, TimePoint};
use lnmai_core_rs::types::SlideKind;
use maisimai::{SimaiChart, SimaiNote, SlidePattern};

fn simai_text_to_chart_spec_json(content: &str, level_index: u32) -> Result<String> {
    let file = maisimai::parse_file(content)
        .map_err(|e| LnmaiError { json: format!("{e:?}") })?;
    let chart = file
        .charts
        .iter()
        .find(|(lv, _)| *lv == level_index)
        .map(|(_, c)| c)
        .or_else(|| file.charts.get((level_index as usize).saturating_sub(1)).map(|(_, c)| c))
        .or_else(|| file.charts.first().map(|(_, c)| c))
        .ok_or_else(|| LnmaiError {
            json: format!("no chart found for level_index={level_index}"),
        })?;
    let spec = simai_chart_to_chart_spec(chart);
    serde_json::to_string(&spec).map_err(|e| LnmaiError { json: e.to_string() })
}

fn simai_chart_to_chart_spec(chart: &SimaiChart) -> ChartSpec {
    let bpms: Vec<maisimai::Bpm> = chart.bpms.clone();
    let mut taps = Vec::new();
    let mut holds = Vec::new();
    let mut touches = Vec::new();
    let mut touch_holds = Vec::new();
    let mut slides = Vec::new();
    let mut note_index: u64 = 0;

    for note in &chart.notes {
        note_index += 1;
        match note {
            SimaiNote::Tap {
                measure,
                button,
                is_break,
                is_ex,
                ..
            } => {
                taps.push(TapChartNote {
                    timing: measure_to_time(*measure, &bpms),
                    slot: button_to_slot(*button),
                    is_break: *is_break,
                    is_ex: *is_ex,
                    button_queue_index: note_index,
                    note_index,
                });
            }
            SimaiNote::Hold {
                measure,
                button,
                duration,
                is_ex,
            } => {
                holds.push(HoldChartNote {
                    timing: measure_to_time(*measure, &bpms),
                    slot: button_to_slot(*button),
                    length: measure_to_duration(*duration, &bpms),
                    is_break: false,
                    is_ex: *is_ex,
                    is_touch: false,
                    is_classic: None,
                    button_queue_index: note_index,
                    touch_hold_group_id: None,
                    touch_hold_group_size: None,
                    note_index,
                });
            }
            SimaiNote::Slide {
                measure,
                start,
                pattern,
                duration,
                delay,
                ..
            } => {
                let kind = if matches!(pattern, SlidePattern::Wifi) {
                    SlideKind::Wifi
                } else {
                    SlideKind::Single
                };
                slides.push(SlideChartNote {
                    timing: measure_to_time(*measure, &bpms),
                    slot: button_to_slot(*start),
                    length: measure_to_duration(*duration, &bpms),
                    start_timing: measure_to_time(*measure + *delay, &bpms),
                    slide_kind: kind,
                    note_index,
                    ..Default::default()
                });
            }
            SimaiNote::TouchTap {
                measure,
                region,
                position,
                ..
            } => {
                touches.push(TouchChartNote {
                    timing: measure_to_time(*measure, &bpms),
                    sensor_pos: region_pos_to_sensor(*region, *position),
                    is_break: false,
                    touch_queue_index: note_index,
                    touch_group_id: None,
                    touch_group_size: None,
                    note_index,
                });
            }
            SimaiNote::TouchHold {
                measure,
                region,
                position,
                duration,
                ..
            } => {
                touch_holds.push(TouchHoldChartNote {
                    timing: measure_to_time(*measure, &bpms),
                    sensor_pos: region_pos_to_sensor(*region, *position),
                    length: measure_to_duration(*duration, &bpms),
                    is_break: false,
                    is_ex: false,
                    touch_queue_index: note_index,
                    touch_group_id: None,
                    touch_group_size: None,
                    touch_hold_group_id: None,
                    touch_hold_group_size: None,
                    note_index,
                });
            }
        }
    }

    ChartSpec {
        taps,
        holds,
        touches,
        touch_holds,
        slides,
        slide_skipping: None,
    }
}

fn button_to_slot(button: u8) -> OuterSlot {
    match button % 8 {
        0 => OuterSlot::S1,
        1 => OuterSlot::S2,
        2 => OuterSlot::S3,
        3 => OuterSlot::S4,
        4 => OuterSlot::S5,
        5 => OuterSlot::S6,
        6 => OuterSlot::S7,
        _ => OuterSlot::S8,
    }
}

fn region_pos_to_sensor(region: char, position: u8) -> SensorArea {
    let idx = (position % 8) + 1;
    match region.to_ascii_uppercase() {
        'A' => match idx {
            1 => SensorArea::A1, 2 => SensorArea::A2, 3 => SensorArea::A3, 4 => SensorArea::A4,
            5 => SensorArea::A5, 6 => SensorArea::A6, 7 => SensorArea::A7, _ => SensorArea::A8,
        },
        'B' => match idx {
            1 => SensorArea::B1, 2 => SensorArea::B2, 3 => SensorArea::B3, 4 => SensorArea::B4,
            5 => SensorArea::B5, 6 => SensorArea::B6, 7 => SensorArea::B7, _ => SensorArea::B8,
        },
        'D' => match idx {
            1 => SensorArea::D1, 2 => SensorArea::D2, 3 => SensorArea::D3, 4 => SensorArea::D4,
            5 => SensorArea::D5, 6 => SensorArea::D6, 7 => SensorArea::D7, _ => SensorArea::D8,
        },
        'E' => match idx {
            1 => SensorArea::E1, 2 => SensorArea::E2, 3 => SensorArea::E3, 4 => SensorArea::E4,
            5 => SensorArea::E5, 6 => SensorArea::E6, 7 => SensorArea::E7, _ => SensorArea::E8,
        },
        _ => SensorArea::C,
    }
}

fn measure_to_time(measure: f32, bpms: &[maisimai::Bpm]) -> TimePoint {
    let s = maisimai::measure_to_seconds(measure, bpms);
    TimePoint::from_micros((s * 1_000_000.0) as i64)
}

fn measure_to_duration(measure: f32, bpms: &[maisimai::Bpm]) -> Duration {
    let s = maisimai::measure_to_seconds(measure, bpms);
    Duration::from_micros((s * 1_000_000.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simai_chart_with_taps() -> &'static str {
        "&title=Round Trip\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,2,3,4,E\n"
    }

    #[test]
    fn session_round_trip() {
        unsafe {
            assert!(initialize_runtime().is_ok());
        }

        let empty = Session::<Empty>::create().expect("create session");
        let simai = simai_chart_with_taps();
        let (loaded, envelope) = empty
            .load_chart_text(simai, 6)
            .expect("load chart text");
        assert!(envelope.json.contains("ok"));

        let lowered = loaded
            .get_lowered_chart_json()
            .expect("get lowered chart");
        let v: serde_json::Value =
            serde_json::from_str(&lowered.json).expect("parse lowered");
        let result = &v["result"];
        let taps = result["taps"].as_array().expect("taps array");
        assert_eq!(taps.len(), 4);
        assert_eq!(taps[0]["noteIndex"], 1);
        assert_eq!(taps[0]["slot"], "S1");
        assert_eq!(taps[1]["noteIndex"], 2);
        assert_eq!(taps[1]["slot"], "S2");

        loaded.free().expect("free");
    }

    #[test]
    fn advance_frame_produces_events() {
        unsafe {
            assert!(initialize_runtime().is_ok());
        }

        let simai = "&title=Hit\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,E\n";
        let empty = Session::<Empty>::create().expect("create");
        let (mut loaded, _) = empty.load_chart_text(simai, 6).expect("load");

        let batch = serde_json::json!({
            "current_time": {"ticks": 0},
            "events": [{
                "ButtonClick": {
                    "tp": {"ticks": 0},
                    "zone": "K1"
                }
            }]
        });
        let result = loaded
            .advance_frame_light(&batch.to_string())
            .expect("advance frame");
        assert!(result.json.contains("ok"));

        loaded.free().expect("free");
    }

    #[test]
    fn load_chart_json_round_trip() {
        use lnmai_core_rs::chart_loader::ChartSpec;
        use lnmai_core_rs::areas::OuterSlot;
        use lnmai_core_rs::time::TimePoint;

        unsafe {
            assert!(initialize_runtime().is_ok());
        }

        let spec = ChartSpec {
            taps: vec![
                lnmai_core_rs::chart_loader::TapChartNote {
                    timing: TimePoint::from_micros(0),
                    slot: OuterSlot::S1,
                    note_index: 1,
                    button_queue_index: 1,
                    ..Default::default()
                },
                lnmai_core_rs::chart_loader::TapChartNote {
                    timing: TimePoint::from_micros(500_000),
                    slot: OuterSlot::S2,
                    note_index: 2,
                    button_queue_index: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let spec_json = serde_json::to_string(&spec).expect("serialize");

        let empty = Session::<Empty>::create().expect("create");
        let (loaded, _) = empty.load_chart_json(&spec_json).expect("load json");

        let lowered = loaded.get_lowered_chart_json().expect("lowered");
        let v: serde_json::Value = serde_json::from_str(&lowered.json).expect("parse");
        assert_eq!(v["result"]["taps"].as_array().unwrap().len(), 2);

        loaded.free().expect("free");
    }
}
