//! Template library, metadata-only instances, and isolated editing transactions.

use crate::app::types::{
    ChartDoc, Mode, Note, TemplateDef, TemplateInstance, measure_to_secs, secs_to_measure,
};
use crate::player::state::PadPreviewState;
use crate::state::EditorState;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub enum Action {
    CreateSelection,
    New,
    Insert(String),
    EnterDefinition(String),
    EnterInstance(String),
    Exit,
    DeleteDefinition(String),
    DeleteInstance(String),
    Rename(String, String),
}

#[derive(Debug, Clone)]
pub struct Session {
    pub parent: ChartDoc,
    pub editor: EditorState,
    pub template_id: String,
    pub offset: f32,
    pub time: f32,
    pub playing: bool,
    pub saved: bool,
    pub backdrop: std::sync::Arc<ChartDoc>,
}

fn next_id(prefix: &str, ids: impl Iterator<Item = String>) -> String {
    let ids: std::collections::HashSet<_> = ids.collect();
    (1..)
        .map(|i| format!("{prefix}_{i}"))
        .find(|id| !ids.contains(id))
        .unwrap()
}

fn notes_for_definition(chart: &ChartDoc, offset: f32) -> Vec<Note> {
    chart
        .notes
        .iter()
        .map(|note| {
            let mut note = note.clone();
            note.time -= offset;
            note.template_source = None;
            note
        })
        .collect()
}

/// Used by every save/export route, including saves made inside isolation.
pub fn document(editor: &EditorState, chart: &ChartDoc) -> ChartDoc {
    let Some(session) = &editor.template_session else {
        return chart.clone();
    };
    let mut parent = session.parent.clone();
    if let Some(template) = parent
        .templates
        .iter_mut()
        .find(|t| t.id == session.template_id)
    {
        let notes = notes_for_definition(chart, session.offset);
        if template.notes != notes {
            template.notes = notes;
            template.version = template.version.saturating_add(1);
            template.duration = crate::app::templates::duration(&template.notes);
        }
        for instance in &mut parent.template_instances {
            if instance.template_id == template.id {
                instance.template_version = template.version;
            }
        }
    }
    let valid: std::collections::HashSet<_> = crate::app::templates::expand(&parent)
        .notes
        .iter()
        .map(|n| n.id)
        .collect();
    parent
        .autoplay_overrides
        .retain(|item| item.key.is_standalone() || valid.contains(&item.key.note_id));
    parent
}

pub fn mark_saved(editor: &mut EditorState) {
    editor.dirty = false;
    if let Some(session) = &mut editor.template_session {
        session.saved = true;
    }
}

pub fn apply(editor: &mut EditorState, app: &mut PadPreviewState, action: Action) {
    if let Err(error) = execute(editor, app, action) {
        editor.status = Some(error);
    }
    app.refresh_playback_chart();
}

