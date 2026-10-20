# LambdaDX Effect Preview

Run from the repository root without the gameplay backend:

```sh
cargo run --no-default-features --features backend-none --bin lambdadx_effect_preview
```

Drag the canvas with the left mouse button to pan. The wheel zooms around the
cursor. Left/right selects an effect, Space pauses, R replays, and 0 resets the
view. The toolbar selects Good (green), Great (pink), Perfect (orange), Critical
Perfect (gold/yellow), or Raw (all unfiltered systems with authored start colors).
Critical Perfect is the default, preserving the replay's original gold appearance.
Use --judgment good, great, perfect, critical-perfect (or cp), or raw.

Perfect, Great, and Good tint RGBs are sampled from the corresponding source
UI_GAM judgment sprites: (255,157,0), (255,114,178), and (47,202,76). Colored
particle atlases use their authored yellow, pink, and green rows. Ordinary
Perfect shares the Perfect geometry and yellow atlas row with Critical Perfect,
but converts the yellow component to orange without tinting neutral highlights.
White-texture particles use the selected judgment tint directly.

The bottom panel supports progress scrubbing, exact seconds, 60 Hz frame steps,
looping, and playback speed presets from 0.1x to 2x (custom values 0.05x to 4x).
Scrubbing suspends the clock without changing play/pause state. Frame steps pause
playback. The bracket keys step backward/forward. Use --speed 0.25 to start slow.
Preview playback controls live in the separate lambdadx_preview_controls.rs module.

A deterministic capture exits after writing the PNG:

```sh
cargo run --no-default-features --features backend-none --bin lambdadx_effect_preview -- --effect FX_GAM_Notes_Tap_01 --judgment perfect --time 0.12 --zoom 0.8 --screenshot /tmp/lambdadx-tap.png
```

## Runtime Modules

- `data.rs` parses the extracted definitions and samples scalar/color curves.
- `simulation.rs` samples property-keyed particles at absolute times.
- `particle.rs` uploads textures and submits particle quads to Macroquad.
- `shader.rs` implements rate, UVShift, screen, add2, and rainbow materials.
- `simulation_tests.rs` covers UV frames, square geometry, timing, and source parity.

Particle geometry uses the system matrix scale, not the texture aspect ratio.
Positions and UVs retain the source Y-up conventions until the final screen
conversion. Textures are vertically flipped on upload. UVShift uses explicit
judgment atlas-row selection; Raw retains the original vertex-RGB selector.
Material color remains a separate shader uniform.

Stretched billboards deliberately differ from the website: local Y, not X, is
the length axis aligned with velocity. This preserves the distinct four-star
and eight-star size curves in Hold Release. The eight-star group contracts into
streaks, regains a nearly square star shape, and fades later than the four-star
group. Verified against the user-provided 2026-10-19 16.21.36 reference recording.

The unstretched leading edge is retained; new length extends behind it rather
than equally in both directions. Centered stretching leaves a premature hole
between the four arms. Neither the authored particle sizes nor the size curves
are enlarged to hide this gap.

Particle start rotation and integrated angular velocity rotate the sprite in
local space before the anisotropic stretch, rather than being discarded or
rotating the entire elongated quad afterwards. The velocity alignment and
trailing offset stay independent of that spin. The authored property-keyed
random rotation is retained for other groups, so an unknown reference seed need
not give each of the eight stars exactly the same angle.

Sprite orientation is converted from world space into the velocity basis before
stretching; it is not reset to the radial direction at birth. No whole-effect
rotation is added. The four-star group also has a video-fitted radial correction:
an initial 0.3 local-unit offset eases to zero over 0.06 seconds, producing the
short contraction before the authored outward movement. This is an approximation
from the supplied clip, not recovered Unity metadata. The four stars use one
shared velocity-relative rotation phase, held during contraction and eased to
the texture's single tip facing inward before exit. Adjacent stars differ by
90 degrees, keeping opposite stars centrally symmetric. This phase is a fitted
group constraint rather than recovered Unity metadata.
The eight-star group's position and authored size curves are unchanged.

The complete four-star group is scaled to 90% around the effect origin, including
both vertex sizes and distances to the center. This leaves its symmetry, inward
exit orientation, and animation timing intact. The ring and eight stars are not
included in this group scale.

