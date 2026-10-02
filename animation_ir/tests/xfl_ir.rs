//! Phase 1 acceptance: real Animate assets parse into IR and round-trip JSON.

use std::path::{Path, PathBuf};

use animation_ir::{AnimTarget, Document, Element};

fn asset(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("assets")
        .join(rel)
}

#[test]
fn ui_effect_project_converts() {
    let doc = Document::load(asset("ui")).expect("load assets/ui");
    // The named effect movie the runtime plays as `ui.get("tap_perfect")`.
    let anim = doc.animation("tap_perfect").expect("tap_perfect animation");
    assert!(anim.frames > 1, "tap_perfect should be multi-frame");
    assert_eq!(anim.target, AnimTarget::Symbol("tap_perfect".into()));
    // The Hex bitmap is referenced by the effect.
    assert!(doc.bitmaps.iter().any(|b| b.name == "Hex.png"));
    assert!(!doc.symbols.is_empty());
}

#[test]
fn player_ui_pages_convert() {
    let doc = Document::load(asset("player_ui")).expect("load assets/player_ui");
    let page = doc.animation("UI/page_start").expect("page_start animation");
    assert!(page.frames >= 1);
    // A representative widget symbol and a text-bearing page both survive.
    assert!(doc.symbol("UI/ui_btn_primary").is_some());
    let page_symbol = doc.symbol("UI/page_start").expect("page_start symbol");
    let has_text = page_symbol
        .timeline
        .layers
        .iter()
        .flat_map(|l| &l.frames)
        .flat_map(|f| &f.elements)
        .any(|e| matches!(e, Element::Text(_)));
    assert!(has_text, "page_start should contain static text");
}

#[test]
fn tap_hit_effect_converts() {
    let doc = Document::load(asset("effects/tap_hit")).expect("load assets/effects/tap_hit");
    assert!(doc.symbol("tap_hit").is_some());
    assert!(doc.animation("tap_hit").is_some());
}

#[test]
fn json_round_trips() {
    let doc = Document::load(asset("ui")).expect("load assets/ui");
    let json = doc.to_json();
    let back: Document = serde_json::from_str(&json).expect("deserialize snapshot");
    assert_eq!(doc, back);
}

#[test]
fn deterministic_symbol_order() {
    let a = Document::load(asset("ui")).unwrap();
    let b = Document::load(asset("ui")).unwrap();
    assert_eq!(a.to_json(), b.to_json());
    let names: Vec<&str> = a.symbols.iter().map(|s| s.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "symbols must be name-sorted");
}
