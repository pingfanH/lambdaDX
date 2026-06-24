use super::state::{AppState, SavedPlaybackState};
use super::types::{
    Note, NoteTemplateSource, SceneRef, TemplateDef, TemplateInstance, snap_measure, Mode,
};

/// Create a new template from the currently selected notes.
/// Removes the selected notes from the main chart and places a template instance.
/// NO expanded notes are stored in chart.notes — the instance is rendered virtually.
pub fn create_template(app: &mut AppState, name: &str) -> Result<String, String> {
    let indices = if !app.selected_notes.is_empty() {
        app.selected_notes.clone()
    } else if let Some(id) = app.selected_note {
        if let Some(i) = app.find_note_index(id) { vec![i] } else { return Err("Selected note not found".to_string()); }
    } else {
        return Err("No notes selected".to_string());
    };

    if indices.is_empty() {
        return Err("No notes selected".to_string());
    }

    // Preserve timeline view position to prevent jumping.
    let saved_view_time = app.timeline_view_time;

    app.push_undo();

    let mut sorted_indices = indices.clone();
    sorted_indices.sort_unstable();
    sorted_indices.dedup();

    let selected_notes: Vec<Note> = sorted_indices
        .iter()
        .filter_map(|&i| app.chart.notes.get(i).cloned())
        .collect();

    if selected_notes.is_empty() {
        return Err("Selected notes not found".to_string());
    }

    let base_time = selected_notes
        .iter()
        .map(|n| n.time)
        .fold(f32::INFINITY, f32::min);

    let end_time = selected_notes
        .iter()
        .map(|n| {
            let dur = n.hold_duration.max(
                n.slide.iter()
                    .map(|s| s.slide_duration)
                    .fold(0.0_f32, f32::max),
            );
            n.time + dur
        })
        .fold(0.0_f32, f32::max);

    let duration = (end_time - base_time).max(0.25);

    // Clone notes and rebase time so the first note starts at 1.0.
    let mut tpl_notes: Vec<Note> = selected_notes
        .iter()
        .map(|n| {
            let mut cloned = n.clone();
            cloned.id = 0;
            cloned.time = (n.time - base_time) + 1.0;
            cloned.template_source = None;
            cloned
        })
        .collect();

    for note in &mut tpl_notes {
        note.id = app.next_id();
    }

    let tpl_id = format!("tpl_{}", app.next_template_id);
    app.next_template_id += 1;

    let template = TemplateDef {
        id: tpl_id.clone(),
        name: name.to_string(),
        version: 1,
        notes: tpl_notes,
        duration,
    };

    // Remove original notes from the chart (back to front for safe removal).
    for &idx in sorted_indices.iter().rev() {
        if idx < app.chart.notes.len() {
            app.chart.notes.remove(idx);
        }
    }

    // Place a template instance (metadata only — NO expanded notes in chart.notes).
    let instance_id = format!("inst_{}", app.next_instance_id);
    app.next_instance_id += 1;

    let instance = TemplateInstance {
        instance_id,
        template_id: tpl_id.clone(),
        template_version: 1,
        anchor_time: base_time,
    };

    app.chart.templates.push(template);
    app.chart.template_instances.push(instance);

    app.recompute_each();
    app.selected_note = None;
    app.selected_notes.clear();
    app.selected_note_ids.clear();

    // Restore timeline view position.
    app.timeline_view_time = saved_view_time;

    Ok(tpl_id)
}

/// Create a new empty template and immediately enter isolation mode to edit it.
pub fn create_new_template(app: &mut AppState, name: &str) -> Result<String, String> {
    app.push_undo();

    let tpl_id = format!("tpl_{}", app.next_template_id);
    app.next_template_id += 1;

    let template = TemplateDef {
        id: tpl_id.clone(),
        name: name.to_string(),
        version: 1,
        notes: Vec::new(),
        duration: 1.0,
    };

    app.chart.templates.push(template);

    let tpl_idx = app.chart.templates.len() - 1;
    enter_isolation(app, tpl_idx)?;

    Ok(tpl_id)
}

/// Rename an existing template.
pub fn rename_template(app: &mut AppState, template_idx: usize, new_name: &str) -> Result<(), String> {
    let tpl = app
        .chart
        .templates
        .get_mut(template_idx)
        .ok_or("Template not found")?;
    let old_name = tpl.name.clone();
    tpl.name = new_name.to_string();
    app.set_status(format!("Renamed '{}' -> '{}'", old_name, new_name));
    Ok(())
}

