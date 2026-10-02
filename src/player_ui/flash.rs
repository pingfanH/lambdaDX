//! Flash (XFL) page layout, hit regions and pointer interaction.
//!
//! The `player_ui` XFL pages are pure visuals; this module maps their widgets
//! to page-space rectangles (the same 1280×760 stage coordinates the pages were
//! authored in), hit-tests the pointer, and returns an [`Action`]. The bin maps
//! actions onto [`crate::player_ui::state::PlayerUiApp`].

use crate::app::types::RectF;

/// A dynamic-text slot exported by the XFL generator (`text_slots.json`): a
/// page-space box plus the placeholder string baked into the XFL, which the
/// runtime skips and replaces with live data.
#[derive(Clone, serde::Deserialize)]
pub struct Slot {
    pub page: String,
    pub id: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub color: String,
    #[serde(default)]
    pub align: String,
}

#[derive(serde::Deserialize)]
struct SlotFile {
    slots: Vec<Slot>,
}

/// Load the slots manifest; an empty list when missing/malformed.
pub fn load_slots(path: &std::path::Path) -> Vec<Slot> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<SlotFile>(&text)
        .map(|f| f.slots)
        .unwrap_or_default()
}

impl Slot {
    pub fn rgba(&self) -> [u8; 4] {
        let h = self.color.trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
        if h.len() >= 8 {
            [byte(0), byte(2), byte(4), byte(6)]
        } else {
            [byte(0), byte(2), byte(4), 255]
        }
    }

    pub fn is_center(&self) -> bool {
        self.align == "center"
    }
    pub fn is_right(&self) -> bool {
        self.align == "right"
    }
}

/// Page order used by the bin's Flash mode (matches the XFL `animations`).
pub const PAGES: [&str; 5] = [
    "UI/page_start",
    "UI/page_song_select",
    "UI/page_settings",
    "UI/page_gameplay_hud",
    "UI/page_pause",
];

/// How a widget's state frames are chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 0 idle · 1 hover · 2 pressed.
    Button,
    /// 0 normal · 1 hover · 2 selected.
    Row,
    /// 0 normal · 1 selected.
    Tab,
    /// 0 off · 1 on.
    Toggle,
    /// A drag control (fill + knob drawn dynamically).
    Slider,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    GoStart,
    GoSelect,
    GoSettings,
    GoGameplay,
    CloseSettings,
    SelectRow(usize),
    Resume,
    Restart,
    ExitToSelect,
    Section(usize),
    ToggleAudio,
    ResetSettings,
    SetSlider(usize, f32),
    ToggleAutoplay,
    TogglePlay,
    PauseGame,
}

pub struct Hit {
    pub id: &'static str,
    /// Page-space rectangle (top-left + size).
    pub rect: RectF,
    /// Widget symbol, e.g. `UI/ui_btn_primary`.
    pub sym: &'static str,
    pub sx: f32,
    pub sy: f32,
    pub kind: Kind,
    pub action: Action,
}

impl Hit {
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x && x <= self.rect.x + self.rect.w && y >= self.rect.y && y <= self.rect.y + self.rect.h
    }
}

fn button(id: &'static str, x: f32, y: f32, w: f32, h: f32, sym: &'static str, action: Action) -> Hit {
    Hit {
        id,
        rect: RectF { x, y, w, h },
        sym,
        sx: w / 180.0,
        sy: h / 46.0,
        kind: Kind::Button,
        action,
    }
}

