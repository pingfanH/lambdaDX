# Mai2Chart macroquad local demo

## Run

```bash
cargo run --manifest-path demo/macroquad_sim/Cargo.toml
```

## Controls

- Top-right touch buttons (keyboard-free): `Play`, `Record`, `Save`, `Load`, `Clear`, `Audio`, `Rec-/Rec+`, `Play-/Play+`, `PadOnly`, `MobileUI`
- `Space`: play / stop
- `R`: start / stop recording
- `1..8`: record tap lanes
- `T`: record touch lane
- Mouse click / touch on pad: record by pad hit area (`1~8` ring, `T` center)
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
