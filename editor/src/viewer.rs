//! Read-only audio-analysis overlay for Workspace::Viewer.
//!
//! The overlay consumes the AudioAnalysis JSON produced by MaiRis. It does not
//! own any chart or judgment state: the editor keeps using the same
//! PadPreviewState, timeline and pad renderer as the Chart workspace.

use std::fs;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use egui_macroquad::egui;
use macroquad::prelude as mq;
use serde::Deserialize;

use crate::app::types::{ChartDoc, Mode, Note};
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
    /// Audio belonging to the current chart, replaced on every document load.
    /// Generation resolves the current chart directory again so stale cached
    /// paths can never select another song.
    pub audio_source: Option<PathBuf>,
    pub status: Option<String>,
    pub annotation: AnnotationState,
    saved_audio: Option<(bool, bool)>,
    last_rhythm_time: f32,
}

impl MaiRisState {
    /// Replace document-specific assets together, including when the new chart
    /// has no analysis. An environment override is only supplied at startup.
    pub fn load_chart_assets(&mut self, chart: &Path, analysis_override: Option<PathBuf>) {
        let dir = chart_directory(chart);
        self.audio_source = resolve_case_audio(dir);
        let analysis = analysis_override.unwrap_or_else(|| dir.join("analysis.json"));
        self.analysis_path = analysis.is_file().then_some(analysis);
        self.overlay = self.analysis_path.as_deref().and_then(AnalysisOverlay::load_path);
        self.status = None;
        self.last_rhythm_time = 0.0;
    }
}

