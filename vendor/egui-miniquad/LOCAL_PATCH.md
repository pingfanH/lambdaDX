# Local Rendering Patch

Based on egui-miniquad 0.16.0, upstream commit
`2c6528c6064003e0085dce323619201e15046b68` in
`https://github.com/not-fl3/egui-miniquad`. The original MIT and Apache-2.0
licenses are included. `src/lib.rs` and `src/input.rs` are unchanged; the
manifest omits upstream examples and development dependencies.

## Buffer Reuse

The upstream painter uploads every mesh into one vertex/index buffer pair,
immediately draws it, then overwrites that pair for the next mesh. On the
macOS OpenGL backend, `glBufferSubData` repeatedly waits for preceding draws
to finish. Textured notes and multiple clipped panes expose this severely.

`src/painter.rs` instead uses one buffer pair per mesh batch, with three
rotating frame pools. Buffers are reused after warmup and only grow when
needed, with power-of-two capacities. Each pool retains its high-water batch
count. Texture selection, clipping, shader, mesh order, and u16 splitting
remain unchanged. No new rendering backend or lower-resolution mode is used.

The root Cargo patch applies to both direct and transitive egui-miniquad
users so the preview and existing editor/player share the same fix.

## Validation

Measured on macOS, 1500x940 logical window with HiDPI, optimized dev build:

| Layout | Mean Frame | Mean UI | Mean Draw |
| --- | --- | --- | --- |
| Chart, upstream (two runs) | 71.3-92.5ms | 0.61-0.63ms | 66.2-84.9ms |
| Chart, patched | 16.5ms | 0.49ms | 1.14ms |
| Autoplay, patched | 16.7ms | 0.40ms | 0.83ms |

Measurements exclude 20 warmup frames and cover 90-120 subsequent frames.
Native sampling identified GPU waits during `buffer_update`; only the
painter buffer strategy changed between measurements. Temporary frame
instrumentation was removed after comparison.

The preview was built and native rendering inspected. Existing editor/player
targets were checked with `backend-none`; no gameplay behavior was changed
or exercised. Interactive feel and other graphics drivers require user QA.
