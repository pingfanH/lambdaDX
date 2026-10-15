# Editor Autoplay Workspace

Updated: 2026-10-15. Primary means Command on macOS and Ctrl elsewhere.

## Workspace

Use the Chart / Autoplay tabs in the top toolbar to switch workspaces without
opening another window. Entering Autoplay finishes template isolation and
uses simulated-touch playback unless an Autoplay mode is already active.

The top contains the pad preview and event inspector. The bottom is a
multilayer horizontal timeline with four reusable lanes by default, not a
separate row for each note. Event groups are ordered by their start time and
placed in the first available lane. Sequential single notes reuse lane 1;
simultaneous notes and overlapping Hold/Slide spans use subsequent lanes.
The base lane count is adjustable from 1 to 32, and additional lanes appear
automatically when overlaps require them. Assignment stays fixed during an
event edit and is recomputed after confirmation.

Each group retains its core-owned input part: an ordinary note, star head or
slide body segment. Same-head slide trails retain separate ownership even
when they share a reusable lane at different times. Group/event tooltips and
the inspector show the full Note ID; template group labels use a shortened ID.

Wi-Fi currently has one body group containing its sensor events, matching the
Lean core's ownership. Separate branch groups are only possible when the core
provides separate track ownership. Input ownership is independent of the
display lane and does not change when groups are repacked.

## Event Editing

- Click a group or event to inspect it. Click empty track space, press Escape,
  or use the inspector's deselect button to clear both event and group selection.
  The group and event lists also have an empty selection.
- Drag a marker horizontally to change its time. Release confirms one undo
  step, including release outside the track area. Escape cancels the draft.
- Edit an event's time in seconds, input action and button/sensor zone in the
  inspector. Time fields retain microsecond precision.
- New-event zone and action controls are independent of the inspected event.
  Add inserts that input at the red playhead in the explicitly selected group,
  without selecting the inserted event. Choose an owner group before adding.
- Delete/Backspace or the inspector's delete button removes the selected event.
- Reset restores the selected group's generated default events. It is also in
  the group/event context menu. Deleting every event instead disables that
  group; a dimmed span remains available for selecting and resetting it.
- Primary+Z and Primary+Shift+Z undo/redo confirmed edits. These commands are
  also available in the Edit menu.

Markers distinguish clicks, press and release events. Overlapping inputs
within a group use compact subrows; presses and releases have separate hit
areas even at identical timestamps. Only matching sensor/button endpoints
are connected. Subrow placement stays fixed during a drag, without changing
event times or ownership. A small colored dot in a group's clip
marks authored events. An unmatched press occupies its lane until released.

## Navigation And Preview

The Note ID filter matches full IDs and group labels without changing lane
assignment. Lanes scroll vertically, while the start-time field moves the
visible time window. Primary+wheel and pinch zoom the horizontal scale;
the zoom slider uses the same red-playhead anchor. Zoom centers the red line
in the event plot, including when zooming out exposes time before zero.
Current also centers the playback position at the current scale. Playing and
recording keep the red line centered using the actual plot width and zoom.
While paused, manual panning remains available between zoom operations without
residual smoothed scrolling moving a newly centered view. The ruler shows chart
measures, including BPM changes.

Chart and Autoplay share a 10-4800 px/s zoom range. Lower values show more
time; higher values spread events farther apart. The initial scale is
360 px/s, three times the previous 120 px/s. Switching workspaces preserves
the scale without imposing a smaller Autoplay zoom limit.

Click or drag the ruler to seek. Event dragging and ruler seeking respect the
shared subdivision when snapping is enabled; snapping can be disabled for
free timing. The selected group's Jump button seeks to its note head.

Space, or the configured Play binding, toggles playback. Text fields, IME
composition and modal windows retain keyboard ownership. The shortcut help
shows commands for the active workspace.

The inspector offers simulated-touch and direct-core playback. Both use the
authored event sequence, including restoration of active inputs after seek.
Confirmed changes rebuild the core in the background; event editing is
temporarily disabled until the matching core is ready.

The Inner-screen switch is available in the Autoplay inspector and in
Settings > Playback. It is on by default and maps generated K1-K8 clicks,
presses and releases to the corresponding A1-A8 sensors. Turning it off keeps
the core's button inputs. Display, simulated playback, direct-core playback
and seek use the same configured events. Changing the switch restores the
current position and held inputs using the existing core session; it does
not reload the chart or restart audio. Edited groups and external replay
files keep their explicitly chosen inputs. Save Settings persists the switch.

## Storage

Only edited groups are stored in native JSON's autoplay_overrides. Ownership
uses the stable Note ID, role, slide, segment and optional core track index.
Event times are relative microseconds from the owning note head, so moving a
note or template instance moves its authored sequence with it.

The generated plan remains the default. Reset removes an override; an empty
override explicitly disables its part. Simai export contains chart notes,
not authored input events. Save As JSON to retain both; see
[Editor Chart Files](EDITOR_FILES.md).
