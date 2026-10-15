use super::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct ChartFolder(PathBuf);

impl ChartFolder {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lambda-dx-save-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path { &self.0 }
}

impl Drop for ChartFolder {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn empty_chart() -> ChartDoc {
    let mut chart = crate::app::chart::fallback_chart();
    chart.notes.clear();
    chart.bpm = 120.0;
    chart.bpms = vec![crate::app::types::BpmChange { measure: 1.0, bpm: 120.0 }];
    chart
}

fn source_editor(path: &Path, level: Option<u32>) -> EditorState {
    EditorState {
        source: Some(ChartSource { path: path.to_path_buf(), level }),
        chart_dir: path.to_string_lossy().into_owned(), dirty: true,
        ..Default::default()
    }
}

#[test]
fn saving_without_a_source_requests_save_as_and_stays_dirty() {
    let mut editor = EditorState { dirty: true, ..Default::default() };
    save_chart(&mut editor, &empty_chart());
    assert!(editor.save_as_request);
    assert!(editor.dirty);
    assert!(editor.source.is_none());
}

#[test]
fn failed_save_keeps_the_document_dirty_and_source_unchanged() {
    let folder = ChartFolder::new();
    let path = folder.path().join("chart.json");
    std::fs::write(&path, "{}").unwrap();
    let mut editor = source_editor(&path, None);
    let mut chart = empty_chart();
    chart.bpm = f32::NAN;
    save_chart(&mut editor, &chart);
    assert!(editor.dirty);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    assert_eq!(editor.source.as_ref().unwrap().path, path);
    assert!(editor.status.as_ref().unwrap().starts_with("Save failed:"));
}

#[test]
fn missing_loaded_slot_is_not_recreated_or_written_to_another_difficulty() {
    let folder = ChartFolder::new();
    let path = folder.path().join("maidata.txt");
    let text = "&inote_2=(120){4}1,E\n";
    std::fs::write(&path, text).unwrap();
    let mut editor = source_editor(&path, Some(6));
    save_chart(&mut editor, &empty_chart());
    assert!(editor.dirty);
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

#[test]
fn native_save_retains_precision_ids_speeds_templates_and_recording_metadata() {
    use crate::app::types::{Note, TemplateDef, TemplateInstance};
    let folder = ChartFolder::new();
    let path = folder.path().join("recording.json");
    let mut chart = empty_chart();
    chart.notes.push(Note {
        id: 42, time: 1.1234567, lane: 3, hi_speed: 1.234567,
        is_star: true, is_fake_rotate: true, ..Default::default()
    });
    chart.templates.push(TemplateDef {
        id: "pattern".into(), name: "Pattern".into(), version: 2,
        notes: chart.notes.clone(), duration: 0.125,
    });
    chart.template_instances.push(TemplateInstance {
        instance_id: "instance".into(), template_id: "pattern".into(),
        template_version: 2, anchor_time: 2.1234567,
    });
    let recording = serde_json::json!({
        "created_at_epoch_ms": 123, "source": "keep", "hits": [{"time": 0.2, "lane": 5}],
        "record_speed": 0.5, "play_speed": 1.0, "extra": "untouched", "chart": chart,
    });
    std::fs::write(&path, serde_json::to_string(&recording).unwrap()).unwrap();
    let mut editor = source_editor(&path, None);
    save_chart(&mut editor, &chart);
    assert!(!editor.dirty, "{:?}", editor.status);
    let reopened = crate::app::chart::load_chart_from_path(&path, None).unwrap();
    assert_eq!(reopened.notes, chart.notes);
    assert_eq!(reopened.templates[0].notes, chart.templates[0].notes);
    assert_eq!(reopened.template_instances[0].anchor_time, chart.template_instances[0].anchor_time);
    let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for field in ["created_at_epoch_ms", "source", "hits", "record_speed", "play_speed", "extra"] {
        assert_eq!(saved[field], recording[field], "{field}");
    }
}

#[test]
fn empty_beat_version_document_keeps_templates_after_native_save() {
    use crate::app::types::TemplateDef;
    let folder = ChartFolder::new();
    let path = folder.path().join("chart.json");
    let mut chart = empty_chart();
    chart.version = "old-beat".into();
    chart.templates.push(TemplateDef {
        id: "empty".into(), name: "Empty".into(), version: 1, notes: Vec::new(), duration: 1.0,
    });
    let mut editor = EditorState::default();
    save_as_to(&mut editor, &chart, &path).unwrap();
    let reopened = crate::app::chart::load_chart_from_path(&path, None).unwrap();
    assert_eq!(reopened.templates.len(), 1);
    assert!(!reopened.version.contains("beat"));
}

#[test]
fn save_as_adopts_new_source_and_subsequent_save_only_updates_that_file() {
    let folder = ChartFolder::new();
    let old = folder.path().join("old.json");
    let new = folder.path().join("new.json");
    std::fs::write(&old, "{}").unwrap();
    let mut editor = source_editor(&old, None);
    let mut chart = empty_chart();
    save_as_to(&mut editor, &chart, &new).unwrap();
    assert_eq!(editor.source.as_ref().unwrap().path, new);
    assert_eq!(editor.chart_dir, new.to_string_lossy());
    assert!(!editor.dirty);
    editor.dirty = true;
    chart.title = "New title".into();
    save_chart(&mut editor, &chart);
    assert!(!editor.dirty);
    assert_eq!(std::fs::read_to_string(&old).unwrap(), "{}");
    assert_eq!(crate::app::chart::load_chart_from_path(&new, None).unwrap().title, "New title");
}

#[test]
fn failed_save_as_keeps_source_and_dirty_flag() {
    let folder = ChartFolder::new();
    let old = folder.path().join("chart.json");
    let bad = folder.path().join("missing/chart.json");
    let mut editor = source_editor(&old, None);
    assert!(save_as_to(&mut editor, &empty_chart(), &bad).is_err());
    assert!(editor.dirty);
    assert_eq!(editor.source.as_ref().unwrap().path, old);
    assert!(!bad.exists());
}

#[test]
fn failed_open_preserves_unsaved_chart_source_and_history() {
    let folder = ChartFolder::new();
    let source = folder.path().join("chart.json");
    let mut editor = source_editor(&source, None);
    let mut chart = empty_chart();
    editor.push_history(&chart);
    chart.title = "Unsaved edit".into();
    editor.pending_action = Some(crate::state::PendingAction::Open);
    let mut app = crate::player::state::PadPreviewState::new_at(chart, None, None, 0.0, None);
    crate::load_editor_chart(&mut editor, &mut app, &folder.path().join("missing.txt"), None);
    assert_eq!(app.chart.title, "Unsaved edit");
    assert_eq!(editor.source.as_ref().unwrap().path, source);
    assert_eq!(editor.undo.len(), 1);
    assert!(editor.dirty);
    assert!(editor.pending_action.is_none());
    assert!(editor.status.as_ref().unwrap().starts_with("Load failed:"));
}

#[test]
fn opening_a_saved_file_loads_saved_chart_and_source() {
    let folder = ChartFolder::new();
    let path = folder.path().join("saved.json");
    let mut chart = empty_chart();
    chart.title = "Saved edit".into();
    std::fs::write(&path, native_json(&chart, "{}").unwrap()).unwrap();
    let bundle = crate::runtime::try_load_chart_bundle(&path, None).unwrap();
    assert_eq!(bundle.chart.title, "Saved edit");
    let source = bundle.source.unwrap();
    assert_eq!(source.path, path);
    assert_eq!(source.level, None);
    assert!(bundle.simai.is_none());
}

#[test]
fn maichart_source_is_not_silently_overwritten_as_a_single_difficulty() {
    let folder = ChartFolder::new();
    let path = folder.path().join("chart.json");
    let text = r#"{"songName":"Multi","notes":[1,2,3]}"#;
    std::fs::write(&path, text).unwrap();
    let mut editor = source_editor(&path, None);
    save_chart(&mut editor, &empty_chart());
    assert!(editor.dirty);
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

#[cfg(feature = "backend-lean")]
#[test]
fn simai_export_flattens_instances_without_changing_the_native_document() {
    use crate::app::types::{Note, TemplateDef, TemplateInstance};
    let folder = ChartFolder::new();
    let path = folder.path().join("export.txt");
    std::fs::write(&path, "keep").unwrap();
    let mut chart = parse("(120){4}1,E");
    chart.templates.push(TemplateDef {
        id: "pattern".into(), name: "Pattern".into(), version: 1,
        notes: vec![Note { id: 9, time: 1.0, lane: 5, ..Default::default() }], duration: 1.0,
    });
    chart.template_instances.push(TemplateInstance {
        instance_id: "instance".into(), template_id: "pattern".into(), template_version: 1, anchor_time: 2.0,
    });
    export_to(&EditorState::default(), &chart, &path).unwrap();
    let reopened = crate::app::chart::load_chart_from_path(&path, None).unwrap();
    assert_eq!(reopened.notes.iter().map(|n| (n.time, n.lane)).collect::<Vec<_>>(), [(1.0, 1), (2.0, 5)]);
    assert!(reopened.templates.is_empty());
    assert!(reopened.template_instances.is_empty());
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(chart.template_instances.len(), 1);
}

#[test]
fn saving_inside_template_isolation_folds_the_parent_and_preserves_autoplay_edits() {
    use crate::app::autoplay_edit::{AutoplayKey, AutoplayOverride};
    use crate::app::types::Note;
    use crate::core::types::{SensorArea, TimedInputEvent};
    use crate::templates::{Action, apply};
    let folder = ChartFolder::new();
    let path = folder.path().join("template.json");
    let mut chart = empty_chart();
    chart.notes = vec![Note { id: 11, time: 3.0, lane: 1, ..Default::default() }];
    chart.autoplay_overrides = vec![AutoplayOverride {
        key: AutoplayKey { note_id: 11, role: 0, slide: 0, segment: 0, track: None },
        events: vec![TimedInputEvent::SensorClick { tp: 1234, area: SensorArea::A1 }],
    }];
    let mut editor = EditorState::default();
    editor.selected.insert(11);
    let mut app = crate::player::state::PadPreviewState::new_at(chart, None, None, 0.0, None);
    app.forced_time = Some(0.0);
    apply(&mut editor, &mut app, Action::CreateSelection);
    let authored = app.chart.autoplay_overrides.clone();
    apply(&mut editor, &mut app, Action::EnterInstance("inst_1".into()));
    editor.push_history(&app.chart);
    app.chart.notes[0].lane = 7;
    save_as_to(&mut editor, &app.chart, &path).unwrap();
    assert!(!editor.dirty);
    let reopened = crate::app::chart::load_chart_from_path(&path, None).unwrap();
    assert!(reopened.notes.is_empty(), "isolated notes must not leak into the main document");
    assert_eq!(reopened.templates[0].notes[0].time, 1.0);
    assert_eq!(reopened.templates[0].notes[0].lane, 7);
    assert_eq!(reopened.templates[0].version, 2);
    assert_eq!(reopened.template_instances[0].anchor_time, 3.0);
    assert_eq!(reopened.autoplay_overrides, authored);
    apply(&mut editor, &mut app, Action::Exit);
    assert!(!editor.dirty);
    assert_eq!(app.chart.templates, reopened.templates);
    assert_eq!(editor.source.as_ref().unwrap().path, path);
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.templates[0].notes[0].lane, 1);
}

#[test]
fn simai_save_with_editor_metadata_requests_json_and_leaves_source_intact() {
    use crate::app::types::TemplateDef;
    let folder = ChartFolder::new();
    let path = folder.path().join("maidata.txt");
    let text = "&inote_6=(120){4}1,E\n";
    std::fs::write(&path, text).unwrap();
    let mut chart = empty_chart();
    chart.templates.push(TemplateDef {
        id: "t".into(), name: "T".into(), version: 1, notes: Vec::new(), duration: 1.0,
    });
    let mut editor = source_editor(&path, Some(6));
    save_chart(&mut editor, &chart);
    assert!(editor.save_as_request);
    assert!(editor.dirty);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert!(save_as_to(&mut editor, &chart, &folder.path().join("other.txt")).is_err());
    assert!(!folder.path().join("other.txt").exists());
}

#[cfg(unix)]
#[test]
fn save_preserves_source_symlink_and_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let folder = ChartFolder::new();
    let target = folder.path().join("chart.json");
    let link = folder.path().join("link.json");
    std::fs::write(&target, "{}").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, &link).unwrap();
    let mut editor = source_editor(&link, None);
    save_chart(&mut editor, &empty_chart());
    assert!(!editor.dirty, "{:?}", editor.status);
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::metadata(target).unwrap().permissions().mode() & 0o777, 0o640);
}

