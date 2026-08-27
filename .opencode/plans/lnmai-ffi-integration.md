# lnmai-core-ffi Integration Plan

## Goal
- Remove auto-disappearance of notes at judgment point
- Remove auto-playback sounds  
- Connect to lnmai-core-ffi for real input→judgment
- Show good/great/perfect text on corresponding lanes

## Files to modify

### 1. Cargo.toml
- Uncomment: `lnmai-core-ffi = {path = "./lnmai-core-ffi"}`

### 2. src/player/state.rs
- Add `use lnmai_core::session::{self, Session, Empty, Loaded, initialize_runtime};`
- Add `JudgeText` type ({zone, grade, until})
- Add fields: lnmai_session, lnmai_initialized, judge_texts, lnmai_input_events
- Add methods: init_lnmai, create_lnmai_session, destroy_lnmai_session, advance_lnmai_frame, tick_judge_texts
- Modify toggle_play/toggle_replay to manage session lifecycle
- Empty update_playback/service_hit_sounds

### 3. src/player/input.rs
- Add zone mapping helpers (lane→"K1".."K8", PadZone→"A1".."E8")
- Add handle_lane_input_lnmai to buffer buttonClick/sensorClick events in Playing mode
- Map keyboard 1-8→buttonClick, T→sensorClick(C), touch→sensorClick

### 4. src/player/player.rs
- Call app.init_lnmai() at startup
- Replace update_playback() + service_hit_sounds() with app.advance_lnmai_frame()
- Add app.tick_judge_texts()

### 5. src/player/ui.rs
- Remove A-zone disappear checks (lines ~315-322)
- Add judgment text rendering at lane positions with color coding
