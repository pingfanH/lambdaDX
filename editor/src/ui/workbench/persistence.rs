use super::*;
use crate::app::platform;
use egui_dock::{Node, Tree};

const FILE: &str = "editor_workspaces.json";

#[cfg(test)]
mod agent_layout_tests {
    use super::*;
    #[test]
    fn old_layouts_load_and_agent_pane_roundtrips_without_session_data() {
        let old: Layouts = serde_json::from_str(r#"{"version":1,"layouts":[{"Pane":"Chart"},{"Pane":"Simai"}]}"#).unwrap();
        assert!(old.valid());
        let mut layouts = old;
        layouts.current = vec![Some(SavedNode::Pane(PaneKind::Agent))];
        let text = serde_json::to_string(&layouts).unwrap();
        let restored: Layouts = serde_json::from_str(&text).unwrap();
        assert!(restored.valid());
        assert_eq!(restored, layouts);
        assert!(!text.contains("credential"));
        assert!(!text.contains("messages"));
        let mut workbench = Workbench::default();
        workbench.set_visible(PaneKind::Agent, true);
        assert!(workbench.contains_anywhere(PaneKind::Agent));
        workbench.workspace = 1;
        assert!(workbench.contains_anywhere(PaneKind::Agent));
        assert!(!workbench.has(PaneKind::Agent));
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Layouts {
    version: u32,
    layouts: Vec<Option<SavedNode>>,
    #[serde(default)]
    current: Vec<Option<SavedNode>>,
    #[serde(default)]
    workspace: usize,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    preview_mode: Option<u8>,
}

#[derive(Default)]
pub(super) struct Persistence {
    path: Option<std::path::PathBuf>,
    written: Option<Layouts>,
    pending: Option<(Layouts, f64)>,
    error: Option<String>,
}

impl Persistence {
    fn path(&self) -> Result<Option<std::path::PathBuf>, String> {
        if let Some(path) = &self.path { return Ok(Some(path.clone())); }
        // Tests opt into a temporary path instead of writing user preferences.
        if cfg!(test) { return Ok(None); }
        platform::cache_dir().map(|dir| Some(dir.join(FILE)))
    }

    fn read(&self) -> Result<Option<String>, String> {
        fn read_path(path: &std::path::Path) -> Result<Option<String>, String> {
            match std::fs::read_to_string(path) {
                Ok(text) => Ok(Some(text)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!("read {}: {error}", path.display())),
            }
        }
        let Some(path) = self.path()? else { return Ok(None); };
        if let Some(text) = read_path(&path)? { return Ok(Some(text)); }
        if self.path.is_some() || cfg!(test) { return Ok(None); }
        read_path(&platform::output_dir()?.join(FILE))
    }

    fn write(&mut self, layouts: &Layouts) -> Result<(), String> {
        let result = (|| {
            if let Some(path) = self.path()? {
                let text = serde_json::to_string_pretty(layouts).map_err(|e| e.to_string())?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
                }
                let temp = path.with_extension(format!("{}.tmp", std::process::id()));
                std::fs::write(&temp, text).map_err(|e| format!("write {}: {e}", temp.display()))?;
                if let Err(error) = std::fs::rename(&temp, &path) {
                    let _ = std::fs::remove_file(&temp);
                    return Err(format!("save {}: {error}", path.display()));
                }
            }
            Ok(())
        })();
        self.error = result.as_ref().err().cloned();
        if result.is_ok() {
            self.written = Some(layouts.comparison());
            self.pending = None;
        }
        result
    }

    pub(super) fn failed(&self) -> bool { self.error.is_some() }
}

impl Layouts {
    fn valid(&self) -> bool {
        matches!(self.version, 1 | 2)
            && self.workspace < WORKSPACE_COUNT
            && self.preview_mode.is_none_or(|mode| mode <= 3)
            && [&self.layouts, &self.current].into_iter().all(|layouts| {
                layouts.len() <= WORKSPACE_COUNT
                    && layouts.iter().flatten().all(|node| node.valid(&mut Vec::new(), 0))
            })
    }

    fn comparison(&self) -> Self {
        let mut value = self.clone();
        for node in value.current.iter_mut().flatten() { node.ignore_playback_position(); }
        value
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum SavedNode {
    Pane(PaneKind),
    ConfiguredPane { kind: PaneKind, view: PaneView },
    Split {
        vertical: bool,
        fraction: f32,
        first: Box<SavedNode>,
        second: Box<SavedNode>,
    },
}

impl SavedNode {
    fn ignore_playback_position(&mut self) {
        match self {
            Self::ConfiguredPane { kind, view } => view.ignore_playback_position(*kind),
            Self::Split { first, second, .. } => {
                first.ignore_playback_position();
                second.ignore_playback_position();
            }
            Self::Pane(_) => {}
        }
    }

    fn capture(tree: &Tree<Pane>, at: NodeIndex) -> Option<Self> {
        match &tree[at] {
            Node::Leaf { tabs, .. } => tabs.first().map(|pane| Self::ConfiguredPane { kind: pane.kind, view: pane.view.clone() }),
            Node::Vertical { fraction, .. } | Node::Horizontal { fraction, .. } => {
                Some(Self::Split {
                    vertical: matches!(tree[at], Node::Vertical { .. }),
                    fraction: *fraction,
                    first: Box::new(Self::capture(tree, NodeIndex(at.0 * 2 + 1))?),
                    second: Box::new(Self::capture(tree, NodeIndex(at.0 * 2 + 2))?),
                })
            }
            Node::Empty => None,
        }
    }

    fn valid(&self, seen: &mut Vec<PaneKind>, depth: usize) -> bool {
        if depth > PaneKind::ALL.len() {
            return false;
        }
        match self {
            Self::Pane(kind) | Self::ConfiguredPane { kind, .. } => {
                if seen.contains(kind) {
                    return false;
                }
                seen.push(*kind);
                true
            }
            Self::Split {
                fraction,
                first,
                second,
                ..
            } => {
                fraction.is_finite()
                    && *fraction > 0.0
                    && *fraction < 1.0
                    && first.valid(seen, depth + 1)
                    && second.valid(seen, depth + 1)
            }
        }
    }

    fn first(&self) -> PaneKind {
        match self {
            Self::Pane(kind) | Self::ConfiguredPane { kind, .. } => *kind,
            Self::Split { first, .. } => first.first(),
        }
    }

    fn fill(&self, tree: &mut Tree<Pane>, at: NodeIndex, next_id: &mut u64) {
        if let Self::Split {
            vertical,
            fraction,
            first,
            second,
        } = self
        {
            *next_id += 1;
            let [a, b] = tree.split_tabs(
                at,
                if *vertical {
                    Split::Below
                } else {
                    Split::Right
                },
                *fraction,
                vec![Pane {
                    id: *next_id,
                    kind: second.first(),
                    view: PaneView::default(),
                }],
            );
            first.fill(tree, a, next_id);
            second.fill(tree, b, next_id);
        } else if let Self::ConfiguredPane { view, .. } = self {
            if let Some(pane) = tree[at].tabs_mut().and_then(|tabs| tabs.first_mut()) {
                pane.view = view.clone();
            }
        }
    }

    fn restore(&self, next_id: &mut u64) -> DockState<Pane> {
        *next_id += 1;
        let mut dock = DockState::new(vec![Pane {
            id: *next_id,
            kind: self.first(),
            view: PaneView::default(),
        }]);
        self.fill(dock.main_surface_mut(), NodeIndex::root(), next_id);
        dock
    }
}

impl Workbench {
    pub(super) fn load_layouts(&mut self) {
        let result = (|| -> Result<Option<Layouts>, String> {
            // The cache is the live copy; `read_cache_text` also picks up the
            // legacy `output/` file once for installs from before the cache.
            let Some(text) = self.persistence.read()? else { return Ok(None); };
            let parsed = serde_json::from_str::<Layouts>(&text).ok().filter(Layouts::valid);
            if let Some(saved) = parsed {
                return Ok(Some(saved));
            }
            let Some(path) = self.persistence.path()? else { return Ok(None); };
            let backup = path.with_extension("invalid.json");
            std::fs::write(&backup, text).map_err(|e| e.to_string())?;
            Err(format!(
                "Invalid workspace layout; backup: {}",
                backup.display()
            ))
        })();
        match result {
            Ok(Some(saved)) => {
                self.workspace = saved.workspace;
                self.locked = saved.locked;
                self.preview_mode = saved.preview_mode;
                self.preview_mode_applied = false;
                for (index, node) in saved.layouts.iter().enumerate() {
                    if let Some(node) = node {
                        let dock = node.restore(&mut self.next_id);
                        self.saved[index] = Some(dock.clone());
                        self.docks[index] = dock;
                    }
                }
                for (index, node) in saved.current.iter().enumerate() {
                    if let Some(node) = node {
                        self.docks[index] = node.restore(&mut self.next_id);
                    }
                }
                self.applied_views.clear();
                if saved.version == 2 {
                    self.persistence.written = Some(self.layout_snapshot().comparison());
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.persistence.error = Some(error.clone());
                self.message = Some(error);
            }
        }
    }

    fn layout_snapshot(&self) -> Layouts {
        Layouts {
            version: 2,
            layouts: self.saved.iter().map(|layout| {
                layout
                    .as_ref()
                    .and_then(|dock| SavedNode::capture(dock.main_surface(), NodeIndex::root()))
            }).collect(),
            current: self.docks.iter().map(|dock| SavedNode::capture(dock.main_surface(), NodeIndex::root())).collect(),
            workspace: self.workspace,
            locked: self.locked,
            preview_mode: self.preview_mode,
        }
    }

    pub(super) fn save_layouts(&mut self) -> Result<(), String> {
        let snapshot = self.layout_snapshot();
        self.persistence.write(&snapshot)
    }

    pub fn persist_current_layout(&mut self) -> Result<(), String> {
        if let Some(error) = &self.persistence.error { return Err(error.clone()); }
        self.save_layouts()
    }

    pub fn flush_layouts(&mut self, now: f64, pointer_down: bool) {
        if self.remember_requested {
            self.remember_requested = false;
            self.remember_layout();
        }
        if pointer_down || self.dragging() {
            self.persistence.pending = None;
            return;
        }
        if self.persistence.failed() { return; }
        let snapshot = self.layout_snapshot().comparison();
        if self.persistence.written.as_ref() == Some(&snapshot) {
            self.persistence.pending = None;
            return;
        }
        match &self.persistence.pending {
            Some((pending, since)) if pending == &snapshot => {
                if now - since < 0.35 { return; }
            }
            _ => {
                self.persistence.pending = Some((snapshot, now));
                return;
            }
        }
        if let Err(error) = self.save_layouts() {
            self.message = Some(format!("布局自动保存失败: {error}"));
        }
    }

    pub(super) fn retry_layout_save(&mut self) {
        self.persistence.error = None;
        self.persistence.written = None;
        self.persistence.pending = None;
        self.message = None;
    }

    pub fn remember_layout(&mut self) {
        self.saved[self.workspace] = Some(self.docks[self.workspace].clone());
        self.message = Some(match self.save_layouts() {
            Ok(()) => "布局已保存".into(),
            Err(error) => format!("布局保存失败: {error}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestFile(std::path::PathBuf);

    impl TestFile {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("lambdadx_layout_test_{}_{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir.join(FILE))
        }

        fn workbench(&self) -> Workbench {
            let mut workbench = Workbench::default();
            workbench.persistence.path = Some(self.0.clone());
            workbench
        }

        fn read(&self) -> Layouts {
            serde_json::from_str(&std::fs::read_to_string(&self.0).unwrap()).unwrap()
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn restart_persistence_records_live_splits_and_pane_options() {
        let file = TestFile::new();
        let mut workbench = file.workbench();
        workbench.workspace = 2;
        workbench.set_visible(PaneKind::History, true);
        workbench.set_visible(PaneKind::Properties, false);
        if let Node::Horizontal { fraction, .. } = &mut workbench.docks[2].main_surface_mut()[NodeIndex::root()] {
            *fraction = 0.61;
        }
        let mut editor = crate::state::EditorState::default();
        let mut pad_scale = 1.0;
        workbench.activate_views(&mut editor, &mut pad_scale);
        editor.chart_note_scale = Some(0.65);
        editor.hide_chart_tools = true;
        editor.layout = crate::state::EditorLayout::Horizontal;
        editor.snap_division = 48;
        pad_scale = 1.4;
        workbench.capture_views(&editor, pad_scale);
        workbench.locked = true;
        workbench.flush_layouts(0.0, false);
        assert!(!file.0.exists(), "wait until the edit settles");
        workbench.flush_layouts(0.4, false);
        let saved = file.read();
        assert_eq!(saved.version, 2);
        assert!(saved.current[2].is_some());
        assert!(saved.layouts.iter().all(Option::is_none));
        let mut restored = file.workbench();
        restored.load_layouts();
        assert_eq!(restored.layout_snapshot(), workbench.layout_snapshot());
        assert_eq!(restored.workspace, 2);
        assert!(restored.locked);
        assert!(restored.has(PaneKind::History));
        assert!(!restored.has(PaneKind::Properties));
        let mut restored_editor = crate::state::EditorState::default();
        let mut restored_scale = 1.0;
        restored.activate_views(&mut restored_editor, &mut restored_scale);
        assert_eq!(restored_editor.chart_note_scale, Some(0.65));
        assert!(restored_editor.hide_chart_tools);
        assert_eq!(restored_editor.layout, crate::state::EditorLayout::Horizontal);
        assert_eq!(restored_editor.snap_division, 48);
        assert_eq!(restored_scale, 1.4);
    }

    #[test]
    fn simai_modal_preferences_survive_a_real_layout_restart() {
        use crate::simai_editor::modal::Style;
        let file = TestFile::new();
        let mut workbench = file.workbench();
        workbench.workspace = 3;
        let mut editor = crate::state::EditorState::default(); let mut scale = 1.0;
        workbench.activate_views(&mut editor, &mut scale);
        editor.simai_editor.options.edit_style = Style::Helix;
        editor.simai_editor.options.auto_completion = false;
        editor.simai_editor.options.auto_pairs = false;
        workbench.capture_views(&editor, scale);
        workbench.persist_current_layout().unwrap();
        let mut restored = file.workbench(); restored.load_layouts();
        let mut editor = crate::state::EditorState::default();
        restored.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.simai_editor.options.edit_style, Style::Helix);
        assert!(!editor.simai_editor.options.auto_completion);
        assert!(!editor.simai_editor.options.auto_pairs);
    }

    #[test]
    fn restart_persistence_preserves_manual_snapshot_separately() {
        let file = TestFile::new();
        let mut workbench = file.workbench();
        workbench.remember_layout();
        let manual = workbench.layout_snapshot().layouts;
        workbench.set_visible(PaneKind::History, true);
        workbench.flush_layouts(0.0, false);
        workbench.flush_layouts(0.4, false);
        assert_eq!(file.read().layouts, manual);
        let mut restored = file.workbench();
        restored.load_layouts();
        assert!(restored.has(PaneKind::History));
        restored.restore_layout();
        assert!(!restored.has(PaneKind::History));
        assert_eq!(restored.layout_snapshot().layouts, manual);
    }

    #[test]
    fn restart_persistence_migrates_legacy_snapshots_and_template_visibility() {
        let file = TestFile::new();
        let mut original = file.workbench();
        original.set_visible(PaneKind::Templates, true);
        let legacy = serde_json::json!({
            "version": 1,
            "layouts": [SavedNode::capture(original.docks[0].main_surface(), NodeIndex::root())],
        });
        std::fs::write(&file.0, serde_json::to_string(&legacy).unwrap()).unwrap();
        let mut restored = file.workbench();
        restored.load_layouts();
        assert!(restored.has(PaneKind::Templates));
        let mut editor = crate::state::EditorState::default();
        let mut scale = 1.0;
        restored.activate_views(&mut editor, &mut scale);
        assert!(editor.template_panel);
        restored.set_visible(PaneKind::Templates, editor.template_panel);
        assert!(restored.has(PaneKind::Templates));
        restored.flush_layouts(0.0, false);
        restored.flush_layouts(0.4, false);
        assert_eq!(file.read().version, 2);
        assert!(file.read().layouts[0].is_some());
    }

    #[test]
    fn restart_persistence_flushes_on_quit_before_debounce() {
        let file = TestFile::new();
        let mut workbench = file.workbench();
        workbench.set_visible(PaneKind::History, true);
        workbench.flush_layouts(0.0, true);
        assert!(!file.0.exists());
        workbench.persist_current_layout().unwrap();
        let mut restored = file.workbench();
        restored.load_layouts();
        assert!(restored.has(PaneKind::History));
    }

    #[test]
    fn restart_persistence_ignores_playback_motion_but_saves_options() {
        let file = TestFile::new();
        let mut workbench = file.workbench();
        let mut editor = crate::state::EditorState::default();
        let mut scale = 1.0;
        workbench.activate_views(&mut editor, &mut scale);
        workbench.capture_views(&editor, scale);
        workbench.persist_current_layout().unwrap();
        let original = std::fs::read_to_string(&file.0).unwrap();
        for time in [1.0, 2.0, 3.0] {
            editor.view_start = time;
            workbench.capture_views(&editor, scale);
            workbench.flush_layouts(time as f64, false);
        }
        assert!(workbench.persistence.pending.is_none());
        assert_eq!(std::fs::read_to_string(&file.0).unwrap(), original);
        editor.hide_chart_tools = true;
        workbench.capture_views(&editor, scale);
        workbench.flush_layouts(4.0, false);
        workbench.flush_layouts(4.4, false);
        assert_ne!(std::fs::read_to_string(&file.0).unwrap(), original);
    }

    #[test]
    fn restart_persistence_preserves_corrupt_preferences_until_retry() {
        let file = TestFile::new();
        let invalid = "{broken layout";
        std::fs::write(&file.0, invalid).unwrap();
        let mut workbench = file.workbench();
        workbench.load_layouts();
        assert!(workbench.persistence.failed());
        assert_eq!(std::fs::read_to_string(file.0.with_extension("invalid.json")).unwrap(), invalid);
        workbench.flush_layouts(0.0, false);
        workbench.flush_layouts(1.0, false);
        assert_eq!(std::fs::read_to_string(&file.0).unwrap(), invalid);
        assert!(workbench.persist_current_layout().is_err());
        workbench.retry_layout_save();
        workbench.flush_layouts(2.0, false);
        workbench.flush_layouts(2.4, false);
        assert!(file.read().valid());
    }

    #[test]
    fn restart_persistence_survives_full_editor_startup_and_following_frames() {
        fn frame(
            ctx: &egui_macroquad::egui::Context,
            ui: &mut crate::ui::EditorUi,
            editor: &mut crate::state::EditorState,
            app: &mut crate::player::state::PadPreviewState,
            time: f64,
        ) {
            let _ = ctx.run(egui_macroquad::egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 800.0))),
                time: Some(time),
                ..Default::default()
            }, |ctx| ui.draw(ctx, editor, app, 0.0, &crate::audio::WaveformPyramid::default(), 0.0));
        }
        let file = TestFile::new();
        let mut ui = crate::ui::EditorUi::new();
        ui.workbench = file.workbench();
        let ctx = egui_macroquad::egui::Context::default();
        let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1,,E", None).unwrap();
        let mut app = crate::player::state::PadPreviewState::new_at(chart, None, None, 0.0, None);
        app.forced_time = Some(0.0);
        let mut editor = crate::state::EditorState::default();
        for step in 1..=3 { frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1); }
        ui.workbench.set_visible(PaneKind::Templates, true);
        ui.workbench.set_visible(PaneKind::Properties, false);
        editor.layout = crate::state::EditorLayout::Horizontal;
        editor.chart_note_scale = Some(0.7);
        editor.hide_chart_tools = true;
        editor.snap_division = 48;
        ui.pad_scale = 1.3;
        for step in 4..=12 { frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1); }
        assert!(ui.workbench.has(PaneKind::Templates));
        let expected = file.read();
        assert_eq!(expected, ui.workbench.layout_snapshot());

        let ctx = egui_macroquad::egui::Context::default();
        let mut ui = crate::ui::EditorUi::new();
        ui.workbench = file.workbench();
        let mut editor = crate::state::EditorState::default();
        for step in 1..=12 { frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1); }
        assert_eq!(ui.workbench.layout_snapshot(), expected);
        assert!(ui.workbench.has(PaneKind::Templates));
        assert!(editor.template_panel);
        assert!(!ui.workbench.has(PaneKind::Properties));
        assert_eq!(editor.chart_note_scale, Some(0.7));
        assert!(editor.hide_chart_tools);
        assert_eq!(editor.layout, crate::state::EditorLayout::Horizontal);
        assert_eq!(editor.snap_division, 48);
        assert_eq!(ui.pad_scale, 1.3);
        assert_eq!(file.read(), expected);
    }

    #[test]
    fn pad_options_survive_full_editor_restart() {
        fn frame(
            ctx: &egui_macroquad::egui::Context,
            ui: &mut crate::ui::EditorUi,
            editor: &mut crate::state::EditorState,
            app: &mut crate::player::state::PadPreviewState,
            time: f64,
        ) {
            let _ = ctx.run(egui_macroquad::egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 800.0))),
                time: Some(time),
                ..Default::default()
            }, |ctx| ui.draw(ctx, editor, app, 0.0, &crate::audio::WaveformPyramid::default(), 0.0));
        }
        for (surface, mode) in [
            crate::state::PadSurface::FollowFocus,
            crate::state::PadSurface::Chart,
            crate::state::PadSurface::Autoplay,
        ]
        .into_iter()
        .flat_map(|surface| (0..=3).map(move |mode| (surface, mode)))
        {
            let file = TestFile::new();
            let mut ui = crate::ui::EditorUi::new();
            ui.workbench = file.workbench();
            let ctx = egui_macroquad::egui::Context::default();
            let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1,,E", None).unwrap();
            let mut app = crate::player::state::PadPreviewState::new_at(chart, None, None, 0.0, None);
            app.forced_time = Some(0.0);
            let mut editor = crate::state::EditorState::default();
            for step in 1..=3 {
                frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1);
            }
            editor.pane_pad_preview_only = true;
            editor.pane_pad_surface = surface;
            editor.autoplay_mode = mode;
            ui.pad_scale = 1.4;
            for step in 4..=12 {
                frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1);
            }
            assert!(file.0.exists());

            let ctx = egui_macroquad::egui::Context::default();
            let mut ui = crate::ui::EditorUi::new();
            ui.workbench = file.workbench();
            let mut editor = crate::state::EditorState::default();
            let mut app = crate::player::state::PadPreviewState::new_at(
                app.chart.clone(), None, None, 0.0, None,
            );
            app.forced_time = Some(0.0);
            for step in 1..=12 {
                frame(&ctx, &mut ui, &mut editor, &mut app, step as f64 * 0.1);
            }
            assert!(editor.pane_pad_preview_only);
            assert_eq!(editor.pane_pad_surface, surface);
            assert_eq!(ui.pad_scale, 1.4);
            assert_eq!(editor.autoplay_mode, mode, "the Pad preview mode must survive restart");
            assert!(editor.mode_dirty, "the restored mode must also be applied to playback");
            crate::runtime::apply_preview_mode(&mut app, editor.autoplay_mode);
            assert_eq!(app.autoplay, mode != 1);
            assert_eq!(app.params.no_core, mode == 0);
            assert_eq!(app.autoplay_direct_core, mode == 3);
        }
    }

    #[test]
    fn layout_roundtrip_keeps_order_and_proportions() {
        let mut next = 0;
        for index in 0..WORKSPACE_COUNT {
            let dock = preset(index, &mut next);
            let saved = SavedNode::capture(dock.main_surface(), NodeIndex::root()).unwrap();
            assert!(saved.valid(&mut Vec::new(), 0));
            let restored = saved.restore(&mut next);
            assert_eq!(
                Some(saved),
                SavedNode::capture(restored.main_surface(), NodeIndex::root())
            );
        }
    }
}