fn chart_directory(path: &Path) -> &Path {
    if path.is_dir() { path } else { path.parent().unwrap_or_else(|| Path::new(".")) }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnnotationPhase {
    #[default]
    Annotating,
    Review,
    Confirmed,
}

#[derive(Debug, Clone, Default)]
pub struct AnnotationState {
    pub active: bool,
    pub phase: AnnotationPhase,
    pub song_id: Option<String>,
    pub difficulty: f32,
    pub original: Vec<Note>,
    pub original_ids: HashSet<u64>,
    original_by_id: HashMap<u64, usize>,
    pub preview_signature: u64,
    pub review_deleted: Vec<Note>,
    pub review_added: Vec<Note>,
    pub review_moved: Vec<MovedNote>,
    pub moved_ids: HashSet<u64>,
    pub selected_ghost: Option<u64>,
    pub ghost_pointer_down: bool,
    pub confirm_pending: bool,
    pub pending: Option<PathBuf>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MovedNote {
    pub original: Note,
    pub current: Note,
}

impl AnnotationState {
    pub fn is_editing(&self) -> bool {
        self.active && self.pending.is_none() && self.phase == AnnotationPhase::Annotating
    }

    pub fn set_original(&mut self, notes: &[Note]) {
        self.original = notes.to_vec();
        self.original_ids = notes.iter().map(|note| note.id).collect();
        self.original_by_id = notes.iter().enumerate().map(|(i, note)| (note.id, i)).collect();
        self.preview_signature = 0;
        self.review_deleted.clear();
        self.review_added.clear();
        self.review_moved.clear();
        self.moved_ids.clear();
        self.selected_ghost = None;
        self.ghost_pointer_down = false;
    }

    pub fn sync_preview(&mut self, notes: &[Note]) {
        if !self.active || self.pending.is_some() { return; }
        let signature = notes.iter().fold(1469598103934665603_u64, |state, note| {
            (state ^ note.id ^ (note.time.to_bits() as u64) << 17 ^ (note.lane as u64) << 3)
                .wrapping_mul(1099511628211)
        });
        if signature == self.preview_signature { return; }
        self.preview_signature = signature;
        let current_ids: HashSet<_> = notes.iter().map(|note| note.id).collect();
        self.review_deleted = self.original.iter()
            .filter(|note| !current_ids.contains(&note.id)).cloned().collect();
        self.review_added.clear();
        self.review_moved.clear();
        self.moved_ids.clear();
        for current in notes {
            if let Some(&index) = self.original_by_id.get(&current.id) {
                let original = &self.original[index];
                if original.time != current.time || original.lane != current.lane {
                    self.moved_ids.insert(current.id);
                    self.review_moved.push(MovedNote { original: original.clone(), current: current.clone() });
                }
            } else {
                self.review_added.push(current.clone());
            }
        }
        if self.selected_ghost.is_some_and(|id| current_ids.contains(&id)) {
            self.selected_ghost = None;
            self.ghost_pointer_down = false;
        }
    }
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
    let mut start_annotation = false;
    let mut finish_annotation = false;
    let mut next_annotation = false;
    let mut cancel_annotation = false;
    let mut restore_ghost = false;
    poll_annotation(editor, app);
    editor.mairis.annotation.sync_preview(&app.chart.notes);
    {
    let MaiRisState {
        overlay,
        settings,
        cases,
        selected_case,
        pending_load,
        analysis_path: _,
        status,
        annotation,
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
        ui.label("生成测试会分析当前谱面的音频；也可在谱面目录放入 analysis.json。");
        ui.separator();
    }
    ui.label("声音");
    ui.checkbox(&mut settings.hide_chart_hit_sfx, "隐藏谱面击打音");
    ui.checkbox(&mut settings.hide_chart_answer_sfx, "隐藏谱面正解音");
    ui.checkbox(&mut settings.use_rhythm_answer, "使用解析节奏的正解音");
    ui.separator();
    ui.label("标注模式");
    if !annotation.active {
        if ui.button("随机训练歌 + 开始标注").clicked() {
            start_annotation = true;
        }
    } else {
        if let Some(song) = &annotation.song_id {
            ui.label(song);
        }
        match annotation.phase {
            AnnotationPhase::Annotating => {
                ui.label("绿色=新增，红色=删除，橙色=移动（原位置保留影子）");
                ui.label(format!("删除 {} · 新增 {} · 移动 {}",
                    annotation.review_deleted.len(), annotation.review_added.len(), annotation.review_moved.len()));
                if ui.add_enabled(annotation.is_editing(), egui::Button::new("完成标注")).clicked() {
                    finish_annotation = true;
                }
            }
            AnnotationPhase::Review => {
                ui.label(format!(
                    "删除 {} · 新增 {} · 移动 {}（绿色=新增，红色=删除，橙色=移动）",
                    annotation.review_deleted.len(),
                    annotation.review_added.len(),
                    annotation.review_moved.len()
                ));
                ui.label("待确认，尚未保存标注结果");
                if ui.button("查看结果并确认").clicked() {
                    annotation.confirm_pending = true;
                }
                if ui.button("返回修改").clicked() { resume_annotation(annotation); }
            }
            AnnotationPhase::Confirmed => {
                ui.label("标注已确认保存");
                if ui.button("返回修改").clicked() { resume_annotation(annotation); }
                if ui.button("下一组").clicked() {
                    next_annotation = true;
                }
            }
        }
        if !annotation.review_deleted.is_empty() {
            ui.label("删除幽灵（点击选中）");
            egui::ScrollArea::vertical()
                .id_salt("annotation-deleted-notes")
                .max_height(120.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for note in &annotation.review_deleted {
                        let time = crate::app::types::note_secs(note, &app.chart.bpms);
                        let label = format!("#{:?}  {:.2}s  lane {}", note.id, time, note.lane);
                        if ui
                            .selectable_label(annotation.selected_ghost == Some(note.id), label)
                            .clicked()
                        {
                            annotation.selected_ghost = Some(note.id);
                            editor.selected.clear();
                            editor.keyboard.candidate = None;
                            editor.pending_seek = Some(time.max(0.0));
                        }
                    }
                });
            if annotation.is_editing() {
                ui.label("幽灵可点击选中；Delete / Backspace 恢复，不能拖动");
                if annotation.selected_ghost.is_some() && ui.button("恢复选中幽灵").clicked() {
                    restore_ghost = true;
                }
            }
        }
        if let Some(note) = &annotation.status {
            ui.label(note);
        }
        if ui.button("退出标注").clicked() {
            cancel_annotation = true;
        }
    }
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
    if ui.add_enabled(!annotation.active, egui::Button::new("加载并播放")).clicked() {
        if let Some(index) = *selected_case {
            if let Some(case) = cases.get(index) {
                *pending_load = Some(case.chart.clone());
            }
        }
    }
    if annotation.active {
        ui.small("退出标注后可加载其他测试用例");
    }
    let mut selected = *selected_case;
    egui::ScrollArea::vertical()
        .id_salt("mairis-test-cases")
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
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
        });
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
    if start_annotation || next_annotation {
        start_annotation_round(editor, app);
    }
    if finish_annotation {
        finish_annotation_round(editor, app);
    }
    if cancel_annotation {
        editor.mairis.annotation = AnnotationState::default();
        editor.hide_chart_tools = true;
        editor.tool = crate::state::Tool::Select;
    }
    if restore_ghost {
        restore_selected_ghost(editor, &mut app.chart);
    }
}

