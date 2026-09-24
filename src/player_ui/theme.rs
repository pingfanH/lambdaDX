//! Palette + motion constants for the black-minimal, cyan-accent player UI.
//!
//! Surfaces follow the Editor-1 / Blender dark stack (near-black panels split by
//! 1px hairlines, a single playhead accent). Motion borrows the sticker-sheet
//! idioms from the reference site: staggered reveals, spring pops, pressed-in
//! offsets and a page sweep — rendered flat and precise instead of inked.

use macroquad::color::Color;

// ── Surfaces ─────────────────────────────────────────────────────────
pub const VOID: Color = Color::from_rgba(0x17, 0x17, 0x19, 255);
/// Re-exported from the shared pad renderer so the pad backdrop matches.
pub use crate::player::render::{BORDER_SOFT, GRID, PANEL};
pub const PANEL_ALT: Color = Color::from_rgba(0x24, 0x24, 0x27, 255);
pub const RAISED: Color = Color::from_rgba(0x2a, 0x2a, 0x2e, 255);
pub const RAISED_HOVER: Color = Color::from_rgba(0x34, 0x34, 0x3a, 255);

// ── Lines ────────────────────────────────────────────────────────────
pub const BORDER: Color = Color::from_rgba(0x3a, 0x3a, 0x40, 255);

// ── Text ─────────────────────────────────────────────────────────────
pub const TEXT: Color = Color::from_rgba(0xd6, 0xd6, 0xda, 255);
pub const TEXT_DIM: Color = Color::from_rgba(0x9a, 0x9a, 0xa2, 255);
pub const TEXT_MUTED: Color = Color::from_rgba(0x6e, 0x6e, 0x78, 255);

// ── Accents ──────────────────────────────────────────────────────────
pub const ACCENT: Color = Color::from_rgba(0x52, 0xd6, 0xe8, 255);
pub const ACCENT_DIM: Color = Color::from_rgba(0x2c, 0x74, 0x81, 255);
pub const ACCENT_FAINT: Color = Color::from_rgba(0x52, 0xd6, 0xe8, 28);
pub const DANGER: Color = Color::from_rgba(0xff, 0x3b, 0x50, 255);
pub const DANGER_DIM: Color = Color::from_rgba(0x7a, 0x22, 0x2b, 255);
pub const SUCCESS: Color = Color::from_rgba(0x69, 0xd3, 0x91, 255);

/// Stamp/level accent used on level pills.
pub const LEVEL: Color = Color::from_rgba(0xc8, 0xff, 0x2e, 255);

// ── Motion ───────────────────────────────────────────────────────────
/// Page reveal / list-card entrance.
pub const D_CARD: f64 = 0.24;
/// Per-item stagger between list cards.
pub const STAGGER: f64 = 0.035;
/// Spring pop (tooltips, overlays, badge).
pub const D_POP: f64 = 0.20;
/// Page sweep band traverse.
pub const D_SWEEP: f64 = 0.42;
/// Hover smoothing rate (larger = snappier).
pub const HOVER_RATE: f32 = 14.0;
/// Selected-row slide distance.
pub const SELECT_SLIDE: f32 = 6.0;
/// Hover-row slide distance.
pub const HOVER_SLIDE: f32 = 3.0;
/// Cut corner size for panels/cards.
pub const CUT: f32 = 14.0;
