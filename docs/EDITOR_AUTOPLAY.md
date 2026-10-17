# Editor Autoplay Workspace

Updated: 2026-10-17. Primary means Command on macOS and Ctrl elsewhere.

## Workspace

Use Chart / Autoplay / Combined in the top toolbar to switch workspaces
without opening another document. Each pane contains one editor, not a tab
group. Click a pane, or use F6 / Shift+F6, to move focus. Hover alone does not
redirect keyboard commands. Finish or cancel a draft before changing layouts.

The Autoplay preset puts Pad and Properties above a horizontal timeline.
Resize, split, swap or maximize panes using their headers; explicitly save
the layout from View > Layout to restore it next launch.
Pane view settings travel with that pane when it is moved or swapped.
The Chart, Autoplay and Combined presets keep separate pane settings, including
orientation, zoom, scrolling, follow and snapping. Chart and Autoplay have
independent subdivisions. Saving a workspace layout now saves these view
settings too; the document and playback clock remain shared.

The timeline shows one element per real owner: a note head, each Slide
segment and each standalone clip is its own row, styled like the clip boxes.
Elements share reusable lanes, so sequential actions sit on one lane and
overlapping ones occupy later lanes. The filter menu keeps the base lane
count from 1 to 32 and adds `按分组聚合`, which folds the parts of one Slide
or note back into a single box. Assignment stays fixed during edits, and
confirmation prefers existing lanes where they remain available.

Curve elements paint their path thumbnail, duration band and timing inside
the box; sensor elements paint their intervals there. An explicit sensor
expansion grows in the element's own lane instead of opening a second view
below the timeline. Tooltips and Properties expose the Note ID and owner.

Wi-Fi currently has one body group containing its sensor events, matching the
Lean core's ownership. Separate branch groups are only possible when the core
provides separate track ownership. Input ownership is independent of the
display lane and does not change when groups are repacked.

## Overview And Sensor Detail

- Click an element to select it without seeking. Curve elements show their
  path on Pad and stay untouched; sensor elements select their only interval
  directly. Drag the box body to move the element, or an inline interval
  body/endpoint to edit that interval alone.
- Drag the box's left grip to move its start, or its right grip to change its
  length. For a curve the right grip sets the duration, or the speed in
  constant-speed mode; for a sensor element it scales that element's event
  times. Each resize is one undo step and snaps like other time edits.
- Shift-click or Command-click adds or removes an element from the pick, and
  dragging empty lane space rubber-bands a time range across all lanes.
  Delete disables every picked note-owned element and removes standalone
  clips, all in one undo step; G and Shift+D transform the whole pick.
- Double-click a curve element or press E / Enter to edit its points and
  handles on Pad. For other elements these commands open the sensor
  expansion. The selection and drawing mode survive that switch.
- Matching Press/Release events form intervals within the same real owner
  and sensor. Drag the body to move both endpoints; drag an endpoint to
  change only that time. Ordering limits prevent crossing its paired end.
- Endpoint hit areas stay 20 logical points wide. Close or zero-duration
  endpoints are vertically separated without changing their timestamps,
  leaving a draggable body between them. Nearby intervals on the same
  sensor use separate subrows so their hit areas do not cover one another.
- Clicks and unmatched events remain individual markers. A zero-duration
  pair stays a pair, not a Click. Events from different owners never pair.
- G moves the selected clip, interval or event; Shift+D duplicates it and
  enters movement. Mouse motion, wheel and Left/Right adjust the draft;
  Shift+Left/Right uses a larger step. Enter or a click confirms; Esc cancels.
- A mouse drag confirms on release, including outside the timeline. Each
  confirmed drag or duplicate is one undo step. Confirmation never also
  creates a new event. Active edits temporarily suspend automatic following
  without turning off follow.
- Tab / Shift+Tab visits clips in overview/curve detail, or events/endpoints
  in sensor detail.
  Holding the key repeats navigation.

The header filter menu also offers the original raw-event view. It preserves
single-marker dragging and raw action editing, but does not provide clip
G/Shift+D transforms. Switching modes exits interval drawing.

## Drawing And Selection

- V selects; D enters drawing even without a selected clip. The main
  timeline header contains Select, Draw, Click/Hold and Add controls.
  Starting drawing or adding an event never forces sensor detail open.
