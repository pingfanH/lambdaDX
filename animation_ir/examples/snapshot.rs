//! Print IR coverage for an XFL directory or `.fla`.
//!
//! ```text
//! cargo run -p animation_ir --example snapshot -- assets/ui
//! cargo run -p animation_ir --example snapshot -- assets/player_ui --json
//! ```

use animation_ir::{Document, Element};

fn count_elements(doc: &Document) -> [usize; 5] {
    // [bitmap, instance, shape, group, text]
    let mut counts = [0usize; 5];
    let mut visit = |elements: &[Element]| {
        for e in elements {
            match e {
                Element::Bitmap { .. } => counts[0] += 1,
                Element::Instance { .. } => counts[1] += 1,
                Element::Shape { .. } => counts[2] += 1,
                Element::Group { members, .. } => {
                    counts[3] += 1;
                    // Group members are leaves in this count.
                    for m in members {
                        match m {
                            Element::Bitmap { .. } => counts[0] += 1,
                            Element::Instance { .. } => counts[1] += 1,
                            Element::Shape { .. } => counts[2] += 1,
                            Element::Text(_) => counts[4] += 1,
                            Element::Group { .. } => {}
                        }
                    }
                }
                Element::Text(_) => counts[4] += 1,
            }
        }
    };
    for layer in &doc.scene.layers {
        for frame in &layer.frames {
            visit(&frame.elements);
        }
    }
    for symbol in &doc.symbols {
        for layer in &symbol.timeline.layers {
            for frame in &layer.frames {
                visit(&frame.elements);
            }
        }
    }
    counts
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: snapshot <xfl-dir-or-fla> [--json]");
        std::process::exit(2);
    };
    let json = args.any(|a| a == "--json");

    let doc = match Document::load(&path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if json {
        println!("{}", doc.to_json_pretty());
        return;
    }

    let [bitmap, instance, shape, group, text] = count_elements(&doc);
    println!("{path}: {}x{} @ {}fps", doc.width, doc.height, doc.frame_rate);
    println!("  extensions: width={} height={}", doc.width, doc.height);
    println!("  symbols:    {}", doc.symbols.len());
    println!("  bitmaps:    {}", doc.bitmaps.len());
    println!("  animations: {}", doc.animations.len());
    println!("  elements:   {shape} shape, {bitmap} bitmap, {instance} instance, {group} group, {text} text");
}