/// Interactive widgets of a page, in page coordinates.
pub fn hits(page: &str) -> Vec<Hit> {
    match page {
        "UI/page_start" => vec![
            button("start", 56.0, 362.0, 360.0, 50.0, "UI/ui_btn_primary", Action::GoSelect),
            button("settings", 56.0, 424.0, 360.0, 50.0, "UI/ui_btn_secondary", Action::GoSettings),
        ],
        "UI/page_song_select" => {
            let mut v = vec![
                button("back", 20.0, 14.0, 84.0, 36.0, "UI/ui_btn_quiet", Action::GoStart),
                button("play", 690.0, 608.0, 320.0, 50.0, "UI/ui_btn_primary", Action::GoGameplay),
            ];
            for i in 0..6 {
                v.push(Hit {
                    id: "row",
                    rect: RectF { x: 16.0, y: 164.0 + i as f32 * 76.0, w: 388.0, h: 66.0 },
                    sym: "UI/ui_row",
                    sx: 1.0,
                    sy: 1.0,
                    kind: Kind::Row,
                    action: Action::SelectRow(i),
                });
            }
            v
        }
        "UI/page_settings" => {
            let mut v = vec![
                button("back", 20.0, 14.0, 84.0, 36.0, "UI/ui_btn_quiet", Action::CloseSettings),
                button("reset", 272.0, 548.0, 180.0, 46.0, "UI/ui_btn_quiet", Action::ResetSettings),
                Hit {
                    id: "toggle",
                    rect: RectF { x: 788.0, y: 166.0, w: 40.0, h: 22.0 },
                    sym: "UI/ui_toggle",
                    sx: 1.0,
                    sy: 1.0,
                    kind: Kind::Toggle,
                    action: Action::ToggleAudio,
                },
            ];
            for i in 0..3 {
                v.push(Hit {
                    id: "tab",
                    rect: RectF { x: 16.0, y: 112.0 + i as f32 * 68.0, w: 208.0, h: 60.0 },
                    sym: "UI/ui_tab",
                    sx: 1.0,
                    sy: 1.0,
                    kind: Kind::Tab,
                    action: Action::Section(i),
                });
            }
            for i in 0..4 {
                v.push(Hit {
                    id: "slider",
                    rect: RectF { x: 272.0, y: 240.0 + i as f32 * 72.0, w: 560.0, h: 36.0 },
                    sym: "UI/ui_slider",
                    sx: 560.0 / 300.0,
                    sy: 1.0,
                    kind: Kind::Slider,
                    action: Action::SetSlider(i, 0.0),
                });
            }
            v
        }
        "UI/page_gameplay_hud" => vec![
            button("pause", 16.0, 13.0, 84.0, 40.0, "UI/ui_btn_secondary", Action::PauseGame),
            button("auto", 1124.0, 17.0, 70.0, 32.0, "UI/ui_btn_quiet", Action::ToggleAutoplay),
            button("play", 1202.0, 17.0, 62.0, 32.0, "UI/ui_btn_primary", Action::TogglePlay),
        ],
        "UI/page_pause" => vec![
            button("resume", 458.0, 360.0, 180.0, 46.0, "UI/ui_btn_primary", Action::Resume),
            button("restart", 458.0, 416.0, 180.0, 46.0, "UI/ui_btn_secondary", Action::Restart),
            button("settings", 458.0, 472.0, 180.0, 46.0, "UI/ui_btn_quiet", Action::GoSettings),
            button("exit", 458.0, 528.0, 180.0, 46.0, "UI/ui_btn_danger", Action::ExitToSelect),
        ],
        _ => Vec::new(),
    }
}

/// Page transform: the pages are authored on a 1280×760 stage, drawn uniformly
/// scaled and centered.
pub struct PageXf {
    pub ox: f32,
    pub oy: f32,
    pub scale: f32,
}

impl PageXf {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        Self {
            ox: (w - 1280.0 * scale) * 0.5,
            oy: (h - 760.0 * scale) * 0.5,
            scale,
        }
    }

    /// Screen point → page point.
    pub fn to_page(&self, mx: f32, my: f32) -> (f32, f32) {
        ((mx - self.ox) / self.scale, (my - self.oy) / self.scale)
    }

    /// A widget's screen-space matrix (`page ∘ translate(pos) ∘ scale`).
    pub fn widget(&self, hit: &Hit) -> [f32; 6] {
        [
            self.scale * hit.sx,
            0.0,
            0.0,
            self.scale * hit.sy,
            self.ox + self.scale * hit.rect.x,
            self.oy + self.scale * hit.rect.y,
        ]
    }

    pub fn x(&self, px: f32) -> f32 {
        self.ox + self.scale * px
    }
    pub fn y(&self, py: f32) -> f32 {
        self.oy + self.scale * py
    }
}

