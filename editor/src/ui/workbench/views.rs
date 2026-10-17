//! View preferences travel with a pane; document and transport stay shared.
use super::*;
use crate::state::{EditorLayout, EditorState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct PaneView {
    orientation: EditorLayout,
    start: f32,
    pps: f32,
    snap: bool,
    division: u32,
    follow: bool,
    follow_offset: Option<f32>,
    cursor_follow: bool,
    detached: bool,
    invert: bool,
    pad_scale: f32,
    /// Properties pane: edit autoplay events instead of notes.
    pub(super) properties_autoplay: bool,
    /// Pad pane: preview only, no placing or dragging.
    pad_preview_only: bool,
    autoplay: crate::autoplay_editor::ViewOptions,
    simai: crate::simai_editor::Options,
}

impl Default for PaneView {
    fn default() -> Self {
        Self { orientation: EditorLayout::Vertical, start: 0.0,
            pps: crate::timeline::geometry::DEFAULT_PPS, snap: true, division: 16,
            follow: true, follow_offset: None, cursor_follow: false, detached: false,
            invert: false, pad_scale: 1.0, properties_autoplay: false, pad_preview_only: false,
            autoplay: Default::default(), simai: Default::default() }
    }
}

impl Workbench {
    pub fn capture_views(&mut self, editor: &EditorState, pad_scale: f32) {
        for dock in &mut self.docks {
            for (_, pane) in dock.iter_all_tabs_mut() {
                if !self.applied_views.contains(&(pane.kind, pane.id)) { continue; }
                let view = &mut pane.view;
                match pane.kind {
                    PaneKind::Chart => {
                        view.orientation = editor.layout;
                        view.start = editor.view_start;
                        view.pps = editor.pixels_per_second;
                        view.snap = editor.snap_enabled;
                        view.division = editor.snap_division;
                        view.follow = editor.follow_playhead;
                        view.follow_offset = editor.follow_view_offset;
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
            if self.applied_views.contains(&(pane.kind, pane.id)) { continue; }
            let v = &pane.view;
            match pane.kind {
                PaneKind::Chart => {
                    editor.layout = v.orientation;
                    editor.view_start = v.start;
                    editor.pixels_per_second = v.pps.clamp(crate::timeline::geometry::MIN_PPS, crate::timeline::geometry::MAX_PPS);
                    editor.snap_enabled = v.snap;
                    editor.snap_division = v.division.clamp(1, 256);
                    editor.follow_playhead = v.follow;
                    editor.follow_view_offset = v.follow_offset;
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
                }
                PaneKind::Properties => editor.pane_properties_autoplay = v.properties_autoplay,
                PaneKind::Simai => editor.simai_editor.options = v.simai.clone(),
                _ => {}
            }
        }
        self.applied_views = active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_upgrade_independent_pane_views() {
        let mut workbench = Workbench::default();
        let mut editor = EditorState::default();
        let mut scale = 1.0;
        workbench.activate_views(&mut editor, &mut scale);
        editor.layout = EditorLayout::Horizontal;
        editor.snap_division = 32;
        editor.pixels_per_second = 720.0;
        scale = 1.4;
        workbench.capture_views(&editor, scale);
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Vertical);
        assert_eq!(editor.snap_division, 16);
        assert_eq!(scale, 1.0);
        editor.autoplay_snap_division = 64;
        workbench.capture_views(&editor, scale);
        workbench.workspace = 1;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.autoplay_snap_division, 16);
        workbench.workspace = 0;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Horizontal);
        assert_eq!(editor.snap_division, 32);
        assert_eq!(editor.pixels_per_second, 720.0);
        assert_eq!(scale, 1.4);
        workbench.workspace = 2;
        workbench.activate_views(&mut editor, &mut scale);
        assert_eq!(editor.layout, EditorLayout::Vertical);
        assert_eq!(editor.autoplay_snap_division, 64);
    }
}
