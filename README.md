# LambdaDX demo

## Clone

Clone normally:

```bash
git clone git@github.com:pingfanH/lambdaDX.git
```

## Nix Workflow

Enter the development shell from the flake:

```bash
nix develop
```

Build the player through the pinned flake graph:

```bash
nix build .#player
```

Run the player:

```bash
nix run .#player
```

`nix run .` uses the same player app. The Lean FFI artifacts are provided by the
`lnmai-core` flake input and consumed through `LNMAI_CORE_ARTIFACTS`; raw Cargo
builds are intentionally not the supported path for the FFI-enabled player.

The flake stages `lnmai-core-rs`, `lnmai-core-ffi`, `lnmai-core`, and
`maisimai` from the revisions pinned in `flake.lock`, then builds the Rust
player as a Nix package. Rebuilds happen when the staged source, Cargo lock, or
locked input revisions change.

## Flake input update workflow

This repo uses a flake-pinned dependency chain:

- `lambdaDX` -> `lnmai-core-rs`, `lnmai-core-ffi`, `lnmai-core`, `maisimai`
- `lnmai-core-rs` -> `lnmai-core-ffi`, `lnmai-core`, `maisimai`
- `lnmai-core-ffi` -> `lnmai-core`

If you modify one of those input repos and already pushed the change to GitHub,
do not edit the staged workspace under `target/nix/workspace/source`. Update
the nearest parent flake lock that consumes that repo, commit that lock change,
push it, and then continue outward to the next parent.

Preferred command form:

```bash
nix flake update <input-name>
```

For multiple inputs:

```bash
nix flake update <input-a> <input-b>
```

Examples for the current repo chain:

If `lnmai-core` changed and is already pushed:

```bash
cd lnmai-core-rs/lnmai-core-ffi
nix flake update lnmai-core
git add flake.lock lnmai-core
git commit -m "Update lnmai-core"
git push

cd ../
nix flake update lnmai-core lnmai-core-ffi
git add flake.lock lnmai-core-ffi
git commit -m "Update lnmai-core chain"
git push

cd ../
nix flake update lnmai-core lnmai-core-ffi lnmai-core-rs
git add flake.lock lnmai-core-rs
git commit -m "Update lnmai-core chain"
git push
```

If `lnmai-core-ffi` changed and is already pushed:

```bash
cd lnmai-core-rs
nix flake update lnmai-core-ffi
git add flake.lock lnmai-core-ffi
git commit -m "Update lnmai-core-ffi"
git push

cd ../
nix flake update lnmai-core-ffi lnmai-core-rs
git add flake.lock lnmai-core-rs
git commit -m "Update lnmai-core-ffi chain"
git push
```

If `lnmai-core-rs` changed and is already pushed:

```bash
nix flake update lnmai-core-rs
git add flake.lock lnmai-core-rs
git commit -m "Update lnmai-core-rs"
git push
```

If `maisimai` changed and is already pushed:

```bash
nix flake update maisimai
git add flake.lock
git commit -m "Update maisimai"
git push
```

After a lock refresh, rebuild through the flake entrypoint:

```bash
nix build .#player
```

## Run

Run the player through the Nix pipeline:

```bash
nix run .#player
```

Pass runtime arguments through after `--`:

```bash
nix run .#player -- --help
```

Nix build outputs are linked through `./result` when using `nix build`.

## Controls

- Top-right touch buttons (keyboard-free): `Play`, `Record`, `Save`, `Load`, `Clear`, `Audio`, `Rec-/Rec+`, `Play-/Play+`, `PadOnly`, `MobileUI`
- `Space`: play / stop
- `R`: start / stop recording
- `1..8`: record tap lanes
- `T`: record touch lane
- Mouse click / touch on pad: record by pad hit area (`1~8` ring, `T` center)
- In `player`, multi-touch now feeds both touch clicks and held sensor state to `lnmai`, so simultaneous touches map to sensor status correctly.
- `[` / `]`: record speed `0.1x ~ 3.0x`
- `-` / `=`: playback speed `0.1x ~ 3.0x`
- `P`: pad-only view
- `M`: mobile UI mode (hide timeline)
- `A`: audio on/off
- `S`: save recording document and latest chart
- `L`: load latest saved chart
- `C`: clear current recording hits

## Output files

Saved to:

- `demo/macroquad_sim/output/recording_<epoch_ms>.json`
- `demo/macroquad_sim/output/latest_chart.json`

## Audio

Optional audio file path (any one):

- `demo/macroquad_sim/assets/demo.ogg`
- `demo/macroquad_sim/assets/demo.mp3`
- `demo/macroquad_sim/assets/demo.wav`

Audio feature is enabled in `Cargo.toml` (`macroquad` with `features = ["audio"]`), so you should not see:
`warn: macroquad's "audio" feature disabled.`

Audio speed behavior:

- `assets/demo.wav` and `assets/demo.mp3` are both supported directly.
- Both are decoded to PCM, then runtime resampled, so audio speed follows record/playback speed (`0.1x ~ 3.0x`).
- Desktop scripts still auto-convert `demo.mp3` to `demo.wav` when `ffmpeg` is available, but conversion is now optional.

## Debug scripts

Desktop:

```bash
bash scripts/run_desktop.sh
```

Desktop (force mobile-ui for touch-only workflow):

```bash
bash scripts/run_desktop_mobile.sh
```

Android (requires Android SDK + `cargo-apk`):

```bash
bash demo/macroquad_sim/scripts/run_android_debug.sh
```

Note:

- Script uses `adb install --no-incremental -r` to avoid `INSTALL_PARSE_FAILED_NOT_APK` seen on some devices/emulators with incremental install.
