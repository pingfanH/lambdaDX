//! Strict headless replay of the player's real autoplay/input/progress pipeline.
#![allow(dead_code, unused_variables, unused_imports)]

#[path = "../app/mod.rs"]
mod app;
#[path = "../core/mod.rs"]
mod core;
#[path = "../player/mod.rs"]
mod player;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use core::types::{JudgeGrade, RenderCommand};
use player::engine::{step_engine_events, timed_input_tp};
use player::state::PadPreviewState;
use serde::Serialize;
use serde_json::json;

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(BufWriter::new(file), value).map_err(|e| e.to_string())
}

fn line(out: &mut BufWriter<File>, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *out, value).map_err(|e| e.to_string())?;
    writeln!(out).map_err(|e| e.to_string())
}

/// Check raw core commands against both bridge output and the applied state.
fn check_progress(
    pad: &PadPreviewState, spec: &core::types::ChartSpec, commands: &[RenderCommand],
    updates: &[player::engine::SlideArcProgress], failures: &mut Vec<String>,
) -> usize {
    let engine = pad.judge_engine.as_ref().unwrap();
    let mut checked = 0;
    for command in commands {
        let (note, track, remaining, bar, complete) = match command {
            RenderCommand::UpdateSlideProgress { note_index, remaining } => (*note_index, None, Some(*remaining), None, false),
            RenderCommand::UpdateSlideTrackProgress { note_index, track_index, remaining } =>
                (*note_index, Some(*track_index), Some(*remaining), None, false),
            RenderCommand::HideSlideBars { note_index, end_index } => (*note_index, None, None, Some(*end_index as usize), false),
            RenderCommand::HideSlideTrackBars { note_index, track_index, end_index } =>
                (*note_index, Some(*track_index), None, Some(*end_index as usize), false),
            RenderCommand::HideAllSlideBars { note_index } => (*note_index, None, None, None, true),
            RenderCommand::ShowJudgeResult { .. } => continue,
        };
        checked += 1;
        let Some(runtime) = engine.runtime_slide_index(note) else {
            failures.push(format!("unmapped raw command: {command:?}")); continue;
        };
        let Some(update) = updates.iter().find(|u| u.runtime_slide_index == runtime && u.track_index == track) else {
            failures.push(format!("dropped raw command: {command:?}")); continue;
        };
        let Some(slide) = spec.slides.get(runtime) else {
            failures.push(format!("missing runtime spec {runtime}")); continue;
        };
        let wifi = slide.slide_kind == core::types::RuntimeSlideKind::Wifi;
        let Some((id, sub, segment, _)) = engine.slide_position(pad.playback_chart(), runtime) else {
            failures.push(format!("unmapped chart position: {command:?}")); continue;
        };
        let Some(state) = pad.slide_progress.get(&(id, sub)) else {
            failures.push(format!("missing renderer state: {command:?}")); continue;
        };
        if let Some(bar) = bar {
            let stored = if let Some(track) = track { state.track_hidden_until.get(&track).copied() }
                else { state.seg_hidden_until.get(segment).copied().flatten() };
            if update.hidden_until_bar.is_none_or(|value| value < bar) || stored.is_none_or(|value| value < bar) {
                failures.push(format!("lost hide cutoff: {command:?}, update={update:?}, stored={stored:?}"));
            }
        }
        if let Some(value) = remaining.filter(|_| wifi) {
            let expected = value as f32 / if slide.is_classic { 8.0 } else { 9.0 };
            if update.shared_frac.is_none_or(|value| value + 0.000001 < expected) || state.shared_frac + 0.000001 < expected {
                failures.push(format!("lost WiFi shared cursor: {command:?}"));
            }
        }
        let expected = if complete { Some(1.0) }
            else if let Some(value) = remaining.filter(|_| !wifi) {
                let total = slide.judge_queues.iter().map(Vec::len).max().unwrap_or(0);
                (total > 0).then(|| total.saturating_sub(value as usize) as f32 / total as f32)
            } else if let Some(bar) = bar.filter(|_| track.is_some()) {
                let queue = slide.judge_queues.get(track.unwrap() as usize);
                queue.and_then(|queue| queue.iter().map(|a| a.arrow_progress_when_finished.max(a.arrow_progress_when_on)).max())
                    .filter(|&total| total > 0).map(|total| (bar as f32 / total as f32).min(1.0))
            } else { None };
        if let Some(expected) = expected {
            let stored = if let Some(track) = track { state.track_frac.get(&track).copied() }
                else { state.seg_frac.get(segment).copied() };
            if update.frac + 0.000001 < expected || stored.is_none_or(|value| value + 0.000001 < expected) {
                failures.push(format!("lost progress value: {command:?}, expected={expected}, update={update:?}, stored={stored:?}"));
            }
        }
    }
    checked
}