fn start_annotation_round(editor: &mut EditorState, app: &mut PadPreviewState) {
    let root = mairis_root();
    let songs_path = root.join("runs/onset-all-1000-songs-20261022/songs.jsonl");
    let Ok(text) = fs::read_to_string(&songs_path) else {
        editor.mairis.annotation.status =
            Some(format!("missing {}", songs_path.display()));
        return;
    };
    let songs: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    if songs.is_empty() {
        editor.mairis.annotation.status = Some("training song list is empty".to_string());
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0) as u64;
    let song = &songs[(stamp as usize) % songs.len()];
    let Some(audio) = song.get("audio").and_then(|value| value.as_str()) else {
        editor.mairis.annotation.status = Some("song has no audio".to_string());
        return;
    };
    let song_id = song
        .get("song_id")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();
    let out = root.join("runs").join(format!("annotation-{stamp}"));
    let script = root.join("tools/generate_song.sh");
    let difficulty = editor.mairis.settings.difficulty;
    let result = Command::new("bash")
        .arg(&script)
        .arg(audio)
        .arg(&out)
        .arg(format!("{difficulty:.1}"))
        .arg(stamp.to_string())
        .spawn();
    match result {
        Ok(_) => {
            editor.mairis.annotation = AnnotationState {
                active: true,
                phase: AnnotationPhase::Annotating,
                song_id: Some(song_id),
                difficulty,
                pending: Some(out.join("chart.json")),
                status: Some("generating…".to_string()),
                ..AnnotationState::default()
            };
            editor.mairis.status = Some(format!("annotation: generating {}", out.display()));
        }
        Err(error) => {
            editor.mairis.annotation.status = Some(format!("generator: {error}"));
        }
    }
    let _ = app;
}