- In selection mode, Click/Hold converts the selected event or interval;
  the highlighted type reflects that selection. Click to Hold preserves
  the exact start time and sensor and adds Release one subdivision later.
  Hold to Click retains the Press time and removes its paired Release,
  even when the Release endpoint is selected. Choosing the current type
  preserves its duration and adds no undo step. In drawing mode these
  controls only choose the type of future events, not the last-created one.
- Select an interval, then click a sensor in Pad to change its region. Both
  Press and Release move to the new sensor without changing their times.
  The eight K targets outside the ring select physical buttons; the Pad
  itself selects A/B/C/D/E sensors. Raw mode changes only the selected event.
- In drawing mode, choose Click or Hold. Clicking Pad adds that action at
  the red playhead; Hold starts one subdivision long.
  Clicking empty main-track space adds the chosen action at the snapped
  pointer time; dragging draws a Press/Release interval. With no selection
  when drawing starts, each action creates a standalone clip, even in an
  empty chart. Selecting a clip before pressing D instead appends to that
  explicit owner. The sensor is the last explicitly picked Pad sensor, not
  the display lane under the pointer. The new interval
  shows editable endpoints directly in the main track. Optional sensor
  detail supports the same click/drag actions using its row's sensor.
  Existing events remain selectable and selecting one exits drawing.
- In selection mode, blank space clears selection and exits drawing, without
  creating an event. Visible detail may remain open, but it is not a hidden
  owner for new events. Press D to create standalone clips from this state.
- Esc first cancels an active draft; otherwise it exits drawing, then clears
  selection, then closes detail on successive presses.
- Delete/Backspace removes the selected interval, including when only its
  Press or Release endpoint is selected. Raw mode deletes just one event. With a whole clip
  selected, it disables that clip's note-owned parts. Empty note overrides
  leave a dimmed span available for restoration; deleting the last event
  of a standalone clip removes that clip entirely.
- The clip context menu restores all parts in that clip. Properties restores
  only the real owner shown in its owner selector.
- Primary+Z / Primary+Shift+Z undo/redo confirmed document edits, also through
  Edit or the History pane. Layout operations are not document undo steps.

## Properties

Choose a real owner in Properties. Slide and saved-curve groups expose their
curve source, point editor and motion settings; switch to Sensor detail for
the baked inputs. For other groups, the normal view lists Sensor / Press /
Release columns. Edits stay within that owner even if its overview clip
contains multiple parts. Raw mode instead exposes each event's action, zone
and time. Times retain microsecond precision, and field drags produce one
undo step when finished.

Sensor labels select their interval for direct Pad editing and Click/Hold
conversion; no sensor dropdown is required. Enter drawing mode or clear
selection to choose a new-event type without converting an existing event.
Choose Click/Hold, then use the plus button to add at
the red playhead using the last explicitly picked sensor. Hold creates a Press/Release pair
one subdivision long; raw mode adds the chosen Click, Press or Release.
Selecting an existing event alone does not change the new-event sensor.
Clicking Pad without an event selected only sets the new sensor unless
drawing mode is active. Drawing and Add work without a selected owner;
they create standalone clips instead of modifying an unrelated note.

Properties also provides seek-to-note-head, sensor detail, owner reset and
selected-event deletion. Playback settings expose simulated touch, direct
core and the inner-screen mapping.

## Navigation And Preview

The Note ID filter matches IDs and labels without changing lane assignment.
Hold the middle button and move away from the press position to scroll time
in that direction. Distance controls speed; holding the pointer off-center
keeps scrolling, returning to the origin stops, and releasing ends the gesture.
Capture continues outside the pane. Shift+wheel and horizontal trackpad
scrolling also pan time without seeking; the ordinary vertical wheel scrolls lanes.
The Chart editor shares middle-button rate scrolling and Shift+wheel panning,
using up/down in the vertical layout and left/right in the horizontal layout.
When Chart follow is enabled, panning also seeks the shared playback clock;
with follow disabled it only moves the view.
The start-time
field is in the filter menu. Primary+wheel and pinch preserve the time under
the pointer; the header's numeric zoom field preserves the viewport center.
Zoom does not switch follow off. The ruler follows chart BPM changes, and
subdivision lines continue through overview, sensor detail and raw-event
tracks. Zooming out progressively thins the grid to coarser subdivisions
without changing the selected snap interval.