#[cfg(feature = "backend-lean")]
fn parse(body: &str) -> ChartDoc {
    crate::app::maidata::from_maidata_level(&format!("&inote_6={body}\n"), 6).unwrap()
}

#[cfg(feature = "backend-lean")]
fn round_trip(chart: &ChartDoc) -> ChartDoc {
    let text = simai::document(chart, 6, "").unwrap();
    crate::app::maidata::from_maidata_level(&text, 6).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

#[cfg(feature = "backend-lean")]
#[test]
fn save_targets_loaded_slot_even_while_another_difficulty_is_pending() {
    let folder = ChartFolder::new();
    let path = folder.path().join("maidata.txt");
    std::fs::write(&path, "&inote_2=(120){4}1,E\n&inote_6=(120){4}3,E\n").unwrap();
    let mut editor = source_editor(&path, Some(6));
    editor.level_slots = vec![2, 6];
    editor.level_index = 0;
    let mut chart = parse("(120){4}7,E");
    chart.simai_level = 14;
    save_chart(&mut editor, &chart);
    assert!(!editor.dirty, "{:?}", editor.status);
    assert_eq!(crate::app::chart::load_chart_from_path(&path, Some(1)).unwrap().notes[0].lane, 1);
    assert_eq!(crate::app::chart::load_chart_from_path(&path, Some(2)).unwrap().notes[0].lane, 7);
    editor.cancel_file_action();
    assert_eq!(editor.level_index, 1);
}

#[cfg(feature = "backend-lean")]
#[test]
fn export_is_reimportable_without_changing_open_source_or_dirty_flag() {
    let folder = ChartFolder::new();
    let source = folder.path().join("maidata.txt");
    let target = folder.path().join("export.txt");
    let text = "\u{feff}&title=Original\r\n&custom=Keep\r\n&inote_2=(100){4}1,E\r\n&inote_6=(120){4}3,E\r\n";
    std::fs::write(&source, text).unwrap();
    let editor = source_editor(&source, Some(6));
    let chart = parse("(120){4}7,E");
    export_to(&editor, &chart, &target).unwrap();
    assert!(editor.dirty);
    assert_eq!(editor.source.as_ref().unwrap().path, source);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), text);
    assert!(export_to(&editor, &chart, &source).is_err());
    let exported = std::fs::read_to_string(&target).unwrap();
    assert!(exported.starts_with('\u{feff}'));
    assert!(exported.contains("&custom=Keep\r\n"));
    assert!(exported.contains("&inote_2=(100){4}1,E\r\n"));
    let reopened = crate::app::chart::load_chart_from_path(&target, None).unwrap();
    assert_eq!(reopened.notes[0].lane, 7);
}