## Verification

The editor, player UI, and standalone PadView load the same runtime through
`player::effects`. `player::feedback` owns judgment metadata, sprite resources,
and active particle instances; the preview binary still only owns its controls.
PadView chooses Tap/EXTap/Break/Touch by note metadata, starts Hold on the core's
head event, and stops it on the tail before spawning Hold Release. Misses do not
spawn hit particles. Touch fireworks use Center Back.
Slide completion (including break slides) uses only its shaped `Judgement`
overlay, not Tap/Break hit particles. A slide's head is still a separate
Tap/Break event and retains its normal head feedback.

Sprite placement retains
the manifest's registration point and texture aspect ratio, including slide,
circle, and wifi judgment families. Missing particle resources leave judgment
sprites available and fall back to Animate/primitives for hit feedback.

Judgment art is rendered from the named `Judgement/...` symbols in
`assets/effects/lambdadx_replay_1600_master.fla` (unpacked project fallback).
The manifest only supplies registration metadata; symbol artwork and timeline
frames come from Flash. `tools/make_lambdadx_replay_xfl.py --judgments-only` adds
missing grade symbols without regenerating existing authored symbols.

All slide judgment families share `slide_just_scale` (default 1.0), including
the Flash symbols, break overlays, and classic fallback. The F1 panel exposes
only this size multiplier; registration and rotation follow the source geometry.
Legacy per-variant offsets, rotations, and flips are ignored when loading old
settings and omitted on the next save.

Successful touch fireworks layer `Sfx/touch_hanabi.wav` over the ordinary touch
sound. They obey the audio and hit-sound switches and stay silent on misses or
scrubs, independently of the visual-effects switch. `hanabi_fx_scale` (default
1.0) adjusts fireworks without changing `hit_fx_size`; configure it in F1 under
the sound/hit-effects section. The same multiplier applies to particle, Flash,
and procedural fallback drawing.

Note skins are independent: select Classic or LambdaDX under Settings > Note
skin in the editor, F1 panel, or the player's Display settings. The player and
editor menu persist the selection; the F1 panel uses its existing Save action.
Skin textures are cached and switches replace all slots together without
resetting notes, judgment state, or playback. Original design-00 source art is
retained under `assets/Skins/lambdadx/source`, with its slot manifest in skin.json.

`hit_fx` and `hide_slide_just` still respect saved settings. The shipped defaults
enable both layers, but an existing `~/.lambdadx/note_params.json` (or legacy
`output/note_params.json`) can disable them. No personal settings are overwritten.

A short real-PadView smoke export, isolated from saved settings:

```sh
MAI2_CACHE_DIR=/tmp/lambdadx-pad-qa MAI2_DATA_DIR=/tmp/lambdadx-pad-qa cargo run --bin lambda_dx_pad_preview -- tools/fixtures/pad_feedback/maidata.txt /dev/null --diff 1 --export-core --export-video /tmp/lambdadx-pad-feedback.mp4 --export-size 900x900 --export-fps 60 --export-duration 16 --export-no-audio
```

Particle placement respects world-aligned versus local-aligned billboards.
Rotating an emitter around the pad must not rotate world-aligned textures along
with it; velocity-aligned sprites retain their independent pre-stretch spin.

```sh
cargo test --no-default-features --features backend-none --bin lambdadx_effect_preview
```

The optional source comparison requires Node.js and a local copy of the original
renderer bundle from
https://maiwiki.moe/_app/immutable/chunks/ihKeLN9J.js . It evaluates the original
CPU renderer and compares particle corners, UVs, and colors across all 29 effects,
five judgment modes, and seven times. The source caller's color override is set
to each explicit judgment tint (the website itself only supplied yellow).
Position tolerance is 0.03 source units;
UV/color tolerance is 0.0001. For stretched billboards it compares centers,
UVs, and colors (undoing the trailing offset for the center comparison);
The four-star group scale and video-fitted radial offset are also removed for
source comparison. Corrected stretch geometry has separate regression tests based
on the reference recording. This is not a GPU pixel-equivalence test.

```sh
LAMBDADX_REFERENCE_BUNDLE=/tmp/lambdadx-player.js cargo test --no-default-features --features backend-none --bin lambdadx_effect_preview source_reference_all_effects -- --ignored --nocapture
```