fn poll_annotation(editor: &mut EditorState, app: &mut PadPreviewState) {
    let Some(path) = editor.mairis.annotation.pending.clone() else {
        return;
    };
    if !path.is_file() {
        return;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    match crate::runtime::load_chart_into(app, &dir, None) {
        Ok(source) => {
            editor.chart_dir = dir.display().to_string();
            editor.source = Some(source);
            editor.level_slots.clear();
            editor.level_index = 0;
            editor.hide_chart_tools = false;
            editor.tool = crate::state::Tool::Eraser;
            editor.mairis.load_chart_assets(&dir, None);
            editor.mairis.annotation.set_original(&app.chart.notes);
            editor.mairis.annotation.phase = AnnotationPhase::Annotating;
            editor.mairis.annotation.pending = None;
            editor.mairis.annotation.review_deleted.clear();
            editor.mairis.annotation.review_added.clear();
            editor.undo.clear();
            editor.redo.clear();
            editor.selected.clear();
            editor.hovered = None;
            editor.keyboard.reset_document();
            editor.selection_transform = None;
            editor.last_edit = None;
            editor.placement_sequence.clear();
            editor.transform_pointer_down = false;
            editor.star_placement = None;
            editor.hold_placement = None;
            editor.pointer_press = None;
            editor.timeline_pan = None;
            editor.dragging = None;
            editor.drag_grab = None;
            editor.handle_grab = None;
            editor.marquee_start = None;
            editor.scrubbing = false;
            editor.drag_history_pushed = false;
            editor.drag_multi_orig.clear();
            editor.drag_orig_notes.clear();
            editor.editing_slide = None;
            editor.pending_shape = None;
            editor.simai_editor.reset();
            editor.dirty = false;
            editor.mairis.annotation.status = Some("标注中：Erase 删除 / Place 补音".to_string());
            editor.mairis.status = Some(format!("annotation: {}", path.display()));
            editor.reload_waveform = true;
            app.start_playback_at(0.0);
        }
        Err(error) => {
            editor.mairis.annotation.pending = None;
            editor.mairis.annotation.status = Some(format!("load failed: {error}"));
        }
    }
}

fn finish_annotation_round(editor: &mut EditorState, app: &mut PadPreviewState) {
    if !editor.mairis.annotation.is_editing() { return; }
    editor.mairis.annotation.sync_preview(&app.chart.notes);
    editor.mairis.annotation.phase = AnnotationPhase::Review;
    editor.mairis.annotation.confirm_pending = true;
    editor.mairis.annotation.status = Some("请复核彩色 diff，确认后保存".into());
    editor.tool = crate::state::Tool::Select;
}

fn resume_annotation(annotation: &mut AnnotationState) {
    annotation.phase = AnnotationPhase::Annotating;
    annotation.confirm_pending = false;
    annotation.status = Some("继续修改；完成后重新确认保存".into());
}

/// Finishing opens a review dialog. Feedback only becomes a confirmed training
/// artifact after an explicit save; closing the dialog leaves review pending.
pub fn annotation_confirmation(ctx: &egui::Context, editor: &mut EditorState, app: &mut PadPreviewState) {
    if !editor.mairis.annotation.confirm_pending { return; }
    let mut open = true;
    let mut revise = false;
    let mut confirm = false;
    let mut next = false;
    egui::Window::new("确认标注结果")
        .id(egui::Id::new("mairis-annotation-confirmation"))
        .open(&mut open)
        .collapsible(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let annotation = &editor.mairis.annotation;
            ui.label(annotation.song_id.as_deref().unwrap_or("当前曲目"));
            ui.label(format!("删除 {} · 新增 {} · 移动 {}",
                annotation.review_deleted.len(), annotation.review_added.len(), annotation.review_moved.len()));
            ui.label("核实时间轴上的颜色和以下修改；确认后保存反馈及修正谱面。");
            egui::ScrollArea::vertical().id_salt("annotation-review-results")
                .max_height(240.0).show(ui, |ui| {
                    for note in &annotation.review_deleted {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("删除 #{}, {:.3}s · 轨道 {}",
                            note.id, crate::app::types::note_secs(note, &app.chart.bpms), note.lane));
                    }
                    for note in &annotation.review_added {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, format!("新增 #{}, {:.3}s · 轨道 {}",
                            note.id, crate::app::types::note_secs(note, &app.chart.bpms), note.lane));
                    }
                    for moved in &annotation.review_moved {
                        ui.colored_label(egui::Color32::from_rgb(255, 184, 64), format!(
                            "移动 #{}: {:.3}s / {} → {:.3}s / {}",
                            moved.current.id,
                            crate::app::types::note_secs(&moved.original, &app.chart.bpms), moved.original.lane,
                            crate::app::types::note_secs(&moved.current, &app.chart.bpms), moved.current.lane));
                    }
                });
            if let Some(status) = &annotation.status { ui.label(status); }
            ui.horizontal_wrapped(|ui| {
                revise = ui.button("返回修改").clicked();
                confirm = ui.button("确认并保存").clicked();
                next = ui.button("确认保存并下一组").clicked();
            });
        });
    if revise {
        resume_annotation(&mut editor.mairis.annotation);
    } else if confirm || next {
        match save_annotation_result(Path::new(&editor.chart_dir), &editor.mairis.annotation, &app.chart) {
            Ok(()) => {
                editor.mairis.annotation.phase = AnnotationPhase::Confirmed;
                editor.mairis.annotation.confirm_pending = false;
                editor.mairis.annotation.status = Some(format!("已确认保存：{}/feedback.jsonl", editor.chart_dir));
                if next { start_annotation_round(editor, app); }
            }
            Err(error) => editor.mairis.annotation.status = Some(format!("保存失败：{error}，请重试")),
        }
    } else if !open {
        editor.mairis.annotation.confirm_pending = false;
    }
}

