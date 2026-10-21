//! CJK-capable font loading, shared by the pad preview's macroquad text (HUD)
//! and the egui params panel.
//!
//! macroquad's built-in font and egui's default fonts are ASCII-only, so any
//! Chinese text renders as blanks/tofu. The resolver checks the usual platform
//! paths, fontconfig, and font directories, then falls back to the bundled
//! WenQuanYi Micro Hei. macroquad/fontdue additionally skips CFF2 variable
//! collections (they rasterize as solid boxes) and bitmap-only fonts.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    "/run/current-system/sw/share/fonts/opentype/noto-cjk/NotoSansCJK-VF.otf.ttc",
    "/nix/var/nix/profiles/default/share/fonts/opentype/noto-cjk/NotoSansCJK-VF.otf.ttc",
];

/// Directories searched for a CJK font when the fixed candidates miss (NixOS
/// keeps fonts in the store, so the usual `/usr/share/fonts` paths are empty).
const FONT_DIRS: &[&str] = &[
    "/usr/share/fonts",
    "/usr/local/share/fonts",
    "/run/current-system/sw/share/fonts",
    "/nix/var/nix/profiles/default/share/fonts",
    "/System/Library/Fonts",
    "/System/Library/Fonts/Supplemental",
    "/Library/Fonts",
    "C:\\Windows\\Fonts",
];

/// Filename fragments that identify a font likely to contain Han glyphs.
const CJK_NAMES: &[&str] = &[
    "cjk",
    "noto sans",
    "source han",
    "wqy",
    "wenquanyi",
    "droid sans fallback",
    "pingfang",
    "hiragino",
    "heiti",
    "songti",
    "msyh",
    "simhei",
    "simsun",
    "arial unicode",
];

/// Candidate paths (`MAI2_FONT_PATH` first when set).
pub fn candidates() -> Vec<PathBuf> {
    let mut v = static_candidates();
    v.extend(discovered_candidates());
    v.push(bundled_candidate());
    dedupe(&mut v);
    v
}

fn static_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("MAI2_FONT_PATH") {
        if !p.trim().is_empty() {
            v.push(PathBuf::from(p));
        }
    }
    v.extend(CANDIDATES.iter().map(PathBuf::from));
    v
}

fn discovered_candidates() -> Vec<PathBuf> {
    let mut v = fontconfig_fonts();
    for dir in FONT_DIRS {
        scan_dir(Path::new(dir), 4, &mut v);
    }
    for dir in user_font_dirs() {
        scan_dir(&dir, 4, &mut v);
    }
    v
}

fn bundled_candidate() -> PathBuf {
    crate::app::platform::asset_dir().join("wqy-microhei.ttc")
}

fn dedupe(paths: &mut Vec<PathBuf>) {
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

/// Candidate order for macroquad: usable platform fonts, then the bundled
/// glyf fallback, then fontconfig/directory discoveries. This keeps bitmap-only
/// `.otb` fonts and CFF2 variable collections from winning on minimal Linux.
fn macroquad_candidates() -> Vec<PathBuf> {
    let mut v = static_candidates();
    v.push(bundled_candidate());
    v.extend(fontconfig_fonts());
    for dir in FONT_DIRS {
        scan_dir(Path::new(dir), 4, &mut v);
    }
    for dir in user_font_dirs() {
        scan_dir(&dir, 4, &mut v);
    }
    dedupe(&mut v);
    v
}

fn user_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for key in ["XDG_DATA_HOME", "HOME"] {
        let Ok(root) = std::env::var(key) else { continue };
        if root.trim().is_empty() {
            continue;
        }
        let root = PathBuf::from(root);
        if key == "XDG_DATA_HOME" {
            dirs.push(root.join("fonts"));
        } else {
            dirs.push(root.join(".local/share/fonts"));
            dirs.push(root.join(".fonts"));
        }
    }
    dirs
}

fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            scan_dir(&path, depth - 1, out);
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !name.ends_with(".ttf") && !name.ends_with(".otf") && !name.ends_with(".ttc") {
            continue;
        }
        if CJK_NAMES.iter().any(|needle| name.contains(needle)) {
            out.push(path);
        }
    }
}

/// Fonts fontconfig reports for Chinese. `fc-match` often returns a Latin
/// fallback on minimal installs, so prefer the `fc-list` candidates and rank
/// proportional CJK families ahead of monospace/serif ones.
fn fontconfig_fonts() -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        use std::process::Command;

        let Ok(output) = Command::new("fc-list")
            .args(["-f", "%{file}|%{family[0]}\n", ":lang=zh"])
            .output()
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let mut ranked = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((file, family)) = line.rsplit_once('|') else {
                continue;
            };
            let path = PathBuf::from(file.trim());
            if !path.is_file() {
                continue;
            }
            ranked.push((family_rank(family), path));
        }
        ranked.sort_by(|a, b| a.0.cmp(&b.0));
        let mut seen = HashSet::new();
        ranked
            .into_iter()
            .map(|(_, path)| path)
            .filter(|path| seen.insert(path.clone()))
            .collect()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

