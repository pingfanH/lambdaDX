# LambdaDX demo

## Clone

Clone normally:

```bash
git clone git@github.com:pingfanH/lambdaDX.git
```

## Nix shell

Enter the development shell with Rust, `elan`, Lean build tooling, and Linux native libraries:

```bash
nix-shell
```

Or with flakes enabled:

```bash
nix develop
```

One-command build from flake-pinned dependency sync through Lean FFI compile:

```bash
nix run .
```

That defaults to:

```bash
cargo build --bin lambda_dx_player
```

To pass custom Cargo build arguments through the Nix pipeline:

```bash
nix run . -- --bins
nix run . -- --release --bin lambda_dx_editor
```

Nix builds are written to `target/nix`. Each `nix run` stages a workspace under
`target/nix/workspace/source`, replacing `lnmai-core-rs`, `lnmai-core-ffi`,
`lnmai-core`, and `maisimai` with the revisions pinned in `flake.lock`, so
rebuilds only happen when the staged source or locked dependency revisions
change.

Build all binaries inside the shell:

```bash
cargo build --bins
```

## Run

```bash
cargo run --manifest-path demo/macroquad_sim/Cargo.toml
```

Run the player through the Nix pipeline:

```bash
nix run .#player
```

Pass runtime arguments through after `--`:

```bash
nix run .#player -- --help
```

If you build through Nix, prefer artifacts under `target/nix/` rather than
`target/debug/`.

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
bash demo/macroquad_sim/scripts/run_desktop.sh
```

Desktop (force mobile-ui for touch-only workflow):

```bash
bash demo/macroquad_sim/scripts/run_desktop_mobile.sh
```

Android (requires Android SDK + `cargo-apk`):

```bash
bash demo/macroquad_sim/scripts/run_android_debug.sh
```

Note:

- Script uses `adb install --no-incremental -r` to avoid `INSTALL_PARSE_FAILED_NOT_APK` seen on some devices/emulators with incremental install.
