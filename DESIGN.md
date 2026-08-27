# LambdaDX Player Design System

## 0. Research Log

- Embedded refs: shortlisted Spotify, PlayStation, and Figma; picked `taste-skill` + Spotify because the player needs music-first scanning, strong album-art focus, and compact playback controls without copying Spotify branding.
- Lazyweb: 3 desktop queries, 3 screens viewed (Amazon Music album detail, Epidemic Sound track browser, myNoise settings) -> persistent song list, stable selected-song detail, bottom transport, and grouped settings controls.
- Interaction refs: inspected beui.dev `tabs`, `morphing-modal`, and `button` sources -> gliding selected state, contextual pause modal, immediate press feedback, and reduced-motion-safe fades.
- UI/UX database: queried desktop rhythm-game/music-player dark accessible guidance -> kept OLED contrast and visible focus, rejected the suggested purple-heavy palette and display font because they conflict with the product and available native font stack.
- Imagen drafts: skipped because the built-in image generation tool is unavailable in this session. Existing song artwork and the live game pad are the visual reference assets.

## 1. Atmosphere & Identity

LambdaDX Player is a focused arcade console: immediate, rhythmic, and legible at arm's length. The signature is a cyan timing line that appears in active controls, progress, and selected difficulty, paired with coral only for pause or destructive actions. Album artwork and the circular pad provide the visual energy; interface surfaces stay restrained.

Personas: a keyboard player selecting a chart quickly, a touch player operating at distance, and a chart author checking a song at reduced speed. The core task path is Start -> Song Select -> Difficulty -> Play -> Pause/Resume or Exit.

## 2. Color

| Role | Token | Value | Usage |
|---|---|---:|---|
| Canvas | `BG_VOID` | `#080B10` | App background |
| Surface primary | `BG_PANEL` | `#111720` | Main panels |
| Surface secondary | `BG_RAISED` | `#18212C` | Controls and selected rows |
| Surface hover | `BG_HOVER` | `#22303D` | Hovered controls |
| Scrim | `BG_SCRIM` | `rgba(3, 6, 10, .82)` | Pause/settings overlay |
| Text primary | `TEXT_PRIMARY` | `#F4F7FA` | Titles and values |
| Text secondary | `TEXT_SECONDARY` | `#A8B4C2` | Metadata |
| Text muted | `TEXT_MUTED` | `#687585` | Hints and disabled states |
| Accent primary | `ACCENT_CYAN` | `#35D7E8` | Primary actions, focus, progress |
| Accent hover | `ACCENT_CYAN_HOVER` | `#74E7F1` | Hovered primary action |
| Accent warm | `ACCENT_CORAL` | `#FF6B5F` | Pause, warnings, destructive actions |
| Success | `STATUS_SUCCESS` | `#69D391` | Ready/loaded state |
| Border | `BORDER` | `#2B3948` | Structural separators |

Rules: accent color is semantic, never decorative. Cyan means continue/select; coral means interrupt/leave. Color is never the only selected-state signal: weight, outline, or label also changes.

## 3. Typography

| Level | Logical size | Weight | Usage |
|---|---:|---:|---|
| Display | 44 | 700 | Start screen product name |
| H1 | 32 | 700 | Page title / pause title |
| H2 | 24 | 650 | Selected song |
| H3 | 18 | 600 | Section heading |
| Body | 15 | 400 | Default labels |
| Body small | 13 | 400 | Metadata and descriptions |
| Caption | 11 | 600 | Overlines and key hints |

Font stack: system sans with PingFang/Hiragino/Noto CJK first where available, then bundled Arial and platform sans. No serif or decorative type. Text tracks at zero; labels use weight and case for hierarchy. CJK titles remain on one line with ellipsis rather than awkward character-level wrapping.

## 4. Spacing & Layout

Base unit: 4 px. Tokens: `XS=4`, `SM=8`, `MD=12`, `LG=16`, `XL=24`, `XXL=32`, `PAGE=48` logical pixels.

- Desktop content maximum: 1180 logical pixels centered in the viewport.
- Start: unframed two-column composition, copy/actions left and live pad signal right.
- Song Select: list-detail layout; list owns vertical scrolling, detail remains stable.
- Settings: fixed category rail and one scrolling content panel. Narrow layouts collapse to a top category row.
- Gameplay: the pad owns the canvas. A compact top HUD overlays without changing pad geometry.
- Pause: centered modal over a strong scrim, with background context still recognizable.
- At widths below 760 logical pixels, list-detail and settings become one column. Primary actions remain at least 48 logical pixels high.

## 5. Components

### Command Button
- Structure: label, optional key hint, stable minimum height.
- Variants: primary cyan, secondary raised, quiet, danger coral.
- States: default, hover, pressed, keyboard focus, disabled.
- Motion: 120 ms tint/opacity; pressed state darkens without changing layout bounds.

### Segmented Selector
- Structure: one container with mutually exclusive labelled segments.
- Variants: difficulty, playback speed, settings category.
- States: selected uses cyan fill/outline plus stronger text; hover and focus remain visible.
- Motion: selected indicator retargets immediately; reduced-motion path is a direct color change.

### Song Row
- Structure: rank/thumbnail surrogate, title/artist, BPM/note metadata, selected marker.
- States: default, hover, selected, unavailable, error.
- Layout: list row with fixed metadata columns; title truncates cleanly.

### Setting Row
- Structure: label and supporting copy left, semantic control right.
- Controls: toggle, slider, segmented choice.
- Accessibility: control label includes current value; touch target is at least 48 px.

### Game HUD
- Structure: back/pause action, song and difficulty, elapsed time/progress, status chips.
- Layout: fixed top overlay; pad area remains the sole interactive gameplay region below it.

### Pause Modal
- Structure: status overline, title, current song/time, Resume/Restart/Settings/Exit commands.
- States: enter, open, settings child view, close.
- Motion: 220 ms opacity and small vertical settle; no continuous animation.

## 6. Motion & Interaction

| Token | Duration | Use |
|---|---:|---|
| `MOTION_MICRO` | 120 ms | Hover, press, toggle |
| `MOTION_STANDARD` | 220 ms | Selection and modal enter/exit |
| `MOTION_EMPHASIS` | 360 ms | Start-to-library page reveal |

Only opacity and visual tint are animated in egui; component bounds remain stable. Escape goes back or opens/closes pause, Space pauses/resumes only during gameplay, Enter activates the focused primary action, and arrow keys move through songs/difficulties. Reduced motion uses direct state changes; native egui currently exposes no portable OS reduce-motion query, recorded as debt below.

## 7. Depth & Surface

Strategy: mixed tonal shift plus one-pixel borders. Main hierarchy comes from `BG_VOID` -> `BG_PANEL` -> `BG_RAISED`; borders define controls and modal edges. No drop shadows, blur glass, gradients, or decorative floating cards. Album art and pad rendering supply depth.

## 8. Accessibility Constraints & Accepted Debt

Constraints: WCAG 2.2 AA contrast target; keyboard path for every flow; minimum 48 px primary target; selection never relies on color alone; no flashing; CJK fonts and truncation must be visually checked; pause overlay must preserve a clear Resume first action.

| Item | Location | Why accepted | Owner / Exit |
|---|---|---|---|
| Screen-reader semantics are limited | Native egui player | egui-macroquad does not expose a complete platform accessibility bridge in the current stack | Revisit when the dependency supports AccessKit integration |
| OS reduced-motion preference is not detectable | Native egui player | No portable setting is exposed by the current stack | Add a user-facing reduced-motion toggle if meaningful motion expands |