fn family_rank(family: &str) -> u8 {
    let family = family.to_ascii_lowercase();
    let sc = family.contains(" sc") || family.ends_with("sc");
    let base = if family.contains("sans") && family.contains("cjk") && !family.contains("mono") {
        0
    } else if family.contains("sans") && family.contains("cjk") {
        1
    } else if family.contains("cjk") {
        2
    } else {
        3
    };
    if sc { base } else { base.saturating_add(4) }
}

fn has_cjk_glyphs(bytes: &[u8]) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
            return false;
        };
        #[derive(Default)]
        struct Points(usize);
        impl ttf_parser::OutlineBuilder for Points {
            fn move_to(&mut self, _: f32, _: f32) { self.0 += 1; }
            fn line_to(&mut self, _: f32, _: f32) { self.0 += 1; }
            fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.0 += 2; }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.0 += 3; }
            fn close(&mut self) {}
        }
        // A cmap entry alone is not enough: bitmap-only `.otb` collections
        // (Unifont on NixOS) list Han glyphs but have no outlines.
        ['P', 'v', '中', '文'].into_iter().all(|c| {
            let Some(id) = face.glyph_index(c) else {
                return false;
            };
            let mut points = Points::default();
            face.outline_glyph(id, &mut points).is_some() && points.0 > 4
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = bytes;
        true
    }
}

/// Whether fontdue can actually rasterize this font at UI sizes. This rejects
/// bitmap collections and any face whose outlines produce an empty bitmap.
fn fontdue_renders(bytes: &[u8]) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) else {
            return false;
        };
        ['P', 'v', '中'].into_iter().all(|c| {
            let (metrics, bitmap) = font.rasterize(c, 24.0);
            metrics.width > 0 && metrics.height > 0 && bitmap.iter().any(|&coverage| coverage > 0)
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = bytes;
        true
    }
}

/// fontdue 0.9 cannot rasterize CFF2 variable collections correctly (it fills
/// the glyph bounds with a solid block). Outline-bearing system CJK fonts on
/// NixOS are exactly that format, so the macroquad path skips them and uses the
/// bundled glyf-outline fallback instead. egui/ab_glyph handles CFF2 fine.
fn has_cff2_outlines(bytes: &[u8]) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        ttf_parser::Face::parse(bytes, 0)
            .map(|face| face.tables().cff2.is_some())
            .unwrap_or(false)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = bytes;
        false
    }
}

/// Raw bytes of the first readable candidate (for egui's own font parser).
pub fn bytes() -> Option<Vec<u8>> {
    static RESOLVED: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    RESOLVED
        .get_or_init(|| {
            for path in candidates() {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.is_empty() || !has_cjk_glyphs(&bytes) {
                    continue;
                }
                return Some(bytes);
            }
            None
        })
        .clone()
}

/// Font bytes for macroquad's fontdue rasterizer. Prefers the first CJK font
/// with glyf/CFF1 outlines; variable CFF2 collections are skipped because they
/// render as solid boxes (see [`has_cff2_outlines`]).
fn macroquad_bytes() -> Option<Vec<u8>> {
    static RESOLVED: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    RESOLVED
        .get_or_init(|| {
            for path in macroquad_candidates() {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.is_empty()
                    || !has_cjk_glyphs(&bytes)
                    || has_cff2_outlines(&bytes)
                    || !fontdue_renders(&bytes)
                {
                    continue;
                }
                return Some(bytes);
            }
            None
        })
        .clone()
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
    if let Some(bytes) = macroquad_bytes() {
        if let Ok(font) = macroquad::text::load_ttf_font_from_bytes(&bytes) {
            return Some(font);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_prefers_proportional_simplified_cjk() {
        assert!(family_rank("Noto Sans CJK SC") < family_rank("Noto Sans Mono CJK SC"));
        assert!(family_rank("Noto Sans Mono CJK SC") < family_rank("Noto Serif CJK SC"));
        assert!(family_rank("Noto Sans CJK SC") < family_rank("Noto Sans CJK TC"));
    }

    #[test]
    fn bundled_arial_does_not_count_as_cjk() {
        assert!(!has_cjk_glyphs(include_bytes!("../../assets/Arial.ttf")));
    }

    #[test]
    fn bundled_fallback_uses_glyf_outlines_and_is_not_a_solid_box() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/wqy-microhei.ttc");
        let bytes = std::fs::read(&path).expect("bundled CJK fallback");
        assert!(has_cjk_glyphs(&bytes));
        assert!(!has_cff2_outlines(&bytes));

        let font = fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
            .unwrap();
        for c in "vwWMm中".chars() {
            let (_, bitmap) = font.rasterize(c, 24.0);
            let filled = bitmap.iter().filter(|&&coverage| coverage > 0).count();
            assert!(filled > 0, "{c:?} is blank");
            assert!(filled < bitmap.len(), "{c:?} rasterized as a solid box");
        }
    }

    #[test]
    fn macroquad_font_skips_cff2_collections() {
        if let Some(bytes) = macroquad_bytes() {
            assert!(!has_cff2_outlines(&bytes));
        }
    }
}
