//! View preferences travel with a pane; document and transport stay shared.
use super::*;
use crate::state::{EditorLayout, EditorState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct PaneView {
    orientation: EditorLayout,
    hide_chart_tools: bool,
    note_scale: Option<f32>,
    start: f32,
    pps: f32,
    snap: bool,
    division: u32,
    follow: bool,
    follow_offset: Option<f32>,
    follow_zoom_detached: bool,
    cursor_follow: bool,
    detached: bool,
    invert: bool,
    pad_scale: f32,
    /// Properties pane: edit autoplay events instead of notes.
    pub(super) properties_autoplay: bool,
    /// Pad pane: preview only, no placing or dragging.
    pad_preview_only: bool,
    pad_surface: crate::state::PadSurface,
    autoplay: crate::autoplay_editor::ViewOptions,
    simai: crate::simai_editor::Options,
}

impl Default for PaneView {
    fn default() -> Self {
        Self {
            orientation: EditorLayout::Vertical,
            hide_chart_tools: false,
            note_scale: None,
            start: 0.0,
            pps: crate::timeline::geometry::DEFAULT_PPS,
            snap: true,
            division: 32,
            follow: true,
            follow_offset: None,
            follow_zoom_detached: false,
            cursor_follow: false,
            detached: false,
            invert: false,
            pad_scale: 1.0,
            properties_autoplay: false,
            pad_preview_only: false,
            pad_surface: crate::state::PadSurface::FollowFocus,
            autoplay: Default::default(),
            simai: Default::default(),
        }
    }
}

impl Workbench {
    pub fn capture_views(&mut self, editor: &EditorState, pad_scale: f32) {
        for dock in &mut self.docks {
            for (_, pane) in dock.iter_all_tabs_mut() {
                if !self.applied_views.contains(&(pane.kind, pane.id)) {
                    continue;
                }
                let view = &mut pane.view;
                match pane.kind {
                    PaneKind::Chart => {
                        view.orientation = editor.layout;
                        view.hide_chart_tools = editor.hide_chart_tools;
                        view.note_scale = editor.chart_note_scale;
                        view.start = editor.view_start;
                        view.pps = editor.pixels_per_second;
                        view.snap = editor.snap_enabled;
                        view.division = editor.snap_division;
                        view.follow = editor.follow_playhead;
                        view.follow_offset = editor.follow_view_offset;
                        view.follow_zoom_detached = editor.follow_zoom_detached;
                        view.detached = editor.keyboard.detached;
                        view.cursor_follow = editor.keyboard_seek_on_navigation;
                        view.invert = editor.scroll_invert;
                    }
                    PaneKind::Autoplay => {
                        view.autoplay = editor.autoplay_editor.view_options();
                        view.snap = editor.autoplay_snap_enabled;
                        view.division = editor.autoplay_snap_division;
                    }
                    PaneKind::Pad => {
                        view.pad_scale = pad_scale;
                        view.pad_preview_only = editor.pane_pad_preview_only;
                        view.pad_surface = editor.pane_pad_surface;
                    }
                    PaneKind::Properties => {
                        view.properties_autoplay = editor.pane_properties_autoplay;
                    }
                    PaneKind::Simai => view.simai = editor.simai_editor.options.clone(),
                    _ => {}
                }
            }
        }
    }

    pub fn activate_views(&mut self, editor: &mut EditorState, pad_scale: &mut f32) {
        let mut active = Vec::new();
        for (_, pane) in self.docks[self.workspace].iter_all_tabs() {
            active.push((pane.kind, pane.id));
            if self.applied_views.contains(&(pane.kind, pane.id)) {
                continue;
            }
            let v = &pane.view;
            match pane.kind {
                PaneKind::Chart => {
                    editor.layout = v.orientation;
                    editor.hide_chart_tools = v.hide_chart_tools;
                    editor.chart_note_scale = v.note_scale;
                    editor.view_start = v.start;
                    editor.pixels_per_second = v.pps.clamp(
                        crate::timeline::geometry::MIN_PPS,
                        crate::timeline::geometry::MAX_PPS,
                    );
                    editor.snap_enabled = v.snap;
                    editor.snap_division = v.division.clamp(1, 256);
                    editor.follow_playhead = v.follow;
                    editor.follow_view_offset = v.follow_offset;
                    editor.follow_zoom_detached = v.follow_zoom_detached;
                    editor.keyboard.detached = v.detached;
                    editor.keyboard_seek_on_navigation = v.cursor_follow;
                    editor.scroll_invert = v.invert;
                }
                PaneKind::Autoplay => {
                    editor.autoplay_editor.restore_view(&v.autoplay);
                    editor.autoplay_snap_enabled = v.snap;
                    editor.autoplay_snap_division = v.division.clamp(1, 256);
                }
                PaneKind::Pad => {
                    *pad_scale = v.pad_scale.clamp(0.3, 2.0);
                    editor.pane_pad_preview_only = v.pad_preview_only;
                    editor.pane_pad_surface = v.pad_surface;
                }
                PaneKind::Properties => editor.pane_properties_autoplay = v.properties_autoplay,
                PaneKind::Simai => editor.simai_editor.options = v.simai.clone(),
                _ => {}
            }
        }
        if self.applied_views != active {
            editor.template_panel = self.has(PaneKind::Templates);
        }
        self.applied_views = active;
    }
}

impl PaneView {
    pub(super) fn ignore_playback_position(&mut self, kind: PaneKind) {
        if kind == PaneKind::Chart && self.follow && !self.detached && !self.follow_zoom_detached {
            self.start = 0.0;
        }
        if kind == PaneKind::Autoplay {
            self.autoplay.ignore_playback_position();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_layout_keeps_template_visibility_and_pane_configuration() {
        let mut workbench = Workbench::default();
        let mut editor = EditorState::default();
        let mut scale = 1.0;
        workbench.activate_views(&mut editor, &mut scale);
        workbench.set_visible(PaneKind::Templates, true);
        editor.layout = EditorLayout::Horizontal;
        editor.snap_division = 48;
        scale = 1.4;
        workbench.capture_views(&editor, scale);
        workbench.saved[0] = Some(workbench.docks[0].clone());
        workbench.set_visible(PaneKind::Templates, false);
        editor.template_panel = false;
        editor.layout = EditorLayout::Vertical;
        scale = 0.8;
        workbench.restore_layout();
        workbench.activate_views(&mut editor, &mut scale);
        workbench.set_visible(PaneKind::Templates, editor.template_panel);
        assert!(workbench.has(PaneKind::Templates), "restored panes must not be removed by an old visibility toggle");
        assert_eq!(editor.layout, EditorLayout::Horizontal);
        assert_eq!(editor.snap_division, 48);
        assert_eq!(scale, 1.4);
    }

    #[test]
    fn chart_note_scale_is_independent_and_restores_with_layout() {
        let legacy: PaneView = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.note_scale, None);
        let view = PaneView { note_scale: Some(0.65), ..Default::default() };
        let restored: PaneView = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
        assert_eq!(restored.note_scale, Some(0.65));

        let mut workbench = Workbench::default();
        let mut editor = EditorState::default();
        let mut pad_scale = 1.0;
        workbench.activate_views(&mut editor, &mut pad_scale);
        editor.chart_note_scale = Some(0.65);
        workbench.capture_views(&editor, pad_scale);
        workbench.saved[0] = Some(workbench.docks[0].clone());
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut pad_scale);
        assert_eq!(editor.chart_note_scale, None);
        editor.chart_note_scale = Some(1.75);
        workbench.capture_views(&editor, pad_scale);
        workbench.workspace = 0;
        workbench.activate_views(&mut editor, &mut pad_scale);
        assert_eq!(editor.note_scale(), 0.65);
        editor.chart_note_scale = Some(1.25);
        workbench.capture_views(&editor, pad_scale);
        workbench.restore_layout();
        workbench.activate_views(&mut editor, &mut pad_scale);
        assert_eq!(editor.note_scale(), 0.65);
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut pad_scale);
        assert_eq!(editor.note_scale(), 1.75);
        assert_eq!(pad_scale, 1.0, "chart sizing must not change the Pad");
    }

    #[test]
    fn hidden_chart_tools_preference_roundtrips_and_defaults_to_visible() {
        let legacy: PaneView = serde_json::from_str("{}").unwrap();
        assert!(!legacy.hide_chart_tools);
        let view = PaneView { hide_chart_tools: true, ..Default::default() };
        let restored: PaneView = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
        assert!(restored.hide_chart_tools);
    }

    #[test]
    fn workspace_upgrade_independent_pane_views() {
        let mut workbench = Workbench::default();
        let mut editor = EditorState::default();
        let mut scale = 1.0;
        workbench.activate_views(&mut editor, &mut scale);
        editor.layout = EditorLayout::Horizontal;
        editor.snap_division = 32;
        editor.hide_chart_tools = true;
        editor.pixels_per_second = 720.0;
        scale = 1.4;
        workbench.capture_views(&editor, scale);
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Vertical);
        assert_eq!(editor.snap_division, 32);
        assert!(!editor.hide_chart_tools);
        assert_eq!(scale, 1.0);
        editor.autoplay_snap_division = 64;
        workbench.capture_views(&editor, scale);
        workbench.workspace = 1;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.autoplay_snap_division, 32);
        workbench.workspace = 0;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Horizontal);
        assert_eq!(editor.snap_division, 32);
        assert!(editor.hide_chart_tools);
        assert_eq!(editor.pixels_per_second, 720.0);
        assert_eq!(scale, 1.4);
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Vertical);
        assert_eq!(editor.autoplay_snap_division, 64);
    }
}
