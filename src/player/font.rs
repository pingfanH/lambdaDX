//! CJK-capable font loading, shared by the pad preview's macroquad text (HUD)
//! and the egui params panel.
//!
//! macroquad's built-in font and egui's default fonts are ASCII-only, so any
//! Chinese text renders as blanks/tofu. We prefer a plain `.ttf` (macOS ships
//! `Arial Unicode.ttf`, which fontdue can parse) because fontdue often rejects
//! `.ttc` collections.

use std::cell::RefCell;
use std::path::PathBuf;

use macroquad::prelude::{Color, Font, TextParams, draw_text_ex, measure_text};

/// Candidate font files, most preferred first (`MAI2_FONT_PATH` overrides).
const CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/System/Library/Fonts/Supplemental/Songti.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto-cjk/NotoSansCJK-VF.otf.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\msyh.ttf",
];

/// Candidate paths (`MAI2_FONT_PATH` first when set).
pub fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("MAI2_FONT_PATH") {
        if !p.trim().is_empty() {
            v.push(PathBuf::from(p));
        }
    }
    v.extend(CANDIDATES.iter().map(PathBuf::from));
    v
}

/// Raw bytes of the first readable candidate (for egui's own font parser).
pub fn bytes() -> Option<Vec<u8>> {
    for path in candidates() {
        if let Ok(b) = std::fs::read(&path) {
            if !b.is_empty() {
                return Some(b);
            }
        }
    }
    None
}

thread_local! {
    static FONT: RefCell<Option<Option<Font>>> = RefCell::new(None);
}

/// A macroquad font handle for CJK text, loaded once and cached per thread.
pub fn handle() -> Option<Font> {
    FONT.with(|f| {
        let mut slot = f.borrow_mut();
        if slot.is_none() {
            *slot = Some(load());
        }
        slot.clone().flatten()
    })
}

fn load() -> Option<Font> {
    for path in candidates() {
        if let Ok(b) = std::fs::read(&path) {
            if let Ok(font) = macroquad::text::load_ttf_font_from_bytes(&b) {
                return Some(font);
            }
        }
    }
    // Bundled Arial is ASCII-only but always present.
    macroquad::text::load_ttf_font_from_bytes(include_bytes!("../../assets/Arial.ttf")).ok()
}

/// Draw text with the CJK font (falls back to the default font).
pub fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        s,
        x,
        y,
        TextParams {
            font: handle().as_ref(),
            font_size: size.round() as u16,
            font_scale: 1.0,
            color,
            ..Default::default()
        },
    );
}

/// Width of `s` in the CJK font.
pub fn text_width(s: &str, size: f32) -> f32 {
    let font = handle();
    measure_text(s, font.as_ref(), size.round() as u16, 1.0).width
}