fn replay(path: &Path, level: u32, output: &Path, inner: bool, stalled: bool, direct: bool) -> Result<bool, String> {
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let text = app::chart::read_simai_source(path).ok_or("Missing maidata.txt")?;
    fs::write(output.join("source.txt"), &text).map_err(|e| e.to_string())?;
    let chart = app::maidata::from_maidata_level(&text, level)?;
    let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
    pad.params.autoplay_inner_screen = inner;
    pad.autoplay = true;
    pad.autoplay_direct_core = direct;
    pad.forced_time = Some(0.0);
    pad.audio_enabled = false;
    pad.load_engine(&text, level)?;
    let spec = pad.judge_engine.as_ref().unwrap().chart_spec()?;
    write_json(&output.join("chart.json"), &pad.chart)?;
    write_json(&output.join("spec.json"), &spec)?;
    write_json(&output.join("plan.json"), &pad.autoplay_plan)?;
    write_json(&output.join("events.json"), &pad.autoplay_tactic)?;
    let mut frames = BufWriter::new(File::create(output.join("frames.jsonl")).map_err(|e| e.to_string())?);
    let mut inputs = BufWriter::new(File::create(output.join("inputs.jsonl")).map_err(|e| e.to_string())?);
    let mut failures = Vec::new();
    let mut judged = BTreeMap::<u64, Vec<core::types::JudgeEvent>>::new();
    let mut progress = BTreeMap::<(usize, Option<u64>), f32>::new();
    let mut raw_finished = BTreeSet::new();
    let mut emitted = 0;
    let mut checked_progress = 0;
    let mut grades = BTreeMap::<String, usize>::new();
    let sdez = pad.judge_engine.as_ref().unwrap().is_sdez();
    let mut expected: BTreeSet<_> = spec.taps.iter().map(|n| n.note_index)
        .chain(spec.holds.iter().map(|n| n.note_index))
        .chain(spec.touches.iter().map(|n| n.note_index))
        .chain(spec.touch_holds.iter().map(|n| n.note_index))
        .chain(spec.slide_heads.iter().filter(|_| !sdez).map(|n| n.note_index))
        .chain(spec.slides.iter().filter(|n| !n.is_conn_slide || n.is_group_end).map(|n| n.note_index))
        .collect();
    let end = spec.taps.iter().map(|n| n.timing)
        .chain(spec.holds.iter().map(|n| n.timing + n.length))
        .chain(spec.touches.iter().map(|n| n.timing))
        .chain(spec.touch_holds.iter().map(|n| n.timing + n.length))
        .chain(spec.slides.iter().map(|n| n.start_timing + n.length))
        .chain(pad.autoplay_tactic.iter().map(timed_input_tp)).max().unwrap_or(0) + 3_000_000;
    let mut now = 0_i64;
    let mut frame = 0;
    while now <= end {
        pad.forced_time = Some(now as f32 / 1e6);
        player::autoplay::tick(&mut pad);
        let due = std::mem::take(&mut pad.engine_events);
        emitted += due.len();
        line(&mut inputs, &json!({"frame": frame, "timeUs": now, "events": due}))?;
        if std::env::var_os("MAI2_AUDIT_INPUT_ONLY").is_none() {
            pad.judge_engine.as_mut().unwrap().set_autoplay(pad.autoplay);
        }
        let results = step_engine_events(pad.judge_engine.as_mut().unwrap(), now as f32 / 1e6, due);
        for result in results {
            let result = match result {
                Ok(result) => result,
                Err(error) => { failures.push(format!("frame {frame}: {error}")); continue; }
            };
            let direct = pad.judge_engine.as_mut().unwrap().take_slide_progress();
            let engine = pad.judge_engine.as_ref().unwrap();
            let mut updates = engine.slide_progress_updates(&result.render_commands);
            updates.extend(direct);
            let mut mapped = Vec::new();
            for update in &updates {
                let position = engine.slide_position(pad.playback_chart(), update.runtime_slide_index);
                if position.is_none() { failures.push(format!("unmapped progress: {update:?}")); }
                let slot = progress.entry((update.runtime_slide_index, update.track_index)).or_default();
                if update.frac < *slot && update.hidden_until_bar.is_none() && update.shared_frac.is_none() {
                    failures.push(format!("regressing progress: {update:?}, previous={slot}"));
                }
                *slot = slot.max(update.frac);
                mapped.push(json!({"runtime": update.runtime_slide_index, "track": update.track_index,
                    "fraction": update.frac, "sharedFraction": update.shared_frac,
                    "hiddenUntilBar": update.hidden_until_bar, "chartPosition": position}));
            }
            for command in &result.render_commands {
                let key = match command {
                    RenderCommand::UpdateSlideProgress { note_index, remaining } => Some((*note_index, None, *remaining)),
                    RenderCommand::UpdateSlideTrackProgress { note_index, track_index, remaining } =>
                        Some((*note_index, Some(*track_index), *remaining)),
                    _ => None,
                };
                if let Some((note, track, remaining)) = key {
                    let runtime = engine.runtime_slide_index(note);
                    let slide = runtime.and_then(|runtime| spec.slides.get(runtime));
                    let terminal = slide.map_or(0, |slide| {
                        if slide.slide_kind == core::types::RuntimeSlideKind::Wifi {
                            if slide.is_classic { 8 } else { 9 }
                        } else { 0 }
                    });
                    if remaining == terminal { raw_finished.insert((note, track)); }
                    if !updates.iter().any(|u| Some(u.runtime_slide_index) == runtime && u.track_index == track) {
                        failures.push(format!("dropped raw progress: {command:?}"));
                    }
                }
            }
            for event in &result.events {
                *grades.entry(format!("{:?}", event.grade)).or_default() += 1;
                if event.grade != JudgeGrade::Perfect { failures.push(format!("non-Perfect: {event:?}")); }
                judged.entry(event.note_index).or_default().push(event.clone());
                expected.remove(&event.note_index);
            }
            pad.apply_core_slide_progress_updates(&updates);
            checked_progress += check_progress(&pad, &spec, &result.render_commands, &updates, &mut failures);
            let visual: Vec<_> = pad.slide_progress.iter().map(|(key, value)| json!({
                "noteId": key.0, "slide": key.1, "segments": value.seg_frac,
                "tracks": value.track_frac, "hiddenUntil": value.track_hidden_until,
                "segmentHiddenUntil": value.seg_hidden_until, "sharedFraction": value.shared_frac,
            })).collect();
            line(&mut frames, &json!({"frame": frame, "timeUs": now, "result": result,
                "mappedProgress": mapped, "visualProgress": visual}))?;
        }
        pad.pad_feedback.clear();
        frame += 1;
        now += if stalled && frame % 47 == 0 { 183_337 } else { 16_667 };
    }
    for note in expected { failures.push(format!("missing judgment for runtime note {note}")); }
    for hold in spec.holds.iter().map(|note| note.note_index)
        .chain(spec.touch_holds.iter().map(|note| note.note_index)) {
        let events = judged.get(&hold).map(Vec::as_slice).unwrap_or_default();
        for phase in [core::types::JudgePhase::Head, core::types::JudgePhase::Tail] {
            let count = events.iter().filter(|event| event.phase == phase).count();
            if count != 1 { failures.push(format!("hold {hold}: expected one {phase:?} judgment, got {count}")); }
        }
    }
    for (runtime, slide) in spec.slides.iter().enumerate() {
        let tracks: Vec<_> = if slide.track_count > 1 || slide.is_conn_slide {
            (0..slide.track_count).map(Some).collect()
        } else { vec![None] };
        for track in tracks {
            if !raw_finished.contains(&(slide.note_index, track)) {
                failures.push(format!("missing terminal raw progress: runtime={runtime}, note={}, track={track:?}", slide.note_index));
            }
            if progress.get(&(runtime, track)).copied().unwrap_or(0.0) < 1.0 {
                failures.push(format!("incomplete mapped progress: runtime={runtime}, track={track:?}"));
            }
        }
        let engine = pad.judge_engine.as_ref().unwrap();
        if let Some((id, sub, segment, _)) = engine.slide_position(pad.playback_chart(), runtime) {
            if pad.slide_progress.get(&(id, sub)).and_then(|p| p.seg_frac.get(segment)).copied().unwrap_or(0.0) < 1.0 {
                failures.push(format!("incomplete visual progress: runtime={runtime}, note={id}, sub={sub}, segment={segment}"));
            }
        } else { failures.push(format!("unmapped runtime arc {runtime}")); }
    }
    if pad.autoplay_tactic_cursor != pad.autoplay_tactic.len() { failures.push("unconsumed autoplay events".into()); }
    if emitted != pad.autoplay_tactic.len() { failures.push(format!("input count mismatch: {emitted}/{}", pad.autoplay_tactic.len())); }
    frames.flush().map_err(|e| e.to_string())?;
    inputs.flush().map_err(|e| e.to_string())?;
    write_json(&output.join("judgments.json"), &judged)?;
    write_json(&output.join("summary.json"), &json!({"passed": failures.is_empty(), "source": path,
        "level": level, "innerScreen": inner, "stalled": stalled, "direct": direct,
        "frames": frame, "inputEvents": emitted, "plannedEvents": pad.autoplay_tactic.len(),
        "judgedObjects": judged.len(), "grades": grades, "runtimeArcs": spec.slides.len(),
        "checkedProgressCommands": checked_progress, "failures": failures}))?;
    println!("{} lv{level} {} {}: grades={grades:?}, {} failures; {}", path.display(),
        if inner { "screen" } else { "buttons" }, if stalled { "stalled" } else { "60fps" }, failures.len(), output.display());
    Ok(failures.is_empty())
}

