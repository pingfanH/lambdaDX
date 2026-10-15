# Editor Chart Files

Updated: 2026-10-15. Primary means Command on macOS and Ctrl elsewhere.

## Commands

| Command | Shortcut | Result |
| --- | --- | --- |
| Open chart | Primary+O | Open a chart file or folder. A folder prefers maidata.txt, then chart.json. |
| Save | Primary+S | Update the file and difficulty actually loaded. |
| Save As | Primary+Shift+S | Write a new .txt or .json file and adopt it as the next Save target. |
| Export Simai | Primary+Shift+E | Write a .txt copy without changing the open source or its unsaved state. |

All four commands are in the File menu and F3 search. Reload uses the current
path and difficulty. Save requests Save As when there is no loaded source.
Native file dialogs currently require macOS; other platforms can open files
through the chart path field or MAI2_EDITOR_CHART.

## Formats

- Simai saves update the loaded inote slot, not the displayed rating or a pending
  difficulty switch. Other difficulties, custom headers and rating strings such
  as 14+ are retained. Title, artist, audio offset and initial BPM are written.
- Simai output retains BPM changes, rests, hi-speed changes, flags, zero-length
  holds, continuous slides and same-head trails. It uses the document's measure
  clock, not the preview core's temporary compilation clock.
- Native JSON retains editor IDs, floating-point times, per-note speeds and
  template definitions/instances, plus authored Autoplay event overrides.
  Saving a recording wrapper replaces its chart but keeps recorded hits and
  other wrapper metadata. Saving inside template isolation writes the full
  parent document with the updated definition.
- Multi-difficulty maichart JSON is not overwritten as a single-difficulty
  editor document. Use Save As instead.
- Saving a Simai source with templates or authored Autoplay events requests
  Save As JSON instead of silently losing editor metadata.
- Simai export expands template instances into ordinary notes and omits the
  template library and authored input events. It cannot represent different
  hi-speeds at the same instant. Unfinished slide endpoints must be completed
  before Simai save/export.

See [Templates](EDITOR_TEMPLATES.md) and [Autoplay Workspace](EDITOR_AUTOPLAY.md)
for the editing workflows.

## Failure Handling

Serialization and validation finish before the destination is touched. Writes
use a temporary sibling file and atomic rename. Errors leave the old file and
unsaved state intact; cancelling Save As does not change the current source.
Export refuses to overwrite the open source; use Save for that operation.
Opening a missing or invalid file keeps the live chart and undo history.
