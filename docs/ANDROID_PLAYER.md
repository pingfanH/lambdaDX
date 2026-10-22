# Android Player

Build the standalone ARM64 player with its offline assets:

```sh
python3 tools/build_android_player.py --debug
```

Omit `--debug` for a release build. The signed output is
`target/android/lambda_dx_player_ui.apk`.

## Audio Levels

Settings > Audio has independent music, hit-sound and answer-cue volume
sliders (0-100%). They default to 100% for existing configurations. Hit
volume covers tap, hold, slide, touch, break and firework sounds; answer
volume only controls `Sfx/answer.wav`. The existing sound toggles remain
independent of these levels.

Music volume updates during playback without restarting or seeking the
song. Save Settings, Done and Back persist the levels in the shared
`note_params.json` override. The editor and pad preview expose the same
levels in the F1 sound/effects section.

## Game Classification

`Cargo.toml` sets both application attributes:

```xml
android:appCategory="game"
android:isGame="true"
```

The build checks the final signed APK with Android SDK `aapt dump badging`
and fails if the `application-isGame` marker is absent. It does not rely
only on the Cargo configuration or the generated manifest.

### On-Device Findings (2026-10-22)

The connected PJA110 device's installed APK contains both attributes,
matching the local APK. `apkanalyzer manifest print` reports `appCategory=0`
(game) and `isGame=true`, and `aapt dump badging` reports `application-isGame`.
However, `adb shell cmd game ext game_list` does not list
`io.lambdadx.lambda_dx_player_ui` in the Oplus game service.

This distinguishes the APK's declared category from inclusion in this
device's game service. Changing the same manifest flags again does not
establish that inclusion. Check the device's Games app for an option to
add LambdaDX Player manually. Do not spoof another game's package name
or clear application data as a classification workaround.

The Games launcher activity rejected an ADB launch because it requires
`oppo.permission.OPPO_COMPONENT_SAFE`. The manual-add screen could not be
verified through the normal launcher during this check. Game-center
inclusion remains unverified; a passing APK check must not be reported
as proof that the device's game center has accepted the app.
