//! Dump the structure of an XFL directory or `.fla` file.
//!
//! ```text
//! cargo run --example xfl_dump -- <path-to-xfl-dir-or-fla> [ANIM] [FRAME]
//! ```

use macroanimate::xfl::{PartContent, XflAtlas};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: xfl_dump <path> [ANIM] [FRAME]");
        std::process::exit(2);
    };
    let atlas = match XflAtlas::load(path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    println!(
        "size {}x{} @ {}fps",
        atlas.width, atlas.height, atlas.frame_rate
    );
    println!("bitmaps: {}", atlas.bitmaps.len());
    for (name, b) in &atlas.bitmaps {
        println!(
            "  {name} -> {} ({} bytes)",
            b.path,
            b.bytes.as_ref().map(Vec::len).unwrap_or(0)
        );
    }
    println!("symbols: {}", atlas.symbols.len());
    for (name, (ty, tl)) in &atlas.symbols {
        println!("  {name} [{ty:?}] {} layers, {} frames", tl.layers.len(), tl.frame_count());
    }

    let mut anims: Vec<&String> = atlas.animations.keys().collect();
    anims.sort();
    println!("animations: {anims:?}");

    let anim = args.get(1).cloned().unwrap_or_else(|| "Scene 1".into());
    let frame: usize = args.get(2).and_then(|f| f.parse().ok()).unwrap_or(0);
    let parts = atlas.parts(&anim, frame);
    println!("parts of `{anim}` @ {frame}: {}", parts.len());
    for (i, p) in parts.iter().enumerate() {
        let kind = match &p.content {
            PartContent::Bitmap(n) => format!("bitmap {n}"),
            PartContent::Vector(v) => format!("vector {} paths", v.len()),
            PartContent::Text(t) => format!("text {:?} @{:.0}px", t.text, t.size),
            PartContent::NineSlice { parts, .. } => format!("slice {} parts", parts.len()),
        };
        let m = p.matrix;
        println!(
            "  #{i} {kind} mx=[{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}]",
            m[0], m[1], m[2], m[3], m[4], m[5]
        );
    }
}