/// Insert an instance of an existing template at the current playback/view position.
/// Only stores instance metadata — no expanded notes in chart.notes.
pub fn insert_instance(app: &mut AppState, template_idx: usize) -> Result<(), String> {
    let template_id = app
        .chart
        .templates
        .get(template_idx)
        .ok_or("Template not found")?
        .id
        .clone();
    let template_version = app
        .chart
        .templates
        .get(template_idx)
        .ok_or("Template not found")?
        .version;

    let anchor_time = current_anchor_time(app);

    app.push_undo();

    let instance_id = format!("inst_{}", app.next_instance_id);
    app.next_instance_id += 1;

    let instance = TemplateInstance {
        instance_id,
        template_id,
        template_version,
        anchor_time,
    };

    let tpl_name = app.chart.templates.get(template_idx).map(|t| t.name.clone()).unwrap_or_default();
    app.chart.template_instances.push(instance);
    app.set_status(format!("Inserted template '{}' at {:.2}", tpl_name, anchor_time));

    Ok(())
}

/// Get the current anchor time (in measures) for template insertion.
/// Uses playback position if playing, otherwise timeline view position.
/// Both song_time() and timeline_view_time are in SECONDS.
pub fn current_anchor_time(app: &AppState) -> f32 {
    let secs = if matches!(app.mode, Mode::Playing | Mode::Recording) {
        app.song_time()
    } else {
        app.timeline_view_time
    };
    let measure = super::types::secs_to_measure(secs.max(0.0), &app.chart.bpms);
    snap_measure(measure.max(1.0))
}

/// Expand a template instance into concrete notes for rendering.
/// These are NOT stored — they're generated on-the-fly for display/playback.
pub fn expand_instance(
    instance: &TemplateInstance,
    template: &TemplateDef,
) -> Vec<Note> {
    let offset = instance.anchor_time - 1.0;

    template
        .notes
        .iter()
        .map(|n| {
            let mut note = n.clone();
            note.time = n.time + offset;
            note.template_source = Some(NoteTemplateSource {
                instance_id: instance.instance_id.clone(),
                template_id: instance.template_id.clone(),
                template_version: instance.template_version,
                source_note_id: n.id,
            });
            note
        })
        .collect()
}

/// Get all expanded notes from all template instances.
/// Used for rendering and playback — these are virtual, not stored.
pub fn all_expanded_notes(app: &AppState) -> Vec<Note> {
    let mut result = Vec::new();
    for inst in &app.chart.template_instances {
        if let Some(tpl) = app.chart.templates.iter().find(|t| t.id == inst.template_id) {
            result.extend(expand_instance(inst, tpl));
        }
    }
    result.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    result
}

/// Enter isolation mode to edit a template's internal notes.
/// Resets playback to the start of the template.
pub fn enter_isolation(app: &mut AppState, template_idx: usize) -> Result<(), String> {
    let tpl_id = app
        .chart
        .templates
        .get(template_idx)
        .ok_or("Template not found")?
        .id
        .clone();

    // Save current playback state.
    app.saved_playback = Some(SavedPlaybackState {
        mode: app.mode,
        mode_wall_anchor: app.mode_wall_anchor,
        mode_song_offset: app.mode_song_offset,
        timeline_view_time: app.timeline_view_time,
        playback_cursor: app.playback_cursor,
    });

    // Stop audio.
    app.stop_audio_if_any();
    if app.mode == Mode::Playing || app.mode == Mode::Recording {
        app.mode = Mode::Idle;
    }

    // Push current scene.
    app.scene_stack.push(app.active_scene.clone());

    // Save main chart notes (only non-template notes are in chart.notes now).
    if app.active_scene == SceneRef::Main {
        app.main_chart_notes = app.chart.notes.clone();
    }

    // Load template notes into editor context.
    let tpl = app.chart.templates.get(template_idx).unwrap();
    app.chart.notes = tpl.notes.clone();

    app.active_scene = SceneRef::Template {
        template_id: tpl_id,
        instance_anchor: None, // editing template definition directly
    };
    app.selected_note = None;
    app.selected_notes.clear();
    app.selected_note_ids.clear();

    // Reset playback to start of template.
    app.timeline_view_time = 0.0;
    app.playback_cursor = 0;
    app.mode_song_offset = 0.0;
    app.mode_wall_anchor = macroquad::prelude::get_time();

    let tpl_name = &app.chart.templates.get(template_idx).unwrap().name;
    app.set_status(format!("Editing template: {}", tpl_name));

    Ok(())
}