#[cfg(feature = "backend-lean")]
#[test]
fn simai_round_trip_keeps_note_types_flags_zero_holds_and_slide_paths() {
    let mut chart = parse("(120){4}1b/2x/3$/4$$/Cf/B3x/Chf[0]/5h[0],1b-5[0.25##1]/2-6[0.25##1]b,3-5-8[0.5##2]/6!-2[0##1],1w5[0.25##1],E");
    for (i, note) in chart.notes.iter_mut().enumerate() { note.id = 100 + i as u64; }
    let reopened = round_trip(&chart);
    assert_eq!(reopened.notes.len(), chart.notes.len());
    for (expected, actual) in chart.notes.iter().zip(&reopened.notes) {
        let mut expected = expected.clone();
        expected.id = actual.id;
        expected.time = actual.time;
        assert_eq!(expected, *actual, "lane {}", actual.lane);
    }
    assert!(reopened.notes[2].is_star);
    assert!(reopened.notes[3].is_fake_rotate);
    assert!(reopened.notes[4].is_hanabi);
    assert!(reopened.notes[8].is_break);
    assert!(!reopened.notes[8].slide[0].slide_is_break);
    assert!(!reopened.notes[9].is_break);
    assert!(reopened.notes[9].slide[0].slide_is_break);
}

