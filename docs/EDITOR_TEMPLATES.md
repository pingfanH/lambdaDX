# Editor Templates

Updated: 2026-10-15. The template workflow follows the original
Mai2Chart/demo/macroquad_sim editor.

## Library And Scene

Open the Template menu and enable the template library and Scene panel.

- Select notes and choose Create From Selection. The selected notes become a
  shared template definition, with an instance at their original position.
- New Template creates an empty definition and enters isolation immediately.
- Select a definition and use Insert to place an instance at the current
  timeline time, respecting the editor's subdivision and snapping setting.
- Edit or double-click a library entry to edit its definition from measure 1.
- The library context menu provides insertion, editing, renaming and deletion.
  Deleting a definition also deletes its instances; deleting an instance does
  not delete the definition.

Scene lists the main chart and its instances in time order. Clicking an
instance selects it and jumps to its anchor. Double-clicking enters isolation
at that instance's timeline position.

## Timeline Instances

Instances have blue bounds, corner markers and their template name. Their
expanded notes are visible in both the timeline and pad, but cannot be edited
as ordinary notes outside isolation.

- Click an instance's empty area to select it. Ordinary note heads overlapping
  its bounds remain clickable.
- Shift-drag an instance to move it along time; snapping uses the same grid as
  note editing. Release confirms one undo step; Escape cancels.
- Shift-click, or double-click, an instance to enter isolation.
- Shift-right-click deletes an instance. Delete/Backspace also works when an
  instance is selected and note selection/keyboard editing does not own input.

## Isolation

Definition isolation uses local measure 1 as its start. Instance isolation
uses the instance's anchor, so its notes remain aligned with the main chart.
The parent chart is faded and cannot be edited.

Use the Main breadcrumb or Return to Main to finish. Escape also returns when
no note placement, slide editing or keyboard interaction owns the key.
Switching to the Autoplay workspace finishes isolation first.

Changes update the shared definition and all its instances. The definition's
version advances when its notes change. Returning to Main groups the isolation
session into one parent undo step; ordinary note edits inside isolation have
their own local undo history. The parent's playback time and playing/paused
state are restored on return.

## Files And Autoplay

Native JSON preserves definitions, instances and authored Autoplay events.
Saving during isolation folds the edited definition into the full parent
document; it does not save only the isolated fragment.

Simai export expands instances into ordinary notes. It does not include the
template library or authored input events. Save As JSON is required to retain
those editor features; see [Editor Chart Files](EDITOR_FILES.md).

Autoplay events belong to individual expanded notes. Moving an instance keeps
its events aligned to the notes. Deleting an instance removes only its own
authored events, and undo restores them.
