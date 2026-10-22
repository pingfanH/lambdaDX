//! Read-only audio-analysis overlay for Workspace::Viewer.
//!
//! The overlay consumes the AudioAnalysis JSON produced by MaiRis. It does not
//! own any chart or judgment state: the editor keeps using the same
//! PadPreviewState, timeline and pad renderer as the Chart workspace.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use egui_macroquad::egui;
use macroquad::prelude as mq;
use serde::Deserialize;

use crate::app::types::{ChartDoc, Mode};
use crate::keyboard::navigation::reference_time;
use crate::player::state::PadPreviewState;
use crate::state::{EditorLayout, EditorState};
use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, RULER_H};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudioAnalysis {
    #[serde(default)]
    pub bpm: f32,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub tempo: Vec<TempoPoint>,
    #[serde(default)]
    pub beats: Vec<f32>,
    #[serde(default)]
    pub downbeats: Vec<f32>,
    #[serde(default)]
    pub onsets: Vec<Onset>,
    #[serde(default)]
    pub energy_curve: Vec<EnergyPoint>,
    #[serde(default)]
    pub sections: Vec<AudioSection>,
    #[serde(default)]
    pub tempo_candidates: Vec<TempoCandidate>,
    #[serde(default)]
    pub rhythm: RhythmAnalysis,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TempoPoint {
    #[serde(default)]
    pub time_sec: f32,
    #[serde(default)]
    pub bpm: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RhythmAnalysis {
    #[serde(default)]
    pub grid_per_beat: u32,
    #[serde(default)]
    pub quantized_onsets: Vec<QuantizedOnset>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QuantizedOnset {
    #[serde(default)]
    pub time_sec: f32,
    #[serde(default)]
    pub step: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Onset {
    #[serde(default)]
    pub time_sec: f32,
    #[serde(default)]
    pub strength: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnergyPoint {
    #[serde(default)]
    pub time_sec: f32,
    #[serde(default)]
    pub value: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudioSection {
    #[serde(default)]
    pub start_sec: f32,
    #[serde(default)]
    pub end_sec: f32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub energy: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TempoCandidate {
    #[serde(default)]
    pub bpm: f32,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct AnalysisOverlay {
    pub analysis: AudioAnalysis,
    pub show_beats: bool,
    pub show_downbeats: bool,
    pub show_onsets: bool,
    pub show_energy: bool,
    pub show_grid: bool,
}

#[derive(Debug, Clone)]
pub struct MaiRisSettings {
    pub rhythm_note_scale: f32,
    pub hide_chart_hit_sfx: bool,
    pub hide_chart_answer_sfx: bool,
    pub use_rhythm_answer: bool,
    pub use_difficulty: bool,
    pub difficulty: f32,
}

impl Default for MaiRisSettings {
    fn default() -> Self {
        Self {
            rhythm_note_scale: 1.0,
            hide_chart_hit_sfx: false,
            hide_chart_answer_sfx: false,
            use_rhythm_answer: false,
            use_difficulty: true,
            difficulty: 10.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MaiRisState {
    pub overlay: Option<AnalysisOverlay>,
    pub settings: MaiRisSettings,
    pub cases: Vec<TestCase>,
    pub selected_case: Option<usize>,
    pub pending_load: Option<PathBuf>,
    pub analysis_path: Option<PathBuf>,
    /// Audio belonging to the loaded case, used when a generated test case
    /// needs its own copy. Kept separately because `chart_dir` may point at a
    /// chart file or at a run folder whose track is only a symlink.
    pub audio_source: Option<PathBuf>,
    pub status: Option<String>,
    saved_audio: Option<(bool, bool)>,
    last_rhythm_time: f32,
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub chart: PathBuf,
    pub analysis: Option<PathBuf>,
    pub audio: Option<PathBuf>,
    pub notes: usize,
    pub modified: u64,
}

impl AnalysisOverlay {
    /// Prefer MAI2_EDITOR_ANALYSIS, then chart_dir/analysis.json.
    pub fn load(chart_dir: &Path) -> Option<Self> {
        let path = std::env::var_os("MAI2_EDITOR_ANALYSIS")
            .map(PathBuf::from)
            .or_else(|| {
                let candidate = chart_dir.join("analysis.json");
                candidate.is_file().then_some(candidate)
            })?;
        Self::load_path(&path)
    }

    pub fn load_path(path: &Path) -> Option<Self> {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("viewer: read {}: {error}", path.display());
                return None;
            }
        };
        match serde_json::from_str::<AudioAnalysis>(&text) {
            Ok(analysis) => {
                eprintln!(
                    "viewer: loaded {} ({} beats, {} onsets, {:.1} BPM)",
                    path.display(),
                    analysis.beats.len(),
                    analysis.onsets.len(),
                    analysis.bpm
                );
                Some(Self {
                    analysis,
                    show_beats: true,
                    show_downbeats: true,
                    show_onsets: true,
                    show_energy: true,
                    show_grid: true,
                })
            }
            Err(error) => {
                eprintln!("viewer: parse {}: {error}", path.display());
                None
            }
        }
    }
}

/// Draw analysis lines on top of the existing timeline. Coordinates follow the
/// same logic as timeline::vertical and timeline::horizontal so the overlay
/// stays glued to the chart while the user seeks or zooms.
pub fn draw_timeline(
    overlay: &AnalysisOverlay,
    settings: &MaiRisSettings,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: mq::Rect,
    current_time: f32,
    scale: f32,
) {
    let g = scale.max(0.1);
    match editor.layout {
        EditorLayout::Vertical => {
            draw_timeline_vertical(overlay, settings, editor, chart, rect, current_time, g)
        }
        EditorLayout::Horizontal => {
            draw_timeline_horizontal(overlay, settings, editor, chart, rect, current_time, g)
        }
    }
}

fn draw_timeline_vertical(
    overlay: &AnalysisOverlay,
    settings: &MaiRisSettings,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: mq::Rect,
    current_time: f32,
    g: f32,
) {
    let plot_left = rect.x + LANE_GUTTER * g;
    let plot_right = rect.x + rect.w;
    let plot_top = rect.y + RULER_H * g;
    let plot_bottom = rect.y + rect.h;
    let judge_y = plot_bottom - JUDGE_MARGIN * g;
    let pps = editor.pixels_per_second * g;
    let now = reference_time(editor, current_time);
    let offset = chart.audio_offset;
    let y_of = |time: f32| judge_y - (time - offset - now) * pps;
    let in_plot = |y: f32| y >= plot_top - 1.0 && y <= plot_bottom + 1.0;

    for section in &overlay.analysis.sections {
        let y0 = y_of(section.start_sec);
        let y1 = y_of(section.end_sec);
        let top = y0.min(y1).clamp(plot_top, plot_bottom);
        let bottom = y0.max(y1).clamp(plot_top, plot_bottom);
        if bottom - top < 0.5 {
            continue;
        }
        let alpha = if section.label == "high" { 0.07 } else { 0.04 };
        mq::draw_rectangle(
            plot_left,
            top,
            (plot_right - plot_left).max(0.0),
            bottom - top,
            mq::Color::new(0.35, 0.65, 0.90, alpha),
        );
        if !section.label.is_empty() {
            mq::draw_text(
                &section.label,
                plot_left + 4.0,
                (top + 11.0).min(plot_bottom - 2.0),
                10.0,
                mq::Color::new(0.55, 0.62, 0.72, 0.9),
            );
        }
    }

    if overlay.show_energy && overlay.analysis.energy_curve.len() > 1 {
        let base_x = plot_right - 8.0;
        for pair in overlay.analysis.energy_curve.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            let ya = y_of(a.time_sec);
            let yb = y_of(b.time_sec);
            if !in_plot(ya) && !in_plot(yb) {
                continue;
            }
            let xa = base_x - a.value.clamp(0.0, 1.0) * 38.0;
            let xb = base_x - b.value.clamp(0.0, 1.0) * 38.0;
            mq::draw_line(
                xa,
                ya,
                xb,
                yb,
                1.4,
                mq::Color::new(0.36, 0.75, 0.74, 0.55),
            );
        }
    }

    if overlay.show_beats {
        for time in &overlay.analysis.beats {
            let y = y_of(*time);
            if !in_plot(y) {
                continue;
            }
            mq::draw_line(
                plot_left,
                y,
                plot_right,
                y,
                1.0,
                mq::Color::new(1.0, 1.0, 1.0, 0.055),
            );
        }
    }
    if overlay.show_downbeats {
        for time in &overlay.analysis.downbeats {
            let y = y_of(*time);
            if !in_plot(y) {
                continue;
            }
            mq::draw_line(
                plot_left,
                y,
                plot_right,
                y,
                1.0,
                mq::Color::new(0.47, 0.72, 0.96, 0.22),
            );
        }
    }
    if overlay.show_onsets {
        for onset in &overlay.analysis.onsets {
            let y = y_of(onset.time_sec);
            if !in_plot(y) {
                continue;
            }
            let scale = settings.rhythm_note_scale.clamp(0.5, 2.0);
            let length = (5.0 + onset.strength.clamp(0.0, 1.0) * 10.0) * scale;
            mq::draw_line(
                plot_right - 24.0,
                y,
                plot_right - 24.0 + length,
                y,
                1.2 * scale,
                mq::Color::new(1.0, 0.82, 0.40, 0.75),
            );
        }
    }

    if overlay.show_grid && overlay.analysis.rhythm.grid_per_beat > 0 {
        let scale = settings.rhythm_note_scale.clamp(0.5, 2.0);
        for onset in &overlay.analysis.rhythm.quantized_onsets {
            let y = y_of(onset.time_sec);
            if !in_plot(y) {
                continue;
            }
            mq::draw_line(
                plot_left,
                y,
                plot_left + 5.0 * scale,
                y,
                1.0 * scale,
                mq::Color::new(0.48, 0.85, 0.56, 0.8),
            );
        }
    }

    for point in overlay.analysis.tempo.iter().skip(1) {
        let y = y_of(point.time_sec);
        if !in_plot(y) {
            continue;
        }
        mq::draw_line(
            plot_left,
            y,
            plot_left + 18.0,
            y,
            1.5,
            mq::Color::new(0.96, 0.64, 0.38, 0.9),
        );
        mq::draw_text(
            &format!("{:.0}", point.bpm),
            plot_left + 22.0,
            y - 2.0,
            10.0,
            mq::Color::new(0.96, 0.64, 0.38, 0.9),
        );
    }

    let caption = format!(
        "{:.1} BPM  ·  {} beats  ·  {} tempo  ·  1/{} grid  ·  {}",
        overlay.analysis.bpm,
        overlay.analysis.beats.len(),
        overlay.analysis.tempo.len(),
        overlay.analysis.rhythm.grid_per_beat.max(1),
        if overlay.analysis.backend.is_empty() {
            "analysis"
        } else {
            &overlay.analysis.backend
        }
    );
    mq::draw_text(
        &caption,
        plot_left + 4.0,
        rect.y + 14.0 * g,
        11.0,
        mq::Color::new(0.62, 0.68, 0.78, 0.9),
    );
}

fn draw_timeline_horizontal(
    overlay: &AnalysisOverlay,
    settings: &MaiRisSettings,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: mq::Rect,
    _current_time: f32,
    g: f32,
) {
    let plot_left = rect.x + LANE_GUTTER * g;
    let plot_right = rect.x + rect.w;
    let plot_top = rect.y + RULER_H * g;
    let plot_bottom = rect.y + rect.h;
    let pps = editor.pixels_per_second * g;
    let offset = chart.audio_offset;
    let x_of = |time: f32| plot_left + (time - offset - editor.view_start) * pps;
    let in_plot = |x: f32| x >= plot_left - 1.0 && x <= plot_right + 1.0;

    for section in &overlay.analysis.sections {
        let x0 = x_of(section.start_sec).clamp(plot_left, plot_right);
        let x1 = x_of(section.end_sec).clamp(plot_left, plot_right);
        if x1 - x0 < 0.5 {
            continue;
        }
        let alpha = if section.label == "high" { 0.07 } else { 0.04 };
        mq::draw_rectangle(
            x0,
            plot_top,
            x1 - x0,
            (plot_bottom - plot_top).max(0.0),
            mq::Color::new(0.35, 0.65, 0.90, alpha),
        );
    }
    if overlay.show_beats {
        for time in &overlay.analysis.beats {
            let x = x_of(*time);
            if !in_plot(x) {
                continue;
            }
            mq::draw_line(
                x,
                plot_top,
                x,
                plot_bottom,
                1.0,
                mq::Color::new(1.0, 1.0, 1.0, 0.055),
            );
        }
    }
    if overlay.show_downbeats {
        for time in &overlay.analysis.downbeats {
            let x = x_of(*time);
            if !in_plot(x) {
                continue;
            }
            mq::draw_line(
                x,
                plot_top,
                x,
                plot_bottom,
                1.0,
                mq::Color::new(0.47, 0.72, 0.96, 0.22),
            );
        }
    }
    if overlay.show_onsets {
        for onset in &overlay.analysis.onsets {
            let x = x_of(onset.time_sec);
            if !in_plot(x) {
                continue;
            }
            let scale = settings.rhythm_note_scale.clamp(0.5, 2.0);
            let length = (5.0 + onset.strength.clamp(0.0, 1.0) * 10.0) * scale;
            mq::draw_line(
                x,
                plot_bottom - 24.0,
                x,
                plot_bottom - 24.0 - length,
                1.2 * scale,
                mq::Color::new(1.0, 0.82, 0.40, 0.75),
            );
        }
    }
    if overlay.show_grid && overlay.analysis.rhythm.grid_per_beat > 0 {
        let scale = settings.rhythm_note_scale.clamp(0.5, 2.0);
        for onset in &overlay.analysis.rhythm.quantized_onsets {
            let x = x_of(onset.time_sec);
            if !in_plot(x) {
                continue;
            }
            mq::draw_line(
                x,
                plot_bottom - 24.0,
                x,
                plot_bottom - 24.0 + 5.0 * scale,
                1.0 * scale,
                mq::Color::new(0.48, 0.85, 0.56, 0.8),
            );
        }
    }
    for point in overlay.analysis.tempo.iter().skip(1) {
        let x = x_of(point.time_sec);
        if !in_plot(x) {
            continue;
        }
        mq::draw_line(
            x,
            plot_top,
            x,
            plot_top + 18.0,
            1.5,
            mq::Color::new(0.96, 0.64, 0.38, 0.9),
        );
        mq::draw_text(
            &format!("{:.0}", point.bpm),
            x + 3.0,
            plot_top + 12.0,
            10.0,
            mq::Color::new(0.96, 0.64, 0.38, 0.9),
        );
    }
}

/// MaiRis pane: analysis summary, timeline visibility, rhythm marker size and
/// the chart/rhythm sound switches.
pub fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, app: &mut PadPreviewState) {
    let mut generate = false;
    {
    let MaiRisState {
        overlay,
        settings,
        cases,
        selected_case,
        pending_load,
        analysis_path: _,
        status,
        ..
    } = &mut editor.mairis;
    ui.heading("MaiRis");
    if let Some(overlay) = overlay.as_mut() {
        ui.label(format!(
            "{:.1} BPM  ·  {} tempo  ·  {} beats  ·  1/{} grid",
            overlay.analysis.bpm,
            overlay.analysis.tempo.len(),
            overlay.analysis.beats.len(),
            overlay.analysis.rhythm.grid_per_beat.max(1)
        ));
        if !overlay.analysis.backend.is_empty() {
            ui.label(format!("backend: {}", overlay.analysis.backend));
        }
        ui.separator();
        ui.label("Timeline");
        ui.checkbox(&mut overlay.show_beats, "显示拍点");
        ui.checkbox(&mut overlay.show_downbeats, "显示下拍");
        ui.checkbox(&mut overlay.show_onsets, "显示起音");
        ui.checkbox(&mut overlay.show_energy, "显示能量");
        ui.checkbox(&mut overlay.show_grid, "显示节奏网格");
        ui.add(
            egui::Slider::new(&mut settings.rhythm_note_scale, 0.5..=2.0)
                .text("节奏 note 大小"),
        );
        ui.separator();
    } else {
        ui.label("未加载 analysis.json");
        ui.label("设置 MAI2_EDITOR_ANALYSIS，或把 analysis.json 放进谱面目录。");
        ui.separator();
    }
    ui.label("声音");
    ui.checkbox(&mut settings.hide_chart_hit_sfx, "隐藏谱面击打音");
    ui.checkbox(&mut settings.hide_chart_answer_sfx, "隐藏谱面正解音");
    ui.checkbox(&mut settings.use_rhythm_answer, "使用解析节奏的正解音");
    ui.separator();
    ui.label("测试用例");
    ui.horizontal(|ui| {
        if ui.button("刷新用例").clicked() {
            *cases = discover_cases(&mairis_root());
            *status = Some(format!("{} cases", cases.len()));
        }
        if ui.button("生成测试").clicked() {
            generate = true;
        }
    });
    ui.checkbox(&mut settings.use_difficulty, "按难度生成");
    if settings.use_difficulty {
        ui.add(
            egui::Slider::new(&mut settings.difficulty, 7.0..=15.0)
                .step_by(0.5)
                .text("难度"),
        );
    }
    if cases.is_empty() {
        *cases = discover_cases(&mairis_root());
    }
    if selected_case.is_none() && !cases.is_empty() {
        *selected_case = cases.iter().position(|case| case.audio.is_some()).or(Some(0));
    }
    if ui.button("加载并播放").clicked() {
        if let Some(index) = *selected_case {
            if let Some(case) = cases.get(index) {
                *pending_load = Some(case.chart.clone());
            }
        }
    }
    let mut selected = *selected_case;
    let newest = cases
        .iter()
        .enumerate()
        .filter(|(_, case)| case.audio.is_some())
        .max_by_key(|(_, case)| case.modified)
        .map(|(index, _)| index)
        .or(Some(0));
    for (index, case) in cases.iter().enumerate() {
        let newest_mark = if Some(index) == newest { "[最新] " } else { "" };
        let audio = if case.audio.is_some() { "" } else { "[无音频] " };
        let label = format!(
            "{}. {}{}{}  ·  {} notes",
            index + 1,
            newest_mark,
            audio,
            case.name,
            case.notes
        );
        if ui
            .selectable_label(selected == Some(index), label)
            .clicked()
        {
            selected = Some(index);
        }
    }
    *selected_case = selected;
    if let Some(index) = *selected_case {
        if let Some(case) = cases.get(index) {
            ui.label(case.chart.display().to_string());
        }
    }
    if let Some(status) = status {
        ui.label(status.clone());
    }
    ui.label(format!("播放状态: {:?}", app.mode));
    }
    if generate {
        spawn_generation(editor);
    }
}

/// Keep the player's sound flags in sync with the MaiRis pane. Values are
/// captured on entry and restored when the viewer workspace is left, so the
/// Chart workspace keeps its own sound settings.
pub fn apply_audio_options(app: &mut PadPreviewState, state: &mut MaiRisState, active: bool) {
    if active {
        let (base_hit, base_answer) = *state
            .saved_audio
            .get_or_insert((app.params.hit_sfx, app.params.answer_sfx));
        let hit = base_hit && !state.settings.hide_chart_hit_sfx;
        let answer = base_answer && !state.settings.hide_chart_answer_sfx;
        if app.params.hit_sfx != hit || app.params.answer_sfx != answer {
            app.params.hit_sfx = hit;
            app.params.answer_sfx = answer;
            crate::app::params::set(app.params.clone());
        }
    } else if let Some((hit, answer)) = state.saved_audio.take() {
        app.params.hit_sfx = hit;
        app.params.answer_sfx = answer;
        crate::app::params::set(app.params.clone());
    }
}

/// Play answer.wav on the analysed beat grid. Only forward playback within one
/// frame is considered, so seeking and pausing never machine-gun the sound.
pub fn tick_rhythm_answer(
    app: &mut PadPreviewState,
    state: &mut MaiRisState,
    current_time: f32,
) {
    let previous = state.last_rhythm_time;
    state.last_rhythm_time = current_time;
    if !state.settings.use_rhythm_answer
        || app.mode != Mode::Playing
        || current_time <= previous
        || current_time - previous > 0.5
    {
        return;
    }
    let Some(overlay) = state.overlay.as_ref() else {
        return;
    };
    let mut played = 0;
    for beat in &overlay.analysis.beats {
        if *beat > previous + 1e-4 && *beat <= current_time + 1e-4 {
            app.play_answer();
            played += 1;
            if played >= 3 {
                break;
            }
        }
    }
}

pub fn load_case(editor: &mut EditorState, app: &mut PadPreviewState, path: &Path) {
    let source = match crate::runtime::load_chart_into(app, path, None) {
        Ok(source) => source,
        Err(error) => {
            editor.status = Some(format!("MaiRis load failed: {error}"));
            return;
        }
    };
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
    };
    let audio = resolve_case_audio(&dir);
    editor.chart_dir = dir.display().to_string();
    editor.source = Some(source);
    editor.level_slots.clear();
    editor.level_index = 0;
    editor.workspace = crate::state::Workspace::Viewer;
    editor.hide_chart_tools = true;
    let analysis = Some(dir.join("analysis.json")).filter(|item| item.is_file());
    editor.mairis.analysis_path = analysis.clone();
    editor.mairis.audio_source = audio.clone();
    if let Some(analysis) = analysis {
        if let Some(overlay) = AnalysisOverlay::load_path(&analysis) {
            editor.mairis.overlay = Some(overlay);
        }
    }
    editor.status = Some(format!("MaiRis case: {}", path.display()));
    app.start_playback_at(0.0);
}

pub fn mairis_root() -> PathBuf {
    std::env::var_os("MAIRIS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../MaiRis"))
}

pub fn discover_cases(root: &Path) -> Vec<TestCase> {
    let mut cases = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("runs")) else {
        return cases;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let chart = dir.join("chart.json");
        if !chart.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&chart) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let notes = value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .map_or(0, Vec::len);
        let analysis = dir.join("analysis.json");
        let audio = resolve_case_audio(&dir);
        let modified = fs::metadata(&chart)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());
        cases.push(TestCase {
            name: dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("run")
                .to_string(),
            chart,
            analysis: analysis.is_file().then_some(analysis),
            audio,
            notes,
            modified,
        });
    }
    // A case without audio cannot be auditioned; keep those at the bottom so
    // the first row is always something the load-and-play button can use.
    cases.sort_by(|a, b| {
        a.audio
            .is_none()
            .cmp(&b.audio.is_none())
            .then_with(|| b.modified.cmp(&a.modified))
            .then_with(|| b.name.cmp(&a.name))
    });
    cases
}

/// Find the track next to a chart, following either the conventional file
/// names or the absolute/relative path recorded in the run's config.json.
pub fn resolve_case_audio(dir: &Path) -> Option<PathBuf> {
    for name in ["track.mp3", "track.wav", "track.ogg", "track.flac"] {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    let config = fs::read_to_string(dir.join("config.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&config).ok()?;
    for key in ["audio", "track", "audio_path"] {
        let Some(text) = value.get(key).and_then(|item| item.as_str()) else {
            continue;
        };
        let candidate = PathBuf::from(text);
        let candidate = if candidate.is_absolute() {
            candidate
        } else {
            dir.join(candidate)
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn spawn_generation(editor: &mut EditorState) {
    let Some(analysis) = editor.mairis.analysis_path.clone() else {
        editor.status = Some("MaiRis: load an analysis.json first".to_string());
        return;
    };
    let root = mairis_root();
    let script = root.join("tools/generate_patterns.py");
    let Some(blocks) = latest_blocks(&root) else {
        editor.status = Some("MaiRis: no runs/pattern-*/blocks.json".to_string());
        return;
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let out = root.join("runs").join(format!("test-{stamp}"));
    if let Err(error) = fs::create_dir_all(&out) {
        editor.status = Some(format!("MaiRis: create {}: {error}", out.display()));
        return;
    }
    let _ = fs::copy(&analysis, out.join("analysis.json"));
    let chart_path = PathBuf::from(&editor.chart_dir);
    let source_dir = if chart_path.is_dir() {
        chart_path
    } else {
        chart_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };
    let audio = editor
        .mairis
        .audio_source
        .clone()
        .filter(|path| path.is_file())
        .or_else(|| resolve_case_audio(&source_dir));
    let Some(audio) = audio else {
        editor.status = Some(format!(
            "MaiRis: no track found next to {}; generated case would be silent",
            source_dir.display()
        ));
        return;
    };
    let extension = audio
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp3");
    let target = out.join(format!("track.{extension}"));
    if let Err(error) = fs::copy(&audio, &target) {
        editor.status = Some(format!(
            "MaiRis: copy {} -> {}: {error}",
            audio.display(),
            target.display()
        ));
        return;
    }
    let python = resolve_python();
    let probability = analysis
        .parent()
        .map(|dir| dir.join("probabilities.json"))
        .filter(|path| path.is_file());
    let wrapper = root.join("tools/generate_with_onset_model.sh");
    let mut command;
    if wrapper.is_file() {
        // The wrapper scores onsets with the trained selector (nix-shell has
        // scikit-learn) and then calls the normal generator with the scores.
        command = Command::new("bash");
        command
            .arg(&wrapper)
            .arg(&analysis)
            .arg(&blocks)
            .arg(&out)
            .arg(probability.clone().unwrap_or_else(|| PathBuf::from("-")));
        command.arg(if probability.is_some() { "0.50" } else { "0.10" });
        command
            .arg("raw")
            .arg(stamp.to_string())
            .arg(&audio)
            .env("MAIRIS_PYTHON", &python);
    } else {
        command = Command::new(&python);
        command.arg(&script).arg(&analysis).arg(blocks).arg(&out);
        if let Some(probability) = &probability {
            command.arg(probability).arg("0.50");
        } else {
            command.arg("-").arg("0.10");
        }
        command.arg("raw").arg(stamp.to_string());
    }
    if editor.mairis.settings.use_difficulty {
        command
            .arg("--difficulty")
            .arg(format!("{:.1}", editor.mairis.settings.difficulty));
    }
    match command.spawn() {
        Ok(_) => {
            editor.status = Some(format!("MaiRis generating {}", out.display()));
            editor.mairis.status = Some("generation started".to_string());
        }
        Err(error) => {
            editor.status = Some(format!("MaiRis generator: {error}"));
        }
    }
}

/// Find a Python interpreter without relying on the GUI process having the
/// user's shell PATH. The generator only uses the standard library, so any
/// Nix Python environment is sufficient.
fn resolve_python() -> String {
    if let Some(path) = std::env::var_os("MAIRIS_PYTHON") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path.display().to_string();
        }
    }
    let mut candidates: Vec<(u32, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir("/nix/store") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(rest) = name.split("-python3-").nth(1) else {
                continue;
            };
            if !rest.ends_with("-env") {
                continue;
            }
            let version = rest
                .trim_end_matches("-env")
                .split('.')
                .take(2)
                .filter_map(|part| part.parse::<u32>().ok())
                .fold(0, |score, part| score * 100 + part);
            let path = entry.path().join("bin/python3");
            if path.is_file() {
                candidates.push((version, path.display().to_string()));
            }
        }
    }
    candidates.sort();
    candidates
        .pop()
        .map(|(_, path)| path)
        .unwrap_or_else(|| "python3".to_string())
}

fn latest_blocks(root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = fs::read_dir(root.join("runs"))
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("blocks.json"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_analysis_json() {
        let text = r#"{
          "bpm": 188.0,
          "backend": "dsp-v1",
          "beats": [0.0, 0.319, 0.638],
          "downbeats": [0.0],
          "onsets": [{"time_sec": 0.1, "strength": 0.5}],
          "energy_curve": [{"time_sec": 0.0, "value": 0.4}],
          "sections": [{"start_sec": 0.0, "end_sec": 8.0, "label": "mid", "energy": 0.5}],
          "tempo_candidates": [{"bpm": 188.0, "confidence": 0.7}]
        }"#;
        let analysis: AudioAnalysis = serde_json::from_str(text).expect("parse");
        assert_eq!(analysis.beats.len(), 3);
        assert_eq!(analysis.onsets.len(), 1);
        assert_eq!(analysis.sections[0].label, "mid");
    }

    #[test]
    fn resolves_audio_recorded_in_config() {
        let dir = std::env::temp_dir().join(format!(
            "mairis-audio-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("track.ogg"), b"").expect("audio");
        fs::write(
            dir.join("config.json"),
            r#"{"audio":"track.ogg"}"#,
        )
        .expect("config");
        let audio = resolve_case_audio(&dir).expect("audio path");
        assert_eq!(audio, dir.join("track.ogg"));
        let _ = fs::remove_dir_all(&dir);
    }

}