/// Enter isolation mode for a specific template instance on the timeline.
/// Double-click on a template block calls this.
pub fn enter_instance_isolation(app: &mut AppState, instance_idx: usize) -> Result<(), String> {
    let inst = app.chart.template_instances.get(instance_idx)
        .ok_or("Instance not found")?
        .clone();

    let tpl_idx = app.chart.templates.iter()
        .position(|t| t.id == inst.template_id)
        .ok_or("Template not found for instance")?;

    // Save current playback state.
    app.saved_playback = Some(SavedPlaybackState {
        mode: app.mode,
        mode_wall_anchor: app.mode_wall_anchor,
        mode_song_offset: app.mode_song_offset,
        timeline_view_time: app.timeline_view_time,
        playback_cursor: app.playback_cursor,
    });

    app.stop_audio_if_any();
    if app.mode == Mode::Playing || app.mode == Mode::Recording {
        app.mode = Mode::Idle;
    }

    app.scene_stack.push(app.active_scene.clone());

    if app.active_scene == SceneRef::Main {
        app.main_chart_notes = app.chart.notes.clone();
    }

    // Load template notes, offset by instance anchor so they appear at their timeline position.
    let tpl = app.chart.templates.get(tpl_idx).unwrap();
    let offset = inst.anchor_time - 1.0;
    let offset_notes: Vec<Note> = tpl.notes.iter().map(|n| {
        let mut cn = n.clone();
        cn.time = n.time + offset;
        cn
    }).collect();
    app.chart.notes = offset_notes;

    app.active_scene = SceneRef::Template {
        template_id: inst.template_id.clone(),
        instance_anchor: Some(inst.anchor_time),
    };
    app.selected_note = None;
    app.selected_notes.clear();
    app.selected_note_ids.clear();

    // Set view to the instance's position (in seconds, since timeline_view_time is in seconds).
    let anchor_secs = super::types::measure_to_secs(inst.anchor_time, &app.chart.bpms);
    app.timeline_view_time = anchor_secs;
    // Don't reset playback — keep current position so music continues.
    app.playback_cursor = 0;

    let tpl_name = &app.chart.templates.get(tpl_idx).unwrap().name;
    app.set_status(format!("Editing instance of '{}'", tpl_name));

    Ok(())
}

/// Exit isolation mode, saving changes back to the template definition.
/// Restores the main chart and playback state.
pub fn exit_isolation(app: &mut AppState) -> Result<(), String> {
    let current_scene = app.active_scene.clone();

    match current_scene {
        SceneRef::Main => return Err("Already on Main scene".to_string()),
        SceneRef::Template { ref template_id, instance_anchor } => {
            // If editing via instance, un-offset the notes before saving.
            let notes_to_save = if let Some(anchor) = instance_anchor {
                let offset = anchor - 1.0;
                app.chart.notes.iter().map(|n| {
                    let mut cn = n.clone();
                    cn.time = n.time - offset;
                    cn
                }).collect()
            } else {
                app.chart.notes.clone()
            };

            // Save edited notes back to the template definition.
            if let Some(tpl) = app
                .chart
                .templates
                .iter_mut()
                .find(|t| t.id == *template_id)
            {
                tpl.notes = notes_to_save;
                tpl.version += 1;

                // Update duration from the notes.
                let end_time = tpl
                    .notes
                    .iter()
                    .map(|n| {
                        let dur = n.hold_duration.max(
                            n.slide
                                .iter()
                                .map(|s| s.slide_duration)
                                .fold(0.0_f32, f32::max),
                        );
                        n.time + dur
                    })
                    .fold(0.0_f32, f32::max);
                tpl.duration = (end_time - 1.0).max(0.25);

                // Update instance versions.
                let tpl_id_owned = tpl.id.clone();
                let tpl_new_version = tpl.version;
                for inst in &mut app.chart.template_instances {
                    if inst.template_id == tpl_id_owned {
                        inst.template_version = tpl_new_version;
                    }
                }
            }

            // Pop the parent scene.
            let parent = app.scene_stack.pop().unwrap_or(SceneRef::Main);
            app.active_scene = parent;

            // Restore main chart notes (non-template notes only).
            if app.active_scene == SceneRef::Main {
                app.chart.notes = std::mem::take(&mut app.main_chart_notes);
                // Safety cleanup: remove any notes with template_source that may have leaked.
                app.chart.notes.retain(|n| n.template_source.is_none());

                // Additional cleanup: remove notes that match any template instance's
                // expanded notes (in case they leaked into chart.notes).
                let mut template_note_times: Vec<(f32, u8)> = Vec::new();
                for inst in &app.chart.template_instances {
                    if let Some(tpl) = app.chart.templates.iter().find(|t| t.id == inst.template_id) {
                        let offset = inst.anchor_time - 1.0;
                        for n in &tpl.notes {
                            template_note_times.push((snap_measure(n.time + offset), n.lane));
                        }
                    }
                }
                app.chart.notes.retain(|n| {
                    let ns = snap_measure(n.time);
                    !template_note_times.iter().any(|(t, l)| (*t - ns).abs() < 0.003 && *l == n.lane)
                });
            }

            // Restore playback state.
            if let Some(saved) = app.saved_playback.take() {
                app.mode = saved.mode;
                app.mode_wall_anchor = saved.mode_wall_anchor;
                app.mode_song_offset = saved.mode_song_offset;
                app.timeline_view_time = saved.timeline_view_time;
                app.playback_cursor = saved.playback_cursor;

                if app.mode == Mode::Playing {
                    app.request_audio_start();
                }
            }

            app.recompute_each();
            app.selected_note = None;
            app.selected_notes.clear();
            app.selected_note_ids.clear();

            app.set_status("Returned to Main".to_string());
        }
    }

    Ok(())
}