fn execute(
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    action: Action,
) -> Result<(), String> {
    if matches!(action, Action::Exit) {
        cancel_interaction(editor, &mut app.chart);
        let chart = document(editor, &app.chart);
        let Some(session) = editor.template_session.take() else {
            return Ok(());
        };
        let changed = chart.templates != session.parent.templates
            || chart.autoplay_overrides != session.parent.autoplay_overrides;
        let saved = session.saved && !editor.dirty;
        let changed_since_save = session.saved && editor.dirty;
        let source = editor.source.clone();
        let chart_dir = editor.chart_dir.clone();
        let status = editor.status.clone();
        *editor = session.editor;
        if changed {
            editor.push_history(&session.parent);
        }
        if saved {
            editor.dirty = false;
        }
        editor.dirty |= changed_since_save;
        editor.source = source;
        editor.chart_dir = chart_dir;
        editor.status = status;
        editor.template_action = None;
        if app.mode == Mode::Playing {
            app.toggle_play();
        }
        app.chart = chart;
        app.refresh_playback_chart();
        editor.pending_seek = Some(session.time);
        if session.playing {
            app.toggle_play();
        }
        return Ok(());
    }
    if editor.template_session.is_some() {
        return Err("Return to Main before changing the template library".into());
    }
    match action {
        Action::CreateSelection | Action::New => {
            cancel_interaction(editor, &mut app.chart);
            let empty = matches!(action, Action::New);
            let mut notes: Vec<_> = app
                .chart
                .notes
                .iter()
                .filter(|note| editor.selected.contains(&note.id))
                .cloned()
                .collect();
            if !empty && notes.is_empty() {
                return Err("Select notes to create a template".into());
            }
            if empty {
                notes.clear();
            }
            editor.push_history(&app.chart);
            let anchor = notes
                .iter()
                .map(|note| note.time)
                .fold(f32::INFINITY, f32::min);
            let anchor = if anchor.is_finite() { anchor } else { 1.0 };
            for (index, note) in notes.iter_mut().enumerate() {
                note.id = index as u64 + 1;
                note.time += 1.0 - anchor;
                note.template_source = None;
            }
            let id = next_id("tpl", app.chart.templates.iter().map(|t| t.id.clone()));
            let name = editor.template_name.trim();
            let template = TemplateDef {
                id: id.clone(),
                name: if name.is_empty() {
                    id.clone()
                } else {
                    name.into()
                },
                version: 1,
                duration: if empty {
                    1.0
                } else {
                    crate::app::templates::duration(&notes)
                },
                notes,
            };
            app.chart.templates.push(template);
            editor.selected_template = Some(id.clone());
            if !empty {
                let old: Vec<_> = app
                    .chart
                    .notes
                    .iter()
                    .filter(|note| editor.selected.contains(&note.id))
                    .map(|note| note.id)
                    .collect();
                app.chart
                    .notes
                    .retain(|note| !editor.selected.contains(&note.id));
                let instance_id = insert(&mut app.chart, &id, anchor)?;
                let expanded = crate::app::templates::expand(&app.chart);
                for authored in &mut app.chart.autoplay_overrides {
                    if authored.key.is_standalone() { continue; }
                    if let Some(index) = old.iter().position(|id| *id == authored.key.note_id) {
                        if let Some(note) = expanded.notes.iter().find(|note| {
                            note.template_source.as_ref().is_some_and(|s| {
                                s.instance_id == instance_id && s.source_note_id == index as u64 + 1
                            })
                        }) {
                            authored.key.note_id = note.id;
                        }
                    }
                }
                editor.selected_instance = Some(instance_id);
            }
            editor.selected.clear();
            editor.template_panel = true;
            if empty {
                enter(editor, app, &id, None)?;
            }
        }
        Action::Insert(id) => {
            if !app.chart.templates.iter().any(|t| t.id == id) {
                return Err("Template not found".into());
            }
            let time = if app.mode == Mode::Idle {
                app.timeline_view_time
            } else {
                app.song_time()
            };
            let anchor = crate::timeline::geometry::snap(
                secs_to_measure(time, &app.chart.bpms),
                editor.snap_enabled,
                editor.snap_division,
            )
            .max(1.0);
            editor.push_history(&app.chart);
            editor.selected_instance = Some(insert(&mut app.chart, &id, anchor)?);
            editor.selected.clear();
        }
        Action::EnterDefinition(id) => enter(editor, app, &id, None)?,
        Action::EnterInstance(id) => {
            let instance = app
                .chart
                .template_instances
                .iter()
                .find(|i| i.instance_id == id)
                .ok_or("Instance not found")?
                .clone();
            enter(
                editor,
                app,
                &instance.template_id,
                Some(instance.anchor_time),
            )?;
        }
        Action::DeleteDefinition(id) => {
            if !app.chart.templates.iter().any(|t| t.id == id) {
                return Ok(());
            }
            editor.push_history(&app.chart);
            let ids: std::collections::HashSet<_> = app
                .playback_chart()
                .notes
                .iter()
                .filter(|note| {
                    note.template_source
                        .as_ref()
                        .is_some_and(|s| s.template_id == id)
                })
                .map(|note| note.id)
                .collect();
            app.chart.templates.retain(|t| t.id != id);
            app.chart.template_instances.retain(|i| i.template_id != id);
            app.chart
                .autoplay_overrides
                .retain(|item| item.key.is_standalone() || !ids.contains(&item.key.note_id));
            editor.selected_instance = None;
        }
        Action::DeleteInstance(id) => {
            if !app
                .chart
                .template_instances
                .iter()
                .any(|i| i.instance_id == id)
            {
                return Ok(());
            }
            editor.push_history(&app.chart);
            let ids: std::collections::HashSet<_> = app
                .playback_chart()
                .notes
                .iter()
                .filter(|note| {
                    note.template_source
                        .as_ref()
                        .is_some_and(|s| s.instance_id == id)
                })
                .map(|note| note.id)
                .collect();
            app.chart.template_instances.retain(|i| i.instance_id != id);
            app.chart
                .autoplay_overrides
                .retain(|item| item.key.is_standalone() || !ids.contains(&item.key.note_id));
            editor.selected_instance = None;
        }
        Action::Rename(id, name) => {
            let name = name.trim();
            if name.is_empty() {
                return Err("Template name cannot be empty".into());
            }
            let template = app
                .chart
                .templates
                .iter()
                .find(|t| t.id == id)
                .ok_or("Template not found")?;
            if template.name == name {
                return Ok(());
            }
            editor.push_history(&app.chart);
            app.chart
                .templates
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or("Template not found")?
                .name = name.into();
        }
        Action::Exit => unreachable!(),
    }
    Ok(())
}