The header offers follow and return-to-playhead controls; fit-selection and
fit-song are in its menu and F3. With follow enabled, playback leaves the view
stationary while the red line is in the left half of the time plot. Only after
it reaches the midpoint does the view ease into scrolling over about 200 ms,
then track at playback speed with the red line centered. The threshold uses
the usable plot width, excluding track labels, at every layout size and zoom.
Manual browsing, fitting, zooming and editing leave the follow toggle enabled.
Pausing stops the transition immediately. During playback, active gestures
take priority and reset the transition; once they end, the same midpoint rule
resumes from the current view without snapping back. A playhead beyond the
midpoint, or offscreen to the left after a seek or loop wrap, is brought back
smoothly. Enabling follow alone does not center the view.
Return-to-playhead centers immediately at the existing scale and enables follow.
Automatic following only moves the view, never the playback clock.
Chart follow stays active while paused too: seeks update the view, and panning
moves both the view and clock. Pointer zoom preserves its anchor without seeking.

Chart and Autoplay have independent viewports and a shared playback clock.
Both use a 10-4800 px/s zoom range, initially 360 px/s. Lower values show more
time; higher values spread events farther apart. Switching workspaces keeps
each pane's scale and position. View > Layout > Save persists the current
workspace's pane settings across launches.

Chart has whole-song progress rails on both time-axis edges: left/right in
vertical layout and top/bottom in horizontal layout. Both rails cover the
entire song regardless of zoom. Click or drag to seek and bring that time
into view; the light span marks the visible range, red marks the playhead,
and yellow marks the loop. Releasing outside the pane ends the seek.

Click or drag the ruler to seek. Holding the locator beyond either plot edge
continues panning and seeking, even with the pointer stationary. Returning
inside stops edge panning; releasing ends the seek. Autoplay resumes midpoint
following only during playback with follow enabled.
Event dragging and ruler seeking respect the
Autoplay pane's subdivision when snapping is enabled; snapping can be disabled for
free timing. Properties' seek button jumps to the owning note head.

Shift-drag the Autoplay ruler or Chart overview to set a loop without seeking.
Drag either yellow boundary on the ruler to resize an existing loop. Dragging
elsewhere still seeks. A translucent yellow background marks the same range in
the chart, Simai text and all Autoplay lanes, including expanded sensor rows.
Release confirms even outside the ruler; Esc restores the previous range.
Playback provides the loop toggle, range fields and loop-selection command.
Starting playback inside the enabled loop continues there; starting outside
returns to its beginning. Reaching the end also returns to the beginning.
Loop-selection adds 0.25 seconds before and 0.5 seconds after the selection.
F3 also exposes loop-selection and clear-loop.

Space, or the configured Play binding, toggles playback. Text fields, IME
composition and modal windows retain keyboard ownership. The shortcut help
shows commands for the active workspace.

The inspector offers simulated-touch and direct-core playback. Both use the
authored event sequence, including restoration of active inputs after seek.
Confirmed changes rebuild the core in the background; event editing is
temporarily disabled until the matching core is ready. Timeline browsing,
zooming and ruler seeking remain available during that synchronization.

The Inner-screen switch is available in the Autoplay inspector and in
Settings > Playback. It is on by default and maps generated K1-K8 clicks,
presses and releases to the corresponding A1-A8 sensors. Turning it off keeps
the core's button inputs. Display, simulated playback, direct-core playback
and seek use the same configured events. Changing the switch restores the
current position and held inputs using the existing core session; it does
not reload the chart or restart audio. Edited groups and external replay
files keep their explicitly chosen inputs. Save Settings persists the switch.

## Pad Curves

- Select a Slide clip to see its paths directly, without first converting
  sensor events. Generated paths use the existing Slide rendering geometry
  and core segment start/duration, initially in Fixed duration mode without
  extra smoothing. Merely selecting or previewing a group leaves its original
  autoplay events untouched.
