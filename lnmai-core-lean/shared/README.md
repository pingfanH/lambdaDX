# Shared Rust FFI Sources

This directory contains the single-source Rust implementation files shared by:

- the root crate in `src/`
- the embedded example bindings in `lnmai-core/bindings/rust/`

Files:

- `rust_ffi_api.rs` — typed helpers around the string-based parse/build/step APIs
- `rust_ffi_raw.rs` — raw Lean/FFI symbol declarations and runtime init
- `rust_ffi_session.rs` — typestate session wrapper and JSON envelope helpers
- `rust_ffi_types.rs` — typed Rust mirrors of Lean JSON payloads

Guideline:

- when refining Rust-side FFI behavior or payload schemas, update these shared files first
- keep `src/` and `lnmai-core/bindings/rust/` as thin include-based entrypoints

Current slide head/body schema notes:

- lowered charts now expose explicit `slideHeads` plus slide-body `slides`
- ordinary slides lower to head + body; no-head singleton slides and connected child parts lower to body only
- lowered head/body pairs now carry explicit `logicalSlideId`
- lowered slide heads and slide bodies now use distinct `noteIndex` values while sharing `logicalSlideId`
- lowered slide bodies use `headTiming` as the body-side head anchor field in the current schema
- lowered slide bodies use segment-local slide break semantics for their `isBreak` field; slide heads keep head break semantics
- folded identical simultaneous slide bodies carry `multiple`; lowered slide heads remain explicit separate head notes
- Rust mirrors require `headTiming` for slide-body payloads
- normalized slide mirrors expose explicit `hasHeadNote` and `hasBody`; `isSlideNoHead` remains compatibility metadata rather than the long-term semantic authority

Current runtime score/input notes:

- `GameState` no longer exposes a core touch-mode flag; desktop-style outer-button-to-touch mapping belongs in the host input layer before calling the core
- touch, touch-hold body, and slide-body runtime input should be supplied as sensor input
- lowered touch and touch-hold chart mirrors preserve `sourceGroupId` / `sourceGroupIndex` / `sourceGroupSize` so typed Rust round-trips keep shared-group semantics
- `HoldNote.releaseIgnoreTime` is serialized separately from scored `playerReleaseTime`; Rust mirrors default missing older JSON to `0`
- `GameState.noteFastLateDisplay` and `GameState.breakFastLateDisplay` mirror the core fast/late counter policy
- `ScoreState.dxScore` mirrors the core loss-delta field; use `ScoreState::dx_score_remaining()` on the Rust mirror for achieved DX score
- `ScoreState::combo_state()` derives AP+/AP/FC+/FC from judge-count categories, matching the core result semantics