/// Get the breadcrumb path as a list of scene names.
pub fn breadcrumb_path(app: &AppState) -> Vec<String> {
    let mut path: Vec<String> = Vec::new();
    for scene in &app.scene_stack {
        path.push(scene_display_name(scene, app));
    }
    path.push(scene_display_name(&app.active_scene, app));
    path
}

fn scene_display_name(scene: &SceneRef, app: &AppState) -> String {
    match scene {
        SceneRef::Main => "Main".to_string(),
        SceneRef::Template { template_id, .. } => app
            .chart
            .templates
            .iter()
            .find(|t| t.id == *template_id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| template_id.clone()),
    }
}

/// Navigate to a specific breadcrumb level by popping the scene stack.
pub fn navigate_to_breadcrumb(app: &mut AppState, target_depth: usize) {
    while app.scene_stack.len() > target_depth {
        let _ = exit_isolation(app);
    }
}

/// Check if we're currently in isolation mode (editing a template).
pub fn is_in_isolation(app: &AppState) -> bool {
    app.active_scene != SceneRef::Main
}

/// Get the name of the currently editing template, if in isolation mode.
pub fn current_template_name(app: &AppState) -> Option<String> {
    match &app.active_scene {
        SceneRef::Template { template_id, .. } => app
            .chart
            .templates
            .iter()
            .find(|t| t.id == *template_id)
            .map(|t| t.name.clone()),
        _ => None,
    }
}

/// Get the time range (start, end) of a template instance in the main chart.
pub fn instance_time_range(app: &AppState, inst: &TemplateInstance) -> (f32, f32) {
    let duration = app
        .chart
        .templates
        .iter()
        .find(|t| t.id == inst.template_id)
        .map(|t| t.duration)
        .unwrap_or(1.0);
    (inst.anchor_time, inst.anchor_time + duration)
}

/// Find the template instance index at a given measure time, if any.
pub fn instance_at_time(app: &AppState, time: f32) -> Option<usize> {
    app.chart
        .template_instances
        .iter()
        .position(|inst| {
            let (start, end) = instance_time_range(app, inst);
            time >= start && time < end
        })
}

/// Move a template instance to a new anchor time.
pub fn move_instance(app: &mut AppState, instance_idx: usize, new_anchor: f32) {
    if let Some(inst) = app.chart.template_instances.get_mut(instance_idx) {
        inst.anchor_time = snap_measure(new_anchor.max(1.0));
    }
}

/// Update template duration to encompass all its notes, including any at new positions.
pub fn update_template_duration(app: &mut AppState, template_id: &str) {
    if let Some(tpl) = app.chart.templates.iter_mut().find(|t| t.id == template_id) {
        let end_time = tpl
            .notes
            .iter()
            .map(|n| {
                let dur = n.hold_duration.max(
                    n.slide.iter()
                        .map(|s| s.slide_duration)
                        .fold(0.0_f32, f32::max),
                );
                n.time + dur
            })
            .fold(0.0_f32, f32::max);
        tpl.duration = (end_time - 1.0).max(0.25);
    }
}
