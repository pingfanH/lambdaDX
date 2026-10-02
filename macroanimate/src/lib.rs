// src/lib.rs
pub mod sparrow_atlas;
pub mod texture_atlas;
pub mod xfl;

// Re-export core data structures for clean root access
pub use sparrow_atlas::{SparrowFrame, parse_sparrow};
pub use texture_atlas::{TextureAtlas, draw_part_mesh, get_texture_parts, parse_texture_atlas};
pub use xfl::{
    DrawPart, PartContent, TextAlign, TextDraw, TextRun, XflAsset, XflAtlas, XflClip, XflDrawXf,
    XflError, draw_part, draw_part_tinted, draw_parts, draw_parts_tinted, draw_parts_xf,
};