- Curve detail shows a path thumbnail and motion interval for each real
  owner/segment. Click a row or its path on Pad to choose that segment; the
  active path is brighter and the other paths remain visible. Properties
  also offers a segment selector. Double-click the row/path or use E / Enter
  from the timeline to edit the selected curve.
- Use the pen-tool button in the timeline header or Properties to create a
  standalone curve at the snapped playhead. It always adds a new clip, even
  when another group is selected; cancelling the draft removes only that new
  clip. Draw on Pad, then release to switch to point editing. Hand-drawn and
  Slide-derived curves use the same group and point editor. Keep Pad and
  Properties visible in the layout.
- Drag a control point to move it together with its two Bezier handles.
  Selecting a point reveals its handles; drag either handle to change the
  tangent. Aligned keeps their directions opposite while retaining their
  individual lengths; Free, or Alt-dragging a handle, makes them independent.
  Properties also provides exact point X/Y coordinates.
- Click a path segment to insert a point without changing its shape, or
  click elsewhere to append. Alt-click a point or Delete removes it. The
  pencil tool replaces the draft with a new freehand stroke. Auto smooth
  rebuilds all tangents; Straight segments rebuilds vector handles.
  The red marker previews the position at the current playback time.
- Constant speed keeps Speed unchanged: duration is path length divided by
  speed, so extending the path extends its duration. Speed uses Pad radii
  per second (R/s). Fixed duration keeps Duration unchanged: extending the
  path increases speed. Both advance by distance along the curve, not by
  control-point count. Switching modes initially preserves the motion time.
- Start sets absolute playback time. Touch radius is measured in Pad radii
  and determines which sensors the moving contact overlaps.
- Apply or Enter converts the path into paired sensor Press/Release events
  and commits one undo step. Esc/Cancel discards the draft without changing
  the document or core. Space can play the existing content during a draft;
  the draft itself does not reach the core until Apply. Other edit panes are
  locked until the draft ends.
- Apply retains curve-group selection rather than switching to sensor
  markers, including when editing began in raw-event view. Reopening the
  group restores its points and individual handles. Moving the entire event
  group in time retains its curve. Editing individual raw sensor events or
  interval endpoints detaches curve metadata; the edited events remain.
  Map Slide can deliberately replace those events with a fresh mapping.
  Restore default events removes both event and curve overrides.

Wi-Fi maps to straight legs from the head's ring point, matching the Pad
renderer: a per-track owner keeps the single leg for its branch, and an owner
that combines all three branches maps to the full fan of three legs through
the head. A chain containing Wi-Fi expands the fan in place and continues
from its middle leg, which is the endpoint the format declares for Wi-Fi.

Curve baking uses the loaded Pad sensor geometry and sends the resulting
events through the existing core input path. It does not guarantee Perfect
judgments: speed, start time, sensor coverage and the required sensor order
still matter.

## Remaining Design Targets

Whole-curve duplication is not yet distinct from raw-event duplication;
Shift+D duplicates inputs and detaches curve metadata from that owner.
Multi-clip marquee selection, transfer/copy to another owner, idle arrow-key
sensor navigation, real-time Pad recording and isolated playback of committed
content while a draft remains open are not implemented. See the
[interaction specification](EDITOR_INTERACTION_SPEC.md) for those proposals.

## Storage

Edited groups and standalone clips are stored in native JSON's autoplay_overrides.
Curve overrides additionally retain their control points, incoming/outgoing
Bezier handles and alignment mode, smoothing, timing mode, speed, duration
and contact radius alongside the baked events. Old JSON without curve
metadata remains supported; curves without explicit handles derive them
from their original smoothing setting. Note ownership
uses the stable Note ID, role, slide, segment and optional core track index.
Event times are relative microseconds from the owning note head, so moving a
note or template instance moves its authored sequence with it.

Standalone keys reserve role = 18446744073709551615; note_id is a separate
clip ID, not a chart note reference. Their event times are absolute
microseconds, and their playback groups have no runtime-note dependency.
They survive seeking and template cleanup without creating synthetic notes.

The generated plan remains the default. Reset removes an override; an empty
override explicitly disables its part. Simai export contains chart notes,
not authored input events. Save As JSON to retain both; see
[Editor Chart Files](EDITOR_FILES.md).