fn save_annotation_result(dir: &Path, annotation: &AnnotationState, chart: &ChartDoc) -> Result<(), String> {
    let secs = |note: &Note| crate::app::types::note_secs(note, &chart.bpms);
    let mut lines = Vec::new();
    for note in &annotation.review_deleted {
        lines.push(serde_json::json!({
            "action": "delete", "note_id": note.id,
            "song_id": annotation.song_id, "difficulty": annotation.difficulty,
            "time_sec": secs(note), "lane": note.lane,
            "old_time_sec": secs(note), "new_time_sec": null,
            "old_lane": note.lane, "new_lane": null,
        }));
    }
    for note in &annotation.review_added {
        lines.push(serde_json::json!({
            "action": "add", "note_id": note.id,
            "song_id": annotation.song_id, "difficulty": annotation.difficulty,
            "time_sec": secs(note), "lane": note.lane,
            "old_time_sec": null, "new_time_sec": secs(note),
            "old_lane": null, "new_lane": note.lane,
        }));
    }
    for moved in &annotation.review_moved {
        lines.push(serde_json::json!({
            "action": "move", "note_id": moved.current.id,
            "song_id": annotation.song_id, "difficulty": annotation.difficulty,
            "time_sec": secs(&moved.current), "lane": moved.current.lane,
            "old_time_sec": secs(&moved.original), "new_time_sec": secs(&moved.current),
            "old_lane": moved.original.lane, "new_lane": moved.current.lane,
        }));
    }
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    let mut original = chart.clone();
    original.notes = annotation.original.clone();
    let snapshots = [
        ("original.chart.json", serde_json::to_vec_pretty(&original).map_err(|e| e.to_string())?),
        ("annotated.chart.json", serde_json::to_vec_pretty(chart).map_err(|e| e.to_string())?),
        ("feedback.jsonl", text.into_bytes()),
    ];
    for (name, data) in snapshots {
        let temporary = dir.join(format!(".{name}.tmp"));
        fs::write(&temporary, data).and_then(|()| fs::rename(&temporary, dir.join(name)))
            .map_err(|error| format!("{name}: {error}"))?;
    }
    Ok(())
}

