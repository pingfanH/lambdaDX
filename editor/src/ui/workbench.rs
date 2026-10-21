//! Layout owns pane identities, not document data. Each editor has one live host.

use super::style::{dock_style, glyph, icon};
use crate::state::Workspace;
use crate::theme::*;
use egui_dock::{DockArea, DockState, NodeIndex, Split, SurfaceIndex, TabDestination, TabIndex, TabInsert, TabViewer};
use egui_macroquad::egui::*;
use lucide_icons::Icon;
use serde::{Deserialize, Serialize};

mod persistence;
mod dragging;
mod views;
use views::PaneView;

const WORKSPACE_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneKind {
    Chart,
    Pad,
    Properties,
    Overview,
    Autoplay,
    Templates,
    History,
    Simai,
    Agent,
}

impl PaneKind {
    pub const ALL: [Self; 9] = [
        Self::Chart,
        Self::Pad,
        Self::Properties,
        Self::Overview,
        Self::Autoplay,
        Self::Templates,
        Self::History,
        Self::Simai,
        Self::Agent,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Chart => "谱面",
            Self::Pad => "Pad",
            Self::Properties => "属性",
            Self::Overview => "总览",
            Self::Autoplay => "Autoplay",
            Self::Templates => "模板 / Scene",
            Self::History => "历史",
            Self::Simai => "Simai",
            Self::Agent => "Agent",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Chart => Icon::Music2,
            Self::Pad => Icon::CircleDot,
            Self::Properties => Icon::SlidersHorizontal,
            Self::Overview => Icon::AudioLines,
            Self::Autoplay => Icon::ListVideo,
            Self::Templates => Icon::Library,
            Self::History => Icon::History,
            Self::Simai => Icon::FileCode2,
            Self::Agent => Icon::MessageSquare,
        }
    }
    fn workspace(self) -> Option<Workspace> {
        match self {
            Self::Chart | Self::Templates | Self::Simai => Some(Workspace::Chart),
            Self::Autoplay => Some(Workspace::Autoplay),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct Pane {
    id: u64,
    kind: PaneKind,
    view: PaneView,
}

pub struct Region {
    pub kind: PaneKind,
    pub rect: Rect,
    id: Id,
    pane_id: u64,
    layer: LayerId,
}
impl Region {
    pub fn ui(&self, ctx: &Context) -> Ui {
        let mut ui = Ui::new(
            ctx.clone(),
            self.id.with("content"),
            UiBuilder::new().layer_id(self.layer).max_rect(self.rect),
        );
        ui.set_clip_rect(self.rect);
        ui
    }
}

enum Action {
    Replace(u64, PaneKind),
    Split(u64, Split, PaneKind),
    Swap(u64, u64),
    Move(u64, u64, Split),
    Close(u64),
    Maximize(u64),
    Restore,
}

pub struct Workbench {
    docks: Vec<DockState<Pane>>,
    saved: Vec<Option<DockState<Pane>>>,
    workspace: usize,
    next_id: u64,
    maximized: Option<u64>,
    locked: bool,
    initialized: bool,
    regions: Vec<(PaneKind, Rect)>,
    focused: PaneKind,
    message: Option<String>,
    dragging: Option<u64>,
    drag_owned: bool,
    applied_views: Vec<(PaneKind, u64)>,
    persistence: persistence::Persistence,
    remember_requested: bool,
}

impl Default for Workbench {
    fn default() -> Self {
        let mut next_id = 0;
        Self {
            docks: (0..WORKSPACE_COUNT).map(|i| preset(i, &mut next_id)).collect(),
            saved: vec![None; WORKSPACE_COUNT],
            workspace: 0,
            next_id,
            maximized: None,
            locked: false,
            initialized: false,
            regions: Vec::new(),
            focused: PaneKind::Chart,
            message: None,
            dragging: None,
            drag_owned: false,
            applied_views: Vec::new(),
            persistence: Default::default(),
            remember_requested: false,
        }
    }
}

fn preset(workspace: usize, next_id: &mut u64) -> DockState<Pane> {
    let mut pane = |kind| {
        *next_id += 1;
        Pane { id: *next_id, kind, view: PaneView::default() }
    };
    let autoplay = workspace == 1;
    let mut dock = DockState::new(vec![pane(if workspace == 1 {
        PaneKind::Pad
    } else if workspace == 3 {
        PaneKind::Simai
    } else {
        PaneKind::Chart
    })]);
    if workspace == 1 {
        let [top, _] = dock.main_surface_mut().split_below(
            NodeIndex::root(),
            0.36,
            vec![pane(PaneKind::Autoplay)],
        );
        dock.main_surface_mut()
            .split_right(top, 0.66, vec![pane(PaneKind::Properties)]);
    } else {
        let [main, right] =
            dock.main_surface_mut()
                .split_right(NodeIndex::root(), 0.72, vec![pane(PaneKind::Pad)]);
        dock.main_surface_mut().split_below(
            main,
            if workspace == 2 { 0.60 } else if workspace == 3 { 0.76 } else { 0.90 },
            vec![pane(if workspace == 2 {
                PaneKind::Autoplay
            } else if workspace == 3 {
                PaneKind::Chart
            } else {
                PaneKind::Overview
            })],
        );
        dock.main_surface_mut()
            .split_below(right, 0.46, vec![pane(PaneKind::Properties)]);
    }
    // Presets only choose a layout and its options; the panes decide behaviour.
    if let Some((_, pane)) = dock
        .iter_all_tabs_mut()
        .find(|(_, pane)| pane.kind == PaneKind::Properties)
    {
        pane.view.properties_autoplay = autoplay;
    }
    dock
}

impl Workbench {
    pub fn initialize(&mut self, workspace: Workspace) -> Option<Workspace> {
        if !self.initialized {
            self.load_layouts();
            self.workspace = match std::env::var("MAI2_EDITOR_WORKSPACE").as_deref() {
                Ok("simai") => 3,
                Ok("combined") => 2,
                Ok("autoplay") => 1,
                Ok("chart") => 0,
                _ if workspace == Workspace::Autoplay => 1,
                _ => self.workspace,
            };
            self.initialized = true;
            self.focused = self.default_focus();
            if std::env::var_os("MAI2_EDITOR_TEMPLATES").is_some() {
                self.set_visible(PaneKind::Templates, true);
            }
            return Some(if self.workspace == 1 { Workspace::Autoplay } else { Workspace::Chart });
        }
        None
    }
    pub fn has(&self, kind: PaneKind) -> bool {
        self.docks[self.workspace]
            .iter_all_tabs()
            .any(|(_, p)| p.kind == kind)
    }
    /// Includes inactive workspaces and panes hidden by maximize.
    pub fn contains_anywhere(&self, kind: PaneKind) -> bool {
        self.docks.iter().any(|dock| dock.iter_all_tabs().any(|(_, pane)| pane.kind == kind))
    }
    pub fn visible(&self, kind: PaneKind) -> bool {
        self.docks[self.workspace]
            .iter_all_tabs()
            .any(|(_, p)| p.kind == kind && self.maximized.is_none_or(|id| p.id == id))
    }
    pub fn focused(&self, kind: PaneKind) -> bool { self.focused == kind && self.visible(kind) }

    fn default_focus(&self) -> PaneKind {
        match self.workspace { 1 => PaneKind::Autoplay, 3 => PaneKind::Simai, _ => PaneKind::Chart }
    }

    pub fn dragging(&self) -> bool { self.dragging.is_some() || self.drag_owned }

    pub fn cycle_focus(&mut self, reverse: bool) -> Option<Workspace> {
        let mut regions = self.regions.clone();
        regions.sort_by(|(_, a), (_, b)| a.top().total_cmp(&b.top()).then(a.left().total_cmp(&b.left())));
        if regions.is_empty() { return None; }
        let index = regions.iter().position(|(kind, _)| *kind == self.focused).unwrap_or(0);
        self.focused = regions[(index + if reverse { regions.len() - 1 } else { 1 }) % regions.len()].0;
        self.focused.workspace()
    }

    pub fn active_workspace(&mut self, ctx: &Context, current: Workspace) -> Workspace {
        let clicked = ctx.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        if let Some(kind) = clicked.and_then(|pos| {
            self.regions
                .iter()
                .find(|(_, r)| r.contains(pos))
                .map(|(k, _)| *k)
        }) {
            self.focused = kind;
            if let Some(workspace) = kind.workspace() {
                return workspace;
            }
        }
        if !self.visible(self.focused) {
            if let Some((_, pane)) = self.docks[self.workspace].iter_all_tabs()
                .find(|(_, p)| self.maximized.is_none_or(|id| id == p.id)) {
                self.focused = pane.kind;
            }
        }
        if !self.visible(PaneKind::Chart) && self.visible(PaneKind::Autoplay) {
            Workspace::Autoplay
        } else if !self.visible(PaneKind::Autoplay) && self.visible(PaneKind::Chart) {
            Workspace::Chart
        } else {
            current
        }
    }
    pub fn workspace_picker(&mut self, ui: &mut Ui, compact: bool) -> Option<Workspace> {
        let old = self.workspace;
        let labels = ["谱面", "Autoplay", "组合", "Simai"];
        if compact {
            ComboBox::from_id_salt("workspace")
                .width(80.0)
                .selected_text(labels[self.workspace])
                .show_ui(ui, |ui| {
                    for (index, label) in labels.iter().enumerate() {
                        ui.selectable_value(&mut self.workspace, index, *label);
                    }
                });
        } else {
            for (index, label) in labels.iter().enumerate() {
                let active = self.workspace == index;
                let r =
                    ui.button(RichText::new(*label).color(if active { TEXT } else { TEXT_DIM }));
                if r.clicked() {
                    self.workspace = index;
                }
                if active {
                    ui.painter().hline(
                        r.rect.x_range(),
                        r.rect.bottom() + 2.0,
                        Stroke::new(1.0_f32, ACCENT_BLUE),
                    );
                }
            }
        }
        if old == self.workspace {
            None
        } else {
            self.maximized = None;
            self.regions.clear();
            self.focused = self.default_focus();
            self.dragging = None;
            Some(if self.workspace == 1 {
                Workspace::Autoplay
            } else {
                Workspace::Chart
            })
        }
    }
    pub fn layout_menu(&mut self, ui: &mut Ui) {
        crate::ui::style::enabled_ui(ui, !self.locked, |ui| {
            ui.menu_button("添加分区", |ui| {
                for kind in PaneKind::ALL {
                    if ui
                        .add_enabled(!self.has(kind), Button::new(kind.title()))
                        .clicked()
                    {
                        self.add(kind);
                        ui.close_menu();
                    }
                }
            });
        });
        if ui.button("恢复完整布局").clicked() {
            self.maximized = None;
            ui.close_menu();
        }
        ui.separator();
        if ui
            .button("保存当前工作区布局")
            .on_hover_text("保存当前布局与分区配置快照；日常调整会自动记住")
            .clicked()
        {
            self.remember_requested = true;
            ui.close_menu();
        }
        if ui
            .add_enabled(
                !self.locked && self.saved[self.workspace].is_some(),
                Button::new("恢复已保存布局"),
            )
            .clicked()
        {
            self.restore_layout();
            ui.close_menu();
        }
        if ui
            .add_enabled(!self.locked, Button::new("恢复默认布局"))
            .clicked()
        {
            self.reset_layout();
            ui.close_menu();
        }
        ui.checkbox(&mut self.locked, "锁定布局");
        if self.persistence.failed() && ui.button("重试保存布局").clicked() {
            self.retry_layout_save();
        }
        if let Some(message) = &self.message { ui.label(message); }
    }
    pub fn restore_layout(&mut self) {
        if self.locked { return; }
        if let Some(saved) = &self.saved[self.workspace] {
            self.docks[self.workspace] = saved.clone();
            self.applied_views.clear();
            self.maximized = None;
            self.regions.clear();
        } else { self.message = Some("当前工作区没有已保存的布局".into()); }
    }
    pub fn reset_layout(&mut self) {
        if self.locked { return; }
        self.docks[self.workspace] = preset(self.workspace, &mut self.next_id);
        // Restoring the default also forgets this workspace's saved layout, so
        // the reset survives a restart instead of being undone by the file.
        self.saved[self.workspace] = None;
        self.message = Some(match self.save_layouts() {
            Ok(()) => "已恢复默认布局".into(),
            Err(error) => format!("恢复默认布局失败: {error}"),
        });
        self.applied_views.clear();
        self.maximized = None;
        self.regions.clear();
    }
    pub fn maximize_focused(&mut self) {
        if self.maximized.take().is_some() { return; }
        self.maximized = self.docks[self.workspace].iter_all_tabs()
            .find(|(_, p)| p.kind == self.focused).map(|(_, p)| p.id);
    }
    pub fn set_visible(&mut self, kind: PaneKind, visible: bool) {
        if visible {
            if !self.has(kind) {
                self.add(kind);
            }
        } else {
            let id = self.docks[self.workspace]
                .iter_all_tabs()
                .find(|(_, p)| p.kind == kind)
                .map(|(_, p)| p.id);
            if let Some(id) = id {
                self.apply(Action::Close(id));
            }
        }
    }
    fn location(&self, id: u64) -> Option<(SurfaceIndex, NodeIndex)> {
        self.docks[self.workspace]
            .iter_all_tabs()
            .find_map(|(at, pane)| (pane.id == id).then_some(at))
    }
    fn add(&mut self, kind: PaneKind) {
        let dock = &mut self.docks[self.workspace];
        let id = dock
            .iter_all_tabs()
            .find(|(_, p)| p.kind == self.focused)
            .map(|(_, p)| p.id)
            .or_else(|| dock.iter_all_tabs().next().map(|(_, p)| p.id));
        if let Some(id) = id {
            self.apply(Action::Split(id, Split::Right, kind));
        }
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::Maximize(id) => self.maximized = Some(id),
            Action::Restore => self.maximized = None,
            _ if self.locked => {}
            action => {
                self.maximized = None;
                match action {
                    Action::Replace(id, kind) => {
                        let other = self.docks[self.workspace]
                            .iter_all_tabs()
                            .find(|(_, p)| p.kind == kind)
                            .map(|(_, p)| p.id);
                        if let Some(other) = other {
                            self.apply(Action::Swap(id, other));
                        } else if let Some((_, p)) = self.docks[self.workspace]
                            .iter_all_tabs_mut()
                            .find(|(_, p)| p.id == id)
                        {
                            p.kind = kind;
                            p.view = PaneView::default();
                        }
                    }
                    Action::Split(id, split, kind) if !self.has(kind) => {
                        if let Some((surface, node)) = self.location(id) {
                            self.next_id += 1;
                            self.docks[self.workspace][surface].split_tabs(
                                node,
                                split,
                                0.5,
                                vec![Pane {
                                    id: self.next_id,
                                    kind,
                                    view: PaneView::default(),
                                }],
                            );
                        }
                    }
                    Action::Swap(a, b) => {
                        if let (Some((sa, na)), Some((sb, nb))) =
                            (self.location(a), self.location(b))
                        {
                            let first =
                                self.docks[self.workspace][sa][na].tabs().unwrap()[0].clone();
                            let second =
                                self.docks[self.workspace][sb][nb].tabs().unwrap()[0].clone();
                            self.docks[self.workspace][sa][na].tabs_mut().unwrap()[0] = second;
                            self.docks[self.workspace][sb][nb].tabs_mut().unwrap()[0] = first;
                        }
                    }
                    Action::Move(source, target, direction) if source != target => {
                        if let (Some((ss, sn)), Some((ts, tn))) = (self.location(source), self.location(target)) {
                            self.docks[self.workspace].move_tab(
                                (ss, sn, TabIndex(0)),
                                TabDestination::Node(ts, tn, TabInsert::Split(direction)),
                            );
                        }
                    }
                    Action::Close(id) if self.docks[self.workspace].iter_all_tabs().count() > 1 => {
                        if let Some((s, n)) = self.location(id) {
                            self.docks[self.workspace].remove_tab((s, n, TabIndex(0)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn show(
        &mut self,
        ctx: &Context,
        enabled: bool,
        mut tools: impl FnMut(&mut Ui, PaneKind),
    ) -> Vec<Region> {
        let areas: Vec<_> = self.docks[self.workspace]
            .iter_all_tabs()
            .map(|(_, p)| (p.id, p.kind))
            .collect();
        let mut actions = Vec::new();
        let mut regions = Vec::new();
        self.drag_owned = self.dragging.is_some();
        // Transparent bodies let the macroquad chart and pad remain visible below egui.
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            // Headers own independent UIs on this layer and handle their own
            // disabled appearance; do not post-process their shapes twice.
            ui.add_enabled_ui(enabled, |ui| {
            let mut viewer = Viewer {
                regions: &mut regions,
                actions: &mut actions,
                areas: &areas,
                locked: self.locked || !enabled,
                tools: &mut tools,
                enabled,
                focused: self.focused,
                dragging: &mut self.dragging,
            };
            if let Some(id) = self.maximized {
                if let Some((_, p)) = self.docks[self.workspace]
                    .iter_all_tabs_mut()
                    .find(|(_, p)| p.id == id)
                {
                    let (_, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    viewer.header(p, &response, true);
                    viewer.ui(ui, p);
                }
            } else {
                let style = dock_style(ui.style());
                DockArea::new(&mut self.docks[self.workspace])
                    .id(Id::new(("editor-workbench", self.workspace)))
                    .style(style)
                    .show_add_buttons(false)
                    .show_close_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .draggable_tabs(false)
                    .tab_context_menus(false)
                    .show_inside(ui, &mut viewer);
            }
            });
        });
        self.finish_drag(ctx, &regions, enabled, &mut actions);
        self.regions = regions
            .iter()
            .map(|r| {
                (
                    r.kind,
                    Rect::from_min_max(r.rect.min - vec2(0.0, 26.0), r.rect.max),
                )
            })
            .collect();
        for action in actions {
            self.apply(action);
        }
        regions
    }
}

struct Viewer<'a, F> {
    regions: &'a mut Vec<Region>,
    actions: &'a mut Vec<Action>,
    areas: &'a [(u64, PaneKind)],
    locked: bool,
    tools: &'a mut F,
    enabled: bool,
    focused: PaneKind,
    dragging: &'a mut Option<u64>,
}
impl<F: FnMut(&mut Ui, PaneKind)> Viewer<'_, F> {
    fn header(&mut self, pane: &Pane, response: &Response, maximized: bool) {
        let mut ui = Ui::new(
            response.ctx.clone(),
            response.id.with("header"),
            UiBuilder::new()
                .layer_id(response.layer_id)
                .max_rect(response.rect.shrink2(vec2(4.0, 1.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        ui.set_clip_rect(response.rect);
        ui.painter().rect_filled(response.rect, 0, surface(BG_TOOLBAR));
        if pane.kind == self.focused {
            ui.painter().hline(response.rect.x_range(), response.rect.bottom() - 1.0, Stroke::new(1.0_f32, ACCENT_BLUE.gamma_multiply(0.55)));
        }
        ui.spacing_mut().item_spacing.x = 3.0;
        if !self.locked && !maximized && response.drag_started_by(PointerButton::Primary) {
            *self.dragging = Some(pane.id);
        }
        crate::ui::style::enabled_ui(&mut ui, !self.locked, |ui| {
            ui.menu_button(glyph(pane.kind.icon(), 13.0), |ui| {
                for kind in PaneKind::ALL {
                    if ui
                        .selectable_label(pane.kind == kind, kind.title())
                        .clicked()
                    {
                        self.actions.push(Action::Replace(pane.id, kind));
                        ui.close_menu();
                    }
                }
            })
            .response
            .on_hover_text("分区类型 / 已显示的类型会交换位置");
        });
        let size = vec2((ui.available_width() - 49.0).max(0.0), 24.0);
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_size(size);
            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
            if ui.available_width() > 105.0 {
                let title = ui.add(Label::new(RichText::new(pane.kind.title()).color(TEXT))
                    .truncate().sense(if self.locked || maximized { Sense::click() } else { Sense::click_and_drag() }));
                if title.double_clicked() && self.enabled {
                    self.actions.push(if maximized { Action::Restore } else { Action::Maximize(pane.id) });
                }
                if !self.locked && !maximized {
                    if title.drag_started_by(PointerButton::Primary) { *self.dragging = Some(pane.id); }
                    title.on_hover_cursor(CursorIcon::Grab).on_hover_text("拖动分区；边缘拆分，中央交换");
                }
                ui.add_space(6.0);
            }
            crate::ui::style::enabled_ui(ui, self.enabled, |ui| (self.tools)(ui, pane.kind));
        });
        crate::ui::style::enabled_ui(&mut ui, self.enabled, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(glyph(Icon::Ellipsis, 14.0), |ui| {
                    area_menu(ui, pane, self.actions, self.areas, self.locked, maximized)
                })
                .response
                .on_hover_text("分区布局");
                if icon(
                    ui,
                    if maximized {
                        Icon::Minimize2
                    } else {
                        Icon::Maximize2
                    },
                    "最大化 / 恢复分区",
                    false,
                )
                .clicked()
                {
                    self.actions.push(if maximized {
                        Action::Restore
                    } else {
                        Action::Maximize(pane.id)
                    });
                }
            })
        });
        if response.double_clicked() && self.enabled {
            self.actions.push(if maximized {
                Action::Restore
            } else {
                Action::Maximize(pane.id)
            });
        }
        if self.enabled {
            response.context_menu(|ui| {
                area_menu(ui, pane, self.actions, self.areas, self.locked, maximized)
            });
        }
    }
}
impl<F: FnMut(&mut Ui, PaneKind)> TabViewer for Viewer<'_, F> {
    type Tab = Pane;
    fn title(&mut self, _: &mut Pane) -> WidgetText {
        "".into()
    }
    fn id(&mut self, tab: &mut Pane) -> Id {
        Id::new(("editor-pane", tab.id))
    }
    fn ui(&mut self, ui: &mut Ui, pane: &mut Pane) {
        let rect = ui.available_rect_before_wrap();
        self.regions.push(Region {
            kind: pane.kind,
            rect,
            id: ui.id(),
            pane_id: pane.id,
            layer: ui.layer_id(),
        });
        ui.allocate_rect(rect, Sense::hover());
    }
    fn scroll_bars(&self, _: &Pane) -> [bool; 2] {
        [false, false]
    }
    fn closeable(&mut self, _: &mut Pane) -> bool {
        false
    }
    fn allowed_in_windows(&self, _: &mut Pane) -> bool {
        false
    }
    fn on_tab_button(&mut self, pane: &mut Pane, response: &Response) {
        self.header(pane, response, false);
    }
}

fn area_menu(
    ui: &mut Ui,
    pane: &Pane,
    actions: &mut Vec<Action>,
    areas: &[(u64, PaneKind)],
    locked: bool,
    maximized: bool,
) {
    crate::ui::style::enabled_ui(ui, !locked, |ui| {
        for (label, direction) in [("向右拆分", Split::Right), ("向下拆分", Split::Below)] {
            ui.menu_button(label, |ui| {
                for kind in PaneKind::ALL {
                    if ui
                        .add_enabled(
                            !areas.iter().any(|(_, k)| *k == kind),
                            Button::new(kind.title()),
                        )
                        .clicked()
                    {
                        actions.push(Action::Split(pane.id, direction, kind));
                        ui.close_menu();
                    }
                }
            });
        }
        ui.menu_button("交换位置", |ui| {
            for &(id, kind) in areas {
                if id != pane.id && ui.button(kind.title()).clicked() {
                    actions.push(Action::Swap(pane.id, id));
                    ui.close_menu();
                }
            }
        });
    });
    ui.separator();
    if ui
        .button(if maximized {
            "恢复分区"
        } else {
            "最大化分区"
        })
        .clicked()
    {
        actions.push(if maximized {
            Action::Restore
        } else {
            Action::Maximize(pane.id)
        });
        ui.close_menu();
    }
    if ui
        .add_enabled(!locked && areas.len() > 1, Button::new("关闭分区"))
        .clicked()
    {
        actions.push(Action::Close(pane.id));
        ui.close_menu();
    }
}