#[cfg(feature = "backend-lean")]
#[test]
fn simai_round_trip_keeps_bpm_changes_off_grid_times_and_speeds() {
    use crate::app::types::{BpmChange, Note, note_secs};
    let mut chart = empty_chart();
    chart.bpms.push(BpmChange { measure: 2.5, bpm: 180.0 });
    for (i, time) in [1.12345, 1.3333334, 2.34567, 2.5, 2.6666667, 4.12345].into_iter().enumerate() {
        chart.notes.push(Note {
            id: i as u64, time, lane: i as u8 + 1,
            hi_speed: if i < 3 { 0.75 } else { 1.25 }, ..Default::default()
        });
    }
    let reopened = round_trip(&chart);
    assert_eq!(reopened.bpms.len(), chart.bpms.len());
    for (a, b) in chart.bpms.iter().zip(&reopened.bpms) {
        assert_eq!(a.bpm, b.bpm);
        assert!((a.measure - b.measure).abs() < 0.00002);
    }
    for (a, b) in chart.notes.iter().zip(&reopened.notes) {
        assert!((note_secs(a, &chart.bpms) - note_secs(b, &reopened.bpms)).abs() < 0.00002, "{a:?}\n{b:?}");
        assert_eq!(a.hi_speed, b.hi_speed);
    }
}