pub(crate) fn restore_selected_ghost(editor: &mut EditorState, chart: &mut ChartDoc) {
    if !editor.mairis.annotation.is_editing() { return; }
    let Some(id) = editor.mairis.annotation.selected_ghost else { return; };
    let Some(note) = editor.mairis.annotation.review_deleted.iter().find(|note| note.id == id).cloned() else { return; };
    if chart.notes.iter().any(|current| current.id == id) { return; }
    editor.push_history(chart);
    chart.notes.push(note);
    chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    editor.mairis.annotation.selected_ghost = None;
    editor.mairis.annotation.ghost_pointer_down = false;
    editor.mairis.annotation.sync_preview(&chart.notes);
    editor.select_only(id);
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
    let dir = chart_directory(path).to_path_buf();
    editor.chart_dir = dir.display().to_string();
    editor.source = Some(source);
    editor.level_slots.clear();
    editor.level_index = 0;
    editor.workspace = crate::state::Workspace::Viewer;
    editor.hide_chart_tools = true;
    editor.mairis.load_chart_assets(&dir, None);
    editor.mairis.annotation = AnnotationState::default();
    editor.reload_waveform = true;
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
    for name in ["track.mp3", "track.wav", "demo.mp3", "demo.wav", "track.ogg", "track.flac"] {
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
    let root = mairis_root();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let out = root.join("runs").join(format!("test-{stamp}"));
    let (mut command, audio) = match generation_command(editor, &root, &out, &stamp.to_string()) {
        Ok(request) => request,
        Err(error) => {
            editor.mairis.status = Some(error.clone());
            editor.status = Some(error);
            return;
        }
    };
    match command.spawn() {
        Ok(_) => {
            editor.status = Some(format!("MaiRis generating {}", out.display()));
            editor.mairis.status = Some(format!("正在分析并生成：{}", audio.display()));
        }
        Err(error) => {
            let message = format!("MaiRis generator: {error}");
            editor.mairis.status = Some(message.clone());
            editor.status = Some(message);
        }
    }
}

/// Start from the current document's audio, never a previous case's cached
/// audio/analysis/probabilities. Each unique output directory gets fresh analysis.
fn generation_command(
    editor: &EditorState, root: &Path, out: &Path, seed: &str,
) -> Result<(Command, PathBuf), String> {
    let chart_path = Path::new(&editor.chart_dir);
    let dir = chart_directory(chart_path);
    let audio = resolve_case_audio(dir)
        .ok_or_else(|| format!("MaiRis: 当前谱面目录没有音频：{}", dir.display()))?;
    let script = root.join("tools/generate_song.sh");
    if !script.is_file() {
        return Err(format!("MaiRis: missing {}", script.display()));
    }
    let blocks = latest_blocks(root)
        .ok_or_else(|| "MaiRis: no runs/pattern-*/blocks.json".to_string())?;
    let difficulty = if editor.mairis.settings.use_difficulty {
        format!("{:.1}", editor.mairis.settings.difficulty)
    } else {
        "-".to_string()
    };
    let mut command = Command::new("bash");
    command.arg(script).arg(&audio).arg(out).arg(difficulty).arg(seed)
        .env("MAIRIS_PYTHON", resolve_python())
        .env("MAIRIS_BLOCKS", blocks);
    Ok((command, audio))
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

    fn annotation_fixture() -> (EditorState, ChartDoc) {
        let chart = crate::app::maidata::from_maidata(
            "&inote_1=(120){4}1,2,(240)3,4,E", None,
        ).unwrap();
        let mut editor = EditorState::default();
        editor.mairis.annotation.active = true;
        editor.mairis.annotation.song_id = Some("annotation-test".into());
        editor.mairis.annotation.difficulty = 13.5;
        editor.mairis.annotation.set_original(&chart.notes);
        (editor, chart)
    }

    struct FeedbackDir(PathBuf);

    impl FeedbackDir {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("mairis-feedback-{}-{stamp}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for FeedbackDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    fn test_song(root: &Path, name: &str, sample: i16, bpm: Option<f32>) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("maidata.txt"), "&inote_1=(120){4}1,2,E").unwrap();
        let mut wav = hound::WavWriter::create(dir.join("track.wav"), hound::WavSpec {
            channels: 1, sample_rate: 44100, bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }).unwrap();
        for _ in 0..441 { wav.write_sample(sample).unwrap(); }
        wav.finalize().unwrap();
        if let Some(bpm) = bpm {
            fs::write(dir.join("analysis.json"), format!("{{\"bpm\":{bpm}}}")).unwrap();
        }
        dir
    }

    fn test_player() -> PadPreviewState {
        let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1,E", None).unwrap();
        let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
        app.forced_time = Some(0.0);
        app
    }

    #[test]
    fn reopening_chart_uses_new_audio_for_generation_even_with_stale_cached_paths() {
        let temp = FeedbackDir::new();
        let old = test_song(&temp.0, "old song", 100, Some(120.0));
        let new = test_song(&temp.0, "新 song", 900, Some(180.0));
        let mut app = test_player();
        let mut editor = EditorState::default();
        crate::load_editor_chart(&mut editor, &mut app, &old, None);
        editor.mairis.annotation.active = true;
        editor.reload_waveform = false;
        crate::load_editor_chart(&mut editor, &mut app, &new.join("maidata.txt"), None);
        assert_eq!(editor.mairis.audio_source, Some(new.join("track.wav")));
        assert_eq!(editor.mairis.analysis_path, Some(new.join("analysis.json")));
        assert_eq!(editor.mairis.overlay.as_ref().unwrap().analysis.bpm, 180.0);
        assert!(editor.reload_waveform);
        assert!(!editor.mairis.annotation.active);
        assert_eq!(app.audio_wav_pcm.as_ref().unwrap().samples[0], 900);

        // Reproduce stale state from an older editor session. Generation must
        // still derive its input from the open document, including file paths.
        editor.mairis.audio_source = Some(old.join("track.wav"));
        editor.mairis.analysis_path = Some(old.join("analysis.json"));
        fs::create_dir_all(temp.0.join("tools")).unwrap();
        fs::create_dir_all(temp.0.join("runs/pattern-test")).unwrap();
        fs::write(temp.0.join("runs/pattern-test/blocks.json"), "{}").unwrap();
        fs::write(temp.0.join("tools/generate_song.sh"), r#"set -eu
mkdir -p "$2"
cp "$1" "$2/track.wav"
printf '%s\n' "$1" "$3" "$4" > "$2/request"
"#).unwrap();
        let out = temp.0.join("new test");
        editor.mairis.settings.difficulty = 13.5;
        let (mut command, audio) = generation_command(&editor, &temp.0, &out, "42").unwrap();
        assert_eq!(audio, new.join("track.wav"));
        let result = command.output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(fs::read(out.join("track.wav")).unwrap(), fs::read(&audio).unwrap());
        let request = fs::read_to_string(out.join("request")).unwrap();
        assert_eq!(request.lines().collect::<Vec<_>>(), [audio.to_str().unwrap(), "13.5", "42"]);
    }

    #[test]
    fn reopening_chart_without_analysis_clears_old_assets_and_failed_open_preserves_current() {
        let temp = FeedbackDir::new();
        let old = test_song(&temp.0, "old", 100, Some(120.0));
        let new = test_song(&temp.0, "new", 900, None);
        let mut app = test_player();
        let mut editor = EditorState::default();
        crate::load_editor_chart(&mut editor, &mut app, &old, None);
        assert!(editor.mairis.overlay.is_some());
        crate::load_editor_chart(&mut editor, &mut app, &new, None);
        assert_eq!(editor.mairis.audio_source, Some(new.join("track.wav")));
        assert!(editor.mairis.overlay.is_none());
        assert!(editor.mairis.analysis_path.is_none());
        crate::load_editor_chart(&mut editor, &mut app, &temp.0.join("missing.txt"), None);
        assert_eq!(editor.mairis.audio_source, Some(new.join("track.wav")));
        assert!(editor.status.as_ref().unwrap().starts_with("Load failed:"));

        fs::remove_file(new.join("track.wav")).unwrap();
        editor.mairis.audio_source = Some(old.join("track.wav"));
        let error = generation_command(&editor, &temp.0, &temp.0.join("output"), "42").unwrap_err();
        assert!(error.contains("当前谱面目录没有音频"));
    }

    #[test]
    fn loading_case_without_analysis_or_with_invalid_analysis_clears_old_overlay() {
        let temp = FeedbackDir::new();
        let old = test_song(&temp.0, "old", 100, Some(120.0));
        let new = test_song(&temp.0, "new", 900, None);
        let mut app = test_player();
        let mut editor = EditorState::default();
        editor.mairis.load_chart_assets(&old, None);
        load_case(&mut editor, &mut app, &new.join("maidata.txt"));
        assert!(editor.mairis.overlay.is_none());
        assert!(editor.mairis.analysis_path.is_none());
        assert_eq!(editor.mairis.audio_source, Some(new.join("track.wav")));
        assert!(editor.reload_waveform);
        fs::write(new.join("analysis.json"), "broken").unwrap();
        editor.mairis.load_chart_assets(&old, None);
        load_case(&mut editor, &mut app, &new);
        assert!(editor.mairis.overlay.is_none());
    }

    #[test]
    fn annotation_moves_track_time_and_lane_through_undo_and_redo() {
        let (mut editor, mut chart) = annotation_fixture();
        let original = chart.notes[0].clone();
        editor.select_only(original.id);
        editor.nudge_selection(&mut chart, 0.125, 2);
        editor.mairis.annotation.sync_preview(&chart.notes);
        let diff = &editor.mairis.annotation;
        assert!(diff.review_added.is_empty() && diff.review_deleted.is_empty());
        assert_eq!(diff.review_moved.len(), 1);
        assert_eq!(diff.review_moved[0].original, original);
        assert_eq!(diff.review_moved[0].current.time, original.time + 0.125);
        assert_eq!(diff.review_moved[0].current.lane, original.lane + 2);
        assert!(diff.moved_ids.contains(&original.id));
        editor.undo(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert!(editor.mairis.annotation.review_moved.is_empty());
        assert!(editor.mairis.annotation.moved_ids.is_empty());
        editor.redo(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert_eq!(editor.mairis.annotation.review_moved.len(), 1);
    }

    #[test]
    fn annotation_deleted_ids_are_reserved_and_ghost_restore_is_undoable() {
        let (mut editor, mut chart) = annotation_fixture();
        let deleted = chart.notes.iter().max_by_key(|n| n.id).unwrap().clone();
        editor.delete_note(&mut chart, deleted.id);
        let added = editor.place_note(&mut chart, deleted.time + 0.125, 6);
        assert!(added > deleted.id, "new notes must not reuse a deleted baseline ID");
        editor.select_only(added);
        editor.nudge_selection(&mut chart, 0.125, 1);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert_eq!(editor.mairis.annotation.review_added[0].id, added);
        assert_eq!(editor.mairis.annotation.review_added[0].lane, 7);
        assert!(editor.mairis.annotation.review_moved.is_empty());
        editor.mairis.annotation.selected_ghost = Some(deleted.id);
        restore_selected_ghost(&mut editor, &mut chart);
        assert!(chart.notes.iter().any(|n| n == &deleted));
        assert!(editor.mairis.annotation.review_deleted.is_empty());
        assert_eq!(editor.single_selected(), Some(deleted.id));
        editor.undo(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert_eq!(editor.mairis.annotation.review_deleted[0].id, deleted.id);
        editor.redo(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert!(editor.mairis.annotation.review_deleted.is_empty());
        editor.select_only(added);
        editor.delete_selection(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert!(editor.mairis.annotation.review_added.is_empty());
        editor.undo(&mut chart);
        editor.mairis.annotation.sync_preview(&chart.notes);
        assert_eq!(editor.mairis.annotation.review_added[0].id, added);
    }

    #[test]
    fn annotation_finish_requires_review_and_saves_precise_feedback_and_snapshots() {
        let (mut editor, mut chart) = annotation_fixture();
        let original = chart.clone();
        let moved_id = chart.notes[2].id;
        editor.select_only(moved_id);
        editor.nudge_selection(&mut chart, 0.25, 1);
        let deleted_id = chart.notes[0].id;
        editor.delete_note(&mut chart, deleted_id);
        let added_id = editor.place_note(&mut chart, 2.0, 8);
        let dir = FeedbackDir::new();
        editor.chart_dir = dir.0.display().to_string();
        let mut app = PadPreviewState::new_at(chart.clone(), None, None, 0.0, None);
        finish_annotation_round(&mut editor, &mut app);
        assert_eq!(editor.mairis.annotation.phase, AnnotationPhase::Review);
        assert!(editor.mairis.annotation.confirm_pending);
        assert!(!editor.mairis.annotation.is_editing());
        assert!(!dir.0.join("feedback.jsonl").exists(), "finish must not save before confirmation");
        save_annotation_result(&dir.0, &editor.mairis.annotation, &chart).unwrap();
        let saved_original: ChartDoc = serde_json::from_slice(&fs::read(dir.0.join("original.chart.json")).unwrap()).unwrap();
        let saved_edited: ChartDoc = serde_json::from_slice(&fs::read(dir.0.join("annotated.chart.json")).unwrap()).unwrap();
        assert_eq!(saved_original.notes, original.notes);
        assert_eq!(saved_edited, chart);
        let feedback = fs::read_to_string(dir.0.join("feedback.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = feedback.lines().map(|s| serde_json::from_str(s).unwrap()).collect();
        assert_eq!(rows.len(), 3);
        let moved = rows.iter().find(|r| r["action"] == "move").unwrap();
        assert_eq!(moved["note_id"], moved_id);
        assert_eq!(moved["old_time_sec"], 1.0);
        assert_eq!(moved["new_time_sec"], 1.25, "BPM change must affect feedback timing");
        assert_eq!(moved["song_id"], "annotation-test");
        assert_eq!(moved["difficulty"], 13.5);
        assert!(rows.iter().any(|r| r["action"] == "delete" && r["note_id"] == deleted_id));
        assert!(rows.iter().any(|r| r["action"] == "add" && r["note_id"] == added_id));
        assert!(save_annotation_result(&dir.0.join("absent"), &editor.mairis.annotation, &chart).is_err());
        resume_annotation(&mut editor.mairis.annotation);
        assert!(editor.mairis.annotation.is_editing());
        assert!(!editor.mairis.annotation.confirm_pending);
    }

    #[test]
    fn annotation_unchanged_round_can_be_confirmed_without_inventing_feedback() {
        let (mut editor, chart) = annotation_fixture();
        editor.mairis.annotation.sync_preview(&chart.notes);
        let dir = FeedbackDir::new();
        save_annotation_result(&dir.0, &editor.mairis.annotation, &chart).unwrap();
        assert_eq!(fs::read_to_string(dir.0.join("feedback.jsonl")).unwrap(), "");
    }

    /// Run manually with MAIRIS_BENCH_CHART pointing to a generated chart.json.
    #[test]
    #[ignore = "latency probe needs a local generated chart and prints timings"]
    fn annotation_history_latency_probe() {
        let path = std::env::var_os("MAIRIS_BENCH_CHART").expect("set MAIRIS_BENCH_CHART");
        let original: ChartDoc = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        for annotation in [false, true] {
            let mut chart = original.clone();
            let mut editor = EditorState::default();
            editor.simai_editor.refresh(&chart);
            editor.mairis.annotation.active = annotation;
            if annotation { editor.mairis.annotation.set_original(&chart.notes); }
            editor.select_only(chart.notes[0].id);
            let started = std::time::Instant::now();
            for _ in 0..10 {
                editor.nudge_selection(&mut chart, 0.0013, 1);
                editor.mairis.annotation.sync_preview(&chart.notes);
                editor.undo(&mut chart);
                editor.mairis.annotation.sync_preview(&chart.notes);
                editor.redo(&mut chart);
                editor.mairis.annotation.sync_preview(&chart.notes);
                editor.undo(&mut chart);
            }
            eprintln!("annotation={annotation}, notes={}, 10 move/undo/redo/undo cycles: {:?}",
                chart.notes.len(), started.elapsed());
            assert_eq!(chart.notes.len(), original.notes.len());
        }
    }

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

