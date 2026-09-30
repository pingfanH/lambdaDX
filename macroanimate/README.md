<img align="right" width="300" src="https://raw.githubusercontent.com/FunkinRustUp/macroanimate/refs/heads/main/logo.gif" alt="MacroAnimate logo" />

<h1 align="center">MacroAnimate</h1>

<br/>

Adobe Animate **sparrow** and **texture atlas** parser and renderer for Macroquad.

## Features

- **Lightweight Sparrow Parser**: Streamlined layout parsing using raw string scanning instead of heavy crates.
- **Hierarchical Rig Engine**: On the go evaluation of multi-layered nested asset transformations.
- **Optimized Vertex Deforms**: Custom quad arrays mapped directly to Macroquad's low-level drawing APIs.

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
macroquad = "0.4.15"
macroanimate = "0.2.1"
```

## Quick Start

### 1. Rendering flat Sparrow frames

```rs
use macroquad::prelude::*;
use macroanimate::{parse_sparrow, SparrowFrame};

#[macroquad::main("Sparrow Demo")]
async fn main() {
    let texture = load_texture("assets/character.png").await.unwrap();
    let xml_data = std::fs::read_to_string("assets/character.xml").unwrap();

    // Parse frames with a matching prefix
    let frames = parse_sparrow(&xml_data, "idle");
    let mut current_frame = 0;

    loop {
        clear_background(BLACK);

        if let Some(f) = frames.get(current_frame) {
            draw_texture_ex(
                &texture,
                100.0 - f.frame_x,
                150.0 - f.frame_y,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(f.x, f.y, f.width, f.height)),
                    dest_size: Some(vec2(f.width, f.height)),
                    ..Default::default()
                },
            );
        }

        // Cycle through frame index based on your game's timer loop
        next_frame().await
    }
}
```

### 2. Rendering Deformed Texture Atlases

Because Adobe Animate texture atlases rely heavily on complex matrix transformations (limb stretching, translation, and shearing), they cannot be rendered via traditional flat texture queries. This library handles this efficiently by mapping the structural limbs onto low-level hardware vertex quads via Macroquad's mesh pipeline.

```rs
use macroquad::prelude::*;
use macroanimate::{parse_texture_atlas, get_texture_parts, draw_part_mesh};

#[macroquad::main("Texture Atlas Demo")]
async fn main() {
    let texture = load_texture("assets/test/spritemap1.png").await.unwrap();
    let spritemap = std::fs::read_to_string("assets/test/spritemap1.json").unwrap();
    let animation = std::fs::read_to_string("assets/test/Animation.json").unwrap();

    let atlas = parse_texture_atlas(&spritemap, &animation);
    let mut frame_index = 0;

    loop {
        clear_background(BLACK);

        // Resolve matrix transformations for the current frame
        let parts = get_texture_parts(&atlas, "idle", frame_index);

        // Iterate and draw vertex meshes using the sheet metadata configurations
        for part in &parts {
            if let Some(sprite) = atlas.sprites.get(&part.sprite_name) {
                draw_part_mesh(
                    &texture,
                    sprite,
                    &part.matrix,
                    2044.0, // Texture sheet width
                    1923.0, // Texture sheet height
                    750.0,  // Target layout X offset
                    250.0,  // Target layout Y offset
                );
            }
        }

        next_frame().await
    }
}
```

### 3. Loading Animate XFL projects and `.fla` files directly

`XflAtlas::load` accepts either an unpacked **XFL** directory or a `.fla`
(ZIP) file. It parses `DOMDocument.xml`, every `LIBRARY/*.xml` symbol, the
timeline layers/frames, motion tweens (matrix interpolation by
rotation/scale/translation), nested symbol instances and bitmap assets. Solid
vector fills (`DOMShape`) are rendered too.

```rust
use macroanimate::xfl::{XflAtlas, draw_parts};
use macroquad::prelude::*;
use std::collections::HashMap;

#[macroquad::main("XFL Demo")]
async fn main() {
    let atlas = XflAtlas::load("assets/ui").expect("load XFL/FL");
    let mut textures: HashMap<String, Texture2D> = HashMap::new();
    for (name, bitmap) in &atlas.bitmaps {
        // `bytes` is populated for `.fla` sources; directories read from disk.
        let bytes = match &bitmap.bytes {
            Some(bytes) => bytes.clone(),
            None => std::fs::read(format!("assets/ui/{}", bitmap.path)).expect("bitmap"),
        };
        textures.insert(name.clone(), Texture2D::from_file_with_format(&bytes, None));
    }

    let mut frame = 0usize;
    loop {
        clear_background(BLACK);
        let parts = atlas.parts("Scene 1", frame);
        let origin = (
            screen_width() * 0.5 - atlas.width * 0.5,
            screen_height() * 0.5 - atlas.height * 0.5,
        );
        draw_parts(&parts, &textures, origin);

        frame += 1;
        next_frame().await
    }
}
```

There is also an `xfl_dump` example that prints a project's symbols, bitmaps,
animations and the draw parts for a given animation/frame:

```sh
cargo run --example xfl_dump -- path/to/project-or.fla [ANIM] [FRAME]
```

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Additional Info

MacroAnimate is mainly built for the FNF RustUp Engine, any features that will be supported will go through if the RustUp Engine needs any support for it.