fn insert(chart: &mut ChartDoc, id: &str, anchor: f32) -> Result<String, String> {
    let template = chart
        .templates
        .iter()
        .find(|t| t.id == id)
        .ok_or("Template not found")?;
    let instance_id = next_id(
        "inst",
        chart
            .template_instances
            .iter()
            .map(|i| i.instance_id.clone()),
    );
    chart.template_instances.push(TemplateInstance {
        instance_id: instance_id.clone(),
        template_id: id.into(),
        template_version: template.version,
        anchor_time: anchor,
    });
    Ok(instance_id)
}

fn enter(
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    id: &str,
    anchor: Option<f32>,
) -> Result<(), String> {
    let template = app
        .chart
        .templates
        .iter()
        .find(|t| t.id == id)
        .ok_or("Template not found")?
        .clone();
    cancel_interaction(editor, &mut app.chart);
    let offset = anchor.unwrap_or(1.0) - 1.0;
    let backdrop = std::sync::Arc::new(crate::app::templates::expand(&app.chart));
    let session = Session {
        parent: app.chart.clone(),
        editor: editor.clone(),
        template_id: id.into(),
        offset,
        time: if app.mode == Mode::Idle {
            app.timeline_view_time
        } else {
            app.song_time()
        },
        playing: app.mode == Mode::Playing,
        saved: false,
        backdrop,
    };
    if session.playing {
        app.toggle_play();
    }
    app.stop_audio_if_any();
    app.chart.notes = template.notes;
    for note in &mut app.chart.notes {
        note.time += offset;
    }
    app.chart.template_instances.clear();
    app.chart.templates.clear();
    app.chart.autoplay_overrides.clear();
    editor.undo.clear();
    editor.redo.clear();
    editor.selected.clear();
    editor.selected_instance = None;
    editor.keyboard.reset_document();
    editor.dragging = None;
    editor.handle_grab = None;
    editor.pointer_press = None;
    editor.star_placement = None;
    editor.hold_placement = None;
    editor.editing_slide = None;
    editor.view_start = measure_to_secs(offset + 1.0, &app.chart.bpms).max(0.0);
    editor.pending_seek = Some(editor.view_start);
    editor.template_panel = true;
    editor.template_session = Some(Box::new(session));
    Ok(())
}

pub fn cancel_interaction(editor: &mut EditorState, chart: &mut ChartDoc) {
    crate::keyboard::edit::cancel(editor, chart);
    crate::timeline::transform::finish(editor, chart, false);
    editor.keyboard.reset_document();
    editor.dragging = None;
    editor.drag_grab = None;
    editor.drag_history_pushed = false;
    editor.drag_multi_orig.clear();
    editor.drag_orig_notes.clear();
    editor.handle_grab = None;
    editor.pointer_press = None;
    editor.marquee_start = None;
    editor.marquee_base.clear();
    editor.transform_pointer_down = false;
    editor.star_placement = None;
    editor.hold_placement = None;
    editor.editing_slide = None;
    editor.pending_shape = None;
    editor.hovered = None;
    editor.cursor = None;
    editor.scrubbing = false;
    editor.pending_seek = None;
}