fn main() {
    if let Err(error) = run() { eprintln!("{error}"); std::process::exit(1); }
}

fn run() -> Result<(), String> {
    let mut paths = Vec::new();
    let mut output = PathBuf::from("out/autoplay_audit");
    let mut level = None;
    let mut screen = true;
    let mut stalled = false;
    let mut direct = false;
    let mut matrix = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => output = PathBuf::from(args.next().ok_or("--out needs a path")?),
            "--diff" => level = Some(args.next().ok_or("--diff needs a level")?.parse::<u32>().map_err(|e| e.to_string())?),
            "--buttons" => screen = false,
            "--stalled" => stalled = true,
            "--direct" => direct = true,
            "--matrix" => matrix = true,
            "--help" => { println!("autoplay_audit <chart paths...> [--diff N] [--buttons] [--stalled] [--direct] [--matrix] [--out DIR]\nDefaults to the highest difficulty. --matrix checks all 8 input/delivery/frame-timing combinations."); return Ok(()); }
            value if value.starts_with('-') => return Err(format!("Unknown option {value}")),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() { return Err("Provide at least one chart path".into()); }
    let mut passed = true;
    let mut summaries = Vec::new();
    let scenarios: Vec<_> = if matrix {
        [true, false].into_iter().flat_map(|inner| [false, true].into_iter()
            .flat_map(move |stalled| [false, true].into_iter().map(move |direct| (inner, stalled, direct)))).collect()
    } else { vec![(screen, stalled, direct)] };
    for (index, path) in paths.iter().enumerate() {
        let text = app::chart::read_simai_source(path).ok_or_else(|| format!("Missing source: {}", path.display()))?;
        let levels = level.map_or_else(|| app::maidata::inote_keys(&text).into_iter().max().into_iter().collect(), |n| vec![n]);
        for level in levels {
            for &(screen, stalled, direct) in &scenarios {
            let dir = output.join(format!("chart{index}-lv{level}-{}-{}{}", if screen { "screen" } else { "buttons" },
                if stalled { "stalled" } else { "60fps" }, if direct { "-direct" } else { "" }));
            passed &= replay(path, level, &dir, screen, stalled, direct)?;
            let summary: serde_json::Value = serde_json::from_reader(
                File::open(dir.join("summary.json")).map_err(|error| error.to_string())?
            ).map_err(|error| error.to_string())?;
            summaries.push(json!({"directory": dir, "summary": summary}));
            }
        }
    }
    write_json(&output.join("index.json"), &json!({"passed": passed, "runs": summaries}))?;
    if passed { Ok(()) } else { Err("Strict autoplay audit FAILED; see summary.json and full JSONL traces".into()) }
}
