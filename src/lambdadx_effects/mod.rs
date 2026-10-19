//! LambdaDX Macroquad runtime for extracted particle and shader effects.
//! The preview binary intentionally only owns the window
//! and input state; effect data and rendering live in this module.

mod data;
mod particle;
mod reference_motion;
mod shader;
mod simulation;

pub use particle::EffectLibrary;
pub use simulation::{EffectInstance, Judgment};