#[cfg(feature = "backend-lean")]
#[test]
fn dense_triplets_do_not_accumulate_export_timing_drift() {
    use crate::app::types::{Note, note_secs};
    let mut chart = empty_chart();
    chart.notes = (0..600).map(|i| Note {
        id: i, time: 1.0 + i as f32 / 12.0, lane: (i % 8) as u8 + 1,
        hi_speed: 1.0, ..Default::default()
    }).collect();
    let reopened = round_trip(&chart);
    assert_eq!(reopened.notes.len(), chart.notes.len());
    let last = chart.notes.last().unwrap();
    let actual = reopened.notes.last().unwrap();
    assert!((note_secs(last, &chart.bpms) - note_secs(actual, &reopened.bpms)).abs() < 0.00005,
        "expected {}, got {}", last.time, actual.time);
}

#[cfg(feature = "backend-lean")]
#[test]
fn save_updates_the_open_chart_and_keeps_other_difficulties() {
    let folder = ChartFolder::new();
    let source = "&title=Original\n&artist=Composer\n&first=0.125\n&designer=Keep me\n&lv_2=4\n&inote_2=(100){4}1,2,E\n&lv_6=14+\n&inote_6=(120){4}3,4h[4:2],E\n";
    let path = folder.path().join("maidata.txt");
    std::fs::write(&path, source).unwrap();
    let mut chart = crate::app::chart::load_chart_from_path(folder.path(), None).unwrap();
    chart.notes[0].lane = 7;
    chart.notes[1].hold_duration = 1.0;
    chart.title = "Edited".into();
    let mut editor = EditorState {
        chart_dir: folder.path().to_string_lossy().into_owned(),
        source: Some(ChartSource::resolve(folder.path(), Some(6))),
        level_slots: vec![2, 6], level_index: 1, dirty: true,
        ..Default::default()
    };
    save_chart(&mut editor, &chart);
    assert!(!editor.dirty, "{:?}", editor.status);
    let reopened = crate::app::chart::load_chart_from_path(folder.path(), None).unwrap();
    assert_eq!(reopened.title, "Edited");
    assert_eq!(reopened.notes[0].lane, 7);
    assert!((reopened.notes[1].hold_duration - 1.0).abs() < 1e-5);
    let saved = std::fs::read_to_string(path).unwrap();
    assert!(saved.contains("&inote_2=(100){4}1,2,E\n"));
    assert!(saved.contains("&designer=Keep me\n"));
    assert!(saved.contains("&lv_6=14+\n"));
    assert_eq!(reopened.audio_offset, 0.125);
}