/// Pointer interaction state for one page.
#[derive(Default)]
pub struct FlashUi {
    pub hover: Option<usize>,
    pub press: Option<usize>,
    /// Per-widget state, for cross-fading a state change: `(page, hit) -> frame`.
    state: std::collections::HashMap<(usize, usize), usize>,
    /// `(page, hit) -> (from_frame, start_time)` while a change is fading.
    trans: std::collections::HashMap<(usize, usize), (usize, f64)>,
}

impl FlashUi {
    /// Register `frame` as a widget's shown state and, if it changed, return the
    /// previous frame with the alpha to draw it at (fading out over ~0.12 s).
    pub fn transition(&mut self, key: (usize, usize), frame: usize, now: f64) -> Option<(usize, f32)> {
        match self.state.get(&key).copied() {
            Some(prev) if prev != frame => {
                self.trans.insert(key, (prev, now));
                self.state.insert(key, frame);
            }
            None => {
                self.state.insert(key, frame);
            }
            _ => {}
        }
        let (from, t0) = *self.trans.get(&key)?;
        let t = ((now - t0) / 0.12).clamp(0.0, 1.0) as f32;
        if t >= 1.0 {
            self.trans.remove(&key);
            return None;
        }
        Some((from, 1.0 - t))
    }

    /// Update hover/press and return an action, if any.
    pub fn update(
        &mut self,
        page: &str,
        pointer: Option<(f32, f32)>,
        down: bool,
        pressed: bool,
        released: bool,
    ) -> Option<Action> {
        let list = hits(page);
        self.hover = pointer.and_then(|(x, y)| list.iter().rposition(|h| h.contains_point(x, y)));
        let mut action = None;
        if pressed {
            self.press = self.hover;
        }
        if down {
            if let (Some(p), Some((x, _))) = (self.press, pointer) {
                if let Some(h) = list.get(p).filter(|h| h.kind == Kind::Slider) {
                    // Report the slider's ordinal (its Action payload), not its
                    // position in the hit list.
                    let ord = match h.action {
                        Action::SetSlider(o, _) => o,
                        _ => p,
                    };
                    let frac = ((x - h.rect.x) / h.rect.w).clamp(0.0, 1.0);
                    action = Some(Action::SetSlider(ord, frac));
                }
            }
        }
        if released {
            if let (Some(p), Some(h)) = (self.press, self.hover) {
                if p == h {
                    action = list.get(h).map(|hit| hit.action);
                }
            }
            self.press = None;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_button_click_returns_action() {
        let mut ui = FlashUi::default();
        let p = (100.0, 380.0); // inside the "start" button (56,362,360,50)
        assert!(ui.update("UI/page_start", Some(p), false, true, false).is_none());
        assert_eq!(
            ui.update("UI/page_start", Some(p), false, false, true),
            Some(Action::GoSelect)
        );
    }

    #[test]
    fn page_xf_maps_screen_to_page() {
        let xf = PageXf::new(2560.0, 1520.0, 2.0);
        assert_eq!(xf.to_page(0.0, 0.0), (0.0, 0.0));
        assert_eq!(xf.to_page(2.0 * 640.0, 2.0 * 380.0), (640.0, 380.0));
    }

    #[test]
    #[test]
    fn state_change_cross_fades_previous_frame() {
        let mut ui = FlashUi::default();
        let key = (2usize, 5usize);
        assert_eq!(ui.transition(key, 0, 0.0), None);
        assert_eq!(ui.transition(key, 1, 0.0), Some((0, 1.0)));
        match ui.transition(key, 1, 0.06) {
            Some((0, a)) => assert!((a - 0.5).abs() < 0.15, "alpha {a}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(ui.transition(key, 1, 0.3), None);
    }

    fn slider_drag_reports_fraction() {
        let mut ui = FlashUi::default();
        let page = "UI/page_settings";
        let slider = hits(page)
            .into_iter()
            .position(|h| h.kind == Kind::Slider)
            .expect("a slider");
        let h = &hits(page)[slider];
        let mid = (h.rect.x + h.rect.w * 0.5, h.rect.y + 10.0);
        ui.update(page, Some(mid), false, true, false); // press on slider
        let a = ui.update(page, Some(mid), true, false, false);
        match a {
            Some(Action::SetSlider(_, frac)) => assert!((frac - 0.5).abs() < 0.01),
            other => panic!("expected slider action, got {other:?}"),
        }
    }
}
